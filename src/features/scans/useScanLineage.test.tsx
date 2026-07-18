import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useScanLineage } from "./useScanLineage";
import type { ScanRunDetail } from "./types";
import * as api from "./api";

vi.mock("./api");

function makeRun(id: string, retryOfRunId: string | null = null): ScanRunDetail {
  return {
    id,
    watchlistId: "wl-1",
    presetId: "preset-1",
    status: "completed",
    baseTradeDate: "2025-07-17",
    totalSymbols: 5,
    succeededSymbols: 5,
    failedSymbols: 0,
    startedAt: "2025-07-17T00:00:00Z",
    finishedAt: "2025-07-17T00:01:00Z",
    presetSnapshotJson: {},
    symbolsSnapshotJson: [],
    retryOfRunId,
  };
}

function deferred<T>(): {
  promise: Promise<T>;
  resolve: (value: T) => void;
  reject: (reason?: unknown) => void;
} {
  let resolve!: (value: T) => void;
  let reject!: (reason?: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

describe("useScanLineage — stale race prevention", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  afterEach(() => {
    vi.clearAllMocks();
  });

  it("shows single run when no retry parent", async () => {
    const run = makeRun("run-only");

    const getScanRun = vi.mocked(api.getScanRun);
    getScanRun.mockResolvedValue(run);

    const { result } = renderHook(
      () => useScanLineage(run),
    );

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    expect(result.current.runs).toHaveLength(1);
    expect(result.current.runs[0]?.id).toBe("run-only");
    expect(result.current.isLoading).toBe(false);
  });

  it("shows empty when run is null", async () => {
    let capturedResult: ReturnType<typeof useScanLineage> | null = null;
    const { rerender } = renderHook(
      ({ run }: { run: ScanRunDetail | null }) => {
        const result = useScanLineage(run);
        capturedResult = result;
        return result;
      },
      { initialProps: { run: makeRun("run-a") } },
    );

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    rerender({ run: null });

    // Wait for microtask-deferred state update
    await act(async () => {
      await Promise.resolve();
    });

    expect(capturedResult?.runs).toHaveLength(0);
    expect(capturedResult?.isLoading).toBe(false);
  });

  it("builds lineage chain with parent", async () => {
    const child = makeRun("child-run", "parent-run");
    const parent = makeRun("parent-run");

    const getScanRun = vi.mocked(api.getScanRun);
    getScanRun.mockImplementation(async (id: string) => {
      await new Promise((resolve) => setTimeout(resolve, 5));
      return id === "parent-run" ? parent : child;
    });

    const { result } = renderHook(
      () => useScanLineage(child),
    );

    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 50));
    });

    expect(result.current.runs).toHaveLength(2);
    expect(result.current.runs[0]?.id).toBe("parent-run");
    expect(result.current.runs[1]?.id).toBe("child-run");
    expect(result.current.isLoading).toBe(false);
  });

  it("does not restore stale lineage after run becomes null", async () => {
    const parentA = makeRun("parent-a");
    const childA = makeRun("child-a", "parent-a");

    const getScanRun = vi.mocked(api.getScanRun);
    const parentDeferred = deferred<ScanRunDetail>();
    getScanRun.mockImplementation(async (id: string) => {
      if (id === "parent-a") {
        return parentDeferred.promise;
      }
      return childA;
    });

    const { result, rerender } = renderHook(
      ({ run }: { run: ScanRunDetail | null }) => {
        const r = useScanLineage(run);
        return r;
      },
      { initialProps: { run: childA } },
    );

    // Flush microtasks to start the fetch
    await act(async () => {
      await Promise.resolve();
    });

    // Verify loading is true and runs is still being built
    expect(result.current.isLoading).toBe(true);

    // Change run to null while parent fetch is pending
    rerender({ run: null });

    // After null transition, runs should be empty
    await act(async () => {
      await Promise.resolve();
    });
    expect(result.current.runs).toHaveLength(0);
    expect(result.current.isLoading).toBe(false);

    // Now resolve the stale parent fetch
    parentDeferred.resolve(parentA);

    // Flush microtasks
    await act(async () => {
      await Promise.resolve();
    });
    await act(async () => {
      await new Promise((resolve) => setTimeout(resolve, 10));
    });

    // Stale lineage should NOT be restored
    expect(result.current.runs).toHaveLength(0);
    expect(result.current.isLoading).toBe(false);
  });
});
