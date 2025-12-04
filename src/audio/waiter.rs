use std::{sync::Arc, time::Duration};

use rodio::Sink;
use tokio::{
    sync::{mpsc, Notify},
    task::{self, JoinHandle},
    time,
};

pub struct Handle {
    task: JoinHandle<()>,
    notify: Arc<Notify>,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl Handle {
    pub fn new(sink: Arc<Sink>, tx: mpsc::Sender<crate::Message>) -> Self {
        let notify = Arc::new(Notify::new());

        Self {
            task: task::spawn(Self::waiter(sink, tx, Arc::clone(&notify))),
            notify,
        }
    }

    pub fn notify(&self) {
        self.notify.notify_one();
    }

    async fn waiter(sink: Arc<Sink>, tx: mpsc::Sender<crate::Message>, notify: Arc<Notify>) {
        loop {
            notify.notified().await;

            while !sink.empty() {
                time::sleep(Duration::from_millis(8)).await;
            }

            if tx.try_send(crate::Message::Next).is_err() {
                break;
            };
        }
    }
}
