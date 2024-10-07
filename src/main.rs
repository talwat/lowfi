use clap::{Parser, Subcommand};

mod play;
mod player;
mod scrape;
mod tracks;

/// An extremely simple lofi player.
#[derive(Parser)]
#[command(about, version)]
struct Args {
    /// Whether to use an alternate terminal screen.
    #[clap(long, short)]
    alternate: bool,

    /// Whether to hide the bottom control bar.
    #[clap(long, short)]
    minimalist: bool,

    /// Whether to start lowfi paused.
    #[clap(long, short)]
    paused: bool,

    /// Whether to include ALSA & other logs.
    #[clap(long, short)]
    debug: bool,

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
