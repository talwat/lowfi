//! Responsible for specifically recieving terminal input
//! using [`crossterm`].

use crossterm::event::{self, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures::{FutureExt as _, StreamExt as _};
use tokio::sync::mpsc::Sender;

use crate::player::{ui, Messages};

/// Starts the listener to recieve input from the terminal for various events.
pub async fn listen(sender: Sender<Messages>) -> eyre::Result<()> {
    let mut reader = EventStream::new();

    loop {
        let Some(Ok(event::Event::Key(event))) = reader.next().fuse().await else {
            continue;
        };

        if event.kind == KeyEventKind::Release {
            continue;
        }

        let messages = match event.code {
            // Arrow key volume controls.
            KeyCode::Up => Messages::ChangeVolume(0.1),
            KeyCode::Right => Messages::ChangeVolume(0.01),
            KeyCode::Down => Messages::ChangeVolume(-0.1),
            KeyCode::Left => Messages::ChangeVolume(-0.01),
            KeyCode::Char(character) => match character.to_ascii_lowercase() {
                // Ctrl+C
                'c' if event.modifiers == KeyModifiers::CONTROL => Messages::Quit,

                // Quit
                'q' => Messages::Quit,

                // Skip/Next
                's' | 'n' | 'l' => Messages::Next,

                // Pause
                'p' | ' ' => Messages::PlayPause,

                // Volume up & down
                '+' | '=' | 'k' => Messages::ChangeVolume(0.1),
                '-' | '_' | 'j' => Messages::ChangeVolume(-0.1),

                _ => continue,
            },
            // Media keys
            KeyCode::Media(media) => match media {
                event::MediaKeyCode::Pause
                | event::MediaKeyCode::Play
                | event::MediaKeyCode::PlayPause => Messages::PlayPause,
                event::MediaKeyCode::Stop => Messages::Pause,
                event::MediaKeyCode::TrackNext => Messages::Next,
                event::MediaKeyCode::LowerVolume => Messages::ChangeVolume(-0.1),
                event::MediaKeyCode::RaiseVolume => Messages::ChangeVolume(0.1),
                event::MediaKeyCode::MuteVolume => Messages::ChangeVolume(-1.0),
                _ => continue,
            },
            _ => continue,
        };

        if let Messages::ChangeVolume(_) = messages {
            ui::flash_audio();
        }

        sender.send(messages).await?;
    }
}
