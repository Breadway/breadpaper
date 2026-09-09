mod config;
mod current;
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

/// Set wallpaper + global pywal palette on every live output.
pub fn set(path: &Path) -> Result<()> {
    let path = validate(path)?;
    apply_wallpaper(&path)?;
    generate_palette(&path)?;
    reload_theme()?;

    // A global set applies one palette to every output: the one `wal -i`
    // just wrote to `~/.cache/wal`. Reuse it rather than re-running an
    // isolated `wal -i` once per output for an identical result.
    let palette = bread_theme::load_palette();

    let mut cur = current::Current::load();

    // Prune only from a compositor-authoritative list; the awww fallback is
    // fine for *writing* per-output files but not for removal decisions.
    let connected = hypr_output_names().filter(|names| !names.is_empty());
    if let Some(ref names) = connected {
        prune_disconnected_outputs(&mut cur, names);
    }

    // Write per-output files for every currently connected output, not just
    // the ones already in current.json (a later-connected monitor would
    // otherwise never get a file).
    let targets: Vec<String> = match connected {
        Some(names) => names,
        None => {
            let fallback = live_outputs();
            if fallback.is_empty() {
                cur.all()
                    .keys()
                    .filter(|k| k.as_str() != "*")
                    .cloned()
                    .collect()
            } else {
                fallback
            }
        }
    };

    if targets.is_empty() {
        cur.set_output("*", path.clone());
    } else {
        for output in &targets {
            cur.set_output(output.clone(), path.clone());
            write_output_theme(output, &palette)?;
        }
    }
    cur.save()?;

    emit_changed(&path, None);
    Ok(())
}

/// Write `palettes/<output>.json` + `themes/<output>.css` for a palette that
/// is already known (no isolated `wal -i` re-run).
fn write_output_theme(output: &str, palette: &bread_theme::Palette) -> Result<()> {
    bread_theme::write_output_palette(output, palette)
        .with_context(|| format!("write per-output palette for {output}"))?;
    bread_theme::write_output_css(output, palette)
        .with_context(|| format!("write per-output css for {output}"))?;
    Ok(())
}

/// Persisted, non-wildcard outputs that are not in `connected`.
fn stale_outputs(cur: &current::Current, connected: &[String]) -> Vec<String> {
    cur.all()
        .keys()
        .filter(|k| k.as_str() != "*" && !connected.iter().any(|c| c == *k))
        .cloned()
        .collect()
}

/// Remove `current.json` entries and their `palettes/<out>.json` /
/// `themes/<out>.css` files for outputs not in `connected`. Never touches the
/// `"*"` wildcard entry.
fn prune_disconnected_outputs(cur: &mut current::Current, connected: &[String]) {
    for output in stale_outputs(cur, connected) {
        cur.remove_output(&output);
        for p in [
            bread_theme::output_palette_path(&output),
            bread_theme::output_css_path(&output),
        ] {
            match std::fs::remove_file(&p) {
                Ok(()) => {}
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                Err(e) => eprintln!("breadpaper: prune {}: {e}", p.display()),
            }
        }
    }
}

/// Set wallpaper + per-output theme on a single compositor output.
///
/// For a non-focused output this only writes that output's per-monitor
/// files. If `output` is the focused Hyprland monitor it additionally
/// syncs the session-global state (shared stylesheet + `~/.cache/wal`) from
/// that output's palette, so unbound apps and app-specific chrome match the
/// focused screen.
pub fn set_on(path: &Path, output: &str) -> Result<()> {
    if output.is_empty() {
        bail!("output name is empty");
    }
    let path = validate(path)?;
    wallpaper::apply_on(&path, output)?;
    let palette = theme::generate_for_output(output, &path)?;

    let mut cur = current::Current::load();
    cur.set_output(output, path.clone());
    cur.save()?;

    if is_focused_output(output) {
        sync_focused_global(&path, &palette)?;
    }

    emit_changed(&path, Some(output));
    Ok(())
}

/// The focused monitor defines the session-global look, so sync BOTH the
/// shared stylesheet AND pywal's global cache (`~/.cache/wal/colors.json`) to
/// this output's palette. Every app rebuilds its own CSS from `load_palette()`
/// (which reads that cache) when `theme.css` changes; without the cache sync
/// its app-specific chrome keeps the previous palette while shared-sheet
/// widgets move to the new one, a mixed palette. `set()` (global) already
/// keeps the two in lockstep; this makes a focused `set_on` consistent.
fn sync_focused_global(path: &Path, palette: &bread_theme::Palette) -> Result<()> {
    generate_palette(path)?;
    theme::write_shared_from(palette)?;
    Ok(())
}

/// Re-apply every wallpaper + per-output theme stored in current.json.
pub fn apply_saved() -> Result<()> {
    let cur = current::Current::load();
    let mut first_err = None;
    for (output, path) in cur.all() {
        let result = restore_one(output, path);
        match result {
            Ok(()) => {
                let name = (output != "*").then_some(output.as_str());
                emit_changed(path, name);
            }
            Err(e) => {
                eprintln!("breadpaper: apply {output}: {e:#}");
                if first_err.is_none() {
                    first_err = Some(e);
                }
            }
        }
    }
    match first_err {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

fn restore_one(output: &str, path: &Path) -> Result<()> {
    if output == "*" {
        wallpaper::apply(path)?;
        return Ok(());
    }
    wallpaper::apply_on(path, output)?;
    let palette = theme::generate_for_output(output, path)?;
    if is_focused_output(output) {
        sync_focused_global(path, &palette)?;
    }
    Ok(())
}

/// Honor `bread.command.paper.*` until killed. Subscribe reconnects with
/// backoff if breadd is down or restarts; this never errors the caller.
pub fn listen() -> Result<()> {
    let client = BreadClient::connect(APP_ID);
    let _subscription = client.subscribe("bread.command.paper.**", handle_command);
    let _monitors = client.subscribe("bread.monitor.connected", |_| {
        if let Err(e) = apply_saved() {
            eprintln!("breadpaper: apply_saved on monitor connect failed: {e:#}");
        }
    });
    loop {
        thread::park();
    }
}

/// Fire-and-forget `bread.paper.changed`. Silent no-op if breadd is down
/// (`BreadClient::emit` never blocks or errors the caller).
///
/// `output` is `None` when the wallpaper was applied to every output.
fn emit_changed(path: &Path, output: Option<&str>) {
    let mut data = json!({ "path": path.to_string_lossy() });
    if let Some(name) = output {
        data["output"] = json!(name);
    }
    BreadClient::connect(APP_ID).emit("bread.paper.changed", data);
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
    let output = data.get("output").and_then(Value::as_str);
    let path = Path::new(path_str);
    let result = match output {
        Some(name) => set_on(path, name),
        None => set(path),
    };
    match result {
        Ok(()) => {
            let applied = path
                .canonicalize()
                .map(|p| p.to_string_lossy().into_owned())
                .unwrap_or_else(|_| path_str.to_string());
            let mut payload = json!({ "path": applied });
            if let Some(name) = output {
                payload["output"] = json!(name);
            }
            client.emit("bread.paper.set.done", payload);
        }
        Err(e) => {
            eprintln!("breadpaper: bread.command.paper.set failed: {e:#}");
            let mut payload = json!({ "error": format!("{e:#}"), "path": path_str });
            if let Some(name) = output {
                payload["output"] = json!(name);
            }
            client.emit("bread.paper.set.failed", payload);
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
/// park() thread. GTK needs its own process (and argv); mixing it into
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

/// Wallpaper path last persisted for `output` in current.json.
pub fn get_on(output: &str) -> Result<PathBuf> {
    current::Current::load()
        .get_output(output)
        .map(Path::to_path_buf)
        .with_context(|| format!("no wallpaper saved for output {output}"))
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
            "unsupported file type '.{}'; expected one of: {}",
            ext,
            IMAGE_EXTENSIONS.join(", ")
        );
    }

    Ok(canonical)
}

fn live_outputs() -> Vec<String> {
    if let Some(names) = hypr_output_names() {
        return names;
    }
    wallpaper::query_outputs()
}

fn hypr_output_names() -> Option<Vec<String>> {
    let v = bread_utils::hypr::request_json("j/monitors")?;
    let names: Vec<String> = v
        .as_array()?
        .iter()
        .filter_map(|m| m.get("name").and_then(|n| n.as_str()).map(str::to_string))
        .collect();
    if names.is_empty() { None } else { Some(names) }
}

fn is_focused_output(output: &str) -> bool {
    bread_utils::hypr::focused_monitor()
        .map(|m| m.name == output)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "breadpaper-lib-{name}-{}-{}",
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
    fn emit_changed_is_silent_without_breadd() {
        // BreadClient::emit must never panic or error just because the
        // socket is missing; this is the fail-silent contract.
        emit_changed(Path::new("/tmp/wallpaper.png"), None);
        emit_changed(Path::new("/tmp/wallpaper.png"), Some("eDP-1"));
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
    fn handle_set_with_output_vs_without_is_silent_without_breadd() {
        // Missing files fail in validate; never reaches awww/wal.
        handle_set(&json!({ "path": "/no/such/breadpaper-wallpaper.png" }));
        handle_set(&json!({
            "path": "/no/such/breadpaper-wallpaper.png",
            "output": "eDP-1"
        }));
    }

    #[test]
    fn handle_command_library_is_silent_without_breadd() {
        handle_command(BreadEvent {
            event: "bread.command.paper.library".into(),
            timestamp: 0,
            data: json!({}),
        });
    }

    #[test]
    fn validate_rejects_bad_extensions() {
        let dir = tmp_dir("validate");
        let txt = dir.join("notes.txt");
        std::fs::write(&txt, b"x").unwrap();
        assert!(validate(&txt).is_err());

        let png = dir.join("ok.png");
        std::fs::write(&png, b"x").unwrap();
        assert_eq!(validate(&png).unwrap(), png.canonicalize().unwrap());
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn stale_outputs_selects_absent_non_wildcard_entries_only() {
        let mut cur = current::Current::default();
        cur.set_output("*", "/abs/global.png");
        cur.set_output("mon-a", "/abs/a.png");
        cur.set_output("mon-b", "/abs/b.png");

        // mon-b disconnected; "*" and the still-connected mon-a are kept.
        let stale = stale_outputs(&cur, &["mon-a".to_string()]);
        assert_eq!(stale, vec!["mon-b".to_string()]);
    }

    #[test]
    fn stale_outputs_is_empty_when_all_connected() {
        let mut cur = current::Current::default();
        cur.set_output("mon-a", "/abs/a.png");
        cur.set_output("mon-b", "/abs/b.png");
        assert!(stale_outputs(&cur, &["mon-a".to_string(), "mon-b".to_string()]).is_empty());
    }

    #[test]
    fn handle_set_bad_extension_with_output_is_silent_without_breadd() {
        let dir = tmp_dir("bad-ext");
        let txt = dir.join("notes.txt");
        std::fs::write(&txt, b"x").unwrap();
        handle_set(&json!({
            "path": txt.to_string_lossy(),
            "output": "HDMI-A-1"
        }));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
