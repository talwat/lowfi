use eyre::{bail, eyre};
use futures::{stream::FuturesOrdered, StreamExt};
use lazy_static::lazy_static;
use std::path::{Path, PathBuf};

use reqwest::Client;
use scraper::{Html, Selector};
use serde::Deserialize;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};

lazy_static! {
    static ref RELEASES: Selector = Selector::parse(".table-body > a").unwrap();
    static ref RELEASE_LABEL: Selector = Selector::parse("label").unwrap();
    // static ref RELEASE_DATE: Selector = Selector::parse(".release-feat-props > .text-xs").unwrap();
    // static ref RELEASE_NAME: Selector = Selector::parse(".release-feat-props > h2").unwrap();
    static ref RELEASE_AUTHOR: Selector = Selector::parse(".release-feat-props .artist-link").unwrap();
    static ref RELEASE_TEXTAREA: Selector = Selector::parse("textarea").unwrap();
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Track {
    title: String,
    file_id: String,
    artists: String,
}

#[derive(Deserialize, Debug)]
struct Release {
    #[serde(skip)]
    pub path: String,
    #[serde(skip)]
    pub name: String,
    pub tracks: Vec<Track>,
}

#[derive(thiserror::Error, Debug)]
enum ReleaseError {
    #[error("invalid track: {0}")]
    Invalid(#[from] eyre::Error),

    #[error("track explicitly ignored")]
    Ignored,
}

impl Release {
    pub async fn scan(path: String, client: Client) -> Result<Self, ReleaseError> {
        let content = get(&client, &path).await?;
        let html = Html::parse_document(&content);

        let textarea = html
            .select(&RELEASE_TEXTAREA)
            .next()
            .ok_or(eyre!("unable to find textarea: {path}"))?;
        let mut release: Self = serde_json::from_str(&textarea.inner_html()).unwrap();
        release.tracks.reverse();

        let author = html
            .select(&RELEASE_AUTHOR)
            .next()
            .ok_or(eyre!("unable to find author: {path}"))?;
        if author.inner_html() == "Kenji" {
            return Err(ReleaseError::Ignored);
        }

        Ok(release)
    }
}

/// Sends a get request, with caching.
async fn get(client: &Client, path: &str) -> eyre::Result<String> {
    let trimmed = path.trim_matches('/');
    let cache = PathBuf::from(format!("./cache/chillhop/{trimmed}.html"));

    if let Ok(x) = fs::read_to_string(&cache).await {
        Ok(x)
    } else {
        let resp = client
            .get(format!("https://chillhop.com/{trimmed}"))
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

async fn scan_page(
    number: usize,
    client: &Client,
) -> eyre::Result<Vec<impl futures::Future<Output = Result<Release, ReleaseError>>>> {
    let path = format!("releases/?page={number}");
    let content = get(client, &path).await?;
    let html = Html::parse_document(&content);

    let elements = html.select(&RELEASES);
    Ok(elements
        .filter_map(|x| {
            let label = x.select(&RELEASE_LABEL).next()?.inner_html();
            if label == "Compilation" || label == "Mix" {
                return None;
            }

            Some(Release::scan(x.attr("href")?.to_string(), client.clone()))
        })
        .collect())
}

pub async fn scrape() -> eyre::Result<()> {
    const PAGE_COUNT: usize = 40;
    const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36";

    fs::create_dir_all("./cache/chillhop").await.unwrap();
    let client = Client::builder().user_agent(USER_AGENT).build().unwrap();

    let mut futures = FuturesOrdered::new();

    // This is slightly less memory efficient than I'd hope, but it is what it is.
    for page in 0..=PAGE_COUNT {
        for x in scan_page(page, &client).await? {
            futures.push_front(x);
        }
    }

    while let Some(result) = futures.next().await {
        let release = match result {
            Ok(release) => release,
            Err(error) => {
                eprintln!("error: {}, skipping", error);
                continue;
            }
        };

        for track in release.tracks {
            let title = html_escape::decode_html_entities(&track.title);
            let artist = html_escape::decode_html_entities(
                track.artists.split(", ").next().unwrap_or(&track.artists),
            );

            println!("{}!{artist} - {title}", track.file_id)
        }
    }

    Ok(())
}
