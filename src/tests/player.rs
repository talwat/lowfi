#[cfg(test)]
mod current {
    use crate::player::Current;
    use std::time::Duration;

    fn test_info(path: &str, display: &str) -> crate::tracks::Info {
        crate::tracks::Info {
            path: path.into(),
            display: display.into(),
            width: display.len(),
            duration: Some(Duration::from_secs(180)),
        }
    }

    #[test]
    fn default_is_loading() {
        let c = Current::default();
        assert!(c.loading());
    }

    #[test]
    fn track_is_not_loading() {
        let info = test_info("x.mp3", "Track X");
        let c = Current::Track(info);
        assert!(!c.loading());
    }

    #[test]
    fn loading_without_progress() {
        let c = Current::Loading(None);
        assert!(c.loading());
    }

    #[test]
    fn current_clone_works() {
        let info = test_info("p.mp3", "P");
        let c1 = Current::Track(info);
        let c2 = c1.clone();
        assert!(!c2.loading());
    }
}
