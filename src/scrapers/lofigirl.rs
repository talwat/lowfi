//! Has all of the functions for the `scrape` command.
//!
//! This command is completely optional, and as such isn't subject to the same
//! quality standards as the rest of the codebase.

use futures::{stream::FuturesOrdered, StreamExt};
use lazy_static::lazy_static;
use scraper::{Html, Selector};

const BASE_URL: &str = "https://lofigirl.com/wp-content/uploads/";

lazy_static! {
    static ref SELECTOR: Selector = Selector::parse("html > body > pre > a").unwrap();
}

async fn parse(path: &str) -> eyre::Result<Vec<String>> {
    let response = reqwest::get(format!("{}{}", BASE_URL, path)).await?;
    let document = response.text().await?;

    let html = Html::parse_document(&document);
    Ok(html
        .select(&SELECTOR)
        .skip(5)
        .map(|x| String::from(x.attr("href").unwrap()))
        .collect())
}

/// This function basically just scans the entire file server, and returns a list of paths to mp3 files.
///
/// It's a bit hacky, and basically works by checking all of the years, then months, and then all of the files.
/// This is done as a way to avoid recursion, since async rust really hates recursive functions.
async fn scan(extension: &str, include_full: bool) -> eyre::Result<Vec<String>> {
    let extension = &format!(".{}", extension);

    let items = parse("").await?;

    let mut years: Vec<u32> = items
        .iter()
        .filter_map(|x| {
            let year = x.strip_suffix("/")?;
            year.parse().ok()
        })
        .collect();

    years.sort();

    // A little bit of async to run all of the months concurrently.
    let mut futures = FuturesOrdered::new();

    for year in years {
        let months = parse(&year.to_string()).await?;

        for month in months {
            futures.push_back(async move {
                let path = format!("{}/{}", year, month);

                let items = parse(&path).await.unwrap();
                items
                    .into_iter()
                    .filter_map(|x| {
                        if x.ends_with(extension) {
                            if include_full {
                                Some(format!("{BASE_URL}{path}{x}"))
                            } else {
                                Some(format!("{path}{x}"))
                            }
                        } else {
                            None
                        }
                    })
                    .collect::<Vec<String>>()
            });
        }
    }

    let mut files = Vec::new();
    while let Some(mut result) = futures.next().await {
        files.append(&mut result);
    }

    eyre::Result::Ok(files)
}

pub async fn scrape(extension: String, include_full: bool) -> eyre::Result<()> {
    let files = scan(&extension, include_full).await?;
    for file in files {
        println!("{file}");
    }

    Ok(())
}
