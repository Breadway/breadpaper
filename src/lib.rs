mod pywal;
mod theme;
mod wallpaper;

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use bread_utils::bread_client::BreadClient;

/// App id in bread's sibling-app registry (`KNOWN_APPS`). Events publish as
/// `bread.paper.*`. See `EVENTS.md`.
const APP_ID: &str = "paper";

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp"];

pub fn set(path: &Path) -> Result<()> {
    let path = validate(path)?;
    apply_wallpaper(&path)?;
    generate_palette(&path)?;
    reload_theme()?;
    emit_changed(&path);
    Ok(())
}

/// Fire-and-forget `bread.paper.changed`. Silent no-op if breadd is down
/// (`BreadClient::emit` never blocks or errors the caller).
fn emit_changed(path: &Path) {
    BreadClient::connect(APP_ID).emit(
        "bread.paper.changed",
        serde_json::json!({ "path": path.to_string_lossy() }),
    );
}

pub fn get() -> Result<PathBuf> {
    let home = std::env::var("HOME").context("HOME not set")?;
    let wal_file = PathBuf::from(home).join(".cache/wal/wal");

    let contents = std::fs::read_to_string(&wal_file)
        .with_context(|| format!("no wallpaper set yet ({})", wal_file.display()))?;

    Ok(PathBuf::from(contents.trim()))
}

pub fn apply_wallpaper(path: &Path) -> Result<()> {
    wallpaper::apply(path)
}

pub fn generate_palette(path: &Path) -> Result<()> {
    pywal::generate(path)
}

pub fn reload_theme() -> Result<()> {
    theme::reload()
}

fn validate(path: &Path) -> Result<PathBuf> {
    let canonical = path
        .canonicalize()
        .with_context(|| format!("not found: {}", path.display()))?;

    let ext = canonical
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();

    if !IMAGE_EXTENSIONS.contains(&ext.as_str()) {
        bail!(
            "unsupported file type '.{}' — expected one of: {}",
            ext,
            IMAGE_EXTENSIONS.join(", ")
        );
    }

    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn emit_changed_is_silent_without_breadd() {
        // BreadClient::emit must never panic or error just because the
        // socket is missing — this is the fail-silent contract.
        emit_changed(Path::new("/tmp/wallpaper.png"));
    }
}
