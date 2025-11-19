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

use std::{fmt::Debug, io::Cursor, time::Duration};

use bytes::Bytes;
use rodio::{Decoder, Source as _};
use unicode_segmentation::UnicodeSegmentation;

pub mod list;
pub use list::List;
pub mod error;
pub mod format;
pub use error::{Error, Result};

use crate::tracks::error::WithTrackContext;

/// Just a shorthand for a decoded [Bytes].
pub type DecodedData = Decoder<Cursor<Bytes>>;

/// Tracks which are still waiting in the queue, and can't be played yet.
///
/// This means that only the data & track name are included.
#[derive(PartialEq)]
pub struct Queued {
    /// Display name of the track.
    pub display: String,

    /// Full downloadable path/url of the track.
    pub path: String,

    /// The raw data of the track, which is not decoded and
    /// therefore much more memory efficient.
    pub data: Bytes,
}

impl Debug for Queued {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Queued")
            .field("display", &self.display)
            .field("path", &self.path)
            .field("data", &self.data.len())
            .finish()
    }
}

impl Queued {
    /// This will actually decode and format the track,
    /// returning a [`DecodedTrack`] which can be played
    /// and also has a duration & formatted name.
    pub fn decode(self) -> Result<Decoded> {
        Decoded::new(self)
    }

    pub fn new(path: String, data: Bytes, display: Option<String>) -> Result<Self> {
        let display = match display {
            None => self::format::name(&path)?,
            Some(custom) => custom,
        };

        Ok(Self {
            path,
            display,
            data,
        })
    }
}

/// The [`Info`] struct, which has the name and duration of a track.
///
/// This is not included in [Track] as the duration has to be acquired
/// from the decoded data and not from the raw data.
#[derive(Debug, Eq, PartialEq, Clone)]
pub struct Info {
    /// The full downloadable path/url of the track.
    pub path: String,

    /// This is a formatted name, so it doesn't include the full path.
    pub display: String,

    /// This is the *actual* terminal width of the track name, used to make
    /// the UI consistent.
    pub width: usize,

    /// The duration of the track, this is an [Option] because there are
    /// cases where the duration of a track is unknown.
    pub duration: Option<Duration>,
}

impl Info {
    /// Converts the info back into a full track list entry.
    pub fn to_entry(&self) -> String {
        let mut entry = self.path.clone();
        entry.push('!');
        entry.push_str(&self.display);

        entry
    }

    /// Creates a new [`Info`] from decoded data & the queued track.
    pub fn new(decoded: &DecodedData, path: String, display: String) -> Result<Self> {
        Ok(Self {
            duration: decoded.total_duration(),
            width: display.graphemes(true).count(),
            path,
            display,
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
    /// This is equivalent to [`QueuedTrack::decode`].
    pub fn new(track: Queued) -> Result<Self> {
        let (path, display) = (track.path.clone(), track.display.clone());
        let data = Decoder::builder()
            .with_byte_len(track.data.len().try_into().unwrap())
            .with_data(Cursor::new(track.data))
            .build()
            .track(track.display)?;

        let info = Info::new(&data, path, display)?;
        Ok(Self { info, data })
    }
}
