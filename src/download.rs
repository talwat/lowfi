use tokio::sync::mpsc::{self, Receiver, Sender};

pub struct Downloader {
    /// TODO: Actually have a track type here.
    queue: Receiver<()>,
    handle: crate::Handle,
}

impl Downloader {
    async fn downloader(tx: Sender<()>) -> crate::Result<()> {
        
        // todo
        Ok(())
    }

    pub async fn init(buffer_size: usize) -> Self {
        let (tx, rx) = mpsc::channel(buffer_size);
        Self {
            queue: rx,
            handle: tokio::spawn(Self::downloader(tx)),
        }
    }
}

pub async fn downloader() {

}