//! Responsible for specifically recieving terminal input
//! using [`crossterm`].

use crossterm::event::{self, EventStream, KeyCode, KeyEventKind, KeyModifiers};
use futures::{FutureExt as _, StreamExt as _};
use tokio::sync::mpsc::Sender;
use crate::Message;

/// Starts the listener to recieve input from the terminal for various events.
pub async fn listen(sender: Sender<Message>) -> crate::Result<()> {
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
            KeyCode::Up => Message::ChangeVolume(0.1),
            KeyCode::Right => Message::ChangeVolume(0.01),
            KeyCode::Down => Message::ChangeVolume(-0.1),
            KeyCode::Left => Message::ChangeVolume(-0.01),
            KeyCode::Char(character) => match character.to_ascii_lowercase() {
                // Ctrl+C
                'c' if event.modifiers == KeyModifiers::CONTROL => Message::Quit,

                // Quit
                'q' => Message::Quit,

                // Skip/Next
                's' | 'n' | 'l' => Message::Next,

                // Pause
                'p' | ' ' => Message::PlayPause,

                // Volume up & down
                '+' | '=' | 'k' => Message::ChangeVolume(0.1),
                '-' | '_' | 'j' => Message::ChangeVolume(-0.1),

                // Bookmark
                'b' => Message::Bookmark,

                _ => continue,
            },
            // Media keys
            KeyCode::Media(media) => match media {
                event::MediaKeyCode::Pause
                | event::MediaKeyCode::Play
                | event::MediaKeyCode::PlayPause => Message::PlayPause,
                event::MediaKeyCode::Stop => Message::Pause,
                event::MediaKeyCode::TrackNext => Message::Next,
                event::MediaKeyCode::LowerVolume => Message::ChangeVolume(-0.1),
                event::MediaKeyCode::RaiseVolume => Message::ChangeVolume(0.1),
                event::MediaKeyCode::MuteVolume => Message::ChangeVolume(-1.0),
                _ => continue,
            },
            _ => continue,
        };

        sender.send(messages).await?;
    }
}