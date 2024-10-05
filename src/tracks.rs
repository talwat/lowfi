//! Has all of the structs for managing the state
//! of tracks, as well as downloading them &
//! finding new ones.

use std::{io::Cursor, time::Duration};

use bytes::Bytes;
use inflector::Inflector;
use rand::Rng;
use reqwest::Client;
use rodio::{Decoder, Source};

/// Downloads a raw track, but doesn't decode it.
async fn download(track: &str, client: &Client) -> eyre::Result<Bytes> {
    let url = format!("https://lofigirl.com/wp-content/uploads/{}", track);
    let response = client.get(url).send().await?;
    let data = response.bytes().await?;

    Ok(data)
}

/// Gets a random track from `tracks.txt` and returns it.
fn random() -> &'static str {
    let tracks: Vec<&str> = include_str!("../data/tracks.txt")
        .split_ascii_whitespace()
        .collect();

    let random = rand::thread_rng().gen_range(0..tracks.len());
    tracks[random]
}

/// Just a shorthand for a decoded [Bytes].
pub type DecodedData = Decoder<Cursor<Bytes>>;

/// The TrackInfo struct, which has the name and duration of a track.
///
/// This is not included in [Track] as the duration has to be acquired
/// from the decoded data and not from the raw data.
#[derive(Debug, PartialEq, Clone)]
pub struct TrackInfo {
    /// This is a formatted name, so it doesn't include the full path.
    pub name: String,

    /// The duration of the track, this is an [Option] because there are
    /// cases where the duration of a track is unknown.
    pub duration: Option<Duration>,
}

impl TrackInfo {
    /// Formats a name with [Inflector].
    /// This will also strip the first few numbers that are
    /// usually present on most lofi tracks.
    fn format_name(name: &'static str) -> String {
        let mut formatted = name
            .split("/")
            .nth(2)
            .unwrap()
            .strip_suffix(".mp3")
            .unwrap()
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
    pub fn new(name: &'static str, decoded: &DecodedData) -> Self {
        Self {
            duration: decoded.total_duration(),
            name: Self::format_name(name),
        }
    }
}

/// This struct is seperate from [Track] since it is generated lazily from
/// a track, and not when the track is first downloaded.
pub struct DecodedTrack {
    /// Has both the formatted name and some information from the decoded data.
    pub info: TrackInfo,

    /// The decoded data, which is able to be played by [rodio].
    pub data: DecodedData,
}

impl DecodedTrack {
    /// Creates a new track.
    /// This is equivalent to [Track::decode].
    pub fn new(track: Track) -> eyre::Result<Self> {
        let data = Decoder::new(Cursor::new(track.data))?;
        let info = TrackInfo::new(track.name, &data);

        Ok(Self { info, data })
    }
}

/// The main track struct, which only includes data & the track name.
pub struct Track {
    /// This name is not formatted, and also includes the month & year of the track.
    pub name: &'static str,

    /// The raw data of the track, which is not decoded and
    /// therefore much more memory efficient.
    pub data: Bytes,
}

impl Track {
    /// Fetches and downloads a random track from the tracklist.
    pub async fn random(client: &Client) -> eyre::Result<Self> {
        let name = random();
        let data = download(name, client).await?;

        Ok(Self { data, name })
    }

    /// This will actually decode and format the track,
    /// returning a [`DecodedTrack`] which can be played
    /// and also has a duration & formatted name.
    pub fn decode(self) -> eyre::Result<DecodedTrack> {
        DecodedTrack::new(self)
    }
}
