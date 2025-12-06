use std::{sync::Arc, time::Duration};

use rodio::Sink;
use tokio::{
    sync::{mpsc, Notify},
    task::{self, JoinHandle},
    time,
};

/// Lightweight helper that waits for the current sink to drain and then
/// notifies the player to advance to the next track.
pub struct Handle {
    /// Background task monitoring the sink.
    task: JoinHandle<()>,

    /// Notification primitive used to wake the waiter.
    notify: Arc<Notify>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Handle {
    /// Create a new `Handle` which watches the provided `sink` and sends
    /// `Message::Next` down `tx` when the sink becomes empty.
    pub fn new(sink: Arc<Sink>, tx: mpsc::Sender<crate::Message>) -> Self {
        let notify = Arc::new(Notify::new());

        Self {
            task: task::spawn(Self::waiter(sink, tx, Arc::clone(&notify))),
            notify,
        }
    }

    /// Notify the waiter that playback state may have changed and it should
    /// re-check the sink emptiness condition.
    pub fn notify(&self) {
        self.notify.notify_one();
    }

    /// Background loop that waits for the sink to drain and then attempts
    /// to send a `Message::Next` to the provided channel.
    async fn waiter(sink: Arc<Sink>, tx: mpsc::Sender<crate::Message>, notify: Arc<Notify>) {
        loop {
            notify.notified().await;

            while !sink.empty() {
                time::sleep(Duration::from_millis(8)).await;
            }

            if tx.try_send(crate::Message::Next).is_err() {
                break;
            }
        }
    }
}
