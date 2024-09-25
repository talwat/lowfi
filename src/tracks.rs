use std::io::Cursor;

use bytes::Bytes;
use rand::Rng;
use reqwest::Client;

pub type Data = Cursor<Bytes>;

async fn download(track: &str, client: &Client) -> eyre::Result<Data> {
    let url = format!("https://lofigirl.com/wp-content/uploads/{}", track);
    let response = client.get(url).send().await?;
    let file = Cursor::new(response.bytes().await?);

    Ok(file)
}

async fn random() -> eyre::Result<&'static str> {
    let tracks = include_str!("../data/tracks.txt");
    let tracks: Vec<&str> = tracks.split_ascii_whitespace().collect();

    let random = rand::thread_rng().gen_range(0..tracks.len());
    let track = tracks[random];

    Ok(track)
}

#[derive(Debug, PartialEq)]
pub struct Track {
    pub name: &'static str,
    pub data: Data,
}

impl Track {
    pub async fn random(client: &Client) -> eyre::Result<Self> {
        let name = random().await?;
        let data = download(&name, client).await?;

        Ok(Self { name, data })
    }
}
