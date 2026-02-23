use egui::{CentralPanel, SidePanel, TopBottomPanel, ScrollArea};
use std::path::PathBuf;
use crate::fs::FsTree;
use crate::ui::render_sidebar;

pub struct App {
    tree: FsTree,
    current_file: Option<PathBuf>,
    buffer: String,
}

impl Default for App {
    fn default() -> Self {
        App {
            tree: FsTree::default(),
            current_file: None,
            buffer: String::new(),
        }
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Top toolbar
        TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("📁 Open Folder").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.tree = FsTree::new(path);
                        self.current_file = None;
                        self.buffer.clear();
                    }
                }

                ui.separator();

                if let Some(ref path) = self.current_file {
                    if let Some(name) = path.file_name() {
                        ui.label(format!("📄 {}", name.to_string_lossy()));
                    }
                } else {
                    ui.label("No file open");
                }
            });
        });

        // Left sidebar
        SidePanel::left("sidebar")
            .min_width(200.0)
            .default_width(250.0)
            .show(ctx, |ui| {
                ui.label("📂 Files");
                ui.separator();

                ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        if let Some(selected_path) = render_sidebar(ui, &mut self.tree) {
                            // Load the file
                            match std::fs::read_to_string(&selected_path) {
                                Ok(content) => {
                                    self.buffer = content;
                                    self.current_file = Some(selected_path);
                                }
                                Err(e) => {
                                    eprintln!("Failed to read file: {}", e);
                                }
                            }
                        }
                    });
            });

        // Central panel with file content
        CentralPanel::default().show(ctx, |ui| {
            ScrollArea::vertical()
                .auto_shrink([false; 2])
                .show(ui, |ui| {
                    ui.label(&self.buffer);
                });
        });
    }
}
