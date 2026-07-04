import { useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { convertFileSrc } from "@tauri-apps/api/core";
import { importFolder } from "./lib/api";
import { formatCapturedAt, formatDuration } from "./lib/format";
import type { ImportFileError, SourceFile } from "./types/project";

export default function App() {
  const [folderPath, setFolderPath] = useState<string | null>(null);
  const [sourceFiles, setSourceFiles] = useState<SourceFile[]>([]);
  const [importErrors, setImportErrors] = useState<ImportFileError[]>([]);
  const [importing, setImporting] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  const totalDuration = useMemo(
    () =>
      sourceFiles.reduce((sum, f) => sum + (f.durationSeconds ?? 0), 0),
    [sourceFiles],
  );

  async function handlePickFolder() {
    const selected = await open({ directory: true, multiple: false });
    if (typeof selected !== "string") return; // cancelled
    setFolderPath(selected);
  }

  async function handleImport() {
    if (!folderPath) return;
    setImporting(true);
    setFailure(null);
    try {
      const outcome = await importFolder(folderPath);
      setSourceFiles(outcome.project.sourceFiles);
      setImportErrors(outcome.errors);
    } catch (err) {
      setFailure(String(err));
    } finally {
      setImporting(false);
    }
  }

  return (
    <main style={{ padding: "1.5rem", fontFamily: "sans-serif" }}>
      <h1>Vacation Video Importer</h1>

      <div style={{ display: "flex", gap: "0.5rem", alignItems: "center" }}>
        <button onClick={handlePickFolder}>Choose folder…</button>
        <span>{folderPath ?? "No folder selected"}</span>
        <button onClick={handleImport} disabled={!folderPath || importing}>
          {importing ? "Importing…" : "Import"}
        </button>
      </div>

      {failure && <p style={{ color: "red" }}>Import failed: {failure}</p>}

      {sourceFiles.length > 0 && (
        <>
          <p>
            <strong>{sourceFiles.length}</strong> file(s), total duration{" "}
            <strong>{formatDuration(totalDuration)}</strong>
          </p>

          {importErrors.length > 0 && (
            <details style={{ marginBottom: "1rem" }}>
              <summary style={{ color: "#b45309", cursor: "pointer" }}>
                {importErrors.length} file(s) had problems during import
              </summary>
              <ul>
                {importErrors.map((e) => (
                  <li key={e.path}>
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
                      <img
                        src={convertFileSrc(f.thumbnailPath)}
                        alt=""
                        width={120}
                      />
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

const cellStyle: React.CSSProperties = {
  border: "1px solid #ccc",
  padding: "0.4rem",
  textAlign: "left",
  verticalAlign: "middle",
};
