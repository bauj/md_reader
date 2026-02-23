use egui::{CentralPanel, Key, ScrollArea, SidePanel, TextEdit, TopBottomPanel};
use std::collections::HashSet;
use std::path::PathBuf;
use crate::fs::FsTree;
use crate::markdown::{parse_markdown, ParsedDoc};
use crate::ui::{render_outline, render_sidebar};

#[derive(PartialEq, Clone, Copy)]
pub enum ViewMode {
    Preview,
    Edit,
    Split,
}

/// What to do after the unsaved-changes dialog is resolved.
enum PendingAction {
    LoadFile(PathBuf),
    CloseFile,
    Quit,
}

pub struct App {
    tree: FsTree,

    current_file: Option<PathBuf>,
    buffer: String,
    modified: bool,

    parsed_doc: Option<ParsedDoc>,
    needs_reparse: bool,

    view_mode: ViewMode,

    /// Set when the user triggers an action that requires discarding/saving first.
    pending_action: Option<PendingAction>,

    // Outline panel
    outline_open: bool,
    outline_collapsed: HashSet<usize>,
    scroll_to_block: Option<usize>,
}

impl Default for App {
    fn default() -> Self {
        App {
            tree: FsTree::default(),
            current_file: None,
            buffer: String::new(),
            modified: false,
            parsed_doc: None,
            needs_reparse: false,
            view_mode: ViewMode::Preview,
            pending_action: None,
            outline_open: true,
            outline_collapsed: HashSet::new(),
            scroll_to_block: None,
        }
    }
}

impl App {
    fn load_file(&mut self, path: PathBuf) {
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.buffer = content;
                self.current_file = Some(path);
                self.modified = false;
                self.needs_reparse = true;
            }
            Err(e) => eprintln!("Failed to read file: {e}"),
        }
    }

    fn close_file(&mut self) {
        self.current_file = None;
        self.buffer.clear();
        self.parsed_doc = None;
        self.modified = false;
        self.view_mode = ViewMode::Preview;
    }

    fn save_file(&mut self) {
        if let Some(ref path) = self.current_file.clone() {
            match std::fs::write(path, &self.buffer) {
                Ok(_) => {
                    self.modified = false;
                    self.needs_reparse = true;
                }
                Err(e) => eprintln!("Failed to save file: {e}"),
            }
        }
    }

    /// Call this whenever the user wants to do something that requires a clean state.
    /// If there are unsaved changes, stores the action for after the dialog.
    /// Returns `true` if the app should quit immediately (Quit with no unsaved changes).
    fn request_action(&mut self, action: PendingAction) -> bool {
        if self.modified {
            self.pending_action = Some(action);
            false
        } else {
            match action {
                PendingAction::LoadFile(path) => { self.load_file(path); false }
                PendingAction::CloseFile      => { self.close_file(); false }
                PendingAction::Quit           => true,
            }
        }
    }

    fn window_title(&self) -> String {
        match &self.current_file {
            Some(path) => {
                let name = path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if self.modified {
                    format!("md_reader — {name} ●")
                } else {
                    format!("md_reader — {name}")
                }
            }
            None => "md_reader".to_string(),
        }
    }

    /// Open a save-file dialog, create the file on disk, and load it.
    /// Returns true if a file was created.
    fn create_new_file(&mut self) -> bool {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Markdown", &["md"])
            .set_file_name("untitled.md")
            .save_file()
        {
            match std::fs::write(&path, "") {
                Ok(_) => {
                    self.load_file(path);
                    return true;
                }
                Err(e) => eprintln!("Failed to create file: {e}"),
            }
        }
        false
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(self.window_title()));

        // ── Keyboard shortcuts ────────────────────────────────────────────
        let ctrl_s     = ctx.input(|i| i.key_pressed(Key::S)         && i.modifiers.ctrl);
        let ctrl_o     = ctx.input(|i| i.key_pressed(Key::O)         && i.modifiers.ctrl);
        let ctrl_w     = ctx.input(|i| i.key_pressed(Key::W)         && i.modifiers.ctrl);
        let ctrl_n     = ctx.input(|i| i.key_pressed(Key::N)         && i.modifiers.ctrl);
        let ctrl_q     = ctx.input(|i| i.key_pressed(Key::Q)         && i.modifiers.ctrl);
        let ctrl_left  = ctx.input(|i| i.key_pressed(Key::PageUp)    && i.modifiers.ctrl);
        let ctrl_right = ctx.input(|i| i.key_pressed(Key::PageDown)  && i.modifiers.ctrl);

        if ctrl_s { self.save_file(); }
        if ctrl_w { self.request_action(PendingAction::CloseFile); }
        if ctrl_q && self.request_action(PendingAction::Quit) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }

        if ctrl_n { self.create_new_file(); }

        if ctrl_o {
            if let Some(path) = rfd::FileDialog::new().pick_folder() {
                self.tree = FsTree::new(path);
            }
        }

        if ctrl_left || ctrl_right {
            let files = self.tree.all_files();
            if !files.is_empty() {
                let current_idx = self.current_file.as_ref()
                    .and_then(|p| files.iter().position(|f| f == p));
                let next_idx = match current_idx {
                    None if ctrl_right => 0,
                    None               => files.len() - 1,
                    Some(i) if ctrl_right => (i + 1).min(files.len() - 1),
                    Some(i)               => i.saturating_sub(1),
                };
                let path = files[next_idx].clone();
                self.request_action(PendingAction::LoadFile(path));
            }
        }

        // ── Re-parse when needed ──────────────────────────────────────────
        if self.needs_reparse {
            self.parsed_doc = Some(parse_markdown(&self.buffer));
            self.needs_reparse = false;
        }

        // ── Unsaved-changes dialog ────────────────────────────────────────
        if self.pending_action.is_some() {
            let mut choice: Option<bool> = None; // Some(true) = save, Some(false) = discard, None = cancel

            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    ui.label("You have unsaved changes. What would you like to do?");
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("💾 Save").clicked()    { choice = Some(true); }
                        if ui.button("🗑 Discard").clicked() { choice = Some(false); }
                        if ui.button("Cancel").clicked()     { choice = None; self.pending_action = None; }
                    });
                });

            if let Some(save) = choice {
                if save { self.save_file(); }
                match self.pending_action.take() {
                    Some(PendingAction::LoadFile(path)) => self.load_file(path),
                    Some(PendingAction::CloseFile)      => self.close_file(),
                    Some(PendingAction::Quit)           => ctx.send_viewport_cmd(egui::ViewportCommand::Close),
                    None => {}
                }
            }
        }

        // ── Toolbar ───────────────────────────────────────────────────────
        let mode_before = self.view_mode;

        TopBottomPanel::top("toolbar").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("📁 Open Folder").clicked() {
                    if let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.tree = FsTree::new(path);
                    }
                }

                ui.separator();

                ui.selectable_value(&mut self.view_mode, ViewMode::Preview, "👁 Preview");
                ui.selectable_value(&mut self.view_mode, ViewMode::Edit,    "✏ Edit");
                ui.selectable_value(&mut self.view_mode, ViewMode::Split,   "⬜ Split");

                ui.separator();

                if let Some(ref path) = self.current_file {
                    if let Some(name) = path.file_name() {
                        let label = if self.modified {
                            format!("📄 {} ●", name.to_string_lossy())
                        } else {
                            format!("📄 {}", name.to_string_lossy())
                        };
                        ui.label(label);
                    }
                } else {
                    ui.label("No file open");
                }

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui.add_enabled(self.modified, egui::Button::new("💾 Save")).clicked() {
                        self.save_file();
                    }
                    if ui.add_enabled(self.current_file.is_some(), egui::Button::new("✖ Close")).clicked() {
                        self.request_action(PendingAction::CloseFile);
                    }
                });
            });
        });

        // If the user switched to Edit or Split with no file open, create one.
        let switched_to_edit = mode_before == ViewMode::Preview
            && (self.view_mode == ViewMode::Edit || self.view_mode == ViewMode::Split);

        if switched_to_edit && self.current_file.is_none() {
            let created = self.create_new_file();
            if !created {
                self.view_mode = ViewMode::Preview; // user cancelled dialog
            }
        }

        // ── Sidebar ───────────────────────────────────────────────────────
        SidePanel::left("sidebar")
            .min_width(200.0)
            .default_width(250.0)
            .show(ctx, |ui| {
                ScrollArea::vertical()
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        ui.label("📂 Files");
                        ui.separator();
                        if let Some(selected_path) = render_sidebar(ui, &mut self.tree) {
                            self.request_action(PendingAction::LoadFile(selected_path));
                        }

                        // Outline panel — only shown when a doc is parsed
                        if let Some(ref doc) = self.parsed_doc {
                            if let Some(block_idx) = render_outline(ui, doc, &mut self.outline_open, &mut self.outline_collapsed) {
                                self.scroll_to_block = Some(block_idx);
                            }
                        }
                    });
            });

        // ── Central panel ─────────────────────────────────────────────────
        let scroll_to = self.scroll_to_block.take();
        CentralPanel::default().show(ctx, |ui| {
            match self.view_mode {
                ViewMode::Preview => render_preview(ui, &self.parsed_doc, &self.buffer, scroll_to, "preview"),
                ViewMode::Edit    => render_editor(ui, &mut self.buffer, &mut self.modified, &mut self.needs_reparse, "editor"),
                ViewMode::Split   => {
                    ui.columns(2, |cols| {
                        render_editor(&mut cols[0], &mut self.buffer, &mut self.modified, &mut self.needs_reparse, "split_editor");
                        render_preview(&mut cols[1], &self.parsed_doc, &self.buffer, scroll_to, "split_preview");
                    });
                }
            }
        });
    }
}

fn render_preview(ui: &mut egui::Ui, doc: &Option<ParsedDoc>, buffer: &str, scroll_to: Option<usize>, id: &str) {
    ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            if let Some(doc) = doc {
                crate::markdown::render_markdown(ui, doc, scroll_to);
            } else if !buffer.is_empty() {
                ui.label("Failed to parse markdown.");
            } else {
                ui.vertical_centered(|ui| {
                    ui.add_space(40.0);
                    ui.label(egui::RichText::new("No file open").color(egui::Color32::GRAY));
                });
            }
        });
}

fn render_editor(
    ui: &mut egui::Ui,
    buffer: &mut String,
    modified: &mut bool,
    needs_reparse: &mut bool,
    id: &str,
) {
    ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            let response = ui.add(
                TextEdit::multiline(buffer)
                    .font(egui::TextStyle::Monospace)
                    .desired_width(f32::INFINITY)
                    .desired_rows(40),
            );
            if response.changed() {
                *modified = true;
                *needs_reparse = true;
            }
        });
}
