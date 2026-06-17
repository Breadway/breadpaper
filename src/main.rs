use std::path::PathBuf;
use std::process;

use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "breadpaper", about = "Wallpaper manager for the bread desktop")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Set wallpaper, generate pywal palette, and reload bread themes
    Set {
        /// Path to the image file
        path: PathBuf,
    },
    /// Print the current wallpaper path
    Get,
}

fn main() {
    let cli = Cli::parse();

    let result = match cli.command {
        Command::Set { path } => breadpaper::set(&path),
        Command::Get => breadpaper::get().map(|p| println!("{}", p.display())),
    };

    if let Err(e) = result {
        eprintln!("error: {e:#}");
        process::exit(1);
    }
}
