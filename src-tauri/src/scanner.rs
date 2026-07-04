//! Recursive folder scan for video files.

use std::path::{Path, PathBuf};

use crate::extensions::is_supported_extension;

pub fn find_video_files(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut found = Vec::new();

    for entry in walkdir::WalkDir::new(root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let is_video = path
            .extension()
            .and_then(|e| e.to_str())
            .map(is_supported_extension)
            .unwrap_or(false);
        if is_video {
            found.push(path.to_path_buf());
        }
    }

    found.sort();
    Ok(found)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_videos_recursively_and_skips_non_video_files() {
        let dir = std::env::temp_dir().join(format!("scanner-test-{}", uuid::Uuid::new_v4()));
        let nested = dir.join("sub").join("deeper");
        std::fs::create_dir_all(&nested).unwrap();

        std::fs::write(dir.join("clip.mp4"), b"x").unwrap();
        std::fs::write(dir.join("sub").join("clip.MOV"), b"x").unwrap(); // case-insensitive
        std::fs::write(nested.join("clip.mts"), b"x").unwrap();
        std::fs::write(nested.join("notes.txt"), b"x").unwrap();
        std::fs::write(dir.join("photo.jpg"), b"x").unwrap();

        let found = find_video_files(&dir).unwrap();
        assert_eq!(found.len(), 3);
        assert!(found.iter().all(|p| p
            .extension()
            .and_then(|e| e.to_str())
            .map(is_supported_extension)
            .unwrap_or(false)));

        std::fs::remove_dir_all(&dir).ok();
    }
}
