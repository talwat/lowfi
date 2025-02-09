//! The module which manages all user interface, including inputs.

use std::{
    fmt::Write,
    io::{stdout, Stdout},
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

/// Represents an abstraction for drawing the actual lowfi window itself.
///
/// The main purpose of this struct is just to add the fancy border,
/// as well as clear the screen before drawing.
pub struct Window {
    /// The top & bottom borders, which are here since they can be
    /// prerendered, as they don't change from window to window.
    ///
    /// [None] if the option to not include windows is set.
    borders: Option<[String; 2]>,

    /// The output, currently just an [`Stdout`].
    out: Stdout,
}

impl Window {
    /// Initializes a new [Window].
    ///
    /// * `width` - Width of the windows.
    /// * `borders` - Whether to include borders in the window, or not.
    pub fn new(width: usize, borders: bool) -> Self {
        let borders = borders.then(|| {
            [
                format!("┌{}┐\r\n", "─".repeat(width + 2)),
                // This one doesn't have a leading \r\n to avoid extra space under the window.
                format!("└{}┘", "─".repeat(width + 2)),
            ]
        });

        Self {
            borders,
            out: stdout(),
        }
    }

    /// Actually draws the window, with each element in `content` being on a new line.
    pub fn draw(&mut self, content: Vec<String>) -> eyre::Result<()> {
        let len = content.len() as u16;

        let menu: String = content.into_iter().fold(String::new(), |mut output, x| {
            if self.borders.is_some() {
                write!(output, "│ {} │\r\n", x.reset()).unwrap();
            } else {
                write!(output, "{}\r\n", x.reset()).unwrap();
            }

            output
        });

        // We're doing this because Windows is stupid and can't stand
        // writing to the last line repeatedly.
        #[cfg(windows)]
        let output_len = len;
        #[cfg(not(windows))]
        let output_len = len - 1;

        let (mut rendered, height) = self.borders.as_ref().map_or_else(
            || (menu.trim().to_owned(), output_len),
            |borders| {
                (
                    format!("{}{}{}", borders[0], menu, borders[1]),
                    output_len + 2,
                )
            },
        );

        // Similar reasoning to the previous comment defining `output_len`.
        #[cfg(windows)]
        rendered.push_str("\r\n");

        crossterm::execute!(
            self.out,
            Clear(ClearType::FromCursorDown),
            MoveToColumn(0),
            Print(rendered),
            MoveToColumn(0),
            MoveUp(height),
        )?;

        Ok(())
    }
}

/// The code for the terminal interface itself.
///
/// * `minimalist` - All this does is hide the bottom control bar.
/// * `borders` - Whether to include borders or not.
/// * `width` - The width of player
async fn interface(
    player: Arc<Player>,
    minimalist: bool,
    borders: bool,
    width: usize,
) -> eyre::Result<()> {
    let mut window = Window::new(width, borders);

    loop {
        // Load `current` once so that it doesn't have to be loaded over and over
        // again by different UI components.
        let current = player.current.load();
        let current = current.as_ref();

        let action = components::action(&player, current, width);

        let volume = player.sink.volume();
        let percentage = format!("{}%", (volume * 100.0).round().abs());

        let timer = VOLUME_TIMER.load(Ordering::Relaxed);
        let middle = match timer {
            0 => components::progress_bar(&player, current, width - 16),
            _ => components::audio_bar(volume, &percentage, width - 17),
        };

        if timer > 0 && timer <= AUDIO_BAR_DURATION {
            // We'll keep increasing the timer until it eventually hits `AUDIO_BAR_DURATION`.
            VOLUME_TIMER.fetch_add(1, Ordering::Relaxed);
        } else {
            // If enough time has passed, we'll reset it back to 0.
            VOLUME_TIMER.store(0, Ordering::Relaxed);
        }

        let controls = components::controls(width);

        let menu = if minimalist {
            vec![action, middle]
        } else {
            vec![action, middle, controls]
        };

        window.draw(menu)?;

        sleep(Duration::from_secs_f32(FRAME_DELTA)).await;
    }
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
    /// Just a wrapper for [`Environment::cleanup`] which ignores any errors thrown.
    fn drop(&mut self) {
        // Well, we're dropping it, so it doesn't really matter if there's an error.
        let _ = self.cleanup();
    }
}

/// Initializes the UI, this will also start taking input from the user.
///
/// `alternate` controls whether to use [`EnterAlternateScreen`] in order to hide
/// previous terminal history.
pub async fn start(player: Arc<Player>, sender: Sender<Messages>, args: Args) -> eyre::Result<()> {
    let environment = Environment::ready(args.alternate)?;
    let interface = task::spawn(interface(
        Arc::clone(&player),
        args.minimalist,
        !args.no_borders,
        21 + args.width.min(32) * 2,
    ));

    input::listen(sender.clone()).await?;
    interface.abort();

    environment.cleanup()?;

    Ok(())
}
