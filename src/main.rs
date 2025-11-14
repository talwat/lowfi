//! An extremely simple lofi player.

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

use clap::{Parser, Subcommand};
use std::path::PathBuf;

mod messages;
mod play;
mod player;
mod tracks;
mod dbg;

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

    /// For detailed debug logs.
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
#[derive(Subcommand, Clone, Debug)]
enum Commands {
    /// Scrapes a music source for files.
    #[cfg(feature = "scrape")]
    Scrape {
        // The source to scrape from.
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
    debug_log!("main.rs - main: starting lowfi application");
    color_eyre::install()?;

    debug_log!("main.rs - main: parsing command line arguments");
    let cli = Args::parse();

    if cli.debug {
        debug_log!("main.rs - main: debug mode enabled, initializing logger");
        // Initialize env_logger to surface logs from dependencies (rodio/cpal/etc.)
        let mut builder = env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("debug,html5ever=warn,selectors=warn"));
        builder.format(|buf, record| {
            use std::io::Write;
            let level = record.level();
            let mut msg = record.args().to_string();
            while msg.ends_with('\n') { msg.pop(); }
            writeln!(buf, "{}: {}", level, msg)
        }).init();
        dbg::enable();
        debug_log!("main.rs - main: logger initialized and debug logging enabled");
    }

    if let Some(command) = cli.command {
        debug_log!("main.rs - main: executing command: {:?}", command);
        match command {
            #[cfg(feature = "scrape")]
            Commands::Scrape { source } => {
                debug_log!("main.rs - main: executing scrape command for source: {:?}", source);
                match source {
                   Source::Archive => scrapers::archive::scrape().await?,
                    Source::Lofigirl => scrapers::lofigirl::scrape().await?,
                    Source::Chillhop => scrapers::chillhop::scrape().await?,
                }
            }
        }
    } else {
        debug_log!("main.rs - main: no command specified, starting audio player");
        play::play(cli).await?;
    };

    debug_log!("main.rs - main: application completed successfully");
    Ok(())
}