use std::collections::HashSet;
use egui::{Color32, Rect, RichText, Ui, vec2};
use crate::markdown::{Block, Inline, ParsedDoc};

pub fn render_outline(
    ui:           &mut Ui,
    doc:          &ParsedDoc,
    open:         &mut bool,
    collapsed:    &mut HashSet<usize>,
    active_color: Color32,
) -> Option<usize> {
    let mut scroll_to = None;

    ui.add_space(6.0);

    // ── Section header row ────────────────────────────────────────────────────
    let header_resp = ui.horizontal(|ui| {
        let arrow = if *open { "v" } else { ">" };
        let arrow_resp = ui.add(
            egui::Label::new(
                RichText::new(arrow)
                    .size(9.0)
                    .color(ui.visuals().weak_text_color()),
            )
            .sense(egui::Sense::click()),
        );
        ui.add_space(4.0);
        let label_resp = ui.add(
            egui::Label::new(
                RichText::new("OUTLINE")
                    .size(10.0)
                    .color(ui.visuals().weak_text_color()),
            )
            .sense(egui::Sense::click()),
        );
        if arrow_resp.hovered() || label_resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
        arrow_resp.clicked() || label_resp.clicked()
    });
    if header_resp.inner {
        *open = !*open;
    }

    if !*open {
        return None;
    }

    ui.add_space(4.0);

    // ── Thin separator under header ───────────────────────────────────────────
    let sep_rect = ui.allocate_space(vec2(ui.available_width(), 1.0)).1;
    ui.painter().rect_filled(
        sep_rect,
        0.0,
        Color32::from_rgba_unmultiplied(
            ui.visuals().text_color().r(),
            ui.visuals().text_color().g(),
            ui.visuals().text_color().b(),
            20,
        ),
    );
    ui.add_space(4.0);

    let headings: Vec<(u32, String, usize)> = doc
        .blocks
        .iter()
        .enumerate()
        .filter_map(|(i, block)| match block {
            Block::Heading(level, inlines) if *level <= 3 => {
                Some((*level, inlines_to_text(inlines), i))
            }
            _ => None,
        })
        .collect();

    if headings.is_empty() {
        ui.add_space(2.0);
        ui.label(
            RichText::new("No headings")
                .color(ui.visuals().weak_text_color())
                .size(11.0),
        );
        return None;
    }

    ui.spacing_mut().item_spacing.y = 0.0;

    let mut skip_below: Option<u32> = None;

    for (i, (level, title, block_idx)) in headings.iter().enumerate() {
        if let Some(skip_level) = skip_below {
            if *level > skip_level {
                continue;
            } else {
                skip_below = None;
            }
        }

        let has_children = headings[i + 1..]
            .iter()
            .take_while(|(l, _, _)| *l > *level)
            .next()
            .is_some();

        let is_collapsed = collapsed.contains(block_idx);

        // Indentation: H1 flush, H2 slight, H3 deeper
        let indent = match level {
            1 => 0.0,
            2 => 10.0,
            _ => 20.0,
        };

        // Vertical padding per level
        let top_pad = match level {
            1 => 11.0,
            2 => 7.0,
            _ => 4.0,
        };
        if top_pad > 0.0 { ui.add_space(top_pad); }

        let clicked = ui.horizontal(|ui| {
            ui.add_space(indent);

            // Fold toggle
            if has_children {
                let arrow = if is_collapsed { ">" } else { "v" };
                let resp = ui.add(
                    egui::Label::new(
                        RichText::new(arrow)
                            .size(9.0)
                            .color(ui.visuals().weak_text_color()),
                    )
                    .sense(egui::Sense::click()),
                );
                if resp.hovered() {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                if resp.clicked() {
                    if is_collapsed {
                        collapsed.remove(block_idx);
                    } else {
                        collapsed.insert(*block_idx);
                    }
                }
                ui.add_space(2.0);
            } else {
                ui.add_space(13.0);
            }

            // Heading text styled by level
            let text = match level {
                1 => RichText::new(title.as_str()).strong().size(16.5),
                2 => RichText::new(title.as_str()).size(15.0),
                _ => RichText::new(title.as_str())
                    .size(13.5)
                    .color(ui.visuals().weak_text_color()),
            };

            // Reserve a shape slot before the label so the highlight paints behind the text
            let highlight_idx = ui.painter().add(egui::Shape::Noop);

            let resp = ui.add(
                egui::Label::new(text)
                    .sense(egui::Sense::click())
                    .truncate(),
            );

            // Hover highlight and hand cursor for all headings
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                ui.painter().set(
                    highlight_idx,
                    egui::Shape::rect_filled(
                        resp.rect.expand2(egui::vec2(4.0, 1.0)),
                        2.0,
                        ui.visuals().widgets.hovered.bg_fill,
                    ),
                );
            }

            // Left accent bar for H1 (painted over the label area)
            if *level == 1 {
                let bar_x = resp.rect.min.x - 4.0;
                let bar = Rect::from_min_size(
                    egui::pos2(bar_x, resp.rect.min.y + 1.0),
                    vec2(2.0, resp.rect.height() - 2.0),
                );
                let alpha = if resp.hovered() { 200u8 } else { 100u8 };
                ui.painter().rect_filled(
                    bar,
                    1.0,
                    Color32::from_rgba_unmultiplied(
                        active_color.r(), active_color.g(), active_color.b(), alpha,
                    ),
                );
            }

            resp.clicked()
        }).inner;

        if clicked {
            scroll_to = Some(*block_idx);
        }

        if has_children && is_collapsed {
            skip_below = Some(*level);
        }
    }

    ui.add_space(8.0);
    scroll_to
}

fn inlines_to_text(inlines: &[Inline]) -> String {
    inlines
        .iter()
        .map(|i| match i {
            Inline::Text(s)
            | Inline::Bold(s)
            | Inline::Italic(s)
            | Inline::BoldItalic(s)
            | Inline::Code(s) => s.as_str(),
            Inline::Link(text, _) => text.as_str(),
            Inline::Image(_, alt) => alt.as_str(),
        })
        .collect()
}
