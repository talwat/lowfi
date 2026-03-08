//! An extremely simple lofi player.

use crate::player::Player;
use clap::{Parser, Subcommand};
use std::path::PathBuf;

pub mod audio;
pub mod bookmark;
pub mod downloader;
pub mod error;
pub mod message;
pub mod player;
#[cfg(feature = "scrape")]
mod scrapers;
pub mod tasks;
mod tests;
pub mod tracks;
pub mod ui;
pub mod volume;

#[cfg(feature = "scrape")]
use crate::scrapers::Source;
pub use error::{Error, Result};
pub use message::Message;
pub use tasks::Tasks;

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

    /// Exclude window borders.
    #[clap(long, short)]
    borderless: bool,

    /// Include a clock.
    #[clap(long, short)]
    clock: bool,

    /// Start lowfi paused.
    #[clap(long, short)]
    paused: bool,

    /// FPS of the UI.
    #[clap(long, short, default_value_t = 12)]
    fps: u8,

    /// Timeout in seconds for music downloads.
    #[clap(long, default_value_t = 16)]
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

    /// List Track options
    #[clap(long, short, alias = "options", short_alias = 'o')]
    options: bool,

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

/// Shorthand to get a boolean environment variable.
#[inline]
pub fn env(name: &str) -> bool {
    std::env::var(name).is_ok_and(|x| x == "1")
}

/// Returns the application data directory used for persistency.
///
/// The function returns the platform-specific user data directory with
/// a `lowfi` subfolder. Callers may use this path to store config,
/// bookmarks, and other persistent files.
#[inline]
pub fn data_dir() -> crate::Result<PathBuf> {
    let dir = dirs::data_dir().unwrap().join("lowfi");

    Ok(dir)
}

/// Program entry point.
///
/// Parses CLI arguments, initializes the audio stream and player, then
/// runs the main event loop. On exit it performs cleanup of the UI and
/// returns the inner result.
#[tokio::main(flavor = "current_thread")]
async fn main() -> eyre::Result<()> {
    let args = Args::parse();

    if args.options {
        let option_list = tracks::List::load_all().await?;
        for list in option_list {
            println!("{}", list.name);
        }
        return Ok(());
    }

    #[cfg(feature = "scrape")]
    if let Some(command) = &args.command {
        return match command {
            Commands::Scrape { source } => match source {
                Source::Archive => scrapers::archive::scrape().await,
                Source::Lofigirl => scrapers::lofigirl::scrape().await,
                Source::Chillhop => scrapers::chillhop::scrape().await,
            },
        };
    }

    let stream = audio::stream()?;
    let environment = ui::Environment::ready(&args)?;
    let (mut player, tasks) = Player::init(args, stream.mixer())
        .await
        .inspect_err(|_| environment.cleanup(false).unwrap())?;

    let result = tokio::select! {
        r = player.run() => r,
        r = tasks => r,
    };

    environment.cleanup(result.is_ok())?;
    player.close().await?;

    Ok(result?)
}
