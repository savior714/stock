"use client";

import { useCallback, useEffect, useRef, useState } from "react";

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
import { listScanPresets } from "@/features/scan-presets/api";
import type { ScanPresetSummary } from "@/features/scan-presets/types";
import { reconcileSelectedId } from "@/lib/scanner-utils";
import { ThemeContext } from "@/lib/theme";
import type { ThemeMode } from "@/lib/theme";

type Section = "Scanner" | "Results" | "Logs";
type DrawerView = "watchlists" | "presets" | null;

function useFocusTrap(containerRef: React.RefObject<HTMLElement | null>) {
  useEffect(() => {
    const container = containerRef.current;
    if (!container) return;

    const focusableSelectors = [
      'a[href]',
      'button:not([disabled])',
      'input:not([disabled])',
      'select:not([disabled])',
      'textarea:not([disabled])',
      '[tabindex]:not([tabindex="-1"])',
    ].join(", ");

    const focusableElements = Array.from(
      container.querySelectorAll<HTMLElement>(focusableSelectors),
    ).filter((el) => el.offsetParent !== null);

    if (focusableElements.length === 0) return;

    const firstElement = focusableElements[0];
    const lastElement = focusableElements[focusableElements.length - 1];

    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key !== "Tab") return;

      if (e.shiftKey) {
        if (document.activeElement === firstElement) {
          e.preventDefault();
          lastElement.focus();
        }
      } else {
        if (document.activeElement === lastElement) {
          e.preventDefault();
          firstElement.focus();
        }
      }
    };

    container.addEventListener("keydown", handleKeyDown);
    return () => container.removeEventListener("keydown", handleKeyDown);
  }, [containerRef]);
}

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

  const [presets, setPresets] = useState<ScanPresetSummary[]>([]);
  const [presetLoading, setPresetLoading] = useState(true);
  const [presetError, setPresetError] = useState<string | null>(null);

  const [drawer, setDrawer] = useState<DrawerView>(null);
  const [previousFocus, setPreviousFocus] = useState<HTMLElement | null>(null);

  const drawerRef = useRef<HTMLElement | null>(null);
  useFocusTrap(drawerRef);

  const [theme, setTheme] = useState<ThemeMode>(() => {
    if (typeof window === "undefined") return "light";
    const stored = localStorage.getItem("stock-theme");
    if (stored === "light" || stored === "dark" || stored === "system") {
      return stored;
    }
    return "light";
  });

  useEffect(() => {
    document.documentElement.dataset.theme =
      theme === "system"
        ? window.matchMedia("(prefers-color-scheme: dark)").matches
          ? "dark"
          : "light"
        : theme;
  }, [theme]);

  /* eslint-disable react-hooks/set-state-in-effect */
  useEffect(() => {
    setWatchlistLoading(true);
    setWatchlistError(null);
    setPresetLoading(true);
    setPresetError(null);
    let cancelled = false;
    Promise.all([listWatchlists(), listScanPresets()])
      .then(([wlData, presetData]) => {
        if (!cancelled) {
          setWatchlists(wlData);
          setPresets(presetData);
          setSelectedWatchlistId((prev) => reconcileSelectedId(prev, wlData));
          setSelectedPresetId((prev) => reconcileSelectedId(prev, presetData));
        }
      })
      .catch(() => {
        if (!cancelled) {
          setWatchlistError("Watchlist 목록을 불러오지 못했습니다.");
          setPresetError("Preset 목록을 불러오지 못했습니다.");
        }
      })
      .finally(() => {
        if (!cancelled) {
          setWatchlistLoading(false);
          setPresetLoading(false);
        }
      });
    return () => {
      cancelled = true;
    };
  }, []);
  /* eslint-enable react-hooks/set-state-in-effect */

  const refreshScannerResources = useCallback(() => {
    let cancelled = false;
    Promise.all([listWatchlists(), listScanPresets()])
      .then(([wlData, presetData]) => {
        if (!cancelled) {
          setWatchlists(wlData);
          setPresets(presetData);
          setSelectedWatchlistId((prev) => reconcileSelectedId(prev, wlData));
          setSelectedPresetId((prev) => reconcileSelectedId(prev, presetData));
        }
      })
      .catch(() => {
        if (!cancelled) {
          setWatchlistError("Watchlist 목록을 불러오지 못했습니다.");
          setPresetError("Preset 목록을 불러오지 못했습니다.");
        }
      });
  }, []);

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
    setPreviousFocus(document.activeElement as HTMLElement | null);
    setDrawer("watchlists");
    document.body.style.overflow = "hidden";
  }, []);

  const handleOpenPresetDrawer = useCallback(() => {
    setPreviousFocus(document.activeElement as HTMLElement | null);
    setDrawer("presets");
    document.body.style.overflow = "hidden";
  }, []);

  const closeDrawer = useCallback(() => {
    setDrawer(null);
    document.body.style.overflow = "";
    if (previousFocus) {
      (previousFocus as HTMLElement).focus();
      setPreviousFocus(null);
    }
    refreshScannerResources();
  }, [previousFocus, refreshScannerResources]);

  const handlePresetChange = useCallback((id: string) => {
    setSelectedPresetId(id);
  }, []);

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
              presets={presets}
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
