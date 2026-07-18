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
});
