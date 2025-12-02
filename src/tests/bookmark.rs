#[cfg(test)]
mod bookmark {
    use crate::{bookmark::Bookmarks, tracks::Info};

    fn test_info(path: &str, display: &str) -> Info {
        Info {
            path: path.into(),
            display: display.into(),
            width: display.len(),
            duration: None,
        }
    }

    #[tokio::test]
    async fn toggle_and_check() {
        let mut bm = Bookmarks { entries: vec![] };
        let info = test_info("p.mp3", "Nice Track");

        // initially not bookmarked
        assert!(!bm.bookmarked(&info));

        // bookmark it
        let added = bm.bookmark(&info).await.unwrap();
        assert!(added);
        assert!(bm.bookmarked(&info));

        // un-bookmark it
        let removed = bm.bookmark(&info).await.unwrap();
        assert!(!removed);
        assert!(!bm.bookmarked(&info));
    }

    #[tokio::test]
    async fn multiple_bookmarks() {
        let mut bm = Bookmarks { entries: vec![] };
        let info1 = test_info("track1.mp3", "Track One");
        let info2 = test_info("track2.mp3", "Track Two");

        bm.bookmark(&info1).await.unwrap();
        bm.bookmark(&info2).await.unwrap();

        assert!(bm.bookmarked(&info1));
        assert!(bm.bookmarked(&info2));
        assert_eq!(bm.entries.len(), 2);
    }

    #[tokio::test]
    async fn duplicate_bookmark_removes() {
        let mut bm = Bookmarks { entries: vec![] };
        let info = test_info("x.mp3", "X");

        bm.bookmark(&info).await.unwrap();
        let is_added = bm.bookmark(&info).await.unwrap();

        assert!(!is_added);
        assert!(bm.entries.is_empty());
    }
}
