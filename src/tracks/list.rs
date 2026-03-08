//! The module containing all of the logic behind track lists,
//! as well as obtaining track names & downloading the raw audio data

use std::cmp::min;

use bytes::{BufMut as _, Bytes, BytesMut};
use futures_util::StreamExt as _;
use reqwest::Client;
use tokio::fs;

use crate::{
    data_dir,
    downloader::Progress,
    tracks::{
        self,
        error::{self, WithTrackContext as _},
    },
};

use super::Queued;

/// Represents a list of tracks that can be played.
///
/// See the [README](https://github.com/talwat/lowfi?tab=readme-ov-file#the-format) for more details about the format.
#[derive(Clone)]
pub struct List {
    /// The "name" of the list, usually derived from a filename.
    #[allow(dead_code)]
    pub name: String,

    /// Just the raw file, but seperated by `/n` (newlines).
    /// `lines[0]` is the base/heaeder, with the rest being tracks.
    pub lines: Vec<String>,

    /// The file path which the list was read from.
    #[allow(dead_code)]
    pub path: Option<String>,
}

impl List {
    /// Gets the base URL of the [List].
    pub fn header(&self) -> &str {
        self.lines[0].trim()
    }

    /// Gets the path of a random track.
    ///
    /// The second value in the tuple specifies whether the
    /// track has a custom display name.
    pub fn random_path(&self, rng: &mut fastrand::Rng) -> (String, Option<String>) {
        // We're getting from 1 here, since the base is at `self.lines[0]`.
        //
        // We're also not pre-trimming `self.lines` into `base` & `tracks` due to
        // how rust vectors work, since it is slower to drain only a single element from
        // the start, so it's faster to just keep it in & work around it.
        let random = rng.usize(1..self.lines.len());
        let line = self.lines[random].clone();

        if let Some((first, second)) = line.split_once('!') {
            (first.to_owned(), Some(second.to_owned()))
        } else {
            (line, None)
        }
    }

    /// Downloads a raw track, but doesn't decode it.
    pub(crate) async fn download(
        &self,
        track: &str,
        client: &Client,
        progress: Option<Progress>,
    ) -> tracks::Result<(Bytes, String)> {
        // If the track has a protocol, then we should ignore the base for it.
        let path = if track.contains("://") {
            track.to_owned()
        } else {
            format!("{}{}", self.header(), track)
        };

        let data: Bytes = if let Some(x) = path.strip_prefix("file://") {
            let path = if x.starts_with('~') {
                let home_path = dirs::home_dir()
                    .ok_or(error::Kind::InvalidPath)
                    .track(track)?;
                let home = home_path
                    .to_str()
                    .ok_or(error::Kind::InvalidPath)
                    .track(track)?;

                x.replace('~', home)
            } else {
                x.to_owned()
            };

            let result = tokio::fs::read(path.clone()).await.track(x)?;
            result.into()
        } else {
            let response = client.get(path.clone()).send().await.track(track)?;
            let Some(progress) = progress else {
                let bytes = response.bytes().await.track(track)?;
                return Ok((bytes, path));
            };

            let total = response
                .content_length()
                .ok_or(error::Kind::UnknownLength)
                .track(track)?;
            let mut stream = response.bytes_stream();
            let mut bytes = BytesMut::new();
            let mut downloaded: u64 = 0;

            while let Some(item) = stream.next().await {
                let chunk = item.track(track)?;
                downloaded = min(downloaded + (chunk.len() as u64), total);
                progress.set(downloaded as f32 / total as f32);

                bytes.put(chunk);
            }

            bytes.into()
        };

        Ok((data, path))
    }

    /// Fetches and downloads a random track from the [List].
    ///
    /// The Result's error is a bool, which is true if a timeout error occurred,
    /// and false otherwise. This tells lowfi if it shouldn't wait to try again.
    pub async fn random(
        &self,
        client: &Client,
        progress: Progress,
        rng: &mut fastrand::Rng,
    ) -> tracks::Result<Queued> {
        let (path, display) = self.random_path(rng);
        let (data, path) = self.download(&path, client, Some(progress)).await?;

        Queued::new(path, data, display)
    }

    /// Parses text into a [List].
    pub fn new(name: &str, text: &str, path: Option<&str>) -> Self {
        let lines: Vec<String> = text
            .trim_end()
            .lines()
            .map(|x| x.trim_end().to_owned())
            .collect();

        Self {
            lines,
            path: path.map(ToOwned::to_owned),
            name: name.to_owned(),
        }
    }

    /// Reads a [List] from the filesystem using the CLI argument provided.
    pub async fn load(tracks: &str) -> tracks::Result<Self> {
        if tracks == "chillhop" {
            return Ok(Self::new(
                "chillhop",
                include_str!("../../data/chillhop.txt"),
                None,
            ));
        }

        // Check if the track is in ~/.local/share/lowfi, in which case we'll load that.
        let path = data_dir()
            .map_err(|_| error::Kind::InvalidPath)?
            .join(format!("{tracks}.txt"));
        let path = if path.exists() { path } else { tracks.into() };

        let raw = fs::read_to_string(path.clone()).await?;

        // Get rid of special noheader case for tracklists without a header.
        let raw = raw
            .strip_prefix("noheader")
            .map_or_else(|| raw.as_ref(), |stripped| stripped);

        let name = path
            .file_stem()
            .and_then(|x| x.to_str())
            .ok_or(tracks::error::Kind::InvalidName)
            .track(tracks)?;

        Ok(Self::new(name, raw, path.to_str()))
    }

    /// Loads all track lists from the data directory. And prepares to show user track options.
    pub async fn load_all() -> tracks::Result<Vec<Self>> {
        let mut full_list: Vec<Self> = Vec::new();
        full_list.push(Self::new(
            "chillhop",
            include_str!("../../data/chillhop.txt"),
            None,
        ));

        let dir = data_dir().map_err(|_| error::Kind::InvalidPath)?;
        let mut entries = fs::read_dir(dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();

            // filter for .txt
            if path.extension().and_then(|x| x.to_str()) != Some("txt") {
                continue;
            }

            // get filename as str.
            if let Some(track_name) = path.file_name().and_then(|n| n.to_str()) {
                if track_name == "volume.txt" || track_name == "bookmarks.txt" {
                    continue;
                }
            } else {
                continue;
            }

            let raw = fs::read_to_string(path.clone()).await?;

            let name = path
                .file_stem()
                .and_then(|x| x.to_str())
                .ok_or(tracks::error::Kind::InvalidName)
                .track("track list")?;

            full_list.push(Self::new(name, raw.as_ref(), path.to_str()));
        }
        Ok(full_list)
    }
}
