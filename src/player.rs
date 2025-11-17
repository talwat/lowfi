use std::sync::Arc;

use tokio::sync::{
    broadcast,
    mpsc::{self, Receiver, Sender},
};

use crate::{
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

pub struct Player {
    downloader: download::Handle,
    volume: PersistentVolume,
    bookmarks: Bookmarks,
    sink: Arc<rodio::Sink>,
    rx: Receiver<crate::Message>,
    broadcast: broadcast::Sender<ui::Update>,
    current: Current,
    _ui: ui::Handle,
    _tx: Sender<crate::Message>,
    _stream: rodio::OutputStream,
}

impl Drop for Player {
    fn drop(&mut self) {
        self.sink.stop();
    }
}

impl Player {
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
        let mut stream = audio::silent_get_output_stream()?;
        #[cfg(not(target_os = "linux"))]
        let mut stream = rodio::OutputStreamBuilder::open_default_stream()?;
        stream.log_on_drop(false);
        let sink = Arc::new(rodio::Sink::connect_new(stream.mixer()));

        let (tx, rx) = mpsc::channel(8);
        tx.send(Message::Init).await?;
        let (utx, urx) = broadcast::channel(8);
        let current = Current::Loading(download::progress());

        let state = ui::State::initial(sink.clone(), &args, current.clone());
        let ui = ui::Handle::init(tx.clone(), urx, state.clone(), &args).await?;

        let volume = PersistentVolume::load().await?;
        let bookmarks = Bookmarks::load().await?;

        let list = List::load(args.track_list.as_ref()).await?;
        let downloader = Downloader::init(args.buffer_size, list, tx.clone()).await;

        Ok(Self {
            current,
            downloader,
            broadcast: utx,
            rx,
            sink,
            bookmarks,
            volume,
            _ui: ui,
            _stream: stream,
            _tx: tx,
        })
    }

    pub async fn close(&self) -> crate::Result<()> {
        self.bookmarks.save().await?;
        self.volume.save().await?;

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
                Message::Next | Message::Init | Message::Loaded => {
                    self.sink.stop();
                    match self.downloader.track().await {
                        download::Output::Loading(progress) => {
                            self.set_current(Current::Loading(progress)).await?
                        }
                        download::Output::Queued(queued) => self.play(queued).await?,
                    };
                }
                Message::Play => {
                    self.sink.play();

                    // #[cfg(feature = "mpris")]
                    // mpris.playback(PlaybackStatus::Playing).await?;
                }
                Message::Pause => {
                    self.sink.pause();

                    // #[cfg(feature = "mpris")]
                    // mpris.playback(PlaybackStatus::Paused).await?;
                }
                Message::PlayPause => {
                    if self.sink.is_paused() {
                        self.sink.play();
                    } else {
                        self.sink.pause();
                    }

                    // #[cfg(feature = "mpris")]
                    // mpris
                    // .playback(mpris.player().playback_status().await?)
                    // .await?;
                }
                Message::ChangeVolume(change) => {
                    self.sink.set_volume(self.sink.volume() + change);

                    // #[cfg(feature = "mpris")]
                    // mpris
                    // .changed(vec![Property::Volume(player.sink.volume().into())])
                    // .await?;
                }
                Message::Bookmark => {
                    let Current::Track(current) = &self.current else {
                        continue;
                    };

                    self.bookmarks.bookmark(current).await?;
                }
                Message::Quit => break,
            }
        }

        // self.close().await?;
        Ok(())
    }
}
