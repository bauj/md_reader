# md_reader

[![crates.io](https://img.shields.io/crates/v/md_reader.svg)](https://crates.io/crates/md_reader)

A fast, native desktop markdown reader and editor built with [egui](https://github.com/emilk/egui) and the help of Claude. Open a folder and browse your notes with live preview, syntax-highlighted code blocks, and a persistent session that remembers where you left off.

---

## Features

- **Three view modes** — Preview, Edit, and Split (side-by-side)
- **Live preview** — rendered markdown updates as you type in Split/Edit mode
- **Syntax highlighting** — code blocks highlighted via [syntect](https://github.com/trishume/syntect)
- **File-tree sidebar** — browse folders, open multiple files as tabs
- **Document outline** — collapsible heading tree with auto-scroll tracking
- **Full-text search** — Ctrl+F with case-sensitive and whole-word options
- **Four themes** — Light (Manuscript), Coal, Navy, Ayu
- **Zoom** — Ctrl+Scroll to scale content independently of the UI
- **File watching** — auto-reloads files edited externally
- **Session persistence** — restores open tabs, view mode, theme, and zoom on restart
- **Unsaved-changes guard** — prompts before closing a modified file

---

## Preview

![md_reader Split mode screenshot](https://raw.githubusercontent.com/bauj/md_reader/main/assets/screenshots/md_reader_preview.png)

---

## Installation

```bash
cargo install md_reader
```

> **Requirements:** a C compiler and system libraries for your platform (standard Rust toolchain setup). On Linux, you may need `libxcb` / `libwayland` dev packages depending on your desktop environment.

---

## Usage

```bash
# Open the current directory
md_reader

# Open a specific folder
md_reader path/to/notes/

# Open a specific file (also loads its parent directory into the sidebar)
md_reader path/to/file.md
```

---

## Keyboard Shortcuts

| Shortcut | Action |
|---|---|
| `Ctrl+O` | Open folder |
| `Ctrl+N` | New file |
| `Ctrl+S` | Save current file |
| `Ctrl+W` | Close current tab |
| `Ctrl+Q` | Quit |
| `Ctrl+B` | Toggle sidebar |
| `Ctrl+F` | Open search bar |
| `Ctrl+PageUp` | Previous tab |
| `Ctrl+PageDown` | Next tab |
| `PageUp / PageDown` | Scroll preview |
| `Ctrl+Scroll` | Zoom content in/out |
| `Escape` | Close search bar |

---

## Build from Source

```bash
git clone https://github.com/bauj/md_reader
cd md_reader
cargo build --release
./target/release/md_reader
```

---

## License

MIT — see [LICENSE](LICENSE).

## Fonts

The following fonts are embedded in the binary under the [SIL Open Font License 1.1](https://openfontlicense.org):

- [JetBrains Mono](https://www.jetbrains.com/legalnotice/jetbrainsmono/) — used for code blocks and the editor
- [Noto Sans Bold](https://fonts.google.com/noto) — used for bold text in the markdown renderer

License texts are included in [`assets/fonts/licenses/`](assets/fonts/licenses/).
