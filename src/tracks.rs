use std::{io::Cursor, time::Duration};

use bytes::Bytes;
use inflector::Inflector;
use rand::Rng;
use reqwest::Client;
use rodio::{Decoder, Source};

async fn download(track: &str, client: &Client) -> eyre::Result<Bytes> {
    let url = format!("https://lofigirl.com/wp-content/uploads/{}", track);
    let response = client.get(url).send().await?;
    let data = response.bytes().await?;

    Ok(data)
}

async fn random() -> eyre::Result<&'static str> {
    let tracks = include_str!("../data/tracks.txt");
    let tracks: Vec<&str> = tracks.split_ascii_whitespace().collect();

    let random = rand::thread_rng().gen_range(0..tracks.len());
    let track = tracks[random];

    Ok(track)
}

pub type DecodedData = Decoder<Cursor<Bytes>>;

/// The TrackInfo struct, which has the name and duration of a track.
///
/// This is not included in [Track] as the duration has to be acquired
/// from the decoded data and not from the raw data.
#[derive(Debug, PartialEq, Clone)]
pub struct TrackInfo {
    /// This is a formatted name, so it doesn't include the full path.
    pub name: String,
    pub duration: Option<Duration>,
}

impl TrackInfo {
    fn format_name(name: &'static str) -> String {
        let mut formatted = name
            .split("/")
            .nth(2)
            .unwrap()
            .strip_suffix(".mp3")
            .unwrap()
            .to_title_case();

        let mut skip = 0;
        for character in unsafe { formatted.as_bytes_mut() } {
            if character.is_ascii_digit() {
                skip += 1;
            } else {
                break;
            }
        }

        String::from(&formatted[skip..])
    }

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
    pub info: TrackInfo,
    pub data: DecodedData,
}

impl DecodedTrack {
    pub fn new(track: Track) -> eyre::Result<Self> {
        let data = Decoder::new(Cursor::new(track.data))?;
        let info = TrackInfo::new(track.name, &data);

        Ok(Self { info, data })
    }
}

/// The main track struct, which only includes data & the track name.
pub struct Track {
    pub name: &'static str,
    pub data: Bytes,
}

impl Track {
    /// Fetches and downloads a random track from the tracklist.
    pub async fn random(client: &Client) -> eyre::Result<Self> {
        let name = random().await?;
        let data = download(&name, client).await?;

        Ok(Self { data, name })
    }

    /// This will actually decode and format the track,
    /// returning a [`DecodedTrack`] which can be played
    /// and also has a duration & formatted name.
    pub fn decode(self) -> eyre::Result<DecodedTrack> {
        DecodedTrack::new(self)
    }
}
