use std::sync::Arc;

use tokio::{
    sync::{
        broadcast,
        mpsc::{self, Receiver},
    },
    task::JoinHandle,
};

use crate::{
    audio,
    bookmark::Bookmarks,
    download::{self, Downloader},
    tracks::{self, List},
    ui,
    volume::PersistentVolume,
    Message,
};

#[derive(Clone, Debug)]
pub enum Current {
    Loading(download::Progress),
    Track(tracks::Info),
}

impl Current {
    pub fn loading(&self) -> bool {
        return matches!(self, Current::Loading(_));
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
    waiter: JoinHandle<crate::Result<()>>,
    _stream: rodio::OutputStream,
}

impl Drop for Player {
    fn drop(&mut self) {
        self.sink.stop();
        self.waiter.abort();
    }
}

impl Player {
    pub fn environment(&self) -> ui::Environment {
        self.ui.environment
    }

    pub async fn set_current(&mut self, current: Current) -> crate::Result<()> {
        self.current = current.clone();
        self.update(ui::Update::Track(current)).await?;

        let Current::Track(track) = &self.current else {
            return Ok(());
        };

        let bookmarked = self.bookmarks.bookmarked(&track);
        self.update(ui::Update::Bookmarked(bookmarked)).await?;

        Ok(())
    }

    pub async fn update(&mut self, update: ui::Update) -> crate::Result<()> {
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
        let current = Current::Loading(download::progress());

        let list = List::load(args.track_list.as_ref()).await?;
        let state = ui::State::initial(sink.clone(), &args, current.clone(), list.name.clone());
        let ui = ui::Handle::init(tx.clone(), urx, state.clone(), &args).await?;

        let volume = PersistentVolume::load().await?;
        sink.set_volume(volume.float());
        let bookmarks = Bookmarks::load().await?;
        let downloader = Downloader::init(args.buffer_size, list, tx.clone()).await;

        let clone = sink.clone();
        let waiter = tokio::task::spawn_blocking(move || audio::waiter(clone, tx));

        Ok(Self {
            current,
            downloader,
            broadcast: utx,
            rx,
            sink,
            bookmarks,
            ui,
            waiter,
            _stream: stream,
        })
    }

    pub async fn close(&self) -> crate::Result<()> {
        self.bookmarks.save().await?;
        PersistentVolume::save(self.sink.volume() as f32).await?;

        Ok(())
    }

    pub async fn play(&mut self, queued: tracks::Queued) -> crate::Result<()> {
        let decoded = queued.decode()?;
        self.sink.append(decoded.data);
        self.set_current(Current::Track(decoded.info)).await?;

        Ok(())
    }

    pub async fn run(mut self) -> crate::Result<()> {
        while let Some(message) = self.rx.recv().await {
            match message {
                Message::Next | Message::Init | Message::Loaded | Message::End => {
                    if message == Message::Next && self.current.loading() {
                        continue;
                    }

                    audio::playing(false);
                    self.sink.stop();

                    match self.downloader.track().await {
                        download::Output::Loading(progress) => {
                            self.set_current(Current::Loading(progress)).await?;
                        }
                        download::Output::Queued(queued) => {
                            self.play(queued).await?;
                            audio::playing(true);
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
                    self.update(ui::Update::Volume).await?;
                }
                Message::SetVolume(set) => {
                    self.sink.set_volume(set.clamp(0.0, 1.0));
                    self.update(ui::Update::Volume).await?;
                }
                Message::Bookmark => {
                    let Current::Track(current) = &self.current else {
                        continue;
                    };

                    let bookmarked = self.bookmarks.bookmark(current).await?;
                    self.update(ui::Update::Bookmarked(bookmarked)).await?;
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
