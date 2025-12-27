use futures_util::{future::select_all, FutureExt, TryFutureExt};
use std::{future::Future, pin::Pin};
use tokio::sync::mpsc;

/// Wrapper around a [`Vec`] of JoinHandles to provide better error reporting & shutdown.
pub struct Tasks {
    tasks: Vec<Pin<Box<dyn Future<Output = crate::Result<()>> + Send>>>,
    tx: mpsc::Sender<crate::Message>,
}

impl Tasks {
    pub fn new(tx: mpsc::Sender<crate::Message>) -> Self {
        Self {
            tx,
            tasks: Vec::new(),
        }
    }

    pub fn spawn<E: Into<crate::Error> + Send + Sync>(
        &mut self,
        future: impl Future<Output = Result<(), E>> + Send + 'static,
    ) {
        self.tasks.push(future.map_err(|x| x.into()).boxed());
    }

    pub fn tx(&self) -> mpsc::Sender<crate::Message> {
        self.tx.clone()
    }

    pub async fn select(
        self,
        runner: impl Future<Output = Result<(), crate::Error>> + std::marker::Send,
    ) -> crate::Result<()> {
        let futures = self.tasks.into_iter().chain([runner.boxed()]);

        select_all(futures).await.0
    }
}
