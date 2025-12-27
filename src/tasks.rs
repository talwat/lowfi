use std::{future::Future, mem::MaybeUninit};

trait AsyncArrayMap<T, const N: usize> {
    async fn async_map<U, F, Fut>(self, f: F) -> [U; N]
    where
        F: FnMut(T) -> Fut,
        Fut: Future<Output = U>;
}

impl<T, const N: usize> AsyncArrayMap<T, N> for [T; N] {
    async fn async_map<U, F, Fut>(self, mut f: F) -> [U; N]
    where
        F: FnMut(T) -> Fut,
        Fut: Future<Output = U>,
    {
        let mut out: [MaybeUninit<U>; N] = unsafe { MaybeUninit::uninit().assume_init() };

        for (i, v) in self.into_iter().enumerate() {
            out[i].write(f(v).await);
        }

        unsafe { std::mem::transmute_copy(&out) }
    }
}

/// Wrapper around an array of JoinHandles to provide better error reporting & shutdown.
pub struct Tasks<E, const S: usize>(pub [tokio::task::JoinHandle<Result<(), E>>; S]);

impl<T: Send + 'static + Into<crate::Error>, const S: usize> Tasks<T, S> {
    /// Abort tasks, and report either errors thrown from within each task
    /// or from tokio about joining the task.
    pub async fn shutdown(self) -> [crate::Result<()>; S] {
        self.0
            .async_map(async |handle| {
                if !handle.is_finished() {
                    handle.abort();
                }

                match handle.await {
                    Ok(Err(error)) => Err(error.into()),
                    Err(error) if !error.is_cancelled() => Err(crate::Error::JoinError(error)),
                    Ok(Ok(())) | Err(_) => Ok(()),
                }
            })
            .await
    }
}
