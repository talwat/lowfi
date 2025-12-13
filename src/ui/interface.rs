use crate::{
    ui::{self, components, window::Window},
    Args,
};
use std::{env, time::Duration};
use tokio::time::Instant;

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
        if self.0.elapsed().as_millis() >= 500 {
            window.display(Self::now(), 8);
            self.0 = Instant::now();
        }
    }

    /// Simply creates a new clock, and renders it's initial state to the window top.
    pub fn new(window: &mut Window) -> Self {
        window.display(Self::now(), 8);

        Self(Instant::now())
    }
}

/// UI-specific parameters and options.
#[derive(Copy, Clone, Debug, Default)]
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

    /// The total delta between frames, which takes into account
    /// the time it takes to actually render each frame.
    ///
    /// Derived from the FPS.
    pub delta: Duration,
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
            minimalist: args.minimalist,
            borderless: args.borderless,
        })
    }
}

/// Creates a full "menu" from the [`ui::State`], which can be
/// easily put into a window for display.
///
/// The menu really is just a [`Vec`] of the different components,
/// with padding already added.
pub(crate) fn menu(state: &mut ui::State, params: Params) -> Vec<String> {
    let action = components::action(state, state.width);

    let middle = match state.timer {
        Some(timer) => {
            let volume = state.sink.volume();
            let percentage = format!("{}%", (volume * 100.0).round().abs());
            if timer.elapsed() > Duration::from_secs(1) {
                state.timer = None;
            }

            components::audio_bar(state.width - 17, volume, &percentage)
        }
        None => components::progress_bar(state, state.width - 16),
    };

    let controls = components::controls(state.width);
    if params.minimalist {
        vec![action, middle]
    } else {
        vec![action, middle, controls]
    }
}

/// The code for the terminal interface itself.
///
/// * `minimalist` - All this does is hide the bottom control bar.
pub fn draw(state: &mut ui::State, window: &mut Window, params: Params) -> super::Result<()> {
    let menu = menu(state, params);
    window.draw(menu, false)?;
    Ok(())
}
