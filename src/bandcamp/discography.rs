//! The module which parses the Bandcamp discography and fetches the tracks.
//! Can't exist without https://github.com/patrickkfkan/bandcamp-fetch

use std::collections::HashMap;
use reqwest::Client;
use scraper::{Html, Selector};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use regex::Regex;
use eyre::Result;
use crate::debug_log;

// Constant with excluded albums by ID.
// Used to exclude albums.
const EXCLUDED_ALBUMS: &[u64] = &[
    846504073, // https://lofigirl.bandcamp.com/album/the-life-of-a-lofi-boy // Vocals
];

// Function to check album exclusion by ID.
pub fn is_album_excluded(item: &DiscographyItem) -> bool {
    item.id.map_or(false, |id| EXCLUDED_ALBUMS.contains(&id))
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct DiscographyItem {
    pub item_type: String,
    pub id: Option<u64>,
    pub name: String,
    pub url: String,
    pub image_url: Option<String>,
    pub tracks: Option<Vec<TrackInfo>>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct TrackInfo {
    pub name: String,
    pub url: String,
    pub artist: Option<String>,
}

pub struct DiscographyParser;

impl DiscographyParser {
    /// Resolves a fresh mp3-128 stream URL from a Bandcamp track page.
    pub async fn get_track_stream_url(client: &Client, track_url: &str) -> Result<Option<String>> {
        debug_log!("discography.rs - get_track_stream_url: fetching track page: {}", track_url);
        let html = Self::fetch_html(client, track_url).await?;

        let document = Html::parse_document(&html);
        let selector = Selector::parse("script[data-tralbum]").unwrap();
        if let Some(script) = document.select(&selector).next() {
            if let Some(data_tralbum) = script.value().attr("data-tralbum") {
                let decoded = html_escape::decode_html_entities(data_tralbum);
                if let Ok(tralbum) = serde_json::from_str::<Value>(&decoded) {
                    let stream_url = tralbum
                        .get("trackinfo")
                        .and_then(|v| v.as_array())
                        .and_then(|arr| arr.get(0))
                        .and_then(|ti| ti.get("file"))
                        .and_then(|f| f.get("mp3-128"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    return Ok(stream_url);
                }
            }
        }

        Ok(None)
    }
    pub async fn get_discography(client: &Client, artist_url: &str) -> Result<Vec<DiscographyItem>> {
        debug_log!("discography.rs - get_discography: getting discography for artist: {}", artist_url);
        Self::get_discography_with_tracks(client, artist_url, false, None).await
    }

    pub async fn get_discography_with_tracks(
        client: &Client,
        artist_url: &str,
        include_tracks: bool,
        max_albums: Option<usize>,
    ) -> Result<Vec<DiscographyItem>> {
        debug_log!("discography.rs - get_discography_with_tracks: processing artist={} include_tracks={} max_albums={:?}", artist_url, include_tracks, max_albums);
        
        // Check if this is already an album URL.
        let music_url = if artist_url.contains("/album/") {
            artist_url.to_string()
        } else {
            // This is an artist URL, add /music
            format!(
                "{}/music",
                artist_url.trim_end_matches('/').trim_end_matches("/music")
            )
        };

        debug_log!("discography.rs - get_discography_with_tracks: fetching HTML from: {}", music_url);
        let html = Self::fetch_html(client, &music_url).await?;
        debug_log!("discography.rs - get_discography_with_tracks: received HTML, parsing discography");
        let mut items = Self::parse_discography_html(&html, artist_url)?;
        debug_log!("discography.rs - get_discography_with_tracks: parsed {} items", items.len());

        if let Some(max) = max_albums {
            if max > 0 {
                debug_log!("discography.rs - get_discography_with_tracks: applying max_albums limit: {}", max);
                let mut album_counter = 0;
                items.retain(|item| {
                    if item.item_type == "album" {
                        album_counter += 1;
                        let should_keep = album_counter <= max;
                        if !should_keep {
                            debug_log!("discography.rs - get_discography_with_tracks: excluding album {} (limit reached)", item.name);
                        }
                        should_keep
                    } else {
                        true // Keep tracks and other types.
                    }
                });
                debug_log!("discography.rs - get_discography_with_tracks: after max_albums filter: {} items", items.len());
            }
        }

        if include_tracks {
            debug_log!("discography.rs - get_discography_with_tracks: fetching tracks for albums");
            Self::fetch_album_tracks(client, &mut items).await;
        }

        Ok(items)
    }

    async fn fetch_album_tracks(
        client: &Client,
        items: &mut [DiscographyItem],
    ) {
        debug_log!("discography.rs - fetch_album_tracks: starting track extraction");
        let album_urls: Vec<String> = items
            .iter()
            .filter(|item| item.item_type == "album")
            .map(|item| item.url.clone())
            .collect();

        let track_urls: Vec<String> = items
            .iter()
            .filter(|item| item.item_type == "track")
            .map(|item| item.url.clone())
            .collect();

        if album_urls.is_empty() && track_urls.is_empty() {
            debug_log!("discography.rs - fetch_album_tracks: no albums or singles found to process");
            return;
        }

        debug_log!("discography.rs - fetch_album_tracks: found {} albums and {} singles, extracting...", album_urls.len(), track_urls.len());
        println!("Found {} albums and {} singles, extracting...", album_urls.len(), track_urls.len());

        // Combine all URLs and process them together.
        let mut all_urls = album_urls;
        all_urls.extend(track_urls);
        
        const BATCH_SIZE: usize = 10;
        let mut processed = 0;

        for chunk in all_urls.chunks(BATCH_SIZE) {
            let handles: Vec<_> = chunk
                .iter()
                .map(|url| {
                    let url = url.clone();
                    let client = client.clone();
                    tokio::spawn(async move {
                        (url.clone(), Self::get_album_tracks(&client, &url).await)
                    })
                })
                .collect();

            for handle in handles {
                if let Ok((url, result)) = handle.await {
                    processed += 1;
                    match result {
                        Ok(tracks) => {
                            println!("  Item {}/{}: {} tracks", processed, all_urls.len(), tracks.len());
                            if let Some(item) = items.iter_mut().find(|i| i.url == url) {
                                item.tracks = Some(tracks);
                            }
                        }
                        Err(e) => {
                            let prefix = if e.to_string().contains("Rate limited") { "WARNING" } else { "ERROR" };
                            println!("  {} Item {}/{}: {}", prefix, processed, all_urls.len(), e);
                        }
                    }
                }
            }

            println!("Processed {}/{} items", processed, all_urls.len());

            if processed < all_urls.len() {
                tokio::time::sleep(tokio::time::Duration::from_secs(4)).await;
            }
        }
    }

    fn parse_discography_html(html: &str, artist_url: &str) -> Result<Vec<DiscographyItem>> {
        let document = Html::parse_document(html);
        let mut items = HashMap::new();

        // Check for single item page.
        if let Some(single) = Self::check_single_item(&document, artist_url)? {
            return Ok(vec![single]);
        }

        // Extract from data-client-items and HTML links.
        Self::extract_data_client_items(&document, artist_url, &mut items)?;
        Self::extract_html_links(&document, artist_url, &mut items)?;

        // Filter excluded albums.
        let filtered_items: Vec<DiscographyItem> = items
            .into_values()
            .filter(|item| !is_album_excluded(item))
            .collect();

        Ok(filtered_items)
    }

    fn check_single_item(document: &Html, artist_url: &str) -> Result<Option<DiscographyItem>> {
        if !Self::is_single_page(document) {
            return Ok(None);
        }

        let selector = Selector::parse("script[type=\"application/ld+json\"]").unwrap();
        for script in document.select(&selector) {
            if let Some(json_text) = script.text().next() {
                if let Ok(parsed) = serde_json::from_str::<Value>(json_text) {
                    return Self::parse_json_ld(&parsed, artist_url);
                }
            }
        }
        Ok(None)
    }

    fn is_single_page(document: &Html) -> bool {
        let selector = Selector::parse("#discography").unwrap();
        document
            .select(&selector)
            .next()
            .map(|el| {
                let style = el.value().attr("style").unwrap_or("");
                let li_count = document.select(&Selector::parse("#discography li").unwrap()).count();
                style.contains("display: none") || li_count <= 1
            })
            .unwrap_or(true)
    }

    fn extract_html_links(
        document: &Html,
        artist_url: &str,
        items: &mut HashMap<String, DiscographyItem>,
    ) -> Result<()> {
        let album_selector = Selector::parse("a[href*='/album/']").unwrap();
        for link in document.select(&album_selector) {
            if let Some(href) = link.value().attr("href") {
                if !href.starts_with("/album/") {
                    continue;
                }

                let full_url = Self::normalize_url(href, artist_url);
                if items.contains_key(&full_url) {
                    continue;
                }

                let item = DiscographyItem {
                    item_type: "album".to_string(),
                    id: Self::extract_id(link, document, href),
                    name: Self::extract_title(link),
                    url: full_url.clone(),
                    image_url: Self::extract_image_url(link),
                    tracks: None,
                };

                items.insert(full_url, item);
            }
        }

        // Extract tracks (singles).
        let track_selector = Selector::parse("a[href*='/track/']").unwrap();
        for link in document.select(&track_selector) {
            if let Some(href) = link.value().attr("href") {
                if !href.starts_with("/track/") {
                    continue;
                }

                let full_url = Self::normalize_url(href, artist_url);
                if items.contains_key(&full_url) {
                    continue;
                }

                let item = DiscographyItem {
                    item_type: "track".to_string(),
                    id: Self::extract_id(link, document, href),
                    name: Self::extract_title(link),
                    url: full_url.clone(),
                    image_url: Self::extract_image_url(link),
                    tracks: None,
                };

                items.insert(full_url, item);
            }
        }
        Ok(())
    }

    fn extract_image_url(link: scraper::ElementRef) -> Option<String> {
        link.select(&Selector::parse("img").unwrap())
            .next()
            .and_then(|img| {
                img.value()
                    .attr("data-original")
                    .or_else(|| img.value().attr("src"))
            })
            .map(|src| {
                Regex::new(r"_(\d+)\.jpg$")
                    .unwrap()
                    .replace(src, "_9.jpg")
                    .to_string()
            })
    }

    fn extract_title(link: scraper::ElementRef) -> String {
        link.select(&Selector::parse(".title").unwrap())
            .next()
            .map(|el| Self::normalize_text(&el.text().collect::<String>()))
            .unwrap_or_else(|| "Unknown".to_string())
    }

    fn extract_id(link: scraper::ElementRef, document: &Html, href: &str) -> Option<u64> {
        let id_regex = Regex::new(r"(?:album|track)-(\d+)$").unwrap();

        // Check parent element.
        let parent_id = link
            .parent()
            .and_then(|p| p.value().as_element())
            .and_then(|el| {
                el.attrs
                    .iter()
                    .find(|(name, _)| name.local.as_ref() == "data-item-id")
                    .map(|(_, val)| val.as_ref())
            })
            .and_then(|id_str| id_regex.captures(id_str))
            .and_then(|cap| cap.get(1))
            .and_then(|m| m.as_str().parse::<u64>().ok());

        if parent_id.is_some() {
            return parent_id;
        }

        // Check hidden elements.
        let hidden_selector = Selector::parse("li[data-item-id]").unwrap();
        for li in document.select(&hidden_selector) {
            if let Some(link) = li.select(&Selector::parse("a[href*='/album/']").unwrap()).next() {
                if link.value().attr("href") == Some(href) {
                    if let Some(data_item_id) = li.value().attr("data-item-id") {
                        if let Some(cap) = id_regex.captures(data_item_id) {
                            return cap.get(1).and_then(|m| m.as_str().parse::<u64>().ok());
                        }
                    }
                }
            }
        }

        None
    }

    fn extract_data_client_items(
        document: &Html,
        artist_url: &str,
        items: &mut HashMap<String, DiscographyItem>,
    ) -> Result<()> {
        let selector = Selector::parse("ol[data-client-items]").unwrap();

        if let Some(element) = document.select(&selector).next() {
            if let Some(json_str) = element.value().attr("data-client-items") {
                let decoded = html_escape::decode_html_entities(json_str);
                let extra_items: Vec<Value> = serde_json::from_str(&decoded)?;

                for item_data in extra_items {
                    let item_type = item_data.get("type").and_then(|t| t.as_str());
                    let page_url = item_data.get("page_url").and_then(|u| u.as_str());
                    let name = item_data.get("title").and_then(|t| t.as_str());

                    if let (Some(t), Some(u), Some(n)) = (item_type, page_url, name) {
                        if t == "album" || t == "track" {
                            let url = Self::normalize_url(u, artist_url);
                            let image_url = item_data
                                .get("art_id")
                                .and_then(|id| id.as_u64())
                                .map(|art_id| format!("https://f4.bcbits.com/img/a{}_9.jpg", art_id));

                            items.insert(
                                url.clone(),
                                DiscographyItem {
                                    item_type: t.to_string(),
                                    id: item_data.get("id").and_then(|i| i.as_u64()),
                                    name: n.to_string(),
                                    url,
                                    image_url,
                                    tracks: None,
                                },
                            );
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn parse_json_ld(data: &Value, artist_url: &str) -> Result<Option<DiscographyItem>> {
        let item_type = data.get("@type").and_then(|t| t.as_str()).ok_or_else(|| eyre::eyre!("Missing @type field"))?;
        let name = data
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("Unknown")
            .to_string();
        let url = data
            .get("url")
            .and_then(|u| u.as_str())
            .map(|u| Self::normalize_url(u, artist_url))
            .unwrap_or_else(|| artist_url.to_string());
        let image_url = data
            .get("image")
            .and_then(|i| i.as_str())
            .map(|i| Self::normalize_url(i, artist_url));

        let item_type = if item_type.contains("MusicAlbum") {
            "album"
        } else if item_type.contains("MusicRecording") {
            "track"
        } else {
            "unknown"
        };

        Ok(Some(DiscographyItem {
            item_type: item_type.to_string(),
            id: None,
            name,
            url,
            image_url,
            tracks: None,
        }))
    }

    fn normalize_url(url: &str, base_url: &str) -> String {
        if url.starts_with("http://") || url.starts_with("https://") {
            url.to_string()
        } else if url.starts_with("//") {
            format!("https:{}", url)
        } else if url.starts_with("/") {
            let base = base_url.trim_end_matches("/music");
            format!("{}{}", base, url)
        } else {
            format!("{}/{}", base_url.trim_end_matches("/"), url)
        }
    }

    fn normalize_text(text: &str) -> String {
        text.lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }

    async fn fetch_html(client: &Client, url: &str) -> Result<String> {
        debug_log!("discography.rs - fetch_html: fetching HTML from: {}", url);
        tokio::time::sleep(tokio::time::Duration::from_millis(1000)).await;

        for attempt in 1..=5 {
            debug_log!("discography.rs - fetch_html: attempt {}/5 for URL: {}", attempt, url);
            let resp = client
                .get(url)
                .timeout(std::time::Duration::from_secs(30))
                .send()
                .await
                .map_err(|e| eyre::eyre!("Failed to fetch {}: {}", url, e))?;

            debug_log!("discography.rs - fetch_html: HTTP response status={} for URL: {}", resp.status(), url);

            if resp.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
                if attempt < 5 {
                    debug_log!("discography.rs - fetch_html: rate limited, retrying in 20s (attempt {}/5) for URL: {}", attempt, url);
                    eprintln!("Rate limited: {} — retrying in 20s (attempt {}/5)", url, attempt);
                    tokio::time::sleep(tokio::time::Duration::from_secs(20)).await;
                    continue;
                }
                debug_log!("discography.rs - fetch_html: rate limited after {} attempts for URL: {}", attempt, url);
                return Err(eyre::eyre!("Rate limited by Bandcamp after {} attempts", attempt));
            }

            if !resp.status().is_success() {
                debug_log!("discography.rs - fetch_html: HTTP error {} for URL: {}", resp.status(), url);
                return Err(eyre::eyre!("HTTP error {}: {}", resp.status(), url));
            }

            debug_log!("discography.rs - fetch_html: successfully fetched HTML for URL: {}", url);
            return Ok(resp.text().await?);
        }

        unreachable!()
    }

    pub async fn get_album_tracks(client: &Client, album_url: &str) -> Result<Vec<TrackInfo>> {
        let html = Self::fetch_html(client, album_url).await?;
        Self::parse_album_tracks(&html, album_url)
    }


    fn parse_album_tracks(html: &str, album_url: &str) -> Result<Vec<TrackInfo>> {
        let document = Html::parse_document(html);
        let album_artist = Self::extract_album_artist_name(html)
            .unwrap_or_else(|| "Unknown Artist".to_string());

        // Try JSON data first.
        let tralbum_script = document
            .select(&Selector::parse("script[data-tralbum]").unwrap())
            .next();

        if let Some(script) = tralbum_script {
            if let Some(data_tralbum) = script.value().attr("data-tralbum") {
                let decoded = html_escape::decode_html_entities(data_tralbum);
                if let Ok(tralbum_data) = serde_json::from_str::<Value>(&decoded) {
                    return Self::parse_tralbum_tracks(&tralbum_data, album_url, &album_artist);
                }
            }
        }

        // Fallback to HTML parsing.
        Self::parse_tracks_from_html(&document, album_url, &album_artist)
    }

    fn extract_album_artist_name(html: &str) -> Option<String> {
        // Method 1: TralbumData script.
        if let Some(start) = html.find("var TralbumData = ") {
            if let Some(end) = html[start..].find("};") {
                let json_str = &html[start + 18..start + end + 1];
                if let Ok(data) = serde_json::from_str::<Value>(json_str) {
                    if let Some(artist) = data["artist"].as_str() {
                        return Some(artist.to_string());
                    }
                }
            }
        }

        // Method 2: data-tralbum attribute.
        for pattern in &[r#"data-tralbum="([^"]+)""#, r#"data-tralbum='([^']+)'"#] {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(cap) = re.captures(html) {
                    if let Some(json_str) = cap.get(1) {
                        let decoded = html_escape::decode_html_entities(json_str.as_str());
                        if let Ok(parsed) = serde_json::from_str::<Value>(&decoded) {
                            if let Some(artist) = parsed.get("artist").and_then(|v| v.as_str()) {
                                return Some(artist.to_string());
                            }
                        }
                    }
                }
            }
        }

        // Method 3: data-band attribute.
        for pattern in &[r#"data-band="([^"]+)""#, r#"data-band='([^']+)'"#] {
            if let Ok(re) = Regex::new(pattern) {
                if let Some(cap) = re.captures(html) {
                    if let Some(json_str) = cap.get(1) {
                        let decoded = html_escape::decode_html_entities(json_str.as_str());
                        if let Ok(parsed) = serde_json::from_str::<Value>(&decoded) {
                            if let Some(name) = parsed.get("name").and_then(|v| v.as_str()) {
                                return Some(name.to_string());
                            }
                        }
                    }
                }
            }
        }

        None
    }

    fn normalize_artist_title(
        album_artist: &str,
        track_title_raw: &str,
        track_artist_opt: Option<&String>,
    ) -> (String, String) {
        // Remove trailing " by ..." from title.
        let mut clean_title = Regex::new(r"(?i)\s*,?\s+by\s+.+$")
            .ok()
            .map(|re| re.replace(track_title_raw, "").to_string())
            .unwrap_or_else(|| track_title_raw.to_string())
            .trim()
            .to_string();

        let seps = [" - ", " – ", " — "];
        let is_various = album_artist.to_lowercase().contains("various");
        let mut artist = String::new();

        if is_various {
            // Extract artist from title for Various Artists albums.
            for sep in &seps {
                if let Some((a, t)) = clean_title.split_once(sep) {
                    artist = a.trim().to_string();
                    clean_title = t.trim().to_string();
                    break;
                }
            }

            // Fallback to track artist if available.
            if artist.is_empty() {
                if let Some(ta) = track_artist_opt {
                    if !ta.to_lowercase().contains("various") {
                        artist = ta.trim().to_string();
                    }
                }
            }
        } else {
            // Non-various albums: prefer track-specific artist.
            artist = track_artist_opt
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| album_artist.trim().to_string());

            // Move feat./ft. from title to artist.
            let (t, feat) = Self::extract_featured_artist(&clean_title);
            clean_title = t;
            if let Some(f) = feat {
                artist = format!("{} ft. {}", artist, f);
            }
        }

        // Last resort for Various Artists.
        if artist.to_lowercase().contains("various") {
            for sep in &seps {
                if let Some((a, t)) = clean_title.split_once(sep) {
                    artist = a.trim().to_string();
                    clean_title = t.trim().to_string();
                    break;
                }
            }
        }

        if artist.trim().is_empty() {
            artist = "Lofi Girl".to_string();
        }

        (artist, clean_title)
    }

    fn extract_featured_artist(title: &str) -> (String, Option<String>) {
        let patterns = [
            r"(.+?)\s+ft\.\s+(.+)",
            r"(.+?)\s+feat\.\s+(.+)",
            r"(.+?)\s+featuring\s+(.+)",
            r"(.+?)\s+feat\s+(.+)",
            r"(.+?)\s+ft\s+(.+)",
        ];

        for pattern in &patterns {
            if let Ok(re) = regex::RegexBuilder::new(pattern)
                .case_insensitive(true)
                .build()
            {
                if let Some(cap) = re.captures(title) {
                    let clean = cap.get(1).unwrap().as_str().trim().to_string();
                    let featured = cap.get(2).unwrap().as_str().trim().to_string();
                    return (clean, Some(featured));
                }
            }
        }

        (title.to_string(), None)
    }

    fn parse_tralbum_tracks(
        data: &Value,
        album_url: &str,
        album_artist: &str,
    ) -> Result<Vec<TrackInfo>> {
        let trackinfo = data
            .get("trackinfo")
            .and_then(|v| v.as_array())
            .ok_or_else(|| eyre::eyre!("No trackinfo found"))?;

        let base_url = if album_url.contains("/album/") {
            album_url.split("/album/").next().unwrap_or(album_url)
        } else if album_url.contains("/track/") {
            album_url.split("/track/").next().unwrap_or(album_url)
        } else {
            album_url
        };

        Ok(trackinfo
            .iter()
            .filter_map(|track| {
                let track_title_raw = track.get("title")?.as_str()?.to_string();
                let track_artist_opt = track.get("artist").and_then(|v| v.as_str()).map(String::from);

                let (artist, title) = Self::normalize_artist_title(
                    album_artist,
                    &track_title_raw,
                    track_artist_opt.as_ref(),
                );

                let url = track
                    .get("title_link")
                    .and_then(|v| v.as_str())
                    .map(|link| {
                        if link.starts_with("http") {
                            link.replace("/album/", "/")
                        } else {
                            format!("{}{}", base_url, link)
                        }
                    })
                    .unwrap_or_default();

                Some(TrackInfo {
                    name: title,
                    url,
                    artist: Some(artist),
                })
            })
            .collect())
    }

    fn parse_tracks_from_html(
        document: &Html,
        album_url: &str,
        album_artist: &str,
    ) -> Result<Vec<TrackInfo>> {
        let selector = Selector::parse("a[href*=\"/track/\"]").unwrap();

        Ok(document
            .select(&selector)
            .filter_map(|link| {
                let href = link.value().attr("href")?;
                let track_name_raw = link.text().collect::<String>().trim().to_string();

                if track_name_raw.is_empty() {
                    return None;
                }

                let (artist, title) =
                    Self::normalize_artist_title(album_artist, &track_name_raw, None);

                Some(TrackInfo {
                    name: title,
                    url: Self::normalize_url(href, album_url),
                    artist: Some(artist),
                })
            })
            .collect())
    }

    /// Creates an HTTP client with appropriate User-Agent for Bandcamp requests.
    pub fn create_http_client() -> eyre::Result<Client> {
        Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .pool_max_idle_per_host(20)
            .pool_idle_timeout(std::time::Duration::from_secs(30))
            .user_agent("Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36")
            .build()
            .map_err(Into::into)
    }
}
