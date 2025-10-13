//! Color utilities and cover art functions for terminal output.

use std::{collections::HashMap, io::Cursor, sync::Arc};
use bytes::Bytes;
use crossterm::style::{Color, ResetColor, SetForegroundColor};
use lofty::{file::TaggedFileExt, probe::Probe};
use tokio::sync::RwLock;
use crate::debug_log;

/// Converts an RGB array to a crossterm Color.
pub fn rgb_to_color(rgb: [u8; 3]) -> Color {
    Color::Rgb {
        r: rgb[0],
        g: rgb[1],
        b: rgb[2],
    }
}

/// Converts RGB to grayscale using luminance formula.
pub fn rgb_to_gray(rgb: [u8; 3]) -> u8 {
    (0.299 * rgb[0] as f32 + 0.587 * rgb[1] as f32 + 0.114 * rgb[2] as f32) as u8
}

/// Applies color to text using ANSI escape codes.
///
/// Returns the text wrapped in color control sequences.
pub fn colorize(text: &str, color: [u8; 3]) -> String {
    format!(
        "{}{}{}",
        SetForegroundColor(rgb_to_color(color)),
        text,
        ResetColor
    )
}

/// Creates a request with appropriate headers for Bandcamp URLs.
fn create_bandcamp_request(client: &reqwest::Client, url: &str) -> reqwest::RequestBuilder {
    let mut req = client.get(url);
    if url.contains("bandcamp.com") || url.contains("bcbits.com") {
        req = req.header(reqwest::header::REFERER, "https://bandcamp.com/");
    }
    req
}

/// Downloads image data from URL with proper Bandcamp headers.
async fn download_image_data(client: &reqwest::Client, url: &str) -> Option<bytes::Bytes> {
    debug_log!("color: fetch art url={} (shared client)", url);
    let req = create_bandcamp_request(client, url);
    
    match req.send().await {
        Ok(response) => {
            debug_log!("color: http status={}", response.status());
            if response.status().is_success() {
                match response.bytes().await {
                    Ok(data) => {
                        debug_log!("color: downloaded bytes={}", data.len());
                        Some(data)
                    }
                    Err(e) => { 
                        debug_log!("color: bytes error err={}", e); 
                        None 
                    }
                }
            } else {
                debug_log!("color: non-success status");
                None
            }
        }
        Err(e) => { 
            debug_log!("color: request error err={}", e); 
            None 
        }
    }
}

/// Extracts color palette from cover art URL.
pub async fn extract_color_palette_from_url_with_client(client: &reqwest::Client, url: &str) -> Option<Vec<[u8; 3]>> {
    if let Some((palette, _)) = extract_color_palette_and_bytes_from_url_with_client(client, url).await {
        Some(palette)
    } else {
        None
    }
}

/// Extracts color palette and raw bytes from cover art URL.
pub async fn extract_color_palette_and_bytes_from_url_with_client(
    client: &reqwest::Client, 
    url: &str
) -> Option<(Vec<[u8; 3]>, Vec<u8>)> {
    debug_log!("color: fetch art url={} (shared client) for palette+bytes", url);
    if let Some(data) = download_image_data(client, url).await {
        if let Some(palette) = extract_color_palette_from_image_data(&data) {
            Some((palette, data.to_vec()))
        } else {
            None
        }
    } else {
        None
    }
}

fn extract_color_palette_from_bytes(image_bytes: &[u8], source: &str) -> Option<Vec<[u8; 3]>> {
    debug_log!("color: extract from {} start bytes={}", source, image_bytes.len());
    
    // Try to load image directly from bytes.
    let img = match image::load_from_memory(image_bytes) { 
        Ok(i) => i, 
        Err(_) => { 
            debug_log!("color: load image from {} failed", source); 
            return None; 
        } 
    };

    let small = img.resize(100, 100, image::imageops::FilterType::Nearest);
    let rgb_img = small.to_rgb8();

    let pixels: Vec<u8> = rgb_img.as_raw().clone();
    
    let palette_colors = palette_extract::get_palette_rgb(&pixels);
    
    if palette_colors.is_empty() {
        debug_log!("color: palette empty from {}", source);
        return None;
    }

    let palette: Vec<[u8; 3]> = palette_colors
        .into_iter()
        .take(5)
        .map(|c| [c.r, c.g, c.b])
        .collect();

    debug_log!("color: palette colors_from_{}={}", source, palette.len());
    Some(palette)
}

/// Extracts image data from audio file tags.
/// This is a common utility used by both cover.rs and art.rs.
pub fn extract_image_from_tags(data: &Bytes) -> Option<Vec<u8>> {
    debug_log!("color: extract image from tags start bytes={}", data.len());
    let cursor = Cursor::new(data.clone());

    let tagged_file = Probe::new(cursor)
        .guess_file_type()
        .ok()?;
    let tagged_file = match tagged_file.read() { 
        Ok(tf) => tf, 
        Err(_) => { 
            debug_log!("color: read tagged_file failed"); 
            return None; 
        } 
    };

    let tag = tagged_file
        .primary_tag()
        .or_else(|| tagged_file.first_tag())?;

    let picture = match tag.pictures().first() { 
        Some(p) => p, 
        None => { 
            debug_log!("color: no embedded picture"); 
            return None; 
        } 
    };
    
    Some(picture.data().to_vec())
}

/// Extracts color palette from cover art embedded in audio file tags.
pub fn extract_color_palette(data: &Bytes) -> Option<Vec<[u8; 3]>> {
    if let Some(image_data) = extract_image_from_tags(data) {
        extract_color_palette_from_bytes(&image_data, "tags")
    } else {
        None
    }
}

/// Extracts color palette from raw image data.
pub fn extract_color_palette_from_image_data(data: &Bytes) -> Option<Vec<[u8; 3]>> {
    extract_color_palette_from_bytes(data, "url_image")
}

/// Cache for storing color palettes and art data.
pub struct ArtCache {
    color_cache: RwLock<HashMap<String, Vec<[u8; 3]>>>,
    art_cache: RwLock<HashMap<String, Vec<u8>>>,
}

impl ArtCache {
    pub fn new() -> Self {
        Self {
            color_cache: RwLock::new(HashMap::new()),
            art_cache: RwLock::new(HashMap::new()),
        }
    }

    /// Preloads color palette and art for a track if it has an art URL.
    pub async fn preload_color_palette_and_art(
        self: Arc<Self>,
        client: &reqwest::Client,
        info: &crate::tracks::Info,
        skip_art: bool,
        skip_colors: bool,
    ) {
        if skip_art {
            return;
        }
        
        if let Some(art_url) = &info.art_url {
            if !art_url.is_empty() && art_url.starts_with("http") {
                let cache = self.color_cache.read().await;
                if cache.contains_key(art_url) {
                    return;
                }
                drop(cache);

                // Load color palette and art bytes in background.
                let url = art_url.clone();
                let client = client.clone();
                let cache = Arc::clone(&self);
                tokio::spawn(async move {
                    if let Some((palette, bytes)) = extract_color_palette_and_bytes_from_url_with_client(&client, &url).await {
                        // Always cache art bytes.
                        let mut art_cache = cache.art_cache.write().await;
                        art_cache.insert(url.clone(), bytes);
                        drop(art_cache);
                        
                        // Only cache colors if not skipping colors.
                        if !skip_colors {
                            let mut color_cache = cache.color_cache.write().await;
                            color_cache.insert(url, palette);
                        }
                    }
                });
            }
        }
    }

    /// Gets color palette from cache.
    pub async fn get_color_palette(&self, info: &crate::tracks::Info) -> Option<Vec<[u8; 3]>> {
        if let Some(art_url) = &info.art_url {
            if !art_url.is_empty() && art_url.starts_with("http") {
                let cache = self.color_cache.read().await;
                return cache.get(art_url).cloned();
            }
        }
        None
    }

    /// Gets art from cache.
    pub async fn get_art(&self, info: &crate::tracks::Info) -> Option<Vec<u8>> {
        if let Some(art_url) = &info.art_url {
            if !art_url.is_empty() && art_url.starts_with("http") {
                let cache = self.art_cache.read().await;
                return cache.get(art_url).cloned();
            }
        }
        None
    }

    /// Updates track info with colors if they become available.
    pub async fn update_current_with_colors(&self, current: &Arc<crate::tracks::Info>) -> Option<Arc<crate::tracks::Info>> {
        if current.color_palette.is_none() {
            if let Some(palette) = self.get_color_palette(current).await {
                let mut updated_info = current.as_ref().clone();
                updated_info.color_palette = Some(palette);
                return Some(Arc::new(updated_info));
            }
        }
        None
    }

    /// Direct access to art cache for internal use.
    pub async fn cache_art(&self, url: String, data: Vec<u8>) {
        let mut art_cache = self.art_cache.write().await;
        art_cache.insert(url, data);
    }

    /// Direct access to color cache for internal use.
    pub async fn cache_colors(&self, url: String, palette: Vec<[u8; 3]>) {
        let mut color_cache = self.color_cache.write().await;
        color_cache.insert(url, palette);
    }
}
