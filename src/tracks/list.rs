//! The module containing all of the logic behind track lists,
//! as well as obtaining track names & downloading the raw mp3 data.

use bytes::Bytes;
use eyre::OptionExt as _;
use rand::Rng as _;
use reqwest::Client;
use tokio::fs;

use crate::{data_dir, tracks::TrackError};

use super::QueuedTrack;

/// Represents a list of tracks that can be played.
///
/// See the [README](https://github.com/talwat/lowfi?tab=readme-ov-file#the-format) for more details about the format.
#[derive(Clone)]
pub struct List {
    /// The "name" of the list, usually derived from a filename.
    #[allow(dead_code, reason = "this code may not be dead depending on features")]
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
    async fn download(&self, track: &str, client: &Client) -> Result<(Bytes, String), TrackError> {
        // If the track has a protocol, then we should ignore the base for it.
        let full_path = if track.contains("://") {
            track.to_owned()
        } else {
            format!("{}{}", self.base(), track)
        };

        let data: Bytes = if let Some(x) = full_path.strip_prefix("file://") {
            let path = if x.starts_with("~") {
                let home_path = dirs::home_dir().ok_or(TrackError::InvalidPath)?;
                let home = home_path.to_str().ok_or(TrackError::InvalidPath)?;

                x.replace("~", home)
            } else {
                x.to_owned()
            };

            let result = tokio::fs::read(path).await?;
            result.into()
        } else {
            let response = match client.get(full_path.clone()).send().await {
                Ok(x) => Ok(x),
                Err(x) => {
                    if x.is_timeout() {
                        Err(TrackError::Timeout)
                    } else {
                        Err(TrackError::Request(x))
                    }
                }
            }?;
            response.bytes().await?
        };

        Ok((data, full_path))
    }

    /// Fetches and downloads a random track from the [List].
    ///
    /// The Result's error is a bool, which is true if a timeout error occured,
    /// and false otherwise. This tells lowfi if it shouldn't wait to try again.
    pub async fn random(&self, client: &Client) -> Result<QueuedTrack, TrackError> {
        let (path, custom_name) = self.random_path();
        let (data, full_path) = self.download(&path, client).await?;

        let name = custom_name.map_or(super::TrackName::Raw(path.clone()), |formatted| {
            super::TrackName::Formatted(formatted)
        });

        Ok(QueuedTrack {
            name,
            data,
            full_path,
        })
    }

    /// Parses text into a [List].
    pub fn new(name: &str, text: &str) -> Self {
        let lines: Vec<String> = text
            .trim()
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
