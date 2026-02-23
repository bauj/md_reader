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
- On file click: read the file contents into a `String` buffer (`std::fs::read_to_string`)
- Display the raw buffer in a scrollable `egui::ScrollArea` + `egui::Label` in the
  central panel (no rendering yet)

### Crates introduced
- `eframe`, `egui`
- `walkdir`
- `rfd` (native file/folder dialogs)

### Acceptance criteria
- App window opens without crashing
- Can open a folder and see its tree in the sidebar
- Clicking a `.md` file shows its raw text on the right

---

## Phase 2 — Markdown Preview Renderer

**Goal:** The central panel renders markdown as formatted text — headings, paragraphs,
bold, italic, inline code, blockquotes, and unordered/ordered lists.

### Tasks
- Add `pulldown-cmark` and parse the buffer into an event stream
- Define a `ParsedDoc` intermediate representation:
  - A `Vec<Block>` where `Block` is an enum: `Heading(level, inlines)`,
    `Paragraph(inlines)`, `CodeBlock(lang, code)`, `BlockQuote(blocks)`,
    `List(ordered, items)`, `Rule`, etc.
  - `Inline` enum: `Text(String)`, `Bold(inlines)`, `Italic(inlines)`,
    `Code(String)`, `Link(url, inlines)`, `Image(url, alt)`
- Implement `renderer.rs`: walk the `ParsedDoc` and emit egui draw calls
  - Headings: larger font sizes using `egui::RichText::size()`
  - Bold/Italic: `RichText::strong()` / `RichText::italics()`
  - Inline code: monospace font with a subtle background (`egui::Frame`)
  - Blockquotes: left border via a custom painter rect + indented text
  - Horizontal rules: `ui.separator()`
  - Lists: bullet points or numbered labels with indented content
- Cache the `ParsedDoc` — only re-parse when the buffer changes (dirty flag)
- Default view mode is **Preview**

### Crates introduced
- `pulldown-cmark`

### Acceptance criteria
- Opening a markdown file shows rendered output (not raw text)
- Headings are visually distinct by level
- Bold, italic, and inline code are styled correctly
- Lists render with bullets/numbers

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

### Crates introduced
- None (all egui built-ins)

### Acceptance criteria
- Can edit a file and save it with Ctrl+S
- Unsaved changes indicator is visible
- Switching files with unsaved changes prompts the user

---

## Phase 4 — Split View, Toolbar Polish & Keyboard Navigation

**Goal:** A side-by-side split view showing editor and preview simultaneously, plus a
polished toolbar and keyboard shortcuts for common actions.

### Tasks
- Split view layout:
  - Use `egui::CentralPanel` split manually with two `egui::ScrollArea` columns
  - Or use `egui_extras` / manual `ui.columns(2, ...)` for a resizable divider
  - Keep scroll positions independent; optionally sync them (toggle option)
- Toolbar refinements:
  - Show current file name (or "No file open")
  - Show modified indicator (`●`)
  - "Open Folder" | "New File" | view mode toggle buttons | "Save"
- Keyboard shortcuts:
  - `Ctrl+S` — save
  - `Ctrl+O` — open folder
  - `Ctrl+N` — new file (prompts for name in sidebar or dialog)
  - `Ctrl+W` — close current file
  - `Ctrl+[` / `Ctrl+]` — cycle sidebar selection up/down (keyboard navigation)
- New file creation:
  - Prompt for filename inline in the sidebar (editable text field appears)
  - Create the file on disk and open it in the editor
- Window title: `md_reader — <filename> [●]`

### Crates introduced
- None

### Acceptance criteria
- Split view works with editor and live preview side-by-side
- All listed keyboard shortcuts function correctly
- New file can be created from within the app

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

#### UX Polish
- Smooth scrolling in preview (egui ScrollArea handles this natively)
- Clickable links in preview: `egui::Response::clicked()` → `open::that(url)`
- Images in markdown: render inline with `egui_extras::RetainedImage` (load from
  relative path or URL)
- Search bar (Ctrl+F): highlight matching text in both editor and preview
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
