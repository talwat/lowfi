//! Module for handling saving, loading, and adding
//! bookmarks.

use std::sync::atomic::AtomicBool;

use tokio::fs::{create_dir_all, File, OpenOptions};
use tokio::io::{self, AsyncReadExt, AsyncWriteExt};
use tokio::sync::RwLock;

use crate::{data_dir, tracks};

/// Errors that might occur while managing bookmarks.
#[derive(Debug, thiserror::Error)]
pub enum BookmarkError {
    #[error("data directory not found")]
    DataDir,

    #[error("io failure")]
    Io(#[from] io::Error),
}

/// Manages the bookmarks in the current player.
pub struct Bookmarks {
    /// The different entries in the bookmarks file.
    entries: RwLock<Vec<String>>,

    /// The internal bookmarked register, which keeps track
    /// of whether a track is bookmarked or not.
    ///
    /// This is much more efficient than checking every single frame.
    bookmarked: AtomicBool,
}

impl Bookmarks {
    /// Actually opens the bookmarks file itself.
    pub async fn open(write: bool) -> eyre::Result<File, BookmarkError> {
        let data_dir = data_dir().map_err(|_| BookmarkError::DataDir)?;
        create_dir_all(data_dir.clone()).await?;

        OpenOptions::new()
            .create(true)
            .write(write)
            .read(true)
            .append(false)
            .truncate(true)
            .open(data_dir.join("bookmarks.txt"))
            .await
            .map_err(BookmarkError::Io)
    }

    /// Loads bookmarks from the `bookmarks.txt` file.
    pub async fn load() -> eyre::Result<Self, BookmarkError> {
        let mut file = Self::open(false).await?;

        let mut text = String::new();
        file.read_to_string(&mut text).await?;

        let lines: Vec<String> = text
            .trim_start_matches("noheader")
            .trim()
            .lines()
            .filter_map(|x| {
                if x.is_empty() {
                    None
                } else {
                    Some(x.to_string())
                }
            })
            .collect();

        Ok(Self {
            entries: RwLock::new(lines),
            bookmarked: AtomicBool::new(false),
        })
    }

    // Saves the bookmarks to the `bookmarks.txt` file.
    pub async fn save(&self) -> eyre::Result<(), BookmarkError> {
        let mut file = Self::open(true).await?;
        let text = format!("noheader\n{}", self.entries.read().await.join("\n"));

        file.write_all(text.as_bytes()).await?;
        file.flush().await?;

        Ok(())
    }

    /// Bookmarks a given track with a full path and optional custom name.
    ///
    /// Returns whether the track is now bookmarked, or not.
    pub async fn bookmark(&self, track: &tracks::Info) -> eyre::Result<(), BookmarkError> {
        let entry = track.to_entry();
        let idx = self.entries.read().await.iter().position(|x| **x == entry);

        if let Some(idx) = idx {
            self.entries.write().await.remove(idx);
        } else {
            self.entries.write().await.push(entry);
        };

        self.bookmarked
            .swap(idx.is_none(), std::sync::atomic::Ordering::Relaxed);

        Ok(())
    }

    /// Returns whether a track is bookmarked or not by using the internal
    /// bookmarked register.
    pub fn bookmarked(&self) -> bool {
        self.bookmarked.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Sets the internal bookmarked register by checking against
    /// the current track's info.
    pub async fn set_bookmarked(&self, track: &tracks::Info) {
        let val = self.entries.read().await.contains(&track.to_entry());
        self.bookmarked
            .swap(val, std::sync::atomic::Ordering::Relaxed);
    }
}
