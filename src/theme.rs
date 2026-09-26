use std::path::Path;

use anyhow::{Context, Result};
use bread_theme::Palette;

/// Per-output palette + bread-theme files. Does not touch the global palette.
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
