//! Task management.
//!
//! This file aims to abstract a lot of confusing and annoying Rust async logic.
//! For those who are not intimately familiar with async rust, this will be very confusing.
//!
//! Basically, this offers a way to abstract whether lowfi is using tokio's task system,
//! or whether it just keeps a [`Vec`] of futures and then polls them with select. Or any other
//! possible solution that could be dreamt up.

use futures_util::{future::select_all, FutureExt, TryFutureExt};
use std::future::Future;
use tokio::{sync::mpsc, task::JoinHandle};

// TODO: Consider having a, possibly simpler, single task monolithic approach.
// type Task = std::pin::Pin<Box<dyn Future<Output = crate::Result<()>> + Send>>;
type Task = JoinHandle<crate::Result<()>>;

/// Await a [`JoinHandle`], and map the error.
async fn mapped(handle: Task) -> crate::Result<()> {
    match handle.await {
        Ok(res) => res,
        Err(e) if !e.is_cancelled() => Err(crate::Error::JoinError(e)),
        Err(_) => Ok(()),
    }
}

/// Handles all of the processes within lowfi.
/// This entails initializing/closing tasks, and handling any potential errors that arise.
///
/// It should be noted that "tasks" do not actually have to be [`tokio::task`]s.
/// "Task" here just means something that is running. Indeed, it could just be
/// a future which is polled later.
pub struct Tasks {
    /// The actual tasks.
    tasks: Vec<Task>,

    /// A sender, which is kept for convenience to be used when
    /// initializing various other tasks.
    tx: mpsc::Sender<crate::Message>,
}

impl Tasks {
    /// Creates a new task manager.
    pub fn new(tx: mpsc::Sender<crate::Message>) -> Self {
        Self {
            tx,
            tasks: Vec::new(),
        }
    }

    /// Processes a task, and adds it to the internal buffer.
    pub fn spawn<E: Into<crate::Error> + Send + Sync>(
        &mut self,
        future: impl Future<Output = Result<(), E>> + Send + 'static,
    ) {
        self.tasks.push(tokio::spawn(future.map_err(|x| x.into())));
    }

    /// Gets a copy of the internal [`mpsc::Sender`].
    pub fn tx(&self) -> mpsc::Sender<crate::Message> {
        self.tx.clone()
    }

    /// Uses [`select_all`] on the tasks, actively polling them.
    ///
    /// An additional `runner` is for the main player future, which
    /// can't be added as a "task" because it shares data with the
    /// main thread.
    pub async fn select(
        self,
        runner: impl Future<Output = Result<(), crate::Error>> + std::marker::Send,
    ) -> crate::Result<()> {
        let futures = self
            .tasks
            .into_iter()
            .map(|handle| mapped(handle).boxed())
            .chain([runner.boxed()]);

        select_all(futures).await.0
    }
}
