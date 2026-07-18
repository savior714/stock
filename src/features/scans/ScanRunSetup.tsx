"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { canStartScan } from "@/lib/scanner-utils";
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
  selectedPresetId: string;
  onPresetIdChange: (id: string) => void;
  watchlists: WatchlistSummary[];
  presets: ScanPresetSummary[];
  presetsLoading: boolean;
  presetsError: string | null;
  onOpenPresetDrawer: () => void;
  presetExists: boolean;
  watchlistExists: boolean;
  resumeRunId: string | null;
  onResumeRunCompleted?: (run: ScanRunDetail) => void;
};

export default function ScanRunSetup({
  selectedWatchlistId: externalWatchlistId,
  selectedPresetId: externalPresetId,
  onPresetIdChange,
  watchlists,
  presets,
  presetsLoading,
  presetsError,
  onOpenPresetDrawer,
  presetExists,
  watchlistExists,
  resumeRunId,
  onResumeRunCompleted,
}: ScanRunSetupProps) {
  const [state, setState] = useState<SetupState>(emptyState);
  const [presetConditionCount, setPresetConditionCount] = useState<number | null>(null);
  const pollTimerRef = useRef<number | null>(null);
  const consumedResumeRef = useRef<string | null>(null);
  const notifiedTerminalRunsRef = useRef(new Set<string>());

  // ── Centralized notify helper ──
  const notifyResumeCompleted = useCallback(
    (detail: ScanRunDetail) => {
      if (!resumeRunId) return;
      if (detail.id !== resumeRunId) return;
      if (detail.status !== "completed") return;
      if (notifiedTerminalRunsRef.current.has(detail.id)) return;

      notifiedTerminalRunsRef.current.add(detail.id);
      onResumeRunCompleted?.(detail);
    },
    [resumeRunId, onResumeRunCompleted],
  );

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
          notifyResumeCompleted(detail);
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
  }, [notifyResumeCompleted]);

  // ── Resume run initialization ──
  useEffect(() => {
    if (!resumeRunId) return;
    if (consumedResumeRef.current === resumeRunId) return;
    consumedResumeRef.current = resumeRunId;
    notifiedTerminalRunsRef.current.clear();

    let cancelled = false;

    getScanRun(resumeRunId)
      .then((detail) => {
        if (cancelled) return;
        setState((s) => ({
          ...s,
          currentRunId: resumeRunId,
          isRunning: detail.status === "running",
          runDetail: detail,
          errors: [],
          globalError: "",
        }));
        if (detail.status === "running") {
          startPolling(resumeRunId);
        } else if (detail.status === "completed") {
          notifyResumeCompleted(detail);
        }
      })
      .catch(() => {
        // initial fetch failed — start polling so it retries
        consumedResumeRef.current = null;
        setState((s) => ({
          ...s,
          currentRunId: resumeRunId,
          isRunning: true,
          runDetail: null,
          errors: [],
          globalError: "",
        }));
      });

    return () => {
      cancelled = true;
    };
  }, [resumeRunId, startPolling, notifyResumeCompleted]);

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

  const handleCompleted = useCallback((payload: ScanEventPayload) => {
    if (payload.runId !== state.currentRunId) return;
    if (!resumeRunId || payload.runId !== resumeRunId) return;

    // Only notify after successfully fetching the final detail.
    // If fetch fails, the Set is NOT modified and polling fallback
    // will handle the completion.
    getScanRun(payload.runId)
      .then((detail) => {
        notifyResumeCompleted(detail);
      })
      .catch(() => {
        // fetch failed — do NOT add to notified set.
        // polling will catch up and call notifyResumeCompleted.
      });
  }, [state.currentRunId, resumeRunId, notifyResumeCompleted]);

  useEffect(() => {
    if (!state.currentRunId) return;

    let unsubProgress: (() => void) | null = null;
    let unsubResult: (() => void) | null = null;
    let unsubCompleted: (() => void) | null = null;

    subscribeScanEvent("scan://progress", handleProgress).then((unsub) => {
      unsubProgress = unsub;
    });
    subscribeScanEvent("scan://result", handleResult).then((unsub) => {
      unsubResult = unsub;
    });
    subscribeScanEvent("scan://completed", handleCompleted).then((unsub) => {
      unsubCompleted = unsub;
    });

    return () => {
      if (unsubProgress) unsubProgress();
      if (unsubResult) unsubResult();
      if (unsubCompleted) unsubCompleted();
    };
  }, [state.currentRunId, handleProgress, handleResult, handleCompleted]);

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

  const canStart = canStartScan({
    selectedWatchlistId: externalWatchlistId,
    selectedPresetId: externalPresetId,
    watchlistExists,
    presetExists,
    isRunning: state.isRunning,
    isLoading: state.isLoading,
  });

  const watchlistSymbolCount = watchlists.find(
    (w) => w.id === externalWatchlistId,
  ) as WatchlistSummary | undefined;

  return (
    <div className={styles.setupArea}>
      <div className={styles.setupContext}>
        <div>
          <p className={styles.setupContextName}>{watchlists.find((w) => w.id === externalWatchlistId)?.name}</p>
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
              disabled={state.isRunning || presetsLoading}
            >
              <option value="">-- 선택 --</option>
              {presets.map((p) => (
                <option key={p.id} value={p.id}>
                  {p.name}
                </option>
              ))}
            </select>
            {presetsLoading && (
              <span className={styles.setupLoadingText}>Preset 목록을 불러오는 중…</span>
            )}
            {presetsError && (
              <span className={styles.setupErrorText}>{presetsError}</span>
            )}
          </div>
        </div>

        {!externalWatchlistId ? (
          <div className={styles.setupEmptySelect} aria-live="polite">
            Watchlist을(를) 선택하십시오.
          </div>
        ) : !watchlistExists ? (
          <div className={styles.setupEmptySelect} aria-live="polite">
            선택한 Watchlist이 삭제되었습니다. 유효한 Watchlist을 선택하십시오.
          </div>
        ) : !externalPresetId ? (
          <div className={styles.setupEmptySelect} aria-live="polite">
            Scan Preset을 선택하십시오.
          </div>
        ) : !presetExists ? (
          <div className={styles.setupEmptySelect} aria-live="polite">
            선택한 Preset이 삭제되었습니다. 유효한 Preset을 선택하십시오.
          </div>
        ) : null}

        {externalWatchlistId && (
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
