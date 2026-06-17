use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

pub fn apply(path: &Path) -> Result<()> {
    let status = Command::new("awww")
        .arg("set")
        .arg(path)
        .status()
        .context("failed to run awww — is awww-daemon running?")?;

    if !status.success() {
        bail!("awww set exited with {}", status);
    }
    Ok(())
}
