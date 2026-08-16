use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

/// Persisted wallpaper path per output (`~/.config/breadpaper/current.json`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Current {
    #[serde(default)]
    outputs: BTreeMap<String, PathBuf>,
}

impl Current {
    pub fn path() -> PathBuf {
        bread_utils::xdg::config_dir("breadpaper").join("current.json")
    }

    pub fn load() -> Self {
        Self::load_from(&Self::path())
    }

    /// Missing or unreadable file => empty map.
    pub fn load_from(path: &Path) -> Self {
        let Ok(text) = std::fs::read_to_string(path) else {
            return Self::default();
        };
        serde_json::from_str(&text).unwrap_or_default()
    }

    pub fn save(&self) -> Result<()> {
        self.save_to(&Self::path())
    }

    pub fn save_to(&self, path: &Path) -> Result<()> {
        let text = serde_json::to_string_pretty(self).context("serialize current.json")?;
        let text = format!("{text}\n");
        bread_utils::atomic::write_atomic(path, &text, None)
            .with_context(|| format!("write {}", path.display()))
    }

    pub fn set_output(&mut self, output: impl Into<String>, path: impl Into<PathBuf>) {
        self.outputs.insert(output.into(), path.into());
    }

    pub fn get_output(&self, output: &str) -> Option<&Path> {
        self.outputs.get(output).map(PathBuf::as_path)
    }

    pub fn all(&self) -> &BTreeMap<String, PathBuf> {
        &self.outputs
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "breadpaper-current-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn missing_file_is_empty_map() {
        let dir = tmp_dir("missing");
        let path = dir.join("current.json");
        let cur = Current::load_from(&path);
        assert!(cur.all().is_empty());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn roundtrip_pretty_json() {
        let dir = tmp_dir("roundtrip");
        let path = dir.join("nested").join("current.json");
        let mut cur = Current::default();
        cur.set_output("eDP-1", "/abs/path/a.png");
        cur.set_output("HDMI-A-1", "/abs/path/b.png");
        cur.save_to(&path).unwrap();

        let text = std::fs::read_to_string(&path).unwrap();
        assert!(text.contains("\n  \"outputs\""));
        assert!(text.contains("\n    \"HDMI-A-1\""));
        assert!(text.contains("\n    \"eDP-1\""));

        let loaded = Current::load_from(&path);
        assert_eq!(
            loaded.get_output("eDP-1"),
            Some(Path::new("/abs/path/a.png"))
        );
        assert_eq!(
            loaded.get_output("HDMI-A-1"),
            Some(Path::new("/abs/path/b.png"))
        );
        assert_eq!(loaded.all().len(), 2);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn set_one_output_does_not_drop_others() {
        let dir = tmp_dir("keep");
        let path = dir.join("current.json");
        let mut cur = Current::default();
        cur.set_output("eDP-1", "/abs/a.png");
        cur.set_output("HDMI-A-1", "/abs/b.png");
        cur.save_to(&path).unwrap();

        let mut cur = Current::load_from(&path);
        cur.set_output("eDP-1", "/abs/c.png");
        cur.save_to(&path).unwrap();

        let loaded = Current::load_from(&path);
        assert_eq!(loaded.get_output("eDP-1"), Some(Path::new("/abs/c.png")));
        assert_eq!(loaded.get_output("HDMI-A-1"), Some(Path::new("/abs/b.png")));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
