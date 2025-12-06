#![allow(clippy::all)]

use std::path::{Path, PathBuf};

use clap::ValueEnum;
use eyre::bail;
use reqwest::Client;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};

pub mod archive;
pub mod chillhop;
pub mod lofigirl;

/// Represents the different sources which can be scraped.
#[derive(Clone, Copy, PartialEq, Eq, Debug, ValueEnum)]
pub enum Source {
    Lofigirl,
    Archive,
    Chillhop,
}

impl Source {
    /// Gets the cache directory name, for example, `chillhop`.
    pub fn cache_dir(&self) -> &'static str {
        match self {
            Source::Lofigirl => "lofigirl",
            Source::Archive => "archive",
            Source::Chillhop => "chillhop",
        }
    }

    /// Gets the full root URL of the source.
    pub fn url(&self) -> &'static str {
        match self {
            Source::Chillhop => "https://chillhop.com",
            Source::Archive => "https://ia601004.us.archive.org/31/items/lofigirl",
            Source::Lofigirl => "https://lofigirl.com/wp-content/uploads",
        }
    }
}

/// Sends a get request, with caching.
async fn get(client: &Client, path: &str, source: Source) -> eyre::Result<String> {
    let trimmed = path.trim_matches('/');
    let cache = PathBuf::from(format!("./cache/{}/{trimmed}.html", source.cache_dir()));

    if let Ok(x) = fs::read_to_string(&cache).await {
        Ok(x)
    } else {
        let resp = client
            .get(format!("{}/{trimmed}", source.url()))
            .send()
            .await?;

        let status = resp.status();

        if status == 429 {
            bail!("rate limit reached: {path}");
        }

        if status != 404 && !status.is_success() && !status.is_redirection() {
            bail!("non success code {}: {path}", resp.status().as_u16());
        }

        let text = resp.text().await?;

        let parent = cache.parent();
        if let Some(x) = parent {
            if x != Path::new("") {
                fs::create_dir_all(x).await?;
            }
        }

        let mut file = File::create(&cache).await?;
        file.write_all(text.as_bytes()).await?;

        if status.is_redirection() {
            bail!("redirect: {path}")
        }

        if status == 404 {
            bail!("not found: {path}")
        }

        Ok(text)
    }
}
