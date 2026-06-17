use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

pub fn generate(path: &Path) -> Result<()> {
    let status = Command::new("wal")
        .arg("-i")
        .arg(path)
        .arg("-n") // skip wal's own wallpaper backend; awww already set it
        .status()
        .context("failed to run wal — is python-pywal installed?")?;

    if !status.success() {
        bail!("wal exited with {}", status);
    }
    Ok(())
}
