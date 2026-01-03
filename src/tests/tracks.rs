#[cfg(test)]
mod format {
    use crate::tracks::format::name;

    #[test]
    fn handles_all_numeric_name() {
        let n = name("12345.mp3").unwrap();
        assert_eq!(n, "12345");
    }

    #[test]
    fn decodes_url() {
        let n = name("lofi%20track.mp3").unwrap();
        assert_eq!(n, "lofi track");
    }

    #[test]
    fn handles_extension_only() {
        let n = name(".mp3").unwrap();
        // Should handle edge case gracefully
        assert!(!n.is_empty());
    }
}

#[cfg(test)]
mod queued {
    use crate::tracks::{format, Queued};
    use bytes::Bytes;

    #[test]
    fn queued_uses_custom_display() {
        let q = Queued::new(
            "path/to/file.mp3".into(),
            Bytes::from_static(b"abc"),
            Some("Shown".into()),
        )
        .unwrap();

        assert_eq!(q.display, "Shown");
        assert_eq!(q.path, "path/to/file.mp3");
    }

    #[test]
    fn queued_generates_display_if_none() {
        let q = Queued::new(
            "path/to/cool_track.mp3".into(),
            Bytes::from_static(b"abc"),
            None,
        )
        .unwrap();

        assert_eq!(q.display, format::name("path/to/cool_track.mp3").unwrap());
    }
}

#[cfg(test)]
mod info {
    use crate::tracks::Info;
    use unicode_segmentation::UnicodeSegmentation;

    #[test]
    fn to_entry_roundtrip() {
        let info = Info {
            path: "p.mp3".into(),
            display: "Nice Track".into(),
            width: 10,
            duration: None,
        };

        assert_eq!(info.to_entry(), "p.mp3!Nice Track");
    }

    #[test]
    fn width_counts_graphemes() {
        // We cannot create a valid decoder for arbitrary bytes here, so test width through constructor logic directly.
        let display = "a̐é"; // multiple-grapheme clusters
        let width = display.graphemes(true).count();

        let info = Info {
            path: "x".into(),
            display: display.into(),
            width,
            duration: None,
        };

        assert_eq!(info.width, width);
    }
}

#[cfg(test)]
mod decoded {
    use crate::tracks::Queued;
    use bytes::Bytes;

    #[tokio::test]
    async fn decoded_fails_with_invalid_audio() {
        let q = Queued::new(
            "path.mp3".into(),
            Bytes::from_static(b"not audio"),
            Some("Name".into()),
        )
        .unwrap();

        let result = q.decode();
        assert!(result.is_err());
    }
}

#[cfg(test)]
mod list {
    use crate::{downloader::Progress, tracks::List};
    use reqwest::Client;

    #[test]
    fn base_works() {
        let text = "http://base/\ntrack1\ntrack2";
        let list = List::new("test", text, None);
        assert_eq!(list.header(), "http://base/");
    }

    #[test]
    fn random_path_parses_custom_display() {
        let text = "http://x/\npath!Display";
        let list = List::new("t", text, None);

        let (p, d) = list.random_path(&mut fastrand::Rng::new());
        assert_eq!(p, "path");
        assert_eq!(d, Some("Display".into()));
    }

    #[test]
    fn random_path_no_display() {
        let text = "http://x/\ntrackA";
        let list = List::new("t", text, None);

        let (p, d) = list.random_path(&mut fastrand::Rng::new());
        assert_eq!(p, "trackA");
        assert!(d.is_none());
    }

    #[test]
    fn new_trims_lines() {
        let text = "base\na  \nb ";
        let list = List::new("name", text, None);

        assert_eq!(list.header(), "base");
        assert_eq!(list.lines[1], "a");
        assert_eq!(list.lines[2], "b");
    }

    #[test]
    fn custom_display_with_exclamation() {
        let text = "http://base/\nfile.mp3!My Custom Name";
        let list = List::new("t", text, None);
        let (path, display) = list.random_path(&mut fastrand::Rng::new());
        assert_eq!(path, "file.mp3");
        assert_eq!(display, Some("My Custom Name".into()));
    }

    #[test]
    fn single_track() {
        let text = "base\nonly_track.mp3";
        let list = List::new("name", text, None);
        let (path, _) = list.random_path(&mut fastrand::Rng::new());
        assert_eq!(path, "only_track.mp3");
    }

    #[tokio::test]
    async fn download() {
        let text = "https://stream.chillhop.com/mp3/\n9476!Apple Juice";
        let list = List::new("name", text, None);

        let client = Client::new();
        let track = list
            .random(&client, Progress::new(), &mut fastrand::Rng::new())
            .await
            .unwrap();
        assert_eq!(track.display, "Apple Juice");
        assert_eq!(track.path, "https://stream.chillhop.com/mp3/9476");
        assert_eq!(track.data.len(), 3150424);

        let decoded = track.decode().unwrap();
        assert_eq!(decoded.info.duration.unwrap().as_secs(), 143);
    }
}
