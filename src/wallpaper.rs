use std::path::Path;
use std::process::Command;

use anyhow::{Context, Result, bail};

pub fn apply(path: &Path) -> Result<()> {
    run_awww(Command::new("awww").arg("img").arg(path))
}

pub fn apply_on(path: &Path, output: &str) -> Result<()> {
    run_awww(
        Command::new("awww")
            .arg("img")
            .arg(path)
            .arg("--outputs")
            .arg(output),
    )
}

/// Output names from `awww query`. Empty if the daemon isn't running.
pub fn query_outputs() -> Vec<String> {
    let Ok(out) = Command::new("awww").arg("query").output() else {
        return Vec::new();
    };
    if !out.status.success() {
        return Vec::new();
    }
    parse_awww_query(&String::from_utf8_lossy(&out.stdout))
}

fn run_awww(cmd: &mut Command) -> Result<()> {
    let status = cmd
        .status()
        .context("failed to run awww — is awww-daemon running?")?;
    if !status.success() {
        bail!("awww img exited with {}", status);
    }
    Ok(())
}

fn parse_awww_query(stdout: &str) -> Vec<String> {
    stdout.lines().filter_map(parse_awww_query_line).collect()
}

/// `awww query` lines look like `: eDP-1: 1920x1080, scale: 1, ...`.
fn parse_awww_query_line(line: &str) -> Option<String> {
    let rest = line.trim().strip_prefix(':').unwrap_or(line).trim();
    let name = rest.split(':').next()?.trim();
    if name.is_empty() {
        None
    } else {
        Some(name.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_awww_query_names() {
        let sample = "\
: eDP-1: 1920x1080, scale: 1, currently displaying: image: /a.png
: HDMI-A-1: 2560x1440, scale: 1, currently displaying: image: /b.png
";
        assert_eq!(
            parse_awww_query(sample),
            vec!["eDP-1".to_string(), "HDMI-A-1".to_string()]
        );
    }

    #[test]
    fn parse_awww_query_skips_blank() {
        assert!(parse_awww_query("\n  \n").is_empty());
    }
}
