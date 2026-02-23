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
    CloseTab(usize),
    Quit,
}

pub struct OpenTab {
    path:         PathBuf,
    buffer:       String,
    modified:     bool,
    parsed_doc:   Option<ParsedDoc>,
    needs_reparse: bool,
}

pub struct App {
    tree: FsTree,

    tabs:       Vec<OpenTab>,
    active_tab: Option<usize>,

    view_mode: ViewMode,

    pending_action: Option<PendingAction>,

    // Outline panel
    outline_open:     bool,
    outline_collapsed: HashSet<usize>,
    scroll_to_block:  Option<usize>,
}

impl Default for App {
    fn default() -> Self {
        Self::new(None)
    }
}

impl App {
    pub fn new(initial_path: Option<std::path::PathBuf>) -> Self {
        let mut app = App {
            tree:              FsTree::default(),
            tabs:              Vec::new(),
            active_tab:        None,
            view_mode:         ViewMode::Preview,
            pending_action:    None,
            outline_open:      true,
            outline_collapsed: HashSet::new(),
            scroll_to_block:   None,
        };

        if let Some(path) = initial_path {
            if !path.exists() {
                eprintln!("md_reader: path not found: {}", path.display());
            } else if path.is_dir() {
                app.tree = FsTree::new(path);
            } else if path.is_file() {
                if let Some(parent) = path.parent() {
                    app.tree = FsTree::new(parent.to_path_buf());
                }
                app.open_tab(path);
            }
        }

        app
    }
}

impl App {
    /// Open a file in a new tab, or focus its existing tab if already open.
    fn open_tab(&mut self, path: PathBuf) {
        if let Some(idx) = self.tabs.iter().position(|t| t.path == path) {
            self.active_tab = Some(idx);
            self.tree.selected = Some(path);
            return;
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.tree.selected = Some(path.clone());
                self.tabs.push(OpenTab {
                    path,
                    buffer:        content,
                    modified:      false,
                    parsed_doc:    None,
                    needs_reparse: true,
                });
                self.active_tab = Some(self.tabs.len() - 1);
            }
            Err(e) => eprintln!("Failed to read file: {e}"),
        }
    }

    /// Close the tab at `idx`; focus the nearest remaining tab.
    fn close_tab(&mut self, idx: usize) {
        self.tabs.remove(idx);
        self.active_tab = if self.tabs.is_empty() {
            None
        } else {
            Some(idx.saturating_sub(1))
        };
        self.tree.selected = self.active_tab.map(|i| self.tabs[i].path.clone());
    }

    fn save_active(&mut self) {
        if let Some(idx) = self.active_tab {
            let tab = &mut self.tabs[idx];
            match std::fs::write(&tab.path, &tab.buffer) {
                Ok(_) => {
                    tab.modified      = false;
                    tab.needs_reparse = true;
                }
                Err(e) => eprintln!("Failed to save file: {e}"),
            }
        }
    }

    fn save_all_modified(&mut self) {
        for tab in &mut self.tabs {
            if tab.modified {
                match std::fs::write(&tab.path, &tab.buffer) {
                    Ok(_) => tab.modified = false,
                    Err(e) => eprintln!("Failed to save {}: {e}", tab.path.display()),
                }
            }
        }
    }

    /// Guard an action behind an unsaved-changes dialog when needed.
    /// Returns `true` if the app should quit immediately (Quit with no dirty tabs).
    fn request_action(&mut self, action: PendingAction) -> bool {
        let is_dirty = match &action {
            PendingAction::CloseTab(idx) => self.tabs.get(*idx).map_or(false, |t| t.modified),
            PendingAction::Quit          => self.tabs.iter().any(|t| t.modified),
        };
        if is_dirty {
            self.pending_action = Some(action);
            false
        } else {
            match action {
                PendingAction::CloseTab(idx) => { self.close_tab(idx); false }
                PendingAction::Quit          => true,
            }
        }
    }

    fn window_title(&self) -> String {
        match self.active_tab.and_then(|i| self.tabs.get(i)) {
            Some(tab) => {
                let name = tab.path.file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                if tab.modified { format!("md_reader — {name} ●") } else { format!("md_reader — {name}") }
            }
            None => "md_reader".to_string(),
        }
    }

    /// Open a save-file dialog, create the file on disk, and open it as a new tab.
    fn create_new_file(&mut self) -> bool {
        if let Some(path) = rfd::FileDialog::new()
            .add_filter("Markdown", &["md"])
            .set_file_name("untitled.md")
            .save_file()
        {
            match std::fs::write(&path, "") {
                Ok(_) => { self.open_tab(path); return true; }
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
        let ctrl_s     = ctx.input(|i| i.key_pressed(Key::S)        && i.modifiers.ctrl);
        let ctrl_o     = ctx.input(|i| i.key_pressed(Key::O)        && i.modifiers.ctrl);
        let ctrl_w     = ctx.input(|i| i.key_pressed(Key::W)        && i.modifiers.ctrl);
        let ctrl_n     = ctx.input(|i| i.key_pressed(Key::N)        && i.modifiers.ctrl);
        let ctrl_q     = ctx.input(|i| i.key_pressed(Key::Q)        && i.modifiers.ctrl);
        let ctrl_left  = ctx.input(|i| i.key_pressed(Key::PageUp)   && i.modifiers.ctrl);
        let ctrl_right = ctx.input(|i| i.key_pressed(Key::PageDown) && i.modifiers.ctrl);

        if ctrl_s { self.save_active(); }
        if ctrl_w {
            if let Some(idx) = self.active_tab {
                self.request_action(PendingAction::CloseTab(idx));
            }
        }
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
            if !self.tabs.is_empty() {
                let cur  = self.active_tab.unwrap_or(0);
                let next = if ctrl_right {
                    (cur + 1).min(self.tabs.len() - 1)
                } else {
                    cur.saturating_sub(1)
                };
                self.active_tab    = Some(next);
                self.tree.selected = Some(self.tabs[next].path.clone());
            }
        }

        // ── Re-parse active tab when needed ──────────────────────────────
        if let Some(idx) = self.active_tab {
            if self.tabs[idx].needs_reparse {
                let doc = parse_markdown(&self.tabs[idx].buffer);
                self.tabs[idx].parsed_doc    = Some(doc);
                self.tabs[idx].needs_reparse = false;
            }
        }

        // ── Unsaved-changes dialog ────────────────────────────────────────
        if self.pending_action.is_some() {
            let mut choice: Option<bool> = None;

            egui::Window::new("Unsaved changes")
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ctx, |ui| {
                    let msg = match &self.pending_action {
                        Some(PendingAction::Quit) => {
                            let n = self.tabs.iter().filter(|t| t.modified).count();
                            if n > 1 {
                                format!("{n} files have unsaved changes. What would you like to do?")
                            } else {
                                "You have unsaved changes. What would you like to do?".to_string()
                            }
                        }
                        Some(PendingAction::CloseTab(idx)) => {
                            let name = self.tabs.get(*idx)
                                .and_then(|t| t.path.file_name())
                                .map(|n| n.to_string_lossy().to_string())
                                .unwrap_or_default();
                            format!("\"{}\" has unsaved changes. What would you like to do?", name)
                        }
                        None => String::new(),
                    };
                    ui.label(msg);
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        if ui.button("💾 Save").clicked()    { choice = Some(true);  }
                        if ui.button("🗑 Discard").clicked() { choice = Some(false); }
                        if ui.button("Cancel").clicked()     { self.pending_action = None; }
                    });
                });

            if let Some(save) = choice {
                match self.pending_action.take() {
                    Some(PendingAction::CloseTab(idx)) => {
                        if save {
                            let (path, buf) = {
                                let t = &self.tabs[idx];
                                (t.path.clone(), t.buffer.clone())
                            };
                            if std::fs::write(&path, &buf).is_ok() {
                                self.tabs[idx].modified = false;
                            }
                        }
                        self.close_tab(idx);
                    }
                    Some(PendingAction::Quit) => {
                        if save { self.save_all_modified(); }
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
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

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_modified = self.active_tab
                        .and_then(|i| self.tabs.get(i))
                        .map_or(false, |t| t.modified);
                    let has_active = self.active_tab.is_some();

                    if ui.add_enabled(is_modified, egui::Button::new("💾 Save")).clicked() {
                        self.save_active();
                    }
                    if ui.add_enabled(has_active, egui::Button::new("✖ Close")).clicked() {
                        if let Some(idx) = self.active_tab {
                            self.request_action(PendingAction::CloseTab(idx));
                        }
                    }
                });
            });
        });

        // ── Tab bar ───────────────────────────────────────────────────────
        TopBottomPanel::top("tab_bar").show(ctx, |ui| {
            ScrollArea::horizontal().show(ui, |ui| {
                ui.horizontal(|ui| {
                    if self.tabs.is_empty() {
                        ui.label(
                            egui::RichText::new("No files open")
                                .color(egui::Color32::GRAY)
                                .size(12.0),
                        );
                        return;
                    }

                    let mut activate_idx: Option<usize> = None;
                    let mut close_idx:    Option<usize> = None;

                    for (i, tab) in self.tabs.iter().enumerate() {
                        let name = tab.path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "?".to_string());
                        let label = if tab.modified {
                            format!(" {name} ● ")
                        } else {
                            format!(" {name} ")
                        };
                        let is_active = self.active_tab == Some(i);

                        if ui.selectable_label(is_active, &label).clicked() {
                            activate_idx = Some(i);
                        }
                        if ui.small_button("✕").clicked() {
                            close_idx = Some(i);
                        }

                        // Visual separator between tabs
                        ui.separator();
                    }

                    if let Some(i) = activate_idx {
                        self.active_tab    = Some(i);
                        self.tree.selected = Some(self.tabs[i].path.clone());
                    }
                    if let Some(i) = close_idx {
                        self.request_action(PendingAction::CloseTab(i));
                    }
                });
            });
        });

        // If the user switched to Edit or Split with no open tab, create one.
        let switched_to_edit = mode_before == ViewMode::Preview
            && (self.view_mode == ViewMode::Edit || self.view_mode == ViewMode::Split);

        if switched_to_edit && self.active_tab.is_none() {
            let created = self.create_new_file();
            if !created {
                self.view_mode = ViewMode::Preview;
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
                        if let Some(path) = render_sidebar(ui, &mut self.tree) {
                            self.open_tab(path);
                        }

                        // Outline panel — only shown when the active tab has a parsed doc
                        let has_doc = self.active_tab
                            .and_then(|i| self.tabs.get(i))
                            .map_or(false, |t| t.parsed_doc.is_some());

                        if has_doc {
                            let idx = self.active_tab.unwrap();
                            // SAFETY: has_doc guarantees both active_tab and parsed_doc are Some
                            let doc = self.tabs[idx].parsed_doc.as_ref().unwrap();
                            if let Some(block_idx) = render_outline(
                                ui,
                                doc,
                                &mut self.outline_open,
                                &mut self.outline_collapsed,
                            ) {
                                self.scroll_to_block = Some(block_idx);
                            }
                        }
                    });
            });

        // ── Central panel ─────────────────────────────────────────────────
        let scroll_to = self.scroll_to_block.take();
        CentralPanel::default().show(ctx, |ui| {
            match self.active_tab {
                None => {
                    ui.vertical_centered(|ui| {
                        ui.add_space(40.0);
                        ui.label(egui::RichText::new("No file open").color(egui::Color32::GRAY));
                    });
                }
                Some(idx) => match self.view_mode {
                    ViewMode::Preview => {
                        let tab = &self.tabs[idx];
                        render_preview(ui, &tab.parsed_doc, &tab.buffer, scroll_to, "preview");
                    }
                    ViewMode::Edit => {
                        let tab = &mut self.tabs[idx];
                        render_editor(ui, &mut tab.buffer, &mut tab.modified, &mut tab.needs_reparse, "editor");
                    }
                    ViewMode::Split => {
                        ui.columns(2, |cols| {
                            {
                                let tab = &mut self.tabs[idx];
                                render_editor(
                                    &mut cols[0],
                                    &mut tab.buffer,
                                    &mut tab.modified,
                                    &mut tab.needs_reparse,
                                    "split_editor",
                                );
                            }
                            {
                                let tab = &self.tabs[idx];
                                render_preview(
                                    &mut cols[1],
                                    &tab.parsed_doc,
                                    &tab.buffer,
                                    scroll_to,
                                    "split_preview",
                                );
                            }
                        });
                    }
                },
            }
        });
    }
}

fn render_preview(
    ui: &mut egui::Ui,
    doc: &Option<ParsedDoc>,
    buffer: &str,
    scroll_to: Option<usize>,
    id: &str,
) {
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
                *modified      = true;
                *needs_reparse = true;
            }
        });
}
