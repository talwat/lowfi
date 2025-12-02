use std::{sync::Arc, thread::sleep, time::Duration};

use rodio::Sink;
use tokio::{
    sync::{mpsc, Notify},
    task::{self, JoinHandle},
};

pub struct Handle {
    _task: JoinHandle<()>,
    notify: Arc<Notify>,
}

impl Handle {
    pub fn new(sink: Arc<Sink>, tx: mpsc::Sender<crate::Message>) -> Self {
        let notify = Arc::new(Notify::new());

        Self {
            _task: task::spawn(Self::waiter(sink, tx, notify.clone())),
            notify,
        }
    }

    pub fn notify(&self) {
        self.notify.notify_one();
    }

    async fn waiter(sink: Arc<Sink>, tx: mpsc::Sender<crate::Message>, notify: Arc<Notify>) {
        'main: loop {
            notify.notified().await;

            while !sink.empty() {
                if Arc::strong_count(&notify) <= 1 {
                    break 'main;
                }

                sleep(Duration::from_millis(8));
            }

            if let Err(_) = tx.try_send(crate::Message::Next) {
                break;
            };
        }
    }
}
