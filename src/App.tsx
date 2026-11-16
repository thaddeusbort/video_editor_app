import { useMemo, useState } from "react";
import { open, save } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { importFolder, loadProject, saveProject } from "./lib/api";
import { formatCapturedAt, formatDuration } from "./lib/format";
import {
  IMPORT_PROGRESS_EVENT,
  type ImportFileError,
  type ImportPhase,
  type ImportProgress,
  type Project,
} from "./types/project";

const PROJECT_FILTERS = [{ name: "Video project", extensions: ["json"] }];

const PHASE_LABELS: Record<ImportPhase, string> = {
  scanning: "Scanning folder",
  hashing: "Fingerprinting files",
  metadata: "Reading metadata",
  thumbnails: "Extracting thumbnails",
};
const PHASE_ORDER: ImportPhase[] = ["scanning", "hashing", "metadata", "thumbnails"];

type Busy = "importing" | "saving" | "loading" | null;

export default function App() {
  const [folderPath, setFolderPath] = useState<string | null>(null);
  const [project, setProject] = useState<Project | null>(null);
  const [projectPath, setProjectPath] = useState<string | null>(null);
  const [importErrors, setImportErrors] = useState<ImportFileError[]>([]);
  const [progress, setProgress] = useState<ImportProgress | null>(null);
  const [busy, setBusy] = useState<Busy>(null);
  const [failure, setFailure] = useState<string | null>(null);

  const sourceFiles = project?.sourceFiles ?? [];
  const totalDuration = useMemo(
    () => sourceFiles.reduce((sum, f) => sum + (f.durationSeconds ?? 0), 0),
    [sourceFiles],
  );

  async function handlePickFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") return; // cancelled
    setFolderPath(selected);
  }

  async function handleImport() {
    if (!folderPath) return;
    setBusy("importing");
    setFailure(null);
    setProgress(null);
    // Subscribe before invoking so no early progress events are missed.
    const unlisten = await listen<ImportProgress>(IMPORT_PROGRESS_EVENT, (e) =>
      setProgress(e.payload),
    );
    try {
      const outcome = await importFolder(folderPath);
      setProject(outcome.project);
      setImportErrors(outcome.errors);
      setProjectPath(null); // a fresh import hasn't been saved anywhere yet
    } catch (err) {
      setFailure(`Import failed: ${err}`);
    } finally {
      unlisten();
      setProgress(null);
      setBusy(null);
    }
  }

  async function handleSave(saveAs: boolean) {
    if (!project) return;
    let path = projectPath;
    if (saveAs || !path) {
      path = await save({ filters: PROJECT_FILTERS, defaultPath: "project.json" });
      if (!path) return; // cancelled
    }
    setBusy("saving");
    setFailure(null);
    try {
      setProject(await saveProject(project, path));
      setProjectPath(path);
    } catch (err) {
      setFailure(`Save failed: ${err}`);
    } finally {
      setBusy(null);
    }
  }

  async function handleOpen() {
    const selected = await open({ multiple: false, directory: false, filters: PROJECT_FILTERS });
    if (typeof selected !== "string") return; // cancelled
    setBusy("loading");
    setFailure(null);
    try {
      setProject(await loadProject(selected));
      setProjectPath(selected);
      setImportErrors([]); // import errors belong to an import run, not the file
    } catch (err) {
      setFailure(`Open failed: ${err}`);
    } finally {
      setBusy(null);
    }
  }

  return (
    <main style={{ padding: "1.5rem", fontFamily: "sans-serif" }}>
      <h1>Vacation Video Importer</h1>

      <div style={rowStyle}>
        <button onClick={handleOpen} disabled={busy !== null}>
          Open project…
        </button>
        <button onClick={() => handleSave(false)} disabled={!project || busy !== null}>
          {busy === "saving" ? "Saving…" : "Save"}
        </button>
        <button onClick={() => handleSave(true)} disabled={!project || busy !== null}>
          Save as…
        </button>
        <span>{projectPath ?? (project ? "Not saved yet" : "No project")}</span>
      </div>

      <div style={rowStyle}>
        <button onClick={handlePickFolder} disabled={busy !== null}>
          Choose folder…
        </button>
        <span>{folderPath ?? "No folder selected"}</span>
        <button onClick={handleImport} disabled={!folderPath || busy !== null}>
          {busy === "importing" ? "Importing…" : "Import"}
        </button>
      </div>

      {busy === "importing" && <ImportProgressView progress={progress} />}

      {failure && <p style={{ color: "red" }}>{failure}</p>}

      {project && (
        <>
          <p>
            <strong>{sourceFiles.length}</strong> file(s), total duration{" "}
            <strong>{formatDuration(totalDuration)}</strong>
          </p>

          {importErrors.length > 0 && (
            <details style={{ marginBottom: "1rem" }}>
              <summary style={{ color: "#b45309", cursor: "pointer" }}>
                {importErrors.length} problem(s) during import
              </summary>
              <ul>
                {importErrors.map((e, i) => (
                  <li key={i}>
                    {e.path}: {e.message}
                  </li>
                ))}
              </ul>
            </details>
          )}

          <table style={{ borderCollapse: "collapse", width: "100%" }}>
            <thead>
              <tr>
                <th style={cellStyle}>Thumbnail</th>
                <th style={cellStyle}>Filename</th>
                <th style={cellStyle}>Captured</th>
                <th style={cellStyle}>Device</th>
                <th style={cellStyle}>Duration</th>
                <th style={cellStyle}>GPS</th>
              </tr>
            </thead>
            <tbody>
              {sourceFiles.map((f) => (
                <tr key={f.id}>
                  <td style={cellStyle}>
                    {f.thumbnailPath ? (
                      <img src={convertFileSrc(f.thumbnailPath)} alt="" width={120} />
                    ) : (
                      "—"
                    )}
                  </td>
                  <td style={cellStyle}>{f.filename}</td>
                  <td style={cellStyle}>{formatCapturedAt(f.capturedAt)}</td>
                  <td style={cellStyle}>
                    {[f.make, f.model].filter(Boolean).join(" ") || "—"}
                  </td>
                  <td style={cellStyle}>{formatDuration(f.durationSeconds)}</td>
                  <td style={cellStyle}>{f.gps ? "✓" : "—"}</td>
                </tr>
              ))}
            </tbody>
          </table>
        </>
      )}
    </main>
  );
}

function ImportProgressView({ progress }: { progress: ImportProgress | null }) {
  const phase = progress?.phase ?? "scanning";
  const step = PHASE_ORDER.indexOf(phase) + 1;
  // No total yet (scanning) -> omit `value` so the bar shows as indeterminate.
  const known = progress !== null && progress.total > 0;
  return (
    <div style={{ margin: "1rem 0" }}>
      <div>
        Step {step} of {PHASE_ORDER.length}: {PHASE_LABELS[phase]}
        {known && ` (${progress.current} of ${progress.total})`}…
      </div>
      {known ? (
        <progress value={progress.current} max={progress.total} style={{ width: "100%" }} />
      ) : (
        <progress style={{ width: "100%" }} />
      )}
    </div>
  );
}

const rowStyle: React.CSSProperties = {
  display: "flex",
  gap: "0.5rem",
  alignItems: "center",
  marginBottom: "0.5rem",
};

const cellStyle: React.CSSProperties = {
  border: "1px solid #ccc",
  padding: "0.4rem",
  textAlign: "left",
  verticalAlign: "middle",
};
