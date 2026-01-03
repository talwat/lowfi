//! All of the logic and state relating to the downloader.

use std::{
    sync::atomic::{self, AtomicBool, AtomicU8},
    time::Duration,
};

use crate::tracks;
use reqwest::Client;
use tokio::sync::mpsc;

/// Flag indicating whether the downloader is actively fetching a track.
///
/// This is used internally to prevent concurrent downloader starts and to
/// indicate to the UI that a download is in progress.
static LOADING: AtomicBool = AtomicBool::new(false);

/// Global download progress as an integer updated atomically.
///
/// This is just a [`AtomicU8`] from 0 to 255, really representing
/// a progress percentage as just a simple integer. For instance,
/// 0.5 would be represented here as 127.
static PROGRESS: AtomicU8 = AtomicU8::new(0);

/// A convenient wrapper for the global progress. This is updated by the downloader,
/// and then accessed by the UI to display progress when there isn't an available
/// queued track.
#[derive(Clone, Copy, Debug)]
pub struct Progress(&'static AtomicU8);

impl Progress {
    /// Creates a new handle to the global progress.
    pub fn new() -> Self {
        Self(&PROGRESS)
    }

    /// Sets the global progress.
    ///
    /// `value` must be between 0 and 1.
    pub fn set(&self, value: f32) {
        self.0.store(
            (value * f32::from(u8::MAX)).round() as u8,
            atomic::Ordering::Relaxed,
        );
    }

    /// Returns the global progress as a [`f32`] between 0 and 1.
    pub fn get(&self) -> f32 {
        f32::from(self.0.load(atomic::Ordering::Relaxed)) / f32::from(u8::MAX)
    }
}

/// The downloader, which has all of the state necessary
/// to download tracks and add them to the queue.
pub struct Downloader {
    /// The track queue itself, which in this case is actually
    /// just an asynchronous sender.
    ///
    /// It is a [`mpsc::Sender`] because the tracks will have to be
    /// received by a completely different thread, so this avoids
    /// the need to use an explicit [`tokio::sync::Mutex`].
    queue: mpsc::Sender<tracks::Queued>,

    /// The [`mpsc::Sender`] which is used to inform the
    /// [`crate::Player`] with [`crate::Message::Loaded`].
    tx: mpsc::Sender<crate::Message>,

    /// The list of tracks to download from.
    tracks: tracks::List,

    /// The [`reqwest`] client to use for downloads.
    client: Client,

    /// The RNG generator to use.
    rng: fastrand::Rng,
}

impl Downloader {
    /// Actually runs the downloader, consuming it and beginning
    /// the cycle of downloading tracks and reporting to the
    /// rest of the program.
    async fn run(mut self) -> crate::Result<()> {
        const ERROR_TIMEOUT: Duration = Duration::from_secs(1);

        loop {
            let result = self
                .tracks
                .random(&self.client, Progress::new(), &mut self.rng)
                .await;

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
                        tokio::time::sleep(ERROR_TIMEOUT).await;
                    }
                }
            }
        }
    }
}

/// Downloader handle, responsible for managing
/// the downloader task and internal buffer.
pub struct Handle {
    /// The queue receiver, which can be used to actually
    /// fetch a track from the queue.
    queue: mpsc::Receiver<tracks::Queued>,
}

/// The output when a track is requested from the downloader.
pub enum Output {
    /// No track was immediately available from the downloader. When present,
    /// the `Option<Progress>` provides a reference to the global download
    /// progress so callers can show a loading indicator.
    Loading(Option<Progress>),

    /// A successfully downloaded (but not yet decoded) track ready to be
    /// enqueued for decoding/playback.
    Queued(tracks::Queued),
}

impl Handle {
    /// Gets either a queued track, or a progress report,
    /// depending on the state of the internal download buffer.
    #[rustfmt::skip]
    pub fn track(&mut self) -> Output {
        self.queue.try_recv().map_or_else(|_| {
                LOADING.store(true, atomic::Ordering::Relaxed);
                Output::Loading(Some(Progress::new()))
            }, Output::Queued,
        )
    }
}

impl crate::Tasks {
    /// Initializes the downloader with a track list.
    ///
    /// `tx` specifies the [`Sender`] to be notified with [`crate::Message::Loaded`].
    pub fn downloader(
        &mut self,
        size: usize,
        timeout: u64,
        tracks: tracks::List,
    ) -> crate::Result<Handle> {
        let client = Client::builder()
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .timeout(Duration::from_secs(timeout))
            .build()?;

        let (qtx, qrx) = mpsc::channel(size - 1);
        let downloader = Downloader {
            queue: qtx,
            tx: self.tx(),
            tracks,
            client,
            rng: fastrand::Rng::new(),
        };

        self.spawn(downloader.run());
        Ok(Handle { queue: qrx })
    }
}
