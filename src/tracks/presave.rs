//! Module i used to create bandcamp list.
//! Maybe it's usefull.
//! Not needed for regular use.

use serde::{Deserialize, Serialize};
use flate2::{Compression, write::GzEncoder};
use tokio::fs;
use eyre::Result;

use crate::bandcamp::DiscographyParser;

use super::utils::current_timestamp;
use super::list::PresavedBandcampList;
use super::utils::hash_items_with_ids;

/// Creates a presaved Bandcamp list in ./data directory.
pub async fn create_presaved_bandcamp_list(base_url: &str, max_albums: usize) -> Result<()> {
    eprintln!("Creating presaved Bandcamp list for: {}", base_url);
    if max_albums > 0 {
        eprintln!("Limiting to {} albums", max_albums);
    }
    
    let client = DiscographyParser::create_http_client()?;
    
    // Use the new discography module to fetch data.
    let max_albums_option = if max_albums > 0 { Some(max_albums) } else { None };
    let items = DiscographyParser::get_discography_with_tracks(&client, base_url, true, max_albums_option).await
        .map_err(|e| eyre::eyre!("Discography parser failed: {}", e))?;
    
    eprintln!("Found {} items", items.len());
    
    let items_hash = hash_items_with_ids(&items);
    let presaved = PresavedBandcampList {
        base_url: base_url.to_string(),
        timestamp: current_timestamp(),
        items_hash,
        items,
    };
    
    let json_content = serde_json::to_string(&presaved)?;
    
    let filename = format!("bandcamp_{}.json.gz", items_hash);
    let filepath_gz = format!("./data/{}", filename);
    
    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    use std::io::Write as _;
    encoder.write_all(json_content.as_bytes())?;
    let bytes = encoder.finish()?;
    fs::write(&filepath_gz, bytes).await?;
    eprintln!("Presaved list created: {}", filepath_gz);
    
    let total_tracks: usize = presaved.items.iter()
        .map(|item| item.tracks.as_ref().map(|t| t.len()).unwrap_or(0))
        .sum();
    eprintln!("Total tracks: {}", total_tracks);
    
    Ok(())
}
