//! Bookmark persistence and helpers.
//!
//! Bookmarks are persisted to `bookmarks.txt` inside the application data
//! directory and follow the same track-list entry format (see `tracks::Info::to_entry`).

use std::path::PathBuf;
use tokio::{fs, io};

use crate::{data_dir, tracks};

/// Result alias for bookmark operations.
type Result<T> = std::result::Result<T, Error>;

/// Errors that might occur while managing bookmarks.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("data directory not found")]
    Directory,

    #[error("io failure")]
    Io(#[from] io::Error),
}

/// Manages the bookmarks in the current player.
pub struct Bookmarks {
    /// The different entries in the bookmarks file.
    pub(crate) entries: Vec<String>,
}

impl Bookmarks {
    /// Returns the path to `bookmarks.txt`, creating the parent directory
    /// if necessary.
    pub async fn path() -> Result<PathBuf> {
        let data_dir = data_dir().map_err(|_| Error::Directory)?;
        fs::create_dir_all(data_dir.clone()).await?;

        Ok(data_dir.join("bookmarks.txt"))
    }

    /// Loads bookmarks from disk. If no file exists an empty list is returned.
    pub async fn load() -> Result<Self> {
        let text = fs::read_to_string(Self::path().await?)
            .await
            .unwrap_or_default();

        let entries: Vec<String> = text
            .trim_start_matches("noheader")
            .trim()
            .lines()
            .filter_map(|x| {
                if x.is_empty() {
                    None
                } else {
                    Some(x.to_owned())
                }
            })
            .collect();

        Ok(Self { entries })
    }

    /// Saves bookmarks to disk in `bookmarks.txt`.
    pub async fn save(&self) -> Result<()> {
        let text = format!("noheader\n{}", self.entries.join("\n"));
        fs::write(Self::path().await?, text).await?;
        Ok(())
    }

    /// Toggles bookmarking for `track` and returns whether it is now bookmarked.
    ///
    /// If the track exists it is removed; otherwise it is appended to the list.
    pub fn bookmark(&mut self, track: &tracks::Info) -> Result<bool> {
        let entry = track.to_entry();
        let idx = self.entries.iter().position(|x| **x == entry);

        if let Some(idx) = idx {
            self.entries.remove(idx);
        } else {
            self.entries.push(entry);
        }

        Ok(idx.is_none())
    }

    /// Returns true if `track` is currently bookmarked.
    pub fn bookmarked(&mut self, track: &tracks::Info) -> bool {
        self.entries.contains(&track.to_entry())
    }
}
