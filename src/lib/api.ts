// Thin typed wrappers around the Tauri commands. The frontend never touches
// the filesystem, exiftool, or ffmpeg directly — everything goes through Rust.
import { invoke } from "@tauri-apps/api/core";
import type { ImportOutcome, Project } from "../types/project";

export function importFolder(folderPath: string): Promise<ImportOutcome> {
  return invoke("import_folder", { folderPath });
}

export function saveProject(project: Project, path: string): Promise<void> {
  return invoke("save_project", { project, path });
}

export function loadProject(path: string): Promise<Project> {
  return invoke("load_project", { path });
}

export function getSupportedExtensions(): Promise<string[]> {
  return invoke("get_supported_extensions");
}
