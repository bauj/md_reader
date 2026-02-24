use egui::Ui;
use crate::fs::{FsNode, FsNodeKind, FsTree};
use std::path::PathBuf;

pub fn render_sidebar(ui: &mut Ui, tree: &mut FsTree) -> Option<PathBuf> {
    let mut selected_file = None;
    let mut to_expand: Option<PathBuf> = None;

    if let Some(ref root) = tree.root {
        render_node(ui, root, &mut tree.expanded, &mut tree.selected, &mut selected_file, &mut to_expand);
    }

    // Lazy-load children for the directory that was just expanded this frame.
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
) {
    match node.kind {
        FsNodeKind::Dir => {
            let is_expanded = expanded.contains(&node.path);
            let arrow = if is_expanded { "▼" } else { "▶" };
            let label = format!("{} 📁 {}", arrow, node.name);

            if ui.selectable_label(selected.as_ref() == Some(&node.path), label).clicked() {
                if is_expanded {
                    expanded.remove(&node.path);
                } else {
                    expanded.insert(node.path.clone());
                    // If children haven't been loaded yet, request a lazy scan.
                    if node.children.is_none() {
                        *to_expand = Some(node.path.clone());
                    }
                }
            }

            if is_expanded {
                if let Some(ref children) = node.children {
                    ui.indent(&node.name, |ui| {
                        for child in children {
                            render_node(ui, child, expanded, selected, selected_file, to_expand);
                        }
                    });
                }
            }
        }
        FsNodeKind::File => {
            let label = format!("📄 {}", node.name);
            if ui.selectable_label(selected.as_ref() == Some(&node.path), label).clicked() {
                *selected      = Some(node.path.clone());
                *selected_file = Some(node.path.clone());
            }
        }
    }
}
