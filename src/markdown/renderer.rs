use egui::{Color32, Frame, FontId, RichText, TextFormat, Ui};
use egui::text::LayoutJob;
use egui_extras::{Column, TableBuilder};
use crate::markdown::{Block, Inline, ParsedDoc};
use crate::markdown::highlight::Highlighter;
use super::SearchOpts;

pub fn render_markdown(
    ui:             &mut Ui,
    doc:            &ParsedDoc,
    scroll_to:      Option<usize>,
    hl:             &mut Highlighter,
    search_query:   &str,
    search_current: usize,
    opts:           SearchOpts,
) {
    let mut occurrence = 0usize;
    for (i, block) in doc.blocks.iter().enumerate() {
        ui.push_id(i, |ui| {
            if scroll_to == Some(i) {
                ui.scroll_to_cursor(Some(egui::Align::TOP));
            }
            render_block(ui, block, hl, search_query, search_current, opts, &mut occurrence);
        });
        ui.add_space(4.0);
    }
}

fn render_block(
    ui:             &mut Ui,
    block:          &Block,
    hl:             &mut Highlighter,
    search_query:   &str,
    search_current: usize,
    opts:           SearchOpts,
    occurrence:     &mut usize,
) {
    match block {
        Block::Heading(level, inlines) => {
            let size = match level {
                1 => 32.0,
                2 => 26.0,
                3 => 22.0,
                4 => 18.0,
                5 => 16.0,
                _ => 14.0,
            };
            ui.horizontal_wrapped(|ui| {
                for inline in inlines {
                    render_inline(ui, inline, Some(size), true, search_query, search_current, opts, occurrence);
                }
            });
            if *level <= 2 {
                ui.separator();
            }
        }

        Block::Paragraph(inlines) => {
            ui.horizontal_wrapped(|ui| {
                for inline in inlines {
                    render_inline(ui, inline, None, false, search_query, search_current, opts, occurrence);
                }
            });
        }

        Block::CodeBlock(lang, code) => {
            // ── Find match byte offsets within `code` ────────────────────────
            let code_matches = find_matches(code, search_query, opts);
            let qlen         = needle_len(search_query, opts);
            let occ_base     = *occurrence; // global index of the first match in this block

            // ── Build LayoutJob: syntect fg colors + search bg overlays ──────
            let lines    = hl.highlight(lang, code);
            let mut job  = LayoutJob::default();
            let mut bpos = 0usize; // byte cursor within `code`

            for line_spans in lines {
                for (color, text) in line_spans {
                    let span_end  = bpos + text.len();
                    let mut local = 0usize; // cursor within this span's text

                    for (mi, &ms) in code_matches.iter().enumerate() {
                        let me = ms + qlen;
                        if me <= bpos || ms >= span_end { continue; } // no overlap

                        let rel_s = ms.saturating_sub(bpos).max(local);
                        let rel_e = (me - bpos).min(text.len());
                        if rel_s >= rel_e { continue; }

                        if rel_s > local {
                            job.append(&text[local..rel_s], 0.0, TextFormat {
                                font_id: FontId::monospace(13.0),
                                color:   *color,
                                ..Default::default()
                            });
                        }
                        let bg = if occ_base + mi == search_current {
                            Color32::from_rgb(255, 150, 0)
                        } else {
                            Color32::from_rgb(255, 220, 50)
                        };
                        job.append(&text[rel_s..rel_e], 0.0, TextFormat {
                            font_id:    FontId::monospace(13.0),
                            color:      Color32::BLACK,
                            background: bg,
                            ..Default::default()
                        });
                        local = rel_e;
                    }

                    if local < text.len() {
                        job.append(&text[local..], 0.0, TextFormat {
                            font_id: FontId::monospace(13.0),
                            color:   *color,
                            ..Default::default()
                        });
                    }
                    bpos = span_end;
                }
            }
            *occurrence += code_matches.len();

            Frame::canvas(&ui.style())
                .fill(Color32::from_gray(28))
                .inner_margin(egui::Margin::symmetric(10, 8))
                .corner_radius(4.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    if !lang.is_empty() {
                        ui.label(
                            RichText::new(lang.as_str())
                                .size(10.0)
                                .color(Color32::from_gray(140)),
                        );
                        ui.add_space(2.0);
                    }
                    ui.label(job);
                });
        }

        Block::BlockQuote(inlines) => {
            ui.horizontal(|ui| {
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(3.0, ui.spacing().interact_size.y.max(16.0)),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(rect, 2.0, Color32::from_gray(100));
                ui.add_space(6.0);
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        for inline in inlines {
                            render_inline(ui, inline, None, false, search_query, search_current, opts, occurrence);
                        }
                    });
                });
            });
        }

        Block::List(ordered, items) => {
            for (i, item_inlines) in items.iter().enumerate() {
                ui.horizontal_wrapped(|ui| {
                    let bullet = if *ordered {
                        format!("{}.", i + 1)
                    } else {
                        "•".to_string()
                    };
                    ui.label(RichText::new(bullet).size(14.0));
                    ui.add_space(4.0);
                    for inline in item_inlines {
                        render_inline(ui, inline, None, false, search_query, search_current, opts, occurrence);
                    }
                });
            }
        }

        Block::Table(headers, rows) => {
            let col_count = headers.len().max(1);

            // Cell<usize> lets us thread the occurrence counter across the
            // sequential-but-closure-based TableBuilder API without needing
            // multiple &mut borrows to the same variable.
            let occ = std::cell::Cell::new(*occurrence);

            Frame::canvas(&ui.style())
                .fill(Color32::from_gray(24))
                .corner_radius(4.0)
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());

                    TableBuilder::new(ui)
                        .striped(true)
                        .columns(Column::remainder().resizable(true), col_count)
                        .header(24.0, |mut row| {
                            for header_inlines in headers {
                                row.col(|ui| {
                                    ui.horizontal_wrapped(|ui| {
                                        let mut o = occ.get();
                                        for il in header_inlines {
                                            render_inline(ui, il, None, true, search_query, search_current, opts, &mut o);
                                        }
                                        occ.set(o);
                                    });
                                });
                            }
                        })
                        .body(|mut body| {
                            for data_row in rows {
                                body.row(20.0, |mut row| {
                                    for cell_inlines in data_row.iter() {
                                        row.col(|ui| {
                                            ui.horizontal_wrapped(|ui| {
                                                let mut o = occ.get();
                                                for il in cell_inlines {
                                                    render_inline(ui, il, None, false, search_query, search_current, opts, &mut o);
                                                }
                                                occ.set(o);
                                            });
                                        });
                                    }
                                    for _ in data_row.len()..col_count {
                                        row.col(|_| {});
                                    }
                                });
                            }
                        });
                });

            *occurrence = occ.get();
        }

        Block::Rule => {
            ui.separator();
        }
    }
}

fn render_inline(
    ui:             &mut Ui,
    inline:         &Inline,
    size:           Option<f32>,
    strong:         bool,
    search_query:   &str,
    search_current: usize,
    opts:           SearchOpts,
    occurrence:     &mut usize,
) {
    let base = size.unwrap_or(14.0);
    match inline {
        Inline::Text(t) => {
            render_text_highlighted(ui, t, search_query, base, strong, false, opts, occurrence, search_current);
        }
        Inline::Bold(t) => {
            render_text_highlighted(ui, t, search_query, base, true, false, opts, occurrence, search_current);
        }
        Inline::Italic(t) => {
            render_text_highlighted(ui, t, search_query, base, strong, true, opts, occurrence, search_current);
        }
        Inline::BoldItalic(t) => {
            render_text_highlighted(ui, t, search_query, base, true, true, opts, occurrence, search_current);
        }
        Inline::Code(c) => {
            // Precompute match positions before the Frame closure so we do not
            // need to capture `occurrence` (a `&mut usize`) inside it.
            let match_offs  = find_matches(c, search_query, opts);
            let qlen        = needle_len(search_query, opts);
            let occ_base    = *occurrence;
            let match_count = match_offs.len();

            Frame::canvas(&ui.style())
                .fill(Color32::from_gray(45))
                .inner_margin(egui::Margin::symmetric(3, 1))
                .corner_radius(3.0)
                .show(ui, |ui| {
                    if match_offs.is_empty() {
                        ui.monospace(c.as_str());
                    } else {
                        let fg   = ui.visuals().text_color();
                        let nfmt = TextFormat {
                            font_id: FontId::monospace(13.0),
                            color:   fg,
                            ..Default::default()
                        };
                        let mut job = LayoutJob::default();
                        let mut pos = 0usize;
                        for (mi, &start) in match_offs.iter().enumerate() {
                            let end = start + qlen;
                            if start > pos {
                                job.append(&c[pos..start], 0.0, nfmt.clone());
                            }
                            let bg = if occ_base + mi == search_current {
                                Color32::from_rgb(255, 150, 0)
                            } else {
                                Color32::from_rgb(255, 220, 50)
                            };
                            job.append(&c[start..end], 0.0, TextFormat {
                                font_id:    FontId::monospace(13.0),
                                color:      Color32::BLACK,
                                background: bg,
                                ..Default::default()
                            });
                            pos = end;
                        }
                        if pos < c.len() {
                            job.append(&c[pos..], 0.0, nfmt);
                        }
                        ui.label(job);
                    }
                });

            *occurrence += match_count;
        }
        Inline::Link(text, url) => {
            if ui.link(text.as_str()).clicked() {
                let _ = open::that(url.as_str());
            }
        }
    }
}

/// Render `text` splitting on matches according to `opts`.
/// Uses orange background when `*occurrence == current_match`, yellow otherwise.
/// Increments `*occurrence` once per match rendered.
fn render_text_highlighted(
    ui:            &mut Ui,
    text:          &str,
    query:         &str,
    size:          f32,
    bold:          bool,
    italic:        bool,
    opts:          SearchOpts,
    occurrence:    &mut usize,
    current_match: usize,
) {
    let matches = find_matches(text, query, opts);

    if matches.is_empty() {
        ui.label(styled_rt(text, size, bold, italic));
        return;
    }

    let qlen    = needle_len(query, opts);
    let mut pos = 0usize;

    for &start in &matches {
        let end = start + qlen;
        if start > pos {
            ui.label(styled_rt(&text[pos..start], size, bold, italic));
        }
        let bg = if *occurrence == current_match {
            Color32::from_rgb(255, 150, 0)
        } else {
            Color32::from_rgb(255, 220, 50)
        };
        *occurrence += 1;
        ui.label(
            styled_rt(&text[start..end], size, bold, italic)
                .background_color(bg)
                .color(Color32::BLACK),
        );
        pos = end;
    }
    if pos < text.len() {
        ui.label(styled_rt(&text[pos..], size, bold, italic));
    }
}

/// Return byte offsets of all non-overlapping matches of `query` inside `text`,
/// respecting `opts.case_sensitive` and `opts.whole_word`.
fn find_matches(text: &str, query: &str, opts: SearchOpts) -> Vec<usize> {
    if query.is_empty() { return Vec::new(); }

    // Build (possibly lowercased) search targets.
    let needle:   String = if opts.case_sensitive { query.to_string() } else { query.to_lowercase() };
    let haystack: String = if opts.case_sensitive { text.to_string()  } else { text.to_lowercase()  };

    let mut p   = 0usize;
    let mut out = Vec::new();

    while p < haystack.len() {
        match haystack[p..].find(&needle as &str) {
            None      => break,
            Some(rel) => {
                let ms = p + rel;
                let me = ms + needle.len();
                if !opts.whole_word || is_word_boundary(text, ms, me) {
                    out.push(ms);
                    p = me;
                } else {
                    // Advance past the first character of this rejected match.
                    p = ms + text[ms..].chars().next().map_or(1, |c| c.len_utf8());
                }
            }
        }
    }
    out
}

/// Returns `true` when the substring `text[start..end]` sits on a word boundary,
/// i.e. the characters immediately before and after are not alphanumeric or `_`.
fn is_word_boundary(text: &str, start: usize, end: usize) -> bool {
    let is_wc  = |c: char| c.is_alphanumeric() || c == '_';
    let before = text[..start].chars().next_back();
    let after  = text[end..].chars().next();
    before.map_or(true, |c| !is_wc(c)) && after.map_or(true, |c| !is_wc(c))
}

/// Byte length of the (possibly lowercased) needle, for use when slicing matched text.
#[inline]
fn needle_len(query: &str, opts: SearchOpts) -> usize {
    if opts.case_sensitive { query.len() } else { query.to_lowercase().len() }
}

#[inline]
fn styled_rt(text: &str, size: f32, bold: bool, italic: bool) -> RichText {
    let rt = RichText::new(text).size(size);
    let rt = if bold   { rt.strong()  } else { rt };
    let rt = if italic { rt.italics() } else { rt };
    rt
}
