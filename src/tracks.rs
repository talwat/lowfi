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
//! 2. Metadata extracted from audio file tags.
//! 3. Color palette extracted from cover art.
//! 2. [`Info`] created from decoded data, metadata, and color information.
//! 3. [`Decoded`] made from [`Info`] and the original decoded data.

use std::{
    io::{Cursor, Read, Seek, SeekFrom},
    path::Path,
    sync::{Arc, Condvar, Mutex},
    time::Duration,
};

use std::sync::atomic::{AtomicBool, Ordering as AtomicOrdering};

use bytes::Bytes;
use convert_case::{Case, Casing};
use regex::Regex;
use rodio::{Decoder, Source as _};
use unicode_segmentation::UnicodeSegmentation;
use url::form_urlencoded;
use lazy_static::lazy_static;
use lofty::{file::TaggedFileExt, prelude::*, probe::Probe};

pub mod error;
pub mod list;
pub mod cache;
pub mod utils;

#[cfg(feature = "presave")]
pub mod presave;

pub use error::Error;

use crate::tracks::error::Context;
use crate::debug_log;

/// Object-safe trait combining Read and Seek for decoder sources.
pub trait ReadSeek: Read + Seek {}
impl<T: Read + Seek + ?Sized> ReadSeek for T {}

/// A decoder over a boxed reader, allowing both in-memory and streaming sources.
pub type DecodedData = Decoder<Box<dyn ReadSeek + Send + Sync>>;

/// A shared, growable byte buffer that supports concurrent writers and blocking readers.
#[derive(Clone, Default)]
pub struct SharedAudioBuffer(Arc<SharedAudioBufferInner>);

#[derive(Default)]
struct SharedAudioBufferInner {
    data: Mutex<Vec<u8>>,
    ready: Condvar,
    complete: AtomicBool,
}

impl SharedAudioBuffer {
    pub fn new() -> Self { Self::default() }

    pub fn append(&self, chunk: &[u8]) {
        let mut guard = self.0.data.lock().unwrap();
        guard.extend_from_slice(chunk);
        self.0.ready.notify_all();
    }

    pub fn mark_complete(&self) {
        self.0.complete.store(true, AtomicOrdering::Release);
        self.0.ready.notify_all();
    }

    /// Returns a snapshot copy of up to `max_bytes` currently available data.
    pub fn snapshot(&self, max_bytes: usize) -> Bytes {
        let guard = self.0.data.lock().unwrap();
        let take = guard.len().min(max_bytes);
        if take == 0 {
            Bytes::new()
        } else {
            Bytes::copy_from_slice(&guard[..take])
        }
    }

    fn read_exact_range_blocking(&self, start: usize, len: usize, out: &mut [u8]) -> std::io::Result<usize> {
        let mut read_total = 0;
        let mut start_idx = start;
        while read_total < len {
            let mut guard = self.0.data.lock().unwrap();
            // Wait until enough data exists or writer completed.
            while guard.len() < start_idx + (len - read_total) && !self.0.complete.load(AtomicOrdering::Acquire) {
                guard = self.0.ready.wait(guard).unwrap();
            }

            let available = guard.len().saturating_sub(start_idx);
            if available == 0 {
                // No more data and writer completed.
                if self.0.complete.load(AtomicOrdering::Acquire) {
                    return Ok(read_total);
                }
                continue;
            }

            let to_copy = available.min(len - read_total);
            out[read_total..read_total + to_copy]
                .copy_from_slice(&guard[start_idx..start_idx + to_copy]);
            read_total += to_copy;
            start_idx += to_copy;
        }
        Ok(read_total)
    }
}

/// A blocking reader over a `SharedAudioBuffer` that implements `Read + Seek`.
pub struct GrowingReader {
    buffer: SharedAudioBuffer,
    position: usize,
}

impl GrowingReader {
    pub fn new(buffer: SharedAudioBuffer) -> Self { Self { buffer, position: 0 } }
}

impl Read for GrowingReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let read = self.buffer.read_exact_range_blocking(self.position, buf.len(), buf)?;
        self.position += read;
        Ok(read)
    }
}

impl Seek for GrowingReader {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let new_pos: i128 = match pos {
            SeekFrom::Start(n) => n as i128,
            SeekFrom::Current(n) => self.position as i128 + n as i128,
            SeekFrom::End(_n) => {
                // Unknown total length; treat as unsupported.
                return Err(std::io::Error::new(std::io::ErrorKind::Unsupported, "SeekFrom::End not supported for streaming buffer"));
            }
        };

        if new_pos < 0 {
            return Err(std::io::Error::new(std::io::ErrorKind::InvalidInput, "negative seek"));
        }

        self.position = new_pos as usize;
        Ok(self.position as u64)
    }
}

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
#[derive(Clone)]
pub struct QueuedTrack {
    /// Name of the track, which may be raw.
    pub name: TrackName,

    /// Full downloadable path/url of the track.
    pub full_path: String,
    /// Underlying data storage for the track: full bytes or a streaming buffer.
    pub data: TrackData,

    /// Cover art URL for UI coloring.
    pub art_url: Option<String>,
}

/// Data backing for a queued track.
#[derive(Clone)]
pub enum TrackData {
    Full(Bytes),
    Streaming(SharedAudioBuffer),
}

impl QueuedTrack {
    /// This will actually decode and format the track,
    /// returning a [`DecodedTrack`] which can be played
    /// and also has a duration & formatted name.
    pub fn decode(self) -> eyre::Result<DecodedTrack, Error> {
        DecodedTrack::new(self)
    }
}

/// Metadata extracted from audio file tags.
#[derive(Debug, Clone, Default)]
pub struct Metadata {
    pub title: Option<String>,
    pub artist: Option<String>,
}

/// The [`Info`] struct, which has the name, duration, and metadata of a track.
///
/// This is not included in [Track] as the duration has to be acquired
/// from the decoded data and not from the raw data.
#[derive(Debug, Clone)]
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

    /// Metadata extracted from the audio file.
    pub metadata: Metadata,

    /// Color palette extracted from cover art.
    pub color_palette: Option<Vec<[u8; 3]>>,

    /// Cover art URL for lazy color extraction.
    pub art_url: Option<String>,

    /// Raw audio data..
    pub raw_data: Option<Arc<Bytes>>,
}

impl PartialEq for Info {
    fn eq(&self, other: &Self) -> bool {
        self.full_path == other.full_path
            && self.custom_name == other.custom_name
            && self.display_name == other.display_name
            && self.duration == other.duration
    }
}

impl Eq for Info {}

lazy_static! {
    static ref MASTER_PATTERNS: [Regex; 5] = [
        // (master), (master v2)
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
    static ref ID_PATTERN: Regex = Regex::new(r"^[a-z]\d[ .]").unwrap();
}

impl Info {
    /// Converts the info back into a full track list entry.
    pub fn to_entry(&self) -> String {
        let mut entry = self.full_path.clone();

        if self.custom_name {
            entry.push('!');
            entry.push_str(&self.display_name);
        }
        
        // Append art URL if available
        if let Some(url) = &self.art_url {
            if !url.is_empty() {
                entry.push('!');
                entry.push_str(url);
            }
        }

        entry
    }

    /// Decodes a URL string into normal UTF-8.
    fn decode_url(text: &str) -> String {
        // The tuple contains smart pointers, so it's not really practical to use `into()`.
        #[allow(clippy::tuple_array_conversions)]
        form_urlencoded::parse(text.as_bytes())
            .map(|(key, val)| [key, val].concat())
            .collect()
    }

    /// Extracts metadata from audio file.
    fn extract_metadata(data: &Bytes) -> Metadata {
        debug_log!("tracks.rs - extract_metadata: start extracting");
        let cursor = Cursor::new(data.clone());

        let Ok(probe) = Probe::new(cursor).guess_file_type() else {
            debug_log!("tracks.rs - extract_metadata: guess_file_type failed");
            return Metadata::default();
        };

        let Ok(tagged_file) = probe.read() else {
            debug_log!("tracks.rs - extract_metadata: read tagged_file failed");
            return Metadata::default();
        };

        let Some(tag) = tagged_file.primary_tag().or_else(|| tagged_file.first_tag()) else {
            debug_log!("tracks.rs - extract_metadata: no tags found");
            return Metadata::default();
        };

        let title = tag.title().as_deref().map(ToString::to_string);
        let artist = tag.artist().as_deref().map(ToString::to_string);
        debug_log!("tracks.rs - extract_metadata: title_present={} artist_present={}", title.is_some(), artist.is_some());

        Metadata { title, artist }
    }

    /// Formats a name with [`convert_case`].
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
            .replace("(online-audio-converter.com)", "") // Some of these names, man...
            .replace('_', " ");

        // Get rid of "master" suffix with a few regex patterns.
        for regex in MASTER_PATTERNS.iter() {
            name = regex.replace(&name, "").to_string();
        }

        name = ID_PATTERN.replace(&name, "").to_string();

        let name = name
            .replace("13lufs", "")
            .to_case(Case::Title)
            .replace(" .", "")
            .replace(" Ft ", " ft. ")
            .replace("Ft.", "ft.")
            .replace("Feat.", "ft.")
            .replace(" W ", " w/ ");

        // This is incremented for each digit in front of the song name.
        let mut skip = 0;

        for character in name.as_bytes() {
            if character.is_ascii_digit()
                || *character == b'.'
                || *character == b')'
                || *character == b'('
            {
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
        data: Option<&Bytes>,
        art_url: Option<&str>,
    ) -> eyre::Result<Self, Error> {
        let (metadata, color_palette, raw_data) = if let Some(d) = data {
            // Try to extract color palette from audio file first
            let palette = crate::player::ui::cover::extract_color_palette(d);
            (Self::extract_metadata(d), palette, Some(Arc::new(d.clone())))
        } else {
            (Metadata::default(), None, None)
        };

        let (display_name, custom_name) = match name {
            TrackName::Formatted(custom) => (custom, true),
            TrackName::Raw(raw) => {
                // Prefer metadata if available
                if let (Some(ref title), Some(ref artist)) = (&metadata.title, &metadata.artist) {
                    (format!("{} by {}", title, artist), false)
                } else if let Some(ref title) = metadata.title {
                    (title.to_string(), false)
                } else {
                    (Self::format_name(&raw)?, false)
                }
            }
        };

        let width = display_name.graphemes(true).count();

        Ok(Self {
            duration: decoded.total_duration(),
            full_path,
            custom_name,
            display_name,
            width,
            metadata,
            color_palette,
            art_url: art_url.map(ToString::to_string),
            raw_data,
        })
    }

    /// Creates `Info` for streaming tracks without raw bytes available.
    pub fn new_streaming(name: TrackName, full_path: String, decoded: &DecodedData) -> eyre::Result<Self, Error> {
        Self::new(name, full_path, decoded, None, None)
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
        match track.data {
            TrackData::Full(bytes) => {
                let reader: Box<dyn ReadSeek + Send + Sync> = Box::new(Cursor::new(bytes.clone()));
                let data: DecodedData = Decoder::new(reader).track(track.full_path.clone())?;
                let info = Info::new(track.name, track.full_path, &data, Some(&bytes), track.art_url.as_deref())?;
                Ok(Self { info, data })
            }
            TrackData::Streaming(buffer) => {
                // Use a blocking reader over the shared buffer.
                let snapshot_src = buffer.clone();
                let reader: Box<dyn ReadSeek + Send + Sync> = Box::new(GrowingReader::new(buffer));
                let data: DecodedData = Decoder::new(reader).track(track.full_path.clone())?;
                // Try to extract metadata/colors from currently available bytes.
                let snapshot = snapshot_src.snapshot(1024 * 1024); // up to 1MB
                let data_opt = if snapshot.is_empty() { None } else { Some(snapshot) };
                let info = if let Some(bytes) = data_opt.as_ref() {
                    Info::new(track.name, track.full_path, &data, Some(bytes), track.art_url.as_deref())?
                } else {
                    Info::new_streaming(track.name, track.full_path, &data)?
                };
                Ok(Self { info, data })
            }
        }
    }
}
