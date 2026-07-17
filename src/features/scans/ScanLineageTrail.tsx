"use client";

import type { ScanRunDetail } from "./types";
import styles from "./ScanLineageTrail.module.css";

type ScanLineageTrailProps = {
  runs: ScanRunDetail[];
  currentRunId: string;
  onRunSelect?: (run: ScanRunDetail) => void;
};

function getShortId(id: string): string {
  if (!id) return "";
  return id.slice(0, 8);
}

export default function ScanLineageTrail({
  runs,
  currentRunId,
  onRunSelect,
}: ScanLineageTrailProps) {
  if (!runs || runs.length === 0) return null;
  if (runs.length === 1 && !runs[0].retryOfRunId) return null;

  const handleClick = (run: ScanRunDetail) => {
    if (run.id === currentRunId) return;
    onRunSelect?.(run);
  };

  return (
    <div className={styles.lineageTrail} role="navigation" aria-label="Scan lineage">
      {runs.map((run, index) => {
        const isCurrent = run.id === currentRunId;
        const isRoot = index === 0;
        const label = isRoot
          ? "Original"
          : `Retry ${index}`;

        return (
          <div key={run.id} className={styles.lineageNode}>
            {!isRoot && <span className={styles.lineageArrow} aria-hidden="true">→</span>}
            <button
              type="button"
              className={`${styles.lineageButton}${isCurrent ? ` ${styles.lineageButtonCurrent}` : ""}`}
              onClick={() => handleClick(run)}
              disabled={isCurrent}
              title={`Run ID: ${run.id}`}
              aria-current={isCurrent ? "step" : undefined}
            >
              <span className={styles.lineageLabel}>
                {label}
                {!isCurrent && (
                  <span className={styles.lineageShortId}>
                    {getShortId(run.id)}
                  </span>
                )}
              </span>
            </button>
          </div>
        );
      })}
    </div>
  );
}
