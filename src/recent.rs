//! Recently used plans, kept in the user's settings folder (shown on File > Home and Open).

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecentFile {
    pub path: String,
    /// "YYYY-MM-DD HH:MM", local time.
    pub opened: String,
    #[serde(default)]
    pub pinned: bool,
}

fn store() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("PLANCRAFT_RECENT") {
        return Some(PathBuf::from(p));
    }
    let base = std::env::var_os("APPDATA").or_else(|| std::env::var_os("XDG_CONFIG_HOME")).map(PathBuf::from).or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(base.join(crate::APP_NAME).join("recent.json"))
}

pub fn list() -> Vec<RecentFile> {
    let Some(p) = store() else { return vec![] };
    std::fs::read_to_string(p).ok().and_then(|t| serde_json::from_str(&t).ok()).unwrap_or_default()
}

fn save(items: &[RecentFile]) {
    if let Some(p) = store() {
        if let Some(dir) = p.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(p, serde_json::to_string_pretty(items).unwrap_or_default());
    }
}

/// Put a plan at the top of the list (keeps its pin).
pub fn add(path: &Path) {
    let full = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    let mut s = full.to_string_lossy().to_string();
    if let Some(rest) = s.strip_prefix(r"\\?\") {
        s = rest.to_string();
    }
    let mut items = list();
    let pinned = items.iter().any(|r| r.path == s && r.pinned);
    items.retain(|r| r.path != s);
    items.insert(0, RecentFile { path: s, opened: chrono::Local::now().format("%Y-%m-%d %H:%M").to_string(), pinned });
    items.truncate(30);
    save(&items);
}

pub fn set_pinned(path: &str, pinned: bool) -> bool {
    let mut items = list();
    let mut found = false;
    for r in items.iter_mut().filter(|r| r.path == path) {
        r.pinned = pinned;
        found = true;
    }
    save(&items);
    found
}

pub fn remove(path: &str) -> bool {
    let mut items = list();
    let n = items.len();
    items.retain(|r| r.path != path);
    save(&items);
    items.len() != n
}
