"use client";

import { useCallback, useEffect, useState, useCallback as useCb } from "react";

import Sidebar from "@/components/Sidebar";
import ScannerWorkspace from "@/features/scanner/ScannerWorkspace";
import ScanResultsTable from "@/features/scans/ScanResultsTable";
import ScanLogsPanel from "@/features/scans/ScanLogsPanel";
import ScanRunHistory from "@/features/scans/ScanRunHistory";
import WatchlistWorkspace from "@/features/watchlists/WatchlistWorkspace";
import ScanPresetWorkspace from "@/features/scan-presets/ScanPresetWorkspace";
import type { ScanRunDetail, ScanResult, ScanError } from "@/features/scans/types";
import { getScanResults, getScanErrors } from "@/features/scans/api";
import { listWatchlists } from "@/features/watchlists/api";
import type { WatchlistSummary } from "@/features/watchlists/types";

type Section = "Scanner" | "Results" | "Logs";
type DrawerView = "watchlists" | "presets" | null;

export default function Home() {
  const [active, setActive] = useState<Section>("Scanner");
  const [selectedRun, setSelectedRun] = useState<ScanRunDetail | null>(null);
  const [results, setResults] = useState<ScanResult[]>([]);
  const [errors, setErrors] = useState<ScanError[]>([]);
  const [isLoadingResults, setIsLoadingResults] = useState(false);

  const [selectedWatchlistId, setSelectedWatchlistId] = useState("");
  const [selectedPresetId, setSelectedPresetId] = useState("");
  const [watchlists, setWatchlists] = useState<WatchlistSummary[]>([]);
  const [watchlistLoading, setWatchlistLoading] = useState(true);
  const [watchlistError, setWatchlistError] = useState<string | null>(null);

  const [drawer, setDrawer] = useState<DrawerView>(null);

  /* eslint-disable react-hooks/set-state-in-effect */
  useEffect(() => {
    setWatchlistLoading(true);
    setWatchlistError(null);
    let cancelled = false;
    listWatchlists()
      .then((data) => {
        if (!cancelled) setWatchlists(data);
      })
      .catch(() => {
        if (!cancelled) setWatchlistError("Watchlist 목록을 불러오지 못했습니다.");
      })
      .finally(() => {
        if (!cancelled) setWatchlistLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, []);
  /* eslint-enable react-hooks/set-state-in-effect */

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

  const handleWatchlistSelect = useCallback((id: string) => {
    setSelectedWatchlistId(id);
    setSelectedPresetId("");
    if (active !== "Scanner") {
      setActive("Scanner");
    }
  }, [active]);

  const handleOpenWatchlistDrawer = useCallback(() => {
    setDrawer("watchlists");
  }, []);

  const handleOpenPresetDrawer = useCallback(() => {
    setDrawer("presets");
  }, []);

  const closeDrawer = useCallback(() => {
    setDrawer(null);
    let cancelled = false;
    listWatchlists()
      .then((data) => {
        if (!cancelled) setWatchlists(data);
      })
      .catch(() => {
        if (!cancelled) setWatchlistError("Watchlist 목록을 불러오지 못했습니다.");
      });
  }, []);

  const handlePresetChange = useCallback((id: string) => {
    setSelectedPresetId(id);
  }, []);

  const drawerTitle: Record<NonNullable<DrawerView>, string> = {
    watchlists: "Watchlists 관리",
    presets: "Scan Presets 관리",
  };

  return (
    <main className="app-shell">
      <Sidebar
        activeSection={active}
        onSectionChange={setActive}
        watchlists={watchlists}
        selectedWatchlistId={selectedWatchlistId}
        onWatchlistSelect={handleWatchlistSelect}
        onOpenWatchlistDrawer={handleOpenWatchlistDrawer}
        onOpenPresetDrawer={handleOpenPresetDrawer}
        watchlistLoading={watchlistLoading}
        watchlistError={watchlistError}
      />

      <section className="workspace">
        <header className="workspace-header">
          <div className="workspace-header-left">
            <h2>{active === "Scanner" ? "Scanner" : active}</h2>
            {active === "Scanner" && selectedWatchlistId && (
              <p className="workspace-context">
                {watchlists.find((w) => w.id === selectedWatchlistId)?.name || selectedWatchlistId}
              </p>
            )}
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
              <RunDetailBanner run={selectedRun} />
              <ScanResultsTable
                results={results}
                runId={selectedRun.id}
                isLoading={isLoadingResults}
              />
            </div>
          ) : (
            <ScannerWorkspace
              selectedWatchlistId={selectedWatchlistId}
              onWatchlistIdChange={setSelectedWatchlistId}
              selectedPresetId={selectedPresetId}
              onPresetIdChange={handlePresetChange}
              onOpenWatchlistDrawer={handleOpenWatchlistDrawer}
              onOpenPresetDrawer={handleOpenPresetDrawer}
              watchlists={watchlists}
            />
          )
        ) : active === "Results" ? (
          selectedRun ? (
            <ScanResultsTable
              results={results}
              runId={selectedRun.id}
              isLoading={isLoadingResults}
            />
          ) : (
            <ScanRunHistory onRunSelect={handleRunSelect} />
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
            <ScanRunHistory onRunSelect={handleRunSelect} />
          )
        ) : (
          <div className="empty-state">
            <h3>{active} module</h3>
            <p>이 영역은 다음 milestone에서 SQLite 기반 도메인 기능과 연결됩니다.</p>
          </div>
        )}
      </section>

      {drawer ? (
        <div
          className="backdrop"
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) {
              closeDrawer();
            }
          }}
        >
          <aside
            className="drawer"
            role="dialog"
            aria-modal="true"
            aria-labelledby="drawer-title"
          >
            <header className="drawer-header">
              <div>
                <h3 id="drawer-title">{drawerTitle[drawer]}</h3>
              </div>
              <button
                className="close-button"
                type="button"
                onClick={closeDrawer}
                aria-label="관리 Drawer 닫기"
              >
                &times;
              </button>
            </header>

            {drawer === "watchlists" ? (
              <WatchlistWorkspace />
            ) : (
              <ScanPresetWorkspace />
            )}
          </aside>
        </div>
      ) : null}
    </main>
  );
}

function RunDetailBanner({ run }: { run: ScanRunDetail }) {
  return (
    <div className="panel" style={{ padding: "14px 20px" }}>
      <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "6px", alignItems: "center" }}>
        <h3 style={{ margin: 0, fontSize: "14px", fontWeight: 600 }}>Run Detail</h3>
        <span
          style={{
            fontSize: "11px",
            padding: "2px 8px",
            borderRadius: "999px",
            background: "var(--color-surface-raised)",
            color: "var(--color-text-secondary)",
            fontWeight: 500,
          }}
        >
          {run.status}
        </span>
      </div>
      <div className="form-meta" style={{ fontSize: "12px", color: "var(--color-text-tertiary)", marginTop: "4px" }}>
        <span>ID: {run.id}</span>
        <span>Total: {run.totalSymbols}</span>
        <span>Succeeded: {run.succeededSymbols}</span>
        <span>Failed: {run.failedSymbols}</span>
      </div>
    </div>
  );
}
