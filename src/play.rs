use std::sync::Arc;

use tokio::{
    sync::mpsc::{self},
    task::{self},
};

use crate::player::Player;
use crate::player::{ui, Messages};

pub async fn play() -> eyre::Result<()> {
    let (tx, rx) = mpsc::channel(8);

    let player = Arc::new(Player::new().await?);
    let audio = task::spawn(Player::play(player.clone(), rx));
    tx.send(Messages::Init).await?;

    ui::start(player.clone(), tx.clone()).await?;

    audio.abort();
    player.sink.stop();

    Ok(())
}
