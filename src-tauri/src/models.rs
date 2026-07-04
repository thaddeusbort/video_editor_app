//! Versioned data model for a project file.
//!
//! These structs are the Rust side of the schema; the TypeScript side lives
//! in `src/types/project.ts`. Keep the two in sync by hand - field names use
//! `camelCase` on the wire (via `serde(rename_all = "camelCase")`) so they
//! match the TS types exactly with no translation layer in the frontend.

use serde::{Deserialize, Serialize};

/// Bump this whenever `Project`'s shape changes in a way that isn't purely
/// additive-and-optional. `load_project` uses this to reject files written
/// by an incompatible future version instead of silently misreading them.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GpsCoord {
    pub lat: f64,
    pub lon: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InOutRange {
    pub start_seconds: f64,
    pub end_seconds: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ManualOverrides {
    pub captured_at: Option<String>,
    pub make: Option<String>,
    pub model: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SourceFile {
    pub id: String,
    pub path: String,
    pub hash: String,
    pub filename: String,
    pub duration_seconds: Option<f64>,
    pub make: Option<String>,
    pub model: Option<String>,
    /// ISO 8601, with a timezone offset when ExifTool provided one.
    pub captured_at: Option<String>,
    pub gps: Option<GpsCoord>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fps: Option<f64>,
    /// Absolute path into the thumbnail cache; `None` if extraction failed.
    pub thumbnail_path: Option<String>,
    /// The full `exiftool -json` object for this file, untouched, so tags
    /// we haven't mapped yet (DJI maker notes, XMP, etc.) aren't lost.
    pub raw_metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Clip {
    pub id: String,
    pub source_file_id: String,
    pub in_out_ranges: Vec<InOutRange>,
    pub include: bool,
    pub device_nickname_id: Option<String>,
    pub manual_overrides: Option<ManualOverrides>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Project {
    pub schema_version: u32,
    pub created_at: String,
    pub modified_at: String,
    pub source_files: Vec<SourceFile>,
    pub clips: Vec<Clip>,
}

/// Diagnostic for a single file that failed (or partially failed) during an
/// import run. Intentionally *not* part of `Project` - it's a transient
/// report for this run, not something that belongs in the saved project file.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportFileError {
    pub path: String,
    pub message: String,
}

/// Return type of the `import_folder` command: the project plus whatever
/// went wrong along the way, so one bad file can't take down the whole import.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportOutcome {
    pub project: Project,
    pub errors: Vec<ImportFileError>,
}
