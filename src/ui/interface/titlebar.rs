use std::fmt::Display;
use unicode_segmentation::UnicodeSegmentation;

/// The titlebar, which is essentially the entire top of the window.
///
/// The struct offers a basic API for displaying messages to it.
pub struct TitleBar {
    /// The actual content of the titlebar.
    pub(crate) content: String,

    /// The width of the titlebar, identical to the width of the parent window.
    width: usize,

    /// Whether to render a bordered or borderless titlebar.
    borderless: bool,
}

impl TitleBar {
    /// Returns a blank default titlebar string for use elsewhere.
    fn blank_content(width: usize, borderless: bool) -> String {
        if borderless {
            String::new()
        } else {
            let middle = "─".repeat(width + 2);
            format!("┌{middle}┐")
        }
    }

    /// Empties the contents of the titlebar.
    pub fn empty(&mut self) {
        self.content = Self::blank_content(self.width, self.borderless);
    }

    /// Adds text to the top of the titlebar.
    pub fn display(&mut self, display: impl Display) {
        let mut display = display.to_string();
        let graphemes = display.graphemes(true);
        let mut len = graphemes.clone().count();
        let inner = self.width - 2;

        if len > inner {
            display = format!("{}...", graphemes.take(inner - 3).collect::<String>());
            len = inner;
        }

        let (prefix, middle, suffix) = if self.borderless {
            ("  ", " ", "  ")
        } else {
            ("┌─", "─", "─┐")
        };

        self.content = format!("{prefix} {display} {}{suffix}", middle.repeat(inner - len));
    }

    pub fn new(width: usize, borderless: bool) -> Self {
        Self {
            content: Self::blank_content(width, borderless),
            width,
            borderless,
        }
    }
}
