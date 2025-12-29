//! Contains the code for initializing the UI and creating a [`ui::Handle`].

use crate::ui::{self, input, interface};
use tokio::sync::broadcast;

impl crate::Tasks {
    /// Initializes the UI itself, along with all of the tasks that are related to it.
    #[allow(clippy::unused_async)]
    pub async fn ui(&mut self, state: ui::State, args: &crate::Args) -> crate::Result<ui::Handle> {
        let (utx, urx) = broadcast::channel(8);

        #[cfg(feature = "mpris")]
        let mpris = ui::mpris::Server::new(state.clone(), self.tx(), urx.resubscribe()).await?;

        let params = interface::Params::try_from(args)?;
        if params.enabled {
            self.spawn(ui::run(urx, state, params));
            self.spawn(input::listen(self.tx()));
        }

        Ok(ui::Handle {
            updater: utx,
            #[cfg(feature = "mpris")]
            mpris,
        })
    }
}
