pub mod error;
use crate::{download::Downloader, ui::UI};
pub use error::{Error, Result};
pub mod message;
pub mod ui;
pub use message::Message;
use tokio::sync::mpsc::{self, Receiver};
pub mod audio;
pub mod download;

pub type Handle = tokio::task::JoinHandle<crate::Result<()>>;

pub struct Player {
    ui: UI,
    downloader: Downloader,
    sink: rodio::Sink,
    stream: rodio::OutputStream,
    rx: Receiver<crate::Message>,
}

impl Player {
    pub async fn init() -> crate::Result<Self> {
        #[cfg(target_os = "linux")]
        let mut stream = audio::silent_get_output_stream()?;
        #[cfg(not(target_os = "linux"))]
        let mut stream = rodio::OutputStreamBuilder::open_default_stream()?;

        stream.log_on_drop(false);
        let sink = rodio::Sink::connect_new(stream.mixer());
        let (tx, rx) = mpsc::channel(8);

        Ok(Self {
            downloader: Downloader::init(5).await,
            ui: UI::init(tx).await,
            rx,
            sink,
            stream,
        })
    }
}

#[tokio::main]
pub async fn main() -> crate::Result<()> {
    let mut player: Player = Player::init().await?;
    player.ui.render(ui::Render { track: "test".to_owned() }).await?;
    
    while let Some(message) = player.rx.recv().await {
        if message == Message::Quit { break };
    }

    Ok(())
}
