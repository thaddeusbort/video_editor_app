// Data model for the project file. Kept in sync by hand with the Rust structs
// in `src-tauri/src/models.rs` — field names/shapes must match exactly since
// they cross the IPC boundary as plain JSON (serde uses camelCase on the wire).

/**
 * Bump this whenever the shape of `Project` (or anything nested in it)
 * changes in a way that isn't purely additive-and-optional. This is the
 * single place both sides read the "current" version from; `load_project`
 * is responsible for rejecting/migrating files written by older versions.
 */
export const SCHEMA_VERSION = 1;

export interface GpsCoord {
  lat: number;
  lon: number;
}

export interface InOutRange {
  startSeconds: number;
  endSeconds: number;
}

export interface ManualOverrides {
  capturedAt: string | null;
  make: string | null;
  model: string | null;
}

export interface SourceFile {
  id: string;
  path: string;
  /** Partial content hash — see hashing.rs for why this isn't a full SHA. */
  hash: string;
  filename: string;
  durationSeconds: number | null;
  make: string | null;
  model: string | null;
  /** ISO 8601, with timezone offset when ExifTool provided one. */
  capturedAt: string | null;
  gps: GpsCoord | null;
  width: number | null;
  height: number | null;
  fps: number | null;
  /** Absolute path into the thumbnail cache; null if extraction failed. */
  thumbnailPath: string | null;
  /** Full ExifTool JSON blob for this file, untouched, for tags we haven't mapped yet. */
  rawMetadata: unknown;
}

export interface Clip {
  id: string;
  sourceFileId: string;
  inOutRanges: InOutRange[];
  include: boolean;
  deviceNicknameId: string | null;
  manualOverrides: ManualOverrides | null;
}

export interface Project {
  schemaVersion: number;
  createdAt: string;
  modifiedAt: string;
  sourceFiles: SourceFile[];
  clips: Clip[];
}

/** Per-file diagnostic from an import run. Not persisted in the project file. */
export interface ImportFileError {
  path: string;
  message: string;
}

/** Return type of the `import_folder` command. */
export interface ImportOutcome {
  project: Project;
  errors: ImportFileError[];
}
