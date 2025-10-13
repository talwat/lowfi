use eyre::eyre;
use std::path::PathBuf;
use tokio::fs;
use crate::debug_log;

/// This is the representation of the persistent volume,
/// which is loaded at startup and saved on shutdown.
#[derive(Clone, Copy)]
pub struct PersistentVolume {
    /// The volume, as a percentage.
    inner: u16,
}

impl PersistentVolume {
    /// Retrieves the config directory.
    async fn config() -> eyre::Result<PathBuf> {
        debug_log!("persistent_volume.rs - config: getting config directory");
        let config = dirs::config_dir()
            .ok_or_else(|| eyre!("Couldn't find config directory"))?
            .join(PathBuf::from("lowfi"));

        if !config.exists() {
            debug_log!("persistent_volume.rs - config: creating config directory: {}", config.display());
            fs::create_dir_all(&config).await?;
        } else {
            debug_log!("persistent_volume.rs - config: config directory exists: {}", config.display());
        }

        Ok(config)
    }

    /// Returns the volume as a float from 0 to 1.
    pub fn float(self) -> f32 {
        f32::from(self.inner) / 100.0
    }

    /// Loads the [`PersistentVolume`] from [`dirs::config_dir()`].
    pub async fn load() -> eyre::Result<Self> {
        debug_log!("persistent_volume.rs - load: loading persistent volume");
        let config = Self::config().await?;
        let volume = config.join(PathBuf::from("volume.txt"));

        // Basically just read from the volume file if it exists, otherwise return 100.
        let volume = if volume.exists() {
            debug_log!("persistent_volume.rs - load: reading volume from file: {}", volume.display());
            let contents = fs::read_to_string(volume).await?;
            let trimmed = contents.trim();
            let stripped = trimmed.strip_suffix("%").unwrap_or(trimmed);
            let parsed_volume = stripped
                .parse()
                .map_err(|_error| eyre!("volume.txt file is invalid"))?;
            debug_log!("persistent_volume.rs - load: loaded volume: {}%", parsed_volume);
            parsed_volume
        } else {
            debug_log!("persistent_volume.rs - load: volume file not found, creating default: 100%");
            fs::write(&volume, "100").await?;
            100u16
        };

        Ok(Self { inner: volume })
    }

    /// Saves `volume` to `volume.txt`.
    pub async fn save(volume: f32) -> eyre::Result<()> {
        debug_log!("persistent_volume.rs - save: saving volume: {}", volume);
        let config = Self::config().await?;
        let path = config.join(PathBuf::from("volume.txt"));

        // Already rounded & absolute, therefore this should be safe.
        #[expect(
            clippy::as_conversions,
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation
        )]
        let percentage = (volume * 100.0).abs().round() as u16;

        debug_log!("persistent_volume.rs - save: writing volume to file: {}% -> {}", percentage, path.display());
        fs::write(path, percentage.to_string()).await?;

        Ok(())
    }
}
