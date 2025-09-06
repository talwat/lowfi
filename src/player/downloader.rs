//! Contains the [`Downloader`] struct.

use std::{error::Error, sync::Arc};

use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::{self, JoinHandle},
    time::sleep,
};

use super::Player;

/// This struct is responsible for downloading tracks in the background.
///
/// This is not used for the first track or a track when the buffer is currently empty.
pub struct Downloader {
    /// The player for the downloader to download to & with.
    player: Arc<Player>,

    /// The internal reciever, which is used by the downloader to know
    /// when to begin downloading more tracks.
    rx: Receiver<()>,

    /// A copy of the internal sender, which can be useful for keeping
    /// track of it.
    tx: Sender<()>,
}

impl Downloader {
    /// Uses a sender recieved from [Sender] to notify the
    /// download thread that it should resume downloading.
    pub async fn notify(sender: &Sender<()>) -> Result<(), mpsc::error::SendError<()>> {
        sender.send(()).await
    }

    /// Initializes the [Downloader].
    ///
    /// This also sends a [`Sender`] which can be used to notify
    /// when the downloader needs to begin downloading more tracks.
    pub fn new(player: Arc<Player>) -> Self {
        let (tx, rx) = mpsc::channel(8);
        Self { player, rx, tx }
    }

    /// Push a new, random track onto the internal buffer.
    pub async fn push_buffer(&self, debug: bool) {
        let data = self.player.list.random(&self.player.client, None).await;
        match data {
            Ok(track) => self.player.tracks.write().await.push_back(track),
            Err(error) => {
                if debug {
                    panic!("{error} - {:?}", error.source())
                }

                if !error.is_timeout() {
                    sleep(self.player.timeout).await;
                }
            }
        }
    }

    /// Actually starts & consumes the [Downloader].
    pub fn start(mut self, debug: bool) -> (Sender<()>, JoinHandle<()>) {
        let tx = self.tx.clone();

        let handle = task::spawn(async move {
            // Loop through each update notification.
            while self.rx.recv().await == Some(()) {
                //  For each update notification, we'll push tracks until the buffer is completely full.
                while self.player.tracks.read().await.len() < self.player.buffer_size {
                    self.push_buffer(debug).await;
                }
            }
        });

        (tx, handle)
    }
}
