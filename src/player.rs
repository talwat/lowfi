use std::sync::Arc;

use tokio::sync::mpsc::{self, Receiver};

use crate::{
    audio::waiter,
    bookmark::Bookmarks,
    download,
    tracks::{self, List},
    ui,
    volume::PersistentVolume,
    Message, Tasks,
};

/// Represents the currently known playback state.
///
/// * [`Current::Loading`] indicates the player is waiting for data.
/// * [`Current::Track`] indicates the player has a decoded track available.
#[derive(Clone, Debug)]
pub enum Current {
    /// Waiting for a track to arrive. The optional `Progress` is used to
    /// indicate global download progress when present.
    Loading(Option<download::Progress>),

    /// A decoded track that can be played; contains the track `Info`.
    Track(tracks::Info),
}

impl Default for Current {
    fn default() -> Self {
        // By default the player starts in a loading state with no progress.
        Self::Loading(None)
    }
}

impl Current {
    /// Returns `true` if this `Current` value represents a loading state.
    pub const fn loading(&self) -> bool {
        matches!(self, Self::Loading(_))
    }
}

/// The high-level application player.
///
/// `Player` composes the downloader, UI, audio sink and bookkeeping state.
/// It owns background `Handle`s and drives the main message loop in `run`.
pub struct Player {
    /// Persistent bookmark storage used by the player.
    bookmarks: Bookmarks,

    /// Current playback state (loading or track).
    current: Current,

    /// Background downloader that fills the internal queue.
    downloader: download::Handle,

    /// Receiver for incoming `Message` commands.
    rx: Receiver<crate::Message>,

    /// Shared audio sink used for playback.
    sink: Arc<rodio::Sink>,

    /// UI handle for rendering and input.
    ui: ui::Handle,

    /// Notifies when a play head has been appended.
    waiter: waiter::Handle,
}

impl Player {
    /// Sets the in-memory current state and notifies the UI about the change.
    ///
    /// If the new state is a `Track`, this will also update the bookmarked flag
    /// based on persistent bookmarks.
    pub fn set_current(&mut self, current: Current) -> crate::Result<()> {
        self.current = current.clone();
        self.ui.update(ui::Update::Track(current))?;

        let Current::Track(track) = &self.current else {
            return Ok(());
        };

        let bookmarked = self.bookmarks.bookmarked(track);
        self.ui.update(ui::Update::Bookmarked(bookmarked))?;

        Ok(())
    }

    /// Initialize a `Player` with the provided CLI `args` and audio `mixer`.
    ///
    /// This sets up the audio sink, UI, downloader, bookmarks and persistent
    /// volume state. The function returns a fully constructed `Player` ready
    /// to be driven via `run`.
    pub async fn init(
        args: crate::Args,
        mixer: &rodio::mixer::Mixer,
    ) -> crate::Result<(Self, crate::Tasks)> {
        let (tx, rx) = mpsc::channel(8);
        let mut tasks = Tasks::new(tx.clone());
        if args.paused {
            tx.send(Message::Pause).await?;
        }

        tx.send(Message::Init).await?;
        let list = List::load(args.track_list.as_ref()).await?;

        let sink = Arc::new(rodio::Sink::connect_new(mixer));
        let state = ui::State::initial(Arc::clone(&sink), list.name.clone());

        let volume = PersistentVolume::load().await?;
        sink.set_volume(volume.float());

        let player = Self {
            ui: tasks.ui(state, &args).await?,
            downloader: tasks.downloader(args.buffer_size as usize, args.timeout, list)?,
            waiter: tasks.waiter(Arc::clone(&sink)),
            bookmarks: Bookmarks::load().await?,
            current: Current::default(),
            rx,
            sink,
        };

        Ok((player, tasks))
    }

    /// Close any outlying processes, as well as persist state that
    /// should survive such as bookmarks and volume.
    pub async fn close(self) -> crate::Result<()> {
        self.sink.stop();
        self.bookmarks.save().await?;
        PersistentVolume::save(self.sink.volume()).await?;

        Ok(())
    }

    /// Play a queued track by decoding, appending to the sink and notifying
    /// other subsystems that playback has changed.
    pub fn play(&mut self, queued: tracks::Queued) -> crate::Result<()> {
        let decoded = queued.decode()?;
        self.sink.append(decoded.data);
        self.set_current(Current::Track(decoded.info))?;
        self.waiter.notify();

        Ok(())
    }

    /// Drives the main message loop of the player.
    ///
    /// This will return when a `Message::Quit` is received. It handles commands
    /// coming from the frontend and updates playback/UI state accordingly.
    pub async fn run(&mut self) -> crate::Result<()> {
        while let Some(message) = self.rx.recv().await {
            match message {
                Message::Next | Message::Init | Message::Loaded => {
                    if message == Message::Next && self.current.loading() {
                        continue;
                    }

                    self.sink.stop();
                    match self.downloader.track() {
                        download::Output::Loading(progress) => {
                            self.set_current(Current::Loading(progress))?;
                        }
                        download::Output::Queued(queued) => self.play(queued)?,
                    }
                }
                Message::Play => {
                    self.sink.play();
                }
                Message::Pause => {
                    self.sink.pause();
                }
                Message::PlayPause => {
                    if self.sink.is_paused() {
                        self.sink.play();
                    } else {
                        self.sink.pause();
                    }
                }
                Message::ChangeVolume(change) => {
                    self.sink
                        .set_volume((self.sink.volume() + change).clamp(0.0, 1.0));
                    self.ui.update(ui::Update::Volume)?;
                }
                Message::SetVolume(set) => {
                    self.sink.set_volume(set.clamp(0.0, 1.0));
                    self.ui.update(ui::Update::Volume)?;
                }
                Message::Bookmark => {
                    let Current::Track(current) = &self.current else {
                        continue;
                    };

                    let bookmarked = self.bookmarks.bookmark(current)?;
                    self.ui.update(ui::Update::Bookmarked(bookmarked))?;
                }
                Message::Quit => break,
            }

            #[cfg(feature = "mpris")]
            self.ui.mpris.handle(&message).await?;
        }

        Ok(())
    }
}
