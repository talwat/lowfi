use std::{collections::VecDeque, sync::Arc, time::Duration};

use rodio::{Decoder, OutputStream, Sink, Source};
use tokio::{
    sync::{mpsc, RwLock},
    task,
    time::sleep,
};

/// The amount of songs to buffer up.
const BUFFER_SIZE: usize = 5;

use crate::tracks::{self};

#[derive(Debug, PartialEq)]
pub struct Track {
    pub name: &'static str,
    pub data: tracks::Data,
}

impl Track {
    pub async fn random() -> eyre::Result<Self> {
        let name = tracks::random().await?;
        let data = tracks::download(&name).await?;

        Ok(Self { name, data })
    }
}

pub struct Queue {
    tracks: Arc<RwLock<VecDeque<Track>>>,
}

impl Queue {
    pub async fn new() -> Self {
        Self {
            tracks: Arc::new(RwLock::new(VecDeque::with_capacity(5))),
        }
    }

    pub async fn get(&self) -> eyre::Result<Track> {
        // This refills the queue in the background.
        let tracks = self.tracks.clone();
        task::spawn(async move {
            while tracks.read().await.len() < BUFFER_SIZE {
                let track = Track::random().await.unwrap();
                tracks.write().await.push_back(track);
            }
        });

        let track = self.tracks.write().await.pop_front();
        let track = match track {
            Some(x) => x,
            // If the queue is completely empty, then fallback to simply getting a new track.
            // This is relevant particularly at the first song.
            None => Track::random().await?,
        };
        
        Ok(track)
    }
}

pub async fn play() -> eyre::Result<()> {
    let queue = Queue::new().await;

    let (stream, handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&handle).unwrap();

    crossterm::terminal::enable_raw_mode()?;

    // TODO: Reintroduce the Player struct and seperate
    // input/display from song playing so that quits & skips
    // are instant.
    loop {
        sink.stop();

        let track = queue.get().await?;
        sink.append(Decoder::new(track.data)?);

        match crossterm::event::read()? {
            crossterm::event::Event::Key(event) => {
                match event.code {
                    crossterm::event::KeyCode::Char(x) => {
                        if x == 's' {
                            continue;
                        } else if x == 'q' {
                            break;
                        }
                    }
                    _ => ()
                }
            },
            _ => ()
        }
        
        sleep(Duration::from_secs(2)).await;
    }

    crossterm::terminal::disable_raw_mode()?;
    sink.stop();
    drop(stream);
    Ok(())
}
