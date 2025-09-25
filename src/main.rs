//! An extremely simple lofi player.

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod messages;
mod play;
mod player;
mod tracks;

#[allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#[cfg(feature = "scrape")]
mod scrapers;

#[cfg(feature = "scrape")]
use crate::scrapers::Source;

/// An extremely simple lofi player.
#[derive(Parser, Clone)]
#[command(about, version)]
#[allow(clippy::struct_excessive_bools)]
struct Args {
    /// Use an alternate terminal screen.
    #[clap(long, short)]
    alternate: bool,

    /// Hide the bottom control bar.
    #[clap(long, short)]
    minimalist: bool,

    /// Exclude borders in UI.
    #[clap(long, short)]
    borderless: bool,

    /// Start lowfi paused.
    #[clap(long, short)]
    paused: bool,

    /// FPS of the UI.
    #[clap(long, short, default_value_t = 12)]
    fps: u8,

    /// Timeout in seconds for music downloads.
    #[clap(long, default_value_t = 3)]
    timeout: u64,

    /// Include ALSA & other logs.
    #[clap(long, short)]
    debug: bool,

    /// Width of the player, from 0 to 32.
    #[clap(long, short, default_value_t = 3)]
    width: usize,

    /// Use a custom track list
    #[clap(long, short, alias = "list", alias = "tracks", short_alias = 'l')]
    track_list: Option<String>,

    /// Internal song buffer size.
    #[clap(long, short = 's', alias = "buffer", default_value_t = 5)]
    buffer_size: usize,

    /// The command that was ran.
    /// This is [None] if no command was specified.
    #[command(subcommand)]
    command: Option<Commands>,
}

/// Defines all of the extra commands lowfi can run.
#[derive(Subcommand, Clone)]
enum Commands {
    /// Scrapes a music source for files.
    #[cfg(feature = "scrape")]
    Scrape {
        // The source to scrape from.
        #[clap(long, short)]
        source: scrapers::Source,
    },
}

/// Gets lowfi's data directory.
pub fn data_dir() -> eyre::Result<PathBuf, player::Error> {
    let dir = dirs::data_dir()
        .ok_or(player::Error::DataDir)?
        .join("lowfi");

    Ok(dir)
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let cli = Args::parse();

    if let Some(command) = cli.command {
        match command {
            #[cfg(feature = "scrape")]
            Commands::Scrape { source } => match source {
                Source::Archive => scrapers::archive::scrape().await?,
                Source::Lofigirl => scrapers::lofigirl::scrape().await?,
                Source::Chillhop => scrapers::chillhop::scrape().await?,
            },
        }
    } else {
        play::play(cli).await?;
    };

    Ok(())
}
