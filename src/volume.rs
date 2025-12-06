//! Persistent volume management.
//!
//! The module provides a tiny helper that reads and writes the user's
//! configured volume to `volume.txt` inside the platform config directory.
use std::{num::ParseIntError, path::PathBuf};
use tokio::fs;

/// Shorthand for a [`Result`] with a persistent volume error.
type Result<T> = std::result::Result<T, Error>;

/// Errors which occur when loading/unloading persistent volume.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("couldn't find config directory")]
    Directory,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("error parsing volume integer: {0}")]
    Parse(#[from] ParseIntError),
}

/// Representation of the persistent volume stored on disk.
///
/// The inner value is an integer percentage (0..=100). Use
/// [`PersistentVolume::float`] to convert to a normalized `f32` in the
/// range 0.0..=1.0 for playback volume calculations.
#[derive(Clone, Copy)]
pub struct PersistentVolume {
    /// The volume, as a percentage.
    pub(crate) inner: u16,
}

impl PersistentVolume {
    /// Retrieves the config directory, creating it if necessary.
    async fn config() -> Result<PathBuf> {
        let config = dirs::config_dir()
            .ok_or(Error::Directory)?
            .join(PathBuf::from("lowfi"));

        if !config.exists() {
            fs::create_dir_all(&config).await?;
        }

        Ok(config)
    }

    /// Returns the volume as a normalized float in the range 0.0..=1.0.
    pub fn float(self) -> f32 {
        f32::from(self.inner) / 100.0
    }

    /// Loads the [`PersistentVolume`] from the platform config directory.
    ///
    /// If the file does not exist a default of `100` is written and
    /// returned.
    pub async fn load() -> Result<Self> {
        let config = Self::config().await?;
        let volume = config.join(PathBuf::from("volume.txt"));

        // Basically just read from the volume file if it exists, otherwise return 100.
        let volume = if volume.exists() {
            let contents = fs::read_to_string(volume).await?;
            let trimmed = contents.trim();
            let stripped = trimmed.strip_suffix("%").unwrap_or(trimmed);
            stripped.parse()?
        } else {
            fs::write(&volume, "100").await?;
            100u16
        };

        Ok(Self { inner: volume })
    }

    /// Saves `volume` (0.0..=1.0) to `volume.txt` as an integer percent.
    pub async fn save(volume: f32) -> Result<()> {
        let config = Self::config().await?;
        let path = config.join(PathBuf::from("volume.txt"));
        let percentage = (volume * 100.0).abs().round() as u16;
        fs::write(path, percentage.to_string()).await?;

        Ok(())
    }
}
