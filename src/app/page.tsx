"use client";

import { useCallback, useState } from "react";

import ScannerWorkspace from "@/features/scanner/ScannerWorkspace";
import ScanResultsTable from "@/features/scans/ScanResultsTable";
import ScanLogsPanel from "@/features/scans/ScanLogsPanel";
import ScanRunHistory from "@/features/scans/ScanRunHistory";
import type { ScanRunDetail, ScanResult, ScanError } from "@/features/scans/types";
import { getScanResults, getScanErrors } from "@/features/scans/api";

const sections = ["Scanner", "Results", "Logs"] as const;
type Section = (typeof sections)[number];

export default function Home() {
  const [active, setActive] = useState<Section>("Scanner");
  const [selectedRun, setSelectedRun] = useState<ScanRunDetail | null>(null);
  const [results, setResults] = useState<ScanResult[]>([]);
  const [errors, setErrors] = useState<ScanError[]>([]);
  const [isLoadingResults, setIsLoadingResults] = useState(false);

  const handleRunSelect = useCallback(async (run: ScanRunDetail) => {
    setSelectedRun(run);
    setIsLoadingResults(true);
    try {
      const [res, err] = await Promise.all([
        getScanResults(run.id),
        getScanErrors(run.id),
      ]);
      setResults(res);
      setErrors(err);
    } catch {
      setResults([]);
      setErrors([]);
    } finally {
      setIsLoadingResults(false);
    }
  }, []);

  const handleRetry = useCallback(async (retryRunId: string) => {
    setSelectedRun(null);
    setResults([]);
    setErrors([]);
    setActive("Scanner");
  }, []);

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div>
          <p className="eyebrow">macOS daily scanner</p>
          <h1>Stock</h1>
        </div>
        <nav aria-label="Primary navigation">
          {sections.map((section) => (
            <button
              key={section}
              className={active === section ? "nav-button active" : "nav-button"}
              onClick={() => setActive(section)}
            >
              {section}
            </button>
          ))}
        </nav>
      </aside>

      <section className="workspace">
        <header className="workspace-header">
          <div>
            <p className="eyebrow">Personal stock scanner</p>
            <h2>{active}</h2>
          </div>
          {active === "Scanner" && selectedRun && (
            <button
              className="secondary-button"
              type="button"
              onClick={() => {
                setSelectedRun(null);
                setResults([]);
                setErrors([]);
              }}
            >
              New Run
            </button>
          )}
        </header>

        {active === "Scanner" ? (
          selectedRun ? (
            <div style={{ display: "grid", gap: "16px" }}>
              <div className="panel" style={{ padding: "16px" }}>
                <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "8px" }}>
                  <h3 style={{ margin: 0, fontSize: "14px" }}>Run Detail</h3>
                  <span
                    style={{
                      fontSize: "12px",
                      padding: "3px 8px",
                      borderRadius: "999px",
                      background: "#0e1219",
                      color: "#c9d4e7",
                    }}
                  >
                    {selectedRun.status}
                  </span>
                </div>
                <div className="form-meta" style={{ fontSize: "12px", color: "#8f98aa" }}>
                  <span>ID: {selectedRun.id}</span>
                  <span>Total: {selectedRun.totalSymbols}</span>
                  <span>Succeeded: {selectedRun.succeededSymbols}</span>
                  <span>Failed: {selectedRun.failedSymbols}</span>
                </div>
              </div>
              <ScanResultsTable
                results={results}
                runId={selectedRun.id}
                isLoading={isLoadingResults}
              />
            </div>
          ) : (
            <ScannerWorkspace />
          )
        ) : active === "Results" ? (
          selectedRun ? (
            <ScanResultsTable
              results={results}
              runId={selectedRun.id}
              isLoading={isLoadingResults}
            />
          ) : (
            <div style={{ display: "grid", gap: "16px" }}>
              <ScanRunHistory onRunSelect={handleRunSelect} />
            </div>
          )
        ) : active === "Logs" ? (
          selectedRun ? (
            <ScanLogsPanel
              runId={selectedRun.id}
              presetId={selectedRun.presetId}
              watchlistId={selectedRun.watchlistId}
              onRetry={handleRetry}
            />
          ) : (
            <div style={{ display: "grid", gap: "16px" }}>
              <ScanRunHistory onRunSelect={handleRunSelect} />
            </div>
          )
        ) : (
          <div className="empty-state">
            <h3>{active} module</h3>
            <p>이 영역은 다음 milestone에서 SQLite 기반 도메인 기능과 연결됩니다.</p>
          </div>
        )}
      </section>
    </main>
  );
}
