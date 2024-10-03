//! The module which manages all user interface, including inputs.

use std::{
    io::stdout,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use crate::Args;

use super::Player;
use crossterm::{
    cursor::{Hide, MoveTo, MoveToColumn, MoveUp, Show},
    event::{self, KeyCode, KeyModifiers},
    style::{Print, Stylize},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};
use lazy_static::lazy_static;
use tokio::{
    sync::mpsc::Sender,
    task::{self},
    time::sleep,
};

use super::Messages;

mod components;

/// The total width of the UI.
const WIDTH: usize = 27;

/// Self explanitory.
const FPS: usize = 12;

/// How long the audio bar will be visible for when audio is adjusted.
/// This is in frames.
const AUDIO_BAR_DURATION: usize = 10;

/// How long to wait in between frames.
/// This is fairly arbitrary, but an ideal value should be enough to feel
/// snappy but not require too many resources.
const FRAME_DELTA: f32 = 1.0 / FPS as f32;

lazy_static! {
    /// The volume timer, which controls how long the volume display should
    /// show up and when it should disappear.
    static ref VOLUME_TIMER: AtomicUsize = AtomicUsize::new(0);
}

/// The code for the interface itself.
///
/// `volume_timer` is a bit strange, but it tracks how long the `volume` bar
/// has been displayed for, so that it's only displayed for a certain amount of frames.
async fn interface(player: Arc<Player>) -> eyre::Result<()> {
    loop {
        let action = components::action(&player, WIDTH);

        let timer = VOLUME_TIMER.load(Ordering::Relaxed);
        let volume = player.sink.volume();
        let percentage = format!("{}%", (volume * 100.0).round().abs());

        let middle = match timer {
            0 => components::progress_bar(&player, WIDTH - 16),
            _ => components::audio_bar(volume, &percentage, WIDTH - 17),
        };

        if timer > 0 && timer <= AUDIO_BAR_DURATION {
            VOLUME_TIMER.fetch_add(1, Ordering::Relaxed);
        } else if timer > AUDIO_BAR_DURATION {
            VOLUME_TIMER.store(0, Ordering::Relaxed);
        }

        let controls = components::controls(WIDTH);

        // Formats the menu properly
        let menu = [action, middle, controls].map(|x| format!("│ {} │\r\n", x.reset()).to_string());

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
pub async fn start(queue: Arc<Player>, sender: Sender<Messages>, args: Args) -> eyre::Result<()> {
    crossterm::execute!(stdout(), Hide)?;

    terminal::enable_raw_mode()?;

    if args.alternate {
        crossterm::execute!(stdout(), EnterAlternateScreen, MoveTo(0, 0))?;
    }

    task::spawn(interface(Arc::clone(&queue)));

    loop {
        let event::Event::Key(event) = event::read()? else {
            continue;
        };

        let messages = match event.code {
            // Arrow key volume controls.
            KeyCode::Up => Messages::ChangeVolume(0.1),
            KeyCode::Right => Messages::ChangeVolume(0.01),
            KeyCode::Down => Messages::ChangeVolume(-0.1),
            KeyCode::Left => Messages::ChangeVolume(-0.01),
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
                '+' | '=' => Messages::ChangeVolume(0.1),
                '-' | '_' => Messages::ChangeVolume(-0.1),
                _ => continue,
            },
            _ => continue,
        };

        // If it's modifying the volume, then we'll set the `VOLUME_TIMER` to 1
        // so that the ui thread will know that it should show the audio bar.
        if let Messages::ChangeVolume(_) = messages {
            VOLUME_TIMER.store(1, Ordering::Relaxed);
        }

        sender.send(messages).await?;
    }

    if args.alternate {
        crossterm::execute!(stdout(), LeaveAlternateScreen)?;
    }

    crossterm::execute!(stdout(), Clear(ClearType::FromCursorDown), Show)?;
    terminal::disable_raw_mode()?;

    Ok(())
}
