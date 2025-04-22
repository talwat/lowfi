//! Contains the code for the MPRIS server & other helper functions.

use std::{env, process, sync::Arc};

use mpris_server::{
    zbus::{self, fdo, Result},
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus, PlayerInterface, Property, RootInterface,
    Time, TrackId, Volume,
};
use tokio::sync::mpsc::Sender;

use super::ui;
use super::Messages;

const ERROR: fdo::Error = fdo::Error::Failed(String::new());

/// The actual MPRIS player.
pub struct Player {
    /// A reference to the [`super::Player`] itself.
    pub player: Arc<super::Player>,

    /// The audio server sender, which is used to communicate with
    /// the audio sender for skips and a few other inputs.
    pub sender: Sender<Messages>,
}

impl RootInterface for Player {
    async fn raise(&self) -> fdo::Result<()> {
        Err(ERROR)
    }

    async fn quit(&self) -> fdo::Result<()> {
        self.sender
            .send(Messages::Quit)
            .await
            .map_err(|_error| ERROR)
    }

    async fn can_quit(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn set_fullscreen(&self, _: bool) -> Result<()> {
        Ok(())
    }

    async fn can_set_fullscreen(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_raise(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn has_track_list(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn identity(&self) -> fdo::Result<String> {
        Ok("lowfi".to_owned())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok("dev.talwat.lowfi".to_owned())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["https".to_owned()])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["audio/mpeg".to_owned()])
    }
}

impl PlayerInterface for Player {
    async fn next(&self) -> fdo::Result<()> {
        self.sender
            .send(Messages::Next)
            .await
            .map_err(|_error| ERROR)
    }

    async fn previous(&self) -> fdo::Result<()> {
        Err(ERROR)
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.sender
            .send(Messages::Pause)
            .await
            .map_err(|_error| ERROR)
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        self.sender
            .send(Messages::PlayPause)
            .await
            .map_err(|_error| ERROR)
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.pause().await
    }

    async fn play(&self) -> fdo::Result<()> {
        self.sender
            .send(Messages::Play)
            .await
            .map_err(|_error| ERROR)
    }

    async fn seek(&self, _offset: Time) -> fdo::Result<()> {
        Err(ERROR)
    }

    async fn set_position(&self, _track_id: TrackId, _position: Time) -> fdo::Result<()> {
        Err(ERROR)
    }

    async fn open_uri(&self, _uri: String) -> fdo::Result<()> {
        Err(ERROR)
    }

    async fn playback_status(&self) -> fdo::Result<PlaybackStatus> {
        Ok(if !self.player.current_exists() {
            PlaybackStatus::Stopped
        } else if self.player.sink.is_paused() {
            PlaybackStatus::Paused
        } else {
            PlaybackStatus::Playing
        })
    }

    async fn loop_status(&self) -> fdo::Result<LoopStatus> {
        Err(ERROR)
    }

    async fn set_loop_status(&self, _loop_status: LoopStatus) -> Result<()> {
        Ok(())
    }

    async fn rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(self.player.sink.speed().into())
    }

    async fn set_rate(&self, rate: PlaybackRate) -> Result<()> {
        self.player.sink.set_speed(rate as f32);
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn set_shuffle(&self, _shuffle: bool) -> Result<()> {
        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        let metadata = self
            .player
            .current
            .load()
            .as_ref()
            .map_or_else(Metadata::new, |track| {
                let mut metadata = Metadata::builder()
                    .title(track.display_name.clone())
                    .album(self.player.list.name.clone())
                    .build();

                metadata.set_length(
                    track
                        .duration
                        .map(|x| Time::from_micros(x.as_micros() as i64)),
                );

                metadata
            });

        Ok(metadata)
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(self.player.sink.volume().into())
    }

    async fn set_volume(&self, volume: Volume) -> Result<()> {
        self.player.set_volume(volume as f32);
        ui::flash_audio();

        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(Time::from_micros(
            self.player.sink.get_pos().as_micros() as i64
        ))
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(0.2f64)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(3.0f64)
    }

    async fn can_go_next(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_go_previous(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_play(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_pause(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn can_seek(&self) -> fdo::Result<bool> {
        Ok(false)
    }

    async fn can_control(&self) -> fdo::Result<bool> {
        Ok(true)
    }
}

/// A struct which contains the MPRIS [Server], and has some helper functions
/// to make it easier to work with.
pub struct Server {
    /// The inner MPRIS server.
    inner: mpris_server::Server<Player>,
}

impl Server {
    /// Shorthand to emit a `PropertiesChanged` signal, like when pausing/unpausing.
    pub async fn changed(
        &self,
        properties: impl IntoIterator<Item = mpris_server::Property> + Send + Sync,
    ) -> eyre::Result<()> {
        self.inner.properties_changed(properties).await?;

        Ok(())
    }

    /// Shorthand to emit a `PropertiesChanged` signal, specifically about playback.
    pub async fn playback(&self, new: PlaybackStatus) -> zbus::Result<()> {
        self.inner
            .properties_changed(vec![Property::PlaybackStatus(new)])
            .await
    }

    /// Shorthand to get the inner mpris player object.
    pub fn player(&self) -> &Player {
        self.inner.imp()
    }

    /// Creates a new MPRIS server.
    pub async fn new(player: Arc<super::Player>, sender: Sender<Messages>) -> eyre::Result<Self> {
        let suffix = if env::var("LOWFI_FIXED_MPRIS_NAME").is_ok_and(|x| x == "1") {
            String::from("lowfi")
        } else {
            format!("lowfi.{}.instance{}", player.list.name, process::id())
        };

        let server = mpris_server::Server::new(&suffix, Player { player, sender }).await?;

        Ok(Self { inner: server })
    }
}
