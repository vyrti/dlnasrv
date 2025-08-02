use anyhow::Result;
use std::path::PathBuf;
use tokio::fs;

#[derive(Clone, Debug)]
pub struct MediaFile {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    pub size: u64,
    pub mime_type: String,
}

const SUPPORTED_EXTENSIONS: &[(&str, &str)] = &[
    ("mkv", "video/x-matroska"),
    ("mp4", "video/mp4"),
    ("avi", "video/x-msvideo"),
];

pub async fn scan_media_files(dir: &PathBuf) -> Result<Vec<MediaFile>> {
    let mut files = Vec::new();
    let mut entries = fs::read_dir(dir).await?;
    let mut id_counter = 1;

    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if !path.is_file() {
            continue;
        }

        if let Some(extension) = path.extension().and_then(|s| s.to_str()) {
            if let Some((_, mime_type)) = SUPPORTED_EXTENSIONS
                .iter()
                .find(|(ext, _)| *ext == extension.to_lowercase())
            {
                let metadata = entry.metadata().await?;
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();

                files.push(MediaFile {
                    id: id_counter.to_string(),
                    name,
                    path,
                    size: metadata.len(),
                    mime_type: mime_type.to_string(),
                });
                id_counter += 1;
            }
        }
    }
    Ok(files)
}