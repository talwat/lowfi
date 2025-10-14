//! The module containing all of the logic behind track lists,
//! as well as obtaining track names & downloading the raw audio data

use std::{
    cmp::min,
    path::{Path, PathBuf},
    sync::atomic::Ordering,
};

use async_recursion::async_recursion;
use atomic_float::AtomicF32;
use bytes::{BufMut, BytesMut};
use eyre::OptionExt as _;
use futures::StreamExt;
use reqwest::{Client, StatusCode};
use tokio::fs;
use urlencoding::decode;
#[cfg(feature = "bandcamp")]
use flate2::read::GzDecoder;
use serde_json;

use super::{cache, utils};
use super::QueuedTrack;

use crate::{
    bandcamp::DiscographyParser,
    debug_log,
    data_dir,
    tracks::{self, error::Context, TrackData, SharedAudioBuffer}
};

/// Represents a list of tracks that can be played.
///
/// See the [README](https://github.com/talwat/lowfi?tab=readme-ov-file#the-format) for more details about the format.
#[derive(Clone)]
pub struct List {
    /// The "name" of the list, usually derived from a filename.
    #[allow(dead_code)]
    pub name: String,

    /// Just the raw file, but seperated by `/n` (newlines).
    /// `lines[0]` is the base/heaeder, with the rest being tracks.
    lines: Vec<String>,

    /// The file path which the list was read from.
    #[allow(dead_code)]
    pub path: Option<String>,
}

#[cfg(any(feature = "bandcamp", feature = "presave"))]
#[derive(serde::Serialize, serde::Deserialize)]
pub struct PresavedBandcampList {
    pub base_url: String,
    pub timestamp: u64,
    pub items_hash: u64,    // hash of all album IDs for integrity checking.
    pub items: Vec<crate::bandcamp::DiscographyItem>,
}


#[cfg(feature = "bandcamp")]
fn read_embedded() -> Option<PresavedBandcampList> {
    let bytes = include_bytes!("../../data/bandcamp_lofigirl.json.gz");
    let mut decoder = GzDecoder::new(std::io::Cursor::new(bytes));
    let mut out = String::new();
    if std::io::Read::read_to_string(&mut decoder, &mut out).is_ok() {
        if let Ok(presaved) = serde_json::from_str::<PresavedBandcampList>(&out) {
            return Some(presaved);
        }
    }
    None
}

// Private helper functions for path resolution.

/// Expands `~` to home directory path.
fn expand_home_dir(path: &mut String) -> eyre::Result<()> {
    if path.starts_with('~') {
        let home_path = dirs::home_dir().ok_or_eyre("Could not find home directory")?;
        let home_str = home_path
            .to_str()
            .ok_or_eyre("Home directory path is not valid UTF-8")?;
        *path = path.replacen('~', home_str, 1);
    }
    Ok(())
}

/// Fixes URI paths on Windows that start with an extra slash.
#[cfg(windows)]
fn fix_windows_uri_path(path: &mut String) {
    if path.starts_with('/') || path.starts_with('\\') {
        let stripped = &path[1..];

        if let Some(c) = stripped.chars().next() {
            if c.is_ascii_alphabetic() && stripped.get(1..2) == Some(":") {
                *path = stripped.to_string();
            }
        }
    }
}

/// Normalizes a file:// path for local filesystem access.
fn normalize_file_path(encoded_path: &str) -> eyre::Result<String> {
    let mut path_str = decode(encoded_path)
        .map_err(|_| eyre::eyre!("Invalid URL encoding"))?
        .into_owned();

    #[cfg(windows)]
    fix_windows_uri_path(&mut path_str);

    expand_home_dir(&mut path_str)?;

    Ok(path_str)
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
        let random = fastrand::usize(1..self.lines.len());
        let line = self.lines[random].clone();

        // Handle format: URL!Displayed Name!ArtURL
        // or format: URL!Displayed Name
        let parts: Vec<&str> = line.splitn(4, '!').collect();
        if parts.len() >= 2 {
            let first = parts[0];
            let display_name = parts[1];
            let final_display_name = display_name.to_string();
            (first.to_owned(), Some(final_display_name))
        } else {
            (line, None)
        }
    }

    /// Expands a local directory recursively, adding all audio files to the list.
    #[async_recursion]
    async fn expand_directory(path: &Path, expanded_tracks: &mut Vec<String>) -> eyre::Result<()> {
        let mut entries = fs::read_dir(path).await?;

        while let Some(entry) = entries.next_entry().await? {
            let entry_path = entry.path();

            if entry_path.is_dir() {
                Self::expand_directory(&entry_path, expanded_tracks).await?;
            } else if entry_path.is_file() {
                if let Some(path_str) = entry_path.to_str() {
                    expanded_tracks.push(format!("file://{path_str}"));
                }
            }
        }

        Ok(())
    }

    /// Expands a Bandcamp page using the new discography module with caching.
    async fn expand_bandcamp(
        base_url: &str,
        client: &Client,
        expanded_tracks: &mut Vec<String>,
        excluded_paths: &[String],
        max_albums: Option<usize>,
    ) -> eyre::Result<()> {
        debug_log!("list.rs - expand_bandcamp: expansion start base_url={} max_albums={:?}", base_url, max_albums);
        if let Some(max) = max_albums {
            if max > 0 {
                eprintln!("Limiting to ({}) albums", max);
            }
        }

        let url_hash = utils::hash_string(base_url);

        // FIRST: Check for existing cache before parsing.
        let cache: Option<cache::BandcampCache> = {
            let data_dir = data_dir()?;
            if let Some(path) = cache::find_existing_cache_path(&data_dir, url_hash) {
                if let Some(gz) = cache::BandcampCache::read_gz_to_string(&path).await {
                    serde_json::from_str(&gz).ok()
                } else if let Ok(cached_content) = fs::read_to_string(&path).await {
                    serde_json::from_str(&cached_content).ok()
                } else { None }
            } else { None }
        };

        let use_cache = cache.is_some();

        if use_cache {
            debug_log!("list.rs - expand_bandcamp: using existing cache");
            // Use cached data.
            if let Some(ref cached) = cache {
                for item in &cached.items {
                    if let Some(tracks) = &item.tracks {
                        for track in tracks {
                            if !excluded_paths.iter().any(|ex| track.url.contains(ex)) {
                                let entry = format!(
                                    "{}!{} by {}",
                                    track.url,
                                    track.name,
                                    track.artist.as_deref().unwrap_or("Unknown Artist")
                                );

                                expanded_tracks.push(entry);
                            }
                        }
                    }
                }
            }
            
            // Start background update if cache is getting old.
            if let Some(ref cached) = cache {
                if cached.is_expired(432000) { // 5 days
                    // Find the actual cache file path by searching for existing cache.
                    let data_dir = data_dir()?;
                    
                    if let Some(path) = cache::find_existing_cache_path(&data_dir, url_hash) {
                        cache::start_cache_update_background(base_url, client, &path);
                    }
                }
            }
            return Ok(());
        }

        eprintln!("Processing Bandcamp: {}", base_url);
        eprintln!("Using fresh Bandcamp data...");
        debug_log!("list.rs - expand_bandcamp: using fresh data from discography module {}", base_url);
        
        // Parse data from Bandcamp.
        let items = DiscographyParser::get_discography_with_tracks(client, base_url, true, max_albums).await
            .map_err(|e| eyre::eyre!("Discography parser failed: {}", e))?;
        
        debug_log!("list.rs - expand_bandcamp: discography module returned {} items", items.len());
        eprintln!("Found {} items", items.len());
        
        // Convert items to cache format and track lines.
        let mut cached_items = Vec::new();
        for item in items {
            if let Some(tracks) = &item.tracks {
                let mut cached_tracks = Vec::new();
                
                // Process tracks once, creating both cached tracks and expanded entries.
                for track in tracks {
                    if !excluded_paths.iter().any(|ex| track.url.contains(ex)) {
                        // Create cached track.
                        cached_tracks.push(cache::CachedTrackInfo {
                            name: track.name.clone(),
                            url: track.url.clone(),
                            artist: track.artist.clone(),
                        });
                        
                        // Create expanded track entry.
                        let entry = format!(
                            "{}!{} by {}",
                            track.url,
                            track.name,
                            track.artist.as_deref().unwrap_or("Unknown Artist")
                        );
                        expanded_tracks.push(entry);
                    }
                }
                
                // Create cached item.
                let cached_item = cache::CachedDiscographyItem {
                    id: item.id,
                    item_type: item.item_type.clone(),
                    name: item.name.clone(),
                    url: item.url.clone(),
                    tracks: Some(cached_tracks),
                };
                
                cached_items.push(cached_item);
                }
            }

            // Save cache to disk.
            if max_albums.is_none() {
            let new_cache = cache::BandcampCache::new(base_url.to_string(), cached_items);
                let cache_key = format!("bandcamp_cache_{}_{}", url_hash, new_cache.items_hash);
                let cache_path_gz = data_dir()?.join(format!("{}.cache.gz", cache_key));
                let cache_json = serde_json::to_string(&new_cache)?;
                cache::write_cache_with_error_handling(&cache_path_gz, &cache_json).await;
        }

        eprintln!("Found {} tracks", expanded_tracks.len());
        Ok(())
    }

    /// Adds Bandcamp referer header if URL is from Bandcamp.
    fn add_bandcamp_referer_if_needed(url: &str, req: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        if url.contains("bandcamp.com") || url.contains("bcbits.com") {
            req.header(reqwest::header::REFERER, "https://bandcamp.com/")
        } else {
            req
        }
    }

    /// Downloads a raw track, but doesn't decode it.
    async fn download(
        &self,
        track: &str,
        client: &Client,
        progress: Option<&AtomicF32>,
    ) -> Result<(TrackData, String), tracks::Error> {
        debug_log!("list.rs - download: start track='{}'", track);
        // If the track has a protocol, then we should ignore the base for it.
        let full_path = if track.contains("://") {
            track.to_owned()
        } else {
            format!("{}{}", self.base(), track)
        };
        debug_log!("list.rs - download: full_path={}", full_path);

        // Resolve Bandcamp page URL to a fresh stream URL if applicable.
        let mut play_url = full_path.clone();
        let is_bandcamp_host = play_url.contains("bandcamp.com") || play_url.contains("bcbits.com");
        let is_track_page = is_bandcamp_host && play_url.contains("/track/");
        if is_track_page {
            if let Ok(Some(s)) = crate::bandcamp::DiscographyParser::get_track_stream_url(client, &play_url).await {
                debug_log!("list.rs - download: resolved stream URL from Bandcamp track page");
                play_url = s;
            }
        }
        
        let data: TrackData = if let Some(x) = play_url.strip_prefix("file://") {
            let path_str = normalize_file_path(x)
                .map_err(|_e| (track, tracks::error::Kind::InvalidPath))?;

            let result = tokio::fs::read(&path_str).await.track(track)?;
            TrackData::Full(result.into())
        } else {
            if let Some(progress) = progress {
                // Try ranged first-chunk download.
                const FIRST_CHUNK: u64 = 128 * 1024; // 128KB
                let is_bandcamp = play_url.contains("bandcamp.com") || play_url.contains("bcbits.com");
                let req = Self::add_bandcamp_referer_if_needed(&play_url, client.get(play_url.clone()));
                debug_log!("list.rs - download: ranged request url={} referer={}", play_url, is_bandcamp);
                let first = req
                    .header(reqwest::header::RANGE, format!("bytes=0-{}", FIRST_CHUNK - 1))
                    .send()
                    .await
                    .track(track)?;
                debug_log!("list.rs - download: ranged status={}", first.status());

                if first.status() != StatusCode::PARTIAL_CONTENT {
                    // Fallback: stream full body to bytes.
                    let total = first
                        .content_length()
                        .ok_or((track, tracks::error::Kind::UnknownLength))?;
                    let mut stream = first.bytes_stream();
                    let mut bytes = BytesMut::new();
                    let mut downloaded: u64 = 0;

                    while let Some(item) = stream.next().await {
                        let chunk = item.track(track)?;
                        let new = min(downloaded + (chunk.len() as u64), total);
                        downloaded = new;
                        progress.store((new as f32) / (total as f32), Ordering::Relaxed);

                        bytes.put(chunk);
                    }
                    debug_log!("list.rs - download: full-body completed bytes={}", downloaded);
                    TrackData::Full(bytes.into())
                } else {
                    // Streaming path with shared buffer.
                    let buffer = SharedAudioBuffer::new();

                    // Parse total size from Content-Range header if present.
                    let total_opt = first
                        .headers()
                        .get(reqwest::header::CONTENT_RANGE)
                        .and_then(|v| v.to_str().ok())
                        .and_then(|s| s.rsplit('/').next())
                        .and_then(|t| t.parse::<u64>().ok());

                    let mut downloaded: u64 = 0;
                    let mut stream = first.bytes_stream();
                    while let Some(item) = stream.next().await {
                        let chunk = item.track(track)?;
                        downloaded += chunk.len() as u64;
                        if let Some(total) = total_opt {
                            progress.store((downloaded as f32) / (total as f32), Ordering::Relaxed);
                        }
                        buffer.append(&chunk);
                    }
                    debug_log!("list.rs - download: first chunk streamed bytes={}", downloaded);

                    // Spawn task to fetch the rest.
                    let url = play_url.clone();
                    let client_clone = client.clone();
                    let buffer_clone = buffer.clone();
                    tokio::spawn(async move {
                        let range_header = format!("bytes={}-", downloaded);
                        let req2 = Self::add_bandcamp_referer_if_needed(&url, client_clone.get(url.clone()));
                        if let Ok(resp) = req2
                            .header(reqwest::header::RANGE, range_header)
                            .send()
                            .await
                        {
                            let mut stream = resp.bytes_stream();
                            let mut _current = downloaded;
                            while let Some(next) = stream.next().await {
                                match next {
                                    Ok(chunk) => {
                                        _current += chunk.len() as u64;
                                        buffer_clone.append(&chunk);
                                    }
                                    Err(_) => break,
                                }
                            }
                            debug_log!("list.rs - download: background range completed bytes={}", _current);
                        }
                        buffer_clone.mark_complete();
                    });

                    TrackData::Streaming(buffer)
                }
            } else {
                // Background download: full body
                let req = Self::add_bandcamp_referer_if_needed(&play_url, client.get(play_url.clone()));
                let response = req.send().await.track(track)?;
                debug_log!("list.rs - download: background full status={}", response.status());
                let bytes = response.bytes().await.track(track)?;
                TrackData::Full(bytes)
            }
        };

        Ok((data, full_path))
    }

    /// Fetches and downloads a random track from the [List].
    ///
    /// The Result's error is a bool, which is true if a timeout error occured,
    /// and false otherwise. This tells lowfi if it shouldn't wait to try again.
    pub async fn random(
        &self,
        client: &Client,
        progress: Option<&AtomicF32>,
    ) -> Result<QueuedTrack, tracks::Error> {
        let (path, custom_name) = self.random_path();
        let (data, full_path) = self.download(&path, client, progress).await?;

        let name = custom_name.map_or_else(
            || super::TrackName::Raw(path.clone()),
            super::TrackName::Formatted,
        );

        Ok(QueuedTrack {
            name,
            full_path,
            data,
        })
    }

    /// Parses text into a [List].
    pub fn new(name: &str, text: &str, path: Option<&str>) -> Self {
        let lines: Vec<String> = text
            .trim_end()
            .lines()
            .map(|x| x.trim_end().to_owned())
            .collect();

        Self {
            lines,
            path: path.map(ToOwned::to_owned),
            name: name.to_owned(),
        }
    }

    /// Reads a [List] from the filesystem using the CLI argument provided.
    pub async fn load(tracks: Option<&String>, bandcamp_mode: bool) -> eyre::Result<Self> {
        debug_log!("list.rs - load: loading start tracks_arg_present={} bandcamp_mode={} (true=Bandcamp, false=Archive)", tracks.is_some(), bandcamp_mode);
        let (name, raw_content, path_str) = if let Some(arg) = tracks {
            // Check if the track is in ~/.local/share/lowfi, in which case we'll load that.
            let path = data_dir()?.join(format!("{arg}.txt"));
            let final_path = if path.exists() { path } else { PathBuf::from(arg) };

            let raw = fs::read_to_string(&final_path).await?;
            let name = final_path
                .file_stem()
                .and_then(|x| x.to_str())
                .ok_or_eyre("invalid track path")?;

            (name.to_owned(), raw, final_path.to_str().map(String::from))
        } else {
            #[cfg(feature = "bandcamp")]
            if bandcamp_mode {
                // Use embedded Bandcamp list.
                let presaved_list = read_embedded()
                    .ok_or_else(|| eyre::eyre!("Embedded Bandcamp presave not found"))?;
                
                // Convert to track lines for playback.
                let mut content = format!("noheader\n{}\n", presaved_list.base_url);
                for item in &presaved_list.items {
                    if let Some(tracks) = &item.tracks {
                        for track in tracks {
                            let mut entry = format!("{}!{}", track.url, track.name);
                            if let Some(artist) = &track.artist {
                                entry.push_str(&format!(" by {}", artist));
                            }
                            content.push_str(&format!("{}\n", entry));
                        }
                    }
                }
                
                (
                    "bandcamp_lofigirl".to_string(),
                    content,
                    None,
                )
            } else {
                (
                    "archive_lofigirl".to_string(),
                    include_str!("../../data/archive_lofigirl.txt").to_string(),
                    None,
                )
            }
            
            #[cfg(not(feature = "bandcamp"))]
            {
                (
                    "archive_lofigirl".to_string(),
                    include_str!("../../data/archive_lofigirl.txt").to_string(),
                    None,
                )
            }
        };

        let mut lines_to_process = raw_content
            .strip_prefix("noheader")
            .map_or(raw_content.as_ref(), |stripped| stripped)
            .lines()
            .peekable();

        let header = lines_to_process.next().unwrap_or("").to_string();
        debug_log!("list.rs - load: header='{}'", header);
        let mut final_lines: Vec<String> = vec![header.clone()];

        let mut excluded_paths: Vec<String> = Vec::new();

        let client = DiscographyParser::create_http_client()?;

        // If this is Bandcamp mode, start background cache creation.
        #[cfg(feature = "bandcamp")]
        if bandcamp_mode && tracks.is_none() {
            let client_clone = client.clone();
            if let Some(presaved_list) = read_embedded() {
                let base_url = presaved_list.base_url.clone();
			tokio::spawn(async move {
                match cache::create_cache_from_presave(&base_url, &client_clone, &presaved_list).await {
                    Ok(true) => {
                        let cache_key = format!("bandcamp_cache_{}_{}", utils::hash_string(&base_url), presaved_list.items_hash);
                        if let Ok(dir) = crate::data_dir() {
                            let cache_path_gz = dir.join(format!("{}.cache.gz", cache_key));
                        // Starting background update if cashe was created.
                        cache::start_cache_update_background(&base_url, &client_clone, &cache_path_gz);
                    }}
                    Ok(false) => {
                        debug_log!("list.rs - load: cache already existed, skipping background update");
                    }
                    Err(e) => {
                        debug_log!("list.rs - load: background cache creation failed: {}", e);
                    }    
				
            }});
            }
        }

        let mut regular_lines = Vec::new();

        for line in lines_to_process {
            let trimmed = line.trim();

            // Handle exclusion patterns.
            if let Some(excluded) = trimmed.strip_prefix('-') {
                excluded_paths.push(excluded.trim().to_string());
                continue;
            }

            // Handle local directory expansion.
            if let Some(dir_path_str) = trimmed.strip_prefix("dir://") {
                let path_str = match normalize_file_path(dir_path_str) {
                    Ok(p) => p,
                    Err(e) => {
                        eprintln!("Warning: Failed to normalize path {}: {}", dir_path_str, e);
                        debug_log!("list.rs - load: normalize dir failed path={} err={} ", dir_path_str, e);
                        continue;
                    }
                };

                let dir_path = PathBuf::from(path_str);
                if dir_path.is_dir() {
                    if let Err(e) = Self::expand_directory(&dir_path, &mut final_lines).await {
                        eprintln!("Warning: Failed to expand directory {}: {}", dir_path.display(), e);
                    }
                } else {
                    eprintln!("Warning: Directory not found: {}", dir_path.display());
                }
            }
            // Handle Bandcamp pages.
            else if trimmed.starts_with("bdcmp:") {
                let url_spec = trimmed
                    .strip_prefix("bdcmp:")
                    .unwrap_or(trimmed)
                    .trim_start_matches('/');
                
                // Normalize Bandcamp URL.
                let normalized_url = if url_spec.starts_with("http://") || url_spec.starts_with("https://") {
                    url_spec.to_string()
                } else {
                    format!("https://{}", url_spec)
                };

                let client_clone = client.clone();
                let excluded_clone = excluded_paths.clone();
                let task = tokio::spawn(async move {
                    let mut tracks = Vec::new();
                    if let Err(e) = Self::expand_bandcamp(
                        &normalized_url,
                        &client_clone,
                        &mut tracks,
                        &excluded_clone,
                        None,
                    )
                    .await
                    {
                        eprintln!("Warning: Failed to expand Bandcamp page {}: {}", normalized_url, e);
                        debug_log!("list.rs - load: bandcamp expand failed url={} err={}", normalized_url, e);
                    }
                    tracks
                });
                match task.await {
                    Ok(tracks) => { debug_log!("list.rs - load: bandcamp chunk merged count={}", tracks.len()); regular_lines.extend(tracks) },
                    Err(e) => { eprintln!("Warning: Bandcamp processing failed: {}", e); debug_log!("list.rs - load: bandcamp join failed err={}", e); },
                }
            }
            // Regular track entry.
            else {
                regular_lines.push(line.to_string());
            }
        }

        // Add regular lines.
        final_lines.extend(regular_lines);

        let expanded_content = final_lines.join("\n");

        debug_log!("list.rs - load: total lines={}", final_lines.len());
        Ok(Self::new(&name, &expanded_content, path_str.as_deref()))
    }
}
