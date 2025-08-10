//! The module containing all of the logic behind track lists,
//! as well as obtaining track names & downloading the raw audio data

use std::{cmp::min, sync::atomic::Ordering};

use atomic_float::AtomicF32;
use bytes::{BufMut, Bytes, BytesMut};
use eyre::OptionExt as _;
use futures::StreamExt;
use rand::Rng as _;
use reqwest::Client;
use tokio::fs;

use crate::{
    data_dir,
    tracks::{self, error::Context},
};

use super::QueuedTrack;

/// Represents a list of tracks that can be played.
///
/// See the [README](https://github.com/talwat/lowfi?tab=readme-ov-file#the-format) for more details about the format.
#[derive(Clone)]
pub struct List {
    /// The "name" of the list, usually derived from a filename.
    #[allow(dead_code)]
    pub name: String,

    /// Just the raw file, but seperated by `/n` (newlines).
    /// `lines[0]` is the base, with the rest being tracks.
    lines: Vec<String>,
}

impl List {
    /// Gets the base URL of the [List].
    pub fn base(&self) -> &str {
        self.lines[0].trim()
    }

    /// Gets the path of a random track.
    ///
    /// The second value in the tuple specifies whether the
    /// track has a custom display name.
    fn random_path(&self) -> (String, Option<String>) {
        // We're getting from 1 here, since the base is at `self.lines[0]`.
        //
        // We're also not pre-trimming `self.lines` into `base` & `tracks` due to
        // how rust vectors work, since it is slower to drain only a single element from
        // the start, so it's faster to just keep it in & work around it.
        let random = rand::thread_rng().gen_range(1..self.lines.len());
        let line = self.lines[random].clone();

        if let Some((first, second)) = line.split_once('!') {
            (first.to_owned(), Some(second.to_owned()))
        } else {
            (line, None)
        }
    }

    /// Downloads a raw track, but doesn't decode it.
    async fn download(
        &self,
        track: &str,
        client: &Client,
        progress: Option<&AtomicF32>,
    ) -> Result<(Bytes, String), tracks::Error> {
        // If the track has a protocol, then we should ignore the base for it.
        let full_path = if track.contains("://") {
            track.to_owned()
        } else {
            format!("{}{}", self.base(), track)
        };

        let data: Bytes = if let Some(x) = full_path.strip_prefix("file://") {
            let path = if x.starts_with('~') {
                let home_path =
                    dirs::home_dir().ok_or((track, tracks::error::Kind::InvalidPath))?;
                let home = home_path
                    .to_str()
                    .ok_or((track, tracks::error::Kind::InvalidPath))?;

                x.replace('~', home)
            } else {
                x.to_owned()
            };

            let result = tokio::fs::read(path.clone()).await.track(track)?;
            result.into()
        } else {
            let response = client.get(full_path.clone()).send().await.track(track)?;

            if let Some(progress) = progress {
                let total = response
                    .content_length()
                    .ok_or((track, tracks::error::Kind::UnknownLength))?;
                let mut stream = response.bytes_stream();
                let mut bytes = BytesMut::new();
                let mut downloaded: u64 = 0;

                while let Some(item) = stream.next().await {
                    let chunk = item.track(track)?;
                    let new = min(downloaded + (chunk.len() as u64), total);
                    downloaded = new;
                    progress.store((new as f32) / (total as f32), Ordering::Relaxed);

                    bytes.put(chunk);
                }

                bytes.into()
            } else {
                response.bytes().await.track(track)?
            }
        };

        Ok((data, full_path))
    }

    /// Fetches and downloads a random track from the [List].
    ///
    /// The Result's error is a bool, which is true if a timeout error occured,
    /// and false otherwise. This tells lowfi if it shouldn't wait to try again.
    pub async fn random(
        &self,
        client: &Client,
        progress: Option<&AtomicF32>,
    ) -> Result<QueuedTrack, tracks::Error> {
        let (path, custom_name) = self.random_path();
        let (data, full_path) = self.download(&path, client, progress).await?;

        let name = custom_name.map_or_else(
            || super::TrackName::Raw(path.clone()),
            |formatted| super::TrackName::Formatted(formatted),
        );

        Ok(QueuedTrack {
            name,
            full_path,
            data,
        })
    }

    /// Parses text into a [List].
    pub fn new(name: &str, text: &str) -> Self {
        let lines: Vec<String> = text
            .trim_end()
            .lines()
            .map(|x| x.trim_end().to_owned())
            .collect();

        Self {
            lines,
            name: name.to_owned(),
        }
    }

    /// Reads a [List] from the filesystem using the CLI argument provided.
    pub async fn load(tracks: Option<&String>) -> eyre::Result<Self> {
        if let Some(arg) = tracks {
            // Check if the track is in ~/.local/share/lowfi, in which case we'll load that.
            let name = data_dir()?.join(format!("{arg}.txt"));
            let name = if name.exists() { name } else { arg.into() };

            let raw = fs::read_to_string(name.clone()).await?;

            let name = name
                .file_stem()
                .and_then(|x| x.to_str())
                .ok_or_eyre("invalid track path")?;

            Ok(Self::new(name, &raw))
        } else {
            Ok(Self::new(
                "lofigirl",
                include_str!("../../data/lofigirl.txt"),
            ))
        }
    }
}
