//! Module for handling saving, loading, and adding bookmarks.

use std::path::PathBuf;
use tokio::{fs, io};

use crate::{data_dir, tracks};

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
    entries: Vec<String>,
}

impl Bookmarks {
    /// Gets the path of the bookmarks file.
    pub async fn path() -> Result<PathBuf> {
        let data_dir = data_dir().map_err(|_| Error::Directory)?;
        fs::create_dir_all(data_dir.clone()).await?;

        Ok(data_dir.join("bookmarks.txt"))
    }

    /// Loads bookmarks from the `bookmarks.txt` file.
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
                    Some(x.to_string())
                }
            })
            .collect();

        Ok(Self { entries })
    }

    // Saves the bookmarks to the `bookmarks.txt` file.
    pub async fn save(&self) -> Result<()> {
        let text = format!("noheader\n{}", self.entries.join("\n"));
        fs::write(Self::path().await?, text).await?;
        Ok(())
    }

    /// Bookmarks a given track with a full path and optional custom name.
    ///
    /// Returns whether the track is now bookmarked, or not.
    pub async fn bookmark(&mut self, track: &tracks::Info) -> Result<bool> {
        let entry = track.to_entry();
        let idx = self.entries.iter().position(|x| **x == entry);

        if let Some(idx) = idx {
            self.entries.remove(idx);
        } else {
            self.entries.push(entry);
        };

        Ok(idx.is_none())
    }

    /// Sets the internal bookmarked register by checking against
    /// the current track's info.
    pub fn bookmarked(&mut self, track: &tracks::Info) -> bool {
        self.entries.contains(&track.to_entry())
    }
}
