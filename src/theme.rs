use std::time::Duration;

use anyhow::{Result, bail};

pub fn reload() -> Result<()> {
    // Was a bare Command::new("bread-theme").status() with no timeout.
    let out = bread_utils::proc::run("bread-theme", &["reload"], Duration::from_secs(5));
    if !out.success {
        bail!("bread-theme reload failed (is it installed?): {}", out.stderr.trim());
    }
    Ok(())
}
