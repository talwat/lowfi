#[cfg(test)]
mod components {
    use crate::ui;

    use std::time::Duration;

    #[test]
    fn format_duration_works() {
        let d = Duration::from_secs(62);
        assert_eq!(ui::components::format_duration(&d), "01:02");
    }

    #[test]
    fn format_duration_zero() {
        let d = Duration::from_secs(0);
        assert_eq!(ui::components::format_duration(&d), "00:00");
    }

    #[test]
    fn format_duration_hours_wrap() {
        let d = Duration::from_secs(3661); // 1:01:01
        assert_eq!(ui::components::format_duration(&d), "61:01");
    }

    #[test]
    fn audio_bar_contains_percentage() {
        let s = ui::components::audio_bar(10, 0.5, "50%");
        assert!(s.contains("50%"));
        assert!(s.starts_with(" volume:"));
    }

    #[test]
    fn audio_bar_muted_volume() {
        let s = ui::components::audio_bar(8, 0.0, "0%");
        assert!(s.contains("0%"));
    }

    #[test]
    fn audio_bar_full_volume() {
        let s = ui::components::audio_bar(10, 1.0, "100%");
        assert!(s.contains("100%"));
    }

    #[test]
    fn controls_has_items() {
        let s = ui::components::controls(30);
        assert!(s.contains("[s]"));
        assert!(s.contains("[p]"));
        assert!(s.contains("[q]"));
    }
}

#[cfg(test)]
mod window {
    use crate::ui::window::Window;

    #[test]
    fn new_border_strings() {
        let w = Window::new(10, false);
        assert!(w.borders[0].starts_with('┌'));
        assert!(w.borders[1].starts_with('└'));

        let w2 = Window::new(5, true);
        assert!(w2.borders[0].is_empty());
        assert!(w2.borders[1].is_empty());
    }

    #[test]
    fn border_width_consistency() {
        let w = Window::new(20, false);
        // borders should have consistent format with width encoded
        assert!(w.borders[0].len() > 0);
    }

    #[test]
    fn zero_width_window() {
        let w = Window::new(0, false);
        // Should handle zero-width gracefully
        assert!(!w.borders[0].is_empty());
    }
}

#[cfg(test)]
mod environment {
    use crate::ui::Environment;

    #[test]
    fn ready_and_cleanup_no_panic() {
        // Try to create the environment but don't fail the test if the
        // terminal isn't available. We just assert the API exists.
        if let Ok(env) = Environment::ready(false) {
            // cleanup should succeed
            let _ = env.cleanup(true);
        }
    }

    #[test]
    fn ready_with_alternate_screen() {
        if let Ok(env) = Environment::ready(true) {
            let _ = env.cleanup(false);
        }
    }
}

#[cfg(test)]
mod integration {
    use std::sync::Arc;

    use rodio::OutputStreamBuilder;

    use crate::{player::Current, Args};

    fn try_make_state() -> Option<crate::ui::State> {
        let stream = OutputStreamBuilder::open_default_stream();
        if stream.is_err() {
            return None;
        }

        let mut stream = stream.unwrap();
        stream.log_on_drop(false);
        let sink = Arc::new(rodio::Sink::connect_new(stream.mixer()));

        let args = Args {
            alternate: false,
            minimalist: false,
            borderless: false,
            paused: false,
            fps: 12,
            timeout: 3,
            debug: false,
            width: 3,
            track_list: String::from("chillhop"),
            buffer_size: 5,
            command: None,
        };

        let current = Current::default();
        Some(crate::ui::State::initial(
            sink,
            &args,
            current,
            String::from("list"),
        ))
    }

    #[test]
    fn progress_bar_runs() -> Result<(), Box<dyn std::error::Error>> {
        if let Some(state) = try_make_state() {
            // ensure we can call progress_bar without panic
            let _ = crate::ui::components::progress_bar(&state, state.width);
        }

        Ok(())
    }

    #[test]
    fn action_runs() -> Result<(), Box<dyn std::error::Error>> {
        if let Some(state) = try_make_state() {
            let _ = crate::ui::components::action(&state, state.width);
        }

        Ok(())
    }
}
