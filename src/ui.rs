use std::sync::Arc;

use crate::player::Current;
use tokio::{sync::broadcast, time::Instant};

pub mod environment;
pub mod init;
pub use environment::Environment;
pub mod input;
pub mod interface;
pub use interface::Interface;

#[cfg(feature = "mpris")]
pub mod mpris;

/// Shorthand for a [`Result`] with a [`ui::Error`].
type Result<T> = std::result::Result<T, Error>;

/// The error type for the UI, which is used to handle errors
/// that occur while drawing the UI or handling input.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unable to convert number: {0}")]
    Conversion(#[from] std::num::TryFromIntError),

    #[error("unable to write output: {0}")]
    Write(#[from] std::io::Error),

    #[error("sending signal message to backend from ui failed: {0}")]
    SignalSend(#[from] tokio::sync::mpsc::error::SendError<crate::Message>),

    #[error("sharing state between backend and frontend failed: {0}")]
    StateSend(#[from] tokio::sync::broadcast::error::SendError<Update>),

    #[error("you can't disable the UI without MPRIS!")]
    RejectedDisable,

    #[cfg(feature = "mpris")]
    #[error("mpris bus error: {0}")]
    ZBus(#[from] mpris_server::zbus::Error),

    #[cfg(feature = "mpris")]
    #[error("mpris fdo (zbus interface) error: {0}")]
    Fdo(#[from] mpris_server::zbus::fdo::Error),
}

/// The UI state, which is all of the information that
/// the user interface needs to display to the user.
///
/// It should be noted that this is also used by MPRIS to keep
/// track of state.
#[derive(Clone)]
pub struct State {
    /// The audio sink.
    pub sink: Arc<rodio::Sink>,

    /// The current track, which is updated by way of an [`Update`].
    pub current: Current,

    /// Whether the current track is bookmarked.
    pub bookmarked: bool,

    /// The timer, which is used when the user changes volume to briefly display it.
    pub(crate) volume_timer: Option<Instant>,

    /// The name of the playing tracklist, mainly for MPRIS.
    #[allow(dead_code)]
    tracklist: String,
}

impl State {
    /// Creates an initial UI state.
    pub fn initial(sink: Arc<rodio::Sink>, list: String) -> Self {
        Self {
            sink,
            tracklist: list,
            current: Current::default(),
            bookmarked: false,
            volume_timer: None,
        }
    }

    /// Takes care of small updates, like resetting the volume timer.
    pub fn tick(&mut self) {
        let expired = |timer: Instant| timer.elapsed() > std::time::Duration::from_secs(1);
        if self.volume_timer.is_some_and(expired) {
            self.volume_timer = None;
        }
    }
}

/// A UI update sent out by the main player thread, which may
/// not be immediately applied by the UI.
///
/// This corresponds to user actions, like bookmarking a track,
/// skipping, or changing the volume. The difference is that it also
/// contains the new information about the track.
#[derive(Debug, Clone)]
pub enum Update {
    Track(Current),
    Bookmarked(bool),
    Volume,
    Quit,
}

/// The UI handle for controlling the state of the UI, as well as
/// updating MPRIS information and other small interfacing tasks.
pub struct Handle {
    /// Broadcast channel used to send UI updates.
    updater: broadcast::Sender<Update>,

    /// The MPRIS server, which is more or less a handle to the actual MPRIS thread.
    #[cfg(feature = "mpris")]
    pub mpris: mpris::Server,
}

impl Handle {
    /// Sends a `ui::Update` to the broadcast channel.
    pub fn update(&mut self, update: Update) -> crate::Result<()> {
        self.updater.send(update)?;
        Ok(())
    }
}

/// The main UI process, which will both render the UI to the terminal
/// and also update state.
///
/// It does both of these things at a fixed interval, due to things
/// like the track duration changing too frequently.
///
/// `rx` is the receiver for state updates, `state` the initial state,
/// and `params` specifies aesthetic options that are specified by the user.
pub async fn run(
    mut updater: broadcast::Receiver<Update>,
    mut state: State,
    params: interface::Params,
) -> Result<()> {
    let mut interface = Interface::new(params)?;

    loop {
        if let Ok(message) = updater.try_recv() {
            match message {
                Update::Track(track) => state.current = track,
                Update::Bookmarked(bookmarked) => state.bookmarked = bookmarked,
                Update::Volume => state.volume_timer = Some(Instant::now()),
                Update::Quit => break,
            }
        }

        interface.draw(&state).await?;
        state.tick();
    }

    Ok(())
}
