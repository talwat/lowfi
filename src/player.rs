use std::{collections::VecDeque, sync::Arc, time::Duration};

use reqwest::Client;
use rodio::{source::SineWave, Decoder, OutputStream, OutputStreamHandle, Sink, Source};
use tokio::{
    sync::RwLock,
    task, time::sleep,
};

/// The amount of songs to buffer up.
const BUFFER_SIZE: usize = 5;

use crate::tracks::Track;

/// Main struct responsible for queuing up tracks.
///
/// Internally tracks are stored in an [Arc],
/// so it's fine to clone this struct.
#[derive(Debug, Clone)]
pub struct Queue {
    tracks: Arc<RwLock<VecDeque<Track>>>,
}

unsafe impl Send for Queue {}
unsafe impl Sync for Queue {}

impl Queue {
    pub async fn new() -> Self {
        Self {
            tracks: Arc::new(RwLock::new(VecDeque::with_capacity(5))),
        }
    }

    /// This will play the next track, as well as refilling the buffer in the background.
    pub async fn next(&self, client: &Client) -> eyre::Result<Track> {
        // This refills the queue in the background.
        task::spawn({
            let client = client.clone();
            let tracks = self.tracks.clone();

            async move {
                while tracks.read().await.len() < BUFFER_SIZE {
                    let track = Track::random(&client).await.unwrap();
                    tracks.write().await.push_back(track);
                }
            }
        });

        let track = self.tracks.write().await.pop_front();
        let track = match track {
            Some(x) => x,
            // If the queue is completely empty, then fallback to simply getting a new track.
            // This is relevant particularly at the first song.
            None => Track::random(client).await?,
        };

        Ok(track)
    }

    pub async fn play(self, sink: Sink) -> eyre::Result<()> {
        let client = Client::builder().build()?;
        let sink = Arc::new(sink);

        loop {
            sink.stop();

            let track = self.next(&client).await?;
            sink.append(Decoder::new(track.data)?);

            let sink = sink.clone();
            task::spawn_blocking(move || sink.sleep_until_end()).await?;
        }
    }
}

pub async fn play() -> eyre::Result<()> {
    let queue = Queue::new().await;
    let (stream, handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&handle)?;

    let audio = task::spawn(queue.clone().play(sink));

    crossterm::terminal::enable_raw_mode()?;

    'a: loop {
        match crossterm::event::read()? {
            crossterm::event::Event::Key(event) => match event.code {
                crossterm::event::KeyCode::Char(x) => {
                    if x == 'q' {
                        break 'a;
                    }
                }
                _ => (),
            },
            _ => (),
        }
    }

    audio.abort();
    crossterm::terminal::disable_raw_mode()?;
    Ok(())
}
