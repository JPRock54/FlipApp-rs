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