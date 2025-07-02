use std::io::SeekFrom;

use tokio::fs::{create_dir_all, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncSeekExt, AsyncWriteExt};

use crate::data_dir;

/// Bookmarks a given track with a full path and optional custom name.
///
/// Returns whether the track is now bookmarked, or not.
pub async fn bookmark(path: String, custom: Option<String>) -> eyre::Result<bool> {
    let mut entry = format!("{path}");
    if let Some(custom) = custom {
        entry.push('!');
        entry.push_str(&custom);
    }

    let data_dir = data_dir()?;
    create_dir_all(data_dir.clone()).await?;

    // TODO: Only open and close the file at startup and shutdown, not every single bookmark.
    // TODO: Sort of like PersistentVolume, but for bookmarks.
    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .append(false)
        .open(data_dir.join("bookmarks.txt"))
        .await?;

    let mut text = String::new();
    file.read_to_string(&mut text).await?;

    let mut lines: Vec<&str> = text.trim().lines().filter(|x| !x.is_empty()).collect();
    let idx = lines.iter().position(|x| **x == entry);

    if let Some(idx) = idx {
        lines.remove(idx);
    } else {
        lines.push(&entry);
    }

    let text = format!("\n{}", lines.join("\n"));
    file.seek(SeekFrom::Start(0)).await?;
    file.set_len(0).await?;
    file.write_all(text.as_bytes()).await?;

    Ok(idx.is_none())
}
