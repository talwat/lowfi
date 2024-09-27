//! The module which manages all user interface, including inputs.

use std::{io::stderr, sync::Arc, time::Duration};

use crate::tracks::TrackInfo;

use super::Player;
use crossterm::{
    cursor::{Hide, MoveToColumn, MoveUp, Show},
    event::{self, KeyCode, KeyModifiers},
    style::{Print, Stylize},
    terminal::{self, Clear, ClearType},
};
use tokio::{
    sync::mpsc::Sender,
    task::{self},
    time::sleep,
};

use super::Messages;

/// Small helper function to format durations.
fn format_duration(duration: &Duration) -> String {
    let seconds = duration.as_secs() % 60;
    let minutes = duration.as_secs() / 60;

    format!("{:02}:{:02}", minutes, seconds)
}

/// This represents the main "action" bars state.
enum ActionBar {
    Paused(TrackInfo),
    Playing(TrackInfo),
    Loading,
}

impl ActionBar {
    /// Formats the action bar to be displayed.
    /// The second value is the character length of the result.
    fn format(&self) -> (String, usize) {
        let (word, subject) = match self {
            Self::Playing(x) => ("playing", Some(x.name.clone())),
            Self::Paused(x) => ("paused", Some(x.name.clone())),
            Self::Loading => ("loading", None),
        };

        subject.map_or_else(
            || (word.to_owned(), word.len()),
            |subject| {
                (
                    format!("{} {}", word, subject.clone().bold()),
                    word.len() + 1 + subject.len(),
                )
            },
        )
    }
}

/// The code for the interface itself.
async fn interface(queue: Arc<Player>) -> eyre::Result<()> {
    /// The total width of the UI.
    const WIDTH: usize = 27;

    /// The width of the progress bar, not including the borders (`[` and `]`) or padding.
    const PROGRESS_WIDTH: usize = WIDTH - 16;

    loop {
        let (mut main, len) = queue
            .current
            .load()
            .as_ref()
            .map_or(ActionBar::Loading, |x| {
                let name = (*Arc::clone(x)).clone();
                if queue.sink.is_paused() {
                    ActionBar::Paused(name)
                } else {
                    ActionBar::Playing(name)
                }
            })
            .format();

        if len > WIDTH {
            main = format!("{}...", &main[..=WIDTH]);
        } else {
            main = format!("{}{}", main, " ".repeat(WIDTH - len));
        }

        let mut duration = Duration::new(0, 0);
        let elapsed = queue.sink.get_pos();

        let mut filled = 0;
        if let Some(current) = queue.current.load().as_ref() {
            if let Some(x) = current.duration {
                duration = x;

                let elapsed = elapsed.as_secs() as f32 / duration.as_secs() as f32;
                filled = (elapsed * PROGRESS_WIDTH as f32).round() as usize;
            }
        };

        let progress = format!(
            " [{}{}] {}/{} ",
            "/".repeat(filled),
            " ".repeat(PROGRESS_WIDTH.saturating_sub(filled)),
            format_duration(&elapsed),
            format_duration(&duration),
        );
        let bar = [
            format!("{}kip", "[s]".bold()),
            format!("{}ause", "[p]".bold()),
            format!("{}uit", "[q]".bold()),
        ];

        // Formats the menu properly
        let menu =
            [main, progress, bar.join("    ")].map(|x| format!("│ {x} │\r\n").reset().to_string());

        crossterm::execute!(stderr(), Clear(ClearType::FromCursorDown))?;
        crossterm::execute!(
            stderr(),
            MoveToColumn(0),
            Print(format!("┌{}┐\r\n", "─".repeat(WIDTH + 2))),
            Print(menu.join("")),
            Print(format!("└{}┘", "─".repeat(WIDTH + 2))),
            MoveToColumn(0),
            MoveUp(4)
        )?;

        sleep(Duration::from_secs_f32(10.0 / 60.0)).await;
    }
}

/// Initializes the UI, this will also start taking input from the user.
pub async fn start(queue: Arc<Player>, sender: Sender<Messages>) -> eyre::Result<()> {
    terminal::enable_raw_mode()?;
    crossterm::execute!(stderr(), Hide)?;
    //crossterm::execute!(stderr(), EnterAlternateScreen, MoveTo(0, 0))?;

    task::spawn(interface(Arc::clone(&queue)));

    loop {
        let event::Event::Key(event) = event::read()? else {
            continue;
        };

        let KeyCode::Char(code) = event.code else {
            continue;
        };

        match code {
            'c' => {
                // Handles Ctrl+C.
                if event.modifiers == KeyModifiers::CONTROL {
                    break;
                }
            }
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

    //crossterm::execute!(stderr(), LeaveAlternateScreen)?;
    crossterm::execute!(stderr(), Clear(ClearType::FromCursorDown), Show)?;
    terminal::disable_raw_mode()?;

    Ok(())
}
