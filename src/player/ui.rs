use std::{io::stderr, sync::Arc, time::Duration};

use super::Player;
use crossterm::{
    cursor::{MoveToColumn, MoveUp, Show},
    style::Print,
    terminal::{Clear, ClearType},
};
use tokio::{
    sync::mpsc::Sender,
    task::{self},
    time::sleep,
};

use super::Messages;

async fn interface(queue: Arc<Player>) -> eyre::Result<()> {
    const WIDTH: usize = 25;
    const PROGRESS_WIDTH: usize = WIDTH - 4;

    loop {
        // We can get away with only redrawing every 0.25 seconds
        // since it's just an audio player.
        sleep(Duration::from_secs_f32(0.25)).await;
        crossterm::execute!(stderr(), Clear(ClearType::FromCursorDown))?;

        let mut main = match queue.current.load().as_ref() {
            Some(x) => {
                if queue.sink.is_paused() {
                    format!("paused {}", x.format_name())
                } else {
                    format!("playing {}", x.format_name())
                }
            }
            None => "loading...".to_owned(),
        };

        main.push_str("\r\n");

        let mut filled = 0;
        if let Some(current) = queue.current.load().as_ref() {
            if let Some(duration) = current.duration {
                let elapsed = queue.sink.get_pos().as_secs() as f32 / duration.as_secs() as f32;
                filled = (elapsed * PROGRESS_WIDTH as f32).round() as usize;
            }
        };

        let progress = format!(
            " [{}{}] ",
            "/".repeat(filled as usize),
            " ".repeat(PROGRESS_WIDTH - filled)
        );
        let bar = ["[s]kip", "[p]ause", "[q]uit"];

        crossterm::execute!(stderr(), MoveToColumn(0), Print(main))?;
        crossterm::execute!(stderr(), Print(progress), Print("\r\n"))?;
        crossterm::execute!(stderr(), Print(bar.join("   ")), Print("\r\n"))?;
        crossterm::execute!(stderr(), MoveToColumn(0), MoveUp(3))?;
    }
}

pub async fn start(queue: Arc<Player>, sender: Sender<Messages>) -> eyre::Result<()> {
    crossterm::terminal::enable_raw_mode()?;
    //crossterm::execute!(stderr(), EnterAlternateScreen, MoveTo(0, 0))?;

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

    crossterm::execute!(stderr(), Clear(ClearType::FromCursorDown), Show)?;
    crossterm::terminal::disable_raw_mode()?;

    Ok(())
}
