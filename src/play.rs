//! Responsible for the basic initialization & shutdown of the audio server & frontend.

use std::path::PathBuf;
use std::sync::Arc;

use eyre::eyre;
use tokio::fs;
use tokio::{sync::mpsc, task};

use crate::player::Player;
use crate::player::{ui, Messages};
use crate::Args;

/// The attributes that are applied at startup.
/// This includes the volume, but also the config file.
///
/// The volume is seperated from the config since it specifically
/// will be written by lowfi, whereas the config will not.
pub struct InitialProperties {
    /// The volume, as a percentage.
    pub volume: u16,
}

impl InitialProperties {
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

    /// Loads the [InitialProperties], including the config and volume file.
    pub async fn load() -> eyre::Result<Self> {
        let config = Self::config().await?;

        let volume = config.join(PathBuf::from("volume.txt"));

        // Basically just read from the volume file if it exists, otherwise return 100.
        let volume = if volume.exists() {
            let contents = fs::read_to_string(volume).await?;
            let stripped = contents.trim().strip_suffix("%").unwrap_or(&contents);
            stripped
                .parse()
                .map_err(|_| eyre!("volume.txt file is invalid"))?
        } else {
            fs::write(&volume, "100").await?;
            100u16
        };

        Ok(InitialProperties { volume })
    }

    /// Saves `volume.txt`, and uses the home directory which was previously acquired.
    pub async fn save_volume(volume: f32) -> eyre::Result<()> {
        let config = Self::config().await?;
        let path = config.join(PathBuf::from("volume.txt"));

        fs::write(path, ((volume * 100.0).abs().round() as u16).to_string()).await?;

        Ok(())
    }
}

/// Initializes the audio server, and then safely stops
/// it when the frontend quits.
pub async fn play(args: Args) -> eyre::Result<()> {
    // Load the initial properties (volume & config).
    let properties = InitialProperties::load().await?;

    let (tx, rx) = mpsc::channel(8);
    let player = Arc::new(Player::new(!args.alternate, &args).await?);
    let ui = task::spawn(ui::start(Arc::clone(&player), tx.clone(), args));

    tx.send(Messages::Init).await?;

    Player::play(Arc::clone(&player), properties, tx.clone(), rx).await?;

    // Save the volume.txt file for the next session.
    InitialProperties::save_volume(player.sink.volume()).await?;
    player.sink.stop();
    ui.abort();

    Ok(())
}
