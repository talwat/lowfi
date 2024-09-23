use std::io::Cursor;

use bytes::Bytes;
use rand::Rng;
use rodio::{Decoder, OutputStream, Sink};

pub async fn download(track: &str) -> eyre::Result<Decoder<Cursor<Bytes>>> {
    let url = format!("https://lofigirl.com/wp-content/uploads/{}", track);
    let file = Cursor::new(reqwest::get(url).await?.bytes().await?);
    let source = Decoder::new(file).unwrap();
    
    Ok(source)
}

pub async fn play(source: Decoder<Cursor<Bytes>>) -> eyre::Result<()> {
    let (stream, stream_handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&stream_handle)?;
    sink.append(source);

    sink.sleep_until_end();

    Ok(())
}

pub async fn random() -> eyre::Result<()> {
    let tracks = include_str!("../data/tracks.txt");
    let tracks: Vec<&str> = tracks.split_ascii_whitespace().collect();

    let random = rand::thread_rng().gen_range(0..tracks.len());
    let track = tracks[random];

    eprintln!("downloading {}...", track);
    let source = download(track).await?;

    eprintln!("playing {}...", track);
    play(source).await?;

    Ok(())
}