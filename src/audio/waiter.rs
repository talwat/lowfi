use std::{sync::Arc, time::Duration};

use rodio::Sink;
use tokio::{
    sync::{mpsc, Notify},
    time,
};

/// Background loop that waits for the sink to drain and then attempts
/// to send a `Message::Next` to the provided channel.
async fn waiter(
    sink: Arc<Sink>,
    tx: mpsc::Sender<crate::Message>,
    notify: Arc<Notify>,
) -> crate::Result<()> {
    loop {
        notify.notified().await;

        while !sink.empty() {
            time::sleep(Duration::from_millis(16)).await;
        }

        if tx.try_send(crate::Message::Next).is_err() {
            break Ok(());
        }
    }
}

/// Lightweight helper that waits for the current sink to drain and then
/// notifies the player to advance to the next track.
pub struct Handle {
    /// Notification primitive used to wake the waiter.
    notify: Arc<Notify>,
}

impl Handle {
    /// Notify the waiter that playback state may have changed and it should
    /// re-check the sink emptiness condition.
    pub fn notify(&self) {
        self.notify.notify_one();
    }
}

impl crate::Tasks {
    /// Create a new `Handle` which watches the provided `sink` and sends
    /// `Message::Next` down `tx` when the sink becomes empty.
    pub fn waiter(&mut self, sink: Arc<Sink>) -> Handle {
        let notify = Arc::new(Notify::new());
        self.spawn(waiter(sink, self.tx(), notify.clone()));

        Handle { notify }
    }
}
