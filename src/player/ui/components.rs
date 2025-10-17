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
    /// When the app is paused.
    Paused(Info),

    /// When the app is playing.
    Playing(Info),

    /// When the app is loading.
    Loading(f32),

    /// When the app is muted.
    Muted,
}

impl ActionBar {
    /// Formats the action bar to be displayed.
    fn format(&self, star: bool, width: usize, show_artist: bool) -> String {
        match self {
            Self::Playing(info) | Self::Paused(info) => {
                Self::format_track_info(matches!(self, Self::Playing(_)), info, star, width, show_artist)
            }
            Self::Loading(progress) => {
                let progress_str = format!("{: <2.0}%", (progress * 100.0).min(99.0));
                Self::format_simple("loading", &progress_str, width)
            }
            Self::Muted => {
                Self::format_simple("muted,", "+ to increase volume", width)
            }
        }
    }

    /// Formats simple status messages.
    fn format_simple(prefix: &str, content: &str, width: usize) -> String {
        let text = format!("{} {}", prefix, content.bold());
        let visible_len = prefix.len() + 1 + content.len();
        Self::pad_to_width(text, visible_len, width)
    }

    /// Formats track information with proper truncation and styling.
    fn format_track_info(is_playing: bool, info: &Info, star: bool, width: usize, show_artist: bool) -> String {
        let status = if is_playing { "playing" } else { "paused" };
        let prefix = format!("{} {}", status, if star { "*" } else { "" });
        let prefix_len = prefix.graphemes(true).count();
        let available_width = width.saturating_sub(prefix_len);
        let mut result = prefix;
        
        let content_len = if show_artist {
            if let Some((title, artist)) = Self::get_title_artist(info) {
                Self::format_title_artist(&mut result, title, artist, available_width)
            } else {
                Self::format_display_name(&mut result, &info.display_name, info.width, available_width)
            }
        } else {
            // When show_artist is false, show only title (not "Title by Artist")
            if let Some(title) = &info.title {
                let title_width = title.graphemes(true).count();
                Self::format_display_name(&mut result, title, title_width, available_width)
            } else {
                Self::format_display_name(&mut result, &info.display_name, info.width, available_width)
            }
        };

        Self::pad_to_width(result, prefix_len + content_len, width)
    }

    /// Gets title and artist from info, handling both custom names and separate fields.
    fn get_title_artist(info: &Info) -> Option<(&str, &str)> {
        if info.custom_name {
            info.display_name.split_once(" by ")
        } else {
            info.title.as_deref().zip(info.artist.as_deref())
        }
    }

    /// Formats display name with truncation if needed.
    fn format_display_name(result: &mut String, display_name: &str, width: usize, available_width: usize) -> usize {
        if width <= available_width {
            result.push_str(&display_name.bold().to_string());
            width
        } else {
            let truncated = Self::truncate_with_ellipsis_padded(display_name, available_width);
            result.push_str(&truncated.bold().to_string());
            available_width
        }
    }

    /// Formats "Title by Artist" with proper styling and truncation.
    fn format_title_artist(result: &mut String, title: &str, artist: &str, available_width: usize) -> usize {
        const BY_SEPARATOR: &str = " by ";
        let title_len = title.graphemes(true).count();
        let by_len = BY_SEPARATOR.len();
        let artist_len = artist.graphemes(true).count();
        let total_len = title_len + by_len + artist_len;

        if total_len <= available_width {
            result.push_str(&title.bold().to_string());
            result.push_str(BY_SEPARATOR);
            result.push_str(&artist.bold().to_string());
            total_len
        } else if title_len + by_len + 3 <= available_width {
            result.push_str(&title.bold().to_string());
            result.push_str(BY_SEPARATOR);
            let artist_space = available_width - title_len - by_len;
            if artist_space >= 3 {
                let truncated_artist = Self::truncate_with_ellipsis(artist, artist_space);
                result.push_str(&truncated_artist.bold().to_string());
                available_width
            } else {
                Self::format_truncated_title(result, title, available_width)
            }
        } else {
            Self::format_truncated_title(result, title, available_width)
        }
    }

    /// Formats truncated title.
    fn format_truncated_title(result: &mut String, title: &str, available_width: usize) -> usize {
        let truncated_title = Self::truncate_with_ellipsis_padded(title, available_width);
        result.push_str(&truncated_title.bold().to_string());
        available_width
    }

    /// Truncates text and adds ellipsis.
    fn truncate_with_ellipsis(text: &str, max_width: usize) -> String {
        const ELLIPSIS: &str = "...";
        let ellipsis_len = ELLIPSIS.graphemes(true).count();
        
        if max_width <= ellipsis_len {
            return ELLIPSIS.to_string();
        }

        let truncate_len = max_width - ellipsis_len;
        if truncate_len == 0 {
            return ELLIPSIS.to_string();
        }

        let truncated: String = text.graphemes(true).take(truncate_len).collect();
        format!("{}{}", truncated, ELLIPSIS)
    }

    /// Truncates text and adds ellipsis with padding to fill the width.
    fn truncate_with_ellipsis_padded(text: &str, max_width: usize) -> String {
        const ELLIPSIS: &str = "...";
        let ellipsis_len = ELLIPSIS.graphemes(true).count();
        
        if max_width <= ellipsis_len {
            return ELLIPSIS.to_string();
        }

        let truncate_len = max_width - ellipsis_len;
        if truncate_len == 0 {
            return ELLIPSIS.to_string();
        }

        let truncated: String = text.graphemes(true).take(truncate_len).collect();
        let truncated_len = truncated.graphemes(true).count();
        let padding_len = max_width - truncated_len - ellipsis_len;
        
        format!("{}{}{}", truncated, " ".repeat(padding_len), ELLIPSIS)
    }

    /// Pads text to fill the specified width.
    fn pad_to_width(text: String, visible_len: usize, width: usize) -> String {
        if visible_len < width {
            format!("{}{}", text, " ".repeat(width - visible_len))
                    } else {
            text
        }
    }
}

/// Creates the top/action bar, which has the name of the track and it's status.
/// This also creates all the needed padding.
pub fn action(player: &Player, current: Option<&Arc<Info>>, width: usize) -> String {
    current.map_or_else(
        || {
            ActionBar::Loading(player.progress.load(std::sync::atomic::Ordering::Acquire))
                .format(player.bookmarks.bookmarked(), width, player.show_artist())
        },
            |info| {
            let action = if player.sink.volume() < 0.01 {
                ActionBar::Muted
            } else if player.sink.is_paused() {
                ActionBar::Paused(info.deref().clone())
                } else {
                ActionBar::Playing(info.deref().clone())
            };

            action.format(player.bookmarks.bookmarked(), width, player.show_artist())
        },
    )
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
