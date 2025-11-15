use std::{
    sync::{atomic::AtomicU8, Arc},
    time::Duration,
};

use reqwest::Client;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};

use crate::tracks;

pub struct Downloader {
    /// TODO: Actually have a track type here.
    pub progress: Arc<AtomicU8>,
    queue: Receiver<tracks::Queued>,
    handle: JoinHandle<crate::Result<()>>,
}

impl Downloader {
    pub async fn track(&mut self) -> Option<tracks::Queued> {
        return self.queue.recv().await;
    }

    async fn downloader(
        tx: Sender<tracks::Queued>,
        tracks: tracks::List,
        client: Client,
        progress: Arc<AtomicU8>,
        timeout: Duration,
    ) -> crate::Result<()> {
        loop {
            let result = tracks.random(&client, progress.as_ref()).await;
            match result {
                Ok(track) => tx.send(track).await?,
                Err(error) => {
                    if !error.timeout() {
                        tokio::time::sleep(timeout).await;
                    }
                }
            }
        }
    }

    pub async fn init(
        size: usize,
        tracks: tracks::List,
        client: Client,
        progress: Arc<AtomicU8>,
    ) -> Self {
        let (tx, rx) = mpsc::channel(size);

        Self {
            queue: rx,
            progress: progress.clone(),
            handle: tokio::spawn(Self::downloader(
                tx,
                tracks,
                client,
                progress,
                Duration::from_secs(1),
            )),
        }
    }
}

impl Drop for Downloader {
    fn drop(&mut self) {
        self.handle.abort();
    }
}
