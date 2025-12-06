//! Has all of the functions for the `scrape` command.
//!
//! This command is completely optional, and as such isn't subject to the same
//! quality standards as the rest of the codebase.

use std::sync::LazyLock;

use futures_util::{stream::FuturesOrdered, StreamExt};
use reqwest::Client;
use scraper::{Html, Selector};

use crate::scrapers::{get, Source};

static SELECTOR: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("html > body > pre > a").unwrap());

async fn parse(client: &Client, path: &str) -> eyre::Result<Vec<String>> {
    let document = get(client, path, super::Source::Lofigirl).await?;
    let html = Html::parse_document(&document);

    Ok(html
        .select(&SELECTOR)
        .skip(1)
        .map(|x| String::from(x.attr("href").unwrap()))
        .collect())
}

/// This function basically just scans the entire file server, and returns a list of paths to mp3 files.
///
/// It's a bit hacky, and basically works by checking all of the years, then months, and then all of the files.
/// This is done as a way to avoid recursion, since async rust really hates recursive functions.
async fn scan() -> eyre::Result<Vec<String>> {
    let client = Client::new();

    let mut releases = parse(&client, "/").await?;
    releases.truncate(releases.len() - 4);

    // A little bit of async to run all of the months concurrently.
    let mut futures = FuturesOrdered::new();

    for release in releases {
        let client = client.clone();
        futures.push_back(async move {
            let items = parse(&client, &release).await.unwrap();
            items
                .into_iter()
                .filter_map(|x| {
                    if x.ends_with(".mp3") {
                        Some(format!("{release}{x}"))
                    } else {
                        None
                    }
                })
                .collect::<Vec<String>>()
        });
    }

    let mut files = Vec::new();
    while let Some(mut result) = futures.next().await {
        files.append(&mut result);
    }

    eyre::Result::Ok(files)
}

pub async fn scrape() -> eyre::Result<()> {
    println!("{}/", Source::Lofigirl.url());
    let files = scan().await?;
    for file in files {
        println!("{file}");
    }

    Ok(())
}
