//! An extremely simple lofi player.

#![warn(clippy::all, clippy::pedantic, clippy::nursery)]

use clap::{Parser, Subcommand, ValueEnum};
use std::path::PathBuf;

mod messages;
mod play;
mod player;
mod tracks;
mod dbg;
mod bandcamp {
    pub mod discography;
    pub use discography::*;
}

#[allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::restriction)]
#[cfg(feature = "scrape")]
mod scrapers;

#[cfg(feature = "scrape")]
use crate::scrapers::Source;


#[cfg(feature = "color")]
#[derive(ValueEnum, Clone, Debug)]
#[clap(rename_all = "kebab-case")]
pub enum ArtStyle {
    Pixel,
    #[clap(name = "ascii-bg")]
    AsciiBg,
    Ascii,
}

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

    /// For insanely detailed debug logs.
    #[clap(long, short)]
    debug: bool,

    /// Width of the player, from 0 to 32.
    #[clap(long, short, default_value_t = 10)]
    width: usize,

    /// Enable rendering cover art.
    #[cfg(feature = "color")]
    #[clap(long)]
    art: Option<ArtStyle>,

    /// Disable colors. And if you have problems with Bandcamp loading try this.
    #[cfg(feature = "color")]
    #[clap(long)]
    colorless: bool,

    /// Use a custom track list
    #[clap(long, short, alias = "list", alias = "tracks", short_alias = 'l')]
    track_list: Option<String>,

    /// Use Lofi Girl archive (6321 tracks) instead of Bandcamp, include more accurate tags for some tracks.
    #[clap(long)]
    archive: bool,

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

    // Just for debugging purposes.
    /// Creates a presaved Bandcamp list in ./data directory. 
    #[cfg(feature = "presave")]
    PresaveBandcamp {
        url: String,
        #[clap(long, default_value_t = 0)]
        max_albums: usize,
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
            writeln!(buf, "{}: {}", level, record.args())
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
            },
            #[cfg(feature = "presave")]
            Commands::PresaveBandcamp { url, max_albums } => {
                debug_log!("main.rs - main: executing presave command for URL: {} max_albums: {:?}", url, max_albums);
                tracks::presave::create_presaved_bandcamp_list(&url, max_albums).await?;
            },
        }
    } else {
        debug_log!("main.rs - main: no command specified, starting audio player");
        play::play(cli).await?;
    };

    debug_log!("main.rs - main: application completed successfully");
    Ok(())
}
