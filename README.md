WIP
# FlipApp

A lightweight, high-performance Rust-based PDF reader built with `eframe` and powered by PDFium.

---

## Prerequisites

Before building FlipApp, ensure you have the following installed:
* **Rust Toolchain** (Stable): [Install via rustup](https://rustup.rs/)
* **PDFium Binary**: FlipApp requires the PDFium library to render documents. 
  * **Linux**: `libpdfium.so` (included or placed in the project root).
  * **Windows**: `pdfium.dll` (required alongside the executable).

---

## Building on Linux

### Method 1: Native Cargo Build
1. Clone the repository and navigate into the project directory:
   ```bash
   git clone [https://github.com/JPRock54/FlipApp-rs.git](https://github.com/JPRock54/FlipApp-rs.git)
   cd FlipApp-rs
2. Ensure libpdfium.so is present in the project root.
3. Build the release binary:
   ```bash
   cargo build --release
4. The compiled binary will be located at target/release/FlipApp.

### Method 2: Flatpak Build (Recommended for Distribution)
1. Install flatpak and flatpak-builder if you haven't already.
2. Build and export the app to a local repository:
   ```bash
   flatpak-builder --user --force-clean --repo=repo build-dir org.example.FlipApp.yml
3. Install the app locally:
   ```bash
   flatpak --user install --reinstall local-repo org.example.FlipApp
4. Run the application (Also can be run from desktop entry)
   ```bash
   flatpak run org.example.FlipApp

## Building on Windows
1. Open PowerShell or Command Prompt and clone/navigate to the repository:
   ```bash
   git clone [https://github.com/JPRock54/FlipApp-rs.git](https://github.com/JPRock54/FlipApp-rs.git)
   cd FlipApp-rs
2. Build the release version using Cargo:
   ```bash
   cargo build --release
3. Copy pdfium.dll into the output folder (target/release/) right next to FlipApp.exe before running it.