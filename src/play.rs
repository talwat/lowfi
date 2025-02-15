//! Responsible for the basic initialization & shutdown of the audio server & frontend.

use std::path::PathBuf;
use std::sync::Arc;

use eyre::eyre;
use tokio::fs;
use tokio::{sync::mpsc, task};

use crate::player::Player;
use crate::player::{ui, Messages};
use crate::Args;

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
        let config = dirs::config_dir()
            .ok_or(eyre!("Couldn't find config directory"))?
            .join(PathBuf::from("lowfi"));

        if !config.exists() {
            fs::create_dir_all(&config).await?;
        }

        Ok(config)
    }

    /// Returns the volume as a float from 0 to 1.
    pub fn float(self) -> f32 {
        self.inner as f32 / 100.0
    }

    /// Loads the [`PersistentVolume`] from [`dirs::config_dir()`].
    pub async fn load() -> eyre::Result<Self> {
        let config = Self::config().await?;
        let volume = config.join(PathBuf::from("volume.txt"));

        // Basically just read from the volume file if it exists, otherwise return 100.
        let volume = if volume.exists() {
            let contents = fs::read_to_string(volume).await?;
            let trimmed = contents.trim();
            let stripped = trimmed.strip_suffix("%").unwrap_or(trimmed);
            stripped
                .parse()
                .map_err(|_error| eyre!("volume.txt file is invalid"))?
        } else {
            fs::write(&volume, "100").await?;
            100u16
        };

        Ok(Self { inner: volume })
    }

    /// Saves `volume` to `volume.txt`.
    pub async fn save(volume: f32) -> eyre::Result<()> {
        let config = Self::config().await?;
        let path = config.join(PathBuf::from("volume.txt"));

        fs::write(path, ((volume * 100.0).abs().round() as u16).to_string()).await?;

        Ok(())
    }
}

/// Initializes the audio server, and then safely stops
/// it when the frontend quits.
pub async fn play(args: Args) -> eyre::Result<()> {
    // Actually initializes the player.
    let player = Arc::new(Player::new(&args).await?);

    // Initialize the UI, as well as the internal communication channel.
    let (tx, rx) = mpsc::channel(8);
    let ui = task::spawn(ui::start(Arc::clone(&player), tx.clone(), args));

    // Sends the player an "init" signal telling it to start playing a song straight away.
    tx.send(Messages::Init).await?;

    // Actually starts the player.
    Player::play(Arc::clone(&player), tx.clone(), rx).await?;

    // Save the volume.txt file for the next session.
    PersistentVolume::save(player.sink.volume()).await?;
    player.sink.stop();
    ui.abort();

    Ok(())
}
