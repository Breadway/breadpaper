use std::path::Path;
use std::time::Duration;

use anyhow::{Result, bail};

pub fn apply(path: &Path) -> Result<()> {
    // Was a bare Command::new("awww").status() with no timeout — a wedged
    // awww-daemon (compositor not ready yet, IPC socket stuck) used to be
    // able to hang this call indefinitely.
    let out = bread_utils::proc::run("awww", &["img", &path.to_string_lossy()], Duration::from_secs(10));
    if !out.success {
        bail!("awww img failed (is awww-daemon running?): {}", out.stderr.trim());
    }
    Ok(())
}
