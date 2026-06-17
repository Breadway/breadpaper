use std::process::Command;

use anyhow::{Context, Result, bail};

pub fn reload() -> Result<()> {
    let status = Command::new("bread-theme")
        .arg("reload")
        .status()
        .context("failed to run bread-theme — is it installed?")?;

    if !status.success() {
        bail!("bread-theme reload exited with {}", status);
    }
    Ok(())
}
