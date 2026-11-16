# Sidecar binaries

This directory holds the `exiftool` and `ffmpeg` executables that Tauri
bundles as sidecars (see `tauri.conf.json`'s `bundle.externalBin` and
`capabilities/default.json`). They are **not** committed to the repo -
download them per platform and place them here with the target-triple
suffix Tauri expects.

See the top-level `README.md` for exact download links and filenames.

Expected layout (Windows shown; other platforms have no `exiftool_files`):

```
src-tauri/binaries/
  exiftool-x86_64-pc-windows-msvc.exe   <- renamed exiftool(-k).exe
  exiftool_files/                       <- copied as-is from the ExifTool zip
  ffmpeg-x86_64-pc-windows-msvc.exe
```
