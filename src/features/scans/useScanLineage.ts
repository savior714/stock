import { useCallback, useEffect, useRef, useState } from "react";

import { getScanRun } from "@/features/scans/api";
import type { ScanRunDetail } from "@/features/scans/types";

const MAX_LINEAGE_DEPTH = 20;

const runCache = new Map<string, ScanRunDetail>();

function getCachedRun(runId: string): ScanRunDetail | undefined {
  return runCache.get(runId);
}

function setCachedRun(runId: string, run: ScanRunDetail): void {
  runCache.set(runId, run);
}

export function useScanLineage(run: ScanRunDetail | null) {
  const [runs, setRuns] = useState<ScanRunDetail[]>([]);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const requestRef = useRef(0);
  const pendingNullRef = useRef(false);

  const fetchLineage = useCallback(
    async (currentRun: ScanRunDetail, requestId: number) => {
      if (!currentRun?.retryOfRunId) {
        if (requestId !== requestRef.current) {
          return;
        }
        setRuns([currentRun]);
        setIsLoading(false);
        setError(null);
        return;
      }

      setIsLoading(true);
      setError(null);

      try {
        const visited = new Set<string>();
        const chain: ScanRunDetail[] = [];
        let current: ScanRunDetail | undefined = currentRun;

        while (current) {
          if (visited.has(current.id)) {
            break;
          }
          if (chain.length >= MAX_LINEAGE_DEPTH) {
            break;
          }

          visited.add(current.id);
          chain.unshift(current);

          const parentId = current.retryOfRunId;
          if (!parentId) break;

          const cached = getCachedRun(parentId);
          if (cached) {
            current = cached;
            continue;
          }

          try {
            const parent = await getScanRun(parentId);
            setCachedRun(parentId, parent);
            current = parent;
          } catch {
            break;
          }
        }

        if (requestId === requestRef.current) {
          setRuns(chain);
        }
      } catch (err) {
        if (requestId === requestRef.current) {
          setError(err instanceof Error ? err.message : "Failed to load lineage");
          setRuns(currentRun ? [currentRun] : []);
        }
      } finally {
        if (requestId === requestRef.current) {
          setIsLoading(false);
        }
      }
    },
    [],
  );

  useEffect(() => {
    const requestId = ++requestRef.current;

    if (!run) {
      pendingNullRef.current = true;
      // Defer state update to avoid cascading renders
      queueMicrotask(() => {
        if (pendingNullRef.current && requestId === requestRef.current) {
          setRuns([]);
          setIsLoading(false);
          setError(null);
        }
      });
      return;
    }

    pendingNullRef.current = false;
    void fetchLineage(run, requestId);
  }, [run, fetchLineage]);

  return { runs, isLoading, error };
}
