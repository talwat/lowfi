use std::{io::stdout, panic};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event::{KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags},
    terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen},
};

/// Represents the terminal environment, and is used to properly
/// initialize and clean up the terminal.
#[derive(Debug, Clone, Copy)]
pub struct Environment {
    /// Whether keyboard enhancements are enabled.
    enhancement: bool,

    /// Whether the terminal is in an alternate screen or not.
    alternate: bool,
}

impl Environment {
    /// This prepares the terminal, returning an [Environment] helpful
    /// for cleaning up afterwards.
    pub fn ready(alternate: bool) -> super::Result<Self> {
        let mut lock = stdout().lock();

        crossterm::execute!(lock, Hide)?;
        if alternate {
            crossterm::execute!(lock, EnterAlternateScreen, MoveTo(0, 0))?;
        }

        terminal::enable_raw_mode()?;

        let enhancement = terminal::supports_keyboard_enhancement().unwrap_or_default();
        if enhancement {
            crossterm::execute!(
                lock,
                PushKeyboardEnhancementFlags(KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES)
            )?;
        }

        let environment = Self {
            enhancement,
            alternate,
        };

        panic::set_hook(Box::new(move |info| {
            let _ = environment.cleanup(false);
            eprintln!("panic: {info}");
        }));

        Ok(environment)
    }

    /// Uses the information collected from initialization to safely close down
    /// the terminal & restore it to it's previous state.
    pub fn cleanup(&self, elegant: bool) -> super::Result<()> {
        let mut lock = stdout().lock();

        if self.alternate {
            crossterm::execute!(lock, LeaveAlternateScreen)?;
        }

        crossterm::execute!(lock, Clear(ClearType::FromCursorDown), Show)?;

        if self.enhancement {
            crossterm::execute!(lock, PopKeyboardEnhancementFlags)?;
        }

        terminal::disable_raw_mode()?;
        if elegant {
            eprintln!("bye! :)");
        }

        Ok(())
    }
}
