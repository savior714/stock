"use client";

import { useCallback, useEffect, useState } from "react";

import { formatAppError } from "@/lib/app-error";
import { cancelScan, getScanErrors, getScanRun, startScan } from "./api";
import type { ScanError, ScanRunDetail } from "./types";

type ScanLogsPanelProps = {
  runId: string;
  presetId: string;
  watchlistId: string;
  onRetry: (retryRunId: string) => void;
};

type PanelState = {
  errors: ScanError[];
  runDetail: ScanRunDetail | null;
  isLoading: boolean;
  globalError: string;
  retryingSymbol: string | null;
};

const COLUMN_WIDTHS = {
  symbol: "120px",
  code: "140px",
  attempt: "60px",
  retryable: "90px",
};

const CELL_STYLE = {
  padding: "7px 10px",
  fontSize: "13px",
  color: "#c9d4e7",
  borderBottom: "1px solid #303746",
};

const HEADER_STYLE = {
  padding: "8px 10px",
  fontSize: "12px",
  color: "#8f98aa",
  fontWeight: 500,
  borderBottom: "1px solid #303746",
};

const BADGE_BASE = {
  display: "inline-block",
  borderRadius: "999px",
  padding: "2px 8px",
  fontSize: "12px",
  fontWeight: 600,
};

function RetryBadge({ retryable }: { retryable: boolean }) {
  return (
    <span
      style={{
        ...BADGE_BASE,
        background: retryable ? "#14261f" : "#0e1219",
        color: retryable ? "#a7e5c8" : "#8f98aa",
      }}
    >
      {retryable ? "Retryable" : "Permanent"}
    </span>
  );
}

export default function ScanLogsPanel({
  runId,
  presetId,
  watchlistId,
  onRetry,
}: ScanLogsPanelProps) {
  const [state, setState] = useState<PanelState>({
    errors: [],
    runDetail: null,
    isLoading: true,
    globalError: "",
    retryingSymbol: null,
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
        retryingSymbol: null,
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

  const retryableErrors = state.errors.filter((e) => e.retryable);

  const handleRetry = async () => {
    setState((prev) => ({ ...prev, retryingSymbol: "all" }));
    try {
      const newRunId = await startScan({ watchlistId, presetId });
      onRetry(newRunId);
    } catch (err) {
      setState((prev) => ({
        ...prev,
        retryingSymbol: null,
        globalError: formatAppError(err),
      }));
    }
  };

  const isRetryable =
    state.runDetail &&
    (state.runDetail.status === "completed" || state.runDetail.status === "failed");

  if (state.isLoading) {
    return (
      <div className="panel" style={{ padding: "20px" }}>
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Scan logs</p>
            <h3>Logs &amp; Errors</h3>
          </div>
        </div>
        <p className="muted" style={{ padding: "20px 0", textAlign: "center" }}>
          Loading logs\u2026
        </p>
      </div>
    );
  }

  if (state.globalError) {
    return (
      <div className="panel" style={{ padding: "20px" }}>
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Scan logs</p>
            <h3>Logs &amp; Errors</h3>
          </div>
        </div>
        <p style={{ padding: "20px 0", color: "#ffb4be" }}>{state.globalError}</p>
      </div>
    );
  }

  return (
    <div className="panel scan-logs-panel" style={{ padding: "0 0 12px 0", overflow: "hidden" }}>
      <div className="panel-heading" style={{ padding: "20px 20px 0 20px" }}>
        <div>
          <p className="eyebrow">Scan logs</p>
          <h3>
            Logs &amp; Errors
            {state.errors.length > 0 && (
              <span
                style={{
                  marginLeft: "10px",
                  display: "inline-block",
                  borderRadius: "999px",
                  background: "#2a171b",
                  color: "#ffb4be",
                  fontSize: "12px",
                  fontWeight: 600,
                  padding: "2px 10px",
                  minWidth: "24px",
                  textAlign: "center",
                }}
              >
                {state.errors.length}
              </span>
            )}
          </h3>
        </div>
      </div>

      {state.errors.length === 0 ? (
        <div className="empty-state" style={{ minHeight: "120px", padding: "20px" }}>
          <p className="muted" style={{ textAlign: "center" }}>
            No errors \u2014 all symbols processed successfully.
          </p>
        </div>
      ) : (
        <div style={{ overflowX: "auto" }}>
          <table
            style={{
              width: "100%",
              borderCollapse: "collapse",
              border: "1px solid #303746",
              fontSize: "13px",
            }}
          >
            <thead>
              <tr style={{ background: "#171c26" }}>
                <th
                  style={{
                    ...HEADER_STYLE,
                    width: COLUMN_WIDTHS.symbol,
                    textAlign: "left",
                  }}
                >
                  Symbol
                </th>
                <th
                  style={{
                    ...HEADER_STYLE,
                    width: COLUMN_WIDTHS.code,
                    textAlign: "left",
                  }}
                >
                  Code
                </th>
                <th
                  style={{
                    ...HEADER_STYLE,
                    flex: 1,
                    textAlign: "left",
                    minWidth: "200px",
                  }}
                >
                  Message
                </th>
                <th
                  style={{
                    ...HEADER_STYLE,
                    width: COLUMN_WIDTHS.attempt,
                    textAlign: "center",
                  }}
                >
                  Attempt
                </th>
                <th
                  style={{
                    ...HEADER_STYLE,
                    width: COLUMN_WIDTHS.retryable,
                    textAlign: "center",
                  }}
                >
                  Retryable
                </th>
              </tr>
            </thead>
            <tbody>
              {state.errors.map((error, index) => {
                const rowBg = "#0d1118";
                const hoverBg = "#171c26";
                const leftBorder = error.retryable
                  ? "3px solid #345d4d"
                  : "3px solid #6a343c";

                return (
                  <tr
                    key={`${error.symbol ?? "unknown"}-${error.code}-${index}`}
                    style={{
                      background: rowBg,
                      borderLeft: leftBorder,
                      transition: "background 120ms ease",
                    }}
                    onMouseEnter={(e) => {
                      (e.currentTarget as HTMLTableRowElement).style.background = hoverBg;
                    }}
                    onMouseLeave={(e) => {
                      (e.currentTarget as HTMLTableRowElement).style.background = rowBg;
                    }}
                  >
                    <td
                      style={{
                        ...CELL_STYLE,
                        width: COLUMN_WIDTHS.symbol,
                        fontFamily: "ui-monospace, SFMono-Regular, monospace",
                      }}
                    >
                      {error.symbol ?? "unknown"}
                    </td>
                    <td
                      style={{
                        ...CELL_STYLE,
                        width: COLUMN_WIDTHS.code,
                        fontFamily: "ui-monospace, SFMono-Regular, monospace",
                        color: "#8f98aa",
                      }}
                    >
                      {error.code}
                    </td>
                    <td
                      style={{
                        ...CELL_STYLE,
                        flex: 1,
                        minWidth: "200px",
                      }}
                    >
                      {error.message}
                    </td>
                    <td
                      style={{
                        ...CELL_STYLE,
                        width: COLUMN_WIDTHS.attempt,
                        textAlign: "center",
                      }}
                    >
                      {error.attempt}
                    </td>
                    <td
                      style={{
                        ...CELL_STYLE,
                        width: COLUMN_WIDTHS.retryable,
                        textAlign: "center",
                      }}
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

      {isRetryable && retryableErrors.length > 0 && (
        <div
          style={{
            display: "flex",
            alignItems: "center",
            justifyContent: "space-between",
            padding: "14px 20px",
            borderTop: "1px solid #242b39",
          }}
        >
          <span style={{ fontSize: "13px", color: "#8f98aa" }}>
            {retryableErrors.length} error{retryableErrors.length !== 1 ? "s" : ""} can be retried
          </span>
          <button
            onClick={handleRetry}
            disabled={state.retryingSymbol !== null}
            style={{
              display: "inline-flex",
              alignItems: "center",
              gap: "6px",
              padding: "6px 16px",
              borderRadius: "8px",
              border: "1px solid #345d4d",
              background: state.retryingSymbol !== null ? "#14261f" : "#1a3329",
              color: state.retryingSymbol !== null ? "#8f98aa" : "#a7e5c8",
              fontSize: "13px",
              fontWeight: 600,
              cursor: state.retryingSymbol !== null ? "not-allowed" : "pointer",
              outline: "none",
              transition: "background 120ms ease",
            }}
          >
            {state.retryingSymbol !== null ? "Retrying..." : "Retry failed symbols"}
          </button>
        </div>
      )}
    </div>
  );
}
