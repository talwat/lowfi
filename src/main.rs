use clap::{Parser, Subcommand};

mod play;
mod player;
mod scrape;
mod tracks;

/// An extremely simple lofi player.
#[derive(Parser)]
#[command(about)]
struct Args {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Scrapes the lofi girl website file server for files.
    Scrape {
        /// The file extention to search for, defaults to mp3.
        #[clap(long, short, default_value = "mp3")]
        extention: String,

        /// Whether to include the full HTTP URL or just the distinguishing part.
        #[clap(long, short)]
        include_full: bool,
    },
    /// Starts the player.
    Play,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Args::parse();

    match cli.command {
        Commands::Scrape {
            extention,
            include_full,
        } => scrape::scrape(extention, include_full).await,
        Commands::Play => play::play().await,
    }
}
