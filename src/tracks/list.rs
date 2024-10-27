//! The module containing all of the logic behind track lists,
//! as well as obtaining track names & downloading the raw mp3 data.

use bytes::Bytes;
use rand::Rng;
use reqwest::Client;
use tokio::fs;

use super::Track;

/// Represents a list of tracks that can be played.
///
/// See the [README](https://github.com/talwat/lowfi?tab=readme-ov-file#the-format) for more details about the format.
#[derive(Clone)]
pub struct List {
    /// Just the raw file, but seperated by `/n` (newlines).
    /// `lines[0]` is the base, with the rest being tracks.
    lines: Vec<String>,
}

impl List {
    /// Gets the base URL of the [List].
    pub fn base(&self) -> &str {
        self.lines[0].trim()
    }

    /// Gets the name of a random track.
    fn random_name(&self) -> String {
        // We're getting from 1 here, since the base is at `self.lines[0]`.
        //
        // We're also not pre-trimming `self.lines` into `base` & `tracks` due to
        // how rust vectors work, sinceslow to drain only a single element from
        // the start, so it's faster to just keep it in & work around it.
        let random = rand::thread_rng().gen_range(1..self.lines.len());
        self.lines[random].clone()
    }

    /// Downloads a raw track, but doesn't decode it.
    async fn download(&self, track: &str, client: &Client) -> reqwest::Result<Bytes> {
        // If the track has a protocol, then we should ignore the base for it.
        let url = if track.contains("://") {
            track.to_owned()
        } else {
            format!("{}{}", self.base(), track)
        };

        let response = client.get(url).send().await?;
        let data = response.bytes().await?;

        Ok(data)
    }

    /// Fetches and downloads a random track from the [List].
    pub async fn random(&self, client: &Client) -> reqwest::Result<Track> {
        let name = self.random_name();
        let data = self.download(&name, client).await?;

        Ok(Track { name, data })
    }

    /// Parses text into a [List].
    pub fn new(text: &str) -> Self {
        let lines: Vec<String> = text
            .split_ascii_whitespace()
            .map(ToOwned::to_owned)
            .collect();

        Self { lines }
    }

    /// Reads a [List] from the filesystem using the CLI argument provided.
    pub async fn load(tracks: &Option<String>) -> eyre::Result<Self> {
        if let Some(arg) = tracks {
            // Check if the track is in ~/.local/share/lowfi, in which case we'll load that.
            let name = dirs::data_dir()
                .unwrap()
                .join("lowfi")
                .join(format!("{}.txt", arg));

            let raw = if name.exists() {
                fs::read_to_string(name).await?
            } else {
                fs::read_to_string(arg).await?
            };

            Ok(Self::new(&raw))
        } else {
            Ok(Self::new(include_str!("../../data/lofigirl.txt")))
        }
    }
}
