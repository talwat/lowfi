//! Various different individual components that
//! appear in lowfi's UI, like the progress bar.

use std::{ops::Deref as _, sync::Arc, time::Duration};

use crossterm::style::Stylize as _;
use unicode_segmentation::UnicodeSegmentation as _;

use crate::{player::Player, tracks::Info};

#[cfg(feature = "color")]
use crate::player::ui::cover;

/// Small helper function to format durations.
pub fn format_duration(duration: &Duration) -> String {
    let seconds = duration.as_secs() % 60;
    let minutes = duration.as_secs() / 60;

    format!("{minutes:02}:{seconds:02}")
}

/// Helper function to apply color if colorization is enabled.
#[cfg(feature = "color")]
fn maybe_colorize(text: &str, color: Option<[u8; 3]>, colorize: bool) -> String {
    if colorize {
        if let Some(c) = color {
            return cover::colorize(text, c);
        }
    }
    text.to_string()
}

#[cfg(not(feature = "color"))]
fn maybe_colorize(text: &str, _color: Option<[u8; 3]>, _colorize: bool) -> String {
    text.to_string()
}

/// Creates the progress bar, as well as all the padding needed.
pub fn progress_bar(player: &Player, current: Option<&Arc<Info>>, width: usize, colorize: bool) -> String {
    let mut duration = Duration::new(0, 0);
    let elapsed = if current.is_some() {
        player.sink.get_pos()
    } else {
        Duration::new(0, 0)
    };

    let mut filled = 0;
    let palette_color = if let Some(current) = current {
        if let Some(x) = current.duration {
            duration = x;
            
            let elapsed_f = elapsed.as_secs() as f32 / duration.as_secs() as f32;
            filled = (elapsed_f * width as f32).round() as usize;
        }
        current.color_palette.as_ref().and_then(|p| p.get(1)).copied()
    } else {
        None
    };

    let result = format!(
        " [{}{}] {}/{} ",
        "/".repeat(filled),
        " ".repeat(width.saturating_sub(filled)),
        format_duration(&elapsed),
        format_duration(&duration),
    );

    maybe_colorize(&result, palette_color, colorize)
}

/// Creates the audio bar, as well as all the padding needed.
pub fn audio_bar(
    volume: f32,
    percentage: &str,
    width: usize,
    palette: Option<&Vec<[u8; 3]>>,
    colorize: bool,
) -> String {
    let audio = (volume * width as f32).round() as usize;

    let result = format!(
        " volume: [{}{}] {}{} ",
        "/".repeat(audio),
        " ".repeat(width.saturating_sub(audio)),
        " ".repeat(4usize.saturating_sub(percentage.len())),
        percentage,
    );

    let palette_color = palette.and_then(|p| p.get(1)).copied();
    maybe_colorize(&result, palette_color, colorize)
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
    /// Returns the formatted string and its visible character count.
    fn format(&self, star: bool, width: usize, colorize: bool) -> String {
        match self {
            Self::Playing(info) | Self::Paused(info) => {
                Self::format_track_info(matches!(self, Self::Playing(_)), info, star, width, colorize)
            }
            Self::Loading(progress) => {
                let progress_str = format!("{: <2.0}%", (progress * 100.0).min(99.0));
                let text = format!("loading {}", progress_str.clone().bold());
                let visible_len = "loading ".len() + progress_str.len();
                Self::pad_to_width(text, visible_len, width)
            }
            Self::Muted => {
                let msg = "+ to increase volume";
                let text = format!("muted, {}", msg.bold());
                let visible_len = "muted, ".len() + msg.len();
                Self::pad_to_width(text, visible_len, width)
            }
        }
    }

    /// Formats track information with proper truncation and styling.
    fn format_track_info(
        is_playing: bool,
        info: &Info,
        star: bool,
        width: usize,
        colorize: bool,
    ) -> String {
        let status = if is_playing { "playing" } else { "paused" };
        let prefix = format!("{} {}", status, if star { "*" } else { "" });
        let prefix_len = prefix.graphemes(true).count();
        
        let available_width = width.saturating_sub(prefix_len);

        let mut result = prefix;
        
        // Use info.width for accurate character counting.
        let content_len = if info.custom_name {
            // Custom name: check if it contains " by " separator.
            let display_name = &info.display_name;
            
            if let Some((title, artist)) = display_name.split_once(" by ") {
                // Custom name with "by" separator.
                Self::format_title_artist(&mut result, title, artist, info, available_width, colorize)
            } else {
                // Regular custom name: just display it.
                if info.width <= available_width {
                    let styled = Self::style_text(display_name, info, 0, colorize);
                    result.push_str(&styled);
                    info.width
                } else {
                    let truncated = Self::truncate_with_ellipsis_padded(display_name, available_width);
                    let styled = Self::style_text(&truncated, info, 0, colorize);
                    result.push_str(&styled);
                    available_width
                }
            }
        } else if let (Some(title), Some(artist)) = (&info.metadata.title, &info.metadata.artist) {
            // Metadata available: "Title by Artist".
            Self::format_title_artist(&mut result, title, artist, info, available_width, colorize)
        } else {
            // Fallback to display name.
            let display_name = &info.display_name;
            
            if info.width <= available_width {
                let styled = Self::style_text(display_name, info, 0, colorize);
                result.push_str(&styled);
                info.width
            } else {
                let truncated = Self::truncate_with_ellipsis_padded(display_name, available_width);
                let styled = Self::style_text(&truncated, info, 0, colorize);
                result.push_str(&styled);
                available_width
            }
        };

        Self::pad_to_width(result, prefix_len + content_len, width)
    }

    /// Formats "Title by Artist" with proper styling and truncation.
    fn format_title_artist(
        result: &mut String,
        title: &str,
        artist: &str,
        info: &Info,
        available_width: usize,
        colorize: bool,
    ) -> usize {
        let by_separator = " by ";
        let title_len = title.graphemes(true).count();
        let by_len = by_separator.len();
        let artist_len = artist.graphemes(true).count();
        let total_len = title_len + by_len + artist_len;

        if total_len <= available_width {
            result.push_str(&Self::style_text(title, info, 0, colorize));
            result.push_str(&Self::style_separator(by_separator, info, 1, colorize));
            result.push_str(&Self::style_text(artist, info, 2, colorize));
            total_len
        } else if title_len + by_len + 3 <= available_width {
            result.push_str(&Self::style_text(title, info, 0, colorize));
            result.push_str(&Self::style_separator(by_separator, info, 1, colorize));
            let artist_space = available_width - title_len - by_len;
            if artist_space >= 3 { // Ensure ellipsis fits.
                let truncated_artist = Self::truncate_with_ellipsis(artist, artist_space);
                result.push_str(&Self::style_text(&truncated_artist, info, 2, colorize));
                available_width
            } else {
                let truncated_title = Self::truncate_with_ellipsis_padded(title, available_width);
                result.push_str(&Self::style_text(&truncated_title, info, 0, colorize));
                available_width
            }
        } else {
            let truncated_title = Self::truncate_with_ellipsis_padded(title, available_width);
            result.push_str(&Self::style_text(&truncated_title, info, 0, colorize));
            available_width
        }
    }

    /// Applies styling (bold + optional color) to text.
    #[cfg(feature = "color")]
    fn style_text(text: &str, info: &Info, palette_index: usize, colorize: bool) -> String {
        let bold = text.bold().to_string();
        if colorize {
            if let Some(palette) = &info.color_palette {
                if let Some(color) = palette.get(palette_index) {
                    return cover::colorize(&bold, *color);
                }
            }
        }
        bold
    }

    #[cfg(not(feature = "color"))]
    fn style_text(text: &str, _info: &Info, _palette_index: usize, _colorize: bool) -> String {
        text.bold().to_string()
    }

    /// Applies styling to separator (color only, no bold).
    #[cfg(feature = "color")]
    fn style_separator(text: &str, info: &Info, palette_index: usize, colorize: bool) -> String {
        if colorize {
            if let Some(palette) = &info.color_palette {
                if let Some(color) = palette.get(palette_index) {
                    return cover::colorize(text, *color);
                }
            }
        }
        text.to_string()
    }

    #[cfg(not(feature = "color"))]
    fn style_separator(text: &str, _info: &Info, _palette_index: usize, _colorize: bool) -> String {
        text.to_string()
    }

    /// Truncates text and adds ellipsis.
    /// Ensures ellipsis always fits within the specified width.
    fn truncate_with_ellipsis(text: &str, max_width: usize) -> String {
        let ellipsis = "...";
        let ellipsis_len = ellipsis.graphemes(true).count();
        
        if max_width <= ellipsis_len {
            return ellipsis.to_string();
        }

        let truncate_len = max_width - ellipsis_len;
        if truncate_len == 0 {
            return ellipsis.to_string();
        }

        let truncated: String = text.graphemes(true).take(truncate_len).collect();
        format!("{}{}", truncated, ellipsis)
    }

    /// Truncates text and adds ellipsis with padding to fill the width.
    /// Ellipsis is always positioned at the right edge.
    fn truncate_with_ellipsis_padded(text: &str, max_width: usize) -> String {
        let ellipsis = "...";
        let ellipsis_len = ellipsis.graphemes(true).count();
        
        if max_width <= ellipsis_len {
            return ellipsis.to_string();
        }

        let truncate_len = max_width - ellipsis_len;
        if truncate_len == 0 {
            return ellipsis.to_string();
        }

        let truncated: String = text.graphemes(true).take(truncate_len).collect();
        let truncated_len = truncated.graphemes(true).count();
        let padding_len = max_width - truncated_len - ellipsis_len;
        
        format!("{}{}{}", truncated, " ".repeat(padding_len), ellipsis)
    }

    /// Pads text to fill the specified width.
    fn pad_to_width(text: String, visible_len: usize, width: usize) -> String {
        let actual_width = visible_len.min(width);
        if actual_width < width {
            format!("{}{}", text, " ".repeat(width - actual_width))
        } else {
            text
        }
    }
}

/// Creates the top/action bar, which has the name of the track and it's status.
/// This also creates all the needed padding.
pub fn action(player: &Player, current: Option<&Arc<Info>>, width: usize, colorize: bool) -> String {
    current.map_or_else(
        || {
            ActionBar::Loading(player.progress.load(std::sync::atomic::Ordering::Acquire))
                .format(player.bookmarks.bookmarked(), width, colorize)
        },
        |info| {
            let action = if player.sink.volume() < 0.01 {
                ActionBar::Muted
            } else if player.sink.is_paused() {
                ActionBar::Paused(info.deref().clone())
            } else {
                ActionBar::Playing(info.deref().clone())
            };

            action.format(player.bookmarks.bookmarked(), width, colorize)
        },
    )
}

/// Creates the bottom controls bar, and also spaces it properly.
pub fn controls(width: usize, palette: Option<&Vec<[u8; 3]>>, colorize: bool) -> String {
    let control_parts = [
        ("[", "s", "]", "kip"),
        ("[", "p", "]", "ause"),
        (" [", "q", "]", "uit"),
    ];

    let len: usize = control_parts
        .iter()
        .map(|(a, b, c, d)| a.len() + b.len() + c.len() + d.len())
        .sum();

    let palette_color = palette.and_then(|p| p.get(1)).copied();

    let controls_text: Vec<String> = control_parts
        .iter()
        .map(|(open, letter, close, suffix)| {
            let key_part = format!("{}{}{}", open, letter, close).bold().to_string();
            let styled_key = maybe_colorize(&key_part, palette_color, colorize);
            format!("{}{}", styled_key, suffix)
        })
        .collect();

    let spacing = " ".repeat((width.saturating_sub(len)) / (control_parts.len() - 1));
    let mut result = controls_text.join(&spacing);

    let current_len = len + spacing.len() * (control_parts.len() - 1);
    if current_len < width {
        result.push_str(&" ".repeat(width - current_len));
    }

    result
}
