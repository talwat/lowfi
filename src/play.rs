//! Responsible for the basic initialization & shutdown of the audio server & frontend.

use std::{io::stderr, sync::Arc};

use crossterm::{cursor::SavePosition, terminal};
use tokio::{
    sync::mpsc::{self},
    task::{self},
};

use crate::player::Player;
use crate::player::{ui, Messages};

/// Initializes the audio server, and then safely stops
/// it when the frontend quits.
pub async fn play(alternate: bool) -> eyre::Result<()> {
    // Save the position. This is important since later on we can revert to this position
    // and clear any potential error messages that may have showed up.
    // TODO: Figure how to set some sort of flag to hide error messages within rodio,
    // TODO: Instead of just ignoring & clearing them after.
    crossterm::execute!(stderr(), SavePosition)?;

    // Enable raw mode early in theory to prevent uncontrolled text in the terminal from the user.
    terminal::enable_raw_mode()?;

    let (tx, rx) = mpsc::channel(8);

    let player = Arc::new(Player::new().await?);
    let audio = task::spawn(Player::play(Arc::clone(&player), tx.clone(), rx));
    tx.send(Messages::Init).await?;

    ui::start(Arc::clone(&player), tx.clone(), alternate).await?;

    audio.abort();
    player.sink.stop();

    Ok(())
}
