use std::{io::stderr, sync::Arc, time::Duration};

use super::Player;
use crossterm::{
    cursor::{MoveTo, MoveToColumn, MoveUp},
    style::Print,
    terminal::{Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use tokio::{
    sync::mpsc::Sender,
    task::{self},
    time::sleep,
};

use super::Messages;

async fn interface(queue: Arc<Player>) -> eyre::Result<()> {
    const WIDTH: usize = 25;

    loop {
        // We can get away with only redrawing every 0.25 seconds
        // since it's just an audio player.
        sleep(Duration::from_secs_f32(1.0 / 60.0)).await;
        crossterm::execute!(stderr(), Clear(ClearType::FromCursorDown))?;

        let main = match queue.current.load().as_ref() {
            Some(x) => {
                if queue.sink.is_paused() {
                    format!("paused {}\r\n", x.format_name())
                } else {
                    format!("playing {}\r\n", x.format_name())
                }
            }
            None => "loading...\r\n".to_owned(),
        };

        let bar = ["[s]kip", "[p]ause", "[q]uit"];

        crossterm::execute!(stderr(), MoveToColumn(0), Print(main))?;
        crossterm::execute!(stderr(), Print(bar.join("   ")))?;
        crossterm::execute!(stderr(), MoveToColumn(0), MoveUp(1))?;
    }
}

pub async fn start(queue: Arc<Player>, sender: Sender<Messages>) -> eyre::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    crossterm::execute!(stderr(), EnterAlternateScreen, MoveTo(0, 0))?;

    task::spawn(interface(queue.clone()));

    loop {
        let crossterm::event::Event::Key(event) = crossterm::event::read()? else {
            continue;
        };

        let crossterm::event::KeyCode::Char(code) = event.code else {
            continue;
        };

        match code {
            'q' => {
                break;
            }
            's' => {
                if !queue.current.load().is_none() {
                    sender.send(Messages::Next).await?
                }
            }
            'p' => {
                sender.send(Messages::Pause).await?;
            }
            _ => {}
        }
    }

    crossterm::execute!(
        stderr(),
        Clear(ClearType::FromCursorDown),
        LeaveAlternateScreen
    )?;
    crossterm::terminal::disable_raw_mode()?;

    Ok(())
}
