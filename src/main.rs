use eframe::egui;
use pdfium_render::prelude::*;
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
            
            // If we are on page 0, only show 1 page (the cover). 
            // Otherwise, show 2 pages (the spread).
            let pages_to_show = if self.current_page == 0 { 1 } else { 2 };

            for offset in 0..pages_to_show {
                let page_idx = self.current_page + offset;
                if page_idx >= self.max_pages { break; }

                if let Ok(page) = document.pages().get(page_idx as u16) {
                    let render_config = PdfRenderConfig::new().set_target_height(1200);
                    if let Ok(bitmap) = page.render_with_config(&render_config) {
                        let pixels = bitmap.as_raw_bytes().to_vec();
                        
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
            other_textures: &[egui::TextureHandle],
            height: f32, 
            total_width: f32, 
            x_shift: f32,
            progress: f32, 
            is_old: bool,
            is_foreground: bool,
            is_right_aligned: bool, // <--- NEW PARAMETER
        ) {
            if textures.is_empty() { return; }
            
            let is_single_page = textures.len() == 1;
            
            let content_width = if is_single_page {
                (height * (textures[0].size_vec2().x / textures[0].size_vec2().y)) * 2.0
            } else {
                let mut w = 0.0;
                for tex in textures {
                    w += height * (tex.size_vec2().x / tex.size_vec2().y);
                }
                w
            };

            let x_offset = (total_width - content_width) / 2.0;
            let painter = ui.painter();
            let rect = ui.max_rect();

            for (i, texture) in textures.iter().enumerate() {
                let aspect = texture.size_vec2().x / texture.size_vec2().y;
                let page_width = height * aspect;
                
                // 2. Position the page
                // Shift to the right slot ONLY if it's a cover/right-aligned
                let start_x = if is_single_page && is_right_aligned {
                    x_offset + page_width + x_shift
                } else {
                    x_offset + (i as f32 * page_width) + x_shift
                };
                
                // 3. Determine if this specific page should curl
                let is_right_page = i == 1 || (is_single_page && is_right_aligned);
                let should_curl = (is_old && self.direction > 0.0 && is_right_page) || 
                                (is_old && self.direction < 0.0 && i == 0);

                if should_curl && is_foreground {
                    // ... (Keep your existing back_texture logic here) ...
                    let back_texture = if self.direction > 0.0 {
                        if !other_textures.is_empty() { &other_textures[0] } else { texture }
                    } else {
                        if other_textures.len() > 1 { 
                            &other_textures[1] 
                        } else if !other_textures.is_empty() { 
                            &other_textures[0] 
                        } else { 
                            texture 
                        }
                    };

                    // 4. Set the Anchor (The Hinge)
                    // Left pages hinge on their right edge (start_x + width). 
                    // Right pages hinge on their left edge (start_x).
                    let anchor_x = if i == 0 && !(is_single_page && is_right_aligned) {
                        start_x + page_width
                    } else {
                        start_x
                    };
                    
                    self.draw_curled_page(painter, texture, back_texture, anchor_x, height, page_width, progress, rect);
                } else if !is_foreground {
                // ... (Keep your existing background draw logic here) ...
                let page_rect = egui::Rect::from_min_size(
                        egui::pos2(start_x, rect.min.y), 
                        egui::vec2(page_width, height)
                    );
                    painter.image(texture.id(), page_rect, egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0)), egui::Color32::WHITE);
                }
            }
        }

    // NEW FUNCTION GOES HERE
    fn draw_curled_page(
        &self, 
        painter: &egui::Painter, 
        front_tex: &egui::TextureHandle, 
        back_tex: &egui::TextureHandle, 
        x: f32, // The anchor_x (the spine)
        h: f32, 
        w: f32, 
        p: f32, 
        rect: egui::Rect
    ) {
        let strips = 50;
        let y_min = rect.min.y;

        for s in 0..strips {
            let s_f = s as f32 / strips as f32;
            let next_s_f = (s + 1) as f32 / strips as f32;

            // Angle Logic:
            // Forward (Right Page): 0 -> PI. Starts at 0 (Right), ends at PI (Left).
            // Backward (Left Page): PI -> 0. Starts at PI (Left), ends at 0 (Right).
            let angle = if self.direction > 0.0 {
                    p * std::f32::consts::PI
                } else {
                    (1.0 - p) * std::f32::consts::PI
                };

                let cos_angle = angle.cos();
                let sin_angle = angle.sin();

                // 1. DYNAMIC BACK SIDE DETECTION
                // Right-to-Left flip (direction > 0): Left side of spine (cos < 0) is the back.
                // Left-to-Right flip (direction < 0): Right side of spine (cos > 0) is the back.
                let is_back_side = if self.direction > 0.0 {
                    cos_angle < 0.0
                } else {
                    cos_angle > 0.0
                };
                
                let current_tex_id = if is_back_side { back_tex.id() } else { front_tex.id() };
                
                let mut mesh = egui::Mesh::with_texture(current_tex_id);

                let x_pos = x + (s_f * w * cos_angle);
                let next_x_pos = x + (next_s_f * w * cos_angle);
                
                let bend = (s_f * std::f32::consts::PI).sin() * sin_angle * 80.0;

                // 2. DYNAMIC UV MAPPING
                // s_f = 0.0 is ALWAYS the hinge (spine). 
                // Right pages (and the back of left pages) have their spine at the left edge of the image (UV 0.0)
                // Left pages (and the back of right pages) have their spine at the right edge of the image (UV 1.0)
                let (uv_start, uv_end) = if self.direction > 0.0 {
                    if is_back_side {
                        (1.0 - s_f, 1.0 - next_s_f) 
                    } else {
                        (s_f, next_s_f) 
                    }
                } else {
                    if is_back_side {
                        (s_f, next_s_f) 
                    } else {
                        (1.0 - s_f, 1.0 - next_s_f) 
                    }
                };

            let shade = (255.0 - (sin_angle * 50.0)) as u8;
            let color = egui::Color32::from_rgb(shade, shade, shade);

            let idx = mesh.vertices.len() as u32;
            mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x_pos, y_min - bend), uv: egui::pos2(uv_start, 0.0), color });
            mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(next_x_pos, y_min - bend), uv: egui::pos2(uv_end, 0.0), color });
            mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(next_x_pos, y_min + h - bend), uv: egui::pos2(uv_end, 1.0), color });
            mesh.vertices.push(egui::epaint::Vertex { pos: egui::pos2(x_pos, y_min + h - bend), uv: egui::pos2(uv_start, 1.0), color });

            mesh.add_triangle(idx, idx + 1, idx + 2);
            mesh.add_triangle(idx, idx + 2, idx + 3);
            
            painter.add(mesh);
        }
    }
} // <--- End of PdfApp impl

impl eframe::App for PdfApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        let re_render_requested = false;
        let new_direction = 0.0;

        let mut trigger_animation = false;
        let mut new_dir = 0.0; // This fixes the E0425 erro
        // 1. INPUT HANDLING
        if !self.is_animating {
            ctx.input_mut(|i| {
                if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
                    if self.current_page + 1 < self.max_pages {
                        new_dir = 1.0;
                        trigger_animation = true;
                    }
                } else if i.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
                    if self.current_page > 0 {
                        new_dir = -1.0;
                        trigger_animation = true;
                    }
                }
            });
        }

        if trigger_animation {
            self.old_textures = self.textures.clone();
            self.direction = new_dir;
            
            // PAGINATION: Centered Cover (0) -> Spread (1,2) -> Spread (3,4)
            if new_dir > 0.0 {
                // Forward: 0 to 1, then increment by 2
                if self.current_page == 0 {
                    self.current_page = 1;
                } else {
                    self.current_page = (self.current_page + 2).min(self.max_pages.saturating_sub(1));
                }
            } else {
                // Backward: decrement by 2, but 1 goes to 0
                if self.current_page == 1 {
                    self.current_page = 0;
                } else {
                    self.current_page = self.current_page.saturating_sub(2);
                }
            }

            self.render_current_spread(ctx); 
            self.transition_start_time = ctx.input(|i| i.time);
            self.is_animating = true;
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
                
                let animation_duration = 0.6; 
                let time_since_start = ctx.input(|i| i.time) - self.transition_start_time;
                let progress = (time_since_start / animation_duration).min(1.0) as f32;
                
                // Cubic Out Easing: Smooth deceleration
                let t = 1.0 - progress;
                let ease_progress = 1.0 - (t * t * t);

                if self.is_animating && progress < 1.0 {
                    ctx.request_repaint();

                    if self.direction > 0.0 {
                        // --- RIGHT FLIP (Forward) ---
                        if self.old_textures.len() == 1 {
                            // CASE: Opening the Cover
                            if self.textures.len() > 1 {
                                let background_spread = [self.textures[1].clone()];
                                // true = Cover acts as a Right page
                                self.render_spread(ui, &background_spread, &self.old_textures, available_height, available_width, 0.0, 0.0, false, false, true);
                            }
                            self.render_spread(ui, &self.old_textures, &self.textures, available_height, available_width, 0.0, ease_progress, true, true, true);
                        } else {
                            // CASE: Standard Spread Flip OR End of Book
                            if self.old_textures.len() > 0 {
                                if self.textures.len() > 1 {
                                    let background_spread = [self.old_textures[0].clone(), self.textures[1].clone()];
                                    self.render_spread(ui, &background_spread, &self.old_textures, available_height, available_width, 0.0, 0.0, false, false, false);
                                } else {
                                    // FIX: We reached the final single page. Keep the old left page rendered in the background!
                                    let background_spread = [self.old_textures[0].clone()];
                                    // false = Final page acts as a Left page
                                    self.render_spread(ui, &background_spread, &self.old_textures, available_height, available_width, 0.0, 0.0, false, false, false); 
                                }
                            }
                            self.render_spread(ui, &self.old_textures, &self.textures, available_height, available_width, 0.0, ease_progress, true, true, false);
                        }
                    } else {
                        // --- LEFT FLIP (Backward) ---
                        if self.textures.len() == 1 {
                            // CASE: Closing to Cover
                            if self.old_textures.len() > 1 {
                                let background_spread = [self.old_textures[1].clone()];
                                self.render_spread(ui, &background_spread, &self.old_textures, available_height, available_width, 0.0, 0.0, false, false, true);
                            }
                            self.render_spread(ui, &self.old_textures, &self.textures, available_height, available_width, 0.0, ease_progress, true, true, false);
                        } else {
                            // CASE: Standard Backward Flip OR Flipping back FROM the last page
                            if self.textures.len() > 0 {
                                if self.old_textures.len() > 1 {
                                    let background_spread = [self.textures[0].clone(), self.old_textures[1].clone()];
                                    self.render_spread(ui, &background_spread, &self.old_textures, available_height, available_width, 0.0, 0.0, true, false, false);
                                } else {
                                    // FIX: Flipping backward FROM a single last page. Put the new left page in the background.
                                    let background_spread = [self.textures[0].clone()];
                                    self.render_spread(ui, &background_spread, &self.old_textures, available_height, available_width, 0.0, 0.0, true, false, false);
                                }
                            }
                            self.render_spread(ui, &self.old_textures, &self.textures, available_height, available_width, 0.0, ease_progress, true, true, false);
                        }
                    }
                } else {
                    // --- STATIC STATE ---
                    self.is_animating = false;
                    if !self.old_textures.is_empty() {
                        self.old_textures.clear();
                    }
                    
                    // The only time a single page is right-aligned in static mode is if it's the front cover.
                    let is_cover = self.current_page == 0;
                    self.render_spread(ui, &self.textures, &self.textures, available_height, available_width, 0.0, 0.0, false, false, is_cover);
                }
            }
        });
    } // Closing update
}