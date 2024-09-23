use clap::{Parser, Subcommand};

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
    /// Plays a single, random, track.
    Play
}

#[tokio::main]
async fn main() -> eyre::Result<()> {
    let cli = Args::parse();

    match cli.command {
        Commands::Scrape => scrape::scrape().await,
        Commands::Play => tracks::random().await
    }
}