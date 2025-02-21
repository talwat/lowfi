//! The module containing all of the logic behind track lists,
//! as well as obtaining track names & downloading the raw mp3 data.

use bytes::Bytes;
use eyre::OptionExt as _;
use rand::Rng as _;
use reqwest::Client;
use tokio::fs;

use super::Track;

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
        let (path, custom_name) = self.random_path();
        let data = self.download(&path, client).await?;

        let name = custom_name.map_or(super::TrackName::Raw(path), |formatted| {
            super::TrackName::Formatted(formatted)
        });

        Ok(Track { name, data })
    }

    /// Parses text into a [List].
    pub fn new(name: &str, text: &str) -> Self {
        let lines: Vec<String> = text.trim().lines().map(|x| x.trim().to_owned()).collect();

        Self {
            lines,
            name: name.to_owned(),
        }
    }

    /// Reads a [List] from the filesystem using the CLI argument provided.
    pub async fn load(tracks: Option<&String>) -> eyre::Result<Self> {
        if let Some(arg) = tracks {
            // Check if the track is in ~/.local/share/lowfi, in which case we'll load that.
            let name = dirs::data_dir()
                .ok_or_eyre("data directory not found, are you *really* running this on wasm?")?
                .join("lowfi")
                .join(format!("{arg}.txt"));

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
