use eframe::egui;
use pdfium_render::prelude::*;
use rfd::FileDialog;
use std::sync::Arc;

fn main() -> eframe::Result<()> {
    let native_options = eframe::NativeOptions::default();
    eframe::run_native(
        "FlipApp - Rust PDF Viewer",
        native_options,
        Box::new(|cc| Box::new(PdfApp::new(cc))),
    )
}

struct PdfApp {
    pdfium: Arc<Pdfium>,
    textures: Vec<egui::TextureHandle>,
    current_page: usize,
    max_pages: usize,
    current_path: Option<std::path::PathBuf>,
    // Animation fields
    old_textures: Vec<egui::TextureHandle>,
    transition_start_time: f64,
    is_animating: bool,
    direction: f32,
}

impl PdfApp {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let pdfium = Pdfium::new(
            Pdfium::bind_to_system_library().expect("Failed to find Pdfium library")
        );
        
        Self {
            pdfium: Arc::new(pdfium),
            textures: Vec::new(),
            current_page: 0,
            max_pages: 0,
            current_path: None,
            old_textures: Vec::new(),
            transition_start_time: 0.0,
            is_animating: false,
            direction: 0.0,
        }
    }

    fn render_current_spread(&mut self, ctx: &egui::Context) {
        let Some(path) = &self.current_path else { return };
        if let Ok(document) = self.pdfium.load_pdf_from_file(path, None) {
            let mut new_textures = Vec::new();
            let pages_to_show = if self.current_page == 0 { 1 } else { 2 };

            for offset in 0..pages_to_show {
                let page_idx = self.current_page + offset;
                if let Ok(page) = document.pages().get(page_idx as u16) {
                    let render_config = PdfRenderConfig::new().set_target_height(1200);
                    if let Ok(bitmap) = page.render_with_config(&render_config) {
                        let mut pixels = bitmap.as_raw_bytes().to_vec();
                        for chunk in pixels.chunks_exact_mut(4) { chunk.swap(0, 2); }
                        let color_image = egui::ColorImage::from_rgba_unmultiplied(
                            [bitmap.width() as usize, bitmap.height() as usize],
                            &pixels,
                        );
                        let tex_name = format!("page_{}_{}", path.display(), page_idx);
                        new_textures.push(ctx.load_texture(tex_name, color_image, Default::default()));
                    }
                }
            }
            if !new_textures.is_empty() { self.textures = new_textures; }
        }
    }

    // Helper now inside the impl block
    fn render_spread(
        &self, 
        ui: &mut egui::Ui, 
        textures: &[egui::TextureHandle], 
        old_textures: &[egui::TextureHandle],
        height: f32, 
        total_width: f32, 
        x_shift: f32,
        progress: f32, 
        is_old: bool
    ) {
        if textures.is_empty() { return; }
        
        let mut content_width = 0.0;
        for tex in textures {
            content_width += height * (tex.size_vec2().x / tex.size_vec2().y);
        }
        let x_offset = (total_width - content_width) / 2.0;
        let painter = ui.painter();
        let rect = ui.max_rect();

        for (i, texture) in textures.iter().enumerate() {
            let aspect = texture.size_vec2().x / texture.size_vec2().y;
            let page_width = height * aspect;
            let start_x = x_offset + (i as f32 * page_width) + x_shift;
            
            let should_curl = (is_old && self.direction > 0.0 && i == 1) || 
                            (!is_old && self.direction < 0.0 && i == 0);

            if should_curl {
                // SAFE BOUNDS CHECKING
                let back_texture = if self.direction > 0.0 {
                    // Forward: Back of old page 1 is new page 0
                    if !textures.is_empty() { &textures[0] } else { texture }
                } else {
                    // Backward: Back of new page 0 is old page 1
                    // This is where your panic was: old_textures[1]
                    if old_textures.len() > 1 { &old_textures[1] } else { texture }
                };

                let anchor_x = if self.direction < 0.0 { 
                    start_x + page_width 
                } else { 
                    start_x 
                };
                self.draw_curled_page(painter, texture, back_texture, anchor_x, height, page_width, progress, rect);
            } else {
                let page_rect = egui::Rect::from_min_size(
                    egui::pos2(start_x, rect.min.y), 
                    egui::vec2(page_width, height)
                );
                painter.image(texture.id(), page_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
            }
        }
    }

    // NEW FUNCTION GOES HERE
    fn draw_curled_page(&self, painter: &egui::Painter, front_tex: &egui::TextureHandle, back_tex: &egui::TextureHandle, x: f32, h: f32, w: f32, p: f32, rect: egui::Rect) {
        let strips = 50;
        let y_min = rect.min.y;

        for s in 0..strips {
            let s_f = s as f32 / strips as f32;
            let next_s_f = (s + 1) as f32 / strips as f32;

            // PROGRESS LOGIC:
            // Forward: Angle goes 0 -> PI.
            // Backward: Angle goes PI -> 0.
            let angle = if self.direction > 0.0 {
                p * std::f32::consts::PI
            } else {
                (1.0 - p) * std::f32::consts::PI
            };

            let cos_angle = angle.cos();
            let sin_angle = angle.sin();

            // If flipping Right Page: Move POSITIVE x (multiplier 1.0)
            // If flipping Left Page: Move NEGATIVE x (multiplier -1.0)
            let multiplier = if self.direction < 0.0 { -1.0 } else { 1.0 };

            let x_pos = x + (s_f * w * cos_angle * multiplier);
            let next_x_pos = x + (next_s_f * w * cos_angle * multiplier);
            let bend = (s_f * std::f32::consts::PI).sin() * sin_angle * 100.0;

            // Texture Swap: When angle is between 90 and 180, we see the back
            let is_back_side = angle.abs() > std::f32::consts::FRAC_PI_2;
            let current_tex_id = if is_back_side { back_tex.id() } else { front_tex.id() };
            
            let mut mesh = egui::Mesh::with_texture(current_tex_id);
            
            // UV Mirroring
            let (uv_start, uv_end) = if is_back_side { (1.0 - s_f, 1.0 - next_s_f) } else { (s_f, next_s_f) };
            let shade = (255.0 - (sin_angle * 60.0)) as u8;

            let idx = mesh.vertices.len() as u32;
            mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x_pos, y_min - bend), uv: egui::pos2(uv_start, 0.0), color: egui::Color32::from_rgb(shade, shade, shade) });
            mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(next_x_pos, y_min - bend), uv: egui::pos2(uv_end, 0.0), color: egui::Color32::from_rgb(shade, shade, shade) });
            mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(next_x_pos, y_min + h - bend), uv: egui::pos2(uv_end, 1.0), color: egui::Color32::from_rgb(shade, shade, shade) });
            mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x_pos, y_min + h - bend), uv: egui::pos2(uv_start, 1.0), color: egui::Color32::from_rgb(shade, shade, shade) });

            mesh.add_triangle(idx, idx + 1, idx + 2);
            mesh.add_triangle(idx, idx + 2, idx + 3);
            painter.add(mesh);
        }
    }
} // <--- End of PdfApp impl

impl eframe::App for PdfApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let mut re_render_requested = false;
        let mut new_direction = 0.0;

        // 1. INPUT HANDLING
        if !self.is_animating {
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
                    if self.current_page + 1 < self.max_pages {
                        new_direction = 1.0;
                        re_render_requested = true;
                    }
                }
                if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                    if self.current_page > 0 {
                        new_direction = -1.0;
                        re_render_requested = true;
                    }
                }
            });
        }

        // 2. ANIMATION TRIGGER
        if re_render_requested {
            // 1. Capture the current screen as 'old'
            self.old_textures = self.textures.clone();
            
            // 2. Update page index
            if new_direction > 0.0 {
                self.current_page = (self.current_page + 2).min(self.max_pages.saturating_sub(1));
            } else {
                self.current_page = self.current_page.saturating_sub(2);
            }

            // 3. RENDER NEW TEXTURES IMMEDIATELY
            // This ensures self.textures is NOT empty when the animation starts or ends
            self.render_current_spread(ctx); 

            self.direction = new_direction;
            self.transition_start_time = ctx.input(|i| i.time);
            self.is_animating = true;
        }

        // 3. TOP PANEL
        egui::TopBottomPanel::top("controls").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open PDF").clicked() {
                    if let Some(path) = rfd::FileDialog::new()
                        .add_filter("PDF", &["pdf"])
                        .pick_file() 
                    {
                        // 1. Update the path and reset page count
                        self.current_path = Some(path.clone());
                        self.current_page = 0;

                        // 2. Load the document to get the total page count
                        if let Ok(doc) = self.pdfium.load_pdf_from_file(&path, None) {
                            self.max_pages = doc.pages().len() as usize;
                        }

                        // 3. Clear existing textures and animation state
                        self.textures.clear();
                        self.old_textures.clear();
                        self.is_animating = false;

                        // 4. Trigger the initial render for the first page
                        self.render_current_spread(ctx);
                        
                        println!("Loaded: {}", path.display());
                    }
                }
                ui.separator();
                ui.label(format!("Page {} of {}", self.current_page + 1, self.max_pages));
            });
        });

        // 4. CENTRAL PANEL
        egui::CentralPanel::default().show(ctx, |ui| {
                    if self.textures.is_empty() {
                        ui.centered_and_justified(|ui| {
                            ui.label("No PDF loaded.");
                        });
                    } else {
                        let available_height = ui.available_height();
                        let available_width = ui.available_width();
                        let animation_duration = 0.7;
                        let time_since_start = ctx.input(|i| i.time) - self.transition_start_time;
                        let progress = (time_since_start / animation_duration).min(1.0) as f32;
                        
                        // Cubic Out Easing
                        let t = 1.0 - progress;
                        let ease_progress = 1.0 - (t * t * t);

                       if self.is_animating && progress < 1.0 {
                            ctx.request_repaint();
                            let ease_progress = 1.0 - (1.0 - progress).powi(3);

                            if self.direction > 0.0 {
                                // Right Flip: New spread (static underneath), Old page (curling on top)
                                self.render_spread(ui, &self.textures, &self.old_textures, available_height, available_width, 0.0, 0.0, false);
                                self.render_spread(ui, &self.textures, &self.old_textures, available_height, available_width, 0.0, ease_progress, true);
                            } else {
                                // Left Flip: Old spread (static underneath), New page (curling on top)
                                self.render_spread(ui, &self.old_textures, &self.textures, available_height, available_width, 0.0, 0.0, true);
                                self.render_spread(ui, &self.textures, &self.old_textures, available_height, available_width, 0.0, ease_progress, false);
                            }
                        } else {
                            // ANIMATION FINISHED: Clear the 'old' state and draw the new spread flat
                            self.is_animating = false;
                            self.old_textures.clear(); // Safety: free up memory on your Optiplex
                            
                            // Draw the current textures (both pages) as a standard flat spread
                            // Ensure x_shift and progress are 0.0, and is_old is false
                            self.render_spread(ui, &self.textures, &self.textures, available_height, available_width, 0.0, 0.0, false);
                        }
                    }
                });
    } // Closing update
}