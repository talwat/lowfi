use tokio::fs::{create_dir_all, OpenOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

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

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .read(true)
        .append(false)
        .open(data_dir.join("bookmarks.txt"))
        .await?;

    let mut text = String::new();
    file.read_to_string(&mut text).await?;

    let mut lines: Vec<&str> = text.lines().collect();
    let previous_len = lines.len();
    lines.retain(|line| (*line != entry));
    let contains = lines.len() != previous_len;

    if !contains {
        lines.push(&entry);
    }

    file.set_len(0).await?;
    file.write_all(format!("\n{}\n", lines.join("\n")).as_bytes()).await?;

    Ok(!contains)
}
