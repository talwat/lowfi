use std::path::{Path, PathBuf};

use clap::ValueEnum;
use eyre::bail;
use reqwest::Client;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};
use crate::debug_log;

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
    debug_log!("scrapers.rs - get: requesting path='{}' source={:?}", path, source);
    let trimmed = path.trim_matches('/');
    let cache = PathBuf::from(format!("./cache/{}/{trimmed}.html", source.cache_dir()));

    if let Ok(x) = fs::read_to_string(&cache).await {
        debug_log!("scrapers.rs - get: cache hit for path='{}' cache={}", path, cache.display());
        Ok(x)
    } else {
        debug_log!("scrapers.rs - get: cache miss for path='{}' cache={}, making HTTP request", path, cache.display());
        let url = format!("{}/{trimmed}", source.url());
        debug_log!("scrapers.rs - get: HTTP GET request to: {}", url);
        let resp = client
            .get(&url)
            .send()
            .await?;

        let status = resp.status();
        debug_log!("scrapers.rs - get: HTTP response status={} for path='{}'", status, path);

        if status == 429 {
            debug_log!("scrapers.rs - get: rate limit reached for path='{}'", path);
            bail!("rate limit reached: {path}");
        }

        if status != 404 && !status.is_success() && !status.is_redirection() {
            debug_log!("scrapers.rs - get: non-success status={} for path='{}'", status, path);
            bail!("non success code {}: {path}", resp.status().as_u16());
        }

        let text = resp.text().await?;
        debug_log!("scrapers.rs - get: received {} bytes for path='{}'", text.len(), path);

        let parent = cache.parent();
        if let Some(x) = parent {
            if x != Path::new("") {
                debug_log!("scrapers.rs - get: creating cache directory: {}", x.display());
                fs::create_dir_all(x).await?;
            }
        }

        debug_log!("scrapers.rs - get: writing cache file: {}", cache.display());
        let mut file = File::create(&cache).await?;
        file.write_all(text.as_bytes()).await?;

        if status.is_redirection() {
            debug_log!("scrapers.rs - get: redirect response for path='{}'", path);
            bail!("redirect: {path}")
        }

        if status == 404 {
            debug_log!("scrapers.rs - get: not found for path='{}'", path);
            bail!("not found: {path}")
        }

        debug_log!("scrapers.rs - get: successfully cached and returned content for path='{}'", path);
        Ok(text)
    }
}
