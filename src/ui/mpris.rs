//! Contains the code for the MPRIS server & other helper functions.

use std::{
    env,
    hash::{DefaultHasher, Hash, Hasher},
    process,
    sync::Arc,
};

use arc_swap::ArcSwap;
use mpris_server::{
    zbus::{self, fdo, Result},
    LoopStatus, Metadata, PlaybackRate, PlaybackStatus, PlayerInterface, Property, RootInterface,
    Time, TrackId, Volume,
};
use rodio::Sink;
use tokio::sync::{broadcast, mpsc};

use crate::{player::Current, ui::Update};
use crate::{ui, Message};

const ERROR: fdo::Error = fdo::Error::Failed(String::new());

struct Sender {
    inner: mpsc::Sender<Message>,
}

impl Sender {
    pub fn new(inner: mpsc::Sender<Message>) -> Self {
        Self { inner }
    }

    pub async fn send(&self, message: Message) -> fdo::Result<()> {
        self.inner
            .send(message)
            .await
            .map_err(|x| fdo::Error::Failed(x.to_string()))
    }

    pub async fn zbus(&self, message: Message) -> zbus::Result<()> {
        self.inner
            .send(message)
            .await
            .map_err(|x| zbus::Error::Failure(x.to_string()))
    }
}

impl Into<fdo::Error> for crate::Error {
    fn into(self) -> fdo::Error {
        fdo::Error::Failed(self.to_string())
    }
}

/// The actual MPRIS player.
pub struct Player {
    sink: Arc<Sink>,
    current: ArcSwap<Current>,
    list: String,
    sender: Sender,
}

impl RootInterface for Player {
    async fn raise(&self) -> fdo::Result<()> {
        Err(ERROR)
    }

    async fn quit(&self) -> fdo::Result<()> {
        self.sender.send(Message::Quit).await
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
        self.sender.send(Message::Next).await
    }

    async fn previous(&self) -> fdo::Result<()> {
        Err(ERROR)
    }

    async fn pause(&self) -> fdo::Result<()> {
        self.sender.send(Message::Pause).await
    }

    async fn play_pause(&self) -> fdo::Result<()> {
        self.sender.send(Message::PlayPause).await
    }

    async fn stop(&self) -> fdo::Result<()> {
        self.pause().await
    }

    async fn play(&self) -> fdo::Result<()> {
        self.sender.send(Message::Play).await
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
        Ok(if self.current.load().loading() {
            PlaybackStatus::Stopped
        } else if self.sink.is_paused() {
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
        Ok(self.sink.speed().into())
    }

    async fn set_rate(&self, rate: PlaybackRate) -> Result<()> {
        self.sink.set_speed(rate as f32);
        Ok(())
    }

    async fn shuffle(&self) -> fdo::Result<bool> {
        Ok(true)
    }

    async fn set_shuffle(&self, _shuffle: bool) -> Result<()> {
        Ok(())
    }

    async fn metadata(&self) -> fdo::Result<Metadata> {
        Ok(match self.current.load().as_ref() {
            Current::Loading(_) => Metadata::new(),
            Current::Track(track) => {
                let mut hasher = DefaultHasher::new();
                track.path.hash(&mut hasher);

                let id = mpris_server::zbus::zvariant::ObjectPath::try_from(format!(
                    "/com/talwat/lowfi/{}/{}",
                    self.list,
                    hasher.finish()
                ))
                .unwrap();

                let mut metadata = Metadata::builder()
                    .trackid(id)
                    .title(track.display.clone())
                    .album(self.list.clone())
                    .build();

                metadata.set_length(
                    track
                        .duration
                        .map(|x| Time::from_micros(x.as_micros() as i64)),
                );

                metadata
            }
        })
    }

    async fn volume(&self) -> fdo::Result<Volume> {
        Ok(self.sink.volume().into())
    }

    async fn set_volume(&self, volume: Volume) -> Result<()> {
        self.sender.zbus(Message::SetVolume(volume as f32)).await
    }

    async fn position(&self) -> fdo::Result<Time> {
        Ok(Time::from_micros(self.sink.get_pos().as_micros() as i64))
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

    /// Broadcast reciever.
    reciever: broadcast::Receiver<Update>,
}

impl Server {
    /// Shorthand to emit a `PropertiesChanged` signal, like when pausing/unpausing.
    pub async fn changed(
        &mut self,
        properties: impl IntoIterator<Item = mpris_server::Property> + Send + Sync,
    ) -> ui::Result<()> {
        while let Ok(update) = self.reciever.try_recv() {
            if let Update::Track(current) = update {
                self.player().current.swap(Arc::new(current));
            }
        }
        self.inner.properties_changed(properties).await?;

        Ok(())
    }

    pub async fn update_volume(&mut self) -> ui::Result<()> {
        self.changed(vec![Property::Volume(self.player().sink.volume().into())])
            .await?;

        Ok(())
    }

    /// Shorthand to emit a `PropertiesChanged` signal, specifically about playback.
    pub async fn update_playback(&mut self) -> ui::Result<()> {
        let status = self.player().playback_status().await?;
        self.changed(vec![Property::PlaybackStatus(status)]).await?;

        Ok(())
    }

    pub async fn update_metadata(&mut self) -> ui::Result<()> {
        let metadata = self.player().metadata().await?;
        self.changed(vec![Property::Metadata(metadata)]).await?;

        Ok(())
    }

    /// Shorthand to get the inner mpris player object.
    pub fn player(&self) -> &Player {
        self.inner.imp()
    }

    /// Creates a new MPRIS server.
    pub async fn new(
        state: ui::State,
        sender: mpsc::Sender<Message>,
        reciever: broadcast::Receiver<Update>,
    ) -> ui::Result<Server> {
        let suffix = if env::var("LOWFI_FIXED_MPRIS_NAME").is_ok_and(|x| x == "1") {
            String::from("lowfi")
        } else {
            format!("lowfi.{}.instance{}", state.list, process::id())
        };

        let server = mpris_server::Server::new(
            &suffix,
            Player {
                sender: Sender::new(sender),
                sink: state.sink,
                current: ArcSwap::new(Arc::new(state.current)),
                list: state.list,
            },
        )
        .await?;

        Ok(Self {
            inner: server,
            reciever,
        })
    }
}
