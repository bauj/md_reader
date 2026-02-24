use std::collections::HashSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug)]
pub struct FsNode {
    pub name: String,
    pub path: PathBuf,
    pub kind: FsNodeKind,
    /// `None`  — directory whose children have not been scanned yet (lazy).
    /// `Some`  — already scanned (files always carry `Some(vec![])`,
    ///           expanded dirs carry `Some(children)`).
    pub children: Option<Vec<FsNode>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum FsNodeKind {
    File,
    Dir,
}

pub struct FsTree {
    pub root:     Option<FsNode>,
    pub expanded: HashSet<PathBuf>,
    pub selected: Option<PathBuf>,
}

impl Default for FsTree {
    fn default() -> Self {
        FsTree { root: None, expanded: HashSet::new(), selected: None }
    }
}

impl FsTree {
    /// Open `root_path`, scanning only the top level immediately.
    /// Sub-directories are loaded on demand via [`Self::expand`].
    pub fn new(root_path: PathBuf) -> Self {
        let name = root_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("root")
            .to_string();

        let root = FsNode {
            name,
            path: root_path.clone(),
            kind: FsNodeKind::Dir,
            children: Some(scan_one_level(&root_path)),
        };

        let mut tree = FsTree { root: Some(root), expanded: HashSet::new(), selected: None };
        tree.expanded.insert(root_path);
        tree
    }

    /// Scan the children of the directory at `path` one level deep, if they
    /// haven't been loaded yet. Does nothing if the node is not found or already
    /// has children.
    pub fn expand(&mut self, path: &Path) {
        if let Some(ref mut root) = self.root {
            expand_node(root, path);
        }
    }

    /// Re-scan the root directory and re-expand any directories that were
    /// previously open, so the visible tree state is preserved after a
    /// file-system event.
    pub fn rescan(&mut self) {
        let root_path = match self.root.as_ref() {
            Some(r) => r.path.clone(),
            None    => return,
        };
        let old_expanded = self.expanded.clone();
        let old_selected = self.selected.clone();

        *self = FsTree::new(root_path);
        self.selected = old_selected;

        // Re-open every directory that was expanded before the rescan.
        // The root is already expanded by `new()`; skip it to avoid a redundant scan.
        let root_path = self.root.as_ref().map(|r| r.path.clone());
        for path in old_expanded {
            if root_path.as_deref() == Some(path.as_path()) { continue; }
            self.expanded.insert(path.clone());
            self.expand(&path);
        }
    }
}

// ── Private helpers ───────────────────────────────────────────────────────────

/// Scan one directory level and return an unsorted-then-sorted list of nodes.
/// Directories are returned with `children: None` (not yet loaded).
fn scan_one_level(path: &Path) -> Vec<FsNode> {
    let Ok(entries) = std::fs::read_dir(path) else { return Vec::new(); };

    let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
    entries.sort_by_key(|e| {
        let is_dir = e.path().is_dir();
        (!is_dir, e.file_name()) // dirs first, then alphabetical
    });

    entries
        .into_iter()
        .map(|entry| {
            let entry_path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            if entry_path.is_dir() {
                FsNode { name, path: entry_path, kind: FsNodeKind::Dir, children: None }
            } else {
                FsNode { name, path: entry_path, kind: FsNodeKind::File, children: Some(Vec::new()) }
            }
        })
        .collect()
}

/// Depth-first search for the node at `path`; populate its children if empty.
fn expand_node(node: &mut FsNode, path: &Path) -> bool {
    if node.path == path {
        if node.children.is_none() {
            node.children = Some(scan_one_level(&node.path));
        }
        return true;
    }
    if let Some(ref mut children) = node.children {
        for child in children.iter_mut() {
            if child.kind == FsNodeKind::Dir && expand_node(child, path) {
                return true;
            }
        }
    }
    false
}
