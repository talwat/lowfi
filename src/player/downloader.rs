//! Contains the [`Downloader`] struct.

use std::sync::Arc;

use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::{self, JoinHandle},
};

use crate::tracks::Track;

use super::{Player, BUFFER_SIZE};

/// This struct is responsible for downloading tracks in the background.
///
/// This is not used for the first track or a track when the buffer is currently empty.
pub struct Downloader {
    /// The player for the downloader to download to & with.
    player: Arc<Player>,

    /// The internal reciever, which is used by the downloader to know
    /// when to begin downloading more tracks.
    rx: Receiver<()>,
}

impl Downloader {
    /// Initializes the [Downloader].
    ///
    /// This also sends a [`Sender`] which can be used to notify
    /// when the downloader needs to begin downloading more tracks.
    pub fn new(player: Arc<Player>) -> (Self, Sender<()>) {
        let (tx, rx) = mpsc::channel(8);
        (Self { player, rx }, tx)
    }

    /// Actually starts & consumes the [Downloader].
    pub async fn start(mut self) -> JoinHandle<()> {
        task::spawn(async move {
            // Loop through each update notification.
            while self.rx.recv().await == Some(()) {
                //  For each update notification, we'll push tracks until the buffer is completely full.
                while self.player.tracks.read().await.len() < BUFFER_SIZE {
                    let Ok(track) = Track::random(&self.player.client).await else {
                        continue;
                    };

                    self.player.tracks.write().await.push_back(track);
                }
            }
        })
    }
}
