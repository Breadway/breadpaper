use std::path::Path;
use std::time::Duration;

use anyhow::{Result, bail};

pub fn generate(path: &Path) -> Result<()> {
    // Was a bare Command::new("wal").status() with no timeout — pywal can
    // hang on a corrupt/huge image; this used to be able to wedge the
    // caller indefinitely.
    let out = bread_utils::proc::run(
        "wal",
        &["-i", &path.to_string_lossy(), "-n"], // -n: skip wal's own wallpaper backend; awww already set it
        Duration::from_secs(30),
    );
    if !out.success {
        bail!("wal failed (is python-pywal installed?): {}", out.stderr.trim());
    }
    Ok(())
}
