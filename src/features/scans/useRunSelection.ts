import { useCallback, useRef, useState } from "react";

import type { ScanRunDetail, ScanResult, ScanError } from "./types";
import { getScanResults, getScanErrors } from "./api";

export function useRunSelection() {
  const [selectedRun, setSelectedRun] = useState<ScanRunDetail | null>(null);
  const [results, setResults] = useState<ScanResult[]>([]);
  const [errors, setErrors] = useState<ScanError[]>([]);
  const [isLoading, setIsLoading] = useState(false);

  const runSelectionRequestRef = useRef(0);

  const loadAndSelectRun = useCallback(
    async (run: ScanRunDetail, destination: "Results" | "Logs") => {
      const requestId = ++runSelectionRequestRef.current;

      setSelectedRun(run);
      setIsLoading(true);

      try {
        const [nextResults, nextErrors] = await Promise.all([
          getScanResults(run.id),
          getScanErrors(run.id),
        ]);

        if (requestId !== runSelectionRequestRef.current) {
          return;
        }

        setResults(nextResults);
        setErrors(nextErrors);
      } catch {
        if (requestId !== runSelectionRequestRef.current) {
          return;
        }

        setResults([]);
        setErrors([]);
      } finally {
        if (requestId === runSelectionRequestRef.current) {
          setIsLoading(false);
        }
      }
    },
    [],
  );

  const invalidatePendingSelection = useCallback(() => {
    runSelectionRequestRef.current += 1;
  }, []);

  const clearSelection = useCallback(() => {
    invalidatePendingSelection();
    setSelectedRun(null);
    setResults([]);
    setErrors([]);
  }, [invalidatePendingSelection]);

  return {
    selectedRun,
    results,
    errors,
    isLoading,
    loadAndSelectRun,
    clearSelection,
    invalidatePendingSelection,
  };
}
