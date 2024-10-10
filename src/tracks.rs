//! Has all of the structs for managing the state
//! of tracks, as well as downloading them &
//! finding new ones.

use std::{io::Cursor, time::Duration};

use bytes::Bytes;
use inflector::Inflector;
use rand::Rng;
use reqwest::Client;
use rodio::{Decoder, Source};
use url::form_urlencoded;

/// Represents a list of tracks that can be played.
#[derive(Clone)]
pub struct List {
    lines: Vec<String>,
}

impl List {
    /// Gets the base URL of the [List].
    pub fn base(&self) -> &str {
        self.lines[0].trim()
    }

    /// Gets the name of a random track.
    fn random_name(&self) -> String {
        // We're getting from 1 here, since due to how rust vectors work it's
        // slow to drain only a single element from the start, so we can just keep it in.
        let random = rand::thread_rng().gen_range(1..self.lines.len());
        self.lines[random].to_owned()
    }

    /// Downloads a raw track, but doesn't decode it.
    async fn download(&self, track: &str, client: &Client) -> reqwest::Result<Bytes> {
        // If the track has a protocol, then we should ignore the base for it.
        let url = if track.contains("://") {
            track.to_owned()
        } else {
            format!("{}{}", self.base(), track)
        };

        let response = client.get(url).send().await?;
        let data = response.bytes().await?;

        Ok(data)
    }

    /// Fetches and downloads a random track from the [List].
    pub async fn random(&self, client: &Client) -> reqwest::Result<Track> {
        let name = self.random_name();
        let data = self.download(&name, client).await?;

        Ok(Track { name, data })
    }

    /// Parses text into a [List].
    ///
    /// In [List]'s, the first line should be the base URL, followed
    /// by the rest of the tracks.
    ///
    /// Each track will be first appended to the base URL, and then
    /// the result use to download the track.
    ///
    /// lowfi won't put a `/` between the base & track for added flexibility,
    /// so for most cases you should have a trailing `/` in your base url.
    ///
    /// The exception to this is if the track name begins with something like
    /// `https://`, where in that case the base will not be prepended to it.
    pub fn new(text: &str) -> eyre::Result<Self> {
        let lines: Vec<String> = text
            .split_ascii_whitespace()
            .map(|x| x.to_owned())
            .collect();

        Ok(Self { lines })
    }
}

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
        let mut formatted = Self::decode_url(
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

        // SAFETY: All of the track names originate with the `'static` lifetime,
        // SAFETY: so basically this has already been checked.
        for character in unsafe { formatted.as_bytes_mut() } {
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
        Self {
            duration: decoded.total_duration(),
            name: Self::format_name(&name),
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
