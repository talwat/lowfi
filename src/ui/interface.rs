use crate::{
    ui::{self, State},
    Args,
};
use std::{env, time::Duration};

pub mod clock;
pub mod components;
pub mod titlebar;
pub mod window;

pub use clock::Clock;
pub use titlebar::TitleBar;
pub use window::Window;

/// UI-specific parameters and options.
#[derive(Copy, Clone, Debug)]
pub struct Params {
    /// Whether to include borders.
    pub borderless: bool,

    /// Whether to include the bottom control bar.
    pub minimalist: bool,

    /// Whether the visual part of the UI should be enabled.
    /// This only applies if the MPRIS feature is enabled.
    pub enabled: bool,

    /// Whether to include the clock on the top bar.
    pub clock: bool,

    /// The full inner width of the terminal window.
    pub(crate) width: usize,

    /// The total delta between frames, which takes into account
    /// the time it takes to actually render each frame.
    ///
    /// Derived from the FPS.
    pub delta: Duration,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            borderless: false,
            minimalist: false,
            enabled: true,
            clock: false,
            width: 27,
            delta: Duration::from_secs_f32(1.0 / 12.0),
        }
    }
}

impl TryFrom<&Args> for Params {
    type Error = ui::Error;

    fn try_from(args: &Args) -> ui::Result<Self> {
        let delta = 1.0 / f32::from(args.fps);
        let delta = Duration::from_secs_f32(delta);

        let disabled = env::var("LOWFI_DISABLE_UI").is_ok_and(|x| x == "1");
        if disabled && !cfg!(feature = "mpris") {
            return Err(ui::Error::RejectedDisable);
        }

        Ok(Self {
            delta,
            enabled: !disabled,
            clock: args.clock,
            width: 21 + args.width.min(32) * 2,
            minimalist: args.minimalist,
            borderless: args.borderless,
        })
    }
}

/// All of the state related to the interface itself,
/// which is displayed each frame to the standard output.
pub struct Interface {
    /// The [`Window`] to render to.
    pub(crate) window: Window,

    /// The interval to wait between frames.
    interval: tokio::time::Interval,

    /// The visual clock, which is [`None`] if it has
    /// been disabled by the [`Params`].
    clock: Option<Clock>,

    /// The interface parameters that control smaller
    /// aesthetic features and options.
    params: Params,
}

impl Default for Interface {
    #[inline]
    fn default() -> Self {
        Self::new(Params::default())
    }
}

impl Interface {
    /// Creates a new interface.
    pub fn new(params: Params) -> Self {
        let mut window = Window::new(params.width, params.borderless);

        Self {
            clock: params.clock.then(|| Clock::new(&mut window)),
            interval: tokio::time::interval(params.delta),
            window,
            params,
        }
    }

    /// Creates a full "menu" from the [`ui::State`], which can be
    /// easily put into a window for display.
    ///
    /// The menu really is just a [`Vec`] of the different components,
    /// with padding already added.
    pub(crate) fn menu(&self, state: &mut State) -> Vec<String> {
        let action = components::action(state, self.params.width);

        let middle = match state.volume_timer {
            Some(timer) => {
                let volume = state.sink.volume();
                let percentage = format!("{}%", (volume * 100.0).round().abs());
                if timer.elapsed() > Duration::from_secs(1) {
                    state.volume_timer = None;
                }

                components::audio_bar(self.params.width - 17, volume, &percentage)
            }
            None => components::progress_bar(state, self.params.width - 16),
        };

        let controls = components::controls(self.params.width);
        if self.params.minimalist {
            vec![action, middle]
        } else {
            vec![action, middle, controls]
        }
    }

    /// Draws the terminal. This will also wait for the specified
    /// delta to pass before completing.
    pub async fn draw(&mut self, state: &mut State) -> super::Result<()> {
        self.clock.as_mut().map(|x| x.update(&mut self.window));
        self.window.draw(self.menu(state), false)?;
        self.interval.tick().await;

        Ok(())
    }
}
