use egui::Ui;
use crate::fs::{FsNode, FsNodeKind, FsTree};
use std::path::PathBuf;

pub fn render_sidebar(ui: &mut Ui, tree: &mut FsTree) -> Option<PathBuf> {
    let mut selected_file = None;

    if let Some(ref root) = tree.root {
        render_node(ui, root, &mut tree.expanded, &mut tree.selected, &mut selected_file);
    }

    selected_file
}

fn render_node(
    ui: &mut Ui,
    node: &FsNode,
    expanded: &mut std::collections::HashSet<PathBuf>,
    selected: &mut Option<PathBuf>,
    selected_file: &mut Option<PathBuf>,
) {
    match node.kind {
        FsNodeKind::Dir => {
            let is_expanded = expanded.contains(&node.path);

            // Clickable folder toggle + name
            let arrow = if is_expanded { "▼" } else { "▶" };
            let label = format!("{} 📁 {}", arrow, node.name);

            if ui.selectable_label(
                selected.as_ref() == Some(&node.path),
                label,
            ).clicked() {
                if expanded.contains(&node.path) {
                    expanded.remove(&node.path);
                } else {
                    expanded.insert(node.path.clone());
                }
            }

            // Render children if expanded
            if is_expanded {
                ui.indent(&node.name, |ui| {
                    for child in &node.children {
                        render_node(ui, child, expanded, selected, selected_file);
                    }
                });
            }
        }
        FsNodeKind::File => {
            let label = format!("📄 {}", node.name);
            if ui.selectable_label(
                selected.as_ref() == Some(&node.path),
                label,
            ).clicked() {
                *selected = Some(node.path.clone());
                *selected_file = Some(node.path.clone());
            }
        }
    }
}
