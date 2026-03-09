use egui::{CentralPanel, Key, ScrollArea, SidePanel, TextEdit, TopBottomPanel};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use notify::Watcher;
use crate::fs::FsTree;
use crate::markdown::{parse_markdown, Highlighter, ParsedDoc, SearchOpts};
use crate::persist;
use crate::theme::ThemeId;
use crate::ui::{render_outline, render_sidebar};

/// Preview body font choice — persisted across sessions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyFont {
    SourceSans,
    Nunito,
    Rubik,
    Figtree,
    Manrope,
}

impl BodyFont {
    pub fn name(self) -> &'static str {
        match self {
            BodyFont::SourceSans => "Source Sans",
            BodyFont::Nunito    => "Nunito",
            BodyFont::Rubik     => "Rubik",
            BodyFont::Figtree   => "Figtree",
            BodyFont::Manrope   => "Manrope",
        }
    }

    /// The egui FontFamily registered in setup_fonts for this choice.
    pub fn family(self) -> egui::FontFamily {
        match self {
            BodyFont::SourceSans => egui::FontFamily::Name("Body-Sans".into()),
            BodyFont::Nunito    => egui::FontFamily::Name("Body-Nunito".into()),
            BodyFont::Rubik     => egui::FontFamily::Name("Body-Rubik".into()),
            BodyFont::Figtree   => egui::FontFamily::Name("Body-Figtree".into()),
            BodyFont::Manrope   => egui::FontFamily::Name("Body-Manrope".into()),
        }
    }

    fn from_str(s: &str) -> Self {
        match s {
            "source_sans" => BodyFont::SourceSans,
            "nunito"      => BodyFont::Nunito,
            "figtree"     => BodyFont::Figtree,
            "manrope"     => BodyFont::Manrope,
            _             => BodyFont::Rubik,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            BodyFont::SourceSans => "source_sans",
            BodyFont::Nunito    => "nunito",
            BodyFont::Rubik     => "rubik",
            BodyFont::Figtree   => "figtree",
            BodyFont::Manrope   => "manrope",
        }
    }
}

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
    /// Last known scroll offset (px) for the editor pane (Edit / Split-left).
    editor_scroll_y: f32,
    /// Last known scroll offset (px) for the preview pane (Preview / Split-right).
    preview_scroll_y: f32,
    /// Content-relative Y position for each heading block (block_idx, content_y).
    /// Updated each render; used to derive the active outline entry from scroll offset.
    heading_positions: Vec<(usize, f32)>,
    /// Last active outline block; used to detect when highlight changes (for auto-scroll).
    last_active_outline_block: Option<usize>,
    /// Whether the preview is scrolled to the bottom of the document.
    preview_at_bottom: bool,
}

pub struct App {
    /// One entry per open folder: (tree, optional file-system watcher).
    roots: Vec<(FsTree, Option<FileWatcher>)>,

    tabs:       Vec<OpenTab>,
    active_tab: Option<usize>,

    highlighter: Highlighter,

    view_mode: ViewMode,
    active_theme: ThemeId,
    active_body_font: BodyFont,

    recent_files: Vec<PathBuf>,

    pending_action: Option<PendingAction>,
    dialog_focused_button: usize, // 0=Save, 1=Discard, 2=Cancel

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

    // Status bar — last known cursor position in the editor (1-indexed line, col).
    cursor_pos: Option<(usize, usize)>,

    // Split-mode pane ratio: fraction of width given to the editor (0.2–0.8).
    split_ratio: f32,

    // When true, sidebar width is re-fitted to the longest visible label next frame.
    recalc_sidebar_width: bool,

    // Zoom level applied to editor/preview content only (default 1.0).
    content_zoom: f32,

    // Whether the file-tree sidebar is visible.
    sidebar_open: bool,

    // Fraction of sidebar height given to the file-tree panel (resizable divider).
    file_panel_ratio: f32,

    // When true, open a save-file dialog and export the active doc as PDF.
    pending_export_pdf: bool,

    // Track last-applied theme and title to avoid redundant egui calls that
    // trigger repaints every frame.
    last_applied_theme: Option<ThemeId>,
    last_window_title:  String,
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
            roots:             Vec::new(),
            tabs:              Vec::new(),
            active_tab:        None,
            highlighter:       Highlighter::new(),
            view_mode:         ViewMode::from_str(&state.view_mode),
            active_theme:      Self::parse_theme(&state.theme),
            active_body_font:  BodyFont::from_str(&state.body_font),
            recent_files:      state.recent_files.into_iter().filter(|p| p.is_file()).collect(),
            pending_action:       None,
            dialog_focused_button: 0,
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
            cursor_pos:           None,
            split_ratio:          state.split_ratio.unwrap_or(0.5),
            recalc_sidebar_width: false,
            content_zoom:         1.0,
            sidebar_open:         true,
            file_panel_ratio:     0.25,
            pending_export_pdf:   false,
            last_applied_theme:   None,
            last_window_title:    String::new(),
        };

        if let Some(path) = initial_path {
            // CLI argument takes precedence; ignore persisted tabs/roots.
            if !path.exists() {
                eprintln!("md_reader: path not found: {}", path.display());
            } else if path.is_dir() {
                app.add_root(path);
            } else if path.is_file() {
                if let Some(parent) = path.parent() {
                    app.add_root(parent.to_path_buf());
                }
                app.open_tab(path);
            }
        } else {
            // Restore last session.
            for dir in state.root_dirs {
                if dir.is_dir() {
                    app.add_root(dir);
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
                app.active_tab = Some(idx);
                let p = app.tabs[idx].path.clone();
                app.set_selected(Some(p));
            }
        }

        app
    }

    fn parse_theme(s: &str) -> ThemeId {
        match s {
            "coal" => ThemeId::Coal,
            "navy" => ThemeId::Navy,
            "ayu" => ThemeId::Ayu,
            _ => ThemeId::Light,
        }
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
        let theme_str = match self.active_theme {
            ThemeId::Light => "light",
            ThemeId::Coal => "coal",
            ThemeId::Navy => "navy",
            ThemeId::Ayu => "ayu",
        };
        let state = persist::AppState {
            root_dirs:    self.roots.iter()
                              .filter_map(|(t, _)| t.root.as_ref().map(|n| n.path.clone()))
                              .collect(),
            open_tabs:    self.tabs.iter().map(|t| t.path.clone()).collect(),
            active_tab:   self.active_tab,
            view_mode:    self.view_mode.as_str().to_string(),
            recent_files: self.recent_files.clone(),
            theme:        theme_str.to_string(),
            body_font:    self.active_body_font.as_str().to_string(),
            split_ratio:  Some(self.split_ratio),
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
            self.set_selected(Some(path));
            return;
        }
        match std::fs::read_to_string(&path) {
            Ok(content) => {
                self.set_selected(Some(path.clone()));
                self.tabs.push(OpenTab {
                    path,
                    buffer:          content,
                    modified:        false,
                    parsed_doc:      None,
                    needs_reparse:   true,
                    extern_modified: false,
                    editor_scroll_y:     0.0,
                    preview_scroll_y:    0.0,
                    heading_positions: Vec::new(),
                    last_active_outline_block: None,
                    preview_at_bottom: false,
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
        let sel = self.active_tab.map(|i| self.tabs[i].path.clone());
        self.set_selected(sel);
    }

    /// Add a root folder and its file-system watcher.
    fn add_root(&mut self, path: PathBuf) {
        // Don't add the same root twice.
        if self.roots.iter().any(|(t, _)| t.root.as_ref().map(|n| &n.path) == Some(&path)) {
            return;
        }
        let watcher = FileWatcher::new(&path);
        self.roots.push((FsTree::new(path), watcher));
        self.recalc_sidebar_width = true;
    }

    /// Set `selected` on every root tree (the one containing the file will highlight it).
    fn set_selected(&mut self, path: Option<PathBuf>) {
        for (tree, _) in &mut self.roots {
            tree.selected = path.clone();
        }
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
            PendingAction::CloseTab(idx) => self.tabs.get(*idx).is_some_and(|t| t.modified),
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
                if tab.modified { format!("Markdown Reader — {name} ●") } else { format!("Markdown Reader — {name}") }
            }
            None => "Markdown Reader".to_string(),
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
        // Only update title when it actually changes (sending ViewportCommand every frame
        // causes unnecessary work in the windowing system).
        let title = self.window_title();
        if title != self.last_window_title {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(title.clone()));
            self.last_window_title = title;
        }

        // ── Apply theme ────────────────────────────────────────────────────
        // Guard: ctx.set_style() / ctx.set_visuals() call request_repaint() internally,
        // so calling apply_theme() unconditionally every frame creates a continuous
        // repaint loop even when the app is idle. Only apply when the theme changes.
        if self.last_applied_theme != Some(self.active_theme) {
            apply_theme(ctx, self.active_theme);
            self.last_applied_theme = Some(self.active_theme);
        }
        let theme = crate::theme::theme_by_id(self.active_theme);

        // ── Ctrl+Scroll zoom ──────────────────────────────────────────────
        let ctrl_scroll = ctx.input(|i| {
            if i.modifiers.ctrl {
                i.raw_scroll_delta.y
            } else {
                0.0
            }
        });
        if ctrl_scroll != 0.0 {
            ctx.input_mut(|i| i.raw_scroll_delta.y = 0.0); // consume so it doesn't scroll content
            let factor = if ctrl_scroll > 0.0 { 1.1_f32 } else { 1.0 / 1.1 };
            self.content_zoom = (self.content_zoom * factor).clamp(0.4, 4.0);
        }

        // ── Keyboard shortcuts ────────────────────────────────────────────
        let ctrl_s     = ctx.input(|i| i.key_pressed(Key::S)        && i.modifiers.ctrl);
        let ctrl_o     = ctx.input(|i| i.key_pressed(Key::O)        && i.modifiers.ctrl);
        let ctrl_w     = ctx.input(|i| i.key_pressed(Key::W)        && i.modifiers.ctrl);
        let ctrl_n     = ctx.input(|i| i.key_pressed(Key::N)        && i.modifiers.ctrl);
        let ctrl_q     = ctx.input(|i| i.key_pressed(Key::Q)        && i.modifiers.ctrl);
        let ctrl_b     = ctx.input(|i| i.key_pressed(Key::B)        && i.modifiers.ctrl);
        let ctrl_f     = ctx.input(|i| i.key_pressed(Key::F)        && i.modifiers.ctrl);
        let ctrl_left  = ctx.input(|i| i.key_pressed(Key::PageUp)   && i.modifiers.ctrl);
        let ctrl_right = ctx.input(|i| i.key_pressed(Key::PageDown) && i.modifiers.ctrl);
        let page_up    = ctx.input(|i| i.key_pressed(Key::PageUp)    && !i.modifiers.ctrl);
        let page_down  = ctx.input(|i| i.key_pressed(Key::PageDown)  && !i.modifiers.ctrl);
        let arrow_up   = ctx.input(|i| i.key_pressed(Key::ArrowUp)   && i.modifiers.is_none());
        let arrow_down = ctx.input(|i| i.key_pressed(Key::ArrowDown)  && i.modifiers.is_none());
        let page_scroll: f32 = if page_up { 1.0 } else if page_down { -1.0 } else { 0.0 };
        // Arrow scroll fraction — positive = up, negative = down.  0.15 ≈ one line.
        let arrow_scroll: f32 = if arrow_up { 0.15 } else if arrow_down { -0.15 } else { 0.0 };
        let escape     = ctx.input(|i| i.key_pressed(Key::Escape));
        let enter      = ctx.input(|i| i.key_pressed(Key::Enter) && !i.modifiers.shift);
        let shift_enter= ctx.input(|i| i.key_pressed(Key::Enter) &&  i.modifiers.shift);

        if ctrl_b { self.sidebar_open = !self.sidebar_open; }
        if ctrl_s { self.save_active(); }
        if ctrl_w
            && let Some(idx) = self.active_tab {
                self.request_action(PendingAction::CloseTab(idx));
            }
        if ctrl_q && self.request_action(PendingAction::Quit) {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
        if ctrl_n { self.create_new_file(); }
        if ctrl_o
            && let Some(path) = rfd::FileDialog::new().pick_folder() {
                self.add_root(path);
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

        if (ctrl_left || ctrl_right)
            && !self.tabs.is_empty() {
                let cur  = self.active_tab.unwrap_or(0);
                let next = if ctrl_right {
                    (cur + 1).min(self.tabs.len() - 1)
                } else {
                    cur.saturating_sub(1)
                };
                self.active_tab = Some(next);
                let p = self.tabs[next].path.clone();
                self.set_selected(Some(p));
            }

        // ── Drag-and-drop ──────────────────────────────────────────────────
        ctx.input(|i| {
            for file in &i.raw.dropped_files {
                if let Some(path) = &file.path {
                    if path.is_file() && path.extension().is_some_and(|e| e == "md") {
                        // Markdown file: open it
                        self.open_tab(path.clone());
                    } else if path.is_dir() {
                        // Folder: add as root
                        self.add_root(path.clone());
                    }
                }
            }
        });

        // ── File-watcher events ───────────────────────────────────────────
        let mut rescan_tree = false;
        for (_, watcher) in &self.roots {
            if let Some(w) = watcher {
                while let Ok(Ok(event)) = w.rx.try_recv() {
                    use notify::EventKind::*;
                    match event.kind {
                        Modify(_) => {
                            for path in &event.paths {
                                if let Some(tab) = self.tabs.iter_mut().find(|t| &t.path == path) {
                                    if tab.modified {
                                        tab.extern_modified = true;
                                    } else if let Ok(content) = std::fs::read_to_string(&tab.path) {
                                        tab.buffer          = content;
                                        tab.needs_reparse   = true;
                                        tab.extern_modified = false;
                                    }
                                }
                            }
                        }
                        Create(_) | Remove(_) | Other => { rescan_tree = true; }
                        _ => {}
                    }
                }
            }
        }
        if rescan_tree {
            for (tree, _) in &mut self.roots {
                tree.rescan();
            }
        }

        // ── Re-parse active tab when needed ──────────────────────────────
        if let Some(idx) = self.active_tab
            && self.tabs[idx].needs_reparse {
                let doc = parse_markdown(&self.tabs[idx].buffer);
                self.tabs[idx].parsed_doc    = Some(doc);
                self.tabs[idx].needs_reparse = false;
            }

        // ── Unsaved-changes dialog ────────────────────────────────────────
        if self.pending_action.is_some() {
            let mut choice: Option<bool> = None;

            // Handle Tab/Shift+Tab to cycle focus, Enter to activate.
            // Use input_mut + consume_key so egui's own focus system never sees Tab.
            ctx.memory_mut(|m| m.stop_text_input()); // prevent background widgets from receiving key input
            let tab_fwd  = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE,  Key::Tab));
            let tab_back = ctx.input_mut(|i| i.consume_key(egui::Modifiers::SHIFT, Key::Tab));
            let enter    = ctx.input_mut(|i| i.consume_key(egui::Modifiers::NONE,  Key::Enter));

            if tab_fwd {
                self.dialog_focused_button = (self.dialog_focused_button + 1) % 3;
            }
            if tab_back {
                self.dialog_focused_button = if self.dialog_focused_button == 0 { 2 } else { self.dialog_focused_button - 1 };
            }
            if enter {
                match self.dialog_focused_button {
                    0 => choice = Some(true),       // Save
                    1 => choice = Some(false),       // Discard
                    _ => self.pending_action = None, // Cancel
                }
            }

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
                        // Save button
                        let save_text = if self.dialog_focused_button == 0 { "» Save" } else { "Save" };
                        if ui.button(save_text)
                            .on_hover_text("Save changes before closing")
                            .clicked() {
                            choice = Some(true);
                        }

                        // Discard button
                        let discard_text = if self.dialog_focused_button == 1 { "» Discard" } else { "Discard" };
                        if ui.button(discard_text)
                            .on_hover_text("Discard changes and close anyway")
                            .clicked() {
                            choice = Some(false);
                        }

                        // Cancel button
                        let cancel_text = if self.dialog_focused_button == 2 { "» Cancel" } else { "Cancel" };
                        if ui.button(cancel_text)
                            .on_hover_text("Go back and keep the file open")
                            .clicked() {
                            self.pending_action = None;
                            self.dialog_focused_button = 0;
                        }
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
                self.dialog_focused_button = 0;
            }
        }

        // ── View-mode restriction ─────────────────────────────────────────
        // Edit and Split only make sense for .md files.
        let active_is_md = self.active_tab
            .and_then(|i| self.tabs.get(i))
            .is_none_or(|t| is_markdown(&t.path));
        if !active_is_md && self.view_mode != ViewMode::Preview {
            self.view_mode = ViewMode::Preview;
        }

        // ── Toolbar ───────────────────────────────────────────────────────
        let mode_before = self.view_mode;

        TopBottomPanel::top("toolbar")
            .frame(egui::Frame::new()
                .fill(theme.toolbar_bg)
                .inner_margin(egui::Margin::symmetric(8, 4)))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                let sidebar_icon = if self.sidebar_open { "«" } else { "»" };
                let sidebar_tip  = if self.sidebar_open { "Hide sidebar (Ctrl+B)" } else { "Show sidebar (Ctrl+B)" };
                if ui.button(sidebar_icon).on_hover_text(sidebar_tip).clicked() {
                    self.sidebar_open = !self.sidebar_open;
                }

                ui.separator();

                if ui.button("Open Folder").on_hover_text("Open a folder in the sidebar").clicked()
                    && let Some(path) = rfd::FileDialog::new().pick_folder() {
                        self.add_root(path);
                    }

                // ── Recent files dropdown ─────────────────────────────────
                let mut open_path: Option<PathBuf> = None;
                let mut clear     = false;

                ui.menu_button("Recent", |ui| {
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
                                if ui.button("Clear history")
                                    .on_hover_text("Clear all recently opened files")
                                    .clicked() {
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

                ui.selectable_value(&mut self.view_mode, ViewMode::Preview, "Preview")
                    .on_hover_text("Preview markdown rendering");
                if active_is_md {
                    ui.selectable_value(&mut self.view_mode, ViewMode::Edit,  "Edit")
                        .on_hover_text("Edit raw markdown");
                    ui.selectable_value(&mut self.view_mode, ViewMode::Split, "Split")
                        .on_hover_text("View editor and preview side-by-side");
                }

                ui.separator();

                // Theme picker
                ui.menu_button("Theme", |ui| {
                    for theme in crate::theme::THEMES {
                        if ui.selectable_label(self.active_theme == theme.id, theme.id.name()).clicked() {
                            self.active_theme = theme.id;
                            ui.close_menu();
                        }
                    }
                }).response.on_hover_text("Choose color theme");

                // Preview body font picker
                ui.menu_button("Font", |ui| {
                    for &font in &[BodyFont::Rubik, BodyFont::Nunito, BodyFont::Figtree, BodyFont::Manrope, BodyFont::SourceSans] {
                        if ui.selectable_label(self.active_body_font == font, font.name()).clicked() {
                            self.active_body_font = font;
                            ui.close_menu();
                        }
                    }
                }).response.on_hover_text("Choose preview body font");

                // Export PDF (only when a parsed doc is available)
                let has_parsed_doc = self.active_tab
                    .and_then(|i| self.tabs.get(i))
                    .and_then(|t| t.parsed_doc.as_ref())
                    .is_some();
                if ui.add_enabled(has_parsed_doc, egui::Button::new("Export PDF"))
                    .on_hover_text("Export current document as PDF")
                    .clicked()
                {
                    self.pending_export_pdf = true;
                }

                ui.separator();

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let is_modified = self.active_tab
                        .and_then(|i| self.tabs.get(i))
                        .is_some_and(|t| t.modified);
                    let has_active = self.active_tab.is_some();

                    if ui.add_enabled(is_modified, egui::Button::new("Save"))
                        .on_hover_text("Save current file (Ctrl+S)")
                        .clicked() {
                        self.save_active();
                    }
                    if ui.add_enabled(has_active, egui::Button::new("Close"))
                        .on_hover_text("Close current file (Ctrl+W)")
                        .clicked()
                        && let Some(idx) = self.active_tab {
                            self.request_action(PendingAction::CloseTab(idx));
                        }
                });
            });
        });

        // ── Tab bar ───────────────────────────────────────────────────────
        TopBottomPanel::top("tab_bar")
            .frame(egui::Frame::new()
                .fill(theme.tab_bar_bg)
                .inner_margin(egui::Margin::symmetric(8, 4)))
            .show(ctx, |ui| {
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
                            format!(" {name} * ")
                        } else {
                            format!(" {name} ")
                        };
                        let is_active = self.active_tab == Some(i);
                        let full_path = tab.path.to_string_lossy();

                        // Tab label — use egui's native selectable_label so colors adapt to theme
                        if ui.selectable_label(is_active, &label)
                            .on_hover_text(full_path.as_ref())
                            .clicked() {
                            activate_idx = Some(i);
                        }

                        if ui.small_button("×")
                            .on_hover_text("Close this file (Ctrl+W)")
                            .clicked() {
                            close_idx = Some(i);
                        }

                        ui.add_space(4.0);
                    }

                    if let Some(i) = activate_idx {
                        self.active_tab = Some(i);
                        let p = self.tabs[i].path.clone();
                        self.set_selected(Some(p));
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
            .is_some_and(|t| t.extern_modified);

        if extern_mod {
            let mut reload = false;
            let mut keep   = false;
            TopBottomPanel::top("extern_mod_banner").show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("File changed on disk")
                            .color(egui::Color32::from_rgb(220, 160, 0))
                            .strong(),
                    );
                    ui.add_space(8.0);
                    if ui.button("Reload").clicked() { reload = true; }
                    if ui.button("Keep mine").clicked() { keep = true; }
                });
            });
            if reload
                && let Some(idx) = self.active_tab {
                    let tab = &mut self.tabs[idx];
                    if let Ok(content) = std::fs::read_to_string(&tab.path) {
                        tab.buffer          = content;
                        tab.modified        = false;
                        tab.needs_reparse   = true;
                        tab.extern_modified = false;
                    }
                }
            if keep
                && let Some(idx) = self.active_tab {
                    self.tabs[idx].extern_modified = false;
                }
        }

        // ── Sidebar ───────────────────────────────────────────────────────
        // ── Sidebar panel ─────────────────────────────────────────────────
        if self.sidebar_open {

        // Fit width to the longest visible label when a new root is added.
        if self.recalc_sidebar_width {
            self.recalc_sidebar_width = false;
            if let Some((tree, _)) = self.roots.last() {
                let max_w = ctx.screen_rect().width() * 0.25;
                let w = crate::ui::sidebar::ideal_width(ctx, tree).min(max_w);
                let panel_id = egui::Id::new("sidebar");
                ctx.data_mut(|d| {
                    d.insert_persisted(
                        panel_id,
                        egui::containers::panel::PanelState {
                            rect: egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(w, 0.0)),
                        },
                    );
                });
            }
        }

        SidePanel::left("sidebar")
            .min_width(200.0)
            .max_width(ctx.screen_rect().width() * 0.25)
            .default_width(250.0)
            .frame(egui::Frame::new()
                .fill(theme.sidebar_bg)
                .inner_margin(egui::Margin::same(8)))
            .show(ctx, |ui| {
                // Override text color for the sidebar — the global override uses
                // theme.fg (dark ink) which is invisible on dark-sidebar themes like Rust.
                ui.visuals_mut().override_text_color = Some(theme.sidebar_fg);

                // ── File-tree panel (resizable) ───────────────────────────
                let available_h = ui.available_height();
                let handle_h    = 6.0_f32;
                let file_h      = (available_h * self.file_panel_ratio)
                    .max(50.0)
                    .min(available_h - handle_h - 50.0);

                ScrollArea::vertical()
                    .id_salt("file_panel")
                    .max_height(file_h)
                    .auto_shrink([false; 2])
                    .show(ui, |ui| {
                        let mut open_path:  Option<PathBuf> = None;
                        let mut close_root: Option<usize>   = None;

                        if self.roots.is_empty() {
                            ui.add_space(8.0);
                            ui.label(
                                egui::RichText::new("No folder open")
                                    .color(ui.visuals().weak_text_color())
                                    .size(13.0),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new("Open Folder or Ctrl+O")
                                    .color(ui.visuals().weak_text_color())
                                    .size(11.0),
                            );
                        }

                        for i in 0..self.roots.len() {
                            let root_name = self.roots[i].0.root.as_ref()
                                .map(|n| n.name.clone())
                                .unwrap_or_else(|| "?".to_string());

                            // Root header: uppercase section label + close button (inline)
                            ui.horizontal(|ui| {
                                ui.label(
                                    egui::RichText::new(root_name.to_uppercase())
                                        .size(10.5)
                                        .color(ui.visuals().weak_text_color()),
                                );
                                if ui.add(
                                    egui::Label::new(
                                        egui::RichText::new("×")
                                            .size(12.0)
                                            .color(ui.visuals().weak_text_color()),
                                    ).sense(egui::Sense::click()),
                                )
                                .on_hover_text("Close folder")
                                .clicked() {
                                    close_root = Some(i);
                                }
                            });
                            // Thin separator line
                            {
                                let (_, r) = ui.allocate_space(egui::vec2(ui.available_width(), 1.0));
                                ui.painter().rect_filled(r, 0.0, ui.visuals().widgets.noninteractive.bg_stroke.color);
                            }
                            ui.add_space(2.0);

                            if let Some(path) = render_sidebar(ui, &mut self.roots[i].0, theme.sidebar_active) {
                                open_path = Some(path);
                            }
                            ui.add_space(4.0);
                        }

                        if let Some(path) = open_path  { self.open_tab(path); }
                        if let Some(i) = close_root     { self.roots.remove(i); }
                    });

                // ── Resize handle ─────────────────────────────────────────
                let (handle_rect, handle_resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), handle_h),
                    egui::Sense::drag(),
                );
                let sep_color = ui.visuals().widgets.noninteractive.bg_stroke.color;
                ui.painter().hline(
                    handle_rect.x_range(),
                    handle_rect.center().y,
                    egui::Stroke::new(1.0, sep_color),
                );
                if handle_resp.hovered() || handle_resp.dragged() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
                    ui.painter().hline(
                        handle_rect.x_range(),
                        handle_rect.center().y,
                        egui::Stroke::new(2.0, theme.sidebar_active),
                    );
                }
                if handle_resp.dragged() {
                    let new_file_h = (file_h + handle_resp.drag_delta().y)
                        .max(50.0)
                        .min(available_h - handle_h - 50.0);
                    self.file_panel_ratio = new_file_h / available_h;
                }

                // ── Outline panel ─────────────────────────────────────────
                // Outline — only for .md files with a parsed doc
                let has_doc = self.active_tab
                    .and_then(|i| self.tabs.get(i))
                    .is_some_and(|t| t.parsed_doc.is_some() && is_markdown(&t.path));

                if has_doc {
                    let idx = self.active_tab.unwrap();
                    let active_blk = self.active_tab.and_then(|i| {
                        let tab = &self.tabs[i];
                        if tab.preview_at_bottom && !tab.heading_positions.is_empty() {
                            tab.heading_positions.last().map(|(idx, _)| *idx)
                        } else {
                            tab.heading_positions.iter().rfind(|(_, y)| *y <= tab.preview_scroll_y)
                                .map(|(idx, _)| *idx)
                        }
                    });
                    let last_active = self.tabs[idx].last_active_outline_block;
                    ScrollArea::vertical()
                        .id_salt("outline_panel")
                        .auto_shrink([false; 2])
                        .show(ui, |ui| {
                            let doc = self.tabs[idx].parsed_doc.as_ref().unwrap();
                            if let Some(block_idx) = render_outline(
                                ui,
                                doc,
                                &mut self.outline_open,
                                &mut self.outline_collapsed,
                                theme.sidebar_active,
                                active_blk,
                                last_active,
                            ) {
                                self.scroll_to_block = Some(block_idx);
                            }
                        });
                    self.tabs[idx].last_active_outline_block = active_blk;
                }
            });

        // ── Status bar ────────────────────────────────────────────────────
        TopBottomPanel::bottom("status_bar")
            .frame(egui::Frame::new()
                .fill(theme.toolbar_bg)
                .inner_margin(egui::Margin::symmetric(8, 3)))
            .show(ctx, |ui| {
            ui.horizontal(|ui| {
                if let Some(tab) = self.active_tab.and_then(|i| self.tabs.get(i)) {
                    let line_count = tab.buffer.lines().count().max(1);
                    let word_count = tab.buffer.split_whitespace().count();
                    if let Some((row, col)) = self.cursor_pos {
                        ui.label(egui::RichText::new(format!("Ln {row}, Col {col}"))
                            .color(egui::Color32::GRAY)
                            .size(11.0));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(format!("Lines: {line_count}  Words: {word_count}"))
                            .color(egui::Color32::GRAY)
                            .size(11.0));
                    });
                }
            });
        });

        } // end sidebar_open

        // ── Central panel ─────────────────────────────────────────────────
        let scroll_to   = self.scroll_to_block.take();
        // On mode switch, apply the stored pane offset so the new mode opens at the
        // same position as its counterpart. vertical_scroll_offset() overrides egui's
        // stored state for that single frame; normal scrolling resumes immediately after.
        let mode_switched = mode_before != self.view_mode;
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
                        self.cursor_pos = None;
                        let sync = mode_switched.then(|| self.tabs[idx].preview_scroll_y);
                        let (toggled, offset, heading_positions, at_bottom, footnote_scroll) = {
                            let tab  = &self.tabs[idx];
                            let sq   = self.search_query.as_str();
                            let sc   = self.search_current;
                            let opts = SearchOpts { case_sensitive: self.search_case_sensitive, whole_word: self.search_whole_word };
                            render_preview(ui, &tab.parsed_doc, &tab.buffer, scroll_to, page_scroll + arrow_scroll, &format!("preview_{idx}"), &mut self.highlighter, sq, sc, opts, sync, self.content_zoom, &self.active_body_font.family())
                        };
                        self.tabs[idx].preview_scroll_y = offset;
                        self.tabs[idx].heading_positions = heading_positions;
                        self.tabs[idx].preview_at_bottom = at_bottom;
                        if let Some(target) = footnote_scroll {
                            self.scroll_to_block = Some(target);
                        }
                        if let Some(task_idx) = toggled {
                            toggle_task_in_buffer(&mut self.tabs[idx].buffer, task_idx);
                            self.tabs[idx].needs_reparse = true;
                            let _ = std::fs::write(&self.tabs[idx].path, &self.tabs[idx].buffer);
                        }
                    }
                    ViewMode::Edit => {
                        let sm   = self.search_matches.as_slice();
                        let opts = SearchOpts { case_sensitive: self.search_case_sensitive, whole_word: self.search_whole_word };
                        let ql   = needle_len(&self.search_query, opts);
                        let sc   = self.search_current;
                        let tc   = make_token_colors(theme);
                        let sync = mode_switched.then(|| self.tabs[idx].editor_scroll_y);
                        // Outline click: convert block index → raw buffer byte offset.
                        let outline_sto = scroll_to.and_then(|bi| {
                            let tab = &self.tabs[idx];
                            tab.parsed_doc.as_ref().and_then(|doc| {
                                heading_byte_offset(&tab.buffer, &doc.blocks, bi)
                            })
                        });
                        let sto = self.search_scroll_to_offset.take().or(outline_sto);
                        let buffer_changed = {
                            let tab = &mut self.tabs[idx];
                            let before = tab.needs_reparse;
                            let (cpos, offset) = render_editor(ui, &mut tab.buffer, &mut tab.modified, &mut tab.needs_reparse, &format!("editor_{idx}"), sm, ql, sc, sto, page_scroll, tc, sync, self.content_zoom);
                            self.cursor_pos = cpos;
                            tab.editor_scroll_y = offset;
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
                        // Outline click: same byte-offset conversion for the editor pane.
                        let outline_sto = scroll_to.and_then(|bi| {
                            let tab = &self.tabs[idx];
                            tab.parsed_doc.as_ref().and_then(|doc| {
                                heading_byte_offset(&tab.buffer, &doc.blocks, bi)
                            })
                        });
                        let sto  = self.search_scroll_to_offset.take().or(outline_sto);
                        let tc   = make_token_colors(theme);
                        let zoom = self.content_zoom;
                        let tab  = &mut self.tabs[idx];
                        let hl   = &mut self.highlighter;
                        let split_cursor = {
                        // Manual split layout — avoids ui.columns() which always splits 50/50.
                        let available = ui.available_rect_before_wrap();
                        let total_w   = available.width();
                        let handle_w  = 6.0_f32;
                        let left_w    = ((total_w - handle_w) * self.split_ratio).max(80.0);
                        let right_w   = (total_w - handle_w - left_w).max(80.0);
                        let _ = right_w; // used implicitly via right_rect

                        let left_rect   = egui::Rect::from_min_max(available.min,
                            egui::pos2(available.min.x + left_w, available.max.y));
                        let handle_rect = egui::Rect::from_min_max(
                            egui::pos2(available.min.x + left_w, available.min.y),
                            egui::pos2(available.min.x + left_w + handle_w, available.max.y));
                        let right_rect  = egui::Rect::from_min_max(
                            egui::pos2(available.min.x + left_w + handle_w, available.min.y),
                            available.max);

                        // Drag handle interaction
                        let handle_id   = ui.id().with("split_handle");
                        let handle_resp = ui.interact(handle_rect, handle_id, egui::Sense::drag());
                        if handle_resp.dragged() {
                            let new_left = (left_w + handle_resp.drag_delta().x)
                                .max(80.0)
                                .min(total_w - handle_w - 80.0);
                            self.split_ratio = (new_left / (total_w - handle_w)).clamp(0.25, 0.75);
                        }
                        if handle_resp.hovered() || handle_resp.dragged() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                        }

                        // Draw visual divider (brighter when active)
                        let divider_color = if handle_resp.hovered() || handle_resp.dragged() {
                            ui.visuals().widgets.active.bg_fill
                        } else {
                            ui.visuals().widgets.noninteractive.bg_stroke.color
                        };
                        ui.painter().vline(
                            handle_rect.center().x,
                            available.y_range(),
                            egui::Stroke::new(1.5, divider_color),
                        );

                        // Only scroll the pane the pointer is hovering over (mirrors mouse-wheel behaviour).
                        let pointer = ctx.pointer_latest_pos();
                        let left_scroll  = if pointer.is_some_and(|p| left_rect.contains(p))  { page_scroll } else { 0.0 };
                        let right_scroll = if pointer.is_some_and(|p| right_rect.contains(p)) { page_scroll + arrow_scroll } else { 0.0 };

                        // Render editor and preview as bounded children.
                        // On mode switch, restore each pane's last known position.
                        let editor_sync  = mode_switched.then_some(tab.editor_scroll_y);
                        let preview_sync = mode_switched.then_some(tab.preview_scroll_y);

                        let mut left_ui = ui.new_child(egui::UiBuilder::new().max_rect(left_rect));
                        let (ecursor, eoffset) = render_editor(
                            &mut left_ui,
                            &mut tab.buffer,
                            &mut tab.modified,
                            &mut tab.needs_reparse,
                            &format!("split_editor_{idx}"),
                            &sm,
                            ql,
                            sc,
                            sto,
                            left_scroll,
                            tc,
                            editor_sync,
                            zoom,
                        );
                        tab.editor_scroll_y = eoffset;

                        let mut right_ui = ui.new_child(egui::UiBuilder::new().max_rect(right_rect));
                        let (preview_toggled, poffset, preview_heading_positions, preview_at_bottom, footnote_scroll) = render_preview(
                            &mut right_ui,
                            &tab.parsed_doc,
                            &tab.buffer,
                            scroll_to,
                            right_scroll,
                            &format!("split_preview_{idx}"),
                            hl,
                            &sq,
                            sc,
                            opts,
                            preview_sync,
                            zoom,
                            &self.active_body_font.family(),
                        );
                        tab.preview_scroll_y = poffset;
                        tab.heading_positions = preview_heading_positions;
                        tab.preview_at_bottom = preview_at_bottom;
                        if let Some(target) = footnote_scroll {
                            self.scroll_to_block = Some(target);
                        }
                        if let Some(task_idx) = preview_toggled {
                            toggle_task_in_buffer(&mut tab.buffer, task_idx);
                            tab.needs_reparse = true;
                            let _ = std::fs::write(&tab.path, &tab.buffer);
                        }

                        // Advance parent cursor to cover the full area we occupied
                        ui.advance_cursor_after_rect(available);
                        ecursor
                        };
                        self.cursor_pos = split_cursor;
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
                            if ui.add_enabled(has, egui::Button::new("<")).on_hover_text("Previous match").clicked() {
                                navigate = Some(false);
                            }
                            if ui.add_enabled(has, egui::Button::new(">")).on_hover_text("Next match").clicked() {
                                navigate = Some(true);
                            }

                            // Close button
                            if ui.button("×").clicked() { close = true; }
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

        // ── PDF export ────────────────────────────────────────────────────
        if self.pending_export_pdf {
            self.pending_export_pdf = false;
            if let Some(idx) = self.active_tab {
                if let Some(doc) = self.tabs.get(idx).and_then(|t| t.parsed_doc.as_ref()).cloned() {
                    let default_name = self.tabs[idx].path
                        .file_stem()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string()
                        + ".pdf";
                    if let Some(dest) = rfd::FileDialog::new()
                        .set_file_name(&default_name)
                        .add_filter("PDF", &["pdf"])
                        .save_file()
                    {
                        if let Err(e) = crate::pdf_export::export_pdf(&doc, &dest) {
                            eprintln!("PDF export failed: {e}");
                        }
                    }
                }
            }
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.save_state();
    }
}

#[allow(clippy::too_many_arguments)]
fn render_preview(
    ui:             &mut egui::Ui,
    doc:            &Option<ParsedDoc>,
    buffer:         &str,
    scroll_to:      Option<usize>,
    page_scroll:    f32,
    id:             &str,
    hl:             &mut Highlighter,
    search_query:   &str,
    search_current: usize,
    opts:           SearchOpts,
    sync_scroll:    Option<f32>,
    zoom:           f32,
    body_font:      &egui::FontFamily,
) -> (Option<usize>, f32, Vec<(usize, f32)>, bool, Option<usize>) {
    // Measure from max_rect, which correctly reflects the column width in split mode.
    // clip_rect() would return the parent panel's full width because ui.columns() does
    // not narrow the clip rect — only max_rect is set to the column's allocated rect.
    // Reading max_rect here (before the scroll area) is stable; it won't grow later.
    let viewport_w = ui.max_rect().width();
    let content_w  = (viewport_w - 48.0).clamp(200.0, 900.0);
    let side_pad   = ((viewport_w - content_w) / 2.0).max(24.0);

    let mut sa = ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false, false]);
    if let Some(y) = sync_scroll { sa = sa.vertical_scroll_offset(y); }
    let out = sa.show(ui, |ui| {
            if page_scroll != 0.0 {
                let clip = ui.clip_rect();
                let d = page_scroll.abs() * clip.height();
                if page_scroll > 0.0 {
                    ui.scroll_to_rect(
                        egui::Rect::from_x_y_ranges(clip.x_range(), (clip.max.y - d - 1.0)..=(clip.max.y - d)),
                        Some(egui::Align::BOTTOM),
                    );
                } else {
                    ui.scroll_to_rect(
                        egui::Rect::from_x_y_ranges(clip.x_range(), (clip.min.y + d)..=(clip.min.y + d + 1.0)),
                        Some(egui::Align::TOP),
                    );
                }
            }
            ui.add_space(24.0);

            let cursor = ui.cursor();
            let content_rect = egui::Rect::from_min_size(
                egui::pos2(cursor.min.x + side_pad, cursor.min.y),
                egui::vec2(content_w, 200_000.0),
            );

            // new_child() does NOT allocate space in the parent — we control that below.
            let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(content_rect));
            // Hard-clip painting to content_rect so nothing renders outside the column.
            child_ui.shrink_clip_rect(content_rect);

            let (toggled, positions, footnote_scroll) = if let Some(doc) = doc {
                crate::markdown::render_markdown(
                    &mut child_ui, doc, scroll_to, hl,
                    search_query, search_current, opts, zoom,
                    body_font,
                )
            } else {
                if !buffer.is_empty() {
                    child_ui.label("Failed to parse markdown.");
                }
                (None, Vec::new(), None)
            };

            // Advance the parent cursor using the child's actual height but clamping
            // max.x to content_rect.max.x — this prevents any layout overflow from
            // ever reaching the scroll area and triggering the growing-width feedback loop.
            let child_rect = child_ui.min_rect();
            let clamped = egui::Rect::from_min_max(
                child_rect.min,
                egui::pos2(content_rect.max.x, child_rect.max.y),
            );
            ui.advance_cursor_after_rect(clamped);

            ui.add_space(32.0);

            (toggled, positions, footnote_scroll)
        });
    let is_at_bottom = out.content_size.y > 0.0
        && out.state.offset.y + out.inner_rect.height() >= out.content_size.y - 1.0;
    (out.inner.0, out.state.offset.y, out.inner.1, is_at_bottom, out.inner.2)
}

/// Apply Tab indent or Shift+Tab unindent to the editor buffer based on the current cursor/selection.
/// Updates egui's TextEditState so the cursor follows the edit.  Returns true if the buffer changed.
fn editor_apply_indent(
    ctx:          &egui::Context,
    te_id:        egui::Id,
    buffer:       &mut String,
    cursor_range: egui::text::CursorRange,
    indent:       bool,
) -> bool {
    use egui::text::{CCursor, CCursorRange};
    use egui::text_edit::TextEditState;

    // ── helpers ────────────────────────────────────────────────────────────
    fn char_to_byte(s: &str, char_idx: usize) -> usize {
        s.char_indices().nth(char_idx).map_or(s.len(), |(b, _)| b)
    }
    fn byte_to_char(s: &str, byte_pos: usize) -> usize {
        s[..byte_pos.min(s.len())].chars().count()
    }
    fn line_start_byte(s: &str, byte_pos: usize) -> usize {
        s[..byte_pos].rfind('\n').map_or(0, |i| i + 1)
    }

    let primary_char   = cursor_range.primary.ccursor.index;
    let secondary_char = cursor_range.secondary.ccursor.index;
    let has_selection  = primary_char != secondary_char;

    let (new_primary, new_secondary) = if !has_selection {
        // ── No selection: operate on current line / cursor position ──────
        let cursor_byte = char_to_byte(buffer, primary_char);

        if indent {
            // Insert 4 spaces at the cursor position.
            buffer.insert_str(cursor_byte, "    ");
            let p = primary_char + 4;
            (p, p)
        } else {
            // Remove up to 4 leading spaces from the current line.
            let ls_byte = line_start_byte(buffer, cursor_byte);
            let spaces: usize = buffer[ls_byte..].chars().take(4).take_while(|&c| c == ' ').count();
            if spaces == 0 { return false; }
            buffer.drain(ls_byte..ls_byte + spaces);
            // Move cursor back by the removed count, but not before the line start.
            let new_byte = cursor_byte.saturating_sub(spaces).max(ls_byte);
            let p = byte_to_char(buffer, new_byte);
            (p, p)
        }
    } else {
        // ── Selection: indent/unindent every touched line ─────────────────
        let (sel_start, sel_end, primary_first) = if primary_char <= secondary_char {
            (primary_char, secondary_char, true)
        } else {
            (secondary_char, primary_char, false)
        };

        let sel_start_byte = char_to_byte(buffer, sel_start);
        let sel_end_byte   = char_to_byte(buffer, sel_end);
        let first_ls_byte  = line_start_byte(buffer, sel_start_byte);
        let first_ls_char  = byte_to_char(buffer, first_ls_byte);

        // Collect byte offsets of all line starts in the selection (forward order).
        let mut line_starts: Vec<usize> = vec![first_ls_byte];
        let mut pos = first_ls_byte;
        loop {
            match buffer[pos..].find('\n') {
                None => break,
                Some(rel) => {
                    let next = pos + rel + 1;
                    if next > sel_end_byte { break; }
                    line_starts.push(next);
                    pos = next;
                }
            }
        }

        if indent {
            // Insert 4 spaces at each line start, back-to-front to preserve offsets.
            for &ls in line_starts.iter().rev() {
                buffer.insert_str(ls, "    ");
            }
            let n = line_starts.len() * 4;
            let ns = sel_start + 4;
            let ne = sel_end + n;
            if primary_first { (ns, ne) } else { (ne, ns) }
        } else {
            // Remove up to 4 leading spaces from each line, back-to-front.
            let mut total_removed = 0usize;
            let mut first_removed = 0usize;
            for (i, &ls) in line_starts.iter().enumerate().rev() {
                let sp: usize = buffer[ls..].chars().take(4).take_while(|&c| c == ' ').count();
                if sp > 0 { buffer.drain(ls..ls + sp); }
                total_removed += sp;
                if i == 0 { first_removed = sp; }
            }
            if total_removed == 0 { return false; }
            let ns = (sel_start as isize - first_removed as isize)
                .max(first_ls_char as isize) as usize;
            let ne = (sel_end as isize - total_removed as isize).max(0) as usize;
            if primary_first { (ns, ne) } else { (ne, ns) }
        }
    };

    // Update egui's cursor so it follows the edit.
    if let Some(mut state) = TextEditState::load(ctx, te_id) {
        state.cursor.set_char_range(Some(CCursorRange {
            primary:   CCursor::new(new_primary),
            secondary: CCursor::new(new_secondary),
        }));
        state.store(ctx, te_id);
    }
    true
}

#[allow(clippy::too_many_arguments)]
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
    page_scroll:      f32,
    token_colors:     crate::markdown::editor_highlight::TokenColors,
    sync_scroll:      Option<f32>,
    zoom:             f32,
) -> (Option<(usize, usize)>, f32) {
    // Scale monospace font for zoom.
    if zoom != 1.0 {
        let base = ui.style().text_styles[&egui::TextStyle::Monospace].size;
        ui.style_mut().text_styles.insert(
            egui::TextStyle::Monospace,
            egui::FontId::new(base * zoom, egui::FontFamily::Monospace),
        );
    }
    let te_id = egui::Id::new(id).with("te");
    // Drain ALL Tab key-press events from the queue before any widget sees them.
    // consume_key() modifiers matching can be unreliable (Shift+Tab vs Tab), so
    // we walk the events directly and classify each one ourselves.
    let (tab_indent, tab_unindent) = ui.ctx().input_mut(|i| {
        let mut indent   = false;
        let mut unindent = false;
        i.events.retain(|e| {
            if let egui::Event::Key { key: Key::Tab, pressed: true, modifiers, .. } = e {
                if modifiers.shift { unindent = true; } else { indent = true; }
                false // remove from queue
            } else {
                true
            }
        });
        (indent, unindent)
    });

    let mut cursor_out: Option<(usize, usize)> = None;
    let mut sa = ScrollArea::vertical()
        .id_salt(id)
        .auto_shrink([false; 2]);
    if let Some(y) = sync_scroll { sa = sa.vertical_scroll_offset(y); }
    let scroll_out = sa.show(ui, |ui| {
            if page_scroll != 0.0 {
                let clip = ui.clip_rect();
                if page_scroll > 0.0 {
                    // PageUp: scroll up one page
                    ui.scroll_to_rect(
                        egui::Rect::from_x_y_ranges(clip.x_range(), clip.min.y - 1.0..=clip.min.y),
                        Some(egui::Align::BOTTOM),
                    );
                } else {
                    // PageDown: scroll down one page
                    ui.scroll_to_rect(
                        egui::Rect::from_x_y_ranges(clip.x_range(), clip.max.y..=clip.max.y + 1.0),
                        Some(egui::Align::TOP),
                    );
                }
            }
            let matches = search_matches.to_vec();
            let cur     = current_match;
            let qlen    = query_len;
            let tc      = token_colors; // Copy

            let mut layouter = move |ui: &egui::Ui, text: &str, wrap: f32| {
                use egui::text::LayoutJob;
                use crate::markdown::editor_highlight::syntax_spans;

                // 1. Syntax-colored base spans
                let base_spans = syntax_spans(text, tc, zoom);

                // 2. Merge search highlight overlays on top.
                //    Strategy: walk base_spans; split any span that overlaps a search
                //    match into up to three pieces (before, match, after), applying
                //    the highlight background to the match portion.
                let mut job = LayoutJob::default();

                // Build a flat list: for each search match → (start, end, bg_color)
                let highlight_ranges: Vec<(usize, usize, egui::Color32)> = if qlen == 0 {
                    vec![]
                } else {
                    matches.iter().enumerate().filter_map(|(mi, &ms)| {
                        let raw_end = ms + qlen;
                        if ms > text.len() || !text.is_char_boundary(ms) { return None; }
                        let end = {
                            let mut e = raw_end.min(text.len());
                            while e < text.len() && !text.is_char_boundary(e) { e += 1; }
                            e
                        };
                        if ms >= end { return None; }
                        let bg = if mi == cur {
                            egui::Color32::from_rgb(255, 150, 0)
                        } else {
                            egui::Color32::from_rgb(255, 220, 50)
                        };
                        Some((ms, end, bg))
                    }).collect()
                };

                for (span_start, span_end, base_fmt) in &base_spans {
                    let (ss, se) = (*span_start, *span_end);
                    if ss >= se || se > text.len() { continue; }

                    // Find all highlight ranges that overlap this span
                    let mut cursor = ss;
                    for &(hs, he, bg) in &highlight_ranges {
                        let overlap_s = hs.max(ss);
                        let overlap_e = he.min(se);
                        if overlap_s >= overlap_e { continue; }

                        // Emit the part of the span before the highlight
                        if cursor < overlap_s && text.is_char_boundary(cursor) && text.is_char_boundary(overlap_s) {
                            job.append(&text[cursor..overlap_s], 0.0, base_fmt.clone());
                        }
                        // Emit the highlighted portion
                        if text.is_char_boundary(overlap_s) && text.is_char_boundary(overlap_e) {
                            job.append(&text[overlap_s..overlap_e], 0.0, egui::text::TextFormat {
                                font_id:    base_fmt.font_id.clone(),
                                color:      egui::Color32::BLACK,
                                background: bg,
                                ..Default::default()
                            });
                        }
                        cursor = overlap_e;
                    }

                    // Emit the remainder of the span after all highlights
                    if cursor < se && text.is_char_boundary(cursor) && text.is_char_boundary(se) {
                        job.append(&text[cursor..se], 0.0, base_fmt.clone());
                    }
                }

                job.wrap.max_width = wrap;
                ui.fonts(|f| f.layout_job(job))
            };

            // Use TextEdit::show() (instead of ui.add) to access the galley for
            // accurate cursor-rect scrolling.
            let output = TextEdit::multiline(buffer)
                .id(te_id)
                .lock_focus(true)  // prevent Tab from surrendering keyboard focus
                .font(egui::TextStyle::Monospace)
                .desired_width(f32::INFINITY)
                .desired_rows(40)
                .layouter(&mut layouter)
                .show(ui);

            // Apply Tab indent / Shift+Tab unindent now that we have cursor info.
            // output.cursor_range is Some only when the TextEdit has keyboard focus.
            if tab_indent || tab_unindent {
                if let Some(cr) = output.cursor_range {
                    // Editor is focused: apply indent/unindent.
                    if editor_apply_indent(ui.ctx(), te_id, buffer, cr, tab_indent) {
                        *modified      = true;
                        *needs_reparse = true;
                    }
                } else {
                    // Editor not focused: give it focus (Tab was already consumed,
                    // so toolbar buttons won't be reached).
                    ui.ctx().memory_mut(|m| m.request_focus(te_id));
                }
            }

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
                ui.scroll_to_rect(screen_rect, Some(egui::Align::TOP));
            }

            if output.response.changed() {
                *modified      = true;
                *needs_reparse = true;
            }

            cursor_out = output.cursor_range.map(|r| (
                r.primary.rcursor.row + 1,
                r.primary.rcursor.column + 1,
            ));
        });
    (cursor_out, scroll_out.state.offset.y)
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
            Inline::Image(_, alt) => alt.as_str(),
            Inline::FootnoteRef(_) => "",
        }).collect::<Vec<_>>().join("")
    }

    match block {
        Block::Heading(_, ils) | Block::Paragraph(ils) | Block::BlockQuote(ils) => {
            inlines_text(ils)
        }
        Block::List(_, items) => items.iter().map(|i| inlines_text(&i.inlines)).collect::<Vec<_>>().join("\n"),
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
        Block::FootnoteDef(_, ils) => inlines_text(ils),
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

/// Find the byte offset in `buffer` of the heading at `block_idx`.
/// Scans the raw markdown line by line, matching `# … ` prefixes, and handles
/// duplicate headings by counting earlier blocks with the same level + text.
fn heading_byte_offset(buffer: &str, blocks: &[crate::markdown::Block], block_idx: usize) -> Option<usize> {
    use crate::markdown::{Block, Inline};

    let Block::Heading(level, inlines) = blocks.get(block_idx)? else { return None; };

    fn inline_text(ils: &[Inline]) -> String {
        ils.iter().map(|il| match il {
            Inline::Text(s) | Inline::Bold(s) | Inline::Italic(s)
            | Inline::BoldItalic(s) | Inline::Code(s) => s.as_str(),
            Inline::Link(t, _) => t.as_str(),
            Inline::Image(_, alt) => alt.as_str(),
            Inline::FootnoteRef(_) => "",
        }).collect()
    }

    let target = inline_text(inlines);
    let prefix = format!("{} ", "#".repeat(*level as usize));

    // How many earlier blocks share the same level + text?  (handles duplicates)
    let skip = blocks[..block_idx].iter().filter(|b| {
        matches!(b, Block::Heading(l, ils) if *l == *level && inline_text(ils) == target)
    }).count();

    let mut found    = 0usize;
    let mut byte_pos = 0usize;
    for raw_line in buffer.split('\n') {
        let line = raw_line.trim_end_matches('\r');
        if line.starts_with(&prefix) && line[prefix.len()..].trim() == target.trim() {
            if found == skip { return Some(byte_pos); }
            found += 1;
        }
        byte_pos += raw_line.len() + 1; // +1 for the '\n'
    }
    None
}

/// Returns `true` when `path` has a `.md` extension.
fn is_markdown(path: &std::path::Path) -> bool {
    path.extension().and_then(|e| e.to_str()) == Some("md")
}

/// Apply the active theme to egui's visuals.
fn apply_theme(ctx: &egui::Context, theme_id: ThemeId) {
    let theme = crate::theme::theme_by_id(theme_id);

    // Base visuals on content-area luminance — not theme name.
    let is_dark_content = luma(theme.bg) < 128.0;
    let mut visuals = if is_dark_content {
        egui::Visuals::dark()
    } else {
        egui::Visuals::light()
    };

    // ── Surface fills ─────────────────────────────────────────────────────────
    visuals.panel_fill          = theme.bg;
    visuals.window_fill         = theme.bg;
    visuals.override_text_color = Some(theme.fg);
    visuals.hyperlink_color     = theme.link;
    visuals.selection.bg_fill   = theme.selection_bg;
    visuals.faint_bg_color      = theme.code_bg;

    // Separator lines use the theme separator color
    visuals.widgets.noninteractive.bg_stroke =
        egui::Stroke::new(1.0, theme.separator);

    // ── Button backgrounds ────────────────────────────────────────────────────
    // Derived from toolbar_bg using perceived luminance.
    let is_dark_toolbar = luma(theme.toolbar_bg) < 128.0;
    let delta: i32 = if is_dark_toolbar { 28 } else { -22 };
    let btn_normal  = shift(theme.toolbar_bg, delta);
    let btn_hovered = shift(theme.toolbar_bg, delta + 22);
    let btn_active  = shift(theme.toolbar_bg, delta + 11);
    let btn_text    = if is_dark_toolbar { theme.sidebar_fg } else { theme.fg };

    let r4 = egui::CornerRadius::same(4);

    visuals.widgets.noninteractive.bg_fill       = theme.sidebar_bg;
    visuals.widgets.noninteractive.weak_bg_fill  = theme.sidebar_bg;
    visuals.widgets.noninteractive.fg_stroke     = egui::Stroke::new(1.0, btn_text);
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::same(3);

    visuals.widgets.inactive.bg_fill      = btn_normal;
    visuals.widgets.inactive.weak_bg_fill = btn_normal;
    visuals.widgets.inactive.fg_stroke    = egui::Stroke::new(1.0, btn_text);
    visuals.widgets.inactive.corner_radius = r4;

    visuals.widgets.hovered.bg_fill      = btn_hovered;
    visuals.widgets.hovered.weak_bg_fill = btn_hovered;
    visuals.widgets.hovered.fg_stroke    = egui::Stroke::new(1.0, btn_text);
    visuals.widgets.hovered.corner_radius = r4;
    visuals.widgets.hovered.expansion    = 1.0;

    visuals.widgets.active.bg_fill      = btn_active;
    visuals.widgets.active.weak_bg_fill = btn_active;
    visuals.widgets.active.fg_stroke    = egui::Stroke::new(1.0, btn_text);
    visuals.widgets.active.corner_radius = r4;

    visuals.widgets.open.bg_fill      = btn_hovered;
    visuals.widgets.open.weak_bg_fill = btn_hovered;
    visuals.widgets.open.fg_stroke    = egui::Stroke::new(1.0, btn_text);
    visuals.widgets.open.corner_radius = r4;

    // Rounded menus / popups
    visuals.menu_corner_radius   = egui::CornerRadius::same(6);
    visuals.window_corner_radius = egui::CornerRadius::same(8);

    // Subtle popup shadow
    visuals.popup_shadow = egui::Shadow {
        offset: [0, 4],
        blur:   16,
        spread: 0,
        color:  egui::Color32::from_black_alpha(55),
    };

    ctx.set_visuals(visuals);

    // ── Spacing / typography ──────────────────────────────────────────────────
    let mut style = (*ctx.style()).clone();
    style.spacing.item_spacing           = egui::vec2(8.0, 6.0);
    style.spacing.button_padding         = egui::vec2(14.0, 7.0); // more comfortable click targets
    style.spacing.menu_margin            = egui::Margin::same(8);  // a touch more breathing room
    style.spacing.indent                 = 18.0;
    style.spacing.scroll.bar_width       = 5.0;  // sleeker scroll bar
    style.spacing.scroll.bar_inner_margin = 3.0;

    if let Some(f) = style.text_styles.get_mut(&egui::TextStyle::Body)   { f.size = 15.0; }
    if let Some(f) = style.text_styles.get_mut(&egui::TextStyle::Small)  { f.size = 12.0; }
    if let Some(f) = style.text_styles.get_mut(&egui::TextStyle::Button) { f.size = 13.0; }

    ctx.set_style(style);
}

/// Perceived luminance 0–255.
#[inline]
fn luma(c: egui::Color32) -> f32 {
    0.299 * c.r() as f32 + 0.587 * c.g() as f32 + 0.114 * c.b() as f32
}

/// Shift all channels by `delta` (positive = lighter, negative = darker).
#[inline]
fn shift(c: egui::Color32, delta: i32) -> egui::Color32 {
    egui::Color32::from_rgb(
        (c.r() as i32 + delta).clamp(0, 255) as u8,
        (c.g() as i32 + delta).clamp(0, 255) as u8,
        (c.b() as i32 + delta).clamp(0, 255) as u8,
    )
}

/// Build editor token colors from the active theme.
fn make_token_colors(theme: &crate::theme::Theme) -> crate::markdown::editor_highlight::TokenColors {
    use crate::markdown::editor_highlight::TokenColors;
    use egui::Color32;

    let bg = theme.bg;
    let is_dark_bg = luma(bg) < 128.0;

    // Mix a color toward the background (for muted/dimmed variants).
    let mix = |c: Color32, factor: f32| -> Color32 {
        let f = factor.clamp(0.0, 1.0);
        Color32::from_rgb(
            (c.r() as f32 * (1.0 - f) + bg.r() as f32 * f) as u8,
            (c.g() as f32 * (1.0 - f) + bg.g() as f32 * f) as u8,
            (c.b() as f32 * (1.0 - f) + bg.b() as f32 * f) as u8,
        )
    };

    let bold_color = if is_dark_bg {
        Color32::from_rgb(255, 255, 255)
    } else {
        Color32::from_rgb(10, 10, 10)
    };

    // On light backgrounds, sidebar_active (accent) can have low contrast.
    // Blend it 70% toward fg (dark ink) for a readable but tinted heading color.
    let heading_color = if is_dark_bg {
        theme.sidebar_active
    } else {
        let fg = theme.fg;
        Color32::from_rgb(
            (theme.sidebar_active.r() as f32 * 0.3 + fg.r() as f32 * 0.7) as u8,
            (theme.sidebar_active.g() as f32 * 0.3 + fg.g() as f32 * 0.7) as u8,
            (theme.sidebar_active.b() as f32 * 0.3 + fg.b() as f32 * 0.7) as u8,
        )
    };

    TokenColors {
        normal:         theme.fg,
        heading:        heading_color,
        heading_marker: mix(heading_color, 0.5),
        bold:           bold_color,
        italic:         mix(theme.fg, 0.3),
        bold_italic:    bold_color,
        inline_code:    theme.inline_code_fg,
        code_block:     theme.fg_muted,
        fence_marker:   mix(theme.inline_code_fg, 0.25),
        link_text:      theme.link,
        link_url:       mix(theme.link, 0.35),
        list_marker:    theme.fg_muted,
        blockquote:     theme.fg_muted,
        hr:             theme.separator,
    }
}

/// Toggle the nth task list checkbox in `buffer` (0-indexed).
/// Matches lines of the form: optional whitespace + list marker + `[ ]` or `[x]`.
fn toggle_task_in_buffer(buffer: &mut String, index: usize) {
    let mut count   = 0usize;
    let mut byte_pos = 0usize;

    // Iterate over lines without allocating a Vec
    let mut rest = buffer.as_str();
    while !rest.is_empty() {
        let (line, remainder) = match rest.find('\n') {
            Some(i) => (&rest[..i], &rest[i + 1..]),
            None    => (rest, ""),
        };

        if let Some(cb_off) = task_checkbox_offset(line) {
            if count == index {
                let abs = byte_pos + cb_off + 1; // +1: skip '[', point at ' ' or 'x'
                let new_ch = match buffer.as_bytes().get(abs) {
                    Some(&b' ') => 'x',
                    _           => ' ',
                };
                buffer.replace_range(abs..abs + 1, &new_ch.to_string());
                return;
            }
            count += 1;
        }

        byte_pos += line.len() + 1;
        rest = remainder;
    }
}

/// Returns the byte offset of `[` within `line` if the line is a task list item, else `None`.
fn task_checkbox_offset(line: &str) -> Option<usize> {
    let rest = line.trim_start();
    let indent = line.len() - rest.len();

    // Unordered: `- `, `* `, `+ `
    let after_marker = if rest.starts_with("- ") || rest.starts_with("* ") || rest.starts_with("+ ") {
        &rest[2..]
    } else {
        // Ordered: one or more digits followed by `. ` or `) `
        let digits_end = rest.find(|c: char| !c.is_ascii_digit()).unwrap_or(0);
        if digits_end > 0 {
            let after = &rest[digits_end..];
            if after.starts_with(". ") || after.starts_with(") ") {
                &after[2..]
            } else {
                return None;
            }
        } else {
            return None;
        }
    };

    if after_marker.starts_with("[ ]") || after_marker.starts_with("[x]") || after_marker.starts_with("[X]") {
        Some(indent + (rest.len() - after_marker.len()))
    } else {
        None
    }
}
