use std::collections::HashSet;
use egui::{Color32, RichText, Ui};
use crate::markdown::{Block, Inline, ParsedDoc};

/// Renders the document outline panel (H1/H2/H3 headings).
/// `collapsed` tracks which heading block indices are folded.
/// Returns `Some(block_index)` when the user clicks a heading title.
pub fn render_outline(
    ui: &mut Ui,
    doc: &ParsedDoc,
    open: &mut bool,
    collapsed: &mut HashSet<usize>,
) -> Option<usize> {
    let mut scroll_to = None;

    ui.separator();

    let header = if *open { "▼ Outline" } else { "▶ Outline" };
    if ui.selectable_label(false, header).clicked() {
        *open = !*open;
    }

    if !*open {
        return None;
    }

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
        ui.label(RichText::new("No headings").color(Color32::GRAY).size(12.0));
        return None;
    }

    // Level of the nearest collapsed ancestor; headings deeper than this are hidden.
    let mut skip_below: Option<u32> = None;

    for (i, (level, title, block_idx)) in headings.iter().enumerate() {
        // If a parent is collapsed, skip children until we resurface.
        if let Some(skip_level) = skip_below {
            if *level > skip_level {
                continue;
            } else {
                skip_below = None; // back at or above the collapsed heading's level
            }
        }

        // A heading has children if any immediately following heading is deeper.
        let has_children = headings[i + 1..]
            .iter()
            .take_while(|(l, _, _)| *l > *level)
            .count()
            > 0;

        let is_collapsed = collapsed.contains(block_idx);
        let indent = (*level - 1) as f32 * 12.0;

        ui.horizontal(|ui| {
            ui.add_space(indent);

            // Fold toggle — only shown for headings that have children.
            if has_children {
                let arrow = if is_collapsed { "▶" } else { "▼" };
                if ui.small_button(arrow).clicked() {
                    if is_collapsed {
                        collapsed.remove(block_idx);
                    } else {
                        collapsed.insert(*block_idx);
                    }
                }
            } else {
                ui.add_space(16.0); // keep titles aligned
            }

            let text = match level {
                1 => RichText::new(title.as_str()).strong().size(13.0),
                2 => RichText::new(title.as_str()).size(12.0),
                _ => RichText::new(title.as_str()).size(12.0).color(Color32::GRAY),
            };
            if ui.selectable_label(false, text).clicked() {
                scroll_to = Some(*block_idx);
            }
        });

        if has_children && is_collapsed {
            skip_below = Some(*level);
        }
    }

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
        })
        .collect()
}
