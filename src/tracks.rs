//! Has all of the structs for managing the state
//! of tracks, as well as downloading them &
//! finding new ones.

use std::{io::Cursor, time::Duration};

use bytes::Bytes;
use eyre::OptionExt as _;
use inflector::Inflector as _;
use rodio::{Decoder, Source as _};
use unicode_segmentation::UnicodeSegmentation;
use url::form_urlencoded;

pub mod list;

/// Just a shorthand for a decoded [Bytes].
pub type DecodedData = Decoder<Cursor<Bytes>>;

/// The [`Info`] struct, which has the name and duration of a track.
///
/// This is not included in [Track] as the duration has to be acquired
/// from the decoded data and not from the raw data.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Info {
    /// This is a formatted name, so it doesn't include the full path.
    pub name: String,

    /// This is the *actual* terminal width of the track name, used to make
    /// the UI consistent.
    pub width: usize,

    /// The duration of the track, this is an [Option] because there are
    /// cases where the duration of a track is unknown.
    pub duration: Option<Duration>,
}

impl Info {
    /// Decodes a URL string into normal UTF-8.
    fn decode_url(text: &str) -> String {
        #[expect(
            clippy::tuple_array_conversions,
            reason = "the tuple contains smart pointers, so it's not really practical to use `into()`"
        )]
        form_urlencoded::parse(text.as_bytes())
            .map(|(key, val)| [key, val].concat())
            .collect()
    }

    /// Formats a name with [Inflector].
    /// This will also strip the first few numbers that are
    /// usually present on most lofi tracks.
    fn format_name(name: &str) -> eyre::Result<String> {
        let split = name
            .split('/')
            .last()
            .ok_or_eyre("split is never supposed to return nothing")?;

        let stripped = split.strip_suffix(".mp3").unwrap_or(split);
        let formatted = Self::decode_url(stripped)
            .to_lowercase()
            .to_title_case()
            // Inflector doesn't like contractions...
            // Replaces a few very common ones.
            // TODO: Properly handle these.
            .replace(" S ", "'s ")
            .replace(" T ", "'t ")
            .replace(" D ", "'d ")
            .replace(" Ve ", "'ve ")
            .replace(" Ll ", "'ll ")
            .replace(" Re ", "'re ")
            .replace(" M ", "'m ");

        // This is incremented for each digit in front of the song name.
        let mut skip = 0;

        for character in formatted.as_bytes() {
            if character.is_ascii_digit() {
                skip += 1;
            } else {
                break;
            }
        }

        // If the entire name of the track is a number, then just return it.
        if skip == formatted.len() {
            Ok(formatted)
        } else {
            #[expect(
                clippy::string_slice,
                reason = "We've already checked before that the bound is at an ASCII digit."
            )]
            Ok(String::from(&formatted[skip..]))
        }
    }

    /// Creates a new [`TrackInfo`] from a possibly raw name & decoded track data.
    pub fn new(name: TrackName, decoded: &DecodedData) -> eyre::Result<Self> {
        let name = match name {
            TrackName::Raw(raw) => Self::format_name(&raw)?,
            TrackName::Formatted(formatted) => formatted,
        };

        Ok(Self {
            duration: decoded.total_duration(),
            width: name.graphemes(true).count(),
            name,
        })
    }
}

/// This struct is seperate from [Track] since it is generated lazily from
/// a track, and not when the track is first downloaded.
pub struct Decoded {
    /// Has both the formatted name and some information from the decoded data.
    pub info: Info,

    /// The decoded data, which is able to be played by [rodio].
    pub data: DecodedData,
}

impl Decoded {
    /// Creates a new track.
    /// This is equivalent to [`Track::decode`].
    pub fn new(track: Track) -> eyre::Result<Self> {
        let data = Decoder::new(Cursor::new(track.data))?;
        let info = Info::new(track.name, &data)?;

        Ok(Self { info, data })
    }
}

/// Specifies a track's name, and specifically,
/// whether it has already been formatted or if it
/// is still in it's raw form.
#[derive(Debug, Clone)]
pub enum TrackName {
    /// Pulled straight from the list,
    /// with no splitting done at all.
    Raw(String),

    /// If a track has a custom specified name
    /// in the list, then it should be defined with this variant.
    Formatted(String),
}

/// The main track struct, which only includes data & the track name.
#[derive(Clone)]
pub struct Track {
    /// Name of the track.
    pub name: TrackName,

    /// The raw data of the track, which is not decoded and
    /// therefore much more memory efficient.
    pub data: Bytes,
}

impl Track {
    /// This will actually decode and format the track,
    /// returning a [`DecodedTrack`] which can be played
    /// and also has a duration & formatted name.
    pub fn decode(self) -> eyre::Result<Decoded> {
        Decoded::new(self)
    }
}
