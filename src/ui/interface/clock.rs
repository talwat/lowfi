use tokio::time::Instant;

use super::window::Window;

/// An extremely simple clock to be used alongside the [`Window`].
pub struct Clock(Instant);

impl Clock {
    /// Small shorthand for getting the local time now, and formatting it.
    #[inline]
    fn now() -> chrono::format::DelayedFormat<chrono::format::StrftimeItems<'static>> {
        chrono::Local::now().format("%H:%M:%S")
    }

    /// Checks if the last update was long enough ago, and if so,
    /// updates the displayed clock.
    ///
    /// This is to avoid constant calls to [`chrono::Local::now`], which
    /// is somewhat expensive because of timezones.
    pub fn update(&mut self, window: &mut Window) {
        if self.0.elapsed().as_millis() >= 200 {
            window.display(Self::now(), 8);
            self.0 = Instant::now();
        }
    }

    /// Simply creates a new clock, and renders it's initial state to the top of the window.
    pub fn new(window: &mut Window) -> Self {
        window.display(Self::now(), 8);

        Self(Instant::now())
    }
}
