use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};
use bread_theme::Palette;

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

/// Per-output palette + bread-theme files. Does not run `wal -i`.
pub fn generate_for_output(output: &str, path: &Path) -> Result<Palette> {
    bread_theme::generate_output(output, path)
        .with_context(|| format!("bread-theme generate_output({output}, {})", path.display()))?;
    Ok(bread_theme::load_palette_for(output))
}

pub fn write_shared_from(palette: &Palette) -> Result<()> {
    bread_theme::write_shared_css_from(palette)
        .context("bread-theme write_shared_css_from")
        .map(|_| ())
}
