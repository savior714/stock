import { describe, expect, it, vi, beforeEach } from "vitest";
import { renderHook, act } from "@testing-library/react";
import { useRunSelection } from "./useRunSelection";
import type { ScanRunDetail, ScanResult, ScanError } from "./types";
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

function makeResult(symbol: string): ScanResult {
  return {
    symbol,
    tradeDate: "2025-07-17",
    currentPrice: 150,
    rsi: 45,
    mfi: 50,
    bollingerLower: 130,
    bollingerMiddle: 150,
    bollingerUpper: 170,
    matchedConditionCount: 1,
    allConditionsMatched: true,
    anyConditionMatched: true,
    dataStale: false,
  };
}

function makeError(symbol: string): ScanError {
  return {
    symbol,
    code: "NETWORK_RETRY",
    message: "Retry limit",
    detail: "temp",
    retryable: true,
    attempt: 3,
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

describe("useRunSelection — stale race prevention", () => {
  beforeEach(() => {
    vi.clearAllMocks();
  });

  it("selects run and loads results", async () => {
    const run = makeRun("run-a");
    const resultsA: ScanResult[] = [makeResult("AAPL"), makeResult("MSFT")];
    const errorsA: ScanError[] = [];

    const getScanResults = vi.mocked(api.getScanResults);
    const getScanErrors = vi.mocked(api.getScanErrors);
    getScanResults.mockResolvedValue(resultsA);
    getScanErrors.mockResolvedValue(errorsA);

    const { result } = renderHook(() => useRunSelection());

    await act(async () => {
      await result.current.loadAndSelectRun(run, "Results");
    });

    expect(result.current.selectedRun?.id).toBe("run-a");
    expect(result.current.results).toEqual(resultsA);
    expect(result.current.errors).toEqual(errorsA);
    expect(result.current.isLoading).toBe(false);
  });

  it("stale A results do not overwrite B when B selected first", async () => {
    const runA = makeRun("run-a");
    const runB = makeRun("run-b");
    const resultsA: ScanResult[] = [makeResult("AAPL")];
    const resultsB: ScanResult[] = [makeResult("MSFT"), makeResult("NVDA")];
    const errorsA: ScanError[] = [];
    const errorsB: ScanError[] = [makeError("AMD")];

    const resultsADeferred = deferred<ScanResult[]>();
    const errorsADeferred = deferred<ScanError[]>();
    const resultsBDeferred = deferred<ScanResult[]>();
    const errorsBDeferred = deferred<ScanError[]>();

    const getScanResults = vi.mocked(api.getScanResults);
    const getScanErrors = vi.mocked(api.getScanErrors);

    getScanResults.mockImplementation(async (id: string) => {
      if (id === runA.id) return resultsADeferred.promise;
      return resultsBDeferred.promise;
    });
    getScanErrors.mockImplementation(async (id: string) => {
      if (id === runA.id) return errorsADeferred.promise;
      return errorsBDeferred.promise;
    });

    const { result } = renderHook(() => useRunSelection());

    // Select run A
    await act(async () => {
      result.current.loadAndSelectRun(runA, "Results");
    });

    // Select run B before A completes
    await act(async () => {
      result.current.loadAndSelectRun(runB, "Results");
    });

    // B resolves first
    resultsBDeferred.resolve(resultsB);
    errorsBDeferred.resolve(errorsB);

    await act(async () => {
      await Promise.resolve();
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });

    // B state should be visible
    expect(result.current.selectedRun?.id).toBe("run-b");
    expect(result.current.results).toEqual(resultsB);
    expect(result.current.errors).toEqual(errorsB);
    expect(result.current.isLoading).toBe(false);

    // Now A resolves — should NOT overwrite B
    resultsADeferred.resolve(resultsA);
    errorsADeferred.resolve(errorsA);

    await act(async () => {
      await Promise.resolve();
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });

    // B state should still be visible
    expect(result.current.selectedRun?.id).toBe("run-b");
    expect(result.current.results).toEqual(resultsB);
    expect(result.current.errors).toEqual(errorsB);
    expect(result.current.isLoading).toBe(false);
  });

  it("stale A finally does not prematurely end B loading", async () => {
    const runA = makeRun("run-a");
    const runB = makeRun("run-b");
    const resultsA: ScanResult[] = [makeResult("AAPL")];
    const resultsB: ScanResult[] = [makeResult("MSFT")];
    const errorsA: ScanError[] = [];
    const errorsB: ScanError[] = [];

    const resultsADeferred = deferred<ScanResult[]>();
    const errorsADeferred = deferred<ScanError[]>();
    const resultsBDeferred = deferred<ScanResult[]>();
    const errorsBDeferred = deferred<ScanError[]>();

    const getScanResults = vi.mocked(api.getScanResults);
    const getScanErrors = vi.mocked(api.getScanErrors);

    getScanResults.mockImplementation(async (id: string) => {
      if (id === runA.id) return resultsADeferred.promise;
      return resultsBDeferred.promise;
    });
    getScanErrors.mockImplementation(async (id: string) => {
      if (id === runA.id) return errorsADeferred.promise;
      return errorsBDeferred.promise;
    });

    const { result } = renderHook(() => useRunSelection());

    // Select run A
    await act(async () => {
      result.current.loadAndSelectRun(runA, "Results");
    });

    expect(result.current.isLoading).toBe(true);

    // Select run B before A completes
    await act(async () => {
      result.current.loadAndSelectRun(runB, "Results");
    });

    // A resolves first
    resultsADeferred.resolve(resultsA);
    errorsADeferred.resolve(errorsA);

    await act(async () => {
      await Promise.resolve();
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });

    // Loading should still be true because B is pending
    expect(result.current.isLoading).toBe(true);
    expect(result.current.selectedRun?.id).toBe("run-b");

    // Now B resolves
    resultsBDeferred.resolve(resultsB);
    errorsBDeferred.resolve(errorsB);

    await act(async () => {
      await Promise.resolve();
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });

    // Loading should now be false
    expect(result.current.isLoading).toBe(false);
    expect(result.current.results).toEqual(resultsB);
  });

  it("clearSelection resets all state", async () => {
    const run = makeRun("run-a");
    const resultsA: ScanResult[] = [makeResult("AAPL")];
    const errorsA: ScanError[] = [];

    const getScanResults = vi.mocked(api.getScanResults);
    const getScanErrors = vi.mocked(api.getScanErrors);
    getScanResults.mockResolvedValue(resultsA);
    getScanErrors.mockResolvedValue(errorsA);

    const { result } = renderHook(() => useRunSelection());

    await act(async () => {
      await result.current.loadAndSelectRun(run, "Results");
    });

    expect(result.current.selectedRun?.id).toBe("run-a");

    await act(async () => {
      result.current.clearSelection();
    });

    expect(result.current.selectedRun).toBeNull();
    expect(result.current.results).toEqual([]);
    expect(result.current.errors).toEqual([]);
  });

  it("invalidatePendingSelection blocks stale updates", async () => {
    const runA = makeRun("run-a");
    const resultsA: ScanResult[] = [makeResult("AAPL")];
    const errorsA: ScanError[] = [];

    const deferredResult = deferred<ScanResult[]>();
    const deferredError = deferred<ScanError[]>();

    const getScanResults = vi.mocked(api.getScanResults);
    const getScanErrors = vi.mocked(api.getScanErrors);
    getScanResults.mockReturnValue(deferredResult.promise);
    getScanErrors.mockReturnValue(deferredError.promise);

    const { result } = renderHook(() => useRunSelection());

    // Start loading run A
    await act(async () => {
      result.current.loadAndSelectRun(runA, "Results");
    });

    expect(result.current.isLoading).toBe(true);

    // Invalidate before A resolves
    await act(async () => {
      result.current.invalidatePendingSelection();
    });

    // Resolve A
    deferredResult.resolve(resultsA);
    deferredError.resolve(errorsA);

    await act(async () => {
      await Promise.resolve();
    });
    await act(async () => {
      await new Promise((r) => setTimeout(r, 10));
    });

    // Results should NOT have been set (stale update blocked)
    expect(result.current.results).toEqual([]);
    expect(result.current.errors).toEqual([]);
  });
});
