//! An extremely simple lofi player.

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

use clap::{Parser, Subcommand};

mod play;
mod player;
mod tracks;

#[allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
mod scrape;

/// An extremely simple lofi player.
#[derive(Parser)]
#[command(about, version)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "señor clippy, i assure you this is not a state machine"
)]
struct Args {
    /// Whether to use an alternate terminal screen.
    #[clap(long, short)]
    alternate: bool,

    /// Whether to hide the bottom control bar.
    #[clap(long, short)]
    minimalist: bool,

    /// Whether to not include borders in the UI.
    #[clap(long, short)]
    borderless: bool,

    /// Whether to start lowfi paused.
    #[clap(long, short)]
    paused: bool,

    /// Whether to include ALSA & other logs.
    #[clap(long, short)]
    debug: bool,

    /// The width of the player, from 0 to 32.
    #[clap(long, short, default_value_t = 3)]
    width: usize,

    /// This is either a path, or a name of a file in the data directory (eg. ~/.local/share/lowfi).
    #[clap(long, short, alias = "list", short_alias = 'l')]
    tracks: Option<String>,

    /// The command that was ran.
    /// This is [None] if no command was specified.
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Defines all of the extra commands lowfi can run.
#[derive(Subcommand)]
enum Commands {
    /// Scrapes the lofi girl website file server for files.
    Scrape {
        /// The file extension to search for, defaults to mp3.
        #[clap(long, short, default_value = "mp3")]
        extension: String,

        /// Whether to include the full HTTP URL or just the distinguishing part.
        #[clap(long, short)]
        include_full: bool,
    },
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    #[cfg(target_os = "android")]
    compile_error!("Android Audio API not supported due to threading shenanigans");

    let cli = Args::parse();

    if let Some(command) = cli.command {
        match command {
            Commands::Scrape {
                extension,
                include_full,
            } => scrape::scrape(extension, include_full).await,
        }
    } else {
        play::play(cli).await
    }
}
