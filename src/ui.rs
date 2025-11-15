use std::{
    sync::{atomic::AtomicU8, Arc},
    time::Duration,
};

use crate::{
    tracks,
    ui::{environment::Environment, window::Window},
    Args, Message,
};
use rodio::Sink;
use tokio::{
    sync::mpsc::{self, Receiver, Sender},
    task::JoinHandle,
};
mod components;
mod environment;
mod input;
mod interface;
mod window;

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
    Communication(#[from] tokio::sync::mpsc::error::SendError<Message>),
}

pub struct State {
    pub sink: Arc<rodio::Sink>,
    pub progress: Arc<AtomicU8>,
    pub track: Option<tracks::Info>,
    pub bookmarked: bool,
    width: usize,
}

impl State {
    pub fn update(&mut self, update: Update) {
        self.track = update.track;
        self.bookmarked = update.bookmarked;
    }

    pub fn initial(sink: Arc<rodio::Sink>, width: usize, progress: Arc<AtomicU8>) -> Self {
        Self {
            width,
            sink,
            progress,
            track: None,
            bookmarked: false,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Update {
    pub track: Option<tracks::Info>,
    pub bookmarked: bool,
}

#[derive(Debug)]
struct Handles {
    render: JoinHandle<Result<()>>,
    input: JoinHandle<Result<()>>,
}

#[derive(Copy, Clone, Debug)]
struct Params {
    borderless: bool,
    minimalist: bool,
    delta: Duration,
}

#[derive(Debug)]
pub struct UI {
    pub utx: Sender<Message>,
    handles: Handles,
    _environment: Environment,
}

impl Drop for UI {
    fn drop(&mut self) {
        self.handles.input.abort();
        self.handles.render.abort();
    }
}

impl UI {
    pub async fn render(&mut self, data: Update) -> Result<()> {
        self.utx.send(Message::Render(data)).await?;

        Ok(())
    }

    async fn ui(mut rx: Receiver<Message>, mut state: State, params: Params) -> Result<()> {
        let mut interval = tokio::time::interval(params.delta);
        let mut window = Window::new(state.width, params.borderless);

        loop {
            interface::draw(&state, &mut window, params).await?;

            if let Ok(message) = rx.try_recv() {
                match message {
                    Message::Render(update) => state.update(update),
                    Message::Quit => break,
                    _ => continue,
                }
            };

            interval.tick().await;
        }

        // environment.cleanup()?;
        Ok(())
    }

    pub async fn init(
        tx: Sender<Message>,
        progress: Arc<AtomicU8>,
        sink: Arc<Sink>,
        args: &Args,
    ) -> Result<Self> {
        let environment = Environment::ready(args.alternate)?;

        let (utx, urx) = mpsc::channel(8);
        let delta = 1.0 / f32::from(args.fps);
        let delta = Duration::from_secs_f32(delta);
        let width = 21 + args.width.min(32) * 2;

        Ok(Self {
            utx,
            _environment: environment,
            handles: Handles {
                render: tokio::spawn(Self::ui(
                    urx,
                    State::initial(sink, width, progress),
                    Params {
                        delta,
                        minimalist: args.minimalist,
                        borderless: args.borderless,
                    },
                )),
                input: tokio::spawn(input::listen(tx)),
            },
        })
    }
}
