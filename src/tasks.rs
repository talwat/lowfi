//! Task management.
//!
//! This file aims to abstract a lot of potentially annoying Rust async logic, which may be
//! subject to change.

use futures_util::TryFutureExt;
use std::future::Future;
use tokio::{select, sync::mpsc, task::JoinSet};

/// Handles all of the processes within lowfi.
/// This entails initializing/closing tasks, and handling any potential errors that arise.
pub struct Tasks {
    /// The [`JoinSet`], which contains all of the task handles.
    pub set: JoinSet<crate::Result<()>>,

    /// A sender, which is kept for convenience to be used when
    /// initializing various other tasks.
    tx: mpsc::Sender<crate::Message>,
}

impl Tasks {
    /// Creates a new task manager.
    pub fn new(tx: mpsc::Sender<crate::Message>) -> Self {
        Self {
            tx,
            set: JoinSet::new(),
        }
    }

    /// Processes a task, and adds it to the internal [`JoinSet`].
    pub fn spawn<E: Into<crate::Error> + Send + Sync + 'static>(
        &mut self,
        future: impl Future<Output = Result<(), E>> + Send + 'static,
    ) {
        self.set.spawn(future.map_err(Into::into));
    }

    /// Gets a copy of the internal [`mpsc::Sender`].
    pub fn tx(&self) -> mpsc::Sender<crate::Message> {
        self.tx.clone()
    }

    /// Actively polls all of the handles previously added.
    ///
    /// An additional `runner` is for the main player future, which
    /// can't be added as a "task" because it shares data with the
    /// main thread.
    ///
    /// This either returns when the runner completes, or if an error occurs
    /// in any of the internally held tasks.
    pub async fn wait(
        &mut self,
        runner: impl Future<Output = Result<(), crate::Error>> + std::marker::Send,
    ) -> crate::Result<()> {
        select! {
            result = runner => result,
            Some(result) = self.set.join_next() => match result {
                Ok(res) => res,
                Err(e) if !e.is_cancelled() => Err(crate::Error::JoinError(e)),
                Err(_) => Ok(()),
            }
        }
    }
}
