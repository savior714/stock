import { describe, expect, it, vi, beforeEach } from "vitest";
import { act, render, screen, fireEvent } from "@/test/render";
import ScanLogsPanel from "./ScanLogsPanel";
import type { ScanRunDetail, ScanError } from "./types";
import * as api from "./api";
import * as useScanLineageModule from "./useScanLineage";

vi.mock("./api");
vi.mock("./useScanLineage");
vi.mock("./ScanLogsPanel.module.css", () => ({ default: {} }));

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

describe("ScanLogsPanel — lineage navigation", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    const mockLineage = vi.mocked(useScanLineageModule.useScanLineage);
    mockLineage.mockReturnValue({
      runs: [],
      isLoading: false,
      error: null,
    });
    const getScanErrors = vi.mocked(api.getScanErrors);
    getScanErrors.mockResolvedValue([]);
    const getScanRun = vi.mocked(api.getScanRun);
    getScanRun.mockResolvedValue(
      makeRun("run-b", "run-a"),
    );
  });

  it("passes lineage run select to parent onRunSelect", async () => {
    const original = makeRun("run-a");
    const retry = makeRun("run-b", "run-a");
    const handler = vi.fn();
    const mockLineage = vi.mocked(useScanLineageModule.useScanLineage);
    mockLineage.mockReturnValue({
      runs: [original, retry],
      isLoading: false,
      error: null,
    });

    await act(async () => {
      render(
        <ScanLogsPanel
          runId="run-b"
          onRetry={() => {}}
          onRunSelect={handler}
        />,
      );
    });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    fireEvent.click(screen.getByRole("button", { name: /original/i }));
    expect(handler).toHaveBeenCalledWith(original);
  });

  it("current node is disabled", async () => {
    const original = makeRun("run-a");
    const retry = makeRun("run-b", "run-a");
    const mockLineage = vi.mocked(useScanLineageModule.useScanLineage);
    mockLineage.mockReturnValue({
      runs: [original, retry],
      isLoading: false,
      error: null,
    });

    await act(async () => {
      render(
        <ScanLogsPanel
          runId="run-b"
          onRetry={() => {}}
        />,
      );
    });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    const currentButton = screen.getByRole("button", { name: /retry 1/i });
    expect(currentButton).toBeDisabled();
  });

  it("retry button still works", async () => {
    const retryHandler = vi.fn();
    const getScanRun = vi.mocked(api.getScanRun);
    getScanRun.mockResolvedValue(makeRun("run-b"));

    await act(async () => {
      render(
        <ScanLogsPanel
          runId="run-b"
          onRetry={retryHandler}
        />,
      );
    });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    const errors: ScanError[] = [
      {
        symbol: "AAPL",
        code: "NETWORK_RETRY",
        message: "retry limit",
        detail: "temp",
        retryable: true,
        attempt: 3,
      },
    ];
    vi.mocked(api.getScanErrors).mockResolvedValue(errors);

    // Force re-render with errors
    await act(async () => {
      render(
        <ScanLogsPanel
          runId="run-b"
          onRetry={retryHandler}
        />,
      );
    });

    await act(async () => {
      await new Promise((r) => setTimeout(r, 50));
    });

    const retryButton = screen.getByRole("button", { name: /retry/i });
    expect(retryButton).not.toBeDisabled();
  });
});
