//! The Tauri commands exposed to the frontend. This is the only surface the
//! React side talks to - all filesystem access and sidecar process spawning
//! happens here (or in the modules this file calls into).

use std::collections::HashMap;
use std::path::PathBuf;

use tauri::{AppHandle, Emitter, Manager};
use uuid::Uuid;

use crate::extensions;
use crate::hashing;
use crate::metadata;
use crate::models::{
    Clip, ImportFileError, ImportOutcome, ImportPhase, ImportProgress, InOutRange, Project,
    SourceFile, IMPORT_PROGRESS_EVENT, SCHEMA_VERSION,
};
use crate::scanner;
use crate::thumbnails;

#[tauri::command]
pub fn get_supported_extensions() -> Vec<String> {
    extensions::get_supported_extensions()
}

/// Writes the project to `path`, stamping `modifiedAt` with the current
/// time, and returns the project as saved so the frontend picks up the new
/// timestamp.
#[tauri::command]
pub fn save_project(mut project: Project, path: String) -> Result<Project, String> {
    project.modified_at = chrono::Utc::now().to_rfc3339();
    let json = serde_json::to_string_pretty(&project)
        .map_err(|e| format!("failed to serialize project: {e}"))?;

    // Write to a sibling temp file, then rename over the target, so a crash
    // or full disk mid-write can't leave a half-written project file behind.
    let tmp_path = format!("{path}.tmp");
    std::fs::write(&tmp_path, json).map_err(|e| format!("failed to write '{tmp_path}': {e}"))?;
    std::fs::rename(&tmp_path, &path).map_err(|e| {
        let _ = std::fs::remove_file(&tmp_path);
        format!("failed to replace '{path}': {e}")
    })?;
    Ok(project)
}

#[tauri::command]
pub fn load_project(path: String) -> Result<Project, String> {
    let contents =
        std::fs::read_to_string(&path).map_err(|e| format!("failed to read '{path}': {e}"))?;
    let project: Project = serde_json::from_str(&contents)
        .map_err(|e| format!("failed to parse '{path}' as a project file: {e}"))?;
    if project.schema_version != SCHEMA_VERSION {
        return Err(format!(
            "'{path}' has schema version {}, but this app supports version {}",
            project.schema_version, SCHEMA_VERSION
        ));
    }
    Ok(project)
}

#[tauri::command]
pub async fn import_folder(app: AppHandle, folder_path: String) -> Result<ImportOutcome, String> {
    let root = PathBuf::from(&folder_path);
    if !root.is_dir() {
        return Err(format!("'{folder_path}' is not a directory"));
    }

    let report = |phase: ImportPhase, current: usize, total: usize| {
        // Progress is best-effort: a failed emit must never fail the import.
        let _ = app.emit(
            IMPORT_PROGRESS_EVENT,
            ImportProgress {
                phase,
                current,
                total,
            },
        );
    };

    report(ImportPhase::Scanning, 0, 0);
    let video_paths =
        scanner::find_video_files(&root).map_err(|e| format!("failed to scan '{folder_path}': {e}"))?;

    let mut errors: Vec<ImportFileError> = Vec::new();

    // Partial-hash every file up front (see hashing.rs). Anything that
    // can't even be opened is reported and dropped from the rest of the
    // pipeline rather than aborting the whole import.
    let mut hashes: HashMap<PathBuf, String> = HashMap::new();
    for (i, path) in video_paths.iter().enumerate() {
        report(ImportPhase::Hashing, i, video_paths.len());
        match hashing::partial_hash(path) {
            Ok(hash) => {
                hashes.insert(path.clone(), hash);
            }
            Err(e) => errors.push(ImportFileError {
                path: path.to_string_lossy().into_owned(),
                message: format!("failed to read file: {e}"),
            }),
        }
    }
    let readable_paths: Vec<PathBuf> = video_paths
        .iter()
        .filter(|p| hashes.contains_key(*p))
        .cloned()
        .collect();

    let total = readable_paths.len();
    report(ImportPhase::Metadata, 0, total);
    let raw_metadata_by_path = metadata::run_exiftool_batch(&app, &readable_paths, |done| {
        report(ImportPhase::Metadata, done, total)
    })
    .await?;

    let cache_dir = thumbnail_cache_dir(&app)?;

    let mut source_files = Vec::with_capacity(readable_paths.len());
    let mut clips = Vec::with_capacity(readable_paths.len());

    for (i, path) in readable_paths.iter().enumerate() {
        report(ImportPhase::Thumbnails, i, total);
        let hash = hashes.get(path).expect("hashed above").clone();
        let path_key = metadata::absolute_path_key(path);
        let filename = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();

        let raw = match raw_metadata_by_path.get(&path_key) {
            Some(raw) => {
                if let Some(err_msg) = raw.get("Error").and_then(|v| v.as_str()) {
                    errors.push(ImportFileError {
                        path: path_key.clone(),
                        message: format!("exiftool: {err_msg}"),
                    });
                }
                raw.clone()
            }
            None => {
                errors.push(ImportFileError {
                    path: path_key.clone(),
                    message: "exiftool returned no metadata for this file".into(),
                });
                serde_json::json!({})
            }
        };

        let fields = metadata::extract_fields(&raw);

        let thumbnail_path =
            match thumbnails::get_or_create_thumbnail(&app, path, &hash, &cache_dir).await {
                Ok(p) => Some(p.to_string_lossy().into_owned()),
                Err(e) => {
                    errors.push(ImportFileError {
                        path: path_key.clone(),
                        message: format!("thumbnail extraction failed: {e}"),
                    });
                    None
                }
            };

        let source_file_id = Uuid::new_v4().to_string();

        // Default clip spans the whole file. When duration is unknown we
        // can't express "the whole clip" yet, so it's left as a zero-length
        // placeholder - `manualOverrides` or a later re-import can fill it in.
        let full_range = InOutRange {
            start_seconds: 0.0,
            end_seconds: fields.duration_seconds.unwrap_or(0.0),
        };

        source_files.push(SourceFile {
            id: source_file_id.clone(),
            path: path_key,
            hash,
            filename,
            duration_seconds: fields.duration_seconds,
            make: fields.make,
            model: fields.model,
            captured_at: fields.captured_at,
            gps: fields.gps,
            width: fields.width,
            height: fields.height,
            fps: fields.fps,
            thumbnail_path,
            raw_metadata: raw,
        });

        clips.push(Clip {
            id: Uuid::new_v4().to_string(),
            source_file_id,
            in_out_ranges: vec![full_range],
            include: true,
            device_nickname_id: None,
            manual_overrides: None,
        });
    }

    let now = chrono::Utc::now().to_rfc3339();
    let project = Project {
        schema_version: SCHEMA_VERSION,
        created_at: now.clone(),
        modified_at: now,
        source_files,
        clips,
    };

    Ok(ImportOutcome { project, errors })
}

fn thumbnail_cache_dir(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("failed to resolve app data directory: {e}"))?;
    Ok(dir.join("thumbnails"))
}
