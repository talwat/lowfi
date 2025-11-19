use std::{
    sync::atomic::{self, AtomicU8},
    time::Duration,
};

use reqwest::Client;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};

use crate::tracks;

static PROGRESS: AtomicU8 = AtomicU8::new(0);
pub type Progress = &'static AtomicU8;

pub fn progress() -> Progress {
    &PROGRESS
}

pub struct Downloader {
    queue: Sender<tracks::Queued>,
    tx: Sender<crate::Message>,
    tracks: tracks::List,
    client: Client,
    timeout: Duration,
}

impl Downloader {
    pub async fn init(size: usize, tracks: tracks::List, tx: Sender<crate::Message>) -> Handle {
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
            let progress = if PROGRESS.load(atomic::Ordering::Relaxed) == 0 {
                Some(&PROGRESS)
            } else {
                None
            };

            let result = self.tracks.random(&self.client, progress).await;
            match result {
                Ok(track) => {
                    self.queue.send(track).await?;

                    if progress.is_some() {
                        self.tx.send(crate::Message::Loaded).await?;
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
pub struct Handle {
    queue: Receiver<tracks::Queued>,
    handle: JoinHandle<crate::Result<()>>,
}

pub enum Output {
    Loading(Progress),
    Queued(tracks::Queued),
}

impl Handle {
    pub async fn track(&mut self) -> Output {
        match self.queue.try_recv() {
            Ok(queued) => Output::Queued(queued),
            Err(_) => {
                PROGRESS.store(0, atomic::Ordering::Relaxed);
                Output::Loading(progress())
            }
        }
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
