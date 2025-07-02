use std::sync::Arc;
use tokio::{sync::mpsc::Sender, time::sleep};

use crate::{
    messages::Messages,
    player::{downloader::Downloader, Player, TIMEOUT},
    tracks,
};

impl Player {
    /// Fetches the next track from the queue, or a random track if the queue is empty.
    /// This will also set the current track to the fetched track's info.
    async fn fetch(&self) -> Result<tracks::Decoded, tracks::TrackError> {
        // TODO: Consider replacing this with `unwrap_or_else` when async closures are stablized.
        let track = self.tracks.write().await.pop_front();
        let track = if let Some(track) = track {
            track
        } else {
            // If the queue is completely empty, then fallback to simply getting a new track.
            // This is relevant particularly at the first song.

            // Serves as an indicator that the queue is "loading".
            // We're doing it here so that we don't get the "loading" display
            // for only a frame in the other case that the buffer is not empty.
            self.current.store(None);
            self.list.random(&self.client).await?
        };

        let decoded = track.decode()?;

        // Set the current track.
        self.set_current(decoded.info.clone());

        Ok(decoded)
    }

    /// Gets, decodes, and plays the next track in the queue while also handling the downloader.
    ///
    /// This functions purpose is to be called in the background, so that when the audio server recieves a
    /// `Next` signal it will still be able to respond to other signals while it's loading.
    ///
    /// This also sends the either a `NewSong` or `TryAgain` signal to `tx`.
    pub async fn next(
        player: Arc<Self>,
        itx: Sender<()>,
        tx: Sender<Messages>,
        debug: bool,
    ) -> eyre::Result<()> {
        // Stop the sink.
        player.sink.stop();

        let track = player.fetch().await;

        match track {
            Ok(track) => {
                // Start playing the new track.
                player.sink.append(track.data);

                // Notify the background downloader that there's an empty spot
                // in the buffer.
                Downloader::notify(&itx).await?;

                // Notify the audio server that the next song has actually been downloaded.
                tx.send(Messages::NewSong).await?;
            }
            Err(error) => {
                if !error.is_timeout() {
                    if debug {
                        panic!("{:?}", error)
                    }

                    sleep(TIMEOUT).await;
                }

                tx.send(Messages::TryAgain).await?;
            }
        };

        Ok(())
    }
}
