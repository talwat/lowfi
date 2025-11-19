use std::sync::Arc;

use crate::{
    player::Current,
    ui::{self, window::Window},
    Args,
};
use tokio::{
    sync::{broadcast, mpsc::Sender},
    task::JoinHandle,
    time::Instant,
};
mod components;
mod environment;
pub use environment::Environment;
mod input;
mod interface;
mod window;

#[cfg(feature = "mpris")]
pub mod mpris;

type Result<T> = std::result::Result<T, Error>;

/// The error type for the UI, which is used to handle errors that occur
/// while drawing the UI or handling input.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unable to convert number")]
    Conversion(#[from] std::num::TryFromIntError),

    #[error("unable to write output")]
    Write(#[from] std::io::Error),

    #[error("sending message to backend from ui failed")]
    CrateSend(#[from] tokio::sync::mpsc::error::SendError<crate::Message>),

    #[error("sharing state between backend and frontend failed")]
    UiSend(#[from] tokio::sync::broadcast::error::SendError<Update>),

    #[cfg(feature = "mpris")]
    #[error("mpris bus error")]
    ZBus(#[from] mpris_server::zbus::Error),

    #[cfg(feature = "mpris")]
    #[error("mpris fdo (zbus interface) error")]
    Fdo(#[from] mpris_server::zbus::fdo::Error),
}

#[derive(Clone)]
pub struct State {
    pub sink: Arc<rodio::Sink>,
    pub current: Current,
    pub bookmarked: bool,
    list: String,
    timer: Option<Instant>,
    width: usize,
}

impl State {
    pub fn initial(sink: Arc<rodio::Sink>, args: &Args, current: Current, list: String) -> Self {
        let width = 21 + args.width.min(32) * 2;
        Self {
            width,
            sink,
            current,
            list,
            bookmarked: false,
            timer: None,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Update {
    Track(Current),
    Bookmarked(bool),
    Volume,
    Quit,
}

#[derive(Debug)]
struct Tasks {
    render: JoinHandle<Result<()>>,
    input: JoinHandle<Result<()>>,
}

pub struct Handle {
    tasks: Tasks,
    pub environment: Environment,
    #[cfg(feature = "mpris")]
    pub mpris: mpris::Server,
}

impl Drop for Handle {
    fn drop(&mut self) {
        self.tasks.input.abort();
        self.tasks.render.abort();
    }
}

impl Handle {
    async fn ui(
        mut rx: broadcast::Receiver<Update>,
        mut state: State,
        params: interface::Params,
    ) -> Result<()> {
        let mut interval = tokio::time::interval(params.delta);
        let mut window = Window::new(state.width, params.borderless);

        loop {
            interface::draw(&mut state, &mut window, params).await?;

            if let Ok(message) = rx.try_recv() {
                match message {
                    Update::Track(track) => state.current = track,
                    Update::Bookmarked(bookmarked) => state.bookmarked = bookmarked,
                    Update::Volume => state.timer = Some(Instant::now()),
                    Update::Quit => break,
                }
            };

            interval.tick().await;
        }

        Ok(())
    }

    pub async fn init(
        tx: Sender<crate::Message>,
        updater: broadcast::Receiver<ui::Update>,
        state: State,
        args: &Args,
    ) -> Result<Self> {
        let environment = Environment::ready(args.alternate)?;
        Ok(Self {
            #[cfg(feature = "mpris")]
            mpris: mpris::Server::new(state.clone(), tx.clone(), updater.resubscribe()).await?,
            environment,
            tasks: Tasks {
                render: tokio::spawn(Self::ui(updater, state, interface::Params::from(args))),
                input: tokio::spawn(input::listen(tx)),
            },
        })
    }
}
