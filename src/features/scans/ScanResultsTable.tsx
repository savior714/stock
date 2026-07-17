"use client";

import { useState, useMemo } from "react";

import type { ScanResult } from "./types";
import { filterResults, sortResults, filterByMatchMode } from "./model";
import type { ResultFilter, ResultSort } from "./model";
import styles from "./ScanResultsTable.module.css";

const SORT_OPTIONS: { value: ResultSort["field"]; label: string }[] = [
  { value: "symbol", label: "Symbol" },
  { value: "tradeDate", label: "Trade date" },
  { value: "currentPrice", label: "Price" },
  { value: "rsi", label: "RSI" },
  { value: "mfi", label: "MFI" },
  { value: "bollingerMiddle", label: "Bollinger Middle" },
];

function formatNumber(value: number | null): string {
  if (value === null) return "\u2014";
  return value.toFixed(2);
}

export default function ScanResultsTable({ results, runId, isLoading }: {
  results: ScanResult[];
  runId: string;
  isLoading?: boolean;
}) {
  const [matchMode, setMatchMode] = useState<"and" | "or" | "none">("none");
  const [includeStale, setIncludeStale] = useState(true);
  const [symbolFilter, setSymbolFilter] = useState("");
  const [sortField, setSortField] = useState<ResultSort["field"]>("symbol");
  const [sortDir, setSortDir] = useState<"asc" | "desc">("asc");

  const filtered = useMemo(
    () =>
      filterResults(results, {
        matchMode,
        includeStale,
        symbolFilter: symbolFilter || undefined,
      }),
    [results, matchMode, includeStale, symbolFilter],
  );

  const filteredByMode = useMemo(
    () =>
      filterByMatchMode(
        filtered,
        matchMode === "none" ? "none" : matchMode,
      ),
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
      <div className="panel">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Scan results</p>
            <h3>Results for Run {runId}</h3>
          </div>
        </div>
        <p className={styles.mutedCenter}>Loading results…</p>
      </div>
    );
  }

  if (results.length === 0) {
    return (
      <div className="panel">
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
    <div className={`panel ${styles.scanResultsTable}`}>
      <div className="panel-heading">
        <div>
          <p className="eyebrow">Scan results</p>
          <h3>Results for Run {runId}</h3>
        </div>
        <span className={styles.resultCount}>
          {sorted.length} result{sorted.length !== 1 ? "s" : ""}
        </span>
      </div>

      <div className={styles.filterBar}>
        <label className={styles.filterLabel}>
          Match mode
          <select
            value={matchMode}
            onChange={(e) => setMatchMode(e.target.value as "and" | "or" | "none")}
          >
            <option value="none">All</option>
            <option value="and">AND only</option>
            <option value="or">OR only</option>
          </select>
        </label>

        <label className={styles.filterLabel}>
          <input
            type="checkbox"
            checked={includeStale}
            onChange={(e) => setIncludeStale(e.target.checked)}
          />
          Include stale
        </label>

        <input
          type="text"
          placeholder="Filter by symbol"
          value={symbolFilter}
          onChange={(e) => setSymbolFilter(e.target.value)}
          className={styles.symbolFilter}
        />
      </div>

      <div style={{ overflowX: "auto" }}>
        <table className={styles.table}>
          <thead>
            <tr>
              <th
                className={styles.th}
                style={{ width: "80px" }}
                onClick={() => handleSort("symbol")}
              >
                Symbol {sortIndicator("symbol")}
              </th>
              <th
                className={styles.th}
                style={{ width: "110px" }}
                onClick={() => handleSort("tradeDate")}
              >
                Trade date {sortIndicator("tradeDate")}
              </th>
              <th
                className={`${styles.th} ${styles.thRight}`}
                style={{ width: "90px" }}
                onClick={() => handleSort("currentPrice")}
              >
                Price {sortIndicator("currentPrice")}
              </th>
              <th
                className={`${styles.th} ${styles.thRight}`}
                style={{ width: "80px" }}
                onClick={() => handleSort("rsi")}
              >
                RSI {sortIndicator("rsi")}
              </th>
              <th
                className={`${styles.th} ${styles.thRight}`}
                style={{ width: "80px" }}
                onClick={() => handleSort("mfi")}
              >
                MFI {sortIndicator("mfi")}
              </th>
              <th
                className={`${styles.th} ${styles.thRight}`}
                style={{ width: "80px" }}
              >
                BB lower
              </th>
              <th
                className={`${styles.th} ${styles.thRight}`}
                style={{ width: "80px" }}
                onClick={() => handleSort("bollingerMiddle")}
              >
                BB middle {sortIndicator("bollingerMiddle")}
              </th>
              <th
                className={`${styles.th} ${styles.thRight}`}
                style={{ width: "80px" }}
              >
                BB upper
              </th>
              <th
                className={`${styles.th} ${styles.thCenter}`}
                style={{ width: "60px" }}
              >
                AND
              </th>
              <th
                className={`${styles.th} ${styles.thCenter}`}
                style={{ width: "60px" }}
              >
                OR
              </th>
              <th
                className={`${styles.th} ${styles.thCenter}`}
                style={{ width: "60px" }}
              >
                Stale
              </th>
            </tr>
          </thead>
          <tbody>
            {sorted.map((row) => (
              <tr
                key={`${row.symbol}-${row.tradeDate}`}
                className={`${styles.row}${row.dataStale ? ` ${styles.rowStale}` : ""}`}
              >
                <td className={styles.cell}>{row.symbol}</td>
                <td className={styles.cell}>{row.tradeDate}</td>
                <td className={`${styles.cell} ${styles.cellRight}`}>
                  {formatNumber(row.currentPrice)}
                </td>
                <td className={`${styles.cell} ${styles.cellRight}`}>
                  {formatNumber(row.rsi)}
                </td>
                <td className={`${styles.cell} ${styles.cellRight}`}>
                  {formatNumber(row.mfi)}
                </td>
                <td className={`${styles.cell} ${styles.cellRight}`}>
                  {formatNumber(row.bollingerLower)}
                </td>
                <td className={`${styles.cell} ${styles.cellRight}`}>
                  {formatNumber(row.bollingerMiddle)}
                </td>
                <td className={`${styles.cell} ${styles.cellRight}`}>
                  {formatNumber(row.bollingerUpper)}
                </td>
                <td className={`${styles.cell} ${styles.cellCenter}`}>
                  <span
                    className={`${styles.badge}${row.allConditionsMatched ? ` ${styles.badgeSuccess}` : ""}`}
                  >
                    {row.allConditionsMatched ? "Y" : "N"}
                  </span>
                </td>
                <td className={`${styles.cell} ${styles.cellCenter}`}>
                  <span
                    className={`${styles.badge}${row.anyConditionMatched ? ` ${styles.badgeSuccess}` : ""}`}
                  >
                    {row.anyConditionMatched ? "Y" : "N"}
                  </span>
                </td>
                <td className={`${styles.cell} ${styles.cellCenter}`}>
                  {row.dataStale ? (
                    <span className={`${styles.badge} ${styles.badgeDanger}`}>
                      Stale
                    </span>
                  ) : (
                    <span className={styles.badgeMuted}>—</span>
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
