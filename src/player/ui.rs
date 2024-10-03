//! The module which manages all user interface, including inputs.

use std::{
    io::stdout,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use super::Player;
use crossterm::{
    cursor::{Hide, MoveTo, MoveToColumn, MoveUp, Show},
    event::{self, KeyCode, KeyModifiers},
    style::{Print, Stylize},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use tokio::{
    sync::mpsc::Sender,
    task::{self},
    time::sleep,
};

use super::Messages;

mod components;

/// The total width of the UI.
const WIDTH: usize = 27;

/// The width of the progress bar, not including the borders (`[` and `]`) or padding.
const PROGRESS_WIDTH: usize = WIDTH - 16;

/// The width of the audio bar, again not including borders or padding.
const AUDIO_WIDTH: usize = WIDTH - 17;

/// Self explanitory.
const FPS: usize = 12;

/// How long the audio bar will be visible for when audio is adjusted.
/// This is in frames.
const AUDIO_BAR_DURATION: usize = 9;

/// How long to wait in between frames.
/// This is fairly arbitrary, but an ideal value should be enough to feel
/// snappy but not require too many resources.
const FRAME_DELTA: f32 = 1.0 / FPS as f32;

/// The code for the interface itself.
///
/// `volume_timer` is a bit strange, but it tracks how long the `volume` bar
/// has been displayed for, so that it's only displayed for a certain amount of frames.
async fn interface(player: Arc<Player>, volume_timer: Arc<AtomicUsize>) -> eyre::Result<()> {
    loop {
        let action = components::action(&player);

        let timer = volume_timer.load(Ordering::Relaxed);
        let middle = match timer {
            0 => components::progress_bar(&player),
            _ => components::audio_bar(&player),
        };

        if timer > 0 && timer <= AUDIO_BAR_DURATION {
            volume_timer.fetch_add(1, Ordering::Relaxed);
        } else if timer > AUDIO_BAR_DURATION {
            volume_timer.store(0, Ordering::Relaxed);
        }

        let controls = [
            format!("{}kip", "[s]".bold()),
            format!("{}ause", "[p]".bold()),
            format!("{}uit", "[q]".bold()),
        ];

        // Formats the menu properly
        let menu = [action, middle, controls.join("    ")]
            .map(|x| format!("│ {} │\r\n", x.reset()).to_string());

        crossterm::execute!(
            stdout(),
            Clear(ClearType::FromCursorDown),
            MoveToColumn(0),
            Print(format!("┌{}┐\r\n", "─".repeat(WIDTH + 2))),
            Print(menu.join("")),
            Print(format!("└{}┘", "─".repeat(WIDTH + 2))),
            MoveToColumn(0),
            MoveUp(4)
        )?;

        sleep(Duration::from_secs_f32(FRAME_DELTA)).await;
    }
}

/// Initializes the UI, this will also start taking input from the user.
///
/// `alternate` controls whether to use [EnterAlternateScreen] in order to hide
/// previous terminal history.
pub async fn start(
    queue: Arc<Player>,
    sender: Sender<Messages>,
    alternate: bool,
) -> eyre::Result<()> {
    crossterm::execute!(stdout(), Hide)?;

    terminal::enable_raw_mode()?;

    if alternate {
        crossterm::execute!(stdout(), EnterAlternateScreen, MoveTo(0, 0))?;
    }

    let volume_timer = Arc::new(AtomicUsize::new(0));

    task::spawn(interface(Arc::clone(&queue), volume_timer.clone()));

    loop {
        let event::Event::Key(event) = event::read()? else {
            continue;
        };

        let messages = match event.code {
            // Arrow key volume controls.
            KeyCode::Up | KeyCode::Right => Messages::VolumeUp,
            KeyCode::Down | KeyCode::Left => Messages::VolumeDown,
            KeyCode::Char(character) => match character {
                // Ctrl+C
                'c' if event.modifiers == KeyModifiers::CONTROL => break,

                // Quit
                'q' => break,

                // Skip/Next
                's' | 'n' if !queue.current.load().is_none() => Messages::Next,

                // Pause
                'p' => Messages::Pause,

                // Volume up & down
                '+' | '=' => Messages::VolumeUp,
                '-' | '_' => Messages::VolumeDown,
                _ => continue,
            },
            _ => continue,
        };

        // If it's modifying the volume, then we'll set the `volume_timer` to 1
        // so that the ui thread will know that it should show the audio bar.
        if messages == Messages::VolumeDown || messages == Messages::VolumeUp {
            volume_timer.store(1, Ordering::Relaxed);
        }

        sender.send(messages).await?;
    }

    if alternate {
        crossterm::execute!(stdout(), LeaveAlternateScreen)?;
    }

    crossterm::execute!(stdout(), Clear(ClearType::FromCursorDown), Show)?;
    terminal::disable_raw_mode()?;

    Ok(())
}
