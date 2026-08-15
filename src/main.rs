use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(
    name = "breadpaper",
    version,
    about = "Wallpaper manager for the bread desktop"
)]
struct Cli {
    /// Image file to set as wallpaper (shorthand for `set`)
    path: Option<PathBuf>,

    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Set wallpaper, generate pywal palette, and reload bread themes
    Set { path: PathBuf },
    /// Print the current wallpaper path
    Get,
    /// Honor bread.command.paper.set / .library until killed
    Listen,
    /// Open the wallpaper library (alias: browse)
    #[command(visible_alias = "browse")]
    Library {
        /// Extra directory to scan (repeatable; added to configured dirs)
        #[arg(short, long = "dir", value_name = "DIR")]
        dirs: Vec<PathBuf>,
    },
}

fn main() {
    let cli = Cli::parse();

    let result = match (cli.command, cli.path) {
        (Some(Command::Set { path }), _) | (None, Some(path)) => breadpaper::set(&path),
        (Some(Command::Listen), _) => breadpaper::listen(),
        (Some(Command::Library { dirs }), _) => breadpaper::library(dirs),
        (Some(Command::Get), _) | (None, None) => {
            breadpaper::get().map(|p| println!("{}", p.display()))
        }
    };

    if let Err(e) = result {
        eprintln!("error: {e:#}");
        process::exit(1);
    }
}
