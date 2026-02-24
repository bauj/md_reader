# Markdown Reader — Development Plan

## Overview

A Rust-based desktop markdown reader with GUI, built on `egui`/`eframe`. Features a
directory sidebar, rendered markdown preview, and an inline editor. Designed in
incremental phases so each phase produces a working, runnable application.

---

## Phase 1 — Project Skeleton + Sidebar + File Open

**Goal:** A window opens, shows a directory tree in a sidebar, and clicking a file
loads its raw content into a central panel as plain text.

### Tasks
- Set up `eframe`/`egui` with a basic window (`eframe::run_native`)
- Implement `FsTree` struct using `walkdir` to recursively scan a directory
  - Store nodes as a tree of `FsNode { name, path, kind: File|Dir, children }`
  - Dirs are collapsible (track expanded state in a `HashSet<PathBuf>`)
- Render the sidebar using `egui::SidePanel::left`
  - Show folder/file icons (unicode chars: 📁/📄 or ASCII alternatives)
  - Indent children based on depth
  - Highlight the currently selected file
- Add an "Open Folder" button in the toolbar that opens a native folder picker
  (`rfd` crate — Rusty File Dialogs)
  - Support `Ctrl+O` keyboard shortcut to open folder
- On file click: read the file contents into a `String` buffer (`std::fs::read_to_string`)
- Display the raw buffer in a scrollable `egui::ScrollArea` + `egui::Label` in the
  central panel (no rendering yet)
- CLI path argument:
  - Read `std::env::args().nth(1)` in `main()`
  - If the path points to a **directory**: load it as the root tree (same as Open Folder)
  - If the path points to a **file**: load the parent directory as the root tree and
    open the file directly in the viewer
  - If the path does not exist: print an error to stderr and start normally
  - Pass the resolved initial state to `App::new()` instead of using `App::default()`

### Crates introduced
- `eframe`, `egui`
- `walkdir`
- `rfd` (native file/folder dialogs)

### Acceptance criteria
- App window opens without crashing
- Can open a folder via the "Open Folder" button and see its tree in the sidebar
- Can open a folder via `Ctrl+O` keyboard shortcut
- Clicking a `.md` file shows its raw text on the right
- `md_reader ./docs` opens the `docs` folder in the sidebar on launch
- `md_reader ./docs/README.md` opens `docs` in the sidebar and loads `README.md`

---

## Phase 2 — Markdown Preview Renderer

**Goal:** The central panel renders markdown as formatted text — headings, paragraphs,
bold, italic, inline code, blockquotes, unordered/ordered lists, and tables.

### Tasks
- Add `pulldown-cmark` and parse the buffer into an event stream
- Define a `ParsedDoc` intermediate representation:
  - A `Vec<Block>` where `Block` is an enum: `Heading(level, inlines)`,
    `Paragraph(inlines)`, `CodeBlock(lang, code)`, `BlockQuote(blocks)`,
    `List(ordered, items)`, `Table(headers, rows)`, `Rule`, etc.
  - `Table` structure: headers are `Vec<String>`, rows are `Vec<Vec<String>>`
  - `Inline` enum: `Text(String)`, `Bold(inlines)`, `Italic(inlines)`,
    `Code(String)`, `Link(url, inlines)`, `Image(url, alt)`
- Implement `renderer.rs`: walk the `ParsedDoc` and emit egui draw calls
  - Headings: larger font sizes using `egui::RichText::size()`
  - Bold/Italic: `RichText::strong()` / `RichText::italics()`
  - Inline code: monospace font with a subtle background (`egui::Frame`)
  - Blockquotes: left border via a custom painter rect + indented text
  - Horizontal rules: `ui.separator()`
  - Lists: bullet points or numbered labels with indented content
  - Tables: render as grid using `ui.grid()` or `egui_extras::TableBuilder`
    - Header row with bold background color
    - Data rows with alternating row colors for readability
    - Cell padding and borders for clear separation
    - Right-align numeric columns (heuristic detection)
  - **Bug — inline code in table cells not rendered:** backtick-wrapped spans inside
    table cells (e.g. `` `foo` ``) are currently stored as raw strings in
    `Vec<Vec<String>>` during parsing, so the backticks appear literally in the
    rendered cell instead of being styled as inline code.
    Fix: change the table IR from `Vec<Vec<String>>` to `Vec<Vec<Vec<Inline>>>` so
    cells carry the same `Inline` enum as paragraphs, then call `render_inline` on each
    cell's inline list during rendering.
- Cache the `ParsedDoc` — only re-parse when the buffer changes (dirty flag)
- Default view mode is **Preview**

### Crates introduced
- `pulldown-cmark`

### Acceptance criteria
- Opening a markdown file shows rendered output (not raw text)
- Headings are visually distinct by level
- Bold, italic, and inline code are styled correctly
- Lists render with bullets/numbers
- Tables render with clear headers and readable rows
- Table cells display content correctly with proper alignment

---

## Phase 3 — Edit Mode

**Goal:** Switch to an edit mode that shows a raw text editor for the current file,
with the ability to save changes to disk.

### Tasks
- Add `ViewMode` enum: `Edit`, `Preview`, `Split`
- Add toggle buttons in the toolbar: "Edit" | "Preview" | "Split"
- In Edit mode: render `egui::TextEdit::multiline(&mut self.buffer)` filling the
  central panel
  - Enable monospace font for the editor
  - Track `modified: bool` — set to `true` whenever the buffer changes
  - Show a `●` indicator in the toolbar or title bar when there are unsaved changes
- Implement save:
  - Ctrl+S keyboard shortcut via `ctx.input(|i| i.key_pressed(Key::S) && i.modifiers.ctrl)`
  - "Save" button in toolbar (disabled when `!modified`)
  - Write buffer back to `current_file` with `std::fs::write`
  - Set `modified = false` after successful save
- Handle unsaved-changes guard: when switching files while `modified == true`,
  show an `egui::Modal` / inline dialog: "Unsaved changes — Save / Discard / Cancel"
- Close current file:
  - `Ctrl+W` keyboard shortcut closes the current file
  - If `modified == true`, show the unsaved-changes dialog before closing
  - On close: clear `buffer`, `current_file`, `parsed_doc`, reset `modified`
- New file on Edit with no file open:
  - When the user clicks "Edit" (or switches to Split) with no file open, open a
    native save dialog (`rfd::FileDialog::new().save_file()`) to pick a name and location
  - Create the file on disk immediately (empty content) via `std::fs::write`
  - Load it as the current file and switch to Edit mode
  - If the user cancels the dialog, stay in Preview mode

### Crates introduced
- None (all egui built-ins)

### Acceptance criteria
- Can edit a file and save it with Ctrl+S
- Unsaved changes indicator is visible
- Switching files with unsaved changes prompts the user
- `Ctrl+W` closes the current file (with unsaved-changes guard)
- Clicking Edit with no file open prompts to create a new file

---

## Phase 4 — Split View, Toolbar Polish & Keyboard Navigation

**Goal:** A side-by-side split view showing editor and preview simultaneously, plus a
polished toolbar, keyboard shortcuts, and an in-document navigation panel.

### Tasks
- Split view layout:
  - Use `egui::CentralPanel` split manually with two `egui::ScrollArea` columns
  - Or use `egui_extras` / manual `ui.columns(2, ...)` for a resizable divider
  - Keep scroll positions independent; optionally sync them (toggle option)
- **Bug fix — Split view independent scrolling:**
  - Currently both panes scroll together because `ui.columns()` shares the same
    parent `ScrollArea` context and egui routes scroll events to both
  - Fix: give each pane its own `ScrollArea` with a unique `id_source`, so egui
    tracks their scroll positions separately
  - Each `ScrollArea` should only respond to scroll events when the pointer is
    hovering over it (`ui.rect_contains_pointer(ui.min_rect())`)
- Toolbar refinements:
  - Show current file name (or "No file open")
  - Show modified indicator (`●`)
  - "Open Folder" | "New File" | view mode toggle buttons | "Save"
- Keyboard shortcuts:
  - `Ctrl+S` — save
  - `Ctrl+O` — open folder
  - `Ctrl+N` — new file (prompts for name in sidebar or dialog)
  - `Ctrl+W` — close current file *(implemented in Phase 3)*
  - `Ctrl+PageUp` / `Ctrl+PageDown` — cycle to previous/next file in the sidebar
  - `Ctrl+Q` — quit the app:
    - If `modified == true`, show the unsaved-changes dialog before exiting
    - On Save: save the file, then close the app
    - On Discard: close the app immediately
    - On Cancel: abort the quit
    - If no unsaved changes, exit immediately via `ctx.send_viewport_cmd(ViewportCommand::Close)`
- New file creation:
  - Prompt for filename inline in the sidebar (editable text field appears)
  - Create the file on disk and open it in the editor
- Window title: `md_reader — <filename> [●]`
- Document navigation panel (outline):
  - Shown as a collapsible bottom section of the left sidebar, below the file tree
  - Extract all H1/H2/H3 headings from `ParsedDoc` into a `Vec<(u32, String, usize)>`
    where the tuple is `(level, title, block_index)`
  - Render each heading as a clickable label, indented by level:
    - H1: no indent, full weight
    - H2: 12px indent, normal weight
    - H3: 24px indent, slightly dimmed color
  - Clicking a heading sets a `scroll_to_block: Option<usize>` on the app state
  - The preview `ScrollArea` checks `scroll_to_block` each frame and calls
    `ui.scroll_to_cursor()` to jump to the matching block, then clears the flag
  - Highlight the heading entry whose block is currently visible in the viewport
    (track topmost visible block index via `ui.clip_rect()` intersection)

- **Tab bar — open files kept in a top strip:**
  - Replace the single `current_file: Option<PathBuf>` with a `Vec<OpenTab>` structure:
    ```rust
    struct OpenTab {
        path: PathBuf,
        buffer: String,
        modified: bool,
        parsed_doc: Option<ParsedDoc>,
        needs_reparse: bool,
    }
    ```
    and an `active_tab: Option<usize>` index
  - **Opening a file:** double-click on a file in the sidebar opens (or focuses) its tab;
    single-click can still preview without creating a persistent tab
  - Render the tab strip as a `TopBottomPanel` between the main toolbar and the central
    panel; each tab shows:
    - The file's base name
    - A `●` indicator when `modified == true`
    - A small `×` close button on the right of the tab label
  - Clicking a tab makes it the active tab
  - Clicking `×` triggers the unsaved-changes guard before closing that tab
  - `Ctrl+PageUp` / `Ctrl+PageDown` now cycles through open tabs (left / right)
  - `Ctrl+W` closes the active tab
  - `Ctrl+S` saves the active tab's buffer
  - All per-file state (`buffer`, `modified`, `parsed_doc`, `needs_reparse`) moves
    into `OpenTab`; `App` reads/writes only the active tab

### Crates introduced
- None

### Acceptance criteria
- Split view works with editor and live preview side-by-side
- All listed keyboard shortcuts function correctly
- New file can be created from within the app
- Navigation panel lists all H1/H2/H3 headings of the open document
- Clicking a heading scrolls the preview to that section
- H2 and H3 entries are visually indented relative to H1
- Double-clicking a file in the sidebar opens it in a new tab (or focuses its existing tab)
- Open tabs are visible in a strip below the toolbar with filename and modified indicator
- Clicking `×` on a tab closes it (with unsaved-changes guard)
- `Ctrl+PageUp` / `Ctrl+PageDown` cycles between open tabs

---

## Phase 5 — Syntax Highlighting in Code Blocks

**Goal:** Fenced code blocks with a language tag are rendered with full syntax
highlighting.

### Tasks
- Integrate `syntect` for syntax highlighting
  - Load bundled syntax definitions (`SyntaxSet::load_defaults_newlines()`)
  - Load a theme (`ThemeSet::load_defaults()`, default to "base16-ocean.dark" or
    similar)
  - Allow theme selection in a settings panel (dropdown of available themes)
- In `renderer.rs`, when rendering a `CodeBlock(lang, code)`:
  - Look up the syntax by language tag via `syntax_set.find_syntax_by_token(&lang)`
  - Run `syntect::easy::HighlightLines` over each line
  - Map highlighted spans to `egui::RichText` with the correct `Color32`
  - Render inside a `egui::Frame` with a dark background and monospace font
- Fallback: if language is unknown or empty, render as plain monospace in a frame
- Performance: cache highlighted output per code block (keyed by content hash)
  to avoid re-highlighting every frame

### Crates introduced
- `syntect`

### Acceptance criteria
- ` ```rust ` blocks render with Rust syntax colors
- ` ```python `, ` ```js `, etc. also highlight correctly
- Plain ` ``` ` blocks (no language) render as monospace without error

---

## Phase 6 — File Watcher, State Persistence & UX Polish

**Goal:** The app detects external file changes and reloads automatically. App state
(last opened folder, window size) persists across sessions.

### Tasks

#### File Watcher
- Integrate `notify` crate to watch the current open file and root directory
- Run the watcher on a background thread; send events via `std::sync::mpsc::channel`
- Poll the channel each frame in `App::update()`
- On file-modified event for the currently open file:
  - If `modified == false`: auto-reload silently
  - If `modified == true`: show a notification bar "File changed externally — Reload / Keep mine"
- On directory events (new file, renamed, deleted): re-scan `FsTree` and refresh sidebar

#### State Persistence
- Use `serde` + `serde_json` to serialize `AppState`:
  ```
  { last_root_dir, last_open_file, window_width, window_height, sidebar_width, view_mode, recent_files }
  ```
  - `recent_files`: `Vec<(PathBuf, DateTime)>` — list of up to 20 most recently opened files with timestamps
- Store at a platform-appropriate config path:
  - Linux: `~/.config/md_reader/state.json`
  - Use `dirs` crate for cross-platform config dir resolution
- Load state on startup, save on clean exit (`eframe::App::on_exit`)

#### Recent Files Menu
- Add a "Recent Files" button/menu in the toolbar that shows a dropdown list of 10-20 most recently opened files
- Clicking a recent file opens it directly (no need to browse the folder tree)
- Keep track of file access time — update `recent_files` whenever a file is opened
- Show file path in the dropdown (with parent folder name for disambiguation)
- Add "Clear Recent Files" option in the dropdown
- Recent files list persists across sessions via state persistence

#### Bug fix — lazy / async folder loading
- **Problem:** opening a large folder (many nested sub-folders and files) blocks the UI
  thread entirely during `FsTree::scan_dir()`, causing a visible freeze on startup or
  when switching root directories.
- **Root cause:** `scan_dir` is a synchronous recursive `std::fs::read_dir` walk called
  directly in `App::new()` / the Ctrl+O handler, on the main thread.
- **Fix — lazy expansion:** only scan a directory's children when the user expands it
  in the sidebar (toggle arrow clicked). On first open, load only the root level:
  - `FsTree::new(path)` scans **one level deep** only (no recursion)
  - Each `FsNode::Dir` starts with `children: None` (unexpanded) instead of a
    pre-populated `Vec`
  - When the user clicks a folder's expand arrow, `FsTree::expand(path)` scans that
    directory one level deep and populates its children
  - Already-expanded nodes are not re-scanned unless explicitly refreshed
- **Alternative / complement — background thread:** run `scan_dir` on a
  `std::thread::spawn` thread and send the result back via `mpsc::channel`; show a
  spinner or "Loading…" label in the sidebar while the scan is in progress
- **Acceptance criteria:**
  - Opening any folder (even one with thousands of sub-directories) returns
    immediately with the root level visible
  - Sub-directories expand on demand with no perceptible delay for typical depths
  - The file watcher and `FsTree` rescan logic are updated to be consistent with
    lazy loading

#### UX Polish
- **Tab key focus navigation in dialogs:** when a modal dialog is open (e.g. "Unsaved
  changes — Save / Discard / Cancel"), pressing `Tab` should cycle focus through the
  dialog's buttons only. Pressing `Enter` on the focused button activates it.
  - Constrain `Tab` navigation to the dialog's widget scope (e.g. set `ui.scope` or
    track focused widget IDs manually and cycle them)
  - Prevents Tab from accidentally moving focus to background widgets while the dialog
    is blocking interaction
- Smooth scrolling in preview (egui ScrollArea handles this natively)
- Clickable links in preview: `egui::Response::clicked()` → `open::that(url)`
- Images in markdown: render inline with `egui_extras::RetainedImage` (load from
  relative path or URL)
- **Tooltips:** add `.on_hover_text()` to interactive elements throughout the UI:
  - Tab bar: hovering a tab shows the full absolute path of the file
  - Toolbar buttons: short description of each action (e.g. "Open a folder in the sidebar")
  - Sidebar file entries: full path on hover
  - Outline headings: full heading text on hover (useful when truncated)
#### Search (Ctrl+F)
- Toggle a search bar with `Ctrl+F`; pressing `Escape` or `Ctrl+F` again dismisses it
- Search bar appears as a floating overlay in the top-right corner of the central panel
- Search state on `App`:
  ```
  search_query: String,
  search_open: bool,
  search_matches: Vec<usize>,   // byte offsets into buffer
  search_current: usize,        // index into search_matches
  ```
- Matching: case-insensitive substring search over `self.buffer` on every keystroke
  - Collect all byte offsets of matches into `search_matches`
  - Show match count: "3 / 12" (current / total)
- **Search options — two toggle buttons in the search bar:**
  - **Match Case** (`Alt+C` when search bar focused): disables the `.to_lowercase()` normalization;
    compare raw bytes instead. Button visually depressed when active.
    Add `search_case_sensitive: bool` field to `App`.
  - **Match Whole Word** (`Alt+W` when search bar focused): wraps each match candidate with a
    word-boundary check — the character immediately before `match_start` and after `match_end`
    (if they exist) must not be alphanumeric/underscore.
    Add `search_whole_word: bool` field to `App`.
  - Both flags feed into `update_search_matches()` and the `find_matches()` helper in the
    renderer; re-run matching on every toggle.
  - Keyboard shortcuts fire only when the search bar widget is focused
    (same focus-gate pattern as `Enter`/`Shift+Enter`).
- Navigation: `Enter` / `Shift+Enter` or ▲▼ buttons cycle through matches
  - Scroll the preview or editor to bring the current match into view
- Highlighting in **Preview mode**: re-render paragraphs that contain a match,
  wrapping the matched substring in a highlighted `RichText` span (yellow background)
- Highlighting in **Edit mode**: use `TextEdit`'s layouter callback to apply a
  background color to matched ranges via `egui::text::LayoutJob`
- "No results" shown in muted text when `search_matches` is empty
- Status bar at the bottom: line count, word count, cursor position (edit mode)
- Drag-and-drop: accept a dropped `.md` file or folder via `ctx.input(|i| &i.raw.dropped_files)`

### Crates introduced
- `notify`
- `serde`, `serde_json`
- `dirs`
- `open`
- `egui_extras`

### Acceptance criteria
- Externally modified files are detected and the user is prompted to reload
- App reopens the last used folder and file on startup
- Recent Files menu shows the 10-20 most recently opened files
- Clicking a recent file opens it immediately
- Recent files list persists across app restarts
- Can clear recent files history
- Clicking a link opens it in the default browser
- Word/line count shown in status bar
- `Ctrl+F` opens a search bar; matches are highlighted and navigable with Enter/▲▼
- Search bar dismisses with `Escape`

---

## Phase 7 — UI Theming & Visual Polish

**Goal:** Replace egui's default grey aesthetic with a carefully chosen color system.
Offer a set of named themes (light and dark) selectable at runtime, stored in persisted
state. Each theme defines consistent colors for every surface: sidebar, toolbar, editor,
preview, and code blocks.

### Research basis

All hex values below were sourced from production reading apps and validated against
WCAG contrast requirements. The guiding principle: avoid pure `#000000`/`#ffffff` pairs
— they maximise contrast mathematically (21:1) but cause visual vibration on screens.
A range of 10:1–14:1 is optimal for long-form reading.

### Theme definitions

Each theme is a `Theme` struct with named color fields:

```rust
pub struct Theme {
    pub name: &'static str,

    // Surfaces
    pub bg:              Color32,   // central panel / preview background
    pub sidebar_bg:      Color32,   // sidebar + outline panel background
    pub toolbar_bg:      Color32,   // top toolbar background
    pub tab_bar_bg:      Color32,   // tab strip background

    // Text
    pub fg:              Color32,   // body text
    pub fg_muted:        Color32,   // muted / secondary text (H3, labels)
    pub sidebar_fg:      Color32,   // sidebar file/folder labels
    pub sidebar_active:  Color32,   // currently open file highlight

    // Interactive
    pub link:            Color32,   // hyperlinks
    pub selection_bg:    Color32,   // text selection background

    // Code blocks
    pub code_bg:         Color32,   // fenced code block frame background
    pub inline_code_fg:  Color32,   // `inline code` text color

    // Structural
    pub separator:       Color32,   // dividers, rule (---) lines
    pub quote_bg:        Color32,   // blockquote left-bar tint
}
```

### Built-in themes (5 total, mirroring mdBook)

#### Light — clean white, high contrast
Inspired by GitHub Markdown light + mdBook Light.
| Field | Hex | Notes |
|---|---|---|
| `bg` | `#ffffff` | pure white content area |
| `sidebar_bg` | `#fafafa` | barely-off-white sidebar |
| `toolbar_bg` | `#f0f0f0` | light grey toolbar |
| `tab_bar_bg` | `#e8e8e8` | slightly darker tab strip |
| `fg` | `#1f2328` | GitHub's near-black body text (contrast 16:1) |
| `fg_muted` | `#57606a` | muted labels |
| `sidebar_fg` | `#24292f` | sidebar filenames |
| `sidebar_active`| `#0969da` | GitHub blue active link |
| `link` | `#0969da` | |
| `selection_bg` | `#b3d4fc` | |
| `code_bg` | `#f6f8fa` | GitHub code background |
| `inline_code_fg`| `#d63384` | |
| `separator` | `#d0d7de` | |
| `quote_bg` | `#f0f8fc` | very light blue tint |

#### Rust — warm parchment, dark sidebar
Direct port of mdBook's "Rust" theme.
| Field | Hex |
|---|---|
| `bg` | `#ddddd2` |
| `sidebar_bg` | `#3b2e2a` |
| `toolbar_bg` | `#332926` |
| `tab_bar_bg` | `#2e2420` |
| `fg` | `#262625` |
| `fg_muted` | `#6e6b5e` |
| `sidebar_fg` | `#c8c9db` |
| `sidebar_active`| `#e69f67` |
| `link` | `#2b79a2` |
| `selection_bg` | `#c9a87c` |
| `code_bg` | `#c9c9bc` |
| `inline_code_fg`| `#6e6b5e` |
| `separator` | `#b0afa4` |
| `quote_bg` | `#d4d4c8` |

#### Coal — near-black dark mode
Port of mdBook "Coal".
| Field | Hex |
|---|---|
| `bg` | `#131516` |
| `sidebar_bg` | `#292c2f` |
| `toolbar_bg` | `#22252a` |
| `tab_bar_bg` | `#1e2124` |
| `fg` | `#98a3ad` |
| `fg_muted` | `#6b7680` |
| `sidebar_fg` | `#a1adb8` |
| `sidebar_active`| `#3473ad` |
| `link` | `#2b79a2` |
| `selection_bg` | `#2d4f6e` |
| `code_bg` | `#1e2124` |
| `inline_code_fg`| `#c5c8c6` |
| `separator` | `#3a3f44` |
| `quote_bg` | `#1c2024` |

#### Navy — blue-tinted dark mode
Port of mdBook "Navy".
| Field | Hex |
|---|---|
| `bg` | `#161b2c` |
| `sidebar_bg` | `#282d3f` |
| `toolbar_bg` | `#1e2235` |
| `tab_bar_bg` | `#1a1e2e` |
| `fg` | `#bcbdd0` |
| `fg_muted` | `#7c7e94` |
| `sidebar_fg` | `#c8c9db` |
| `sidebar_active`| `#2b79a2` |
| `link` | `#2b79a2` |
| `selection_bg` | `#2b4a6e` |
| `code_bg` | `#1e2235` |
| `inline_code_fg`| `#c5c8c6` |
| `separator` | `#353a52` |
| `quote_bg` | `#1e2338` |

#### Ayu — minimal near-black with warm accent
Port of mdBook "Ayu".
| Field | Hex |
|---|---|
| `bg` | `#0f1419` |
| `sidebar_bg` | `#14191f` |
| `toolbar_bg` | `#111519` |
| `tab_bar_bg` | `#0d1015` |
| `fg` | `#c5c5c5` |
| `fg_muted` | `#5c6773` |
| `sidebar_fg` | `#c8c9db` |
| `sidebar_active`| `#ffb454` |
| `link` | `#0096cf` |
| `selection_bg` | `#273747` |
| `code_bg` | `#191f26` |
| `inline_code_fg`| `#ffb454` |
| `separator` | `#1f2732` |
| `quote_bg` | `#141a22` |

### Typography

- **Preview body font:** system sans-serif via `egui::FontFamily::Proportional`
  - Body size: **16 px** (egui points ≈ CSS px at 96 dpi)
  - H1: 28 px bold · H2: 22 px bold · H3: 18 px semi-bold
  - Paragraph line-height: set via `egui::Style::spacing.item_spacing.y`
  - Target reading width: **700 px** — add `ui.set_max_width(700.0)` centered in the
    central panel
- **Editor font:** `egui::FontFamily::Monospace`, 14 px
- **Sidebar font:** 13 px; active file: same weight as body

### Implementation

- Add `src/theme/mod.rs` exporting `Theme`, `ThemeId` (enum), and `THEMES: &[Theme]`
- Store `active_theme: ThemeId` in persisted state (Phase 6)
- In `App::update()`, call `apply_theme(ctx, &theme)` once per frame:
  ```rust
  fn apply_theme(ctx: &egui::Context, theme: &Theme) {
      let mut visuals = egui::Visuals::dark(); // or light() for light themes
      visuals.panel_fill           = theme.bg;
      visuals.window_fill          = theme.bg;
      visuals.override_text_color  = Some(theme.fg);
      visuals.hyperlink_color      = theme.link;
      visuals.selection.bg_fill    = theme.selection_bg;
      visuals.widgets.noninteractive.bg_fill = theme.sidebar_bg;
      ctx.set_visuals(visuals);
  }
  ```
- Sidebar, toolbar, and tab bar panels use `egui::Frame::none().fill(theme.toolbar_bg)`
  (etc.) via `.frame(...)` on the panel builder
- Renderer reads `theme.code_bg`, `theme.inline_code_fg`, `theme.fg_muted`,
  `theme.quote_bg` directly for code block frames and blockquote styling
- Theme picker: a small dropdown button in the toolbar (paint-brush icon) showing the
  five theme names; selecting one updates `App::active_theme` immediately

### Crates introduced
- None (all egui built-ins + `Color32` constants)

### Acceptance criteria
- All 5 themes apply correctly with no hard-coded colors remaining in renderer or UI code
- Sidebar, toolbar, tab strip, and central panel each use the theme's appropriate surface color
- Body text contrast ≥ 7:1 on all themes (AAA) — verified manually
- Preview body text renders at 16 px with H1/H2/H3 at 28/22/18 px
- Preview content width is capped at ~700 px and centered
- Theme selection persists across restarts
- Switching themes updates the UI immediately without restart

---

## Crate Summary

| Crate | Version | Purpose |
|---|---|---|
| `eframe` | 0.31 | App shell, windowing, event loop |
| `egui` | 0.31 | Immediate mode UI widgets |
| `egui_extras` | 0.31 | Image support, extended widgets |
| `walkdir` | 2 | Recursive directory traversal |
| `rfd` | 0.15 | Native file/folder open dialogs |
| `pulldown-cmark` | 0.12 | CommonMark markdown parser |
| `syntect` | 5 | Syntax highlighting for code blocks |
| `notify` | 8 | Filesystem change watcher |
| `serde` | 1 | Serialization framework |
| `serde_json` | 1 | JSON state persistence |
| `dirs` | 6 | Platform config directory paths |
| `open` | 5 | Open URLs/files in default app |
| `chrono` | 0.4 | DateTime tracking for recent files (optional) |

---

## File / Module Map

```
src/
├── main.rs              # eframe::run_native, load persisted state
├── app.rs               # App struct, implements eframe::App, drives update()
│
├── ui/
│   ├── mod.rs
│   ├── toolbar.rs       # Top bar: buttons, file name, modified indicator
│   ├── sidebar.rs       # FsTree rendering, file selection
│   ├── editor.rs        # TextEdit multiline, cursor tracking
│   ├── preview.rs       # Calls renderer, scroll area wrapper
│   └── statusbar.rs     # Bottom bar: word count, line count, cursor pos
│
├── fs/
│   ├── mod.rs
│   ├── tree.rs          # FsNode, FsTree, walkdir scan, expand/collapse state
│   └── watcher.rs       # notify watcher, mpsc channel integration
│
├── markdown/
│   ├── mod.rs
│   ├── parser.rs        # pulldown-cmark → ParsedDoc (Block/Inline IR)
│   ├── renderer.rs      # ParsedDoc → egui draw calls
│   └── highlight.rs     # syntect integration, highlight cache
│
└── state/
    ├── mod.rs
    └── persist.rs       # AppState serde struct, load/save to disk, recent files management
```
