"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { formatAppError } from "@/lib/app-error";
import { cancelScan, getScanErrors, getScanRun, listScanRuns, startScan } from "./api";
import { subscribeScanEvent, unsubscribeAll } from "./events";
import type {
  ScanError,
  ScanEventPayload,
  ScanRunDetail,
  ScanRunSummary,
} from "./types";
import { listWatchlists } from "@/features/watchlists/api";
import type { WatchlistSummary } from "@/features/watchlists/types";
import { listScanPresets } from "@/features/scan-presets/api";
import type { ScanPresetSummary } from "@/features/scan-presets/types";

type SelectOption = {
  id: string;
  name: string;
};

type SetupState = {
  watchlists: SelectOption[];
  presets: SelectOption[];
  selectedWatchlistId: string;
  selectedPresetId: string;
  isRunning: boolean;
  currentRunId: string | null;
  runDetail: ScanRunDetail | null;
  errors: ScanError[];
  globalError: string;
  isLoading: boolean;
  currentSymbol: string | null;
  isCancelling: boolean;
};

const emptyState: SetupState = {
  watchlists: [],
  presets: [],
  selectedWatchlistId: "",
  selectedPresetId: "",
  isRunning: false,
  currentRunId: null,
  runDetail: null,
  errors: [],
  globalError: "",
  isLoading: false,
  currentSymbol: null,
  isCancelling: false,
};

const FINISHED_STATUSES: Set<ScanRunDetail["status"]> = new Set([
  "completed",
  "cancelled",
  "failed",
]);

function progressPercent(detail: ScanRunDetail): number {
  if (detail.totalSymbols === 0) return 0;
  return Math.round((detail.succeededSymbols + detail.failedSymbols) / detail.totalSymbols * 100);
}

export default function ScanRunSetup() {
  const [state, setState] = useState<SetupState>(emptyState);
  const [presetConditionCount, setPresetConditionCount] = useState<number | null>(null);
  const [isLoadingPresets, setIsLoadingPresets] = useState(true);
  const pollTimerRef = useRef<number | null>(null);

  const loadInitialData = useCallback(async () => {
    try {
      const [watchlists, presets] = await Promise.all([
        listWatchlists(),
        listScanPresets(),
      ]);
      setState((s) => ({
        ...s,
        watchlists: watchlists.map((w) => ({ id: w.id, name: w.name })),
        presets: presets.map((p) => ({ id: p.id, name: p.name })),
      }));
    } catch (error) {
      setState((s) => ({ ...s, globalError: formatAppError(error) }));
    } finally {
      setIsLoadingPresets(false);
    }
  }, []);

  useEffect(() => {
    let cancelled = false;
    void loadInitialData().catch(() => {
      if (!cancelled) {
        setState((s) => ({ ...s, globalError: "목록을 불러오지 못했습니다." }));
        setIsLoadingPresets(false);
      }
    });
    return () => {
      cancelled = true;
    };
  }, [loadInitialData]);

  useEffect(() => {
    if (state.selectedPresetId) {
      listScanPresets()
        .then((presets) => {
          const found = presets.find((p) => p.id === state.selectedPresetId);
          if (found) {
            setPresetConditionCount(found.enabledConditionCount);
          } else {
            setPresetConditionCount(null);
          }
        })
        .catch(() => setPresetConditionCount(null));
    } else {
      setPresetConditionCount(null);
    }
  }, [state.selectedPresetId]);

  const startPolling = useCallback((runId: string) => {
    if (pollTimerRef.current) {
      clearInterval(pollTimerRef.current);
    }
    pollTimerRef.current = window.setInterval(async () => {
      try {
        const detail = await getScanRun(runId);
        setState((s) => ({ ...s, runDetail: detail }));
        if (FINISHED_STATUSES.has(detail.status)) {
          if (pollTimerRef.current) {
            clearInterval(pollTimerRef.current);
            pollTimerRef.current = null;
          }
          try {
            const errors = await getScanErrors(runId);
            setState((s) => ({ ...s, errors, isRunning: false, isCancelling: false }));
          } catch {
            setState((s) => ({ ...s, isRunning: false, isCancelling: false }));
          }
        }
      } catch {
        // polling error — keep trying
      }
    }, 2000);
  }, []);

  useEffect(() => {
    if (state.currentRunId && state.isRunning) {
      startPolling(state.currentRunId);
    }
    return () => {
      if (pollTimerRef.current) {
        clearInterval(pollTimerRef.current);
        pollTimerRef.current = null;
      }
    };
  }, [state.currentRunId, state.isRunning, startPolling]);

  const handleProgress = useCallback((payload: ScanEventPayload) => {
    if (payload.runId !== state.currentRunId) return;
    if ("currentSymbol" in payload) {
      setState((s) => ({
        ...s,
        currentSymbol: payload.currentSymbol ?? null,
      }));
    }
  }, [state.currentRunId]);

  const handleResult = useCallback((payload: ScanEventPayload) => {
    if (payload.runId !== state.currentRunId) return;
    if ("symbol" in payload && "success" in payload) {
      setState((s) => ({
        ...s,
        currentSymbol: payload.success ? null : payload.symbol,
      }));
    }
  }, [state.currentRunId]);

  useEffect(() => {
    if (!state.currentRunId) return;

    let unsubProgress: (() => void) | null = null;
    let unsubResult: (() => void) | null = null;

    subscribeScanEvent("scan://progress", handleProgress).then((unsub) => {
      unsubProgress = unsub;
    });
    subscribeScanEvent("scan://result", handleResult).then((unsub) => {
      unsubResult = unsub;
    });

    return () => {
      if (unsubProgress) unsubProgress();
      if (unsubResult) unsubResult();
    };
  }, [state.currentRunId, handleProgress, handleResult]);

  useEffect(() => {
    return () => {
      unsubscribeAll();
    };
  }, []);

  const handleWatchlistChange = useCallback((e: React.ChangeEvent<HTMLSelectElement>) => {
    setState((s) => ({
      ...s,
      selectedWatchlistId: e.target.value,
      globalError: "",
    }));
  }, []);

  const handlePresetChange = useCallback((e: React.ChangeEvent<HTMLSelectElement>) => {
    setState((s) => ({
      ...s,
      selectedPresetId: e.target.value,
      globalError: "",
    }));
  }, []);

  const handleStart = useCallback(async () => {
    if (!state.selectedWatchlistId || !state.selectedPresetId) return;

    setState((s) => ({ ...s, globalError: "", isLoading: true }));

    try {
      const runId = await startScan({
        watchlistId: state.selectedWatchlistId,
        presetId: state.selectedPresetId,
      });
      setState((s) => ({
        ...s,
        currentRunId: runId,
        isRunning: true,
        runDetail: null,
        errors: [],
      }));
      startPolling(runId);
    } catch (error) {
      setState((s) => ({ ...s, globalError: formatAppError(error) }));
    } finally {
      setState((s) => ({ ...s, isLoading: false }));
    }
  }, [state.selectedWatchlistId, state.selectedPresetId, startPolling]);

  const handleCancel = useCallback(async () => {
    if (!state.currentRunId || state.isCancelling) return;

    setState((s) => ({ ...s, isCancelling: true, globalError: "" }));

    try {
      await cancelScan(state.currentRunId);
    } catch (error) {
      setState((s) => ({ ...s, isCancelling: false, globalError: formatAppError(error) }));
    }
  }, [state.currentRunId, state.isCancelling]);

  const handleReset = useCallback(() => {
    if (pollTimerRef.current) {
      clearInterval(pollTimerRef.current);
      pollTimerRef.current = null;
    }
    setState(emptyState);
    setPresetConditionCount(null);
  }, []);

  const selectedWatchlist = state.watchlists.find(
    (w) => w.id === state.selectedWatchlistId,
  );
  const selectedPreset = state.presets.find(
    (p) => p.id === state.selectedPresetId,
  );

  const watchlistSymbolCount = state.watchlists.find(
    (w) => w.id === state.selectedWatchlistId,
  ) as WatchlistSummary | undefined;

  const canStart =
    state.selectedWatchlistId &&
    state.selectedPresetId &&
    !state.isRunning &&
    !state.isLoading;

  return (
    <div className="panel scan-run-setup" style={{ padding: "20px" }}>
      <div className="panel-heading">
        <div>
          <p className="eyebrow">Run configuration</p>
          <h3>Scan Run Setup</h3>
        </div>
        {(state.currentRunId || state.runDetail) && (
          <button className="secondary-button" type="button" onClick={handleReset}>
            초기화
          </button>
        )}
      </div>

      {isLoadingPresets ? (
        <p className="muted">목록을 불러오는 중입니다.</p>
      ) : (
        <>
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "1fr 1fr",
              gap: "16px",
              marginBottom: "16px",
            }}
          >
            <label style={{ display: "grid", gap: "6px", fontSize: "13px", fontWeight: 600, color: "#c9d0dd" }}>
              Watchlist
              <select
                value={state.selectedWatchlistId}
                onChange={handleWatchlistChange}
                disabled={state.isRunning}
                style={{
                  width: "100%",
                  border: "1px solid #303848",
                  borderRadius: "10px",
                  outline: "none",
                  background: "#0d1118",
                  color: "#f5f7fb",
                  minHeight: "42px",
                  padding: "9px 11px",
                  fontSize: "13px",
                }}
              >
                <option value="">-- 선택 --</option>
                {state.watchlists.map((w) => (
                  <option key={w.id} value={w.id}>
                    {w.name}
                  </option>
                ))}
              </select>
            </label>

            <label style={{ display: "grid", gap: "6px", fontSize: "13px", fontWeight: 600, color: "#c9d0dd" }}>
              Scan Preset
              <select
                value={state.selectedPresetId}
                onChange={handlePresetChange}
                disabled={state.isRunning}
                style={{
                  width: "100%",
                  border: "1px solid #303848",
                  borderRadius: "10px",
                  outline: "none",
                  background: "#0d1118",
                  color: "#f5f7fb",
                  minHeight: "42px",
                  padding: "9px 11px",
                  fontSize: "13px",
                }}
              >
                <option value="">-- 선택 --</option>
                {state.presets.map((p) => (
                  <option key={p.id} value={p.id}>
                    {p.name}
                  </option>
                ))}
              </select>
            </label>
          </div>

          {!state.selectedWatchlistId && !state.selectedPresetId ? (
            <div className="compact-empty" style={{ minHeight: "60px", padding: "12px", marginBottom: "16px" }}>
              <span>Watchlist과 Preset을 모두 선택하십시오.</span>
            </div>
          ) : null}

          {(state.selectedWatchlistId || state.selectedPresetId) && (
            <div className="form-meta" style={{ marginBottom: "16px" }}>
              <span>
                {presetConditionCount !== null ? `${presetConditionCount} conditions` : "— conditions"}
              </span>
              <span>
                {watchlistSymbolCount
                  ? `${watchlistSymbolCount.symbolCount} symbols`
                  : "— symbols"}
              </span>
            </div>
          )}

          <div className="form-actions" style={{ marginBottom: "16px" }}>
            {state.isRunning ? (
              <button
                className="scan-button"
                type="button"
                onClick={handleCancel}
              >
                Cancel
              </button>
            ) : (
              <button
                className="primary-button strong"
                type="button"
                onClick={handleStart}
                disabled={!canStart}
              >
                {state.isLoading ? "Starting…" : "Start Scan"}
              </button>
            )}
          </div>

          {state.globalError ? (
            <div className="message error-message" style={{ marginBottom: "16px" }}>
              {state.globalError}
            </div>
          ) : null}

          {state.runDetail && (
            <div style={{ marginBottom: "16px" }}>
              <div style={{ display: "flex", justifyContent: "space-between", marginBottom: "6px" }}>
                <span style={{ fontSize: "13px", color: "#8f98aa" }}>
                  Run: {state.runDetail.id}
                </span>
                <span
                  style={{
                    fontSize: "12px",
                    padding: "3px 8px",
                    borderRadius: "999px",
                    background: "#0e1219",
                    color: "#c9d4e7",
                  }}
                >
                  {state.runDetail.status}
                </span>
              </div>
              {state.currentSymbol && (
                <div style={{ fontSize: "12px", color: "#8f98aa", marginBottom: "6px" }}>
                  Processing: <strong style={{ color: "#c9d4e7" }}>{state.currentSymbol}</strong>
                </div>
              )}
              <div style={{
                height: "6px",
                borderRadius: "3px",
                background: "#171c26",
                overflow: "hidden",
              }}>
                <div style={{
                  height: "100%",
                  width: `${progressPercent(state.runDetail)}%`,
                  background: "#7185a8",
                  borderRadius: "3px",
                  transition: "width 200ms ease",
                }} />
              </div>
              <div className="form-meta" style={{ marginTop: "8px" }}>
                <span>
                  {state.runDetail.succeededSymbols} succeeded
                </span>
                <span>
                  {state.runDetail.failedSymbols} failed
                </span>
                <span>
                  {progressPercent(state.runDetail)}%
                </span>
              </div>
            </div>
          )}

          {state.errors.length > 0 && (
            <div>
              <h4 style={{ margin: "0 0 8px", fontSize: "14px", color: "#ffb4be" }}>
                Errors ({state.errors.length})
              </h4>
              <div style={{ display: "grid", gap: "6px" }}>
                {state.errors.map((err, idx) => (
                  <div
                    key={idx}
                    style={{
                      fontSize: "12px",
                      padding: "8px 10px",
                      border: "1px solid #303746",
                      borderRadius: "8px",
                      background: "#0d1118",
                      color: "#c9d0dd",
                    }}
                  >
                    <div>
                      <strong>{err.symbol || "unknown"}</strong> — {err.code}
                    </div>
                    <div style={{ color: "#8f98aa", marginTop: "2px" }}>
                      {err.message}
                    </div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </>
      )}
    </div>
  );
}
