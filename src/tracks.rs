//! Has all of the structs for managing the state
//! of tracks, as well as downloading them &
//! finding new ones.

use std::{io::Cursor, time::Duration};

use bytes::Bytes;
use inflector::Inflector;
use rodio::{Decoder, Source};
use unicode_width::UnicodeWidthStr;
use url::form_urlencoded;

pub mod list;

/// Just a shorthand for a decoded [Bytes].
pub type DecodedData = Decoder<Cursor<Bytes>>;

/// The TrackInfo struct, which has the name and duration of a track.
///
/// This is not included in [Track] as the duration has to be acquired
/// from the decoded data and not from the raw data.
#[derive(Debug, PartialEq, Clone)]
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
        form_urlencoded::parse(text.as_bytes())
            .map(|(key, val)| [key, val].concat())
            .collect()
    }

    /// Formats a name with [Inflector].
    /// This will also strip the first few numbers that are
    /// usually present on most lofi tracks.
    fn format_name(name: &str) -> String {
        let formatted = Self::decode_url(
            name.split("/")
                .last()
                .unwrap()
                .strip_suffix(".mp3")
                .unwrap(),
        )
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

        String::from(&formatted[skip..])
    }

    /// Creates a new [`TrackInfo`] from a raw name & decoded track data.
    pub fn new(name: String, decoded: &DecodedData) -> Self {
        let name = Self::format_name(&name);

        Self {
            duration: decoded.total_duration(),
            width: name.width(),
            name,
        }
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
    /// This is equivalent to [Track::decode].
    pub fn new(track: Track) -> eyre::Result<Self> {
        let data = Decoder::new(Cursor::new(track.data))?;
        let info = Info::new(track.name, &data);

        Ok(Self { info, data })
    }
}

/// The main track struct, which only includes data & the track name.
pub struct Track {
    /// This name is not formatted, and also includes the month & year of the track.
    pub name: String,

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
