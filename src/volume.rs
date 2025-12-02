use std::{num::ParseIntError, path::PathBuf};
use tokio::fs;

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("couldn't find config directory")]
    Directory,

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("error parsing volume integer: {0}")]
    Parse(#[from] ParseIntError),
}

/// This is the representation of the persistent volume,
/// which is loaded at startup and saved on shutdown.
#[derive(Clone, Copy)]
pub struct PersistentVolume {
    /// The volume, as a percentage.
    pub(crate) inner: u16,
}

impl PersistentVolume {
    /// Retrieves the config directory.
    async fn config() -> Result<PathBuf> {
        let config = dirs::config_dir()
            .ok_or_else(|| Error::Directory)?
            .join(PathBuf::from("lowfi"));

        if !config.exists() {
            fs::create_dir_all(&config).await?;
        }

        Ok(config)
    }

    /// Returns the volume as a float from 0 to 1.
    pub fn float(self) -> f32 {
        f32::from(self.inner) / 100.0
    }

    /// Loads the [`PersistentVolume`] from [`dirs::config_dir()`].
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

    /// Saves `volume` to `volume.txt`.
    pub async fn save(volume: f32) -> Result<()> {
        let config = Self::config().await?;
        let path = config.join(PathBuf::from("volume.txt"));

        // Already rounded & absolute, therefore this should be safe.
        #[expect(
            clippy::as_conversions,
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation
        )]
        let percentage = (volume * 100.0).abs().round() as u16;
        fs::write(path, percentage.to_string()).await?;

        Ok(())
    }
}
