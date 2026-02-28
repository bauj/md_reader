use egui::{Color32, Rect, Ui, vec2};
use crate::fs::{FsNode, FsNodeKind, FsTree};
use std::path::PathBuf;

pub fn render_sidebar(ui: &mut Ui, tree: &mut FsTree, active_color: Color32) -> Option<PathBuf> {
    let mut selected_file = None;
    let mut to_expand: Option<PathBuf> = None;

    // Tight item spacing for a compact file tree
    ui.spacing_mut().item_spacing.y = 1.0;

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

            let resp = ui.selectable_label(is_selected, text);
            paint_accent_bar(ui, resp.rect, is_selected, active_color);

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
                        ui.spacing_mut().item_spacing.y = 1.0;
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

            let resp = ui.selectable_label(is_selected, text);
            paint_accent_bar(ui, resp.rect, is_selected, active_color);

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

    // Solid left accent bar
    let bar = Rect::from_min_size(row_rect.min, vec2(2.0, row_rect.height()));
    ui.painter().rect_filled(bar, 0.0, active_color);
}
