//! Responsible for the basic initialization & shutdown of the audio server & frontend.

use std::io::stdout;
use std::sync::Arc;

use crossterm::cursor::SavePosition;
use tokio::{sync::mpsc, task};

use crate::player::Player;
use crate::player::{ui, Messages};

/// Initializes the audio server, and then safely stops
/// it when the frontend quits.
pub async fn play(alternate: bool) -> eyre::Result<()> {
    // Save the position. This is important since later on we can revert to this position
    // and clear any potential error messages that may have showed up.
    // TODO: Figure how to set some sort of flag to hide error messages within alsa/rodio,
    // TODO: Instead of just ignoring & clearing them after.
    // TODO: Fix this, as it doesn't work with some terminals when the cursor is at the bottom of the terminal.
    crossterm::execute!(stdout(), SavePosition)?;

    let (tx, rx) = mpsc::channel(8);

    let player = Arc::new(Player::new().await?);
    let audio = task::spawn(Player::play(Arc::clone(&player), tx.clone(), rx));
    tx.send(Messages::Init).await?;

    ui::start(Arc::clone(&player), tx.clone(), alternate).await?;

    audio.abort();
    player.sink.stop();

    Ok(())
}
