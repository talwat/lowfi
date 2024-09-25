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
    /// Scrapes the lofi girl website file server for mp3 files.
    Scrape,
    /// Starts the player.
    Play,
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Args::parse();

    match cli.command {
        Commands::Scrape => scrape::scrape().await,
        Commands::Play => play::play().await,
    }
}
