use egui::{CentralPanel, Key, ScrollArea, SidePanel, TextEdit, TopBottomPanel};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use notify::Watcher;
use crate::fs::FsTree;
use crate::markdown::{parse_markdown, Highlighter, ParsedDoc, SearchOpts};
use crate::persist;
use crate::ui::{render_outline, render_sidebar};

/// Holds a live notify watcher and the channel end we poll each frame.
struct FileWatcher {
    // Kept alive so the background thread keeps running.
    _watcher: notify::RecommendedWatcher,
    rx: mpsc::Receiver<notify::Result<notify::Event>>,
}

impl FileWatcher {
    fn new(path: &Path) -> Option<Self> {
        let (tx, rx) = mpsc::channel();
        let mut watcher = notify::RecommendedWatcher::new(
            move |res| { let _ = tx.send(res); },
            notify::Config::default(),
        ).ok()?;
        watcher.watch(path, notify::RecursiveMode::Recursive).ok()?;
        Some(FileWatcher { _watcher: watcher, rx })
    }
}

#[derive(PartialEq, Clone, Copy)]
pub enum ViewMode {
    Preview,
    Edit,
    Split,
}

impl ViewMode {
    fn as_str(self) -> &'static str {
        match self {
            ViewMode::Preview => "preview",
            ViewMode::Edit    => "edit",
            ViewMode::Split   => "split",
        }
    }
    fn from_str(s: &str) -> Self {
        match s {
            "edit"  => ViewMode::Edit,
            "split" => ViewMode::Split,
            _       => ViewMode::Preview,
        }
    }
}

/// What to do after the unsaved-changes dialog is resolved.
enum PendingAction {
    CloseTab(usize),
    Quit,
}

pub struct OpenTab {
    path:          PathBuf,
    buffer:        String,
    modified:      bool,
    parsed_doc:    Option<ParsedDoc>,
    needs_reparse: bool,
    /// File was changed on disk while we have unsaved local edits.
    extern_modified: bool,
}

pub struct App {
    tree: FsTree,

    tabs:       Vec<OpenTab>,
    active_tab: Option<usize>,

    highlighter: Highlighter,

    view_mode: ViewMode,

    recent_files: Vec<PathBuf>,

    watcher: Option<FileWatcher>,

    pending_action: Option<PendingAction>,

    // Search (Ctrl+F)
    search_open:          bool,
    search_query:         String,
    search_matches:          Vec<usize>, // byte offsets into the raw buffer (Edit-mode layouter + occurrence index)
    search_match_blocks:     Vec<usize>, // block index per match, from plain-text scan (Preview scroll)
    search_current:          usize,      // index into search_matches
    search_request_focus:    bool,       // request TextEdit focus on next frame
    search_scroll_to_offset: Option<usize>, // byte offset to scroll Edit-mode TextEdit to
    search_case_sensitive:   bool,
    search_whole_word:       bool,

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
        let state = persist::load();

        let mut app = App {
            tree:              FsTree::default(),
            tabs:              Vec::new(),
            active_tab:        None,
            highlighter:       Highlighter::new(),
            view_mode:         ViewMode::from_str(&state.view_mode),
            recent_files:      state.recent_files.into_iter().filter(|p| p.is_file()).collect(),
            watcher:              None,
            pending_action:       None,
            search_open:             false,
            search_query:            String::new(),
            search_matches:          Vec::new(),
            search_match_blocks:     Vec::new(),
            search_current:          0,
            search_request_focus:    false,
            search_scroll_to_offset: None,
            search_case_sensitive:   false,
            search_whole_word:       false,
            outline_open:         true,
            outline_collapsed:    HashSet::new(),
            scroll_to_block:      None,
        };

        if let Some(path) = initial_path {
            // CLI argument takes precedence; ignore persisted tabs/root.
            if !path.exists() {
                eprintln!("md_reader: path not found: {}", path.display());
            } else if path.is_dir() {
                app.watcher = FileWatcher::new(&path);
                app.tree = FsTree::new(path);
            } else if path.is_file() {
                if let Some(parent) = path.parent() {
                    app.watcher = FileWatcher::new(parent);
                    app.tree = FsTree::new(parent.to_path_buf());
                }
                app.open_tab(path);
            }
        } else {
            // Restore last session.
            if let Some(dir) = state.root_dir {
                if dir.is_dir() {
                    app.watcher = FileWatcher::new(&dir);
                    app.tree = FsTree::new(dir);
                }
            }
            // Reopen tabs that still exist on disk (preserve order).
            for path in state.open_tabs {
                if path.is_file() {
                    app.open_tab(path);
                }
            }
            // Restore active tab (clamped to valid range).
            if !app.tabs.is_empty() {
                let idx = state.active_tab
                    .unwrap_or(0)
                    .min(app.tabs.len() - 1);
                app.active_tab    = Some(idx);
                app.tree.selected = Some(app.tabs[idx].path.clone());
            }
        }

        app
    }

    /// Recompute `search_matches` (raw buffer offsets for Edit mode) and
    /// `search_match_blocks` (block indices for Preview scroll), then reset current.
    fn update_search_matches(&mut self) {
        self.search_matches.clear();
        self.search_match_blocks.clear();
        self.search_current = 0;

        let opts = SearchOpts {
            case_sensitive: self.search_case_sensitive,
            whole_word:     self.search_whole_word,
        };
        let needle: String = if opts.case_sensitive {
            self.search_query.clone()
        } else {
            self.search_query.to_lowercase()
        };
        if needle.is_empty() { return; }
        let nlen = needle.len();

        // Raw buffer search — used by the Edit-mode layouter.
        if let Some(tab) = self.active_tab.and_then(|i| self.tabs.get(i)) {
            let haystack: String = if opts.case_sensitive {
                tab.buffer.clone()
            } else {
                tab.buffer.to_lowercase()
            };
            let mut p = 0;
            while p < haystack.len() {
                match haystack[p..].find(&needle as &str) {
                    None => break,
                    Some(rel) => {
                        let ms = p + rel;
                        let me = ms + nlen;
                        if !opts.whole_word || is_word_boundary(&haystack, ms, me) {
                            self.search_matches.push(ms);
                            p = me;
                        } else {
                            p = ms + haystack[ms..].chars().next().map_or(1, |c| c.len_utf8());
                        }
                    }
                }
            }
        }

        // Plain-text block search — maps each match to its block for Preview scroll.
        if let Some(doc) = self.active_tab
            .and_then(|i| self.tabs.get(i))
            .and_then(|t| t.parsed_doc.as_ref())
        {
            for (bi, block) in doc.blocks.iter().enumerate() {
                let text = block_plain_text(block);
                let haystack: String = if opts.case_sensitive { text } else { text.to_lowercase() };
                let count = count_matches(&haystack, &needle, opts);
                for _ in 0..count {
                    self.search_match_blocks.push(bi);
                }
            }
        }
    }

    /// Navigate to next/prev match and update both scroll mechanisms.
    fn search_navigate(&mut self, forward: bool) {
        let n = self.search_matches.len();
        if n == 0 { return; }
        self.search_current = if forward {
            (self.search_current + 1) % n
        } else {
            (self.search_current + n - 1) % n
        };
        // Preview mode: scroll to the block containing this match.
        if let Some(&bi) = self.search_match_blocks.get(self.search_current) {
            self.scroll_to_block = Some(bi);
        }
        // Edit mode: scroll the TextEdit to the match byte offset.
        self.search_scroll_to_offset = self.search_matches.get(self.search_current).copied();
        // Re-request focus on the search bar so render_editor's request_focus()
        // (called for scrolling) doesn't steal it away.
        self.search_request_focus = true;
    }

    /// Snapshot current session into a `persist::AppState` and write it to disk.
    fn save_state(&self) {
        let state = persist::AppState {
            root_dir:     self.tree.root.as_ref().map(|n| n.path.clone()),
            open_tabs:    self.tabs.iter().map(|t| t.path.clone()).collect(),
            active_tab:   self.active_tab,
            view_mode:    self.view_mode.as_str().to_string(),
            recent_files: self.recent_files.clone(),
        };
        persist::save(&state);
    }
}

impl App {
    /// Push `path` to the front of the recent-files list, deduplicating and capping at 20.
    fn push_recent(&mut self, path: &PathBuf) {
        self.recent_files.retain(|p| p != path);
        self.recent_files.insert(0, path.clone());
        self.recent_files.truncate(20);
    }

    /// Open a file in a new tab, or focus its existing tab if already open.
    fn open_tab(&mut self, path: PathBuf) {
        self.push_recent(&path);
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
                    buffer:          content,
                    modified:        false,
                    parsed_doc:      None,
                    needs_reparse:   true,
                    extern_modified: false,
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
        let ctrl_f     = ctx.input(|i| i.key_pressed(Key::F)        && i.modifiers.ctrl);
        let ctrl_left  = ctx.input(|i| i.key_pressed(Key::PageUp)   && i.modifiers.ctrl);
        let ctrl_right = ctx.input(|i| i.key_pressed(Key::PageDown) && i.modifiers.ctrl);
        let escape     = ctx.input(|i| i.key_pressed(Key::Escape));
        let enter      = ctx.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift);
        let shift_enter= ctx.input(|i| i.key_pressed(Key::Enter) &&  i.modifiers.shift);

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
                self.watcher = FileWatcher::new(&path);
                self.tree    = FsTree::new(path);
            }
        }
        if ctrl_f {
            if self.search_open {
                // Ctrl+F again → focus the bar already open
                self.search_request_focus = true;
            } else {
                self.search_open          = true;
                self.search_request_focus = true;
                self.update_search_matches();
            }
        }
        if escape && self.search_open {
            self.search_open    = false;
            self.search_matches.clear();
        }
        // Only navigate on Enter/Shift+Enter when the search bar itself is focused,
        // so typing in the editor doesn't accidentally trigger search navigation.
        let search_input_id = egui::Id::new("search_input");
        let search_bar_focused = ctx.memory(|m| m.focused() == Some(search_input_id));
        if self.search_open && !self.search_matches.is_empty() && search_bar_focused {
            if enter            { self.search_navigate(true);  }
            else if shift_enter { self.search_navigate(false); }
        }
        // Alt+C / Alt+W toggle search options (gated to search bar focus).
        if self.search_open && search_bar_focused {
            let alt_c = ctx.input(|i| i.key_pressed(Key::C) && i.modifiers.alt);
            let alt_w = ctx.input(|i| i.key_pressed(Key::W) && i.modifiers.alt);
            if alt_c {
                self.search_case_sensitive = !self.search_case_sensitive;
                self.update_search_matches();
            }
            if alt_w {
                self.search_whole_word = !self.search_whole_word;
                self.update_search_matches();
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

        // ── File-watcher events ───────────────────────────────────────────
        let mut rescan_tree = false;
        if let Some(ref w) = self.watcher {
            while let Ok(Ok(event)) = w.rx.try_recv() {
                use notify::EventKind::*;
                match event.kind {
                    Modify(_) => {
                        for path in &event.paths {
                            if let Some(tab) = self.tabs.iter_mut().find(|t| &t.path == path) {
                                if tab.modified {
                                    // User has unsaved edits — flag for the banner.
                                    tab.extern_modified = true;
                                } else {
                                    // No local edits — silently reload.
                                    if let Ok(content) = std::fs::read_to_string(&tab.path) {
                                        tab.buffer        = content;
                                        tab.needs_reparse = true;
                                        tab.extern_modified = false;
                                    }
                                }
                            }
                        }
                    }
                    Create(_) | Remove(_) | Other => {
                        rescan_tree = true;
                    }
                    _ => {}
                }
            }
        }
        if rescan_tree {
            if let Some(ref root) = self.tree.root {
                let root_path = root.path.clone();
                self.tree = FsTree::new(root_path);
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
                        self.save_state();
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
                        self.watcher = FileWatcher::new(&path);
                        self.tree    = FsTree::new(path);
                    }
                }

                // ── Recent files dropdown ─────────────────────────────────
                let mut open_path: Option<PathBuf> = None;
                let mut clear     = false;

                ui.menu_button("🕐 Recent", |ui| {
                    if self.recent_files.is_empty() {
                        ui.label(
                            egui::RichText::new("No recent files")
                                .color(egui::Color32::GRAY),
                        );
                    } else {
                        egui::ScrollArea::vertical()
                            .max_height(300.0)
                            .show(ui, |ui| {
                                for path in &self.recent_files {
                                    let label = path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .unwrap_or_default();
                                    let tip = path.to_string_lossy();
                                    if ui.button(&label).on_hover_text(tip.as_ref()).clicked() {
                                        open_path = Some(path.clone());
                                        ui.close_menu();
                                    }
                                }
                                ui.separator();
                                if ui.button("🗑 Clear recent files").clicked() {
                                    clear = true;
                                    ui.close_menu();
                                }
                            });
                    }
                });

                if let Some(path) = open_path {
                    self.open_tab(path);
                }
                if clear {
                    self.recent_files.clear();
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

        // ── External-modification banner ──────────────────────────────────
        // Shown when the active tab was changed on disk while we have local edits.
        let extern_mod = self.active_tab
            .and_then(|i| self.tabs.get(i))
            .map_or(false, |t| t.extern_modified);

        if extern_mod {
            let mut reload = false;
            let mut keep   = false;
            TopBottomPanel::top("extern_mod_banner").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("⚠ File changed on disk")
                            .color(egui::Color32::from_rgb(220, 160, 0))
                            .strong(),
                    );
                    ui.add_space(8.0);
                    if ui.button("Reload").clicked() { reload = true; }
                    if ui.button("Keep mine").clicked() { keep = true; }
                });
            });
            if reload {
                if let Some(idx) = self.active_tab {
                    let tab = &mut self.tabs[idx];
                    if let Ok(content) = std::fs::read_to_string(&tab.path) {
                        tab.buffer          = content;
                        tab.modified        = false;
                        tab.needs_reparse   = true;
                        tab.extern_modified = false;
                    }
                }
            }
            if keep {
                if let Some(idx) = self.active_tab {
                    self.tabs[idx].extern_modified = false;
                }
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
                        let tab  = &self.tabs[idx];
                        let sq   = self.search_query.as_str();
                        let sc   = self.search_current;
                        let opts = SearchOpts { case_sensitive: self.search_case_sensitive, whole_word: self.search_whole_word };
                        render_preview(ui, &tab.parsed_doc, &tab.buffer, scroll_to, "preview", &mut self.highlighter, sq, sc, opts);
                    }
                    ViewMode::Edit => {
                        let sm   = self.search_matches.as_slice();
                        let opts = SearchOpts { case_sensitive: self.search_case_sensitive, whole_word: self.search_whole_word };
                        let ql   = needle_len(&self.search_query, opts);
                        let sc   = self.search_current;
                        let sto  = self.search_scroll_to_offset.take();
                        let buffer_changed = {
                            let tab = &mut self.tabs[idx];
                            let before = tab.needs_reparse;
                            render_editor(ui, &mut tab.buffer, &mut tab.modified, &mut tab.needs_reparse, "editor", sm, ql, sc, sto);
                            !before && tab.needs_reparse
                        };
                        // Keep search_matches in sync if the buffer was edited this frame.
                        if buffer_changed && self.search_open {
                            self.update_search_matches();
                        }
                    }
                    ViewMode::Split => {
                        // Borrow separate fields before the closure so Rust
                        // captures them independently (Rust 2021 fine-grained capture).
                        let sm   = self.search_matches.clone();
                        let opts = SearchOpts { case_sensitive: self.search_case_sensitive, whole_word: self.search_whole_word };
                        let ql   = needle_len(&self.search_query, opts);
                        let sc   = self.search_current;
                        let sq   = self.search_query.clone();
                        let sto  = self.search_scroll_to_offset.take();
                        let tab  = &mut self.tabs[idx];
                        let hl   = &mut self.highlighter;
                        ui.columns(2, |cols| {
                            render_editor(
                                &mut cols[0],
                                &mut tab.buffer,
                                &mut tab.modified,
                                &mut tab.needs_reparse,
                                "split_editor",
                                &sm,
                                ql,
                                sc,
                                sto,
                            );
                            render_preview(
                                &mut cols[1],
                                &tab.parsed_doc,
                                &tab.buffer,
                                scroll_to,
                                "split_preview",
                                hl,
                                &sq,
                                sc,
                                opts,
                            );
                        });
                    }
                },
            }
        });

        // ── Floating search bar ───────────────────────────────────────────
        if self.search_open {
            let mut navigate:    Option<bool> = None; // Some(true)=next, Some(false)=prev
            let mut close        = false;
            let mut toggle_case  = false;
            let mut toggle_word  = false;
            let request_focus = self.search_request_focus;
            self.search_request_focus = false;

            egui::Area::new(egui::Id::new("search_bar"))
                .anchor(egui::Align2::RIGHT_TOP, [-8.0, 110.0])
                .order(egui::Order::Foreground)
                .show(ctx, |ui| {
                    egui::Frame::popup(ui.style()).show(ui, |ui| {
                        ui.set_min_width(300.0);
                        ui.horizontal(|ui| {
                            // Query input — stable ID lets us check its focus state.
                            let te = egui::TextEdit::singleline(&mut self.search_query)
                                .hint_text("Search…")
                                .desired_width(160.0)
                                .id(egui::Id::new("search_input"));
                            let resp = ui.add(te);
                            if request_focus { resp.request_focus(); }
                            if resp.changed() { self.update_search_matches(); }

                            // Match count badge
                            let badge = if self.search_query.is_empty() {
                                String::new()
                            } else if self.search_matches.is_empty() {
                                "No results".to_string()
                            } else {
                                format!("{} / {}", self.search_current + 1, self.search_matches.len())
                            };
                            ui.label(egui::RichText::new(badge).color(egui::Color32::GRAY));

                            // Option toggles: Match Case (Aa) and Match Whole Word (W)
                            if ui.add(egui::Button::new("Aa").selected(self.search_case_sensitive))
                                .on_hover_text("Match Case (Alt+C)")
                                .clicked()
                            {
                                toggle_case = true;
                            }
                            if ui.add(egui::Button::new("W").selected(self.search_whole_word))
                                .on_hover_text("Match Whole Word (Alt+W)")
                                .clicked()
                            {
                                toggle_word = true;
                            }

                            // Navigation buttons
                            let has = !self.search_matches.is_empty();
                            if ui.add_enabled(has, egui::Button::new("▲")).clicked() {
                                navigate = Some(false);
                            }
                            if ui.add_enabled(has, egui::Button::new("▼")).clicked() {
                                navigate = Some(true);
                            }

                            // Close button
                            if ui.button("✕").clicked() { close = true; }
                        });
                    });
                });

            if toggle_case {
                self.search_case_sensitive = !self.search_case_sensitive;
                self.update_search_matches();
            }
            if toggle_word {
                self.search_whole_word = !self.search_whole_word;
                self.update_search_matches();
            }
            if let Some(fwd) = navigate {
                self.search_navigate(fwd);
            }
            if close {
                self.search_open = false;
                self.search_matches.clear();
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_state();
    }
}

fn render_preview(
    ui:             &mut egui::Ui,
    doc:            &Option<ParsedDoc>,
    buffer:         &str,
    scroll_to:      Option<usize>,
    id:             &str,
    hl:             &mut Highlighter,
    search_query:   &str,
    search_current: usize,
    opts:           SearchOpts,
) {
    ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            if let Some(doc) = doc {
                crate::markdown::render_markdown(ui, doc, scroll_to, hl, search_query, search_current, opts);
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
    ui:               &mut egui::Ui,
    buffer:           &mut String,
    modified:         &mut bool,
    needs_reparse:    &mut bool,
    id:               &str,
    search_matches:   &[usize],
    query_len:        usize,
    current_match:    usize,
    scroll_to_offset: Option<usize>,
) {
    ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false; 2])
        .show(ui, |ui| {
            let matches   = search_matches.to_vec(); // copy for closure
            let cur       = current_match;
            let qlen      = query_len;
            let text_color = ui.visuals().text_color();

            let mut layouter = move |ui: &egui::Ui, text: &str, wrap: f32| {
                use egui::text::{LayoutJob, TextFormat};
                use egui::FontId;

                let mut job = LayoutJob::default();
                let normal = TextFormat {
                    font_id: FontId::monospace(13.0),
                    color:   text_color,
                    ..Default::default()
                };

                if matches.is_empty() || qlen == 0 {
                    job.append(text, 0.0, normal);
                } else {
                    let mut pos = 0usize;
                    for (mi, &start) in matches.iter().enumerate() {
                        // Skip any offset that is out-of-range or not on a char boundary
                        // (can happen when the buffer was edited this frame and matches
                        // haven't been recomputed yet).
                        if start >= text.len() || !text.is_char_boundary(start) { break; }
                        // Advance end to the nearest char boundary.
                        let raw_end = (start + qlen).min(text.len());
                        let end = {
                            let mut e = raw_end;
                            while e < text.len() && !text.is_char_boundary(e) { e += 1; }
                            e
                        };
                        if start > pos && text.is_char_boundary(pos) {
                            job.append(&text[pos..start], 0.0, normal.clone());
                        }
                        let bg = if mi == cur {
                            egui::Color32::from_rgb(255, 150, 0) // orange = current
                        } else {
                            egui::Color32::from_rgb(255, 220, 50) // yellow = other
                        };
                        job.append(&text[start..end], 0.0, TextFormat {
                            font_id:    FontId::monospace(13.0),
                            color:      egui::Color32::BLACK,
                            background: bg,
                            ..Default::default()
                        });
                        pos = end;
                    }
                    if pos < text.len() && text.is_char_boundary(pos) {
                        job.append(&text[pos..], 0.0, normal);
                    }
                }

                job.wrap.max_width = wrap;
                ui.fonts(|f| f.layout_job(job))
            };

            let te_id = egui::Id::new(id).with("te");
            // Use TextEdit::show() (instead of ui.add) to access the galley for
            // accurate cursor-rect scrolling.
            let output = TextEdit::multiline(buffer)
                .id(te_id)
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .desired_rows(40)
                .layouter(&mut layouter)
                .show(ui);

            // Scroll so the current search match is visible: convert the byte offset
            // to a char cursor, ask the galley for the pixel rect, then tell the
            // enclosing ScrollArea to bring it into view.
            if let Some(byte_offset) = scroll_to_offset {
                let safe_offset = byte_offset.min(buffer.len());
                let char_idx    = buffer[..safe_offset].chars().count();
                let ccursor      = egui::text::CCursor::new(char_idx);
                let cursor       = output.galley.from_ccursor(ccursor);
                let local_rect   = output.galley.pos_from_cursor(&cursor);
                // Translate from galley-local coords to screen coords.
                let screen_rect  = local_rect.translate(output.response.rect.min.to_vec2());
                ui.scroll_to_rect(screen_rect, Some(egui::Align::Center));
            }

            if output.response.changed() {
                *modified      = true;
                *needs_reparse = true;
            }
        });
}

// ── Search helpers ────────────────────────────────────────────────────────────

/// Extract searchable plain text from a block (strips markdown syntax).
fn block_plain_text(block: &crate::markdown::Block) -> String {
    use crate::markdown::{Block, Inline};

    fn inlines_text(inlines: &[Inline]) -> String {
        inlines.iter().map(|il| match il {
            Inline::Text(s) | Inline::Bold(s) | Inline::Italic(s)
            | Inline::BoldItalic(s) | Inline::Code(s) => s.as_str(),
            Inline::Link(t, _) => t.as_str(),
        }).collect::<Vec<_>>().join("")
    }

    match block {
        Block::Heading(_, ils) | Block::Paragraph(ils) | Block::BlockQuote(ils) => {
            inlines_text(ils)
        }
        Block::List(_, items) => items.iter().map(|i| inlines_text(i)).collect::<Vec<_>>().join("\n"),
        Block::CodeBlock(_, code) => code.clone(),
        Block::Table(headers, rows) => {
            let mut s = headers.iter().map(|h| inlines_text(h)).collect::<Vec<_>>().join(" ");
            for row in rows {
                s.push(' ');
                s.push_str(&row.iter().map(|cell| inlines_text(cell)).collect::<Vec<_>>().join(" "));
            }
            s
        }
        Block::Rule => String::new(),
    }
}

/// Count non-overlapping matches of `needle` in `haystack` (already lowercased if needed),
/// respecting whole-word option. Needle/haystack must already have the same case transformation.
fn count_matches(haystack: &str, needle: &str, opts: SearchOpts) -> usize {
    if needle.is_empty() { return 0; }
    let nlen = needle.len();
    let mut count = 0;
    let mut p = 0;
    while p < haystack.len() {
        match haystack[p..].find(needle) {
            None => break,
            Some(rel) => {
                let ms = p + rel;
                let me = ms + nlen;
                if !opts.whole_word || is_word_boundary(haystack, ms, me) {
                    count += 1;
                    p = me;
                } else {
                    p = ms + haystack[ms..].chars().next().map_or(1, |c| c.len_utf8());
                }
            }
        }
    }
    count
}

/// True if the substring `haystack[start..end]` is bounded by non-word characters.
fn is_word_boundary(text: &str, start: usize, end: usize) -> bool {
    let before = text[..start].chars().next_back();
    let after  = text[end..].chars().next();
    let is_word = |c: char| c.is_alphanumeric() || c == '_';
    !before.map(is_word).unwrap_or(false) && !after.map(is_word).unwrap_or(false)
}

/// Byte length of the search needle after case normalisation.
fn needle_len(query: &str, opts: SearchOpts) -> usize {
    if opts.case_sensitive { query.len() } else { query.to_lowercase().len() }
}
