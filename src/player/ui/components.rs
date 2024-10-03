use std::{sync::Arc, time::Duration};

use crossterm::style::Stylize;

use crate::{
    player::{
        ui::{AUDIO_WIDTH, PROGRESS_WIDTH},
        Player,
    },
    tracks::TrackInfo,
};

use super::WIDTH;

/// Small helper function to format durations.
pub fn format_duration(duration: &Duration) -> String {
    let seconds = duration.as_secs() % 60;
    let minutes = duration.as_secs() / 60;

    format!("{:02}:{:02}", minutes, seconds)
}

/// Creates the progress bar, as well as all the padding needed.
pub fn progress_bar(player: &Arc<Player>) -> String {
    let mut duration = Duration::new(0, 0);
    let elapsed = player.sink.get_pos();

    let mut filled = 0;
    if let Some(current) = player.current.load().as_ref() {
        if let Some(x) = current.duration {
            duration = x;

            let elapsed = elapsed.as_secs() as f32 / duration.as_secs() as f32;
            filled = (elapsed * PROGRESS_WIDTH as f32).round() as usize;
        }
    };

    format!(
        " [{}{}] {}/{} ",
        "/".repeat(filled),
        " ".repeat(PROGRESS_WIDTH.saturating_sub(filled)),
        format_duration(&elapsed),
        format_duration(&duration),
    )
}

/// Creates the audio bar, as well as all the padding needed.
pub fn audio_bar(player: &Arc<Player>) -> String {
    let volume = player.sink.volume();

    let audio = (player.sink.volume() * AUDIO_WIDTH as f32).round() as usize;
    let percentage = format!("{}%", (volume * 100.0).round().abs());

    format!(
        " volume: [{}{}] {}{} ",
        "/".repeat(audio),
        " ".repeat(AUDIO_WIDTH.saturating_sub(audio)),
        " ".repeat(4usize.saturating_sub(percentage.len())),
        percentage,
    )
}

/// This represents the main "action" bars state.
enum ActionBar {
    Paused(TrackInfo),
    Playing(TrackInfo),
    Loading,
}

impl ActionBar {
    /// Formats the action bar to be displayed.
    /// The second value is the character length of the result.
    fn format(&self) -> (String, usize) {
        let (word, subject) = match self {
            Self::Playing(x) => ("playing", Some(x.name.clone())),
            Self::Paused(x) => ("paused", Some(x.name.clone())),
            Self::Loading => ("loading", None),
        };

        subject.map_or_else(
            || (word.to_owned(), word.len()),
            |subject| {
                (
                    format!("{} {}", word, subject.clone().bold()),
                    word.len() + 1 + subject.len(),
                )
            },
        )
    }
}

/// Creates the top/action bar, which has the name of the track and it's status.
/// This also creates all the needed padding.
pub fn action(player: &Arc<Player>) -> String {
    let (main, len) = player
        .current
        .load()
        .as_ref()
        .map_or(ActionBar::Loading, |x| {
            let name = (*Arc::clone(x)).clone();
            if player.sink.is_paused() {
                ActionBar::Paused(name)
            } else {
                ActionBar::Playing(name)
            }
        })
        .format();

    if len > WIDTH {
        format!("{}...", &main[..=WIDTH])
    } else {
        format!("{}{}", main, " ".repeat(WIDTH - len))
    }
}
