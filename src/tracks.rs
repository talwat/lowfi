//! Has all of the structs for managing the state
//! of tracks, as well as downloading them & finding new ones.
//!
//! There are several structs which represent the different stages
//! that go on in downloading and playing tracks. The proccess for fetching tracks,
//! and what structs are relevant in each step, are as follows.
//!
//! First Stage, when a track is initially fetched.
//! 1. Raw entry selected from track list.
//! 2. Raw entry split into path & display name.
//! 3. Track data fetched, and [`QueuedTrack`] is created which includes a [`TrackName`] that may be raw.
//!
//! Second Stage, when a track is played.
//! 1. Track data is decoded.
//! 2. [`Info`] created from decoded data.
//! 3. [`Decoded`] made from [`Info`] and the original decoded data.

use std::{io::Cursor, path::Path, time::Duration};

use bytes::Bytes;
use convert_case::{Case, Casing};
use regex::Regex;
use rodio::{Decoder, Source as _};
use unicode_segmentation::UnicodeSegmentation;
use url::form_urlencoded;

pub mod error;
pub mod list;

pub use error::Error;

use crate::tracks::error::Context;
use lazy_static::lazy_static;

/// Just a shorthand for a decoded [Bytes].
pub type DecodedData = Decoder<Cursor<Bytes>>;

/// Specifies a track's name, and specifically,
/// whether it has already been formatted or if it
/// is still in it's raw path form.
#[derive(Debug, Clone)]
pub enum TrackName {
    /// Pulled straight from the list,
    /// with no splitting done at all.
    Raw(String),

    /// If a track has a custom specified name
    /// in the list, then it should be defined with this variant.
    Formatted(String),
}

/// Tracks which are still waiting in the queue, and can't be played yet.
///
/// This means that only the data & track name are included.
pub struct QueuedTrack {
    /// Name of the track, which may be raw.
    pub name: TrackName,

    /// Full downloadable path/url of the track.
    pub full_path: String,

    /// The raw data of the track, which is not decoded and
    /// therefore much more memory efficient.
    pub data: Bytes,
}

impl QueuedTrack {
    /// This will actually decode and format the track,
    /// returning a [`DecodedTrack`] which can be played
    /// and also has a duration & formatted name.
    pub fn decode(self) -> eyre::Result<DecodedTrack, Error> {
        DecodedTrack::new(self)
    }
}

/// The [`Info`] struct, which has the name and duration of a track.
///
/// This is not included in [Track] as the duration has to be acquired
/// from the decoded data and not from the raw data.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Info {
    /// The full downloadable path/url of the track.
    pub full_path: String,

    /// Whether the track entry included a custom name, or not.
    pub custom_name: bool,

    /// This is a formatted name, so it doesn't include the full path.
    pub display_name: String,

    /// This is the *actual* terminal width of the track name, used to make
    /// the UI consistent.
    pub width: usize,

    /// The duration of the track, this is an [Option] because there are
    /// cases where the duration of a track is unknown.
    pub duration: Option<Duration>,
}

lazy_static! {
    static ref MASTER_PATTERNS: [Regex; 5] = [
        // (master), (master v3), (kupla master), (kupla master2)
        Regex::new(r"\s*\(.*?master(?:\s*v?\d+)?\)$").unwrap(),
        // mstr or - mstr or (mstr) — now also matches "mstr v3", "mstr2", etc.
        Regex::new(r"\s*[-(]?\s*mstr(?:\s*v?\d+)?\s*\)?$").unwrap(),
        // - master, master at end without parentheses
        Regex::new(r"\s*[-]?\s*master(?:\s*v?\d+)?$").unwrap(),
        // kupla master1, kupla master v2 (without parentheses or separator)
        Regex::new(r"\s+kupla\s+master(?:\s*v?\d+|\d+)?$").unwrap(),
        // (kupla master) followed by trailing parenthetical numbers, e.g. "... (kupla master) (1)"
        Regex::new(r"\s*\(.*?master(?:\s*v?\d+)?\)(?:\s*\(\d+\))+$").unwrap(),
    ];
}

impl Info {
    /// Decodes a URL string into normal UTF-8.
    fn decode_url(text: &str) -> String {
        // The tuple contains smart pointers, so it's not really practical to use `into()`.
        #[allow(clippy::tuple_array_conversions)]
        form_urlencoded::parse(text.as_bytes())
            .map(|(key, val)| [key, val].concat())
            .collect()
    }

    /// Formats a name with [convert_case].
    ///
    /// This will also strip the first few numbers that are
    /// usually present on most lofi tracks and do some other
    /// formatting operations.
    fn format_name(name: &str) -> eyre::Result<String, Error> {
        let path = Path::new(name);

        let name = path
            .file_stem()
            .and_then(|x| x.to_str())
            .ok_or((name, error::Kind::InvalidName))?;

        let name = Self::decode_url(name).to_lowercase();
        let mut name = name
            .replace("masster", "master")
            .replace("(online-audio-converter.com)", ""); // Some of these names, man...

        // Get rid of "master" suffix with a few regex patterns.
        for regex in MASTER_PATTERNS.iter() {
            name = regex.replace(&name, "").to_string();
        }

        // TODO: Get rid of track numberings beginning with a letter,
        // like B2 or E4.
        let name = name
            .replace("13lufs", "")
            .to_case(Case::Title)
            .replace(" .", "")
            .replace(" Ft ", "ft.")
            .replace("Ft.", "ft.")
            .replace("Feat.", "ft.")
            .replace(" W ", " w/ ");

        // This is incremented for each digit in front of the song name.
        let mut skip = 0;

        for character in name.as_bytes() {
            if character.is_ascii_digit() || *character == b'.' || *character == b')' {
                skip += 1;
            } else {
                break;
            }
        }

        // If the entire name of the track is a number, then just return it.
        if skip == name.len() {
            Ok(name.trim().to_string())
        } else {
            // We've already checked before that the bound is at an ASCII digit.
            #[allow(clippy::string_slice)]
            Ok(String::from(name[skip..].trim()))
        }
    }

    /// Creates a new [`TrackInfo`] from a possibly raw name & decoded data.
    pub fn new(
        name: TrackName,
        full_path: String,
        decoded: &DecodedData,
    ) -> eyre::Result<Self, Error> {
        let (display_name, custom_name) = match name {
            TrackName::Raw(raw) => (Self::format_name(&raw)?, false),
            TrackName::Formatted(custom) => (custom, true),
        };

        Ok(Self {
            duration: decoded.total_duration(),
            width: display_name.graphemes(true).count(),
            full_path,
            custom_name,
            display_name,
        })
    }
}

/// This struct is seperate from [Track] since it is generated lazily from
/// a track, and not when the track is first downloaded.
pub struct DecodedTrack {
    /// Has both the formatted name and some information from the decoded data.
    pub info: Info,

    /// The decoded data, which is able to be played by [rodio].
    pub data: DecodedData,
}

impl DecodedTrack {
    /// Creates a new track.
    /// This is equivalent to [`QueuedTrack::decode`].
    pub fn new(track: QueuedTrack) -> eyre::Result<Self, Error> {
        let data = Decoder::builder()
            .with_byte_len(track.data.len().try_into().unwrap())
            .with_data(Cursor::new(track.data))
            .build()
            .track(track.full_path.clone())?;

        let info = Info::new(track.name, track.full_path, &data)?;

        Ok(Self { info, data })
    }
}
