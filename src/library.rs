use std::collections::HashSet;
use std::path::{Path, PathBuf};

use crate::IMAGE_EXTENSIONS;

/// Caps how many files the picker ever lists. The library is organized in
/// subfolders (show/series), so the walk is recursive — without a bound a
/// huge Pictures tree would stall the window.
pub const MAX_LIBRARY_ITEMS: usize = 200;
pub const MAX_SCAN_DEPTH: usize = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Wallpaper {
    pub path: PathBuf,
    pub name: String,
}

pub fn is_wallpaper_file(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| {
            IMAGE_EXTENSIONS
                .iter()
                .any(|ext| ext.eq_ignore_ascii_case(e))
        })
        .unwrap_or(false)
}

/// Recursively collect images under `dirs`. Missing directories are skipped.
/// Results are sorted by filename (case-insensitive), then full path.
pub fn scan(dirs: &[PathBuf]) -> Vec<Wallpaper> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for dir in dirs {
        if !dir.is_dir() {
            continue;
        }
        walk(dir, MAX_SCAN_DEPTH, &mut out, &mut seen);
        if out.len() >= MAX_LIBRARY_ITEMS {
            break;
        }
    }
    out.sort_by(|a, b| {
        a.name
            .to_lowercase()
            .cmp(&b.name.to_lowercase())
            .then_with(|| a.path.cmp(&b.path))
    });
    out
}

fn walk(dir: &Path, depth: usize, out: &mut Vec<Wallpaper>, seen: &mut HashSet<PathBuf>) {
    if depth == 0 || out.len() >= MAX_LIBRARY_ITEMS {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        if out.len() >= MAX_LIBRARY_ITEMS {
            return;
        }
        let path = entry.path();
        let name = entry.file_name();
        if name.to_string_lossy().starts_with('.') {
            continue;
        }
        if path.is_dir() {
            walk(&path, depth - 1, out, seen);
            continue;
        }
        if !is_wallpaper_file(&path) {
            continue;
        }
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        if !seen.insert(canonical.clone()) {
            continue;
        }
        out.push(Wallpaper {
            name: path
                .file_stem()
                .or_else(|| path.file_name())
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_default(),
            path: canonical,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "breadpaper-scan-{name}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).unwrap();
        }
        std::fs::write(path, []).unwrap();
    }

    #[test]
    fn is_wallpaper_file_accepts_known_extensions() {
        assert!(is_wallpaper_file(Path::new("a.PNG")));
        assert!(is_wallpaper_file(Path::new("b.jpeg")));
        assert!(is_wallpaper_file(Path::new("c.webp")));
        assert!(!is_wallpaper_file(Path::new("d.txt")));
        assert!(!is_wallpaper_file(Path::new("noext")));
    }

    #[test]
    fn scan_skips_missing_dirs() {
        assert!(scan(&[PathBuf::from("/no/such/breadpaper-walls")]).is_empty());
    }

    #[test]
    fn scan_finds_images_and_ignores_other_files() {
        let dir = tmp_dir("find");
        touch(&dir.join("keep.png"));
        touch(&dir.join("notes.txt"));
        touch(&dir.join(".hidden.jpg"));
        touch(&dir.join("nested").join("deep.jpg"));
        let found = scan(std::slice::from_ref(&dir));
        let names: Vec<_> = found.iter().map(|w| w.name.as_str()).collect();
        assert!(names.contains(&"keep"), "{names:?}");
        assert!(names.contains(&"deep"), "{names:?}");
        assert!(
            !names
                .iter()
                .any(|n| n.contains("notes") || n.contains("hidden"))
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_dedups_the_same_file_via_two_roots() {
        let dir = tmp_dir("dedup");
        touch(&dir.join("one.png"));
        let found = scan(&[dir.clone(), dir.clone()]);
        assert_eq!(found.len(), 1);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn scan_respects_item_cap() {
        let dir = tmp_dir("cap");
        for i in 0..(MAX_LIBRARY_ITEMS + 10) {
            touch(&dir.join(format!("{i:04}.png")));
        }
        let found = scan(std::slice::from_ref(&dir));
        assert_eq!(found.len(), MAX_LIBRARY_ITEMS);
        let _ = std::fs::remove_dir_all(&dir);
    }
}
