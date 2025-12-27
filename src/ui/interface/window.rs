use crate::ui::{self, interface::TitleBar};
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

    /// The titlebar of this window.
    pub titlebar: TitleBar,

    /// The status (bottom) bar of the window, which for now shouldn't change since initialization.
    pub(crate) statusbar: String,

    /// The inner width of the window.
    width: usize,

    /// Whether content items should be automatically padded (spaced).
    spaced: bool,

    /// Whether to cautiously handle ANSI sequences by adding [`style::Attribute::Reset`] generously.
    fancy: bool,
}

impl Window {
    /// Initializes a new [Window].
    ///
    /// * `width` - Inner width of the window.
    /// * `borderless` - Whether to include borders in the window, or not.
    pub fn new(width: usize, borderless: bool, spaced: bool, fancy: bool) -> Self {
        let statusbar = if borderless {
            String::new()
        } else {
            let middle = "─".repeat(width + 2);
            format!("└{middle}┘")
        };

        Self {
            spaced,
            statusbar,
            borderless,
            width,
            fancy,
            titlebar: TitleBar::new(width, borderless),
        }
    }

    /// Renders the window itself, but doesn't actually draw it.
    ///
    /// `testing` just determines whether to add special features
    /// like color resets and carriage returns.
    ///
    /// This returns both the final rendered window and also the full
    /// height of the rendered window.
    pub(crate) fn render(&self, content: Vec<String>) -> ui::Result<(String, u16)> {
        const NEWLINE: &str = "\r\n";
        let len: u16 = content.len().try_into()?;

        // Note that this will have a trailing newline, which we use later.
        let menu: String = content.into_iter().fold(String::new(), |mut output, x| {
            // Horizontal Padding & Border
            let padding = if self.borderless { " " } else { "│" };
            let space = if self.spaced {
                " ".repeat(self.width.saturating_sub(x.graphemes(true).count()))
            } else {
                String::new()
            };

            let center = if self.fancy { x.reset().to_string() } else { x };
            write!(output, "{padding} {center}{space} {padding}{NEWLINE}").unwrap();

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
                "{}{NEWLINE}{menu}{}{suffix}",
                self.titlebar.content, self.statusbar,
            ),
            height,
        ))
    }

    /// Actually draws the window, with each element in `content` being on a new line.
    pub fn draw(
        &mut self,
        mut writer: impl std::io::Write,
        content: Vec<String>,
    ) -> ui::Result<()> {
        let (rendered, height) = self.render(content)?;

        crossterm::execute!(
            writer,
            Clear(ClearType::FromCursorDown),
            MoveToColumn(0),
            Print(rendered),
            MoveToColumn(0),
            MoveUp(height - 1),
        )?;

        Ok(())
    }
}
