use tokio::sync::{broadcast, mpsc};

use crate::{bookmark, tracks, ui, volume};

pub type Result<T> = std::result::Result<T, Error>;
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unable to load/save the persistent volume: {0}")]
    PersistentVolume(#[from] volume::Error),

    #[error("unable to load/save bookmarks: {0}")]
    Bookmarks(#[from] bookmark::Error),

    #[error("unable to fetch data: {0}")]
    Request(#[from] reqwest::Error),

    #[error("C string null error: {0}")]
    FfiNull(#[from] std::ffi::NulError),

    #[error("audio playing error: {0}")]
    Rodio(#[from] rodio::StreamError),

    #[error("couldn't send internal message: {0}")]
    Send(#[from] mpsc::error::SendError<crate::Message>),

    #[error("couldn't add track to the queue: {0}")]
    Queue(#[from] mpsc::error::SendError<tracks::Queued>),

    #[error("couldn't update UI state: {0}")]
    Broadcast(#[from] broadcast::error::SendError<ui::Update>),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("directory not found")]
    Directory,

    #[error("couldn't fetch track from downloader")]
    Download,

    #[error("couldn't parse integer: {0}")]
    Parse(#[from] std::num::ParseIntError),

    #[error("track failure")]
    Track(#[from] tracks::Error),

    #[error("ui failure")]
    UI(#[from] ui::Error),

    #[cfg(feature = "mpris")]
    #[error("mpris bus error")]
    ZBus(#[from] mpris_server::zbus::Error),

    // TODO: This has a terrible error message, mainly because I barely understand
    // what this error even represents. What does fdo mean?!?!? Why, MPRIS!?!?
    #[cfg(feature = "mpris")]
    #[error("mpris fdo (zbus interface) error")]
    Fdo(#[from] mpris_server::zbus::fdo::Error),
}
