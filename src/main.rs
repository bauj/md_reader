mod app;
mod fs;
mod markdown;
mod persist;
mod theme;
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
        Box::new(|cc| {
            setup_fonts(&cc.egui_ctx);
            Ok(Box::new(App::new(initial_path)))
        }),
    )
}

fn setup_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();

    fonts.font_data.insert(
        "JetBrainsMono".to_owned(),
        std::sync::Arc::new(egui::FontData::from_static(
            include_bytes!("../assets/fonts/JetBrainsMono-Regular.ttf"),
        )),
    );

    // JetBrains Mono as the first monospace font (code blocks, editor)
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "JetBrainsMono".to_owned());

    ctx.set_fonts(fonts);
}
