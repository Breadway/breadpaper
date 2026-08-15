use std::path::{Path, PathBuf};

use serde::Deserialize;

/// User library directory used when no config file is present.
pub const DEFAULT_USER_LIBRARY: &str = "Pictures/Wallpapers";

/// Packaged BOS backgrounds, scanned when the directory exists.
pub const DEFAULT_SYSTEM_LIBRARY: &str = "/usr/share/backgrounds/bos";

/// Colon-separated override of [`Config::library_dirs`]. Empty means "use
/// the config file / defaults".
pub const LIBRARY_DIRS_ENV: &str = "BREADPAPER_LIBRARY_DIRS";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub library_dirs: Vec<PathBuf>,
}

#[derive(Debug, Default, Deserialize)]
struct ConfigFile {
    #[serde(default)]
    library_dirs: Vec<PathBuf>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            library_dirs: default_library_dirs(),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        bread_utils::xdg::config_dir("breadpaper").join("config.toml")
    }

    pub fn load() -> Self {
        Self::load_from(&Self::path())
    }

    pub fn load_from(path: &Path) -> Self {
        let mut cfg = match std::fs::read_to_string(path) {
            Ok(text) => match toml::from_str::<ConfigFile>(&text) {
                Ok(parsed) if !parsed.library_dirs.is_empty() => Self {
                    library_dirs: parsed.library_dirs,
                },
                Ok(_) => Self::default(),
                Err(e) => {
                    eprintln!(
                        "breadpaper: {} failed to parse ({e}); using defaults",
                        path.display()
                    );
                    Self::default()
                }
            },
            Err(_) => Self::default(),
        };

        if let Some(dirs) = env_library_dirs() {
            cfg.library_dirs = dirs;
        }

        cfg.library_dirs = cfg
            .library_dirs
            .into_iter()
            .map(expand_tilde)
            .filter(|p| !p.as_os_str().is_empty())
            .collect();
        cfg
    }

    pub fn with_extra_dirs(mut self, extra: impl IntoIterator<Item = PathBuf>) -> Self {
        self.library_dirs
            .extend(extra.into_iter().map(expand_tilde));
        self
    }
}

pub fn default_library_dirs() -> Vec<PathBuf> {
    vec![
        bread_utils::xdg::home_dir().join(DEFAULT_USER_LIBRARY),
        PathBuf::from(DEFAULT_SYSTEM_LIBRARY),
    ]
}

pub fn expand_tilde(path: PathBuf) -> PathBuf {
    let Some(s) = path.to_str() else {
        return path;
    };
    if s == "~" {
        return bread_utils::xdg::home_dir();
    }
    if let Some(rest) = s.strip_prefix("~/") {
        return bread_utils::xdg::home_dir().join(rest);
    }
    path
}

fn env_library_dirs() -> Option<Vec<PathBuf>> {
    let raw = std::env::var(LIBRARY_DIRS_ENV).ok()?;
    if raw.is_empty() {
        return None;
    }
    let dirs: Vec<PathBuf> = raw
        .split(':')
        .filter(|s| !s.is_empty())
        .map(PathBuf::from)
        .collect();
    if dirs.is_empty() { None } else { Some(dirs) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Mutex, OnceLock};

    fn env_lock() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
            .lock()
            .unwrap_or_else(|e| e.into_inner())
    }

    fn tmp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "breadpaper-config-{name}-{}-{}",
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
    fn default_dirs_are_pictures_wallpapers_and_bos_backgrounds() {
        let dirs = Config::default().library_dirs;
        assert!(
            dirs.iter().any(|d| d.ends_with(DEFAULT_USER_LIBRARY)),
            "missing ~/{DEFAULT_USER_LIBRARY} in {dirs:?}"
        );
        assert!(
            dirs.iter().any(|d| d == Path::new(DEFAULT_SYSTEM_LIBRARY)),
            "missing {DEFAULT_SYSTEM_LIBRARY} in {dirs:?}"
        );
    }

    #[test]
    fn expand_tilde_prefix() {
        let home = bread_utils::xdg::home_dir();
        assert_eq!(
            expand_tilde(PathBuf::from("~/Pictures/Wallpapers")),
            home.join("Pictures/Wallpapers")
        );
        assert_eq!(expand_tilde(PathBuf::from("~")), home);
        let abs = PathBuf::from("/usr/share/backgrounds/bos");
        assert_eq!(expand_tilde(abs.clone()), abs);
    }

    #[test]
    fn load_from_missing_file_uses_defaults() {
        let _lock = env_lock();
        let prev = std::env::var_os(LIBRARY_DIRS_ENV);
        unsafe { std::env::remove_var(LIBRARY_DIRS_ENV) };
        let cfg = Config::load_from(&PathBuf::from("/no/such/breadpaper-config.toml"));
        if let Some(v) = prev {
            unsafe { std::env::set_var(LIBRARY_DIRS_ENV, v) };
        }
        assert_eq!(cfg.library_dirs, default_library_dirs());
    }

    #[test]
    fn load_from_parses_library_dirs_and_expands_tilde() {
        let _lock = env_lock();
        let prev = std::env::var_os(LIBRARY_DIRS_ENV);
        unsafe { std::env::remove_var(LIBRARY_DIRS_ENV) };

        let dir = tmp_dir("parse");
        let path = dir.join("config.toml");
        std::fs::write(
            &path,
            "library_dirs = [\"~/custom/walls\", \"/opt/walls\"]\n",
        )
        .unwrap();
        let cfg = Config::load_from(&path);
        if let Some(v) = prev {
            unsafe { std::env::set_var(LIBRARY_DIRS_ENV, v) };
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            cfg.library_dirs,
            vec![
                bread_utils::xdg::home_dir().join("custom/walls"),
                PathBuf::from("/opt/walls"),
            ]
        );
    }

    #[test]
    fn empty_library_dirs_key_falls_back_to_defaults() {
        let _lock = env_lock();
        let prev = std::env::var_os(LIBRARY_DIRS_ENV);
        unsafe { std::env::remove_var(LIBRARY_DIRS_ENV) };

        let dir = tmp_dir("empty");
        let path = dir.join("config.toml");
        std::fs::write(&path, "library_dirs = []\n").unwrap();
        let cfg = Config::load_from(&path);
        if let Some(v) = prev {
            unsafe { std::env::set_var(LIBRARY_DIRS_ENV, v) };
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(cfg.library_dirs, default_library_dirs());
    }

    #[test]
    fn env_overrides_config_file() {
        let _lock = env_lock();
        let prev = std::env::var_os(LIBRARY_DIRS_ENV);
        unsafe { std::env::set_var(LIBRARY_DIRS_ENV, "/tmp/a:/tmp/b") };

        let dir = tmp_dir("env");
        let path = dir.join("config.toml");
        std::fs::write(&path, "library_dirs = [\"/from/file\"]\n").unwrap();
        let cfg = Config::load_from(&path);
        match prev {
            Some(v) => unsafe { std::env::set_var(LIBRARY_DIRS_ENV, v) },
            None => unsafe { std::env::remove_var(LIBRARY_DIRS_ENV) },
        }
        let _ = std::fs::remove_dir_all(&dir);
        assert_eq!(
            cfg.library_dirs,
            vec![PathBuf::from("/tmp/a"), PathBuf::from("/tmp/b")]
        );
    }

    #[test]
    fn with_extra_dirs_appends() {
        let cfg = Config {
            library_dirs: vec![PathBuf::from("/a")],
        }
        .with_extra_dirs([PathBuf::from("/b")]);
        assert_eq!(
            cfg.library_dirs,
            vec![PathBuf::from("/a"), PathBuf::from("/b")]
        );
    }
}
