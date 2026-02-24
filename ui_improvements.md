# UI/UX Improvements for Markdown Reader

This document outlines comprehensive UI/UX improvements for the markdown reader, focusing on fonts, spacing, layout, and overall usability.

---

## 1. Fonts and Typography

### Preview Area
- **Body Text**: Use a serif font (e.g., `Lora` or `Georgia`) for better readability in long-form content.
- **Headings**: Use a sans-serif font (e.g., `Inter` or `Roboto`) for better contrast and hierarchy.
- **Font Sizes**:
  - Body: `16px` (keep current size, adjust line height to `1.6`).
  - H1: `32px` (bolder).
  - H2: `24px`.
  - H3: `20px`.
  - H4-H6: Introduce smaller sizes (`16px`, `14px`) for better hierarchy.

### Edit Mode
- **Font**: Use a monospace font (e.g., `JetBrains Mono` or `Fira Code`) for code and edit mode.
- **Ligatures**: Enable ligatures for code (e.g., `=>`, `!=`) to improve readability.

### Sidebar
- **Font**: Use a sans-serif font (e.g., `Inter`) for file/folder names.
- **Font Size**: Increase from `13px` to `14px` for better readability.

---

## 2. Spacing and Layout

### Preview Area
- **Max-Width**: Increase from `700px` to `800px` for better use of screen space.
- **Padding**: Add `20px` padding on all sides to prevent text from touching the edges.
- **Line Height**: Increase to `1.6` for body text to reduce eye strain.

### Sidebar
- **Width**: Reduce default width from `250px` to `220px` to give more space to the preview.
- **Collapsible**: Allow users to collapse the sidebar completely (toggle button in the toolbar).
- **Indentation**: Increase indentation for nested folders/files from `12px` to `16px`.

### Toolbar
- **Height**: Reduce from `40px` to `36px` to save vertical space.
- **Button Spacing**: Add `8px` padding between buttons for better clickability.

---

## 3. Markdown Preview Improvements

### Code Blocks
- **Syntax Highlighting**: Ensure consistency across themes.
- **Copy Button**: Add a copy button (top-right corner) for easy copying.
- **Background**: Use a slightly darker shade than the theme’s `code_bg` for better contrast.

### Tables
- **Zebra Striping**: Add alternating row colors for better readability.
- **Header Styling**: Bold headers with a subtle background color.
- **Borders**: Use thin borders (`1px`) with rounded corners.

### Lists
- **Bullet Points**: Use custom icons (e.g., `•` for unordered, `1.` for ordered) with better alignment.
- **Indentation**: Increase left padding for nested lists from `20px` to `24px`.

### Blockquotes
- **Left Border**: Add a `4px` solid border using `theme.quote_bg` for visual emphasis.
- **Italicize**: Italicize blockquote text for better differentiation.

### Links
- **Hover Effect**: Underline links on hover (current: only color change).
- **Icon**: Add a small icon (e.g., `🔗`) next to external links.

---

## 4. Edit Mode Enhancements

### Line Numbers
- Add line numbers in the left gutter (optional toggle in settings).
- Highlight the current line with a subtle background color.

### Syntax Highlighting
- Extend highlighting to markdown syntax (e.g., `#` for headings, `**` for bold).
- Use the active theme’s colors for consistency.

### Word Wrap
- Add a toggle for word wrap (default: on) to handle long lines.

### Minimap
- Add a minimap (right-side scrollbar) for quick navigation in large files.

---

## 5. Sidebar Improvements

### File Icons
- Replace Unicode icons (`📁`, `📄`) with SVG icons (e.g., from `egui_extras::RetainedImage`).
- Use different icons for `.md`, `.txt`, and other file types.

### Active File Highlighting
- Use a bold font + background color for the active file.
- Add a small checkmark icon (e.g., `✓`) next to the active file.

### Folder Structure
- Allow drag-and-drop to reorder files/folders (if supported by `walkdir`).
- Add a context menu (right-click) for actions like "Rename," "Delete," or "Copy Path."

---

## 6. Toolbar and Navigation

### Toolbar Buttons
- Add icons (e.g., `📁` for "Open Folder," `💾` for "Save") alongside text labels.
- Group related buttons (e.g., "Edit," "Preview," "Split" together).

### Search Bar
- Move the search bar to the top-right corner (floating) for better visibility.
- Add keyboard shortcuts (e.g., `Ctrl+F` to open, `Esc` to close).

### Recent Files
- Add a dropdown menu for recent files.
- Show file icons and timestamps in the dropdown.

---

## 7. Theming and Visual Polish

### Theme Switcher
- Add a dropdown menu in the toolbar to switch themes.
- Show a live preview of the theme before applying.

### Custom Themes
- Allow users to create custom themes (e.g., via a JSON config file).

### Dark/Light Mode Toggle
- Add a quick toggle (e.g., `🌙`/`☀️` icon) to switch between dark and light themes.

---

## 8. Accessibility

### Keyboard Shortcuts
- Add shortcuts for all actions (e.g., `Ctrl+Shift+F` for search, `Ctrl+]`/`Ctrl+[` for indentation).
- Show a cheat sheet (modal dialog) with all shortcuts.

### High Contrast Mode
- Add a high-contrast theme for users with visual impairments.

### Font Scaling
- Allow users to increase/decrease font size (e.g., `Ctrl++`/`Ctrl+-`).

---

## 9. Performance

### Lazy Loading
- Implement lazy loading for large files (e.g., load only the visible portion of the file).
- Add a loading spinner for large files.

### Caching
- Cache parsed markdown and syntax-highlighted code to reduce re-rendering.

---

## 10. Miscellaneous

### Status Bar
- Show word count, line count, and cursor position.
- Add a file encoding indicator (e.g., `UTF-8`).

### Drag-and-Drop
- Support dragging files/folders into the app to open them.

### Auto-Save
- Add an auto-save toggle (e.g., save every 5 minutes).

---

## Summary of Changes

| Area | Key Improvements |
|------|-----------------|
| **Fonts** | Serif for preview, monospace for edit, better hierarchy. |
| **Spacing** | Wider preview, better line height, reduced sidebar width. |
| **Preview** | Better code blocks, tables, lists, and blockquotes. |
| **Edit Mode** | Line numbers, word wrap, minimap, syntax highlighting. |
| **Sidebar** | Icons, active file highlighting, drag-and-drop. |
| **Toolbar** | Icons, search bar, recent files dropdown. |
| **Theming** | Live preview, custom themes, dark/light toggle. |
| **Accessibility** | Keyboard shortcuts, high contrast, font scaling. |
| **Performance** | Lazy loading, caching. |

---

## Next Steps
Prioritize the changes based on user feedback and implement them incrementally. Start with high-impact improvements like fonts, spacing, and preview enhancements, followed by edit mode and sidebar improvements.