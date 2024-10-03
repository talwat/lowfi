//! Responsible for playing & queueing audio.
//! This also has the code for the underlying
//! audio server which adds new tracks.

use std::{collections::VecDeque, sync::Arc, time::Duration};

use arc_swap::ArcSwapOption;
use downloader::Downloader;
use libc::freopen;
use reqwest::Client;
use rodio::{OutputStream, OutputStreamHandle, Sink};
use tokio::{
    select,
    sync::{
        mpsc::{Receiver, Sender},
        RwLock,
    },
    task,
};

use crate::tracks::{DecodedTrack, Track, TrackInfo};

pub mod downloader;
pub mod ui;

/// Handles communication between the frontend & audio player.
#[derive(PartialEq)]
pub enum Messages {
    /// Notifies the audio server that it should update the track.
    Next,

    /// This signal is only sent if a track timed out. In that case,
    /// lowfi will try again and again to retrieve the track.
    TryAgain,

    /// Similar to Next, but specific to the first track.
    Init,

    /// Pauses the [Sink]. This will also unpause it if it is paused.
    Pause,

    /// Change the volume of playback
    ChangeVolume(f32),
}

const TIMEOUT: Duration = Duration::from_secs(8);

/// The amount of songs to buffer up.
const BUFFER_SIZE: usize = 5;

/// Main struct responsible for queuing up & playing tracks.
pub struct Player {
    /// [rodio]'s [`Sink`] which can control playback.
    pub sink: Sink,

    /// The [`TrackInfo`] of the current track.
    /// This is [`None`] when lowfi is buffering.
    pub current: ArcSwapOption<TrackInfo>,

    /// The tracks, which is a [VecDeque] that holds
    /// *undecoded* [Track]s.
    tracks: RwLock<VecDeque<Track>>,

    /// The web client, which can contain a UserAgent & some
    /// settings that help lowfi work more effectively.
    client: Client,

    /// The [OutputStreamHandle], which also can control some
    /// playback, is for now unused and is here just to keep it
    /// alive so the playback can function properly.
    _handle: OutputStreamHandle,

    /// The [OutputStream], which is just here to keep the playback
    /// alive and functioning.
    _stream: OutputStream,
}

/// SAFETY: This is necessary because [OutputStream] does not implement [Send],
/// SAFETY: even though it is perfectly possible.
unsafe impl Send for Player {}

/// SAFETY: See implementation for [Send].
unsafe impl Sync for Player {}

impl Player {
    /// This gets the output stream while also shutting up alsa with [libc].
    fn silent_get_output_stream() -> eyre::Result<(OutputStream, OutputStreamHandle)> {
        extern "C" {
            static stderr: *mut libc::FILE;
        }

        let mode = c"w".as_ptr();

        // This is a bit of an ugly hack that basically just uses `libc` to redirect alsa's
        // output to `/dev/null` so that it wont be shoved down our throats.

        // SAFETY: Simple enough to be impossible to fail. Hopefully.
        unsafe { freopen(c"/dev/null".as_ptr(), mode, stderr) };

        let (stream, handle) = OutputStream::try_default()?;

        // SAFETY: See the first call to `freopen`.
        unsafe { freopen(c"/dev/tty".as_ptr(), mode, stderr) };

        Ok((stream, handle))
    }

    /// Initializes the entire player, including audio devices & sink.
    pub async fn new() -> eyre::Result<Self> {
        let (_stream, handle) = if cfg!(target_os = "linux") {
            Self::silent_get_output_stream()?
        } else {
            OutputStream::try_default()?
        };

        let sink = Sink::try_new(&handle)?;

        let player = Self {
            tracks: RwLock::new(VecDeque::with_capacity(5)),
            current: ArcSwapOption::new(None),
            client: Client::builder()
                .user_agent(concat!(
                    env!("CARGO_PKG_NAME"),
                    "/",
                    env!("CARGO_PKG_VERSION")
                ))
                .timeout(TIMEOUT)
                .build()?,
            sink,
            _handle: handle,
            _stream,
        };

        Ok(player)
    }

    /// Just a shorthand for setting `current`.
    async fn set_current(&self, info: TrackInfo) -> eyre::Result<()> {
        self.current.store(Some(Arc::new(info)));

        Ok(())
    }

    /// This will play the next track, as well as refilling the buffer in the background.
    pub async fn next(queue: Arc<Self>) -> eyre::Result<DecodedTrack> {
        let track = match queue.tracks.write().await.pop_front() {
            Some(x) => x,
            // If the queue is completely empty, then fallback to simply getting a new track.
            // This is relevant particularly at the first song.
            None => Track::random(&queue.client).await?,
        };

        let decoded = track.decode()?;
        queue.set_current(decoded.info.clone()).await?;

        Ok(decoded)
    }

    /// This is the main "audio server".
    ///
    /// `rx` & `tx` are used to communicate with it, for example when to
    /// skip tracks or pause.
    pub async fn play(
        player: Arc<Self>,
        tx: Sender<Messages>,
        mut rx: Receiver<Messages>,
    ) -> eyre::Result<()> {
        // `itx` is used to notify the `Downloader` when it needs to download new tracks.
        let (downloader, itx) = Downloader::new(player.clone());
        downloader.start().await;

        // Start buffering tracks immediately.
        itx.send(()).await?;

        loop {
            let clone = Arc::clone(&player);
            let msg = select! {
                Some(x) = rx.recv() => x,

                // This future will finish only at the end of the current track.
                Ok(_) = task::spawn_blocking(move || clone.sink.sleep_until_end()) => Messages::Next,
            };

            match msg {
                Messages::Next | Messages::Init | Messages::TryAgain => {
                    // Skip as early as possible so that music doesn't play
                    // while lowfi is "loading".
                    player.sink.stop();

                    // Serves as an indicator that the queue is "loading".
                    // This is also set by Player::next.
                    player.current.store(None);

                    let track = Self::next(Arc::clone(&player)).await;

                    match track {
                        Ok(track) => {
                            player.sink.append(track.data);

                            // Notify the background downloader that there's an empty spot
                            // in the buffer.
                            itx.send(()).await?;
                        }
                        Err(error) => {
                            if !error.downcast::<reqwest::Error>()?.is_timeout() {
                                tokio::time::sleep(TIMEOUT).await;
                            }

                            tx.send(Messages::TryAgain).await?
                        }
                    };
                }
                Messages::Pause => {
                    if player.sink.is_paused() {
                        player.sink.play();
                    } else {
                        player.sink.pause();
                    }
                }
                Messages::ChangeVolume(change) => {
                    player
                        .sink
                        .set_volume((player.sink.volume() + change).clamp(0.0, 1.0));
                }
            }
        }
    }
}
