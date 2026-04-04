mod app;
mod scripting;
mod views;

use app::KilnApp;

/// Kiln GUI — Interactive Disassembler
///
/// Main entry point for the Kiln GUI application.
fn main() -> eframe::Result<()> {
    env_logger::init();

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1280.0, 800.0])
            .with_min_inner_size([800.0, 600.0])
            .with_title("Kiln — Interactive Disassembler"),
        ..Default::default()
    };

    eframe::run_native(
        "Kiln",
        options,
        Box::new(|cc| Ok(Box::new(KilnApp::new(cc)))),
    )
}
