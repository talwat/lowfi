use std::sync::Arc;

use mpris_server::{
    zbus::{fdo, Result},
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus, PlayerInterface, RootInterface, Time,
    TrackId, Volume,
};
use tokio::sync::mpsc::Sender;

use super::Messages;

const ERROR: fdo::Error = fdo::Error::Failed(String::new());

/// The actual MPRIS server.
pub struct Player {
    pub player: Arc<super::Player>,
    pub sender: Sender<Messages>,
}

impl RootInterface for Player {
    async fn raise(&self) -> fdo::Result<()> {
        Err(ERROR)
    }

    async fn quit(&self) -> fdo::Result<()> {
        self.sender.send(Messages::Quit).await.map_err(|_| ERROR)
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
        Ok("lowfi".to_string())
    }

    async fn desktop_entry(&self) -> fdo::Result<String> {
        Ok("dev.talwat.lowfi".to_string())
    }

    async fn supported_uri_schemes(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["https".to_string()])
    }

    async fn supported_mime_types(&self) -> fdo::Result<Vec<String>> {
        Ok(vec!["audio/mpeg".to_string()])
    }
}

impl PlayerInterface for Player {
    async fn next(&self) -> fdo::Result<()> {
        self.sender.send(Messages::Next).await.map_err(|_| ERROR)
    }

    async fn previous(&self) -> fdo::Result<()> {
        Err(ERROR)
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.sender
            .send(Messages::PlayPause)
            .await
            .map_err(|_| ERROR)
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        self.sender
            .send(Messages::PlayPause)
            .await
            .map_err(|_| ERROR)
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.play_pause().await
    }

    async fn play(&self) -> fdo::Result<()> {
        self.play_pause().await
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
        let metadata = match self.player.current.load().as_ref() {
            Some(track) => {
                let mut metadata = Metadata::builder().title(track.name.clone()).build();

                metadata.set_length(
                    track
                        .duration
                        .and_then(|x| Some(Time::from_micros(x.as_micros() as i64))),
                );

                metadata
            }
            None => Metadata::new(),
        };

        Ok(metadata)
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(self.player.sink.volume().into())
    }

    async fn set_volume(&self, volume: Volume) -> Result<()> {
        self.player.set_volume(volume as f32);

        Ok(())
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(Time::from_micros(
            self.player.sink.get_pos().as_micros() as i64
        ))
    }

    async fn minimum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(0.2)
    }

    async fn maximum_rate(&self) -> fdo::Result<PlaybackRate> {
        Ok(3.0)
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
