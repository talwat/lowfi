//! An extremely simple lofi player.
pub mod error;
use std::path::PathBuf;

use clap::{Parser, Subcommand};
mod tests;
pub use error::{Error, Result};
pub mod message;
pub mod ui;
pub use message::Message;

use crate::player::Player;
pub mod audio;
pub mod bookmark;
pub mod download;
pub mod player;
pub mod tracks;
pub mod volume;

#[cfg(feature = "scrape")]
mod scrapers;

#[cfg(feature = "scrape")]
use crate::scrapers::Source;

/// An extremely simple lofi player.
#[derive(Parser, Clone)]
#[command(about, version)]
#[allow(clippy::struct_excessive_bools)]
pub struct Args {
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

    /// Track list to play music from
    #[clap(long, short, alias = "list", alias = "tracks", short_alias = 'l', default_value_t = String::from("chillhop"))]
    track_list: String,

    /// Internal song buffer size.
    #[clap(long, short = 's', alias = "buffer", default_value_t = 5, value_parser = clap::value_parser!(u32).range(2..))]
    buffer_size: u32,

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
        source: scrapers::Source,
    },
}

/// Gets lowfi's data directory.
pub fn data_dir() -> crate::Result<PathBuf> {
    let dir = dirs::data_dir().unwrap().join("lowfi");

    Ok(dir)
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let args = Args::parse();

    #[cfg(feature = "scrape")]
    if let Some(command) = &args.command {
        match command {
            Commands::Scrape { source } => match source {
                Source::Archive => scrapers::archive::scrape().await?,
                Source::Lofigirl => scrapers::lofigirl::scrape().await?,
                Source::Chillhop => scrapers::chillhop::scrape().await?,
            },
        }
    }

    let player = Player::init(args).await?;
    let environment = player.environment();
    let result = player.run().await;

    environment.cleanup(result.is_ok())?;
    Ok(result?)
}
