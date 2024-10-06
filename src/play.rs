//! Responsible for the basic initialization & shutdown of the audio server & frontend.

use std::sync::Arc;

use tokio::{sync::mpsc, task};

use crate::player::Player;
use crate::player::{ui, Messages};
use crate::Args;

/// Initializes the audio server, and then safely stops
/// it when the frontend quits.
pub async fn play(args: Args) -> eyre::Result<()> {
    let (tx, rx) = mpsc::channel(8);
    let player = Arc::new(Player::new(!args.alternate).await?);
    let ui = task::spawn(ui::start(Arc::clone(&player), tx.clone(), args));

    tx.send(Messages::Init).await?;

    Player::play(Arc::clone(&player), tx.clone(), rx).await?;
    player.sink.stop();
    ui.abort();

    Ok(())
}
