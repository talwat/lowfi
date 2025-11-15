use crate::ui::{self, components, window::Window};

/// The code for the terminal interface itself.
///
/// * `minimalist` - All this does is hide the bottom control bar.
pub async fn draw(state: &ui::State, window: &mut Window, params: ui::Params) -> super::Result<()> {
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

    let menu = match (params.minimalist, &state.track) {
        (true, _) => vec![action, middle],
        // (false, Some(x)) => vec![x.path.clone(), action, middle, controls],
        _ => vec![action, middle, controls],
    };

    window.draw(menu, false)?;
    Ok(())
}
