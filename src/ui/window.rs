use std::{
    fmt::Display,
    io::{stdout, Stdout},
};

use crossterm::{
    cursor::{MoveToColumn, MoveUp},
    style::{Print, Stylize as _},
    terminal::{Clear, ClearType},
};
use std::fmt::Write as _;
use unicode_segmentation::UnicodeSegmentation as _;

/// Represents an abstraction for drawing the actual lowfi window itself.
///
/// The main purpose of this struct is just to add the fancy border,
/// as well as clear the screen before drawing.
pub struct Window {
    /// Whether or not to include borders in the output.
    borderless: bool,

    /// The top & bottom borders, which are here since they can be
    /// prerendered, as they don't change every single draw.
    ///
    /// If the option to not include borders is set, these will just be empty [String]s.
    pub(crate) borders: [String; 2],

    /// The inner width of the window.
    width: usize,

    /// The output, currently just an [`Stdout`].
    out: Stdout,
}

impl Window {
    /// Initializes a new [Window].
    ///
    /// * `width` - Inner width of the window.
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

    /// Adds text to the top of the window.
    pub fn display(&mut self, display: impl Display, len: usize) {
        let new = format!("┌─ {} {}─┐", display, "─".repeat(self.width - len - 2));
        self.borders[0] = new;
    }

    /// Renders the window itself, but doesn't actually draw it.
    ///
    /// `testing` just determines whether to add special features
    /// like color resets and carriage returns.
    ///
    /// This returns both the final rendered window and also the full
    /// height of the rendered window.
    pub(crate) fn render(
        &self,
        content: Vec<String>,
        space: bool,
        testing: bool,
    ) -> super::Result<(String, u16)> {
        let linefeed = if testing { "\n" } else { "\r\n" };
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

            let center = if testing { x } else { x.reset().to_string() };
            write!(output, "{padding} {center}{space} {padding}{linefeed}").unwrap();

            output
        });

        // We're doing this because Windows is stupid and can't stand
        // writing to the last line repeatedly.
        #[cfg(windows)]
        let (height, suffix) = (len + 3, linefeed);
        #[cfg(not(windows))]
        let (height, suffix) = (len + 2, "");

        // There's no need for another newline after the main menu content, because it already has one.
        Ok((
            format!(
                "{}{linefeed}{menu}{}{suffix}",
                self.borders[0], self.borders[1]
            ),
            height,
        ))
    }

    /// Actually draws the window, with each element in `content` being on a new line.
    pub fn draw(&mut self, content: Vec<String>, space: bool) -> super::Result<()> {
        let (rendered, height) = self.render(content, space, false)?;

        crossterm::execute!(
            self.out,
            Clear(ClearType::FromCursorDown),
            MoveToColumn(0),
            Print(rendered),
            MoveToColumn(0),
            MoveUp(height - 1),
        )?;

        Ok(())
    }
}
