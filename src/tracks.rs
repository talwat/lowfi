use std::io::Cursor;

use bytes::Bytes;
use rand::Rng;

pub type Data = Cursor<Bytes>;

pub async fn download(track: &str) -> eyre::Result<Data> {
    let url = format!("https://lofigirl.com/wp-content/uploads/{}", track);
    let file = Cursor::new(reqwest::get(url).await?.bytes().await?);
    
    Ok(file)
}

pub async fn random() -> eyre::Result<&'static str> {
    let tracks = include_str!("../data/tracks.txt");
    let tracks: Vec<&str> = tracks.split_ascii_whitespace().collect();

    let random = rand::thread_rng().gen_range(0..tracks.len());
    let track = tracks[random];

    Ok(track)
}