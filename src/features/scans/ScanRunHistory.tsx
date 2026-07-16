"use client";

import { useCallback, useEffect, useState } from "react";

import { formatAppError } from "@/lib/app-error";
import { getScanRun, listScanRuns } from "./api";
import type { ScanRunDetail, ScanRunSummary } from "./types";

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
  { bg: string; color: string; label: string }
> = {
  pending: { bg: "#2a2818", color: "#e5c07b", label: "Pending" },
  running: { bg: "#14263a", color: "#7eb8e5", label: "Running" },
  completed: { bg: "#14261f", color: "#a7e5c8", label: "Completed" },
  cancelled: { bg: "#2a1f14", color: "#e5a87b", label: "Cancelled" },
  failed: { bg: "#2a171b", color: "#ffb4be", label: "Failed" },
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
      <div style={{ padding: "20px" }}>
        <p className="muted" style={{ textAlign: "center", padding: "20px 0" }}>
          Loading run history\u2026
        </p>
      </div>
    );
  }

  if (state.globalError) {
    return (
      <div style={{ padding: "20px" }}>
        <p style={{ color: "#ffb4be", padding: "20px 0" }}>{state.globalError}</p>
      </div>
    );
  }

  if (state.runs.length === 0) {
    return (
      <div style={{ padding: "20px" }}>
        <div className="empty-state" style={{ minHeight: "120px" }}>
          <p className="muted" style={{ textAlign: "center" }}>
            No scan runs yet. Start a scan to see history.
          </p>
        </div>
      </div>
    );
  }

  return (
    <div style={{ padding: "0 0 12px 0", overflow: "hidden" }}>
      <div
        style={{
          display: "flex",
          alignItems: "center",
          justifyContent: "space-between",
          padding: "16px 14px 12px 14px",
          borderBottom: "1px solid #242b39",
        }}
      >
        <h3 style={{ margin: 0, fontSize: "15px", fontWeight: 600, color: "#c9d4e7" }}>
          Run History
        </h3>
        <button
          onClick={loadRuns}
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: "4px",
            padding: "4px 12px",
            borderRadius: "8px",
            border: "1px solid #303848",
            background: "#171c26",
            color: "#8f98aa",
            fontSize: "12px",
            fontWeight: 500,
            cursor: "pointer",
            outline: "none",
            transition: "background 120ms ease",
          }}
          onMouseEnter={(e) => {
            (e.currentTarget as HTMLButtonElement).style.background = "#202838";
          }}
          onMouseLeave={(e) => {
            (e.currentTarget as HTMLButtonElement).style.background = "#171c26";
          }}
        >
          Refresh
        </button>
      </div>

      <div style={{ padding: "12px 14px" }}>
        {state.runs.map((run) => {
          const isSelected = state.selectedRun?.id === run.id;
          const config = STATUS_CONFIG[run.status];

          return (
            <div
              key={run.id}
              onClick={() => handleSelectRun(run)}
              style={{
                display: "flex",
                alignItems: "center",
                gap: "12px",
                padding: "12px 14px",
                marginBottom: "8px",
                borderRadius: "12px",
                border: `1px solid ${isSelected ? "#7185a8" : "#242b39"}`,
                background: isSelected ? "#202838" : "#171c26",
                cursor: "pointer",
                transition: "background 120ms ease, border-color 120ms ease",
              }}
              onMouseEnter={(e) => {
                if (!isSelected) {
                  (e.currentTarget as HTMLDivElement).style.background = "#202838";
                }
              }}
              onMouseLeave={(e) => {
                if (!isSelected) {
                  (e.currentTarget as HTMLDivElement).style.background = "#171c26";
                }
              }}
            >
              <span
                style={{
                  display: "inline-block",
                  borderRadius: "999px",
                  padding: "2px 8px",
                  fontSize: "12px",
                  fontWeight: 600,
                  background: config.bg,
                  color: config.color,
                  whiteSpace: "nowrap",
                  minWidth: "72px",
                  textAlign: "center",
                }}
              >
                {config.label}
              </span>

              <div style={{ flex: 1, minWidth: 0 }}>
                <div
                  style={{
                    fontSize: "13px",
                    color: "#c9d4e7",
                    fontFamily: "ui-monospace, SFMono-Regular, monospace",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {truncateId(run.id, 12)}
                </div>
                <div
                  style={{
                    fontSize: "12px",
                    color: "#8f98aa",
                    marginTop: "2px",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
                  {run.watchlistId} \u00b7 {run.presetId}
                </div>
              </div>

              <div
                style={{
                  display: "flex",
                  gap: "12px",
                  fontSize: "13px",
                  color: "#8f98aa",
                  whiteSpace: "nowrap",
                }}
              >
                <span>
                  {run.succeededSymbols}/{run.totalSymbols}
                </span>
                {run.failedSymbols > 0 ? (
                  <span style={{ color: "#ffb4be" }}>{run.failedSymbols} failed</span>
                ) : null}
              </div>

              <div
                style={{
                  fontSize: "12px",
                  color: "#8f98aa",
                  whiteSpace: "nowrap",
                  minWidth: "70px",
                  textAlign: "right",
                }}
              >
                {formatTimestamp(run.startedAt ?? run.finishedAt)}
              </div>
            </div>
          );
        })}
      </div>

      {state.selectedRun && (
        <div
          style={{
            padding: "14px",
            borderTop: "1px solid #242b39",
          }}
        >
          <div
            style={{
              display: "grid",
              gridTemplateColumns: "1fr 1fr",
              gap: "8px 16px",
              fontSize: "13px",
            }}
          >
            <div>
              <span style={{ color: "#8f98aa", fontSize: "12px" }}>Base trade date</span>
              <div style={{ color: "#c9d4e7", marginTop: "2px" }}>
                {state.selectedRun.baseTradeDate ?? "\u2014"}
              </div>
            </div>
            <div>
              <span style={{ color: "#8f98aa", fontSize: "12px" }}>Symbols</span>
              <div style={{ color: "#c9d4e7", marginTop: "2px" }}>
                {state.selectedRun.totalSymbols}
              </div>
            </div>
            <div style={{ gridColumn: "1 / -1" }}>
              <span style={{ color: "#8f98aa", fontSize: "12px" }}>Preset snapshot</span>
              <div
                style={{
                  color: "#c9d4e7",
                  marginTop: "2px",
                  fontFamily: "ui-monospace, SFMono-Regular, monospace",
                  fontSize: "12px",
                  overflow: "hidden",
                  textOverflow: "ellipsis",
                  whiteSpace: "nowrap",
                }}
              >
                {JSON.stringify(state.selectedRun.presetSnapshotJson)}
              </div>
            </div>
            {state.selectedRun.retryOfRunId && (
              <div style={{ gridColumn: "1 / -1" }}>
                <span style={{ color: "#8f98aa", fontSize: "12px" }}>Retry of</span>
                <div
                  style={{
                    color: "#c9d4e7",
                    marginTop: "2px",
                    fontFamily: "ui-monospace, SFMono-Regular, monospace",
                    fontSize: "12px",
                    overflow: "hidden",
                    textOverflow: "ellipsis",
                    whiteSpace: "nowrap",
                  }}
                >
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
