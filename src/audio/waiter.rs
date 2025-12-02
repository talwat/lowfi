use std::{sync::Arc, thread::sleep, time::Duration};

use rodio::Sink;
use tokio::{
    sync::{broadcast, mpsc},
    task::{self, JoinHandle},
};

use crate::ui::{self, Update};

pub struct Handle {
    _task: JoinHandle<()>,
}

impl Handle {
    pub fn new(
        sink: Arc<Sink>,
        tx: mpsc::Sender<crate::Message>,
        rx: broadcast::Receiver<ui::Update>,
    ) -> Self {
        Self {
            _task: task::spawn_blocking(|| Self::waiter(sink, tx, rx)),
        }
    }

    fn waiter(
        sink: Arc<Sink>,
        tx: mpsc::Sender<crate::Message>,
        mut rx: broadcast::Receiver<ui::Update>,
    ) {
        'main: loop {
            if !matches!(rx.blocking_recv(), Ok(Update::Track(_))) {
                continue;
            }

            while !sink.empty() {
                if matches!(rx.try_recv(), Ok(Update::Quit)) {
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
