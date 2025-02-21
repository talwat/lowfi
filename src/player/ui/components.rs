//! Various different individual components that
//! appear in lowfi's UI, like the progress bar.

use std::{ops::Deref as _, sync::Arc, time::Duration};

use crossterm::style::Stylize as _;
use unicode_segmentation::UnicodeSegmentation as _;

use crate::{player::Player, tracks::Info};

/// Small helper function to format durations.
pub fn format_duration(duration: &Duration) -> String {
    let seconds = duration.as_secs() % 60;
    let minutes = duration.as_secs() / 60;

    format!("{minutes:02}:{seconds:02}")
}

/// Creates the progress bar, as well as all the padding needed.
pub fn progress_bar(player: &Player, current: Option<&Arc<Info>>, width: usize) -> String {
    let mut duration = Duration::new(0, 0);
    let elapsed = if current.is_some() {
        player.sink.get_pos()
    } else {
        Duration::new(0, 0)
    };

    let mut filled = 0;
    if let Some(current) = current {
        if let Some(x) = current.duration {
            duration = x;

            let elapsed = elapsed.as_secs() as f32 / duration.as_secs() as f32;
            filled = (elapsed * width as f32).round() as usize;
        }
    };

    format!(
        " [{}{}] {}/{} ",
        "/".repeat(filled),
        " ".repeat(width.saturating_sub(filled)),
        format_duration(&elapsed),
        format_duration(&duration),
    )
}

/// Creates the audio bar, as well as all the padding needed.
pub fn audio_bar(volume: f32, percentage: &str, width: usize) -> String {
    let audio = (volume * width as f32).round() as usize;

    format!(
        " volume: [{}{}] {}{} ",
        "/".repeat(audio),
        " ".repeat(width.saturating_sub(audio)),
        " ".repeat(4usize.saturating_sub(percentage.len())),
        percentage,
    )
}

/// This represents the main "action" bars state.
enum ActionBar {
    /// When the app is currently displaying "paused".
    Paused(Info),

    /// When the app is currently displaying "playing".
    Playing(Info),

    /// When the app is currently displaying "loading".
    Loading,
}

impl ActionBar {
    /// Formats the action bar to be displayed.
    /// The second value is the character length of the result.
    fn format(&self) -> (String, usize) {
        let (word, subject) = match self {
            Self::Playing(x) => ("playing", Some((x.name.clone(), x.width))),
            Self::Paused(x) => ("paused", Some((x.name.clone(), x.width))),
            Self::Loading => ("loading", None),
        };

        subject.map_or_else(
            || (word.to_owned(), word.len()),
            |(subject, len)| (format!("{} {}", word, subject.bold()), word.len() + 1 + len),
        )
    }
}

/// Creates the top/action bar, which has the name of the track and it's status.
/// This also creates all the needed padding.
pub fn action(player: &Player, current: Option<&Arc<Info>>, width: usize) -> String {
    let (main, len) = current
        .map_or(ActionBar::Loading, |info| {
            let info = info.deref().clone();

            if player.sink.is_paused() {
                ActionBar::Paused(info)
            } else {
                ActionBar::Playing(info)
            }
        })
        .format();

    if len > width {
        let chopped: String = main.graphemes(true).take(width + 1).collect();

        format!("{chopped}...")
    } else {
        format!("{}{}", main, " ".repeat(width - len))
    }
}

/// Creates the bottom controls bar, and also spaces it properly.
pub fn controls(width: usize) -> String {
    let controls = [["[s]", "kip"], ["[p]", "ause"], ["[q]", "uit"]];

    let len: usize = controls.concat().iter().map(|x| x.len()).sum();
    let controls = controls.map(|x| format!("{}{}", x[0].bold(), x[1]));

    let mut controls = controls.join(&" ".repeat((width - len) / (controls.len() - 1)));
    // This is needed because changing the above line
    // only works for when the width is even
    controls.push_str(match width % 2 {
        0 => " ",
        _ => "",
    });
    controls
}
