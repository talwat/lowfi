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
    /// Errors while loading or saving the persistent volume settings.
    #[error("unable to load/save the persistent volume")]
    PersistentVolume(#[from] volume::Error),

    /// Errors while loading or saving bookmarks.
    #[error("unable to load/save bookmarks")]
    Bookmarks(#[from] bookmark::Error),

    /// Network request failures from `reqwest`.
    #[error("unable to fetch data")]
    Request(#[from] reqwest::Error),

    /// Failure converting to/from a C string (FFI helpers).
    #[error("C string null error")]
    FfiNull(#[from] std::ffi::NulError),

    /// Errors coming from the audio backend / stream handling.
    #[error("audio playing error")]
    Rodio(#[from] rodio::StreamError),

    /// Failure to send an internal `Message` over the mpsc channel.
    #[error("couldn't send internal message")]
    Send(#[from] mpsc::error::SendError<crate::Message>),

    /// Failure to enqueue a track into the queue channel.
    #[error("couldn't add track to the queue")]
    Queue(#[from] mpsc::error::SendError<tracks::Queued>),

    /// Failure to broadcast UI updates.
    #[error("couldn't update UI state")]
    Broadcast(#[from] broadcast::error::SendError<ui::Update>),

    /// Generic IO error.
    #[error("io error")]
    Io(#[from] std::io::Error),

    /// Data directory was not found or could not be determined.
    #[error("directory not found")]
    Directory,

    /// Downloader failed to provide the requested track.
    #[error("couldn't fetch track from downloader")]
    Download,

    /// Integer parsing errors.
    #[error("couldn't parse integer")]
    Parse(#[from] std::num::ParseIntError),

    /// Track subsystem error.
    #[error("track failure")]
    Track(#[from] tracks::Error),

    /// UI subsystem error.
    #[error("ui failure")]
    UI(#[from] ui::Error),

    /// Error returned when a spawned task join failed.
    #[error("join error")]
    JoinError(#[from] tokio::task::JoinError),
}
