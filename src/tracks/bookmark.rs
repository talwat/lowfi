use tokio::fs::{create_dir_all, OpenOptions};
use tokio::io::AsyncWriteExt;

use crate::data_dir;

pub async fn bookmark(path: String, custom: Option<String>) -> eyre::Result<()> {
    let mut entry = format!("\n{path}");

    if let Some(custom) = custom {
        entry.push('!');
        entry.push_str(&custom);
    }

    let data_dir = data_dir()?;
    create_dir_all(data_dir.clone()).await?;

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .append(true)
        .open(data_dir.join("bookmarks.txt"))
        .await?;

    file.write_all(entry.as_bytes()).await?;

    Ok(())
}
