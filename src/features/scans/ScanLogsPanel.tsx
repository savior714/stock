"use client";

import { useCallback, useEffect, useState } from "react";

import { formatAppError } from "@/lib/app-error";
import { cancelScan, getScanErrors, getScanRun, retryScan } from "./api";
import type { ScanError, ScanRunDetail } from "./types";
import styles from "./ScanLogsPanel.module.css";

type ScanLogsPanelProps = {
  runId: string;
  onRetry: (retryRunId: string) => void;
};

type PanelState = {
  errors: ScanError[];
  runDetail: ScanRunDetail | null;
  isLoading: boolean;
  globalError: string;
  retrying: boolean;
};

const COLUMN_WIDTHS = {
  symbol: "120px",
  code: "140px",
  attempt: "60px",
  retryable: "90px",
};

function getRetryableSymbols(errors: ScanError[]): string[] {
  const retryableSet = new Set<string>();
  for (const err of errors) {
    if (err.retryable && err.symbol !== null) {
      retryableSet.add(err.symbol);
    }
  }
  return Array.from(retryableSet);
}

function RetryBadge({ retryable }: { retryable: boolean }) {
  return (
    <span className={`${styles.badge}${retryable ? ` ${styles.badgeSuccess}` : ""}`}>
      {retryable ? "Retryable" : "Permanent"}
    </span>
  );
}

export default function ScanLogsPanel({
  runId,
  onRetry,
}: ScanLogsPanelProps) {
  const [state, setState] = useState<PanelState>({
    errors: [],
    runDetail: null,
    isLoading: true,
    globalError: "",
    retrying: false,
  });

  const loadErrors = useCallback(async () => {
    setState((prev) => ({ ...prev, isLoading: true, globalError: "" }));
    try {
      const [errors, runDetail] = await Promise.all([
        getScanErrors(runId),
        getScanRun(runId),
      ]);
      setState({
        errors,
        runDetail,
        isLoading: false,
        globalError: "",
        retrying: false,
      });
    } catch (err) {
      setState((prev) => ({
        ...prev,
        isLoading: false,
        globalError: formatAppError(err),
      }));
    }
  }, [runId]);

  useEffect(() => {
    loadErrors();
  }, [loadErrors]);

  const retryableSymbols = getRetryableSymbols(state.errors);
  const retryableCount = retryableSymbols.length;

  const handleRetry = async () => {
    setState((prev) => ({ ...prev, retrying: true }));
    try {
      const newRunId = await retryScan(runId);
      onRetry(newRunId);
    } catch (err) {
      setState((prev) => ({
        ...prev,
        retrying: false,
        globalError: formatAppError(err),
      }));
    }
  };

  const isRetryable =
    state.runDetail &&
    (state.runDetail.status === "completed" || state.runDetail.status === "failed");

  if (state.isLoading) {
    return (
      <div className="panel">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Scan logs</p>
            <h3>Logs &amp; Errors</h3>
          </div>
        </div>
        <p className={styles.mutedCenter}>Loading logs…</p>
      </div>
    );
  }

  if (state.globalError) {
    return (
      <div className="panel">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Scan logs</p>
            <h3>Logs &amp; Errors</h3>
          </div>
        </div>
        <p className={styles.globalError}>{state.globalError}</p>
      </div>
    );
  }

  return (
    <div className={`panel ${styles.scanLogsPanel}`}>
      <div className="panel-heading">
        <div>
          <p className="eyebrow">Scan logs</p>
          <h3>
            Logs &amp; Errors
            {state.errors.length > 0 && (
              <span className={`${styles.errorCountBadge}`}>
                {state.errors.length}
              </span>
            )}
          </h3>
        </div>
      </div>

      {state.errors.length === 0 ? (
        <div className="empty-state" style={{ minHeight: "120px", padding: "20px" }}>
          <p className={styles.mutedCenter}>No errors — all symbols processed successfully.</p>
        </div>
      ) : (
        <div style={{ overflowX: "auto" }}>
          <table className={styles.table}>
            <thead>
              <tr>
                <th
                  className={styles.th}
                  style={{ width: COLUMN_WIDTHS.symbol, textAlign: "left" }}
                >
                  Symbol
                </th>
                <th
                  className={styles.th}
                  style={{ width: COLUMN_WIDTHS.code, textAlign: "left" }}
                >
                  Code
                </th>
                <th
                  className={styles.th}
                  style={{ flex: 1, textAlign: "left", minWidth: "200px" }}
                >
                  Message
                </th>
                <th
                  className={`${styles.th} ${styles.thCenter}`}
                  style={{ width: COLUMN_WIDTHS.attempt }}
                >
                  Attempt
                </th>
                <th
                  className={`${styles.th} ${styles.thCenter}`}
                  style={{ width: COLUMN_WIDTHS.retryable }}
                >
                  Retryable
                </th>
              </tr>
            </thead>
            <tbody>
              {state.errors.map((error, index) => {
                const leftBorder = error.retryable
                  ? styles.rowRetryable
                  : styles.rowPermanent;

                return (
                  <tr
                    key={`${error.symbol ?? "unknown"}-${error.code}-${index}`}
                    className={`${styles.row} ${leftBorder}`}
                  >
                    <td
                      className={`${styles.cell} ${styles.cellMonospace}`}
                      style={{ width: COLUMN_WIDTHS.symbol }}
                    >
                      {error.symbol ?? "unknown"}
                    </td>
                    <td
                      className={`${styles.cell} ${styles.cellMonospace} ${styles.cellMuted}`}
                      style={{ width: COLUMN_WIDTHS.code }}
                    >
                      {error.code}
                    </td>
                    <td
                      className={`${styles.cell}`}
                      style={{ flex: 1, minWidth: "200px" }}
                    >
                      {error.message}
                    </td>
                    <td
                      className={`${styles.cell} ${styles.cellCenter}`}
                      style={{ width: COLUMN_WIDTHS.attempt }}
                    >
                      {error.attempt}
                    </td>
                    <td
                      className={`${styles.cell} ${styles.cellCenter}`}
                      style={{ width: COLUMN_WIDTHS.retryable }}
                    >
                      <RetryBadge retryable={error.retryable} />
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}

      {isRetryable && retryableCount > 0 && (
        <div className={styles.retryBar}>
          <span className={styles.retryText}>
            {retryableCount} failed symbol{retryableCount !== 1 ? "s" : ""} can be retried
          </span>
          <button
            onClick={handleRetry}
            disabled={state.retrying}
            className={`${styles.retryButton}${state.retrying ? ` ${styles.retryButtonDisabled}` : ""}`}
          >
            {state.retrying ? "Retrying..." : `Retry ${retryableCount} symbol${retryableCount !== 1 ? "s" : ""}`}
          </button>
        </div>
      )}
    </div>
  );
}
