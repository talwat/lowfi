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

/// Flag indicating whether the downloader is actively fetching a track.
///
/// This is used internally to prevent concurrent downloader starts and to
/// indicate to the UI that a download is in progress.
static LOADING: AtomicBool = AtomicBool::new(false);

/// Global download progress in the range 0..=100 updated atomically.
///
/// The UI can read this `AtomicU8` to render a global progress indicator
/// when there isn't an immediately queued track available.
pub(crate) static PROGRESS: AtomicU8 = AtomicU8::new(0);

/// A convenient alias for the progress `AtomicU8` pointer type.
pub type Progress = &'static AtomicU8;

/// The downloader, which has all of the state necessary
/// to download tracks and add them to the queue.
pub struct Downloader {
    /// The track queue itself, which in this case is actually
    /// just an asynchronous sender.
    ///
    /// It is a [`Sender`] because the tracks will have to be
    /// received by a completely different thread, so this avoids
    /// the need to use an explicit [`tokio::sync::Mutex`].
    queue: Sender<tracks::Queued>,

    /// The [`Sender`] which is used to inform the
    /// [`crate::Player`] with [`crate::Message::Loaded`].
    tx: Sender<crate::Message>,

    /// The list of tracks to download from.
    tracks: tracks::List,

    /// The [`reqwest`] client to use for downloads.
    client: Client,

    /// The RNG generator to use.
    rng: fastrand::Rng,
}

impl Downloader {
    /// Initializes the downloader with a track list.
    ///
    /// `tx` specifies the [`Sender`] to be notified with [`crate::Message::Loaded`].
    pub fn init(
        size: usize,
        timeout: u64,
        tracks: tracks::List,
        tx: Sender<crate::Message>,
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
        let downloader = Self {
            queue: qtx,
            tx,
            tracks,
            client,
            rng: fastrand::Rng::new(),
        };

        Ok(Handle {
            queue: qrx,
            task: tokio::spawn(downloader.run()),
        })
    }

    /// Actually runs the downloader, consuming it and beginning
    /// the cycle of downloading tracks and reporting to the
    /// rest of the program.
    async fn run(mut self) -> crate::Result<()> {
        const ERROR_TIMEOUT: Duration = Duration::from_secs(1);

        loop {
            let result = self
                .tracks
                .random(&self.client, &PROGRESS, &mut self.rng)
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
    queue: Receiver<tracks::Queued>,

    /// The downloader task, which can be aborted.
    task: JoinHandle<crate::Result<()>>,
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
                Output::Loading(Some(&PROGRESS))
            }, Output::Queued,
        )
    }
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.task.abort();
    }
}
