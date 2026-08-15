mod config;
mod library;
mod pywal;
mod theme;
mod ui;
mod wallpaper;

use std::path::{Path, PathBuf};
#[cfg(not(test))]
use std::process::{Command, Stdio};
use std::thread;

use anyhow::{Context, Result, bail};
use bread_utils::bread_client::{BreadClient, BreadEvent};
use serde_json::{Value, json};

pub use config::{Config, DEFAULT_SYSTEM_LIBRARY, DEFAULT_USER_LIBRARY};
pub use library::{Wallpaper, scan};

/// App id in bread's sibling-app registry (`KNOWN_APPS`). Events publish as
/// `bread.paper.*`. See `EVENTS.md`.
const APP_ID: &str = "paper";

const IMAGE_EXTENSIONS: &[&str] = &["png", "jpg", "jpeg", "webp", "gif", "bmp"];

/// Open the GTK wallpaper library. Extra dirs are appended to the configured
/// scan list (`~/.config/breadpaper/config.toml`, then defaults).
pub fn library(extra_dirs: impl IntoIterator<Item = PathBuf>) -> Result<()> {
    let cfg = Config::load().with_extra_dirs(extra_dirs);
    ui::run(cfg.library_dirs)
}

pub fn set(path: &Path) -> Result<()> {
    let path = validate(path)?;
    apply_wallpaper(&path)?;
    generate_palette(&path)?;
    reload_theme()?;
    emit_changed(&path);
    Ok(())
}

/// Honor `bread.command.paper.*` until killed. Subscribe reconnects with
/// backoff if breadd is down or restarts — this never errors the caller.
pub fn listen() -> Result<()> {
    let client = BreadClient::connect(APP_ID);
    let _subscription = client.subscribe("bread.command.paper.**", handle_command);
    loop {
        thread::park();
    }
}

/// Fire-and-forget `bread.paper.changed`. Silent no-op if breadd is down
/// (`BreadClient::emit` never blocks or errors the caller).
fn emit_changed(path: &Path) {
    BreadClient::connect(APP_ID).emit(
        "bread.paper.changed",
        json!({ "path": path.to_string_lossy() }),
    );
}

fn handle_command(event: BreadEvent) {
    let Some(verb) = event.event.strip_prefix("bread.command.paper.") else {
        return;
    };
    match verb {
        "set" => handle_set(&event.data),
        "library" => handle_library(),
        other => {
            eprintln!("breadpaper: ignoring unrecognized command verb '{other}'");
        }
    }
}

fn handle_set(data: &Value) {
    let client = BreadClient::connect(APP_ID);
    let Some(path_str) = data.get("path").and_then(Value::as_str) else {
        client.emit(
            "bread.paper.set.failed",
            json!({ "error": "missing string \"path\"" }),
        );
        return;
    };
    let path = Path::new(path_str);
    match set(path) {
        Ok(()) => {
            let applied = path
                .canonicalize()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| path_str.to_string());
            client.emit("bread.paper.set.done", json!({ "path": applied }));
        }
        Err(e) => {
            eprintln!("breadpaper: bread.command.paper.set failed: {e:#}");
            client.emit(
                "bread.paper.set.failed",
                json!({ "error": format!("{e:#}"), "path": path_str }),
            );
        }
    }
}

fn handle_library() {
    let client = BreadClient::connect(APP_ID);
    match open_library() {
        Ok(()) => client.emit("bread.paper.library.done", json!({})),
        Err(e) => {
            eprintln!("breadpaper: bread.command.paper.library failed: {e:#}");
            client.emit(
                "bread.paper.library.failed",
                json!({ "error": format!("{e:#}") }),
            );
        }
    }
}

/// Spawn a one-shot `breadpaper library` so the listen loop can stay a
/// park() thread. GTK needs its own process (and argv) — mixing it into
/// `listen` would steal the main thread.
fn open_library() -> Result<()> {
    spawn_library()
}

#[cfg(not(test))]
fn spawn_library() -> Result<()> {
    let exe = std::env::current_exe().context("cannot resolve breadpaper executable")?;
    Command::new(exe)
        .arg("library")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .context("failed to spawn breadpaper library")?;
    Ok(())
}

#[cfg(test)]
fn spawn_library() -> Result<()> {
    // cargo test's current_exe is the test harness, not breadpaper.
    Ok(())
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

    #[test]
    fn subscribe_is_silent_without_breadd() {
        let client = BreadClient::connect(APP_ID);
        let sub = client.subscribe("bread.command.paper.**", |_| {});
        drop(sub);
    }

    #[test]
    fn handle_command_ignores_unrecognized_verb() {
        handle_command(BreadEvent {
            event: "bread.command.paper.next".into(),
            timestamp: 0,
            data: json!({}),
        });
    }

    #[test]
    fn handle_command_ignores_events_outside_its_own_command_namespace() {
        handle_command(BreadEvent {
            event: "bread.command.clip.clear".into(),
            timestamp: 0,
            data: json!({}),
        });
        handle_command(BreadEvent {
            event: "bread.paper.changed".into(),
            timestamp: 0,
            data: json!({ "path": "/tmp/wallpaper.png" }),
        });
    }

    #[test]
    fn handle_set_missing_path_is_silent_without_breadd() {
        handle_set(&json!({}));
        handle_set(&json!({ "path": 1 }));
    }

    #[test]
    fn handle_command_library_is_silent_without_breadd() {
        handle_command(BreadEvent {
            event: "bread.command.paper.library".into(),
            timestamp: 0,
            data: json!({}),
        });
    }
}
