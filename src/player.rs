use std::{collections::VecDeque, sync::Arc};

use reqwest::Client;
use rodio::{Decoder, OutputStream, Sink};
use tokio::{
    select, sync::{
        mpsc::{self, Receiver},
        RwLock,
    }, task
};

/// The amount of songs to buffer up.
const BUFFER_SIZE: usize = 5;

use crate::tracks::Track;

/// Handles communication between the frontend & audio player.
pub enum Messages {
    Skip,
}

/// Main struct responsible for queuing up tracks.
///
/// Internally tracks are stored in an [Arc],
/// so it's fine to clone this struct.
#[derive(Debug, Clone)]
pub struct Queue {
    tracks: Arc<RwLock<VecDeque<Track>>>,
}

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

    /// This is the main "audio server".
    /// 
    /// `rx` is used to communicate with it, for example when to
    /// skip tracks or pause.
    pub async fn play(
        self,
        sink: Sink,
        client: Client,
        mut rx: Receiver<Messages>
    ) -> eyre::Result<()> {
        let sink = Arc::new(sink);

        loop {
            let clone = sink.clone();
            let msg = select! {
                Some(x) = rx.recv() => x,

                // This future will finish only at the end of the current track.
                Ok(()) = task::spawn_blocking(move || clone.sleep_until_end()) => Messages::Skip,
            };

            match msg {
                Messages::Skip => {
                    sink.stop();

                    let track = self.next(&client).await?;
                    sink.append(Decoder::new(track.data)?);
                }
            }
        }
    }
}

pub async fn play() -> eyre::Result<()> {
    let queue = Queue::new().await;
    let (tx, rx) = mpsc::channel(8);
    let (_stream, handle) = OutputStream::try_default()?;
    let sink = Sink::try_new(&handle)?;
    let client = Client::builder().build()?;

    let audio = task::spawn(queue.clone().play(sink, client.clone(), rx));
    tx.send(Messages::Skip).await?; // This is responsible for the initial track being played.

    crossterm::terminal::enable_raw_mode()?;

    'a: loop {
        match crossterm::event::read()? {
            crossterm::event::Event::Key(event) => match event.code {
                crossterm::event::KeyCode::Char(x) => {
                    if x == 'q' {
                        break 'a;
                    } else if x == 's' {
                        tx.send(Messages::Skip).await?;
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
