# Vacation Video Importer

A Tauri v2 + React + TypeScript desktop app for culling and organizing
vacation videos. Currently at **step 1 (import & metadata)**: given a
folder, recursively find video files, extract metadata with ExifTool, grab
a poster-frame thumbnail with FFmpeg, and save/open the result as a
versioned project JSON. Culling, trimming and export come later - see
[Roadmap](#roadmap).

All filesystem access and process spawning happens in Rust (`src-tauri/src`).
The React frontend (`src/`) only calls Tauri commands and renders results.

## Prerequisites

- Rust (stable) + `cargo`
- Node.js + npm
- Tauri v2 system dependencies for your OS - see
  https://v2.tauri.app/start/prerequisites/ (on Linux this means
  `libwebkit2gtk-4.1-dev`, `libgtk-3-dev`, `librsvg2-dev`,
  `libayatana-appindicator3-dev`, `libssl-dev`, `libxdo-dev` or your
  distro's equivalents)

## Sidecar binaries (required before running)

ExifTool and FFmpeg are bundled as Tauri **sidecars** - the app spawns them
as subprocesses via `tauri-plugin-shell`, and never assumes they're on the
system `PATH`. They are not committed to this repo (binaries are
platform-specific and often large); you need to download them yourself and
place them in `src-tauri/binaries/`.

Tauri requires each sidecar to be named `<name>-<target-triple>`. Find your
target triple with:

```sh
rustc --print host-tuple
```

Common triples: `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc`,
`x86_64-apple-darwin`, `aarch64-apple-darwin`.

Place these two files in `src-tauri/binaries/` (see
`src-tauri/binaries/README.md`):

| Platform | `exiftool-<triple>` | `ffmpeg-<triple>` |
| --- | --- | --- |
| Windows | `exiftool-x86_64-pc-windows-msvc.exe` + `exiftool_files\` folder | `ffmpeg-x86_64-pc-windows-msvc.exe` |
| macOS (Intel) | `exiftool-x86_64-apple-darwin` | `ffmpeg-x86_64-apple-darwin` |
| macOS (Apple Silicon) | `exiftool-aarch64-apple-darwin` | `ffmpeg-aarch64-apple-darwin` |
| Linux | `exiftool-x86_64-unknown-linux-gnu` | `ffmpeg-x86_64-unknown-linux-gnu` |

Where to get them:

- **ExifTool**: https://exiftool.org
  - Windows: download the Windows zip. Current releases contain
    two things, and you need **both**:
    1. `exiftool(-k).exe` - rename it to `exiftool-x86_64-pc-windows-msvc.exe`
       (it's only a small launcher).
    2. the `exiftool_files\` folder (bundled Perl runtime + ExifTool
       libraries) - copy the whole folder, unrenamed, to
       `src-tauri\binaries\exiftool_files\`.

    The launcher looks for `exiftool_files\` next to wherever it runs from.
    `src-tauri/tauri.windows.conf.json` bundles that folder as a resource,
    which places it next to `exiftool.exe` both in `target\debug` /
    `target\release` during development and in the installed app. Without
    it, imports fail with "exiftool failed to start ... Could not find
    ...\exiftool_files\perl5*.dll".
  - macOS/Linux: ExifTool ships as a Perl script (`exiftool`) plus a `lib/`
    directory of Perl modules - it is **not** a single self-contained
    binary on these platforms. The simplest reliable option is to install
    ExifTool via your package manager (`brew install exiftool` /
    `apt install libimage-exiftool-perl`) and copy the resulting `exiftool`
    script to `src-tauri/binaries/exiftool-<triple>` - this works as long
    as `perl` is available on the machine running the app (true for
    essentially all Mac/Linux systems). For a fully self-contained bundle
    you'd need a packaged/compiled ExifTool build instead; that's out of
    scope for this step.
  - Make the file executable: `chmod +x src-tauri/binaries/exiftool-<triple>`.
- **FFmpeg**: static builds are available from
  https://ffmpeg.org/download.html (e.g. johnvansickle.com builds for
  Linux, evermeet.cx for macOS, gyan.dev for Windows). Grab the `ffmpeg`
  executable (not `ffprobe`/`ffplay`) and place/rename it as above, then
  `chmod +x` on macOS/Linux.

`tauri.conf.json`'s `bundle.externalBin` and
`src-tauri/capabilities/default.json`'s `shell:allow-execute` permission
are already configured to allow exactly these two sidecars - no further
config is needed once the binaries are in place.

## Setup

```sh
npm install
```

Then place the sidecar binaries as described above.

## Running

```sh
npm run tauri dev
```

## Verifying the sidecar/import pipeline without the UI

Two example binaries exercise the backend directly against a real Tauri
runtime (useful for confirming your sidecar binaries are wired up
correctly without clicking through the UI):

```sh
# Runs a real import against a folder and prints the resulting ImportOutcome JSON
IMPORT_SMOKE_TEST_DIR=/path/to/some/videos cargo run --example import_smoke_test --manifest-path src-tauri/Cargo.toml

# Round-trips save_project/load_project; checks modifiedAt stamping and schema-version rejection
PROJECT_JSON_PATH=/path/to/project.json cargo run --example roundtrip_smoke_test --manifest-path src-tauri/Cargo.toml
```

## Architecture

- **Data model**: `src/types/project.ts` (TypeScript) and
  `src-tauri/src/models.rs` (Rust) define the same versioned schema
  (`schemaVersion: 1`, centralized as a constant in both files). Keep them
  in sync by hand when the schema changes.
- **Tag mapping**: `src-tauri/src/metadata.rs` is the one place that maps
  ExifTool tags to `SourceFile` fields, with the fallback order documented
  inline. The full ExifTool JSON blob is preserved per file in
  `rawMetadata` so unmapped tags (DJI maker notes, XMP, etc.) aren't lost.
- **Hashing**: `src-tauri/src/hashing.rs` uses a partial hash (file size +
  first/last 64 KB), not a full-file SHA - see the comment there for why.
- **Thumbnails**: `src-tauri/src/thumbnails.rs` grabs a poster frame ~2s in
  (falling back to frame 0 for very short clips) and caches it in the app
  data directory, keyed by content hash.
- **Commands**: `src-tauri/src/commands.rs` exposes `import_folder`,
  `save_project`, `load_project`, and `get_supported_extensions`
  (extension list centralized in `src-tauri/src/extensions.rs`). A bad file
  never aborts the whole import - problems are collected into
  `ImportOutcome.errors` and shown in the UI.
- **Import progress**: `import_folder` emits an `import-progress` event
  (`ImportProgress` in both type files) as it moves through scanning,
  fingerprinting, metadata and thumbnails; the UI shows it as a step label
  and progress bar.
- **Saving**: `save_project` stamps `modifiedAt` and writes via a temp file
  plus rename, so an interrupted save can't leave a half-written project.
  `load_project` rejects files whose `schemaVersion` it doesn't support.

## Roadmap

A proposed plan, not a commitment - items marked **(decide)** need a call
before work starts. The data model already reserves fields for steps 2-3
(`include`, `deviceNicknameId`, `manualOverrides`, `inOutRanges`).

### Step 1 - Import & metadata (done)

- [x] Recursive folder scan with an editable extension list
- [x] Batch ExifTool metadata extraction with a central tag mapping;
      full raw metadata kept per file
- [x] Cached poster-frame thumbnails via FFmpeg, keyed by content hash
- [x] Versioned project file (schema v1); save, save as, open
- [x] Per-file error reporting; import progress indicator
- [x] ExifTool and FFmpeg bundled as sidecars (incl. Windows `exiftool_files`)

### Step 2 - Culling & organizing

- [ ] Clip grid sorted by capture time and grouped by day, replacing the table
- [ ] Keep/reject (`include`) with keyboard shortcuts for fast culling
- [ ] Clip preview player. Needs the asset-protocol scope widened from the
      thumbnail cache to source video folders **(decide: scope approach)**
- [ ] Device nicknames ("Drone", "Sam's phone"): a device list on `Project`
      referenced by `Clip.deviceNicknameId`. First schema bump to v2,
      with `load_project` migrating v1 files instead of rejecting them
- [ ] Manual date/device overrides, including "shift every clip from this
      device by N hours" for cameras with a wrong clock
- [ ] Unsaved-changes tracking and a prompt before an import/open replaces
      the current project

### Step 3 - Trimming

- [ ] Set in/out points in the player; several ranges per clip
- [ ] Smooth scrubbing for 4K/HEVC/drone footage, likely via low-res proxy
      files generated in the background **(decide: proxies or not)**

### Step 4 - Export

- [ ] Render the included ranges in capture order with FFmpeg: stream copy
      when codecs match, re-encode otherwise, with progress events
- [ ] Output shape **(decide)**: one movie, one file per day, and/or an
      edit list for another editor

### Cross-cutting / known issues

- Capture times are shown converted to the viewer's timezone; they should
  show the local time as recorded on the trip.
- Add a folder to an existing project, or re-import, matching files by hash
  so culling decisions survive and moved files are found again.
- Thumbnails live in a per-machine cache, so a project opened on another
  computer shows none; regenerate missing ones on open.
- Import fingerprints and thumbnails files one at a time; parallelize
  thumbnail extraction and add a cancel button for large folders.
- macOS/Linux still rely on the system's Perl for ExifTool; bundle a
  self-contained build before distributing.
- CSP is disabled (`security.csp: null`); tighten before shipping.
- TS and Rust types are synced by hand; consider generating the TS side
  from Rust (e.g. ts-rs or specta) **(decide: worth the dependency?)**
