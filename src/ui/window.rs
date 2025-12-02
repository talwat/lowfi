use std::io::{stdout, Stdout};

use crossterm::{
    cursor::{MoveToColumn, MoveUp},
    style::{Print, Stylize},
    terminal::{Clear, ClearType},
};
use std::fmt::Write;
use unicode_segmentation::UnicodeSegmentation;

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
    pub(crate) borders: [String; 2],

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
    pub fn draw(&mut self, content: Vec<String>, space: bool) -> super::Result<()> {
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
