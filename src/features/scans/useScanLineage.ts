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
  const mountedRef = useRef(true);
  const runIdRef = useRef<string | null>(null);
  const triggerRef = useRef(0);

  const fetchLineage = useCallback(async (currentRun: ScanRunDetail) => {
    if (!mountedRef.current) return;

    if (!currentRun?.retryOfRunId) {
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

      if (mountedRef.current) {
        setRuns(chain);
      }
    } catch (err) {
      if (mountedRef.current) {
        setError(err instanceof Error ? err.message : "Failed to load lineage");
        setRuns(currentRun ? [currentRun] : []);
      }
    } finally {
      if (mountedRef.current) {
        setIsLoading(false);
      }
    }
  }, []);

  useEffect(() => {
    mountedRef.current = true;
    runIdRef.current = run?.id ?? null;
    triggerRef.current += 1;
    return () => {
      mountedRef.current = false;
    };
  }, [run]);

  useEffect(() => {
    if (!run) {
      // eslint-disable-next-line react-hooks/set-state-in-effect
      setRuns([]);
      return;
    }
    fetchLineage(run);
  }, [run, fetchLineage, triggerRef]);

  return { runs, isLoading, error };
}
