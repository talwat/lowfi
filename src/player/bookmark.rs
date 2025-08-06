use std::io::SeekFrom;
use std::sync::atomic::AtomicBool;

use tokio::fs::{create_dir_all, File, OpenOptions};
use tokio::io::{self, AsyncReadExt, AsyncSeekExt, AsyncWriteExt};
use tokio::sync::RwLock;

use crate::data_dir;

#[derive(Debug, thiserror::Error)]
pub enum BookmarkError {
    #[error("data directory not found")]
    DataDir,

    #[error("io failure")]
    Io(#[from] io::Error),
}

/// Manages the bookmarks in the current player.
pub struct Bookmarks {
    entries: RwLock<Vec<String>>,
    file: RwLock<File>,
    bookmarked: AtomicBool,
}

impl Bookmarks {
    pub async fn load() -> eyre::Result<Self, BookmarkError> {
        let data_dir = data_dir().map_err(|_| BookmarkError::DataDir)?;
        create_dir_all(data_dir.clone()).await?;

        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .read(true)
            .append(false)
            .truncate(false)
            .open(data_dir.join("bookmarks.txt"))
            .await?;

        let mut text = String::new();
        file.read_to_string(&mut text).await?;

        let lines: Vec<String> = text
            .trim()
            .lines()
            .filter_map(|x| {
                if !x.is_empty() {
                    Some(x.to_string())
                } else {
                    None
                }
            })
            .collect();

        Ok(Self {
            entries: RwLock::new(lines),
            file: RwLock::new(file),
            bookmarked: AtomicBool::new(false),
        })
    }

    pub async fn save(&self) -> eyre::Result<(), BookmarkError> {
        let text = format!("\n{}", self.entries.read().await.join("\n"));

        let mut lock = self.file.write().await;
        lock.seek(SeekFrom::Start(0)).await?;
        lock.set_len(0).await?;
        lock.write_all(text.as_bytes()).await?;
        lock.flush().await?;

        Ok(())
    }

    /// Bookmarks a given track with a full path and optional custom name.
    ///
    /// Returns whether the track is now bookmarked, or not.
    pub async fn bookmark(
        &self,
        mut entry: String,
        custom: Option<String>,
    ) -> eyre::Result<(), BookmarkError> {
        if let Some(custom) = custom {
            entry.push('!');
            entry.push_str(&custom);
        }

        let idx = self.entries.read().await.iter().position(|x| **x == entry);

        if let Some(idx) = idx {
            self.entries.write().await.remove(idx);
        } else {
            self.entries.write().await.push(entry);
        };

        self.set_bookmarked(idx.is_none());

        Ok(())
    }

    pub fn bookmarked(&self) -> bool {
        self.bookmarked.load(std::sync::atomic::Ordering::Relaxed)
    }

    pub fn set_bookmarked(&self, val: bool) {
        self.bookmarked
            .swap(val, std::sync::atomic::Ordering::Relaxed);
    }
}
