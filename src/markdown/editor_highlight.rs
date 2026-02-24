use egui::{Color32, FontId, text::TextFormat};

/// Colors used to paint markdown tokens in the editor.
#[derive(Clone, Copy)]
pub struct TokenColors {
    pub normal:         Color32,
    pub heading:        Color32, // heading text
    pub heading_marker: Color32, // # symbols
    pub bold:           Color32,
    pub italic:         Color32,
    pub bold_italic:    Color32,
    pub inline_code:    Color32,
    pub code_block:     Color32, // inside a fenced code block
    pub fence_marker:   Color32, // ``` / ~~~ lines
    pub link_text:      Color32, // [text] part
    pub link_url:       Color32, // (url) part
    pub list_marker:    Color32, // - / * / 1. at line start
    pub blockquote:     Color32, // > marker
    pub hr:             Color32, // --- / *** horizontal rule
}

/// Returns a flat list of `(byte_start, byte_end, TextFormat)` spans covering
/// every byte of `text`.  The spans are non-overlapping and contiguous.
pub fn syntax_spans(text: &str, colors: TokenColors) -> Vec<(usize, usize, TextFormat)> {
    let mono = FontId::monospace(13.0);

    let fmt = |color: Color32| TextFormat {
        font_id: mono.clone(),
        color,
        ..Default::default()
    };

    let mut spans: Vec<(usize, usize, TextFormat)> = Vec::new();
    let mut in_code_fence = false;
    let mut pos = 0usize; // byte cursor in `text`

    // Iterate lines, keeping the trailing '\n' so byte offsets stay correct.
    for raw_line in text.split_inclusive('\n') {
        let line_start = pos;
        let line_end   = pos + raw_line.len();
        let line       = raw_line.trim_end_matches('\n').trim_end_matches('\r');
        let trimmed    = line.trim_start();

        // ── Code fence delimiter ─────────────────────────────────────────
        if trimmed.starts_with("```") || trimmed.starts_with("~~~") {
            let was_open = in_code_fence;
            in_code_fence = !in_code_fence;
            // Color the fence line itself differently based on open/close.
            if was_open {
                // closing fence
                spans.push((line_start, line_end, fmt(colors.fence_marker)));
            } else {
                // opening fence (may have a language tag after ```)
                let fence_len = if trimmed.starts_with("```") { 3 } else { 3 };
                let fence_abs = line_start + (line.len() - trimmed.len()); // indent offset
                let tag_start = fence_abs + fence_len;
                spans.push((line_start, tag_start, fmt(colors.fence_marker)));
                if tag_start < line_end {
                    // language tag + rest of the fence line
                    spans.push((tag_start, line_end, fmt(colors.inline_code)));
                }
            }
            pos = line_end;
            continue;
        }

        // ── Inside a code fence ──────────────────────────────────────────
        if in_code_fence {
            spans.push((line_start, line_end, fmt(colors.code_block)));
            pos = line_end;
            continue;
        }

        // ── Heading  (#+ text) ───────────────────────────────────────────
        if let Some((marker_len, text_start)) = heading_parts(line) {
            let marker_abs = line_start;
            let text_abs   = line_start + text_start;
            spans.push((marker_abs, text_abs, fmt(colors.heading_marker)));
            if text_abs < line_end {
                // heading text up to the newline
                let text_end = if raw_line.ends_with('\n') { line_end - 1 } else { line_end };
                spans.push((text_abs, text_end, fmt(colors.heading)));
                if text_end < line_end {
                    spans.push((text_end, line_end, fmt(colors.normal)));
                }
            }
            let _ = marker_len;
            pos = line_end;
            continue;
        }

        // ── Horizontal rule (---, ***, ___) ─────────────────────────────
        if is_hr(line) {
            spans.push((line_start, line_end, fmt(colors.hr)));
            pos = line_end;
            continue;
        }

        // ── Blockquote (> ) ─────────────────────────────────────────────
        if line.starts_with("> ") || line == ">" {
            let marker_len = if line.starts_with("> ") { 2 } else { 1 };
            spans.push((line_start, line_start + marker_len, fmt(colors.blockquote)));
            let rest_start = line_start + marker_len;
            let rest_end   = line_end;
            inline_spans(&text[rest_start..rest_end], rest_start, colors, &mono, &mut spans);
            pos = line_end;
            continue;
        }

        // ── List item (-, *, +, or digit.) ──────────────────────────────
        if let Some(marker_len) = list_marker_len(line) {
            spans.push((line_start, line_start + marker_len, fmt(colors.list_marker)));
            let rest_start = line_start + marker_len;
            inline_spans(&text[rest_start..line_end], rest_start, colors, &mono, &mut spans);
            pos = line_end;
            continue;
        }

        // ── Normal paragraph line ────────────────────────────────────────
        inline_spans(&text[line_start..line_end], line_start, colors, &mono, &mut spans);

        pos = line_end;
    }

    // Trailing text without a newline
    if pos < text.len() {
        inline_spans(&text[pos..], pos, colors, &mono, &mut spans);
    }

    spans
}

// ── Inline token scanner ──────────────────────────────────────────────────────

/// Append syntax-colored spans for a single segment of inline markdown text.
/// `base` is the byte offset of `text` within the full document buffer.
fn inline_spans(
    text:   &str,
    base:   usize,
    colors: TokenColors,
    mono:   &FontId,
    out:    &mut Vec<(usize, usize, TextFormat)>,
) {
    let bytes = text.as_bytes();
    let len   = bytes.len();
    let mut i = 0usize;
    let mut seg = 0usize; // start of current unstyled segment

    let fmt = |color: Color32| TextFormat {
        font_id: mono.clone(),
        color,
        ..Default::default()
    };
    let push = |out: &mut Vec<_>, from: usize, to: usize, color: Color32| {
        if from < to {
            out.push((base + from, base + to, TextFormat {
                font_id: mono.clone(),
                color,
                ..Default::default()
            }));
        }
    };

    while i < len {
        let b = bytes[i];

        // ── Inline code  `...` ───────────────────────────────────────────
        if b == b'`' {
            push(out, seg, i, colors.normal);
            let start = i;
            i += 1;
            while i < len && bytes[i] != b'`' && bytes[i] != b'\n' { i += 1; }
            if i < len && bytes[i] == b'`' { i += 1; }
            push(out, start, i, colors.inline_code);
            seg = i;
            continue;
        }

        // ── Image  ![alt](url) ───────────────────────────────────────────
        if b == b'!' && i + 1 < len && bytes[i + 1] == b'[' {
            if let Some((bracket_end, url_end)) = find_link(bytes, i + 1) {
                push(out, seg, i, colors.normal);
                push(out, i, bracket_end, fmt(colors.link_text).color); // reuse
                push(out, bracket_end, url_end, colors.link_url);
                i = url_end;
                seg = i;
                continue;
            }
        }

        // ── Link  [text](url) ────────────────────────────────────────────
        if b == b'[' {
            if let Some((bracket_end, url_end)) = find_link(bytes, i) {
                push(out, seg, i, colors.normal);
                push(out, i, bracket_end, colors.link_text);
                push(out, bracket_end, url_end, colors.link_url);
                i = url_end;
                seg = i;
                continue;
            }
        }

        // ── Bold+Italic  ***...*** ───────────────────────────────────────
        if b == b'*' && peek3(bytes, i, b'*', b'*', b'*') {
            if let Some(close) = find_closing(bytes, i + 3, b"***") {
                push(out, seg, i, colors.normal);
                push(out, i, close, colors.bold_italic);
                i = close;
                seg = i;
                continue;
            }
        }

        // ── Bold  **...** ────────────────────────────────────────────────
        if b == b'*' && peek2(bytes, i, b'*', b'*') {
            if let Some(close) = find_closing(bytes, i + 2, b"**") {
                push(out, seg, i, colors.normal);
                push(out, i, close, colors.bold);
                i = close;
                seg = i;
                continue;
            }
        }

        // ── Italic  *...* ────────────────────────────────────────────────
        // Only trigger when not followed by another '*' (avoid *** / ** handled above).
        if b == b'*' && (i + 1 >= len || bytes[i + 1] != b'*') {
            if let Some(close) = find_closing(bytes, i + 1, b"*") {
                push(out, seg, i, colors.normal);
                push(out, i, close, colors.italic);
                i = close;
                seg = i;
                continue;
            }
        }

        i += 1;
    }

    // Flush remaining segment
    if seg < len {
        push(out, seg, len, colors.normal);
    }
    let _ = fmt; // suppress unused-variable warning
}

// ── Pattern helpers ───────────────────────────────────────────────────────────

/// Returns `(marker_len, text_start)` for heading lines like "## Title".
fn heading_parts(line: &str) -> Option<(usize, usize)> {
    let mut level = 0usize;
    for b in line.bytes() {
        if b == b'#' { level += 1; } else { break; }
    }
    if level == 0 || level > 6 { return None; }
    let after = &line[level..];
    if after.starts_with(' ') {
        Some((level, level + 1)) // marker = "#+ ", text starts after space
    } else if after.is_empty() {
        Some((level, level))
    } else {
        None
    }
}

/// True for lines that are entirely `---`, `***`, or `___` (3+ chars, optional spaces).
fn is_hr(line: &str) -> bool {
    let chars: Vec<char> = line.chars().filter(|&c| !c.is_whitespace()).collect();
    if chars.len() < 3 { return false; }
    let first = chars[0];
    (first == '-' || first == '*' || first == '_') && chars.iter().all(|&c| c == first)
}

/// Returns the byte length of the list marker at the start of `line` (incl. trailing space),
/// or `None` if the line doesn't start a list item.
fn list_marker_len(line: &str) -> Option<usize> {
    let bytes = line.as_bytes();
    let len   = bytes.len();
    if len < 2 { return None; }

    // Unordered: "- ", "* ", "+ " (but not HR — already handled above)
    if (bytes[0] == b'-' || bytes[0] == b'+') && bytes[1] == b' ' {
        return Some(2);
    }
    // '*' only as list marker when it doesn't form a HR (already guarded) — but
    // HR check runs first, so if we reach here with "* " it's a list item.
    if bytes[0] == b'*' && bytes.get(1) == Some(&b' ') {
        return Some(2);
    }

    // Ordered: "1. ", "10. ", etc.
    let mut j = 0;
    while j < len && bytes[j].is_ascii_digit() { j += 1; }
    if j > 0 && j + 1 < len && bytes[j] == b'.' && bytes[j + 1] == b' ' {
        return Some(j + 2);
    }

    None
}

/// Given `bytes` starting at `[`, find the matching `](url)` and return
/// `(bracket_end, url_end)` where:
///   - `bracket_end` = byte after `]`
///   - `url_end`     = byte after `)`
fn find_link(bytes: &[u8], open_bracket: usize) -> Option<(usize, usize)> {
    let len = bytes.len();
    if open_bracket >= len || bytes[open_bracket] != b'[' { return None; }
    let mut i = open_bracket + 1;
    // Find closing ]
    while i < len && bytes[i] != b']' && bytes[i] != b'\n' { i += 1; }
    if i >= len || bytes[i] != b']' { return None; }
    let bracket_end = i + 1; // byte after ']'
    // Must be followed by '('
    if bracket_end >= len || bytes[bracket_end] != b'(' { return None; }
    let mut j = bracket_end + 1;
    while j < len && bytes[j] != b')' && bytes[j] != b'\n' { j += 1; }
    if j >= len || bytes[j] != b')' { return None; }
    Some((bracket_end, j + 1))
}

/// Look for `needle` starting at `start` in `bytes`.  Returns the byte position
/// *after* the found needle (i.e. the end of the matched token), or `None`.
fn find_closing(bytes: &[u8], start: usize, needle: &[u8]) -> Option<usize> {
    let nlen = needle.len();
    let len  = bytes.len();
    let mut i = start;
    while i + nlen <= len {
        if bytes[i] == b'\n' { return None; } // don't cross lines for simple delimiters
        if &bytes[i..i + nlen] == needle {
            return Some(i + nlen);
        }
        i += 1;
    }
    None
}

#[inline]
fn peek2(bytes: &[u8], i: usize, a: u8, b: u8) -> bool {
    bytes.len() > i + 1 && bytes[i] == a && bytes[i + 1] == b
}

#[inline]
fn peek3(bytes: &[u8], i: usize, a: u8, b: u8, c: u8) -> bool {
    bytes.len() > i + 2 && bytes[i] == a && bytes[i + 1] == b && bytes[i + 2] == c
}
