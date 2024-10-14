use bytes::Bytes;
use rand::Rng;
use reqwest::Client;
use tokio::fs;

use super::Track;

/// Represents a list of tracks that can be played.
///
/// # Format
///
/// In [List]'s, the first line should be the base URL, followed
/// by the rest of the tracks.
///
/// Each track will be first appended to the base URL, and then
/// the result use to download the track. All tracks should end
/// in `.mp3` and as such must be in the MP3 format.
///
/// lowfi won't put a `/` between the base & track for added flexibility,
/// so for most cases you should have a trailing `/` in your base url.
/// The exception to this is if the track name begins with something like
/// `https://`, where in that case the base will not be prepended to it.
#[derive(Clone)]
pub struct List {
    lines: Vec<String>,
}

impl List {
    /// Gets the base URL of the [List].
    pub fn base(&self) -> &str {
        self.lines[0].trim()
    }

    /// Gets the name of a random track.
    fn random_name(&self) -> String {
        // We're getting from 1 here, since due to how rust vectors work it's
        // slow to drain only a single element from the start, so we can just keep it in.
        let random = rand::thread_rng().gen_range(1..self.lines.len());
        self.lines[random].to_owned()
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
    pub fn new(text: &str) -> eyre::Result<Self> {
        let lines: Vec<String> = text
            .split_ascii_whitespace()
            .map(|x| x.to_owned())
            .collect();

        Ok(Self { lines })
    }

    /// Reads a [List] from the filesystem using the CLI argument provided.
    pub async fn load(tracks: &Option<String>) -> eyre::Result<Self> {
        if let Some(arg) = tracks {
            // Check if the track is in ~/.local/share/lowfi, in which case we'll load that.
            let name = dirs::data_dir()
                .unwrap()
                .join("lowfi")
                .join(arg)
                .join(".txt");

            let raw = if name.exists() {
                fs::read_to_string(name).await?
            } else {
                fs::read_to_string(arg).await?
            };

            List::new(&raw)
        } else {
            List::new(include_str!("../../data/lofigirl.txt"))
        }
    }
}
