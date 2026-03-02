use egui::{Color32, Rect, Ui, vec2};
use crate::fs::{FsNode, FsNodeKind, FsTree};
use std::path::PathBuf;

/// Compute the sidebar width that fits the longest visible label without clipping.
pub fn ideal_width(ctx: &egui::Context, tree: &FsTree) -> f32 {
    let font_id = egui::FontId::proportional(13.0);
    // egui default indent step (matches ui.indent() behaviour)
    let indent_px = 14.0_f32;
    // Frame inner_margin(8) left + right, plus selectable_label button_padding(4) each side
    let overhead = 8.0 + 8.0 + 4.0 + 4.0;

    let mut max_text_w = 0.0_f32;
    if let Some(ref root) = tree.root {
        collect_max_width(ctx, root, 0, &tree.expanded, &font_id, indent_px, &mut max_text_w);
    }
    (max_text_w + overhead).clamp(180.0, 520.0)
}

fn collect_max_width(
    ctx:       &egui::Context,
    node:      &FsNode,
    depth:     u32,
    expanded:  &std::collections::HashSet<PathBuf>,
    font_id:   &egui::FontId,
    indent_px: f32,
    max_w:     &mut f32,
) {
    let label = match node.kind {
        FsNodeKind::Dir  => format!("▶ 📁  {}", node.name),
        FsNodeKind::File => format!("📄  {}", node.name),
    };
    let text_w = ctx.fonts(|f| f.layout_no_wrap(label, font_id.clone(), Color32::WHITE).size().x);
    let total = indent_px * depth as f32 + text_w;
    if total > *max_w {
        *max_w = total;
    }
    if matches!(node.kind, FsNodeKind::Dir) && expanded.contains(&node.path) {
        if let Some(ref children) = node.children {
            for child in children {
                collect_max_width(ctx, child, depth + 1, expanded, font_id, indent_px, max_w);
            }
        }
    }
}

pub fn render_sidebar(ui: &mut Ui, tree: &mut FsTree, active_color: Color32) -> Option<PathBuf> {
    let mut selected_file = None;
    let mut to_expand: Option<PathBuf> = None;

    // Compact vertical spacing; add horizontal padding around labels
    ui.spacing_mut().item_spacing.y  = 10.0;
    ui.spacing_mut().button_padding  = egui::vec2(6.0, 2.0);

    if let Some(ref root) = tree.root {
        render_node(ui, root, &mut tree.expanded, &mut tree.selected, &mut selected_file, &mut to_expand, active_color);
    }

    if let Some(path) = to_expand {
        tree.expand(&path);
    }

    selected_file
}

fn render_node(
    ui:            &mut Ui,
    node:          &FsNode,
    expanded:      &mut std::collections::HashSet<PathBuf>,
    selected:      &mut Option<PathBuf>,
    selected_file: &mut Option<PathBuf>,
    to_expand:     &mut Option<PathBuf>,
    active_color:  Color32,
) {
    match node.kind {
        FsNodeKind::Dir => {
            let is_expanded = expanded.contains(&node.path);
            let is_selected = selected.as_ref() == Some(&node.path);
            let arrow = if is_expanded { "-" } else { "▶" };
            let label = format!("{} 📁  {}", arrow, node.name);

            let text = if is_selected {
                egui::RichText::new(&label).color(active_color).strong().size(13.0)
            } else {
                egui::RichText::new(&label).strong().size(13.0)
            };

            let resp = ui.add(egui::Label::new(text).truncate().sense(egui::Sense::click()));
            paint_accent_bar(ui, resp.rect, is_selected, active_color);
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            if resp.on_hover_text(node.path.to_string_lossy().as_ref()).clicked() {
                if is_expanded {
                    expanded.remove(&node.path);
                } else {
                    expanded.insert(node.path.clone());
                    if node.children.is_none() {
                        *to_expand = Some(node.path.clone());
                    }
                }
            }

            if is_expanded {
                if let Some(ref children) = node.children {
                    ui.indent(&node.name, |ui| {
                        for child in children {
                            render_node(ui, child, expanded, selected, selected_file, to_expand, active_color);
                        }
                    });
                }
            }
        }
        FsNodeKind::File => {
            let is_selected = selected.as_ref() == Some(&node.path);
            let label = format!("📄  {}", node.name);

            let text = if is_selected {
                egui::RichText::new(&label).color(active_color).size(13.0)
            } else {
                egui::RichText::new(&label)
                    .color(ui.visuals().text_color().linear_multiply(0.85))
                    .size(13.0)
            };

            let resp = ui.add(egui::Label::new(text).truncate().sense(egui::Sense::click()));
            paint_accent_bar(ui, resp.rect, is_selected, active_color);
            if resp.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }

            if resp.on_hover_text(node.path.to_string_lossy().as_ref()).clicked() {
                *selected      = Some(node.path.clone());
                *selected_file = Some(node.path.clone());
            }
        }
    }
}

/// Paint a 2px left-edge accent bar and a subtle background tint on the selected row.
fn paint_accent_bar(ui: &Ui, row_rect: Rect, is_selected: bool, active_color: Color32) {
    if !is_selected {
        return;
    }
    // Subtle background fill
    let fill = Color32::from_rgba_unmultiplied(
        active_color.r(), active_color.g(), active_color.b(), 22,
    );
    ui.painter().rect_filled(row_rect, 2.0, fill);

    // Solid left accent bar — offset 4px left of the label so there's a visible gap
    let bar = Rect::from_min_size(
        egui::pos2(row_rect.min.x - 4.0, row_rect.min.y),
        vec2(2.0, row_rect.height()),
    );
    ui.painter().rect_filled(bar, 0.0, active_color);
}
