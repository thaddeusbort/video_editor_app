//! Poster-frame thumbnail extraction via FFmpeg, cached by content hash.
//!
//! Caching by hash (rather than by path) means a re-import of the same
//! folder - or the same files after a rename/move - reuses the existing
//! thumbnail instead of re-decoding the video.

use std::path::{Path, PathBuf};

use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

/// Seek position for the poster frame. A couple of seconds in avoids the
/// solid-black/fade-in frames many cameras record at time zero.
const POSTER_FRAME_SECONDS: &str = "2";

pub fn cache_path_for(cache_dir: &Path, hash: &str) -> PathBuf {
    cache_dir.join(format!("{hash}.jpg"))
}

/// Extracts (or reuses a cached) poster frame for `video_path`, returning
/// its path in the thumbnail cache.
pub async fn get_or_create_thumbnail(
    app: &AppHandle,
    video_path: &Path,
    hash: &str,
    cache_dir: &Path,
) -> Result<PathBuf, String> {
    std::fs::create_dir_all(cache_dir)
        .map_err(|e| format!("failed to create thumbnail cache dir: {e}"))?;

    let out_path = cache_path_for(cache_dir, hash);
    if out_path.exists() {
        return Ok(out_path);
    }

    if run_ffmpeg_grab(app, video_path, &out_path, POSTER_FRAME_SECONDS)
        .await
        .is_ok()
        && out_path.exists()
    {
        return Ok(out_path);
    }

    // Clip shorter than the seek offset (or the first attempt otherwise
    // failed to land on a frame) - fall back to the very first frame.
    run_ffmpeg_grab(app, video_path, &out_path, "0").await?;
    if !out_path.exists() {
        return Err("ffmpeg did not produce a thumbnail file".to_string());
    }
    Ok(out_path)
}

async fn run_ffmpeg_grab(
    app: &AppHandle,
    video_path: &Path,
    out_path: &Path,
    seek_seconds: &str,
) -> Result<(), String> {
    let output = app
        .shell()
        .sidecar("ffmpeg")
        .map_err(|e| format!("failed to prepare ffmpeg sidecar: {e}"))?
        .args([
            "-y",
            "-ss",
            seek_seconds,
            "-i",
            &video_path.to_string_lossy(),
            "-frames:v",
            "1",
            "-q:v",
            "3",
            &out_path.to_string_lossy(),
        ])
        .output()
        .await
        .map_err(|e| format!("failed to run ffmpeg: {e}"))?;

    if !output.status.success() {
        return Err(format!(
            "ffmpeg exited with {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}
