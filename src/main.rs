// Declare the new modules
mod ui;
mod video_reader;

use eframe::egui;
// Import the MyApp struct from the app module
use ui::MyApp;

fn main() -> eframe::Result<()> {
    // Configure the application's native options, like window size
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default().with_inner_size([800.0, 600.0]),
        ..Default::default()
    };

    // Run the eframe application
    // MyApp::default() will now create an instance of the refactored application
    eframe::run_native(
        "Object detection evaluator",
        options,
        Box::new(|_cc| Ok(Box::new(MyApp::default()))),
    )
}
