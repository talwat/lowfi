use std::sync::{atomic::AtomicU8, Arc};

use reqwest::Client;
use tokio::sync::mpsc::{self, Receiver};

use crate::{
    bookmark::Bookmarks, download::Downloader, tracks::List, ui::UI, volume::PersistentVolume,
    Message,
};

pub struct Player {
    ui: UI,
    volume: PersistentVolume,
    bookmarks: Bookmarks,
    downloader: Downloader,
    sink: Arc<rodio::Sink>,
    stream: rodio::OutputStream,
    rx: Receiver<crate::Message>,
}

impl Drop for Player {
    fn drop(&mut self) {
        self.sink.stop();
    }
}

impl Player {
    pub async fn init(args: crate::Args) -> crate::Result<Self> {
        #[cfg(target_os = "linux")]
        let mut stream = audio::silent_get_output_stream()?;
        #[cfg(not(target_os = "linux"))]
        let mut stream = rodio::OutputStreamBuilder::open_default_stream()?;
        stream.log_on_drop(false);
        let sink = Arc::new(rodio::Sink::connect_new(stream.mixer()));

        let progress = Arc::new(AtomicU8::new(0));
        let (tx, rx) = mpsc::channel(8);
        let ui = UI::init(tx, progress.clone(), sink.clone(), &args).await?;

        let volume = PersistentVolume::load().await?;
        let bookmarks = Bookmarks::load().await?;

        let client = Client::new();
        let list = List::load(args.track_list.as_ref()).await?;
        let downloader = Downloader::init(args.buffer_size, list, client, progress).await;

        Ok(Self {
            downloader,
            ui,
            rx,
            sink,
            stream,
            bookmarks,
            volume,
        })
    }

    pub async fn play(mut self) -> crate::Result<()> {
        // self.ui
        //     .render(ui::Update {
        //         track: None,
        //         bookmarked: false,
        //     })
        //     .await?;

        while let Some(message) = self.rx.recv().await {
            if message == Message::Quit {
                break;
            };
        }

        Ok(())
    }
}
