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
pub async fn draw(state: &ui::State, window: &mut Window, params: Params) -> super::Result<()> {
    let action = components::action(&state, state.width);

    let volume = state.sink.volume();
    let percentage = format!("{}%", (volume * 100.0).round().abs());

    // let timer = VOLUME_TIMER.load(Ordering::Relaxed);
    // let middle = match timer {
    let middle = components::progress_bar(&state, state.width - 16);
    // _ => components::audio_bar(volume, &percentage, width - 17),
    // };

    // if timer > 0 && timer <= AUDIO_BAR_DURATION {
    //     // We'll keep increasing the timer until it eventually hits `AUDIO_BAR_DURATION`.
    //     VOLUME_TIMER.fetch_add(1, Ordering::Relaxed);
    // } else {
    //     // If enough time has passed, we'll reset it back to 0.
    //     VOLUME_TIMER.store(0, Ordering::Relaxed);
    // }

    let controls = components::controls(state.width);

    let menu = match (params.minimalist, &state.current) {
        (true, _) => vec![action, middle],
        // (false, Some(x)) => vec![x.path.clone(), action, middle, controls],
        _ => vec![action, middle, controls],
    };

    window.draw(menu, false)?;
    Ok(())
}
