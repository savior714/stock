"use client";

import { useState, useMemo } from "react";

import type { ScanResult } from "./types";
import { filterResults, sortResults, filterByMatchMode } from "./model";
import type { ResultFilter, ResultSort } from "./model";

type ScanResultsTableProps = {
  results: ScanResult[];
  runId: string;
  isLoading?: boolean;
};

const SORT_OPTIONS: { value: ResultSort["field"]; label: string }[] = [
  { value: "symbol", label: "Symbol" },
  { value: "tradeDate", label: "Trade date" },
  { value: "currentPrice", label: "Price" },
  { value: "rsi", label: "RSI" },
  { value: "mfi", label: "MFI" },
  { value: "bollingerMiddle", label: "Bollinger Middle" },
  { value: "matchedCount", label: "Matched count" },
];

function formatNumber(value: number | null): string {
  if (value === null) return "\u2014";
  return value.toFixed(2);
}

function matchedCount(result: ScanResult): number {
  return [result.allConditionsMatched, result.anyConditionMatched].filter(Boolean).length;
}

export default function ScanResultsTable({ results, runId, isLoading }: ScanResultsTableProps) {
  const [matchMode, setMatchMode] = useState<"and" | "or" | "none">("none");
  const [includeStale, setIncludeStale] = useState(true);
  const [symbolFilter, setSymbolFilter] = useState("");
  const [sortField, setSortField] = useState<ResultSort["field"]>("symbol");
  const [sortDir, setSortDir] = useState<"asc" | "desc">("asc");

  const filtered = useMemo(
    () =>
      filterResults(results, { matchMode, includeStale, symbolFilter: symbolFilter || undefined }),
    [results, matchMode, includeStale, symbolFilter],
  );

  const filteredByMode = useMemo(
    () => filterByMatchMode(filtered, matchMode === "none" ? "none" : matchMode),
    [filtered, matchMode],
  );

  const sorted = useMemo(
    () => sortResults(filteredByMode, { field: sortField, direction: sortDir }),
    [filteredByMode, sortField, sortDir],
  );

  const handleSort = (field: ResultSort["field"]) => {
    if (sortField === field) {
      setSortDir((d) => (d === "asc" ? "desc" : "asc"));
    } else {
      setSortField(field);
      setSortDir("asc");
    }
  };

  const sortIndicator = (field: ResultSort["field"]) => {
    if (sortField !== field) return;
    return sortDir === "asc" ? "\u2191" : "\u2193";
  };

  if (isLoading) {
    return (
      <div className="panel" style={{ padding: "20px" }}>
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Scan results</p>
            <h3>Results for Run {runId}</h3>
          </div>
        </div>
        <p className="muted" style={{ padding: "20px 0", textAlign: "center" }}>
          Loading results\u2026
        </p>
      </div>
    );
  }

  if (results.length === 0) {
    return (
      <div className="panel" style={{ padding: "20px" }}>
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Scan results</p>
            <h3>Results for Run {runId}</h3>
          </div>
        </div>
        <div className="empty-state" style={{ minHeight: "200px" }}>
          <h3>No results</h3>
          <p>No scan results available.</p>
        </div>
      </div>
    );
  }

  return (
    <div className="panel scan-results-table" style={{ padding: "0 0 12px 0", overflow: "hidden" }}>
      <div className="panel-heading" style={{ padding: "20px 20px 0 20px" }}>
        <div>
          <p className="eyebrow">Scan results</p>
          <h3>Results for Run {runId}</h3>
        </div>
        <span style={{ fontSize: "12px", color: "#8f98aa" }}>
          {sorted.length} result{sorted.length !== 1 ? "s" : ""}
        </span>
      </div>

      <div
        style={{
          display: "flex",
          flexWrap: "wrap",
          gap: "10px",
          padding: "14px 20px",
          alignItems: "center",
          borderBottom: "1px solid #242b39",
        }}
      >
        <label
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: "6px",
            fontSize: "12px",
            color: "#8f98aa",
          }}
        >
          Match mode
          <select
            value={matchMode}
            onChange={(e) => setMatchMode(e.target.value as "and" | "or" | "none")}
            style={{
              border: "1px solid #303848",
              borderRadius: "8px",
              outline: "none",
              background: "#0d1118",
              color: "#c9d4e7",
              padding: "4px 8px",
              fontSize: "12px",
              minHeight: "28px",
            }}
          >
            <option value="none">All</option>
            <option value="and">AND only</option>
            <option value="or">OR only</option>
          </select>
        </label>

        <label
          style={{
            display: "inline-flex",
            alignItems: "center",
            gap: "6px",
            fontSize: "12px",
            color: "#8f98aa",
          }}
        >
          <input
            type="checkbox"
            checked={includeStale}
            onChange={(e) => setIncludeStale(e.target.checked)}
            style={{ accentColor: "#7185a8" }}
          />
          Include stale
        </label>

        <input
          type="text"
          placeholder="Filter by symbol"
          value={symbolFilter}
          onChange={(e) => setSymbolFilter(e.target.value)}
          style={{
            border: "1px solid #303848",
            borderRadius: "8px",
            outline: "none",
            background: "#0d1118",
            color: "#f5f7fb",
            padding: "4px 8px",
            fontSize: "12px",
            minHeight: "28px",
            width: "140px",
            marginLeft: "auto",
          }}
        />
      </div>

      <div style={{ overflowX: "auto" }}>
        <table
          style={{
            width: "100%",
            borderCollapse: "collapse",
            border: "1px solid #242b39",
            fontSize: "13px",
          }}
        >
          <thead>
            <tr style={{ background: "#171c26" }}>
              <th
                style={{
                  width: "80px",
                  padding: "8px 10px",
                  textAlign: "left",
                  fontSize: "12px",
                  color: "#8f98aa",
                  fontWeight: 500,
                  borderBottom: "1px solid #242b39",
                  cursor: "pointer",
                  userSelect: "none",
                }}
                onClick={() => handleSort("symbol")}
              >
                Symbol {sortIndicator("symbol")}
              </th>
              <th
                style={{
                  width: "110px",
                  padding: "8px 10px",
                  textAlign: "left",
                  fontSize: "12px",
                  color: "#8f98aa",
                  fontWeight: 500,
                  borderBottom: "1px solid #242b39",
                  cursor: "pointer",
                  userSelect: "none",
                }}
                onClick={() => handleSort("tradeDate")}
              >
                Trade date {sortIndicator("tradeDate")}
              </th>
              <th
                style={{
                  width: "90px",
                  padding: "8px 10px",
                  textAlign: "right",
                  fontSize: "12px",
                  color: "#8f98aa",
                  fontWeight: 500,
                  borderBottom: "1px solid #242b39",
                  cursor: "pointer",
                  userSelect: "none",
                }}
                onClick={() => handleSort("currentPrice")}
              >
                Price {sortIndicator("currentPrice")}
              </th>
              <th
                style={{
                  width: "80px",
                  padding: "8px 10px",
                  textAlign: "right",
                  fontSize: "12px",
                  color: "#8f98aa",
                  fontWeight: 500,
                  borderBottom: "1px solid #242b39",
                  cursor: "pointer",
                  userSelect: "none",
                }}
                onClick={() => handleSort("rsi")}
              >
                RSI {sortIndicator("rsi")}
              </th>
              <th
                style={{
                  width: "80px",
                  padding: "8px 10px",
                  textAlign: "right",
                  fontSize: "12px",
                  color: "#8f98aa",
                  fontWeight: 500,
                  borderBottom: "1px solid #242b39",
                  cursor: "pointer",
                  userSelect: "none",
                }}
                onClick={() => handleSort("mfi")}
              >
                MFI {sortIndicator("mfi")}
              </th>
              <th
                style={{
                  width: "80px",
                  padding: "8px 10px",
                  textAlign: "right",
                  fontSize: "12px",
                  color: "#8f98aa",
                  fontWeight: 500,
                  borderBottom: "1px solid #242b39",
                }}
              >
                BB lower
              </th>
              <th
                style={{
                  width: "80px",
                  padding: "8px 10px",
                  textAlign: "right",
                  fontSize: "12px",
                  color: "#8f98aa",
                  fontWeight: 500,
                  borderBottom: "1px solid #242b39",
                  cursor: "pointer",
                  userSelect: "none",
                }}
                onClick={() => handleSort("bollingerMiddle")}
              >
                BB middle {sortIndicator("bollingerMiddle")}
              </th>
              <th
                style={{
                  width: "80px",
                  padding: "8px 10px",
                  textAlign: "right",
                  fontSize: "12px",
                  color: "#8f98aa",
                  fontWeight: 500,
                  borderBottom: "1px solid #242b39",
                }}
              >
                BB upper
              </th>
              <th
                style={{
                  width: "60px",
                  padding: "8px 10px",
                  textAlign: "center",
                  fontSize: "12px",
                  color: "#8f98aa",
                  fontWeight: 500,
                  borderBottom: "1px solid #242b39",
                  cursor: "pointer",
                  userSelect: "none",
                }}
                onClick={() => handleSort("matchedCount")}
              >
                Matched {sortIndicator("matchedCount")}
              </th>
              <th
                style={{
                  width: "60px",
                  padding: "8px 10px",
                  textAlign: "center",
                  fontSize: "12px",
                  color: "#8f98aa",
                  fontWeight: 500,
                  borderBottom: "1px solid #242b39",
                }}
              >
                AND
              </th>
              <th
                style={{
                  width: "60px",
                  padding: "8px 10px",
                  textAlign: "center",
                  fontSize: "12px",
                  color: "#8f98aa",
                  fontWeight: 500,
                  borderBottom: "1px solid #242b39",
                }}
              >
                OR
              </th>
              <th
                style={{
                  width: "60px",
                  padding: "8px 10px",
                  textAlign: "center",
                  fontSize: "12px",
                  color: "#8f98aa",
                  fontWeight: 500,
                  borderBottom: "1px solid #242b39",
                }}
              >
                Stale
              </th>
            </tr>
          </thead>
          <tbody>
            {sorted.map((row) => (
              <tr
                key={`${row.symbol}-${row.tradeDate}`}
                style={{
                  background: row.dataStale ? "#2a171b" : undefined,
                  transition: "background 120ms ease",
                }}
                onMouseEnter={(e) => {
                  if (!row.dataStale) {
                    (e.currentTarget as HTMLTableRowElement).style.background = "#202838";
                  }
                }}
                onMouseLeave={(e) => {
                  if (!row.dataStale) {
                    (e.currentTarget as HTMLTableRowElement).style.background = "";
                  }
                }}
              >
                <td
                  style={{
                    padding: "7px 10px",
                    fontFamily: "ui-monospace, SFMono-Regular, monospace",
                    fontSize: "13px",
                    color: "#c9d4e7",
                    borderBottom: "1px solid #242b39",
                  }}
                >
                  {row.symbol}
                </td>
                <td
                  style={{
                    padding: "7px 10px",
                    fontSize: "13px",
                    color: "#c9d4e7",
                    borderBottom: "1px solid #242b39",
                  }}
                >
                  {row.tradeDate}
                </td>
                <td
                  style={{
                    padding: "7px 10px",
                    fontSize: "13px",
                    color: "#c9d4e7",
                    textAlign: "right",
                    borderBottom: "1px solid #242b39",
                  }}
                >
                  {formatNumber(row.currentPrice)}
                </td>
                <td
                  style={{
                    padding: "7px 10px",
                    fontSize: "13px",
                    color: "#c9d4e7",
                    textAlign: "right",
                    borderBottom: "1px solid #242b39",
                  }}
                >
                  {formatNumber(row.rsi)}
                </td>
                <td
                  style={{
                    padding: "7px 10px",
                    fontSize: "13px",
                    color: "#c9d4e7",
                    textAlign: "right",
                    borderBottom: "1px solid #242b39",
                  }}
                >
                  {formatNumber(row.mfi)}
                </td>
                <td
                  style={{
                    padding: "7px 10px",
                    fontSize: "13px",
                    color: "#c9d4e7",
                    textAlign: "right",
                    borderBottom: "1px solid #242b39",
                  }}
                >
                  {formatNumber(row.bollingerLower)}
                </td>
                <td
                  style={{
                    padding: "7px 10px",
                    fontSize: "13px",
                    color: "#c9d4e7",
                    textAlign: "right",
                    borderBottom: "1px solid #242b39",
                  }}
                >
                  {formatNumber(row.bollingerMiddle)}
                </td>
                <td
                  style={{
                    padding: "7px 10px",
                    fontSize: "13px",
                    color: "#c9d4e7",
                    textAlign: "right",
                    borderBottom: "1px solid #242b39",
                  }}
                >
                  {formatNumber(row.bollingerUpper)}
                </td>
                <td
                  style={{
                    padding: "7px 10px",
                    fontSize: "13px",
                    color: "#c9d4e7",
                    textAlign: "center",
                    borderBottom: "1px solid #242b39",
                  }}
                >
                  {matchedCount(row)}
                </td>
                <td
                  style={{
                    padding: "7px 10px",
                    fontSize: "13px",
                    textAlign: "center",
                    borderBottom: "1px solid #242b39",
                  }}
                >
                  <span
                    style={{
                      display: "inline-block",
                      minWidth: "20px",
                      borderRadius: "999px",
                      padding: "2px 6px",
                      background: row.allConditionsMatched ? "#14261f" : "#0e1219",
                      color: row.allConditionsMatched ? "#a7e5c8" : "#8f98aa",
                      fontSize: "12px",
                      fontWeight: 600,
                    }}
                  >
                    {row.allConditionsMatched ? "Y" : "N"}
                  </span>
                </td>
                <td
                  style={{
                    padding: "7px 10px",
                    fontSize: "13px",
                    textAlign: "center",
                    borderBottom: "1px solid #242b39",
                  }}
                >
                  <span
                    style={{
                      display: "inline-block",
                      minWidth: "20px",
                      borderRadius: "999px",
                      padding: "2px 6px",
                      background: row.anyConditionMatched ? "#14261f" : "#0e1219",
                      color: row.anyConditionMatched ? "#a7e5c8" : "#8f98aa",
                      fontSize: "12px",
                      fontWeight: 600,
                    }}
                  >
                    {row.anyConditionMatched ? "Y" : "N"}
                  </span>
                </td>
                <td
                  style={{
                    padding: "7px 10px",
                    fontSize: "13px",
                    textAlign: "center",
                    borderBottom: "1px solid #242b39",
                  }}
                >
                  {row.dataStale ? (
                    <span
                      style={{
                        display: "inline-block",
                        borderRadius: "999px",
                        padding: "2px 6px",
                        background: "#2a171b",
                        color: "#ffb4be",
                        fontSize: "12px",
                        fontWeight: 600,
                      }}
                    >
                      Stale
                    </span>
                  ) : (
                    <span style={{ color: "#8f98aa", fontSize: "12px" }}>\u2014</span>
                  )}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  );
}
