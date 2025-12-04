use std::sync::Arc;

use tokio::sync::{
    broadcast,
    mpsc::{self, Receiver, Sender},
};

use crate::{
    audio::waiter,
    bookmark::Bookmarks,
    download::{self, Downloader},
    tracks::{self, List},
    ui,
    volume::PersistentVolume,
    Message,
};

#[derive(Clone, Debug)]
pub enum Current {
    Loading(Option<download::Progress>),
    Track(tracks::Info),
}

impl Default for Current {
    fn default() -> Self {
        Self::Loading(None)
    }
}

impl Current {
    pub const fn loading(&self) -> bool {
        matches!(self, Self::Loading(_))
    }
}

pub struct Player {
    downloader: download::Handle,
    bookmarks: Bookmarks,
    sink: Arc<rodio::Sink>,
    rx: Receiver<crate::Message>,
    broadcast: broadcast::Sender<ui::Update>,
    current: Current,
    ui: ui::Handle,
    waiter: waiter::Handle,
    _tx: Sender<crate::Message>,
    _stream: rodio::OutputStream,
}

impl Drop for Player {
    fn drop(&mut self) {
        self.sink.stop();
    }
}

impl Player {
    pub const fn environment(&self) -> ui::Environment {
        self.ui.environment
    }

    pub fn set_current(&mut self, current: Current) -> crate::Result<()> {
        self.current = current.clone();
        self.update(ui::Update::Track(current))?;

        let Current::Track(track) = &self.current else {
            return Ok(());
        };

        let bookmarked = self.bookmarks.bookmarked(track);
        self.update(ui::Update::Bookmarked(bookmarked))?;

        Ok(())
    }

    pub fn update(&mut self, update: ui::Update) -> crate::Result<()> {
        self.broadcast.send(update)?;
        Ok(())
    }

    pub async fn init(args: crate::Args) -> crate::Result<Self> {
        #[cfg(target_os = "linux")]
        let mut stream = crate::audio::silent_get_output_stream()?;
        #[cfg(not(target_os = "linux"))]
        let mut stream = rodio::OutputStreamBuilder::open_default_stream()?;
        stream.log_on_drop(false);
        let sink = Arc::new(rodio::Sink::connect_new(stream.mixer()));

        let (tx, rx) = mpsc::channel(8);
        tx.send(Message::Init).await?;
        let (utx, urx) = broadcast::channel(8);

        let list = List::load(args.track_list.as_ref()).await?;
        let state = ui::State::initial(Arc::clone(&sink), args.width, list.name.clone());

        let volume = PersistentVolume::load().await?;
        sink.set_volume(volume.float());

        Ok(Self {
            ui: ui::Handle::init(tx.clone(), urx, state, &args).await?,
            downloader: Downloader::init(args.buffer_size as usize, list, tx.clone()),
            waiter: waiter::Handle::new(Arc::clone(&sink), tx.clone()),
            bookmarks: Bookmarks::load().await?,
            current: Current::default(),
            broadcast: utx,
            rx,
            sink,
            _tx: tx,
            _stream: stream,
        })
    }

    pub async fn close(&self) -> crate::Result<()> {
        self.bookmarks.save().await?;
        PersistentVolume::save(self.sink.volume()).await?;

        Ok(())
    }

    pub fn play(&mut self, queued: tracks::Queued) -> crate::Result<()> {
        let decoded = queued.decode()?;
        self.sink.append(decoded.data);
        self.set_current(Current::Track(decoded.info))?;
        self.waiter.notify();

        Ok(())
    }

    pub async fn run(mut self) -> crate::Result<()> {
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
                        download::Output::Queued(queued) => {
                            self.play(queued)?;
                        }
                    };
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
                    self.update(ui::Update::Volume)?;
                }
                Message::SetVolume(set) => {
                    self.sink.set_volume(set.clamp(0.0, 1.0));
                    self.update(ui::Update::Volume)?;
                }
                Message::Bookmark => {
                    let Current::Track(current) = &self.current else {
                        continue;
                    };

                    let bookmarked = self.bookmarks.bookmark(current)?;
                    self.update(ui::Update::Bookmarked(bookmarked))?;
                }
                Message::Quit => break,
            }

            #[cfg(feature = "mpris")]
            match message {
                Message::ChangeVolume(_) | Message::SetVolume(_) => {
                    self.ui.mpris.update_volume().await?
                }
                Message::Play | Message::Pause | Message::PlayPause => {
                    self.ui.mpris.update_playback().await?
                }
                Message::Init | Message::Loaded | Message::Next => {
                    self.ui.mpris.update_metadata().await?
                }
                _ => (),
            }
        }

        self.close().await?;
        Ok(())
    }
}
