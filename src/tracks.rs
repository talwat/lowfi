use std::{io::Cursor, time::Duration};

use bytes::Bytes;
use rand::Rng;
use reqwest::Client;
use rodio::{Decoder, Source};

pub type Data = Decoder<Cursor<Bytes>>;

async fn download(track: &str, client: &Client) -> eyre::Result<Data> {
    let url = format!("https://lofigirl.com/wp-content/uploads/{}", track);
    let response = client.get(url).send().await?;
    let file = Cursor::new(response.bytes().await?);
    let source = Decoder::new(file)?;

    Ok(source)
}

async fn random() -> eyre::Result<&'static str> {
    let tracks = include_str!("../data/tracks.txt");
    let tracks: Vec<&str> = tracks.split_ascii_whitespace().collect();

    let random = rand::thread_rng().gen_range(0..tracks.len());
    let track = tracks[random];

    Ok(track)
}

#[derive(Debug, PartialEq, Clone, Copy)]
pub struct TrackInfo {
    pub name: &'static str,
    pub duration: Option<Duration>,
}

impl TrackInfo {
    pub fn format_name(&self) -> &'static str {
        self.name.split("/").nth(2).unwrap()
    }
}

/// The main track struct, which includes the actual decoded file
/// as well as some basic information about it.
pub struct Track {
    pub info: TrackInfo,

    /// TODO: Make decoding lazy, since decoded files take up more memory than raw ones.
    pub data: Data,
}

impl Track {
    /// Fetches, downloads, and decodes a random track from the tracklist.
    pub async fn random(client: &Client) -> eyre::Result<Self> {
        let name = random().await?;
        let data = download(&name, client).await?;

        Ok(Self {
            info: TrackInfo {
                name,
                duration: data.total_duration(),
            },
            data,
        })
    }
}
