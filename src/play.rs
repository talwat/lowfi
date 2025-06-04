//! Responsible for the basic initialization & shutdown of the audio server & frontend.

use std::env;
use std::io::{stdout, IsTerminal};
use std::path::PathBuf;
use std::sync::Arc;

use eyre::eyre;
use tokio::fs;
use tokio::{sync::mpsc, task};

use crate::messages::Messages;
use crate::player::ui;
use crate::player::Player;
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
            .ok_or_else(|| eyre!("Couldn't find config directory"))?
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

        #[expect(
            clippy::as_conversions,
            clippy::cast_sign_loss,
            clippy::cast_possible_truncation,
            reason = "already rounded & absolute, therefore this should be safe"
        )]
        let percentage = (volume * 100.0).abs().round() as u16;

        fs::write(path, percentage.to_string()).await?;

        Ok(())
    }
}

/// Wrapper around [`rodio::OutputStream`] to implement [Send], currently unsafely.
///
/// This is more of a temporary solution until cpal implements [Send] on it's output stream.
pub struct SendableOutputStream(pub rodio::OutputStream);

// SAFETY: This is necessary because [OutputStream] does not implement [Send],
// due to some limitation with Android's Audio API.
// I'm pretty sure nobody will use lowfi with android, so this is safe.
#[expect(
    clippy::non_send_fields_in_send_ty,
    reason = "this is expected because of the nature of the struct"
)]
unsafe impl Send for SendableOutputStream {}

/// Initializes the audio server, and then safely stops
/// it when the frontend quits.
pub async fn play(args: Args) -> eyre::Result<()> {
    // Actually initializes the player.
    // Stream kept here in the master thread to keep it alive.
    let (player, stream) = Player::new(&args).await?;
    let player = Arc::new(player);

    // Initialize the UI, as well as the internal communication channel.
    let (tx, rx) = mpsc::channel(8);
    let ui = if stdout().is_terminal() && !(env::var("LOWFI_DISABLE_UI") == Ok("1".to_owned())) {
        Some(task::spawn(ui::start(
            Arc::clone(&player),
            tx.clone(),
            args.clone(),
        )))
    } else {
        None
    };

    // Sends the player an "init" signal telling it to start playing a song straight away.
    tx.send(Messages::Init).await?;

    // Actually starts the player.
    Player::play(
        Arc::clone(&player),
        tx.clone(),
        rx,
        args.buffer_size,
        args.debug,
    )
    .await?;

    // Save the volume.txt file for the next session.
    PersistentVolume::save(player.sink.volume()).await?;
    drop(stream.0);
    player.sink.stop();
    ui.and_then(|x| Some(x.abort()));

    Ok(())
}
