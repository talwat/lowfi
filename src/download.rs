use std::{
    sync::atomic::{self, AtomicBool, AtomicU8},
    time::Duration,
};

use reqwest::Client;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};

use crate::tracks;

static LOADING: AtomicBool = AtomicBool::new(false);
pub(crate) static PROGRESS: AtomicU8 = AtomicU8::new(0);
pub type Progress = &'static AtomicU8;

/// The downloader, which has all of the state necessary
/// to download tracks and add them to the queue.
pub struct Downloader {
    queue: Sender<tracks::Queued>,
    tx: Sender<crate::Message>,
    tracks: tracks::List,
    client: Client,
    timeout: Duration,
}

impl Downloader {
    /// Initializes the downloader with a track list.
    ///
    /// `tx` specifies the [`Sender`] to be notified with [`crate::Message::Loaded`].
    pub fn init(size: usize, tracks: tracks::List, tx: Sender<crate::Message>) -> Handle {
        let client = Client::new();

        let (qtx, qrx) = mpsc::channel(size - 1);
        let downloader = Self {
            queue: qtx,
            tx,
            tracks,
            client,
            timeout: Duration::from_secs(1),
        };

        Handle {
            queue: qrx,
            handle: tokio::spawn(downloader.run()),
        }
    }

    async fn run(self) -> crate::Result<()> {
        loop {
            let result = self.tracks.random(&self.client, &PROGRESS).await;
            match result {
                Ok(track) => {
                    self.queue.send(track).await?;

                    if LOADING.load(atomic::Ordering::Relaxed) {
                        self.tx.send(crate::Message::Loaded).await?;
                        LOADING.store(false, atomic::Ordering::Relaxed);
                    }
                }
                Err(error) => {
                    PROGRESS.store(0, atomic::Ordering::Relaxed);
                    if !error.timeout() {
                        tokio::time::sleep(self.timeout).await;
                    }
                }
            }
        }
    }
}

/// Downloader handle, responsible for managing
/// the downloader task and internal buffer.
pub struct Handle {
    queue: Receiver<tracks::Queued>,
    handle: JoinHandle<crate::Result<()>>,
}

/// The output when a track is requested from the downloader.
pub enum Output {
    Loading(Option<Progress>),
    Queued(tracks::Queued),
}

impl Handle {
    /// Gets either a queued track, or a progress report,
    /// depending on the state of the internal download buffer.
    #[rustfmt::skip]
    pub fn track(&mut self) -> Output {
        self.queue.try_recv().map_or_else(|_| {
                LOADING.store(true, atomic::Ordering::Relaxed);
                Output::Loading(Some(&PROGRESS))
            }, Output::Queued,
        )
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
