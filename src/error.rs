//! Application-wide error type.
//!
//! This module exposes a single `Error` enum that aggregates the common
//! error kinds used across the application (IO, networking, UI, audio,
//! persistence). Higher-level functions should generally return
//! `crate::error::Result<T>` to make error handling consistent.

use crate::{bookmark, tracks, ui, volume};
use tokio::sync::{broadcast, mpsc};

/// Result alias using the crate-wide `Error` type.
pub type Result<T> = std::result::Result<T, Error>;

/// Central application error.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unable to load/save the persistent volume")]
    PersistentVolume(#[from] volume::Error),

    #[error("unable to load/save bookmarks")]
    Bookmarks(#[from] bookmark::Error),

    #[error("unable to fetch data")]
    Request(#[from] reqwest::Error),

    #[error("C string null error")]
    FfiNull(#[from] std::ffi::NulError),

    #[error("audio playing error")]
    Rodio(#[from] rodio::StreamError),

    #[error("couldn't send internal message")]
    Send(#[from] mpsc::error::SendError<crate::Message>),

    #[error("couldn't add track to the queue")]
    Queue(#[from] mpsc::error::SendError<tracks::Queued>),

    #[error("couldn't update UI state")]
    Broadcast(#[from] broadcast::error::SendError<ui::Update>),

    #[error("io error")]
    Io(#[from] std::io::Error),

    #[error("directory not found")]
    Directory,

    #[error("couldn't fetch track from downloader")]
    Download,

    #[error("couldn't parse integer")]
    Parse(#[from] std::num::ParseIntError),

    #[error("track failure")]
    Track(#[from] tracks::Error),

    #[error("ui failure")]
    UI(#[from] ui::Error),

    #[error("join error")]
    JoinError(#[from] tokio::task::JoinError),
}
