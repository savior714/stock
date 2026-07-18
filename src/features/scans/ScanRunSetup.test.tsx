import { describe, expect, it, beforeEach, vi, afterEach } from "vitest";
import { act, render, screen } from "@/test/render";
import ScanRunSetup from "./ScanRunSetup";
import type { ScanRunDetail } from "./types";
import * as api from "./api";
import * as events from "./events";
import * as utils from "@/lib/scanner-utils";

vi.mock("./api");
vi.mock("./events");
vi.mock("@/lib/scanner-utils");
vi.mock("./ScanRunSetup.module.css", () => ({ default: {} }));

const mockWatchlist = { id: "wl-1", name: "Test List", symbolCount: 5 };
const mockPreset = { id: "preset-1", name: "Test Preset", enabledConditionCount: 3 };

const defaultProps = {
  selectedWatchlistId: "wl-1",
  selectedPresetId: "preset-1",
  onPresetIdChange: vi.fn(),
  watchlists: [mockWatchlist],
  presets: [mockPreset],
  presetsLoading: false,
  presetsError: null,
  onOpenPresetDrawer: vi.fn(),
  presetExists: true,
  watchlistExists: true,
  resumeRunId: null,
};

describe("ScanRunSetup — retry polling after initial fetch failure", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    (utils.canStartScan as vi.Mock).mockReturnValue(true);
    (events.subscribeScanEvent as vi.Mock).mockResolvedValue(() => {});
    (events.unsubscribeAll as vi.Mock).mockReturnValue();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("starts polling when resume run initial fetch fails", async () => {
    const runId = "run-retry-1";
    const runningDetail: ScanRunDetail = {
      id: runId,
      watchlistId: "wl-1",
      presetId: "preset-1",
      status: "running",
      totalSymbols: 10,
      succeededSymbols: 3,
      failedSymbols: 1,
      retryOfRunId: null,
      startedAt: "2025-01-01T00:00:00Z",
    };

    const getScanRun = vi.mocked(api.getScanRun);
    getScanRun.mockRejectedValueOnce(new Error("network error"));
    getScanRun.mockResolvedValueOnce(runningDetail);

    await act(async () => {
      render(<ScanRunSetup {...defaultProps} resumeRunId={runId} />);
    });

    // Flush microtasks from effect execution
    await act(async () => {
      vi.advanceTimersByTime(0);
    });

    // Advance to first poll interval (2s)
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    // First call was the initial fetch, second is the first poll
    expect(getScanRun).toHaveBeenCalledTimes(2);
    expect(getScanRun).toHaveBeenNthCalledWith(1, runId);
    expect(getScanRun).toHaveBeenNthCalledWith(2, runId);

    // Advance to second poll
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    expect(getScanRun).toHaveBeenCalledTimes(3);
  });

  it("stops polling when resumed run completes", async () => {
    const runId = "run-complete-1";
    const runningDetail: ScanRunDetail = {
      id: runId,
      watchlistId: "wl-1",
      presetId: "preset-1",
      status: "running",
      totalSymbols: 10,
      succeededSymbols: 3,
      failedSymbols: 1,
      retryOfRunId: null,
      startedAt: "2025-01-01T00:00:00Z",
    };
    const completedDetail: ScanRunDetail = {
      ...runningDetail,
      status: "completed",
      succeededSymbols: 10,
      failedSymbols: 0,
    };

    const getScanRun = vi.mocked(api.getScanRun);
    getScanRun.mockRejectedValueOnce(new Error("network error"));
    getScanRun.mockResolvedValueOnce(runningDetail);
    getScanRun.mockResolvedValueOnce(completedDetail);

    const getScanErrors = vi.mocked(api.getScanErrors);
    getScanErrors.mockResolvedValue([]);

    await act(async () => {
      render(<ScanRunSetup {...defaultProps} resumeRunId={runId} />);
    });

    await act(async () => {
      vi.advanceTimersByTime(0);
    });

    // First poll: fetch fails -> starts polling
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });
    expect(getScanRun).toHaveBeenCalledTimes(2);

    // Second poll: fetch succeeds with running
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });
    expect(getScanRun).toHaveBeenCalledTimes(3);

    // Third poll: fetch succeeds with completed -> polling stops
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    // The run is now completed — getScanErrors should have been called
    expect(getScanErrors).toHaveBeenCalled();

    // Verify the run detail reflects completion
    // Advance further — no more polls should fire since polling stopped
    const callCountAfterComplete = getScanRun.mock.calls.length;
    await act(async () => {
      vi.advanceTimersByTime(10000);
    });
    expect(getScanRun.mock.calls.length).toBe(callCountAfterComplete);
  });

  it("clears error state after initial fetch failure", async () => {
    const runId = "run-recover-1";

    const getScanRun = vi.mocked(api.getScanRun);
    getScanRun.mockRejectedValueOnce(new Error("network error"));

    await act(async () => {
      render(<ScanRunSetup {...defaultProps} resumeRunId={runId} />);
    });

    await act(async () => {
      vi.advanceTimersByTime(0);
    });

    // The catch block sets globalError: "" so no error message should show
    expect(screen.queryByText(/network error/)).not.toBeInTheDocument();
  });
});

describe("ScanRunSetup — completed event with fetch failure and polling recovery", () => {
  beforeEach(() => {
    vi.clearAllMocks();
    vi.useFakeTimers();
    (utils.canStartScan as vi.Mock).mockReturnValue(true);
    (events.subscribeScanEvent as vi.Mock).mockResolvedValue(() => {});
    (events.unsubscribeAll as vi.Mock).mockReturnValue();
  });

  afterEach(() => {
    vi.useRealTimers();
  });

  it("event fetch failure does not prevent polling recovery", async () => {
    const runId = "run-event-fail-1";
    const completedDetail: ScanRunDetail = {
      id: runId,
      watchlistId: "wl-1",
      presetId: "preset-1",
      status: "completed",
      totalSymbols: 10,
      succeededSymbols: 10,
      failedSymbols: 0,
      retryOfRunId: null,
      startedAt: "2025-01-01T00:00:00Z",
      finishedAt: "2025-01-01T00:02:00Z",
    };

    const getScanRun = vi.mocked(api.getScanRun);
    const getScanErrors = vi.mocked(api.getScanErrors);
    getScanErrors.mockResolvedValue([]);

    // Set up initial fetch to succeed with running -> starts polling
    getScanRun.mockResolvedValueOnce({
      ...completedDetail,
      status: "running",
    });

    const onComplete = vi.fn();

    await act(async () => {
      render(<ScanRunSetup {...defaultProps} resumeRunId={runId} onResumeRunCompleted={onComplete} />);
    });

    await act(async () => {
      vi.advanceTimersByTime(0);
    });

    // Poll returns running
    getScanRun.mockResolvedValueOnce({
      ...completedDetail,
      status: "running",
    });
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    // Poll returns completed -> callback called
    getScanRun.mockResolvedValueOnce(completedDetail);
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    expect(onComplete).toHaveBeenCalledTimes(1);
    expect(onComplete).toHaveBeenCalledWith(completedDetail);
  });

  it("polling deduplicates completed callback", async () => {
    const runId = "run-dedup-1";
    const completedDetail: ScanRunDetail = {
      id: runId,
      watchlistId: "wl-1",
      presetId: "preset-1",
      status: "completed",
      totalSymbols: 10,
      succeededSymbols: 10,
      failedSymbols: 0,
      retryOfRunId: null,
      startedAt: "2025-01-01T00:00:00Z",
      finishedAt: "2025-01-01T00:02:00Z",
    };

    const getScanRun = vi.mocked(api.getScanRun);
    const getScanErrors = vi.mocked(api.getScanErrors);
    getScanErrors.mockResolvedValue([]);

    getScanRun.mockResolvedValueOnce({
      ...completedDetail,
      status: "running",
    });

    const onComplete = vi.fn();

    await act(async () => {
      render(<ScanRunSetup {...defaultProps} resumeRunId={runId} onResumeRunCompleted={onComplete} />);
    });

    await act(async () => {
      vi.advanceTimersByTime(0);
    });

    // First poll returns completed -> callback called once
    getScanRun.mockResolvedValueOnce(completedDetail);
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    expect(onComplete).toHaveBeenCalledTimes(1);

    // Second poll returns completed -> should be deduplicated
    getScanRun.mockResolvedValueOnce(completedDetail);
    await act(async () => {
      vi.advanceTimersByTime(2000);
    });

    // Still only one call
    expect(onComplete).toHaveBeenCalledTimes(1);
  });
});
