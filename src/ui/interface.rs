use std::time::Duration;

use crate::{
    ui::{self, components, window::Window},
    Args,
};

#[derive(Copy, Clone, Debug)]
pub struct Params {
    pub borderless: bool,
    pub minimalist: bool,
    pub delta: Duration,
}

impl From<&Args> for Params {
    fn from(args: &Args) -> Self {
        let delta = 1.0 / f32::from(args.fps);
        let delta = Duration::from_secs_f32(delta);

        Self {
            delta,
            minimalist: args.minimalist,
            borderless: args.borderless,
        }
    }
}

/// The code for the terminal interface itself.
///
/// * `minimalist` - All this does is hide the bottom control bar.
pub async fn draw(state: &mut ui::State, window: &mut Window, params: Params) -> super::Result<()> {
    let action = components::action(&state, state.width);

    let middle = match state.timer {
        Some(timer) => {
            let volume = state.sink.volume();
            let percentage = format!("{}%", (volume * 100.0).round().abs());
            if timer.elapsed() > Duration::from_secs(1) {
                state.timer = None;
            };

            components::audio_bar(state.width - 17, volume, &percentage)
        }
        None => components::progress_bar(&state, state.width - 16),
    };

    let controls = components::controls(state.width);
    let menu = match (params.minimalist, &state.current) {
        (true, _) => vec![action, middle],
        // (false, Some(x)) => vec![x.path.clone(), action, middle, controls],
        _ => vec![action, middle, controls],
    };

    window.draw(menu, false)?;
    Ok(())
}
