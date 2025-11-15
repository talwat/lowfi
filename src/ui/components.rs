//! Various different individual components that
//! appear in lowfi's UI, like the progress bar.

use std::time::Duration;

use crossterm::style::Stylize as _;
use unicode_segmentation::UnicodeSegmentation as _;

use crate::{tracks, ui};

/// Small helper function to format durations.
pub fn format_duration(duration: &Duration) -> String {
    let seconds = duration.as_secs() % 60;
    let minutes = duration.as_secs() / 60;

    format!("{minutes:02}:{seconds:02}")
}

/// Creates the progress bar, as well as all the padding needed.
pub fn progress_bar(state: &ui::State, width: usize) -> String {
    let mut duration = Duration::new(0, 0);
    let elapsed = if state.track.is_some() {
        state.sink.get_pos()
    } else {
        Duration::new(0, 0)
    };

    let mut filled = 0;
    if let Some(current) = &state.track {
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
pub fn audio_bar(width: usize, volume: f32, percentage: &str) -> String {
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
    /// When the app is paused.
    Paused(tracks::Info),

    /// When the app is playing.
    Playing(tracks::Info),

    /// When the app is loading.
    Loading(u8),

    /// When the app is muted.
    Muted,
}

impl ActionBar {
    /// Formats the action bar to be displayed.
    /// The second value is the character length of the result.
    fn format(&self, star: bool) -> (String, usize) {
        let (word, subject) = match self {
            Self::Playing(x) => ("playing", Some((x.display.clone(), x.width))),
            Self::Paused(x) => ("paused", Some((x.display.clone(), x.width))),
            Self::Loading(progress) => {
                let progress = format!("{: <2.0}%", progress.min(&99));

                ("loading", Some((progress, 3)))
            }
            Self::Muted => {
                let msg = "+ to increase volume";

                ("muted,", Some((String::from(msg), msg.len())))
            }
        };

        subject.map_or_else(
            || (word.to_owned(), word.len()),
            |(subject, len)| {
                (
                    format!("{} {}{}", word, if star { "*" } else { "" }, subject.bold()),
                    word.len() + 1 + len + usize::from(star),
                )
            },
        )
    }
}

/// Creates the top/action bar, which has the name of the track and it's status.
/// This also creates all the needed padding.
pub fn action(state: &ui::State, width: usize) -> String {
    let (main, len) = state
        .track
        .as_ref()
        .map_or_else(
            || {
                ActionBar::Loading(
                    state
                        .progress
                        .load(std::sync::atomic::Ordering::Acquire)
                        .into(),
                )
            },
            |info| {
                if state.sink.volume() < 0.01 {
                    return ActionBar::Muted;
                }

                let info = info.clone();
                if state.sink.is_paused() {
                    ActionBar::Paused(info)
                } else {
                    ActionBar::Playing(info)
                }
            },
        )
        .format(state.bookmarked);

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
