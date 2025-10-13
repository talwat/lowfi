use std::{
    error::Error,
    sync::{atomic::Ordering, Arc},
};
use tokio::{sync::mpsc::Sender, time::sleep};

use crate::{
    messages::Message,
    player::{downloader::Downloader, Player},
    tracks,
};
use crate::debug_log;

impl Player {
    /// Fetches the next track from the queue, or a random track if the queue is empty.
    /// This will also set the current track to the fetched track's info.
    async fn fetch(&self) -> Result<tracks::DecodedTrack, tracks::Error> {
        debug_log!("queue.rs - fetch: fetch start");
        // TODO: Consider replacing this with `unwrap_or_else` when async closures are stablized.
        let track = self.tracks.write().await.pop_front();
        let track = if let Some(track) = track {
            debug_log!("queue.rs - fetch: popped from buffer full_path={}", track.full_path);
            track
        } else {
            // If the queue is completely empty, then fallback to simply getting a new track.
            // This is relevant particularly at the first song.

            // Serves as an indicator that the queue is "loading".
            // We're doing it here so that we don't get the "loading" display
            // for only a frame in the other case that the buffer is not empty.
            self.current.store(None);
            self.progress.store(0.0, Ordering::Relaxed);
            debug_log!("queue.rs - fetch: buffer empty; fetching random");
            self.list.random(&self.client, Some(&self.progress)).await?
        };

        // Start palette fetch early in parallel.
        #[cfg(feature = "color")]
        let palette_future = {
            let art_url_opt = track.art_url.clone();
            let client = self.client.clone();
            let skip_art = self.skip_art;
            async move {
                if !skip_art {
                    if let Some(url) = art_url_opt {
                        if !url.is_empty() && url.starts_with("http") {
                            crate::player::ui::cover::extract_color_palette_from_url_with_client(&client, &url).await
                        } else { None }
                    } else { None }
                } else { None }
            }
        };

        #[cfg(feature = "color")]
        let (palette_opt, decoded_res) = tokio::join!(palette_future, async { track.clone().decode() });
        #[cfg(feature = "color")]
        let decoded = decoded_res?;

        #[cfg(not(feature = "color"))]
        let decoded = track.decode()?;
        debug_log!("queue.rs - fetch: decoded display_name={} duration={:?}", decoded.info.display_name, decoded.info.duration);

        // Load colors and art synchronously before setting current track.
        #[cfg(feature = "color")]
        let final_info = {
            let mut info = decoded.info.clone();
            if info.color_palette.is_none() {
                if let Some(p) = palette_opt {
                    info.color_palette = Some(p.clone());
                } else if let Some(palette) = self.get_color_palette(&info).await {
                    info.color_palette = Some(palette);
                }
            }
            // Ensure art bytes cached synchronously.
            if let Some(art_url) = &info.art_url {
                if !art_url.is_empty() && art_url.starts_with("http") && !self.skip_art {
                    if self.get_art(&info).await.is_none() {
                        if let Some((palette, image_data)) = crate::player::ui::cover::extract_color_palette_and_bytes_from_url_with_client(&self.client, art_url).await {
                            if let Some(art_url) = &info.art_url {
                                self.art_cache.cache_art(art_url.clone(), image_data).await;
                                
                                if !self.skip_colors {
                                    self.art_cache.cache_colors(art_url.clone(), palette).await;
                                }
                            }
                        }
                    }
                }
            }
            info
        };

        #[cfg(not(feature = "color"))]
        let final_info = decoded.info.clone();

        self.set_current(final_info);
        debug_log!("queue.rs - fetch: current track set");

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
        tx: Sender<Message>,
        debug: bool,
    ) -> eyre::Result<()> {
        // Stop the sink.
        player.sink.stop();

        let track = player.fetch().await;

        match track {
            Ok(track) => {
                // Start playing the new track.
                player.sink.append(track.data);
                debug_log!("queue.rs - next: track appended to sink");

                // Set whether it's bookmarked.
                player.bookmarks.set_bookmarked(&track.info).await;

                // Notify the background downloader that there's an empty spot
                // in the buffer.
                Downloader::notify(&itx).await?;
                debug_log!("queue.rs - next: downloader notified");

                // Notify the audio server that the next song has actually been downloaded.
                tx.send(Message::NewSong).await?;
                debug_log!("queue.rs - next: NewSong message sent");
            }
            Err(error) => {
                debug_log!("queue.rs - next: error occurred err={}", error);
                if debug {
                    panic!("{error} - {:?}", error.source())
                }

                if !error.is_timeout() {
                    sleep(player.timeout).await;
                }

                tx.send(Message::TryAgain).await?;
            }
        };

        Ok(())
    }
}
