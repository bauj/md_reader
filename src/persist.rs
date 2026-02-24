use std::path::PathBuf;
use serde::{Deserialize, Serialize};

/// Everything we want to survive across restarts.
#[derive(Serialize, Deserialize, Default)]
pub struct AppState {
    /// Root directories shown in the sidebar (one per open folder).
    pub root_dirs: Vec<PathBuf>,
    /// Ordered list of open tab paths (only those that still exist on disk are restored).
    pub open_tabs: Vec<PathBuf>,
    /// Index into `open_tabs` for the active tab.
    pub active_tab: Option<usize>,
    /// Last used view mode: "preview" | "edit" | "split".
    pub view_mode: String,
    /// Most recently opened files, newest first. Capped at 20 entries.
    pub recent_files: Vec<PathBuf>,
}

/// Returns the platform config path, e.g. `~/.config/md_reader/state.json` on Linux.
fn state_path() -> Option<PathBuf> {
    dirs::config_dir().map(|d| d.join("md_reader").join("state.json"))
}

pub fn load() -> AppState {
    let path = match state_path() {
        Some(p) => p,
        None    => return AppState::default(),
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(t)  => t,
        Err(_) => return AppState::default(),
    };
    serde_json::from_str(&text).unwrap_or_default()
}

pub fn save(state: &AppState) {
    let path = match state_path() {
        Some(p) => p,
        None    => return,
    };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(state) {
        let _ = std::fs::write(&path, json);
    }
}
