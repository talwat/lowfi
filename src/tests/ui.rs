/* The lowfi UI:
┌─────────────────────────────┐
│ loading                     │
│  [           ] 00:00/00:00  │
│ [s]kip    [p]ause    [q]uit │
└─────────────────────────────┘
*/

#[cfg(test)]
mod components {
    use crate::ui::interface;

    use std::time::Duration;

    #[test]
    fn format_duration_works() {
        let d = Duration::from_secs(62);
        assert_eq!(interface::components::format_duration(&d), "01:02");
    }

    #[test]
    fn format_duration_zero() {
        let d = Duration::from_secs(0);
        assert_eq!(interface::components::format_duration(&d), "00:00");
    }

    #[test]
    fn format_duration_hours_wrap() {
        let d = Duration::from_secs(3661); // 1:01:01
        assert_eq!(interface::components::format_duration(&d), "61:01");
    }

    #[test]
    fn audio_bar_contains_percentage() {
        let s = interface::components::audio_bar(10, 0.5, "50%");
        assert!(s.contains("50%"));
        assert!(s.starts_with(" volume:"));
    }

    #[test]
    fn audio_bar_muted_volume() {
        let s = interface::components::audio_bar(8, 0.0, "0%");
        assert!(s.contains("0%"));
    }

    #[test]
    fn audio_bar_full_volume() {
        let s = interface::components::audio_bar(10, 1.0, "100%");
        assert!(s.contains("100%"));
    }

    #[test]
    fn controls_has_items() {
        let s = interface::components::controls(30);
        assert!(s.contains("[s]"));
        assert!(s.contains("[p]"));
        assert!(s.contains("[q]"));
    }
}

#[cfg(test)]
mod window {
    use crate::ui::interface::Window;

    #[test]
    fn new_border_strings() {
        let w = Window::new(10, false);
        assert!(w.borders[0].starts_with('┌'));
        assert!(w.borders[1].starts_with('└'));

        let w2 = Window::new(5, true);
        assert!(w2.borders[0].is_empty());
        assert!(w2.borders[1].is_empty());
    }

    fn sided(text: &str) -> String {
        return format!("│ {text} │");
    }

    #[test]
    fn simple() {
        let w = Window::new(3, false);
        let (render, height) = w.render(vec![String::from("abc")], false, true).unwrap();

        const MIDDLE: &str = "─────";
        assert_eq!(format!("┌{MIDDLE}┐\n{}\n└{MIDDLE}┘", sided("abc")), render);
        assert_eq!(height, 3);
    }

    #[test]
    fn spaced() {
        let w = Window::new(3, false);
        let (render, height) = w
            .render(
                vec![String::from("abc"), String::from(" b"), String::from("c")],
                true,
                true,
            )
            .unwrap();

        const MIDDLE: &str = "─────";
        assert_eq!(
            format!(
                "┌{MIDDLE}┐\n{}\n{}\n{}\n└{MIDDLE}┘",
                sided("abc"),
                sided(" b "),
                sided("c  "),
            ),
            render
        );
        assert_eq!(height, 5);
    }

    #[test]
    fn zero_width_window() {
        let w = Window::new(0, false);
        assert!(!w.borders[0].is_empty());
    }
}

#[cfg(test)]
mod interface {
    use crossterm::style::Stylize;
    use std::{sync::Arc, time::Duration};
    use tokio::time::Instant;

    use crate::{
        download::PROGRESS,
        player::Current,
        tracks,
        ui::{
            interface::{self, Params},
            State,
        },
    };

    #[test]
    fn loading() {
        let sink = Arc::new(rodio::Sink::new().0);
        let mut state = State::initial(sink, 3, String::from("test"));
        let menu = interface::menu(&mut state, Params::default());

        assert_eq!(menu[0], "loading                    ");
        assert_eq!(menu[1], " [           ] 00:00/00:00 ");
        assert_eq!(
            menu[2],
            format!(
                "{}kip    {}ause    {}uit",
                "[s]".bold(),
                "[p]".bold(),
                "[q]".bold()
            )
        );
    }

    #[test]
    fn volume() {
        let sink = Arc::new(rodio::Sink::new().0);
        sink.set_volume(0.5);
        let mut state = State::initial(sink, 3, String::from("test"));
        state.timer = Some(Instant::now());

        let menu = interface::menu(&mut state, Params::default());

        assert_eq!(menu[0], "loading                    ");
        assert_eq!(menu[1], " volume: [/////     ]  50% ");
        assert_eq!(
            menu[2],
            format!(
                "{}kip    {}ause    {}uit",
                "[s]".bold(),
                "[p]".bold(),
                "[q]".bold()
            )
        );
    }

    #[test]
    fn progress() {
        let sink = Arc::new(rodio::Sink::new().0);
        PROGRESS.store(50, std::sync::atomic::Ordering::Relaxed);
        let mut state = State::initial(sink, 3, String::from("test"));
        state.current = Current::Loading(Some(&PROGRESS));

        let menu = interface::menu(&mut state, Params::default());

        assert_eq!(menu[0], format!("loading {}                ", "50%".bold()));
        assert_eq!(menu[1], " [           ] 00:00/00:00 ");
        assert_eq!(
            menu[2],
            format!(
                "{}kip    {}ause    {}uit",
                "[s]".bold(),
                "[p]".bold(),
                "[q]".bold()
            )
        );
    }

    #[test]
    fn track() {
        let sink = Arc::new(rodio::Sink::new().0);
        let track = tracks::Info {
            path: "/path".to_owned(),
            display: "Test Track".to_owned(),
            width: 4 + 1 + 5,
            duration: Some(Duration::from_secs(8)),
        };

        let mut state = State::initial(sink, 3, String::from("test"));
        state.current = Current::Track(track.clone());
        let menu = interface::menu(&mut state, Params::default());

        assert_eq!(
            menu[0],
            format!("playing {}         ", track.display.bold())
        );
        assert_eq!(menu[1], " [           ] 00:00/00:08 ");
        assert_eq!(
            menu[2],
            format!(
                "{}kip    {}ause    {}uit",
                "[s]".bold(),
                "[p]".bold(),
                "[q]".bold()
            )
        );
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
