use tokio::sync::mpsc::{self, Receiver, Sender};
use crate::Message;
mod input;

#[derive(Debug, Clone, PartialEq)]
pub struct Render {
    pub track: String,
}

#[derive(Debug)]
struct Handles {
    render: crate::Handle,
    input: crate::Handle,
}

#[derive(Debug)]
pub struct UI {
    pub utx: Sender<Message>,
    handles: Handles
}

impl Drop for UI {
    fn drop(&mut self) {
        self.handles.input.abort();
        self.handles.render.abort();
    }
}

impl UI {
    pub async fn render(&mut self, data: Render) -> crate::Result<()> {
        self.utx.send(Message::Render(data)).await?;

        Ok(())
    }

    async fn ui(mut rx: Receiver<Message>) -> crate::Result<()> {
        while let Some(message) = rx.recv().await {
            let Message::Render(data) = message else {
                continue;
            };

            eprintln!("data: {data:?}");
        }
        Ok(())
    }

    pub async fn init(tx: Sender<Message>) -> Self {
        let (utx, urx) = mpsc::channel(8);

        Self {
            utx,
            handles: Handles {
                render: tokio::spawn(Self::ui(urx)),
                input: tokio::spawn(input::listen(tx)),
            }
        }
    }
}