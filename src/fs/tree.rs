use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct FsNode {
    pub name: String,
    pub path: PathBuf,
    pub kind: FsNodeKind,
    pub children: Vec<FsNode>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FsNodeKind {
    File,
    Dir,
}

pub struct FsTree {
    pub root: Option<FsNode>,
    pub expanded: HashSet<PathBuf>,
    pub selected: Option<PathBuf>,
}

impl Default for FsTree {
    fn default() -> Self {
        FsTree {
            root: None,
            expanded: HashSet::new(),
            selected: None,
        }
    }
}

impl FsTree {
    pub fn new(root_path: PathBuf) -> Self {
        let root = Self::scan_dir(&root_path);
        let mut tree = FsTree {
            root,
            expanded: HashSet::new(),
            selected: None,
        };
        if let Some(ref root_node) = tree.root {
            tree.expanded.insert(root_node.path.clone());
        }
        tree
    }

    fn scan_dir(path: &Path) -> Option<FsNode> {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("root")
            .to_string();

        let mut children = Vec::new();

        if let Ok(entries) = std::fs::read_dir(path) {
            let mut entries: Vec<_> = entries
                .filter_map(|e| e.ok())
                .collect();
            entries.sort_by_key(|e| {
                let is_dir = e.path().is_dir();
                (!is_dir, e.file_name()) // Dirs first, then alphabetical
            });

            for entry in entries {
                let entry_path = entry.path();
                if entry_path.is_dir() {
                    if let Some(node) = Self::scan_dir(&entry_path) {
                        children.push(node);
                    }
                } else {
                    let file_name = entry
                        .file_name()
                        .to_string_lossy()
                        .to_string();
                    children.push(FsNode {
                        name: file_name,
                        path: entry_path,
                        kind: FsNodeKind::File,
                        children: Vec::new(),
                    });
                }
            }
        }

        Some(FsNode {
            name,
            path: path.to_path_buf(),
            kind: FsNodeKind::Dir,
            children,
        })
    }

    /// Returns all file paths in the tree in depth-first order.
    pub fn all_files(&self) -> Vec<PathBuf> {
        let mut files = Vec::new();
        if let Some(ref root) = self.root {
            Self::collect_files(root, &mut files);
        }
        files
    }

    fn collect_files(node: &FsNode, out: &mut Vec<PathBuf>) {
        match node.kind {
            FsNodeKind::File => out.push(node.path.clone()),
            FsNodeKind::Dir  => node.children.iter().for_each(|c| Self::collect_files(c, out)),
        }
    }

    pub fn toggle_expand(&mut self, path: PathBuf) {
        if self.expanded.contains(&path) {
            self.expanded.remove(&path);
        } else {
            self.expanded.insert(path);
        }
    }

    pub fn select(&mut self, path: PathBuf) {
        self.selected = Some(path);
    }
}
