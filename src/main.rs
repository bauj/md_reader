mod app;
mod fs;
mod markdown;
mod persist;
mod ui;

use app::App;

fn main() -> Result<(), eframe::Error> {
    let initial_path = std::env::args().nth(1).map(std::path::PathBuf::from);

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1200.0, 800.0]),
        ..Default::default()
    };

    eframe::run_native(
        "Markdown Reader",
        options,
        Box::new(|_cc| {
            Ok(Box::new(App::new(initial_path)))
        }),
    )
}
