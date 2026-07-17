"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import Sidebar from "@/components/Sidebar";
import ScannerWorkspace from "@/features/scanner/ScannerWorkspace";
import ScanResultsTable from "@/features/scans/ScanResultsTable";
import ScanLogsPanel from "@/features/scans/ScanLogsPanel";
import ScanRunHistory from "@/features/scans/ScanRunHistory";
import ScanLineageTrail from "@/features/scans/ScanLineageTrail";
import { useScanLineage } from "@/features/scans/useScanLineage";
import WatchlistWorkspace from "@/features/watchlists/WatchlistWorkspace";
import ScanPresetWorkspace from "@/features/scan-presets/ScanPresetWorkspace";
import type { ScanRunDetail, ScanResult, ScanError } from "@/features/scans/types";
import { getScanResults, getScanErrors } from "@/features/scans/api";
import { listWatchlists } from "@/features/watchlists/api";
import type { WatchlistSummary } from "@/features/watchlists/types";
import { listScanPresets } from "@/features/scan-presets/api";
import type { ScanPresetSummary } from "@/features/scan-presets/types";
import { reconcileSelectedId } from "@/lib/scanner-utils";
import { useThemeContext } from "@/lib/theme";

type Section = "Scanner" | "Results" | "Logs";
type DrawerView = "watchlists" | "presets" | null;

function loadResources(
  requestIdRef: React.MutableRefObject<number>,
  setWatchlists: (w: WatchlistSummary[]) => void,
  setWatchlistLoading: (l: boolean) => void,
  setWatchlistError: (e: string | null) => void,
  setPresets: (p: ScanPresetSummary[]) => void,
  setPresetsLoading: (l: boolean) => void,
  setPresetsError: (e: string | null) => void,
  setSelectedWatchlistId: React.Dispatch<React.SetStateAction<string>>,
  setSelectedPresetId: React.Dispatch<React.SetStateAction<string>>,
) {
  setWatchlistLoading(true);
  setPresetsLoading(true);
  setWatchlistError(null);
  setPresetsError(null);

  const requestId = ++requestIdRef.current;

  Promise.allSettled([listWatchlists(), listScanPresets()]).then(([wlResult, presetResult]) => {
    if (requestId !== requestIdRef.current) return;

    if (wlResult.status === "fulfilled") {
      setWatchlists(wlResult.value);
      setSelectedWatchlistId((prev) => reconcileSelectedId(prev, wlResult.value));
      setWatchlistLoading(false);
    } else {
      setWatchlistError("Watchlist 목록을 불러오지 못했습니다.");
      setWatchlistLoading(false);
    }

    if (presetResult.status === "fulfilled") {
      setPresets(presetResult.value);
      setSelectedPresetId((prev) => reconcileSelectedId(prev, presetResult.value));
      setPresetsLoading(false);
    } else {
      setPresetsError("Preset 목록을 불러오지 못했습니다.");
      setPresetsLoading(false);
    }
  });
}

export default function Home() {
  const { theme, setTheme } = useThemeContext();
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

  const [presets, setPresets] = useState<ScanPresetSummary[]>([]);
  const [presetsLoading, setPresetsLoading] = useState(true);
  const [presetsError, setPresetsError] = useState<string | null>(null);

  const [drawer, setDrawer] = useState<DrawerView>(null);

  const drawerRef = useRef<HTMLElement | null>(null);
  const previousFocusRef = useRef<HTMLElement | null>(null);
  const bodyOverflowRef = useRef("");
  const resourceRequestIdRef = useRef(0);
  const [resumeRunId, setResumeRunId] = useState<string | null>(null);

  // ── Callbacks ──
  const refreshScannerResources = useCallback(() => {
    loadResources(
      resourceRequestIdRef,
      setWatchlists,
      setWatchlistLoading,
      setWatchlistError,
      setPresets,
      setPresetsLoading,
      setPresetsError,
      setSelectedWatchlistId,
      setSelectedPresetId,
    );
  }, []);

  const closeDrawer = useCallback(() => {
    setDrawer(null);
    void refreshScannerResources();
  }, [refreshScannerResources]);

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
    setResumeRunId(retryRunId);
    setActive("Scanner");
  }, []);

  const loadAndSelectRun = useCallback(
    async (run: ScanRunDetail, destination: "Results" | "Logs") => {
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
    },
    [],
  );

  const handleResumeRunCompleted = useCallback(
    async (run: ScanRunDetail) => {
      setResumeRunId(null);
      await loadAndSelectRun(run, "Results");
      setActive("Results");
    },
    [loadAndSelectRun],
  );

  const handleWatchlistSelect = useCallback((id: string) => {
    setResumeRunId(null);
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

  const handlePresetChange = useCallback((id: string) => {
    setSelectedPresetId(id);
  }, []);

  // ── Resource loading (unmount-safe with requestIdRef) ──
  useEffect(() => {
    loadResources(
      resourceRequestIdRef,
      setWatchlists,
      setWatchlistLoading,
      setWatchlistError,
      setPresets,
      setPresetsLoading,
      setPresetsError,
      setSelectedWatchlistId,
      setSelectedPresetId,
    );

    return () => {
      resourceRequestIdRef.current += 1;
    };
  }, []);

  // ── Drawer focus trap lifecycle ──
  useEffect(() => {
    if (!drawer || !drawerRef.current) return;

    const container = drawerRef.current;

    previousFocusRef.current =
      document.activeElement instanceof HTMLElement
        ? document.activeElement
        : null;

    bodyOverflowRef.current = document.body.style.overflow;
    document.body.style.overflow = "hidden";

    const closeButton = container.querySelector<HTMLElement>(
      ".close-button",
    );
    closeButton?.focus();

    const focusableSelectors = [
      'a[href]',
      'button:not([disabled])',
      'input:not([disabled])',
      'select:not([disabled])',
      'textarea:not([disabled])',
      '[tabindex]:not([tabindex="-1"])',
    ].join(", ");

    function getFocusableElements(el: HTMLElement): HTMLElement[] {
      return Array.from(
        el.querySelectorAll<HTMLElement>(focusableSelectors),
      ).filter((e) => e.offsetParent !== null);
    }

    function handleKeyDown(event: KeyboardEvent) {
      if (event.key === "Escape") {
        event.preventDefault();
        closeDrawer();
        return;
      }

      if (event.key !== "Tab") return;

      const focusable = getFocusableElements(container);
      if (focusable.length === 0) {
        event.preventDefault();
        container.focus();
        return;
      }

      const first = focusable[0];
      const last = focusable[focusable.length - 1];

      if (event.shiftKey && document.activeElement === first) {
        event.preventDefault();
        last.focus();
      } else if (!event.shiftKey && document.activeElement === last) {
        event.preventDefault();
        first.focus();
      }
    }

    document.addEventListener("keydown", handleKeyDown);

    return () => {
      document.removeEventListener("keydown", handleKeyDown);
      document.body.style.overflow = bodyOverflowRef.current;
      previousFocusRef.current?.focus();
      previousFocusRef.current = null;
    };
  }, [drawer, closeDrawer]);

  const watchlistExists = watchlists.some(
    (w) => w.id === selectedWatchlistId,
  );
  const presetExists = presets.some(
    (p) => p.id === selectedPresetId,
  );

  const drawerTitle: Record<NonNullable<DrawerView>, string> = {
    watchlists: "Watchlists 관리",
    presets: "Scan Presets 관리",
  };

  const drawerId = "drawer-title";

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
        theme={theme}
        onThemeChange={setTheme}
      />

      <section className="workspace">
        <header className="workspace-header">
          <div className="workspace-header-left">
            <h2>{active === "Scanner" ? "Scanner" : active}</h2>
            {active === "Scanner" && selectedWatchlistId && watchlistExists && (
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
              selectedPresetId={selectedPresetId}
              onPresetIdChange={handlePresetChange}
              onOpenPresetDrawer={handleOpenPresetDrawer}
              watchlists={watchlists}
              presets={presets}
              watchlistExists={watchlistExists}
              presetExists={presetExists}
              presetsLoading={presetsLoading}
              presetsError={presetsError}
              resumeRunId={resumeRunId}
              onResumeRunCompleted={handleResumeRunCompleted}
            />
          )
        ) : active === "Results" ? (
          selectedRun ? (
            <ScanResultsTable
              results={results}
              runId={selectedRun.id}
              isLoading={isLoadingResults}
              run={selectedRun}
            />
          ) : (
            <ScanRunHistory onRunSelect={handleRunSelect} />
          )
        ) : active === "Logs" ? (
          selectedRun ? (
            <ScanLogsPanel
              runId={selectedRun.id}
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
            ref={drawerRef}
            className="drawer"
            role="dialog"
            aria-modal="true"
            aria-labelledby={drawerId}
          >
            <header className="drawer-header">
              <div>
                <h3 id={drawerId}>{drawerTitle[drawer]}</h3>
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
  const { runs: lineageRuns } = useScanLineage(run);

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
            color: "var(--color-text-tertiary)",
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
      {lineageRuns.length > 0 && (
        <ScanLineageTrail
          runs={lineageRuns}
          currentRunId={run.id}
        />
      )}
    </div>
  );
}
