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
    event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
    style::{Print, Stylize},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use lazy_static::lazy_static;
use tokio::{sync::mpsc::Sender, task, time::sleep};

use super::{Messages, Player};

mod components;
mod input;

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
    ///
    /// When this is 0, it means that the audio bar shouldn't be displayed.
    /// To make it start counting, you need to set it to 1.
    static ref VOLUME_TIMER: AtomicUsize = AtomicUsize::new(0);
}

/// The code for the terminal interface itself.
///
/// * `minimalist` - All this does is hide the bottom control bar.
async fn interface(player: Arc<Player>, minimalist: bool) -> eyre::Result<()> {
    let mut stdout = std::io::stdout();

    loop {
        // Load `current` once so that it doesn't have to be loaded over and over
        // again by different UI components.
        let current = player.current.load();
        let current = current.as_ref();

        let action = components::action(&player, current, WIDTH);

        let volume = player.sink.volume();
        let percentage = format!("{}%", (volume * 100.0).round().abs());

        let timer = VOLUME_TIMER.load(Ordering::Relaxed);
        let middle = match timer {
            0 => components::progress_bar(&player, current, WIDTH - 16),
            _ => components::audio_bar(volume, &percentage, WIDTH - 17),
        };

        if timer > 0 && timer <= AUDIO_BAR_DURATION {
            // We'll keep increasing the timer until it eventually hits `AUDIO_BAR_DURATION`.
            VOLUME_TIMER.fetch_add(1, Ordering::Relaxed);
        } else if timer > AUDIO_BAR_DURATION {
            // If enough time has passed, we'll reset it back to 0.
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
            stdout,
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

/// The mpris server additionally needs a reference to the player,
/// since it frequently accesses the sink directly as well as
/// the current track.
#[cfg(feature = "mpris")]
async fn mpris(
    player: Arc<Player>,
    sender: Sender<Messages>,
) -> mpris_server::Server<crate::player::mpris::Player> {
    mpris_server::Server::new("lowfi", crate::player::mpris::Player { player, sender })
        .await
        .unwrap()
}

/// Represents the terminal environment, and is used to properly
/// initialize and clean up the terminal.
pub struct Environment {
    /// Whether keyboard enhancements are enabled.
    enhancement: bool,

    /// Whether the terminal is in an alternate screen or not.
    alternate: bool,
}

impl Environment {
    /// This prepares the terminal, returning an [Environment] helpful
    /// for cleaning up afterwards.
    pub fn ready(alternate: bool) -> eyre::Result<Self> {
        let mut lock = stdout().lock();

        crossterm::execute!(lock, Hide)?;

        if alternate {
            crossterm::execute!(lock, EnterAlternateScreen, MoveTo(0, 0))?;
        }

        terminal::enable_raw_mode()?;
        let enhancement = terminal::supports_keyboard_enhancement()?;

        if enhancement {
            crossterm::execute!(
                lock,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )?;
        }

        Ok(Self {
            enhancement,
            alternate,
        })
    }

    /// Uses the information collected from initialization to safely close down
    /// the terminal & restore it to it's previous state.
    pub fn cleanup(&self) -> eyre::Result<()> {
        let mut lock = stdout().lock();

        if self.alternate {
            crossterm::execute!(lock, LeaveAlternateScreen)?;
        }

        crossterm::execute!(lock, Clear(ClearType::FromCursorDown), Show)?;

        if self.enhancement {
            crossterm::execute!(lock, PopKeyboardEnhancementFlags)?;
        }

        terminal::disable_raw_mode()?;

        eprintln!("bye! :)");

        Ok(())
    }
}

impl Drop for Environment {
    /// Just a wrapper for [Environment::cleanup] which ignores any errors thrown.
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

    input::listen(sender.clone()).await?;
    interface.abort();

    environment.cleanup()?;

    Ok(())
}
