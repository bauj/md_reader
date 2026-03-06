mod app;
mod fs;
mod markdown;
mod pdf_export;
mod persist;
mod theme;
mod ui;

use app::App;

fn main() -> Result<(), eframe::Error> {
    let initial_path = std::env::args().nth(1)
        .map(std::path::PathBuf::from)
        .map(|p| p.canonicalize().unwrap_or(p));

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

    // JetBrains Mono as monospace font (code blocks, editor)
    fonts
        .families
        .entry(egui::FontFamily::Monospace)
        .or_default()
        .insert(0, "JetBrainsMono".to_owned());

    // Optional UI font: Outfit (clean geometric sans-serif, replaces egui's default Ubuntu).
    let outfit_path = concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/Outfit-Regular.ttf");
    if let Ok(data) = std::fs::read(outfit_path) {
        fonts.font_data.insert("Outfit".to_owned(), std::sync::Arc::new(egui::FontData::from_owned(data)));
        fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "Outfit".to_owned());
    }

    // Preview body fonts — regular + bold pair per font, registered as named families.
    // Bold variant name convention: "Body-{Name}-Bold", derived automatically in styled_rt().
    load_body_font(&mut fonts, "SourceSans3",
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/SourceSans3-Regular.ttf"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/SourceSans3-Bold.ttf"),
        "Body-Sans");

    load_body_font(&mut fonts, "Nunito",
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/Nunito-Regular.ttf"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/Nunito-Bold.ttf"),
        "Body-Nunito");

    load_body_font(&mut fonts, "Rubik",
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/Rubik-Regular.ttf"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/Rubik-Bold.ttf"),
        "Body-Rubik");

    load_body_font(&mut fonts, "Figtree",
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/Figtree-Regular.ttf"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/Figtree-Bold.ttf"),
        "Body-Figtree");

    load_body_font(&mut fonts, "Manrope",
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/Manrope-Regular.ttf"),
        concat!(env!("CARGO_MANIFEST_DIR"), "/assets/fonts/Manrope-Bold.ttf"),
        "Body-Manrope");

    ctx.set_fonts(fonts);
}

/// Register a body font's regular and bold variants under `"Body-{family}"` and
/// `"Body-{family}-Bold"` named families.  Both files are loaded at runtime so
/// the binary still compiles even if one is missing.
fn load_body_font(
    fonts:        &mut egui::FontDefinitions,
    name:         &str,
    regular_path: &str,
    bold_path:    &str,
    family:       &str,
) {
    if let Ok(data) = std::fs::read(regular_path) {
        fonts.font_data.insert(name.to_owned(), std::sync::Arc::new(egui::FontData::from_owned(data)));
        fonts.families.insert(egui::FontFamily::Name(family.into()), vec![name.to_owned()]);
    }
    let bold_name = format!("{name}-Bold");
    let bold_family = format!("{family}-Bold");
    if let Ok(data) = std::fs::read(bold_path) {
        fonts.font_data.insert(bold_name.clone(), std::sync::Arc::new(egui::FontData::from_owned(data)));
        fonts.families.insert(egui::FontFamily::Name(bold_family.into()), vec![bold_name]);
    }
}
