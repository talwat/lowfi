//! The module which manages all user interface, including inputs.

use std::{
    io::stdout,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use crate::tracks::TrackInfo;

use super::Player;
use crossterm::{
    cursor::{Hide, MoveTo, MoveToColumn, MoveUp, RestorePosition, Show},
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

/// Creates the progress bar, as well as all the padding needed.
fn progress_bar(player: &Arc<Player>) -> String {
    let mut duration = Duration::new(0, 0);
    let elapsed = player.sink.get_pos();

    let mut filled = 0;
    if let Some(current) = player.current.load().as_ref() {
        if let Some(x) = current.duration {
            duration = x;

            let elapsed = elapsed.as_secs() as f32 / duration.as_secs() as f32;
            filled = (elapsed * PROGRESS_WIDTH as f32).round() as usize;
        }
    };

    format!(
        " [{}{}] {}/{} ",
        "/".repeat(filled),
        " ".repeat(PROGRESS_WIDTH.saturating_sub(filled)),
        format_duration(&elapsed),
        format_duration(&duration),
    )
}

/// Creates the audio bar, as well as all the padding needed.
fn audio_bar(player: &Arc<Player>) -> String {
    let volume = player.sink.volume();

    let audio = (player.sink.volume() * AUDIO_WIDTH as f32).round() as usize;
    let percentage = format!("{}%", (volume * 100.0).ceil().abs());

    format!(
        " volume: [{}{}] {}{} ",
        "/".repeat(audio),
        " ".repeat(AUDIO_WIDTH.saturating_sub(audio)),
        " ".repeat(4usize.saturating_sub(percentage.len())),
        percentage,
    )
}

/// The code for the interface itself.
///
/// `volume_timer` is a bit strange, but it tracks how long the `volume` bar
/// has been displayed for, so that it's only displayed for a certain amount of frames.
async fn interface(player: Arc<Player>, volume_timer: Arc<AtomicUsize>) -> eyre::Result<()> {
    loop {
        let (mut main, len) = player
            .current
            .load()
            .as_ref()
            .map_or(ActionBar::Loading, |x| {
                let name = (*Arc::clone(x)).clone();
                if player.sink.is_paused() {
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

        let timer = volume_timer.load(Ordering::Relaxed);
        let middle = match timer {
            0 => progress_bar(&player),
            _ => audio_bar(&player),
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
        let menu = [main, middle, controls.join("    ")]
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
    crossterm::execute!(
        stdout(),
        RestorePosition,
        Clear(ClearType::CurrentLine),
        Clear(ClearType::FromCursorDown),
        Hide
    )?;

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
