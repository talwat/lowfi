use eyre::eyre;
use futures::stream::FuturesUnordered;
use futures::StreamExt;
use indicatif::ProgressBar;
use lazy_static::lazy_static;
use std::fmt;
use std::str::FromStr;

use reqwest::Client;
use scraper::{Html, Selector};
use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};
use tokio::fs;

use crate::scrapers::{get, Source};

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
    #[serde(deserialize_with = "deserialize_u32_from_string")]
    file_id: u32,
    artists: String,
}

impl Track {
    pub fn clean(&mut self) {
        self.artists = html_escape::decode_html_entities(&self.artists).to_string();

        self.title = html_escape::decode_html_entities(&self.title).to_string();
    }
}

#[derive(Deserialize, Debug)]
struct Release {
    #[serde(skip)]
    pub path: String,

    #[serde(skip)]
    pub index: usize,

    pub tracks: Vec<Track>,
}

#[derive(thiserror::Error, Debug)]
enum ReleaseError {
    #[error("invalid release: {0}")]
    Invalid(#[from] eyre::Error),
}

impl Release {
    pub async fn scan(
        path: String,
        index: usize,
        client: Client,
        bar: ProgressBar,
    ) -> Result<Self, ReleaseError> {
        let content = get(&client, &path, Source::Chillhop).await?;
        let html = Html::parse_document(&content);

        let textarea = html
            .select(&RELEASE_TEXTAREA)
            .next()
            .ok_or(eyre!("unable to find textarea: {path}"))?;

        let mut release: Self = serde_json::from_str(&textarea.inner_html()).unwrap();
        release.path = path;
        release.index = index;
        release.tracks.reverse();

        bar.inc(release.tracks.len() as u64);

        Ok(release)
    }
}

async fn scan_page(
    number: usize,
    client: &Client,
    bar: ProgressBar,
) -> eyre::Result<Vec<impl futures::Future<Output = Result<Release, ReleaseError>>>> {
    let path = format!("releases/?page={number}");
    let content = get(client, &path, Source::Chillhop).await?;
    let html = Html::parse_document(&content);

    let elements = html.select(&RELEASES);
    Ok(elements
        .enumerate()
        .filter_map(|(i, x)| {
            let label = x.select(&RELEASE_LABEL).next()?.inner_html();
            if label == "Compilation" {
                return None;
            }

            Some(Release::scan(
                x.attr("href")?.to_string(),
                (number * 12) + i,
                client.clone(),
                bar.clone(),
            ))
        })
        .collect())
}

pub async fn scrape() -> eyre::Result<()> {
    const PAGE_COUNT: usize = 40;
    const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36";
    const TRACK_COUNT: u64 = 1625;

    const IGNORED_TRACKS: [u32; 28] = [
        // 404
        74707, // Lyrics
        21655, 21773, 8172, 55397, 75135, 24827, 8141, 8157, 64052, 31612, 41956, 8001, 9217, 8730,
        55372, 9262, 30131, 9372, 20561, 21652, 9306, 21646, // Abnormal
        8469, 7832, 10448, 9446, 9396,
    ];

    const IGNORED_ARTISTS: [&str; 1] = [
        "Kenji", // Lyrics
    ];

    fs::create_dir_all("./cache/chillhop").await.unwrap();
    let client = Client::builder().user_agent(USER_AGENT).build().unwrap();

    let futures = FuturesUnordered::new();
    let bar = ProgressBar::new(TRACK_COUNT + (12 * (PAGE_COUNT as u64)));

    let mut errors = Vec::new();

    // This is slightly less memory efficient than I'd hope, but it is what it is.
    for page in 0..=PAGE_COUNT {
        bar.inc(12);
        for x in scan_page(page, &client, bar.clone()).await? {
            futures.push(x);
        }
    }

    let mut results: Vec<Result<Release, ReleaseError>> = futures.collect().await;
    bar.finish_and_clear();

    // I mean, is it... optimal? Absolutely not. Does it work? Yes.
    eprintln!("sorting...");
    results.sort_by_key(|x| if let Ok(x) = x { x.index } else { 0 });
    results.reverse();

    eprintln!("printing...");
    let mut printed = Vec::with_capacity(TRACK_COUNT as usize); // Lazy way to get rid of dupes.
    for result in results {
        let release = match result {
            Ok(release) => release,
            Err(error) => {
                errors.push(error);
                continue;
            }
        };

        for mut track in release.tracks {
            if IGNORED_TRACKS.contains(&track.file_id) {
                continue;
            }

            if IGNORED_ARTISTS.contains(&track.artists.as_ref()) {
                continue;
            }

            if printed.contains(&track.file_id) {
                continue;
            }

            printed.push(track.file_id);

            track.clean();
            println!("{}!{}", track.file_id, track.title);
        }
    }

    eprintln!("-- ERROR REPORT --");
    for error in errors {
        eprintln!("{error}");
    }

    Ok(())
}

pub fn deserialize_u32_from_string<'de, D>(deserializer: D) -> Result<u32, D::Error>
where
    D: Deserializer<'de>,
{
    struct U32FromStringVisitor;

    impl<'de> Visitor<'de> for U32FromStringVisitor {
        type Value = u32;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("a string containing an unsigned 32-bit integer")
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            u32::from_str(value).map_err(|_| {
                de::Error::invalid_value(
                    de::Unexpected::Str(value),
                    &"a valid unsigned 32-bit integer",
                )
            })
        }
    }

    deserializer.deserialize_str(U32FromStringVisitor)
}
