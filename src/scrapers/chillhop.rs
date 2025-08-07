use std::path::{Path, PathBuf};

use reqwest::Client;
use tokio::{
    fs::{self, File},
    io::AsyncWriteExt,
};

struct Release {
    pub tracks: Vec<String>,
    pub author: String,
}

struct Data {
    pub releases: Vec<Release>,
}

/// Sends a get request, with caching.
async fn get(client: &Client, path: &str) -> String {
    let cache = PathBuf::from(format!("./cache/chillhop/{path}.html"));
    if let Ok(x) = fs::read_to_string(&cache).await {
        x
    } else {
        let resp = client
            .get(format!("https://chillhop.com/{path}"))
            .send()
            .await
            .unwrap();
        let text = resp.text().await.unwrap();

        let parent = cache.parent();
        if let Some(x) = parent {
            if x != Path::new("") {
                fs::create_dir_all(x).await.unwrap();
            }
        }

        let mut file = File::create(&cache).await.unwrap();
        file.write_all(text.as_bytes()).await.unwrap();

        text
    }
}

pub async fn scrape() {
    const PAGE_COUNT: usize = 40;
    const USER_AGENT: &str = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/138.0.0.0 Safari/537.36";

    fs::create_dir_all("./cache/chillhop").await.unwrap();
    let client = Client::builder().user_agent(USER_AGENT).build().unwrap();

    get(&client, "releases/?page=30").await;
}
