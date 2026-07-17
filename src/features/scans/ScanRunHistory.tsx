"use client";

import { useCallback, useEffect, useState } from "react";

import { formatAppError } from "@/lib/app-error";
import { getScanRun, listScanRuns } from "./api";
import type { ScanRunDetail, ScanRunSummary } from "./types";
import styles from "./ScanRunHistory.module.css";

type ScanRunHistoryProps = {
  onRunSelect: (run: ScanRunDetail) => void;
};

type HistoryState = {
  runs: ScanRunSummary[];
  selectedRun: ScanRunDetail | null;
  isLoading: boolean;
  globalError: string;
};

const STATUS_CONFIG: Record<
  ScanRunSummary["status"],
  { bgClass: string; label: string }
> = {
  pending: { bgClass: styles.statusPending, label: "Pending" },
  running: { bgClass: styles.statusRunning, label: "Running" },
  completed: { bgClass: styles.statusCompleted, label: "Completed" },
  cancelled: { bgClass: styles.statusCancelled, label: "Cancelled" },
  failed: { bgClass: styles.statusFailed, label: "Failed" },
};

function formatTimestamp(iso: string | null): string {
  if (!iso) return "\u2014";
  const date = new Date(iso);
  const now = new Date();
  const diffMs = now.getTime() - date.getTime();
  const diffMins = Math.floor(diffMs / 60000);
  const diffHours = Math.floor(diffMins / 60);
  const diffDays = Math.floor(diffHours / 24);

  if (diffMins < 1) return "Just now";
  if (diffMins < 60) return `${diffMins}m ago`;
  if (diffHours < 24) return `${diffHours}h ago`;
  if (diffDays < 7) return `${diffDays}d ago`;

  return date.toLocaleString("ko-KR", {
    year: "numeric",
    month: "2-digit",
    day: "2-digit",
    hour: "2-digit",
    minute: "2-digit",
  });
}

function truncateId(id: string, maxLen: number): string {
  if (id.length <= maxLen) return id;
  return `${id.slice(0, maxLen)}\u2026`;
}

export default function ScanRunHistory({ onRunSelect }: ScanRunHistoryProps) {
  const [state, setState] = useState<HistoryState>({
    runs: [],
    selectedRun: null,
    isLoading: true,
    globalError: "",
  });

  const loadRuns = useCallback(async () => {
    setState((prev) => ({ ...prev, isLoading: true, globalError: "" }));
    try {
      const runs = await listScanRuns(20);
      setState((prev) => ({ ...prev, runs, isLoading: false }));
    } catch (err) {
      setState((prev) => ({
        ...prev,
        isLoading: false,
        globalError: formatAppError(err),
      }));
    }
  }, []);

  useEffect(() => {
    loadRuns();
  }, [loadRuns]);

  const handleSelectRun = async (run: ScanRunSummary) => {
    try {
      const detail = await getScanRun(run.id);
      setState((prev) => ({
        ...prev,
        selectedRun: detail,
      }));
      onRunSelect(detail);
    } catch (err) {
      setState((prev) => ({
        ...prev,
        globalError: formatAppError(err),
      }));
    }
  };

  if (state.isLoading) {
    return (
      <div className={styles.loading}>
        <p className={styles.mutedCenter}>Loading run history…</p>
      </div>
    );
  }

  if (state.globalError) {
    return (
      <div className={styles.loading}>
        <p className={styles.errorText}>{state.globalError}</p>
      </div>
    );
  }

  if (state.runs.length === 0) {
    return (
      <div className={styles.loading}>
        <div className="empty-state" style={{ minHeight: "120px" }}>
          <p className={styles.mutedCenter}>No scan runs yet. Start a scan to see history.</p>
        </div>
      </div>
    );
  }

  return (
    <div className={styles.historyContainer}>
      <div className={styles.historyHeader}>
        <h3 style={{ margin: 0, fontSize: "15px", fontWeight: 600 }}>
          Run History
        </h3>
        <button
          onClick={loadRuns}
          className={styles.refreshButton}
        >
          Refresh
        </button>
      </div>

      <div className={styles.historyList}>
        {state.runs.map((run) => {
          const isSelected = state.selectedRun?.id === run.id;
          const config = STATUS_CONFIG[run.status];

          return (
            <div
              key={run.id}
              className={`${styles.historyItem}${isSelected ? ` ${styles.historyItemSelected}` : ""}`}
              onClick={() => handleSelectRun(run)}
              role="button"
              tabIndex={0}
              onKeyDown={(e) => {
                if (e.key === "Enter" || e.key === " ") {
                  e.preventDefault();
                  handleSelectRun(run);
                }
              }}
            >
              <span className={`${styles.statusBadge} ${config.bgClass}`}>
                {config.label}
              </span>

              <div className={styles.historyItemInfo}>
                <div className={styles.historyItemId}>
                  {truncateId(run.id, 12)}
                </div>
                <div className={styles.historyItemMeta}>
                  {run.watchlistId} · {run.presetId}
                </div>
              </div>

              <div className={styles.historyItemStats}>
                <span>
                  {run.succeededSymbols}/{run.totalSymbols}
                </span>
                {run.failedSymbols > 0 ? (
                  <span className={styles.failedCount}>
                    {run.failedSymbols} failed
                  </span>
                ) : null}
              </div>

              <div className={styles.historyItemTime}>
                {formatTimestamp(run.startedAt ?? run.finishedAt)}
              </div>
            </div>
          );
        })}
      </div>

      {state.selectedRun && (
        <div className={styles.historyDetail}>
          <div className={styles.detailGrid}>
            <div>
              <span className={styles.detailLabel}>Base trade date</span>
              <div className={styles.detailValue}>
                {state.selectedRun.baseTradeDate ?? "\u2014"}
              </div>
            </div>
            <div>
              <span className={styles.detailLabel}>Symbols</span>
              <div className={styles.detailValue}>
                {state.selectedRun.totalSymbols}
              </div>
            </div>
            <div className={styles.detailFull}>
              <span className={styles.detailLabel}>Preset snapshot</span>
              <div className={`${styles.detailValue} ${styles.detailMonospace}`}>
                {JSON.stringify(state.selectedRun.presetSnapshotJson)}
              </div>
            </div>
            {state.selectedRun.retryOfRunId && (
              <div className={styles.detailFull}>
                <span className={styles.detailLabel}>Retry of</span>
                <div className={`${styles.detailValue} ${styles.detailMonospace}`}>
                  {state.selectedRun.retryOfRunId}
                </div>
              </div>
            )}
          </div>
        </div>
      )}
    </div>
  );
}
