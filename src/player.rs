use std::{collections::VecDeque, sync::Arc};

use arc_swap::ArcSwapOption;
use reqwest::Client;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use tokio::{
    select,
    sync::{
        mpsc::{self, Receiver},
        RwLock,
    },
    task,
};

use crate::tracks::{Track, TrackInfo};

pub mod ui;

/// Handles communication between the frontend & audio player.
pub enum Messages {
    Next,
    Init,
    Pause,
}

/// The amount of songs to buffer up.
const BUFFER_SIZE: usize = 5;

/// Main struct responsible for queuing up & playing tracks.
pub struct Player {
    pub sink: Sink,
    pub current: ArcSwapOption<TrackInfo>,
    tracks: RwLock<VecDeque<Track>>,
    client: Client,
    _handle: OutputStreamHandle,
    _stream: OutputStream,
}

unsafe impl Send for Player {}
unsafe impl Sync for Player {}

impl Player {
    /// Initializes the entire player, including audio devices & sink.
    pub async fn new() -> eyre::Result<Self> {
        let (_stream, handle) = OutputStream::try_default()?;
        let sink = Sink::try_new(&handle)?;

        Ok(Self {
            tracks: RwLock::new(VecDeque::with_capacity(5)),
            current: ArcSwapOption::new(None),
            client: Client::builder().build()?,
            sink,
            _handle: handle,
            _stream,
        })
    }

    async fn set_current(&self, info: TrackInfo) -> eyre::Result<()> {
        self.current.store(Some(Arc::new(info)));

        Ok(())
    }

    /// This will play the next track, as well as refilling the buffer in the background.
    pub async fn next(queue: Arc<Player>) -> eyre::Result<Track> {
        queue.current.store(None);

        let track = queue.tracks.write().await.pop_front();
        let track = match track {
            Some(x) => x,
            // If the queue is completely empty, then fallback to simply getting a new track.
            // This is relevant particularly at the first song.
            None => Track::random(&queue.client).await?,
        };

        queue.set_current(track.info).await?;

        Ok(track)
    }

    /// This is the main "audio server".
    ///
    /// `rx` is used to communicate with it, for example when to
    /// skip tracks or pause.
    pub async fn play(queue: Arc<Player>, mut rx: Receiver<Messages>) -> eyre::Result<()> {
        // This is an internal channel which serves pretty much only one purpose,
        // which is to notify the buffer refiller to get back to work.
        // This channel is useful to prevent needing to check with some infinite loop.
        let (itx, mut irx) = mpsc::channel(8);

        // This refills the queue in the background.
        task::spawn({
            let queue = queue.clone();

            async move {
                while let Some(()) = irx.recv().await {
                    while queue.tracks.read().await.len() < BUFFER_SIZE {
                        let track = Track::random(&queue.client).await.unwrap();
                        queue.tracks.write().await.push_back(track);
                    }
                }
            }
        });

        itx.send(()).await?;

        loop {
            let clone = Arc::clone(&queue);
            let msg = select! {
                Some(x) = rx.recv() => x,

                // This future will finish only at the end of the current track.
                Ok(()) = task::spawn_blocking(move || clone.sink.sleep_until_end()) => Messages::Next,
            };

            match msg {
                Messages::Next | Messages::Init => {
                    itx.send(()).await?;

                    queue.sink.stop();

                    let track = Player::next(queue.clone()).await?;
                    queue.sink.append(track.data);
                }
                Messages::Pause => {
                    if queue.sink.is_paused() {
                        queue.sink.play();
                    } else {
                        queue.sink.pause();
                    }
                }
            }
        }
    }
}
