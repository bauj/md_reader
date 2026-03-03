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
) -> (Option<usize>, Vec<(usize, f32)>) {
    // Capture content width ONCE before any block renders.
    // At this point max_rect still reflects the original content_rect from render_preview.
    // We use this to give every block a fresh bounded max_rect so that overflow from
    // one block cannot grow available_width() for subsequent blocks.
    let content_w = ui.max_rect().width();

    let mut occurrence        = 0usize;
    let mut task_occ          = 0usize;
    let mut toggled:           Option<usize>      = None;
    let mut initial_y:         Option<f32>        = None;
    let mut heading_positions: Vec<(usize, f32)>  = Vec::new();

    for (i, block) in doc.blocks.iter().enumerate() {
        let cursor = ui.cursor();
        let iy = *initial_y.get_or_insert(cursor.min.y);

        // Only track headings that appear in the outline (level <= 3).
        if let Block::Heading(level, _) = block {
            if *level <= 3 {
                // Store content position relative to start of first block.
                // This is stable across scroll frames.
                heading_positions.push((i, cursor.min.y - iy));
            }
        }

        let block_rect = egui::Rect::from_min_size(
            cursor.min,
            egui::vec2(content_w, 200_000.0),
        );

        // new_child() does NOT allocate in the parent — we control that below.
        // Giving each block its own bounded max_rect prevents a wide block from
        // growing the parent's max_rect and widening available_width() for the
        // blocks that follow (which would break text wrapping).
        let mut block_ui = ui.new_child(
            egui::UiBuilder::new()
                .id_salt(i)
                .max_rect(block_rect),
        );

        if scroll_to == Some(i) {
            block_ui.scroll_to_cursor(Some(egui::Align::TOP));
        }

        render_block(&mut block_ui, block, hl, search_query, search_current, opts, &mut occurrence, &mut task_occ, &mut toggled);

        // Advance the parent cursor: full rendered height, width clamped to content_w.
        // This prevents layout overflow from ever reaching the parent UI.
        let block_min = block_ui.min_rect();
        let clamped = egui::Rect::from_min_max(
            block_min.min,
            egui::pos2(block_rect.max.x, block_min.max.y),
        );
        ui.advance_cursor_after_rect(clamped);

        ui.add_space(12.0);
    }

    (toggled, heading_positions)
}

fn render_block(
    ui:             &mut Ui,
    block:          &Block,
    hl:             &mut Highlighter,
    search_query:   &str,
    search_current: usize,
    opts:           SearchOpts,
    occurrence:     &mut usize,
    task_occ:       &mut usize,
    toggled:        &mut Option<usize>,
) {
    match block {
        Block::Heading(level, inlines) => {
            let size = match level {
                1 => 32.0,  // H1
                2 => 24.0,  // H2
                3 => 20.0,  // H3
                4 => 18.0,  // H4
                5 => 16.0,  // H5
                _ => 14.0,  // H6+
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
                ui.spacing_mut().item_spacing.y = 18.0; // Increased line height within paragraphs
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
            let bg = ui.visuals().panel_fill;
            let luminance = 0.299 * bg.r() as f32 + 0.587 * bg.g() as f32 + 0.114 * bg.b() as f32;
            let light_bg = luminance > 128.0;
            let lines    = hl.highlight(lang, code, light_bg);
            let mut job  = LayoutJob::default();
            // Limit the galley to the available column width (frame inner-margin = 12px each side).
            // Without this the LayoutJob requests its natural line width and widens the block.
            job.wrap.max_width   = (ui.available_width() - 24.0).max(100.0);
            job.wrap.break_anywhere = true;
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
                                font_id: FontId::monospace(14.0), // Slightly larger font
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
                            font_id:    FontId::monospace(14.0), // Slightly larger font
                            color:      Color32::BLACK,
                            background: bg,
                            ..Default::default()
                        });
                        local = rel_e;
                    }

                    if local < text.len() {
                        job.append(&text[local..], 0.0, TextFormat {
                            font_id: FontId::monospace(14.0), // Slightly larger font
                            color:   *color,
                            ..Default::default()
                        });
                    }
                    bpos = span_end;
                }
            }
            *occurrence += code_matches.len();

            // Use a slightly darker background for better contrast
            let code_bg = if light_bg {
                Color32::from_rgb(240, 240, 240) // Lighter than faint_bg_color for light themes
            } else {
                Color32::from_rgb(40, 40, 40) // Darker than faint_bg_color for dark themes
            };

            Frame::canvas(&ui.style())
                .fill(code_bg)
                .inner_margin(egui::Margin::symmetric(12, 10))
                .corner_radius(6.0)
                .show(ui, |ui| {
                    let w = ui.available_width();
                    ui.set_min_width(w);
                    ui.set_max_width(w);

                    // Header with language and copy button
                    ui.horizontal(|ui| {
                        ui.vertical_centered(|ui| {
                            if !lang.is_empty() {
                                ui.label(
                                    RichText::new(lang.as_str())
                                        .size(12.0)
                                        .color(Color32::from_gray(140))
                                        .strong(),
                                );
                            }
                        });
                        
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if ui.button("Copy").on_hover_text("Copy to clipboard").clicked() {
                                ui.ctx().copy_text(code.to_string());
                            }
                        });
                    });
                    
                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);
                    
                    ui.label(job);
                });
        }

        Block::BlockQuote(inlines) => {
            ui.horizontal(|ui| {
                // Thicker left border with theme color
                let border_width = 4.0;
                let border_color = ui.visuals().widgets.active.bg_fill;
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(border_width, ui.spacing().interact_size.y.max(24.0)),
                    egui::Sense::hover(),
                );
                ui.painter().rect_filled(rect, border_width, border_color);
                
                ui.add_space(12.0); // Increased spacing between border and text
                
                ui.vertical(|ui| {
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing.y = 18.0; // Better line height
                        // Italicize blockquote text by using a muted color
                        let original_fg = ui.visuals().text_color();
                        ui.visuals_mut().override_text_color = Some(ui.visuals().weak_text_color());
                        for inline in inlines {
                            render_inline(ui, inline, None, false, search_query, search_current, opts, occurrence);
                        }
                        ui.visuals_mut().override_text_color = Some(original_fg);
                    });
                });
            });
        }

        Block::List(ordered, items) => {
            for (i, item) in items.iter().enumerate() {
                // ── Header row: bullet/checkbox + inline text only ─────────
                // Keeping children out of this row prevents the checkbox from
                // being vertically centered across the full item height.
                ui.horizontal(|ui| {
                    ui.add_space(20.0);

                    if let Some(checked) = item.checked {
                        let my_occ = *task_occ;
                        *task_occ += 1;
                        let mut val = checked;
                        if ui.checkbox(&mut val, "").changed() {
                            *toggled = Some(my_occ);
                        }
                    } else {
                        let bullet = if *ordered {
                            format!("{}. ", i + 1)
                        } else {
                            "• ".to_string()
                        };
                        ui.label(
                            RichText::new(bullet)
                                .size(16.0)
                                .color(ui.visuals().weak_text_color()),
                        );
                        ui.add_space(4.0);
                    }

                    ui.vertical(|ui| {
                        if !item.inlines.is_empty() {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.y = 16.0;
                                for inline in &item.inlines {
                                    render_inline(ui, inline, None, false, search_query, search_current, opts, occurrence);
                                }
                            });
                        }
                    });
                });

                // ── Children (sub-lists etc.) indented below ───────────────
                if !item.children.is_empty() {
                    ui.horizontal(|ui| {
                        ui.add_space(36.0);
                        ui.vertical(|ui| {
                            for child in &item.children {
                                render_block(ui, child, hl, search_query, search_current, opts, occurrence, task_occ, toggled);
                            }
                        });
                    });
                }

                ui.add_space(4.0);
            }
        }

        Block::Table(headers, rows) => {
            let col_count = headers.len().max(1);

            // Cell<usize> lets us thread the occurrence counter across the
            // sequential-but-closure-based TableBuilder API without needing
            // multiple &mut borrows to the same variable.
            let occ = std::cell::Cell::new(*occurrence);

            // Simple table styling without zebra striping
            let header_bg = ui.visuals().widgets.active.bg_fill;

            Frame::canvas(&ui.style())
                .fill(ui.visuals().faint_bg_color)
                .corner_radius(6.0)
                .stroke(ui.visuals().widgets.noninteractive.bg_stroke)
                .show(ui, |ui| {
                    let w = ui.available_width();
                    ui.set_min_width(w);
                    ui.set_max_width(w);

                    let spacing_x = ui.spacing().item_spacing.x;
                    let col_width = ((w - spacing_x * (col_count as f32 - 1.0)) / col_count as f32).max(0.0);

                    TableBuilder::new(ui)
                        .striped(false) // Disable zebra striping
                        .columns(Column::exact(col_width), col_count)
                        .header(28.0, |mut row| {
                            for header_inlines in headers {
                                row.col(|ui| {
                                    // Fill the full cell rect as the header background
                                    ui.painter().rect_filled(ui.max_rect(), 0.0, header_bg);
                                    // Vertical centering: push content down by half the slack
                                    let text_h = ui.text_style_height(&egui::TextStyle::Body);
                                    let vpad = ((ui.available_height() - text_h) / 2.0).max(2.0);
                                    ui.add_space(vpad);
                                    // Horizontal centering via top-down centered layout
                                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                                        ui.set_width(ui.available_width());
                                        ui.spacing_mut().item_spacing.y = 0.0;
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
                                body.row(24.0, |mut row| {
                                    for cell_inlines in data_row.iter() {
                                        row.col(|ui| {
                                            // Simple cell without alternating background
                                            ui.horizontal_wrapped(|ui| {
                                                ui.spacing_mut().item_spacing.y = 16.0;
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
            render_text_highlighted(ui, t, search_query, base, strong, false, None, opts, occurrence, search_current);
        }
        Inline::Bold(t) => {
            render_text_highlighted(ui, t, search_query, base, true, false, None, opts, occurrence, search_current);
        }
        Inline::Italic(t) => {
            render_text_highlighted(ui, t, search_query, base, strong, true, None, opts, occurrence, search_current);
        }
        Inline::BoldItalic(t) => {
            render_text_highlighted(ui, t, search_query, base, true, true, None, opts, occurrence, search_current);
        }
        Inline::Code(c) => {
            // Precompute match positions before the Frame closure so we do not
            // need to capture `occurrence` (a `&mut usize`) inside it.
            let match_offs  = find_matches(c, search_query, opts);
            let qlen        = needle_len(search_query, opts);
            let occ_base    = *occurrence;
            let match_count = match_offs.len();

            Frame::canvas(&ui.style())
                .fill(ui.visuals().faint_bg_color)
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
            // Check if URL is external (starts with http:// or https://)
            let is_external = url.starts_with("http://") || url.starts_with("https://");
            
            ui.horizontal(|ui| {
                let resp = ui.label(
                    RichText::new(text.as_str())
                        .color(Color32::from_rgb(43, 121, 162))
                ).on_hover_text(if is_external {
                    format!("Ctrl+Click to open external link: {}", url)
                } else {
                    format!("Ctrl+Click to open: {}", url)
                });

                // Add external link indicator for external URLs
                if is_external {
                    ui.add_space(4.0);
                    ui.label(
                        RichText::new("[external]")
                            .color(Color32::from_rgb(43, 121, 162))
                            .size(10.0)
                            .italics()
                    );
                }

                if resp.hovered() {
                    // Underline on hover
                    ui.painter().line_segment(
                        [resp.rect.left_bottom(), resp.rect.right_bottom()],
                        egui::Stroke::new(1.0, Color32::from_rgb(43, 121, 162))
                    );
                    
                    let ctrl_held = ui.ctx().input(|i| i.modifiers.ctrl);
                    if ctrl_held {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
                if resp.clicked() {
                    let ctrl_held = ui.ctx().input(|i| i.modifiers.ctrl);
                    if ctrl_held {
                        let _ = open::that(url.as_str());
                    }
                }
            });
        }
        Inline::Image(url, alt) => {
            // Try to load image from file path or URL
            if let Ok(img_data) = load_image(url) {
                let texture = ui.ctx().load_texture(
                    url,
                    img_data,
                    Default::default(),
                );
                // Constrain image to reasonable dimensions while maintaining aspect ratio
                ui.image(&texture);
            } else {
                // Fallback: show alt text or URL
                ui.label(
                    RichText::new(format!("[Image: {}]", if alt.is_empty() { url } else { alt }))
                        .color(Color32::GRAY)
                        .italics()
                );
            }
        }
    }
}

/// Render `text` splitting on matches according to `opts`.
/// Uses orange background when `*occurrence == current_match`, yellow otherwise.
/// Increments `*occurrence` once per match rendered.
fn render_text_highlighted(
    ui:             &mut Ui,
    text:           &str,
    query:          &str,
    size:           f32,
    bold:           bool,
    italic:         bool,
    explicit_color: Option<Color32>,
    opts:           SearchOpts,
    occurrence:     &mut usize,
    current_match:  usize,
) {
    let matches = find_matches(text, query, opts);

    if matches.is_empty() {
        ui.label(styled_rt(text, size, bold, italic, explicit_color));
        return;
    }

    let qlen    = needle_len(query, opts);
    let mut pos = 0usize;

    for &start in &matches {
        let end = start + qlen;
        if start > pos {
            ui.label(styled_rt(&text[pos..start], size, bold, italic, explicit_color));
        }
        let bg = if *occurrence == current_match {
            Color32::from_rgb(255, 150, 0)
        } else {
            Color32::from_rgb(255, 220, 50)
        };
        *occurrence += 1;
        ui.label(
            styled_rt(&text[start..end], size, bold, italic, explicit_color)
                .background_color(bg)
                .color(Color32::BLACK),
        );
        pos = end;
    }
    if pos < text.len() {
        ui.label(styled_rt(&text[pos..], size, bold, italic, explicit_color));
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
fn styled_rt(text: &str, size: f32, bold: bool, italic: bool, explicit_color: Option<Color32>) -> RichText {
    let rt = if bold {
        RichText::new(text)
            .size(size)
            .font(FontId::new(size, egui::FontFamily::Name("Bold".into())))
    } else {
        RichText::new(text).size(size)
    };
    let rt = if italic { rt.italics() } else { rt };
    if let Some(c) = explicit_color { rt.color(c) } else { rt }
}

/// Load an image from a file path, returning ImageData for egui.
/// Supports local file paths; URLs are not supported yet.
fn load_image(path: &str) -> Result<egui::ImageData, String> {
    use std::path::Path;

    // Try to load as a local file
    let file_path = Path::new(path);
    if file_path.is_file() {
        let img = image::open(file_path)
            .map_err(|e| format!("Failed to open image: {}", e))?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        let pixels = rgba.into_vec();
        Ok(egui::ImageData::Color(std::sync::Arc::new(
            egui::ColorImage {
                size: [w as usize, h as usize],
                pixels: pixels.chunks_exact(4)
                    .map(|p| egui::Color32::from_rgba_unmultiplied(p[0], p[1], p[2], p[3]))
                    .collect(),
            }
        )))
    } else {
        Err("Image path not found or not a file".to_string())
    }
}
