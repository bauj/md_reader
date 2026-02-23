use egui::{Color32, RichText, Ui};
use crate::markdown::{Block, Inline, ParsedDoc};

/// Renders the document outline panel (H1/H2/H3 headings).
/// Returns `Some(block_index)` when the user clicks a heading.
pub fn render_outline(ui: &mut Ui, doc: &ParsedDoc, open: &mut bool) -> Option<usize> {
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

    for (level, title, block_index) in &headings {
        let level = *level;
        let block_index = *block_index;
        let indent = (level - 1) as f32 * 12.0;
        ui.horizontal(|ui| {
            ui.add_space(indent);
            let text = match level {
                1 => RichText::new(title).strong().size(13.0),
                2 => RichText::new(title).size(12.0),
                _ => RichText::new(title).size(12.0).color(Color32::GRAY),
            };
            if ui.selectable_label(false, text).clicked() {
                scroll_to = Some(block_index);
            }
        });
    }

    scroll_to
}

fn inlines_to_text(inlines: &[Inline]) -> String {
    inlines.iter().map(|i| match i {
        Inline::Text(s) | Inline::Bold(s) | Inline::Italic(s) | Inline::BoldItalic(s) | Inline::Code(s) => s.as_str(),
        Inline::Link(text, _) => text.as_str(),
    }).collect()
}
