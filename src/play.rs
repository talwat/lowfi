//! Responsible for the basic initialization & shutdown of the audio server & frontend.

use crossterm::cursor::Show;
use crossterm::event::PopKeyboardEnhancementFlags;
use crossterm::terminal::{self, Clear, ClearType};
use std::io::{stdout, IsTerminal};
use std::process::exit;
use std::sync::Arc;
use std::{env, panic};
use tokio::{sync::mpsc, task};

use crate::messages::Message;
use crate::player::persistent_volume::PersistentVolume;
use crate::player::Player;
use crate::player::{self, ui};
use crate::Args;

/// Initializes the audio server, and then safely stops
/// it when the frontend quits.
pub async fn play(args: Args) -> eyre::Result<(), player::Error> {
    // TODO: This isn't a great way of doing things,
    // but it's better than vanilla behaviour at least.
    let eyre_hook = panic::take_hook();

    panic::set_hook(Box::new(move |x| {
        let mut lock = stdout().lock();
        crossterm::execute!(
            lock,
            Clear(ClearType::FromCursorDown),
            Show,
            PopKeyboardEnhancementFlags
        )
        .unwrap();
        terminal::disable_raw_mode().unwrap();

        eyre_hook(x);
        exit(1)
    }));

    // Actually initializes the player.
    // Stream kept here in the master thread to keep it alive.
    let (player, stream) = Player::new(&args).await?;
    let player = Arc::new(player);

    // Initialize the UI, as well as the internal communication channel.
    let (tx, rx) = mpsc::channel(8);
    let ui = if stdout().is_terminal() && !(env::var("LOWFI_DISABLE_UI") == Ok("1".to_owned())) {
        Some(task::spawn(ui::start(
            Arc::clone(&player),
            tx.clone(),
            args.clone(),
        )))
    } else {
        None
    };

    // Sends the player an "init" signal telling it to start playing a song straight away.
    tx.send(Message::Init).await?;

    // Actually starts the player.
    Player::play(Arc::clone(&player), tx.clone(), rx, args.debug).await?;

    // Save the volume.txt file for the next session.
    PersistentVolume::save(player.sink.volume())
        .await
        .map_err(player::Error::PersistentVolumeSave)?;

    drop(stream);
    player.sink.stop();
    ui.map(|x| x.abort());

    Ok(())
}
