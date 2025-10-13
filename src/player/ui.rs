//! The module which manages all user interface, including inputs.

#![allow(
    clippy::as_conversions,
    clippy::cast_sign_loss,
    clippy::cast_precision_loss,
    clippy::cast_possible_truncation,
    reason = "the ui is full of these because of various layout & positioning aspects, and for a simple music player making all casts safe is not worth the effort"
)]

use std::{
    fmt::Write as _,
    io::{stdout, Stdout},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};

use crate::Args;

#[cfg(feature = "color")]
use crate::ArtStyle;

use crossterm::{
    cursor::{Hide, MoveTo, MoveToColumn, MoveUp, Show},
    event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
    style::{Print, Stylize as _},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

use lazy_static::lazy_static;
use thiserror::Error;
use tokio::{sync::mpsc::Sender, task, time::sleep};
use unicode_segmentation::UnicodeSegmentation;

use super::Player;
use crate::messages::Message;

mod components;
mod input;

#[cfg(feature = "color")]
mod art;

pub mod cover;

/// The error type for the UI, which is used to handle errors that occur
/// while drawing the UI or handling input.
#[derive(Debug, Error)]
pub enum UIError {
    #[error("unable to convert number")]
    Conversion(#[from] std::num::TryFromIntError),

    #[error("unable to write output")]
    Write(#[from] std::io::Error),

    #[error("sending message to backend from ui failed")]
    Communication(#[from] tokio::sync::mpsc::error::SendError<Message>),
}

/// How long the audio bar will be visible for when audio is adjusted.
/// This is in frames.
const AUDIO_BAR_DURATION: usize = 10;

lazy_static! {
    /// The volume timer, which controls how long the volume display should
    /// show up and when it should disappear.
    ///
    /// When this is 0, it means that the audio bar shouldn't be displayed.
    /// To make it start counting, you need to set it to 1.
    static ref VOLUME_TIMER: AtomicUsize = AtomicUsize::new(0);
}

/// Sets the volume timer to one, effectively flashing the audio display in lowfi's UI.
///
/// The amount of frames the audio display is visible for is determined by [`AUDIO_BAR_DURATION`].
pub fn flash_audio() {
    VOLUME_TIMER.store(1, Ordering::Relaxed);
}

/// Represents an abstraction for drawing the actual lowfi window itself.
///
/// The main purpose of this struct is just to add the fancy border,
/// as well as clear the screen before drawing.
pub struct Window {
    /// Whether or not to include borders in the output.
    borderless: bool,

    /// The top & bottom borders, which are here since they can be
    /// prerendered, as they don't change from window to window.
    ///
    /// If the option to not include borders is set, these will just be empty [String]s.
    borders: [String; 2],

    /// The width of the window.
    width: usize,

    /// The output, currently just an [`Stdout`].
    out: Stdout,
}

impl Window {
    /// Initializes a new [Window].
    ///
    /// * `width` - Width of the windows.
    /// * `borderless` - Whether to include borders in the window, or not.
    pub fn new(width: usize, borderless: bool) -> Self {
        let borders = if borderless {
            [String::new(), String::new()]
        } else {
            let middle = "─".repeat(width + 2);

            [format!("┌{middle}┐"), format!("└{middle}┘")]
        };

        Self {
            borders,
            borderless,
            width,
            out: stdout(),
        }
    }

    /// Actually draws the window, with each element in `content` being on a new line.
    pub fn draw(&mut self, content: Vec<String>, space: bool) -> eyre::Result<(), UIError> {
        let len: u16 = content.len().try_into()?;

        // Note that this will have a trailing newline, which we use later.
        let menu: String = content.into_iter().fold(String::new(), |mut output, x| {
            // Horizontal Padding & Border
            let padding = if self.borderless { " " } else { "│" };
            let space = if space {
                " ".repeat(self.width.saturating_sub(x.graphemes(true).count()))
            } else {
                String::new()
            };
            write!(output, "{padding} {}{space} {padding}\r\n", x.reset()).unwrap();

            output
        });

        // We're doing this because Windows is stupid and can't stand
        // writing to the last line repeatedly.
        #[cfg(windows)]
        let (height, suffix) = (len + 2, "\r\n");
        #[cfg(not(windows))]
        let (height, suffix) = (len + 1, "");

        // There's no need for another newline after the main menu content, because it already has one.
        let rendered = format!("{}\r\n{menu}{}{suffix}", self.borders[0], self.borders[1]);

        // Colored UI or cover art is rendering a little bit slower then just B&W UI.
        // It couses some bugs with debug logs, but overall it's fine.
        crossterm::execute!(
            self.out,
            MoveToColumn(0),
            Print(&rendered),
            Clear(ClearType::FromCursorDown),
            MoveToColumn(0),
            MoveUp(height),
        )?;

        Ok(())
    }
}

#[cfg(feature = "color")]
type OptionalArtStyle = Option<ArtStyle>;
#[cfg(not(feature = "color"))]
type OptionalArtStyle = Option<()>;

/// The code for the terminal interface itself.
///
/// * `minimalist` - All this does is hide the bottom control bar.
async fn interface(
    player: Arc<Player>,
    minimalist: bool,
    borderless: bool,
    debug: bool,
    fps: u8,
    width: usize,
    colorize: bool,
    art_style: OptionalArtStyle,
) -> eyre::Result<(), UIError> {
    let mut window = Window::new(width, borderless || debug);
    let mut last_track_path: Option<String> = None;

    #[cfg(feature = "color")]
    let mut cached_art: Option<art::AlbumCover> = None;

    loop {
        // Load `current` once so that it doesn't have to be loaded over and over
        // again by different UI components.
        let current = player.current.load();
        let current_ref = current.as_ref();
        
        // Check for updated colors in background.
        #[cfg(feature = "color")]
        {
            player.update_current_with_colors().await;
        }

        // Update cover art cache if track changed.
        #[cfg(feature = "color")]
        {
            if let Some(style) = &art_style {
                let current_path = current_ref.map(|c| c.full_path.as_str());
                let last_path = last_track_path.as_deref();

                if current_path != last_path {
                    let current_after_update = player.current.load();
                    
                    // Try to create art from cached cover art first, then fallback to audio file.
                    cached_art = if let Some(current) = current_after_update.as_ref() {
                        if let Some(art_url) = &current.art_url {
                            if !art_url.is_empty() && art_url.starts_with("http") {
                                if let Some(cached_data) = player.get_art(current).await {
                                    art::AlbumCover::from_image_data(&cached_data, width, style, colorize)
                                } else {
                                    if let Ok(client) = player.get_bandcamp_client() {
                                        if let Some((_palette, image_data)) = cover::extract_color_palette_and_bytes_from_url_with_client(&client, art_url).await {
                                            art::AlbumCover::from_image_data(&image_data, width, style, colorize)
                                        } else {
                                            None
                                        }
                                    } else {
                                        None
                                    }
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                    } else {
                        None
                    };
                    
                    // Fallback to audio file if URL art failed and art isn't skipped.
                    if cached_art.is_none() && !player.skip_art {
                        cached_art = current_after_update.as_ref().and_then(|_curr| {
                            player.get_current_track_data().and_then(|data| {
                                art::AlbumCover::from_track_data(&data, width, style, colorize)
                            })
                        });
                    }
                    
                    last_track_path = current_path.map(ToString::to_string);
                }
            } else {
                cached_art = None;
                last_track_path = None;
            }
        }

        #[cfg(not(feature = "color"))]
        let _ = (art_style, &last_track_path);

        // Get fresh reference for UI components after potential color update.
        let current_for_ui = player.current.load();
        let current_ref_ui = current_for_ui.as_ref();
        
        let action = components::action(&player, current_ref_ui, width, colorize);
        let volume = player.sink.volume();
        let percentage = format!("{}%", (volume * 100.0).round().abs());
        let palette = current_ref_ui.and_then(|c| c.color_palette.as_ref());
        let timer = VOLUME_TIMER.load(Ordering::Relaxed);
        let middle = match timer {
            0 => components::progress_bar(&player, current_ref_ui, width - 16, colorize),
            _ => components::audio_bar(volume, &percentage, width - 17, palette, colorize),
        };

        if timer > 0 && timer <= AUDIO_BAR_DURATION {
            // We'll keep increasing the timer until it eventually hits `AUDIO_BAR_DURATION`.
            VOLUME_TIMER.fetch_add(1, Ordering::Relaxed);
        } else {
            // If enough time has passed, we'll reset it back to 0.
            VOLUME_TIMER.store(0, Ordering::Relaxed);
        }

        let controls = components::controls(width, palette, colorize);
        let mut menu = Vec::new();

        #[cfg(feature = "color")]
        if let Some(art) = &cached_art {
            menu.extend(art.lines.iter().cloned());
            if !art.lines.is_empty() {
                menu.push(" ".repeat(width));
            }
        }

        match (minimalist, debug, current_ref) {
            (true, _, _) => {
                menu.push(action);
                menu.push(middle);
            }
            (false, true, Some(_x)) => {
                menu.push(action);
                menu.push(middle);
                menu.push(controls);
            }
            _ => {
                menu.push(action);
                menu.push(middle);
                menu.push(controls);
            }
        }

        window.draw(menu, false)?;

        let delta = 1.0 / f32::from(fps);
        sleep(Duration::from_secs_f32(delta)).await;
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
    pub fn ready(alternate: bool) -> eyre::Result<Self, UIError> {
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
    pub fn cleanup(&self) -> eyre::Result<(), UIError> {
        let mut lock = stdout().lock();

        if self.alternate {
            crossterm::execute!(lock, LeaveAlternateScreen)?;
        }

        crossterm::execute!(lock, Clear(ClearType::FromCursorDown), Show)?;

        if self.enhancement {
            crossterm::execute!(lock, PopKeyboardEnhancementFlags)?;
        }

        terminal::disable_raw_mode()?;

        // Nevermind.
        eprintln!("bye! <3");

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
pub async fn start(
    player: Arc<Player>,
    sender: Sender<Message>,
    args: Args,
) -> eyre::Result<(), UIError> {
    let environment = Environment::ready(args.alternate)?;

    #[cfg(feature = "color")]
    let (colorize, art) = (!args.colorless, args.art);
    #[cfg(not(feature = "color"))]
    let (colorize, art): (bool, OptionalArtStyle) = (false, None);

    let total_width = 22 + args.width.min(32) * 2;

    let interface = task::spawn(interface(
        Arc::clone(&player),
        args.minimalist,
        args.borderless,
        args.debug,
        args.fps,
        total_width,
        colorize,
        art,
    ));

    input::listen(sender.clone()).await?;
    interface.abort();

    environment.cleanup()?;

    Ok(())
}
