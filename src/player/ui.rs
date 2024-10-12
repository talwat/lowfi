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

use crossterm::{
    cursor::{Hide, MoveTo, MoveToColumn, MoveUp, Show},
    event::{
        self, EventStream, KeyCode, KeyModifiers, KeyboardEnhancementFlags,
        PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
    },
    style::{Print, Stylize},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use futures::{FutureExt, StreamExt};
use lazy_static::lazy_static;
use tokio::{sync::mpsc::Sender, task, time::sleep};

use super::{Messages, Player};

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

async fn input(sender: Sender<Messages>) -> eyre::Result<()> {
    let mut reader = EventStream::new();

    loop {
        let Some(Ok(event::Event::Key(event))) = reader.next().fuse().await else {
            continue;
        };

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
                's' | 'n' => Messages::Next,

                // Pause
                'p' => Messages::PlayPause,

                // Volume up & down
                '+' | '=' => Messages::ChangeVolume(0.1),
                '-' | '_' => Messages::ChangeVolume(-0.1),

                _ => continue,
            },
            // Media keys
            KeyCode::Media(media) => match media {
                event::MediaKeyCode::Play => Messages::PlayPause,
                event::MediaKeyCode::Pause => Messages::PlayPause,
                event::MediaKeyCode::PlayPause => Messages::PlayPause,
                event::MediaKeyCode::Stop => Messages::Pause,
                event::MediaKeyCode::TrackNext => Messages::Next,
                event::MediaKeyCode::LowerVolume => Messages::ChangeVolume(-0.1),
                event::MediaKeyCode::RaiseVolume => Messages::ChangeVolume(0.1),
                event::MediaKeyCode::MuteVolume => Messages::ChangeVolume(-1.0),
                _ => continue,
            },
            _ => continue,
        };

        // If it's modifying the volume, then we'll set the `VOLUME_TIMER` to 1
        // so that the UI thread will know that it should show the audio bar.
        if let Messages::ChangeVolume(_) = messages {
            VOLUME_TIMER.store(1, Ordering::Relaxed);
        }

        sender.send(messages).await?;
    }
}

/// The code for the terminal interface itself.
///
/// `volume_timer` is a bit strange, but it tracks how long the `volume` bar
/// has been displayed for, so that it's only displayed for a certain amount of frames.
async fn interface(player: Arc<Player>, minimalist: bool) -> eyre::Result<()> {
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

        let menu = if minimalist {
            vec![action, middle]
        } else {
            vec![action, middle, controls]
        };

        // Formats the menu properly
        let menu: Vec<String> = menu
            .into_iter()
            .map(|x| format!("│ {} │\r\n", x.reset()).to_string())
            .collect();

        crossterm::execute!(
            stdout(),
            Clear(ClearType::FromCursorDown),
            MoveToColumn(0),
            Print(format!("┌{}┐\r\n", "─".repeat(WIDTH + 2))),
            Print(menu.join("")),
            Print(format!("└{}┘", "─".repeat(WIDTH + 2))),
            MoveToColumn(0),
            MoveUp(menu.len() as u16 + 1)
        )?;

        sleep(Duration::from_secs_f32(FRAME_DELTA)).await;
    }
}

#[cfg(feature = "mpris")]
async fn mpris(
    player: Arc<Player>,
    sender: Sender<Messages>,
) -> mpris_server::Server<crate::player::mpris::Player> {
    mpris_server::Server::new("lowfi", crate::player::mpris::Player { player, sender })
        .await
        .unwrap()
}

pub struct Environment {
    enhancement: bool,
    alternate: bool,
}

impl Environment {
    pub fn ready(alternate: bool) -> eyre::Result<Self> {
        crossterm::execute!(stdout(), Hide)?;

        if alternate {
            crossterm::execute!(stdout(), EnterAlternateScreen, MoveTo(0, 0))?;
        }

        terminal::enable_raw_mode()?;
        let enhancement = terminal::supports_keyboard_enhancement()?;

        if enhancement {
            crossterm::execute!(
                stdout(),
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )?;
        }

        Ok(Self {
            enhancement,
            alternate,
        })
    }

    pub fn cleanup(&self) -> eyre::Result<()> {
        if self.alternate {
            crossterm::execute!(stdout(), LeaveAlternateScreen)?;
        }

        crossterm::execute!(stdout(), Clear(ClearType::FromCursorDown), Show)?;

        if self.enhancement {
            crossterm::execute!(stdout(), PopKeyboardEnhancementFlags)?;
        }

        terminal::disable_raw_mode()?;

        eprintln!("bye! :)");

        Ok(())
    }
}

impl Drop for Environment {
    fn drop(&mut self) {
        // Well, we're dropping it, so it doesn't really matter if there's an error.
        let _ = self.cleanup();
    }
}

/// Initializes the UI, this will also start taking input from the user.
///
/// `alternate` controls whether to use [EnterAlternateScreen] in order to hide
/// previous terminal history.
pub async fn start(player: Arc<Player>, sender: Sender<Messages>, args: Args) -> eyre::Result<()> {
    let environment = Environment::ready(args.alternate)?;

    #[cfg(feature = "mpris")]
    {
        player
            .mpris
            .get_or_init(|| mpris(player.clone(), sender.clone()))
            .await;
    }

    let interface = task::spawn(interface(Arc::clone(&player), args.minimalist));

    input(sender.clone()).await?;
    interface.abort();

    environment.cleanup()?;

    Ok(())
}
