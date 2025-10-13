//! Provides functional for caching Bandcamp discography data
//! with automatic background updates and integrity checking.

use flate2::{Compression, write::GzEncoder, read::GzDecoder};
use tokio::fs;
use reqwest::Client;
use eyre::Result;
use crate::{
    tracks::list::PresavedBandcampList,
    bandcamp::DiscographyParser,
    bandcamp::discography::is_album_excluded,
    debug_log,
    data_dir,
};
use super::utils::{current_timestamp, hash_string, HasId, hash_items_with_ids};

#[derive(serde::Serialize, serde::Deserialize)]
pub struct BandcampCache {
    pub base_url: String,
    pub items: Vec<CachedDiscographyItem>,
    pub items_hash: u64,    // hash of all album IDs.
    pub timestamp: u64,     // creation timestamp.
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CachedDiscographyItem {
    pub id: Option<u64>,
    pub item_type: String,
    pub name: String,
    pub url: String,
    pub image_url: Option<String>,
    pub tracks: Option<Vec<CachedTrackInfo>>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct CachedTrackInfo {
    pub name: String,
    pub url: String,
    pub artist: Option<String>,
}

// Implement HasId trait for CachedDiscographyItem.
impl HasId for CachedDiscographyItem {
    fn get_id(&self) -> Option<u64> {
        self.id
    }
}

impl BandcampCache {
    pub fn new(base_url: String, items: Vec<CachedDiscographyItem>) -> Self {
        let items_hash = Self::hash_items(&items);
        Self {
            base_url,
            items,
            items_hash,
            timestamp: current_timestamp(),
        }
    }

    pub fn hash_items(items: &[CachedDiscographyItem]) -> u64 {
        hash_items_with_ids(items)
    }

    pub fn is_expired(&self, max_age_secs: u64) -> bool {
        let now = current_timestamp();
        now - self.timestamp > max_age_secs
    }

    pub fn get_item_ids(&self) -> Vec<Option<u64>> {
        self.items.iter().map(|i| i.id).collect()
    }

    pub fn add_items(&mut self, new_items: Vec<CachedDiscographyItem>) {
        self.items.extend(new_items);
        self.items_hash = Self::hash_items(&self.items);
        self.timestamp = current_timestamp();
    }

    pub async fn read_gz_to_string(path: &std::path::Path) -> Option<String> {
        if !path.exists() { return None; }
        if let Ok(bytes) = fs::read(path).await {
            let mut decoder = GzDecoder::new(std::io::Cursor::new(bytes));
            let mut out = String::new();
            if std::io::Read::read_to_string(&mut decoder, &mut out).is_ok() {
                return Some(out);
            }
        }
        None
    }

    pub async fn write_gz_string(path: &std::path::Path, content: &str) -> Result<()> {
        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        use std::io::Write as _;
        encoder.write_all(content.as_bytes())?;
        let bytes = encoder.finish()?;
        fs::write(path, bytes).await?;
        Ok(())
    }
}

/// Helper function to find existing cache by URL hash.
pub fn find_existing_cache_path(data_dir: &std::path::Path, url_hash: u64) -> Option<std::path::PathBuf> {
    if let Ok(entries) = std::fs::read_dir(data_dir) {
        for entry in entries.flatten() {
            if let Some(file_name) = entry.file_name().to_str() {
                if file_name.starts_with(&format!("bandcamp_cache_{}_", url_hash)) && 
                   (file_name.ends_with(".cache.gz") || file_name.ends_with(".cache")) {
                    return Some(entry.path());
                }
            }
        }
    }
    None
}

/// Writes cache with error handling.
pub async fn write_cache_with_error_handling(path: &std::path::Path, content: &str) {
    if let Err(e) = BandcampCache::write_gz_string(path, content).await {
        eprintln!("Warning: Failed to write cache: {}", e);
    }
}

/// Creates Bandcamp cache from presaved list content in background.
pub async fn create_cache_from_presave(
    base_url: &str,
    _client: &Client,
    presaved_list: &PresavedBandcampList,
) -> Result<bool> {
    debug_log!("cache.rs - create_cache_from_presave: creating cache from presaved list content in background...");
    
    let content_hash = crate::tracks::utils::hash_items_with_ids(&presaved_list.items);
    let url_hash = hash_string(base_url);
    let cache_key = format!("bandcamp_cache_{}_{}", url_hash, content_hash);
    let cache_path_gz = data_dir()?.join(format!("{}.cache.gz", cache_key));

    let data_dir = data_dir()?;
    if find_existing_cache_path(&data_dir, url_hash).is_some() {
        debug_log!("cache.rs - create_cache_from_presave: cache already exists for this URL, skipping creation");
        return Ok(false);
    }

    if presaved_list.items.is_empty() {
        return Err(eyre::eyre!("Presaved list is empty"));
    }

    let mut items = Vec::new();
    for item in &presaved_list.items {
        let cached_tracks = item.tracks.as_ref().map(|tracks| {
            tracks.iter().map(|t| CachedTrackInfo {
                name: t.name.clone(),
                url: t.url.clone(),
                artist: t.artist.clone(),
            }).collect()
        });
        
        let cached_item = CachedDiscographyItem {
            id: item.id,
            item_type: item.item_type.clone(),
            name: item.name.clone(),
            url: item.url.clone(),
            image_url: item.image_url.clone(),
            tracks: cached_tracks,
        };
        items.push(cached_item);
    }
    
    let cache = BandcampCache::new(base_url.to_string(), items);
    
    let cache_json = serde_json::to_string(&cache)?;
    BandcampCache::write_gz_string(&cache_path_gz, &cache_json).await?;
    
    debug_log!("cache.rs - create_cache_from_presave: cache created from presave");
    
    Ok(true)
}

/// Starts background Bandcamp cache update
pub fn start_cache_update_background(
    base_url: &str,
    client: &Client,
    cache_path: &std::path::Path,
) {
    let client_clone = client.clone();
    let base_url_clone = base_url.to_string();
    let cache_path_clone = cache_path.to_path_buf();
    tokio::spawn(async move {
        if let Err(e) = update_cache_background(
            &base_url_clone,
            &client_clone,
            &cache_path_clone,
        ).await {
            debug_log!("cache.rs - start_cache_update_background: background update failed: {}", e);
        }
    });
}

/// Updates Bandcamp cache in the background with incremental updates.
pub async fn update_cache_background(
    base_url: &str,
    client: &Client,
    cache_path: &std::path::Path,
) -> Result<()> {
    debug_log!("cache.rs - update_cache_background: starting background update for: {}", base_url);
    
    let mut cache: BandcampCache = if let Some(s) = BandcampCache::read_gz_to_string(cache_path).await {
        serde_json::from_str(&s).unwrap_or_else(|_| BandcampCache::new(base_url.to_string(), Vec::new()))
    } else if let Ok(cached_content) = fs::read_to_string(cache_path).await {
        serde_json::from_str(&cached_content).unwrap_or_else(|_| BandcampCache::new(base_url.to_string(), Vec::new()))
    } else { BandcampCache::new(base_url.to_string(), Vec::new()) };

    let mut items = DiscographyParser::get_discography(client, base_url).await
        .map_err(|e| eyre::eyre!("Discography parser failed: {}", e))?;

    items.retain(|item| !is_album_excluded(item));
    
    debug_log!("cache.rs - update_cache_background: discovered {} items on page", items.len());
    
    // Convert DiscographyItem to CachedDiscographyItem for hash comparison.
    let temp_cached_items: Vec<CachedDiscographyItem> = items.iter().map(|item| CachedDiscographyItem {
        id: item.id,
        item_type: item.item_type.clone(),
        name: item.name.clone(),
        url: item.url.clone(),
        image_url: item.image_url.clone(),
        tracks: None,
    }).collect();

    // Compare hashes using existing hash_items function.
    let current_hash = BandcampCache::hash_items(&temp_cached_items);
    let cached_hash = cache.items_hash;

    if current_hash == cached_hash {
        debug_log!("cache.rs - update_cache_background: item list unchanged (hash match); cache up-to-date.");
        // Update timestamp to reset the 3-day check cycle.
        cache.timestamp = current_timestamp();
        let cache_json = serde_json::to_string(&cache)?;
        BandcampCache::write_gz_string(cache_path, &cache_json).await?;
        return Ok(());
    }
    
    let mut existing_ids: Vec<Option<u64>> = cache.get_item_ids();
    let mut current_ids: Vec<Option<u64>> = items.iter().map(|item| item.id).collect();
    
    existing_ids.sort();
    current_ids.sort();
    
    let existing_set: std::collections::HashSet<Option<u64>> = existing_ids.into_iter().collect();
    let new_items: Vec<_> = items.into_iter()
        .filter(|item| !existing_set.contains(&item.id))
        .collect();
    
    if !new_items.is_empty() {
        debug_log!("cache.rs - update_cache_background: {} new items to process", new_items.len());
    } else {
        debug_log!("cache.rs - update_cache_background: no new items; cache up-to-date by ID list");
    }
    
    // Process new items by fetching tracks for each new album individually
    if !new_items.is_empty() {
        debug_log!("cache.rs - update_cache_background: fetching tracks for {} new albums", new_items.len());
        
        let mut new_cached_items = Vec::new();
        for new_item in new_items {
            debug_log!("cache.rs - update_cache_background: fetching tracks for album: {}", new_item.name);
            
            let tracks = DiscographyParser::get_album_tracks(client, &new_item.url).await
                .map_err(|e| eyre::eyre!("Failed to fetch tracks for album {}: {}", new_item.name, e))?;

            if !tracks.is_empty() {
                let cached_tracks: Vec<CachedTrackInfo> = tracks
                    .iter()
                    .map(|track| CachedTrackInfo {
                        name: track.name.clone(),
                        url: track.url.clone(),
                        artist: track.artist.clone(),
                    })
                    .collect();
                
                let cached_item = CachedDiscographyItem {
                    id: new_item.id,
                    item_type: new_item.item_type.clone(),
                    name: new_item.name.clone(),
                    url: new_item.url.clone(),
                    image_url: new_item.image_url.clone(),
                    tracks: Some(cached_tracks),
                };
                
                new_cached_items.push(cached_item);
                debug_log!("cache.rs - update_cache_background: processed album {} with {} tracks", new_item.name, tracks.len());
            }
        }
        
        cache.add_items(new_cached_items);
    }

    let cache_json = serde_json::to_string(&cache)?;
    
    // Create new cache file with updated hash.
    let url_hash = hash_string(base_url);
    let new_cache_key = format!("bandcamp_cache_{}_{}", url_hash, cache.items_hash);
    let new_cache_path = cache_path.parent().unwrap().join(format!("{}.cache", new_cache_key));
    let new_cache_path_gz = cache_path.parent().unwrap().join(format!("{}.cache.gz", new_cache_key));
    
    BandcampCache::write_gz_string(&new_cache_path_gz, &cache_json).await?;
    
    // The updated cache will be loaded on next program restart.
    debug_log!("cache.rs - update_cache_background: updated successfully - new tracks will be available on next restart");
    
    // Remove old cache file after successful save.
    if cache_path != new_cache_path {
        let _ = fs::remove_file(cache_path).await; // Ignore errors if file doesn't exist.
        let _ = fs::remove_file(&format!("{}.gz", cache_path.display())).await;
        debug_log!("cache.rs - update_cache_background: old cache file removed: {}", cache_path.display());
    }
    
    debug_log!("cache.rs - update_cache_background: background update completed: {} items total", cache.items.len());
    debug_log!("cache.rs - update_cache_background: saved to: {}", new_cache_path_gz.display());
    Ok(())
}
