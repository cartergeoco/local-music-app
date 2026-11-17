use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub library_root: PathBuf,
}

/// Loads `settings.json` from `dir`, creating it with `default_root` on first run.
pub fn load(dir: &Path, default_root: PathBuf) -> Settings {
    let path = dir.join("settings.json");
    if let Some(s) = std::fs::read(&path).ok().and_then(|b| serde_json::from_slice(&b).ok()) {
        return s;
    }
    let settings = Settings { library_root: default_root };
    if std::fs::create_dir_all(dir).is_ok() {
        if let Ok(json) = serde_json::to_vec_pretty(&settings) {
            std::fs::write(&path, json).ok();
        }
    }
    settings
}
