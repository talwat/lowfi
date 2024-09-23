use std::{io::Cursor, time::Duration};

use rodio::{Decoder, OutputStream, Sink, Source};
use tokio::time::sleep;

pub async fn download() {
    
}

pub async fn play(track: &str) -> eyre::Result<()> {
    eprintln!("downloading {}...", track);
    let url = format!("https://lofigirl.com/wp-content/uploads/{}", track);
    let file = Cursor::new(reqwest::get(url).await?.bytes().await?);

    let source = Decoder::new(file).unwrap();

    let (stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();
    sink.append(source);

    eprintln!("playing {}...", track);
    sink.sleep_until_end();

    Ok(())
}

pub async fn random() -> eyre::Result<()> {
    let tracks = include_str!("../data/tracks.txt");
    let tracks: Vec<&str> = tracks.split_ascii_whitespace().collect();

    play(tracks[0]).await?;

    Ok(())
}