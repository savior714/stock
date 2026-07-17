"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { formatAppError } from "@/lib/app-error";
import styles from "./ScanRunSetup.module.css";
import { cancelScan, getScanErrors, getScanRun, startScan } from "./api";
import { subscribeScanEvent, unsubscribeAll } from "./events";
import type {
  ScanError,
  ScanEventPayload,
  ScanRunDetail,
} from "./types";
import type { WatchlistSummary } from "@/features/watchlists/types";
import type { ScanPresetSummary } from "@/features/scan-presets/types";

type SetupState = {
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
  return Math.round(
    (detail.succeededSymbols + detail.failedSymbols) / detail.totalSymbols * 100,
  );
}

type ScanRunSetupProps = {
  selectedWatchlistId: string;
  onWatchlistIdChange: (id: string) => void;
  selectedPresetId: string;
  onPresetIdChange: (id: string) => void;
  watchlists: WatchlistSummary[];
  presets: ScanPresetSummary[];
  onOpenPresetDrawer: () => void;
  presetExists: boolean;
};

export default function ScanRunSetup({
  selectedWatchlistId: externalWatchlistId,
  onWatchlistIdChange,
  selectedPresetId: externalPresetId,
  onPresetIdChange,
  watchlists,
  presets,
  onOpenPresetDrawer,
  presetExists,
}: ScanRunSetupProps) {
  const [state, setState] = useState<SetupState>(emptyState);
  const [presetConditionCount, setPresetConditionCount] = useState<number | null>(null);
  const pollTimerRef = useRef<number | null>(null);

  useEffect(() => {
    if (externalPresetId && presetExists) {
      const found = presets.find((p) => p.id === externalPresetId);
      setPresetConditionCount(found?.enabledConditionCount ?? null);
    } else {
      setPresetConditionCount(null);
    }
  }, [externalPresetId, presets, presetExists]);

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

  const handlePresetChange = useCallback(
    (e: React.ChangeEvent<HTMLSelectElement>) => {
      onPresetIdChange(e.target.value);
      setState((s) => ({ ...s, globalError: "" }));
    },
    [onPresetIdChange],
  );

  const handleStart = useCallback(
    async (watchlistId: string, presetId: string) => {
      if (!watchlistId || !presetId) return;

      setState((s) => ({ ...s, globalError: "", isLoading: true }));

      try {
        const runId = await startScan({
          watchlistId,
          presetId,
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
    },
    [startPolling],
  );

  const handleCancel = useCallback(async () => {
    if (!state.currentRunId || state.isCancelling) return;

    setState((s) => ({ ...s, isCancelling: true, globalError: "" }));

    try {
      await cancelScan(state.currentRunId);
    } catch (error) {
      setState((s) => ({
        ...s,
        isCancelling: false,
        globalError: formatAppError(error),
      }));
    }
  }, [state.currentRunId, state.isCancelling]);

  const canStart =
    externalWatchlistId &&
    externalPresetId &&
    presetExists &&
    !state.isRunning &&
    !state.isLoading;

  const selectedPreset = presets.find((p) => p.id === externalPresetId);
  const selectedWatchlist = watchlists.find(
    (w) => w.id === externalWatchlistId,
  );

  const watchlistSymbolCount = watchlists.find(
    (w) => w.id === externalWatchlistId,
  ) as WatchlistSummary | undefined;

  return (
    <div className={styles.setupArea}>
      <div className={styles.setupContext}>
        <div>
          <p className={styles.setupContextName}>{selectedWatchlist?.name}</p>
          <p className={styles.setupContextMeta}>
            {watchlistSymbolCount
              ? `${watchlistSymbolCount.symbolCount} symbols`
              : "— symbols"}
          </p>
        </div>
        <div className={styles.setupContextActions}>
          <button
            className="secondary-button"
            type="button"
            onClick={onOpenPresetDrawer}
          >
            Preset 편집
          </button>
        </div>
      </div>

      <div className={styles.setupControls}>
        <div className={styles.setupGrid}>
          <div className={styles.setupSelectGroup}>
            <label>Scan Preset</label>
            <select
              value={externalPresetId}
              onChange={handlePresetChange}
              disabled={state.isRunning}
            >
              <option value="">-- 선택 --</option>
              {presets.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
          </div>
        </div>

        {externalWatchlistId && externalPresetId && presetExists && (
          <div className={styles.setupInfo}>
            <span>
              {presetConditionCount !== null
                ? `${presetConditionCount} conditions`
                : "— conditions"}
            </span>
            <span>
              {watchlistSymbolCount
                ? `${watchlistSymbolCount.symbolCount} symbols`
                : "— symbols"}
            </span>
          </div>
        )}

        {!externalWatchlistId && !externalPresetId ? (
          <div className={styles.setupEmptySelect}>
            Watchlist과 Preset을 모두 선택하십시오.
          </div>
        ) : externalWatchlistId && !externalPresetId ? (
          <div className={styles.setupEmptySelect} aria-live="polite">
            Scan Preset을 선택하십시오.
          </div>
        ) : null}

        {(externalWatchlistId || externalPresetId) && (
          <div className={styles.setupActions}>
            {state.isRunning ? (
              <button
                className="scan-button"
                type="button"
                onClick={handleCancel}
              >
                {state.isCancelling ? "취소 중…" : "실행 취소"}
              </button>
            ) : (
              <button
                className="primary-button strong"
                type="button"
                onClick={() => handleStart(externalWatchlistId, externalPresetId)}
                disabled={!canStart}
              >
                {state.isLoading ? "Scan 시작 중…" : "Scan 실행"}
              </button>
            )}
          </div>
        )}

        {state.globalError ? (
          <div className="message error-message">{state.globalError}</div>
        ) : null}

        {state.runDetail && (
          <div className={styles.setupProgress}>
            <div className={styles.setupProgressHeader}>
              <span className={styles.setupProgressStatus}>
                {state.runDetail.status === "completed"
                  ? "스캔 완료"
                  : state.runDetail.status === "cancelled"
                    ? "스캔 취소됨"
                    : state.runDetail.status === "failed"
                      ? "스캔 실패"
                      : "처리 중…"}
              </span>
              <span className={styles.setupProgressBadge}>{state.runDetail.status}</span>
            </div>
            {state.currentSymbol && (
              <div className={styles.setupProgressCurrent}>
                Processing: <strong>{state.currentSymbol}</strong>
              </div>
            )}
            <div className={styles.setupProgressBarTrack}>
              <div
                className={styles.setupProgressBarFill}
                style={{ width: `${progressPercent(state.runDetail)}%` }}
              />
            </div>
            <div className={styles.setupProgressMeta}>
              <span>{state.runDetail.succeededSymbols} succeeded</span>
              <span>{state.runDetail.failedSymbols} failed</span>
              <span>{progressPercent(state.runDetail)}%</span>
            </div>
          </div>
        )}

        {state.runDetail && FINISHED_STATUSES.has(state.runDetail.status) && state.runDetail.status === "completed" && (
          <div className={styles.setupCompletion}>
            <h4>스캔 완료</h4>
            <p>
              {state.runDetail.totalSymbols}개 중{" "}
              {state.runDetail.succeededSymbols}개 종목 처리 완료
            </p>
          </div>
        )}

        {state.errors.length > 0 && (
          <div className={styles.setupErrors}>
            <h4>Errors ({state.errors.length})</h4>
            <div className={styles.setupErrorList}>
              {state.errors.map((err, idx) => (
                <div key={idx} className={styles.setupErrorItem}>
                  <div>
                    <strong>{err.symbol || "unknown"}</strong> — {err.code}
                  </div>
                  <div className={styles.setupErrorItemDetail}>
                    {err.message}
                  </div>
                </div>
              ))}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
