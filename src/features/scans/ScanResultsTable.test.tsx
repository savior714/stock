import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@/test/render";
import ScanResultsTable from "./ScanResultsTable";
import type { ScanRunDetail, ScanResult } from "./types";
import * as useScanLineageModule from "./useScanLineage";

vi.mock("./useScanLineage");

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

function makeResult(symbol: string, matchedCount: number): ScanResult {
  return {
    symbol,
    tradeDate: "2025-07-17",
    currentPrice: 150,
    rsi: 45,
    mfi: 50,
    bollingerLower: 130,
    bollingerMiddle: 150,
    bollingerUpper: 170,
    matchedConditionCount: matchedCount,
    allConditionsMatched: matchedCount > 0,
    anyConditionMatched: matchedCount > 0,
    dataStale: false,
  };
}

describe("ScanResultsTable — lineage navigation", () => {
  it("passes lineage run select to parent onRunSelect", () => {
    const original = makeRun("run-a");
    const retry = makeRun("run-b", "run-a");
    const handler = vi.fn();
    const mockLineage = vi.mocked(useScanLineageModule.useScanLineage);
    mockLineage.mockReturnValue({
      runs: [original, retry],
      isLoading: false,
      error: null,
    });

    const results: ScanResult[] = [makeResult("AAPL", 1)];

    render(
      <ScanResultsTable
        results={results}
        runId="run-b"
        run={retry}
        onRunSelect={handler}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /original/i }));
    expect(handler).toHaveBeenCalledWith(original);
  });

  it("works with empty results", () => {
    const original = makeRun("run-a");
    const retry = makeRun("run-b", "run-a");
    const handler = vi.fn();
    const mockLineage = vi.mocked(useScanLineageModule.useScanLineage);
    mockLineage.mockReturnValue({
      runs: [original, retry],
      isLoading: false,
      error: null,
    });

    render(
      <ScanResultsTable
        results={[]}
        runId="run-b"
        run={retry}
        onRunSelect={handler}
      />,
    );

    expect(screen.getByText(/no results/i)).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /original/i })).toBeInTheDocument();

    fireEvent.click(screen.getByRole("button", { name: /original/i }));
    expect(handler).toHaveBeenCalledWith(original);
  });

  it("shows lineage in loading state when run exists", () => {
    const original = makeRun("run-a");
    const retry = makeRun("run-b", "run-a");
    const mockLineage = vi.mocked(useScanLineageModule.useScanLineage);
    mockLineage.mockReturnValue({
      runs: [original, retry],
      isLoading: false,
      error: null,
    });

    const { container } = render(
      <ScanResultsTable
        results={[]}
        runId="run-b"
        run={retry}
        isLoading
      />,
    );

    expect(screen.getByText(/loading results/i)).toBeInTheDocument();
    expect(container.querySelector('[role="navigation"]')).toBeInTheDocument();
  });

  it("does not show lineage when run is null", () => {
    const mockLineage = vi.mocked(useScanLineageModule.useScanLineage);
    mockLineage.mockReturnValue({
      runs: [],
      isLoading: false,
      error: null,
    });

    const { container } = render(
      <ScanResultsTable
        results={[]}
        runId="run-b"
        run={null}
      />,
    );

    expect(container.querySelector('[role="navigation"]')).toBeNull();
  });

  it("no-op handler is not used — parent callback is called directly", () => {
    const original = makeRun("run-a");
    const retry = makeRun("run-b", "run-a");
    const mockLineage = vi.mocked(useScanLineageModule.useScanLineage);
    mockLineage.mockReturnValue({
      runs: [original, retry],
      isLoading: false,
      error: null,
    });

    let capturedRun: ScanRunDetail | undefined;
    render(
      <ScanResultsTable
        results={[makeResult("AAPL", 1)]}
        runId="run-b"
        run={retry}
        onRunSelect={(run) => {
          capturedRun = run;
        }}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /original/i }));
    expect(capturedRun?.id).toBe("run-a");
  });
});
