//! ExifTool integration: batch invocation + the tag -> field mapping.
//!
//! This is the one place that knows which ExifTool tags feed which
//! `SourceFile` field. If you need to map an additional tag (or change a
//! fallback order), this is the only file that should need editing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_shell::ShellExt;

use crate::models::GpsCoord;

/// Number of files per `exiftool` invocation. ExifTool is called in batch
/// mode (one process for many files) because spawning it per-file is slow -
/// but very large imports are chunked so we don't build an unreasonably
/// long argument list.
const BATCH_SIZE: usize = 200;

/// Tag fallback order, tried top-to-bottom.
///
/// We always invoke ExifTool with `-G0`, which prefixes *every* tag name
/// with its group (e.g. `QuickTime:ImageWidth`, not just `ImageWidth`) -
/// confirmed against real `exiftool -json -n -G0` output, not assumed. Each
/// list below tries the group(s) that actually produce that tag for common
/// video containers first, then falls back to the bare name in case a build
/// of ExifTool ever emits it unprefixed.
mod tags {
    pub const CAPTURED_AT: &[&str] = &[
        "QuickTime:CreationDate", // has a timezone offset when the device recorded one
        "QuickTime:CreateDate",
        "EXIF:DateTimeOriginal",
        "CreationDate",
        "CreateDate",
        "DateTimeOriginal",
    ];
    pub const MAKE: &[&str] = &["QuickTime:Make", "EXIF:Make", "Make"];
    pub const MODEL: &[&str] = &["QuickTime:Model", "EXIF:Model", "Model"];
    pub const GPS_LAT: &[&str] = &["Composite:GPSLatitude", "GPSLatitude"];
    pub const GPS_LON: &[&str] = &["Composite:GPSLongitude", "GPSLongitude"];
    pub const DURATION: &[&str] = &[
        "QuickTime:Duration",
        "QuickTime:MediaDuration",
        "Duration",
        "MediaDuration",
    ];
    /// Numbered per-track fallback (e.g. `Track1:Duration`) tried only if
    /// none of the tags above were present.
    pub const DURATION_TRACK_SUFFIX: &str = "Duration";
    pub const WIDTH: &[&str] = &[
        "QuickTime:ImageWidth",
        "EXIF:ImageWidth",
        "QuickTime:SourceImageWidth",
        "ImageWidth",
    ];
    pub const HEIGHT: &[&str] = &[
        "QuickTime:ImageHeight",
        "EXIF:ImageHeight",
        "QuickTime:SourceImageHeight",
        "ImageHeight",
    ];
    pub const FPS: &[&str] = &["QuickTime:VideoFrameRate", "VideoFrameRate", "FrameRate"];
}

pub struct ExtractedFields {
    pub duration_seconds: Option<f64>,
    pub make: Option<String>,
    pub model: Option<String>,
    pub captured_at: Option<String>,
    pub gps: Option<GpsCoord>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
}

/// Runs `exiftool -json -n -G0` over all paths (chunked) and returns, for
/// each input path that ExifTool reported on, its raw JSON metadata object.
/// Paths ExifTool couldn't read at all simply won't have an entry.
pub async fn run_exiftool_batch(
    app: &AppHandle,
    paths: &[PathBuf],
) -> Result<HashMap<String, Value>, String> {
    let mut results = HashMap::new();

    for chunk in paths.chunks(BATCH_SIZE) {
        let mut args: Vec<String> = vec![
            "-json".into(),
            "-n".into(),  // numeric output (decimal GPS, seconds, etc.)
            "-G0".into(), // group-prefix tag names, e.g. "QuickTime:CreateDate"
        ];
        args.extend(chunk.iter().map(|p| p.to_string_lossy().into_owned()));

        let output = app
            .shell()
            .sidecar("exiftool")
            .map_err(|e| format!("failed to prepare exiftool sidecar: {e}"))?
            .args(args)
            .output()
            .await
            .map_err(|e| format!("failed to run exiftool: {e}"))?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.trim().is_empty() {
            // ExifTool always prints a JSON array for files it was given, so
            // empty output means the process itself failed to start - e.g.
            // the Windows launcher couldn't find its `exiftool_files` folder.
            return Err(format!(
                "exiftool failed to start (exit code {:?}). Check the sidecar \
                 setup in the README (on Windows, `exiftool_files` must sit next \
                 to `exiftool.exe`). stderr: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr).trim()
            ));
        }
        let parsed: Vec<Value> = serde_json::from_str(&stdout).map_err(|e| {
            format!(
                "exiftool produced unparseable output ({e}); stderr: {}",
                String::from_utf8_lossy(&output.stderr)
            )
        })?;

        for entry in parsed {
            if let Some(source_file) = entry.get("SourceFile").and_then(|v| v.as_str()) {
                results.insert(source_file.to_string(), entry);
            }
        }
    }

    Ok(results)
}

/// Maps a raw ExifTool JSON object to the subset of fields we track on
/// `SourceFile`. Missing tags simply yield `None` - a file with no metadata
/// at all still imports successfully, just with null fields.
pub fn extract_fields(raw: &Value) -> ExtractedFields {
    let lat = find_f64(raw, tags::GPS_LAT);
    let lon = find_f64(raw, tags::GPS_LON);

    ExtractedFields {
        duration_seconds: find_duration_seconds(raw),
        make: find_string(raw, tags::MAKE),
        model: find_string(raw, tags::MODEL),
        captured_at: find_string(raw, tags::CAPTURED_AT)
            .and_then(|raw_date| exiftool_date_to_iso8601(&raw_date)),
        gps: match (lat, lon) {
            (Some(lat), Some(lon)) => Some(GpsCoord { lat, lon }),
            _ => None,
        },
        width: find_f64(raw, tags::WIDTH).map(|v| v as u32),
        height: find_f64(raw, tags::HEIGHT).map(|v| v as u32),
        fps: find_f64(raw, tags::FPS),
    }
}

fn find_duration_seconds(raw: &Value) -> Option<f64> {
    find_f64(raw, tags::DURATION).or_else(|| {
        // Fall back to any numbered track duration, e.g. "Track1:Duration".
        raw.as_object()?
            .iter()
            .find(|(k, _)| k.ends_with(&format!(":{}", tags::DURATION_TRACK_SUFFIX)))
            .and_then(|(_, v)| as_f64(v))
    })
}

fn find_value<'a>(raw: &'a Value, candidates: &[&str]) -> Option<&'a Value> {
    let obj = raw.as_object()?;
    candidates.iter().find_map(|key| obj.get(*key))
}

fn find_string(raw: &Value, candidates: &[&str]) -> Option<String> {
    find_value(raw, candidates).map(|v| match v {
        Value::String(s) => s.clone(),
        other => other.to_string(),
    })
}

fn find_f64(raw: &Value, candidates: &[&str]) -> Option<f64> {
    find_value(raw, candidates).and_then(as_f64)
}

fn as_f64(v: &Value) -> Option<f64> {
    match v {
        Value::Number(n) => n.as_f64(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

/// Converts ExifTool's `YYYY:MM:DD HH:MM:SS[.ff][+-]HH:MM` date format to
/// proper ISO 8601 (`YYYY-MM-DDTHH:MM:SS[+-]HH:MM`). ExifTool doesn't emit
/// ISO 8601 directly, but the only difference is the date separator and the
/// space between date and time, so a straight reformat (no calendar math)
/// is all that's needed - and it avoids pulling in a full date-parsing
/// dependency just for this.
fn exiftool_date_to_iso8601(raw: &str) -> Option<String> {
    let raw = raw.trim();
    if raw.len() < 19 {
        return None;
    }
    let bytes = raw.as_bytes();
    if bytes[4] != b':' || bytes[7] != b':' {
        return None;
    }
    if &raw[0..4] == "0000" {
        return None; // placeholder date some cameras write when clock is unset
    }
    let date_part = format!("{}-{}-{}", &raw[0..4], &raw[5..7], &raw[8..10]);
    let time_and_zone = raw[10..].trim_start();
    Some(format!("{date_part}T{time_and_zone}"))
}

pub fn absolute_path_key(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn date_reformat_adds_iso_separators() {
        assert_eq!(
            exiftool_date_to_iso8601("2023:08:15 14:23:01-07:00").as_deref(),
            Some("2023-08-15T14:23:01-07:00")
        );
        assert_eq!(
            exiftool_date_to_iso8601("2023:08:15 14:23:01").as_deref(),
            Some("2023-08-15T14:23:01")
        );
    }

    #[test]
    fn date_reformat_rejects_unset_placeholder() {
        assert_eq!(exiftool_date_to_iso8601("0000:00:00 00:00:00"), None);
    }

    #[test]
    fn date_reformat_rejects_garbage() {
        assert_eq!(exiftool_date_to_iso8601("not a date"), None);
    }

    // Real `exiftool -json -n -G0` output for an ffmpeg-generated test clip
    // (captured while building this feature) - exercises the exact key
    // shapes ExifTool actually produces, not a hand-guessed fixture.
    #[test]
    fn extract_fields_maps_real_exiftool_output() {
        let raw = json!({
            "SourceFile": "/tmp/clip_a.mp4",
            "QuickTime:CreateDate": "2023:08:15 21:23:01",
            "QuickTime:Duration": 5,
            "QuickTime:ImageWidth": 320,
            "QuickTime:ImageHeight": 240,
            "QuickTime:VideoFrameRate": 25,
            "Composite:GPSLatitude": 37.77489,
            "Composite:GPSLongitude": -122.41939,
        });

        let fields = extract_fields(&raw);
        assert_eq!(fields.captured_at.as_deref(), Some("2023-08-15T21:23:01"));
        assert_eq!(fields.duration_seconds, Some(5.0));
        assert_eq!(fields.width, Some(320));
        assert_eq!(fields.height, Some(240));
        assert_eq!(fields.fps, Some(25.0));
        let gps = fields.gps.expect("gps should be present");
        assert!((gps.lat - 37.77489).abs() < 1e-6);
        assert!((gps.lon - -122.41939).abs() < 1e-6);
    }

    #[test]
    fn extract_fields_maps_device_make_and_model() {
        // Shape ExifTool reports for an iPhone-recorded QuickTime video.
        let raw = json!({
            "QuickTime:Make": "Apple",
            "QuickTime:Model": "iPhone 13 Pro",
        });
        let fields = extract_fields(&raw);
        assert_eq!(fields.make.as_deref(), Some("Apple"));
        assert_eq!(fields.model.as_deref(), Some("iPhone 13 Pro"));
    }

    #[test]
    fn extract_fields_handles_missing_tags_gracefully() {
        // A file exiftool couldn't identify as video at all (e.g. a
        // corrupted/truncated file) - only generic File: tags present.
        let raw = json!({
            "SourceFile": "/tmp/corrupt.mp4",
            "File:FileType": "TXT",
        });

        let fields = extract_fields(&raw);
        assert_eq!(fields.captured_at, None);
        assert_eq!(fields.duration_seconds, None);
        assert_eq!(fields.make, None);
        assert_eq!(fields.model, None);
        assert!(fields.gps.is_none());
        assert_eq!(fields.width, None);
    }

    #[test]
    fn capture_time_prefers_creation_date_over_create_date() {
        let raw = json!({
            "QuickTime:CreationDate": "2024:01:02 03:04:05-05:00",
            "QuickTime:CreateDate": "2024:01:02 08:04:05",
        });
        let fields = extract_fields(&raw);
        assert_eq!(
            fields.captured_at.as_deref(),
            Some("2024-01-02T03:04:05-05:00")
        );
    }

    #[test]
    fn duration_falls_back_to_numbered_track() {
        let raw = json!({
            "Track1:Duration": 12.5,
        });
        let fields = extract_fields(&raw);
        assert_eq!(fields.duration_seconds, Some(12.5));
    }
}
