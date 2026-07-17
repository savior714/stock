import type {
  WatchlistDetail,
  WatchlistInput,
  WatchlistSummary,
} from "@/features/watchlists/types";

import type {
  ScanPresetDetail,
  ScanPresetInput,
  ScanPresetSummary,
} from "@/features/scan-presets/types";

import type {
  ScanCancelledEvent,
  ScanCompletedEvent,
  ScanError,
  ScanErrorEvent,
  ScanEventPayload,
  ScanEventType,
  ScanProgressEvent,
  ScanResult,
  ScanResultEvent,
  ScanRunDetail,
  ScanRunSummary,
  ScanRunStatus,
  ScanStartedEvent,
  StartScanRequest,
} from "@/features/scans/types";

import type { BackendClient } from "./types";
import { createMockStore, type MockStore } from "./mock-store";
import { FIXED_RESULTS, INITIAL_PRESETS } from "./mock-fixtures";

let store: MockStore | null = null;

function getStore(): MockStore {
  if (!store) {
    store = createMockStore();
  }
  return store;
}

export function resetMockStore(): void {
  if (store) {
    store.reset();
  } else {
    store = createMockStore();
  }
}

class MockBackendClient implements BackendClient {
  private store: MockStore;
  private subscribers: Map<string, Set<(payload: ScanEventPayload) => void>> = new Map();
  private activeScans: Map<string, {
    symbols: string[];
    resultsData: Record<string, { result: ScanResult | null; error: ScanError | null; success: boolean }>;
    completed: number;
    succeeded: number;
    failed: number;
    seq: number;
    delayMs: number;
    presetId: string;
    runId: string;
    cancelled: boolean;
  }> = new Map();

  constructor() {
    this.store = getStore();
  }

  watchlists = {
    list: async (): Promise<WatchlistSummary[]> => {
      await delay(10);
      return this.store.watchlists;
    },

    get: async (id: string): Promise<WatchlistDetail> => {
      await delay(10);
      const detail = this.store.getWatchlistDetail(id);
      if (!detail) throw new Error(`Watchlist ${id} not found`);
      return detail;
    },

    create: async (input: WatchlistInput): Promise<WatchlistDetail> => {
      await delay(20);
      return this.store.createWatchlist(input);
    },

    update: async (
      id: string,
      input: WatchlistInput,
    ): Promise<WatchlistDetail> => {
      await delay(20);
      return this.store.updateWatchlist(id, input);
    },

    delete: async (id: string): Promise<void> => {
      await delay(20);
      this.store.deleteWatchlist(id);
    },
  };

  presets = {
    list: async (): Promise<ScanPresetSummary[]> => {
      await delay(10);
      return this.store.presets;
    },

    get: async (id: string): Promise<ScanPresetDetail> => {
      await delay(10);
      const detail = this.store.getPresetDetail(id);
      if (!detail) throw new Error(`Preset ${id} not found`);
      return detail;
    },

    create: async (input: ScanPresetInput): Promise<ScanPresetDetail> => {
      await delay(20);
      return this.store.createPreset(input);
    },

    update: async (
      id: string,
      input: ScanPresetInput,
    ): Promise<ScanPresetDetail> => {
      await delay(20);
      return this.store.updatePreset(id, input);
    },

    delete: async (id: string): Promise<void> => {
      await delay(20);
      this.store.deletePreset(id);
    },
  };

  scans = {
    start: async (request: StartScanRequest): Promise<string> => {
      await delay(30);

      // Get watchlist symbols for this run (not fixture symbols)
      const watchlistDetail = this.store.getWatchlistDetail(request.watchlistId);
      const symbols = watchlistDetail ? watchlistDetail.symbols : [];
      const presetDetail = this.store.getPresetDetail(request.presetId);
      const presetSnapshot = presetDetail ? JSON.stringify(presetDetail) : "{}";
      const symbolsSnapshot = JSON.stringify(symbols);

      const runSummary = this.store.createRun(
        request,
        "pending",
        {
          presetSnapshotJson: JSON.parse(presetSnapshot),
          symbolsSnapshotJson: JSON.parse(symbolsSnapshot),
          retryOfRunId: null,
        },
      );
      const runId = runSummary.id;

      const presetId = request.presetId;
      const fixtureData = FIXED_RESULTS[presetId];
      // Build resultsData only for watchlist symbols, with defaults for missing fixtures
      const resultsData: Record<string, { success: boolean; result: ScanResult | null; error: ScanError | null }> = {};
      const tradeDate = "2025-07-17";
      for (const sym of symbols) {
        const fixture = fixtureData?.[sym];
        if (fixture) {
          resultsData[sym] = {
            success: fixture.success,
            result: fixture.result as ScanResult | null,
            error: fixture.error as ScanError | null,
          };
        } else {
          // Default: success with deterministic data based on symbol hash
          const hash = sym.charCodeAt(0) % 3;
          resultsData[sym] = {
            success: hash !== 2,
            result: hash !== 2
              ? {
                  symbol: sym,
                  tradeDate,
                  currentPrice: 100 + hash * 50,
                  rsi: 30 + hash * 10,
                  mfi: 25 + hash * 8,
                  bollingerLower: 90 + hash * 10,
                  bollingerMiddle: 100 + hash * 12,
                  bollingerUpper: 110 + hash * 14,
                  matchedConditionCount: hash % 2 === 0 ? 1 : 0,
                  allConditionsMatched: hash % 2 === 0,
                  anyConditionMatched: true,
                  dataStale: false,
                }
              : null,
            error: hash === 2
              ? {
                  symbol: sym,
                  code: "NETWORK_RETRY",
                  message: "Yahoo Finance API retry limit exceeded",
                  detail: "temporary network error",
                  retryable: true,
                  attempt: 3,
                }
              : null,
          };
        }
      }
      const delayMs = presetId === "preset-4" ? 200 : 80;

      this.activeScans.set(runId, {
        symbols,
        resultsData,
        completed: 0,
        succeeded: 0,
        failed: 0,
        seq: 2,
        delayMs,
        presetId,
        runId,
        cancelled: false,
      });

      this.store.updateRun(runId, {
        status: "running",
        totalSymbols: symbols.length,
        presetSnapshotJson: JSON.parse(presetSnapshot),
        symbolsSnapshotJson: JSON.parse(symbolsSnapshot),
      });
      this.emitEvent(runId, "scan://started", { runId, sequence: 1 });

      return runId;
    },

    retry: async (runId: string): Promise<string> => {
      await delay(30);

      // Get original run detail
      const originalDetail = this.store.getRunDetail(runId);
      if (!originalDetail) throw new Error(`Run ${runId} not found`);

      // Validate original run status
      if (
        originalDetail.status !== "completed" &&
        originalDetail.status !== "failed"
      ) {
        throw new Error(
          `Run ${runId} is in ${originalDetail.status} state and cannot be retried`,
        );
      }

      // Get original errors and find retryable symbols
      const originalErrors = this.store.getErrors(runId);
      const retryableSet = new Set<string>();
      for (const err of originalErrors) {
        if (err.retryable && err.symbol !== null) {
          retryableSet.add(err.symbol);
        }
      }

      if (retryableSet.size === 0) {
        throw new Error("No retryable symbol errors found for this run");
      }

      // Intersect with original symbols snapshot, preserving order
      const originalSymbols = originalDetail.symbolsSnapshotJson as string[];
      const retrySymbols = originalSymbols.filter(
        (sym) => retryableSet.has(sym),
      );
      // Deduplicate while preserving order
      const seen = new Set<string>();
      const uniqueRetrySymbols = retrySymbols.filter((sym) => {
        if (seen.has(sym)) return false;
        seen.add(sym);
        return true;
      });

      if (uniqueRetrySymbols.length === 0) {
        throw new Error("No retryable symbol errors found for this run");
      }

      // Get preset snapshot from original run
      const presetSnapshot = originalDetail.presetSnapshotJson;

      // Create new retry run
      const retryRequest: StartScanRequest = {
        watchlistId: originalDetail.watchlistId,
        presetId: originalDetail.presetId,
      };
      const retrySummary = this.store.createRun(retryRequest, "pending", {
        presetSnapshotJson: presetSnapshot,
        symbolsSnapshotJson: uniqueRetrySymbols,
        retryOfRunId: runId,
        totalSymbols: uniqueRetrySymbols.length,
      });
      const retryRunId = retrySummary.id;

      // Build results data for retry symbols only
      const presetId = originalDetail.presetId;
      const fixtureData = FIXED_RESULTS[presetId];
      const tradeDate = "2025-07-17";
      const retryResultsData: Record<string, { success: boolean; result: ScanResult | null; error: ScanError | null }> = {};
      for (const sym of uniqueRetrySymbols) {
        const fixture = fixtureData?.[sym];
        if (fixture) {
          retryResultsData[sym] = {
            success: fixture.success,
            result: fixture.result as ScanResult | null,
            error: fixture.error as ScanError | null,
          };
        } else {
          const hash = sym.charCodeAt(0) % 3;
          retryResultsData[sym] = {
            success: hash !== 2,
            result: hash !== 2
              ? {
                  symbol: sym,
                  tradeDate,
                  currentPrice: 100 + hash * 50,
                  rsi: 30 + hash * 10,
                  mfi: 25 + hash * 8,
                  bollingerLower: 90 + hash * 10,
                  bollingerMiddle: 100 + hash * 12,
                  bollingerUpper: 110 + hash * 14,
                  matchedConditionCount: hash % 2 === 0 ? 1 : 0,
                  allConditionsMatched: hash % 2 === 0,
                  anyConditionMatched: true,
                  dataStale: false,
                }
              : null,
            error: hash === 2
              ? {
                  symbol: sym,
                  code: "NETWORK_RETRY",
                  message: "Yahoo Finance API retry limit exceeded",
                  detail: "temporary network error",
                  retryable: true,
                  attempt: 3,
                }
              : null,
          };
        }
      }

      const delayMs = presetId === "preset-4" ? 200 : 80;

      this.activeScans.set(retryRunId, {
        symbols: uniqueRetrySymbols,
        resultsData: retryResultsData,
        completed: 0,
        succeeded: 0,
        failed: 0,
        seq: 2,
        delayMs,
        presetId,
        runId: retryRunId,
        cancelled: false,
      });

      this.store.updateRun(retryRunId, {
        status: "running",
        totalSymbols: uniqueRetrySymbols.length,
      });
      this.emitEvent(retryRunId, "scan://started", {
        runId: retryRunId,
        sequence: 1,
      });

      return retryRunId;
    },

    listRuns: async (limit?: number): Promise<ScanRunSummary[]> => {
      await delay(10);
      const runs = this.store.runs;
      return limit ? runs.slice(0, limit) : runs;
    },

    getRun: async (runId: string): Promise<ScanRunDetail> => {
      await delay(10);
      const detail = this.store.getRunDetail(runId);
      if (!detail) throw new Error(`Run ${runId} not found`);
      return detail;
    },

    getResults: async (
      runId: string,
      _filter?: "and" | "or",
    ): Promise<ScanResult[]> => {
      await delay(10);
      return this.store.getResults(runId);
    },

    getErrors: async (runId: string): Promise<ScanError[]> => {
      await delay(10);
      return this.store.getErrors(runId);
    },

    cancel: async (runId: string): Promise<void> => {
      await delay(20);
      const scan = this.activeScans.get(runId);
      if (!scan) return;
      if (scan.completed === scan.symbols.length) return;

      scan.cancelled = true;
      this.store.updateRun(runId, {
        status: "cancelled",
        finishedAt: new Date().toISOString(),
      });
      this.emitEvent(runId, "scan://cancelled", { runId, sequence: 999 });
    },

    // Test-only: advance a scan by one symbol
    _tick: (runId: string): void => {
      const scan = this.activeScans.get(runId);
      if (!scan || scan.cancelled || scan.completed >= scan.symbols.length) return;

      const symbol = scan.symbols[scan.completed];
      const data = scan.resultsData[symbol];

      if (data.success) {
        scan.succeeded++;
      } else {
        scan.failed++;
      }
      scan.completed++;

      this.store.updateRun(scan.runId, {
        status: "running",
        totalSymbols: scan.symbols.length,
        succeededSymbols: scan.succeeded,
        failedSymbols: scan.failed,
      });

      this.emitEvent(scan.runId, "scan://progress", {
        runId: scan.runId,
        sequence: scan.seq++,
        completed: scan.completed,
        total: scan.symbols.length,
        succeeded: scan.succeeded,
        failed: scan.failed,
        currentSymbol: symbol,
      } as ScanProgressEvent);

      if (data.success) {
        this.emitEvent(scan.runId, "scan://result", {
          runId: scan.runId,
          sequence: scan.seq++,
          symbol,
          success: true,
        } as ScanResultEvent);
      } else if (data.error) {
        this.emitEvent(scan.runId, "scan://error", {
          runId: scan.runId,
          sequence: scan.seq++,
          symbol: data.error.symbol ?? null,
          code: data.error.code,
          message: data.error.message,
        } as ScanErrorEvent);
      }

      if (scan.completed === scan.symbols.length) {
        const finishedAt = new Date().toISOString();
        this.store.updateRun(scan.runId, {
          status: "completed",
          totalSymbols: scan.symbols.length,
          succeededSymbols: scan.succeeded,
          failedSymbols: scan.failed,
          finishedAt,
        });

        const results = scan.symbols
          .map((sym) => scan.resultsData[sym].result)
          .filter(Boolean) as ScanResult[];
        const errors = scan.symbols
          .map((sym) => scan.resultsData[sym].error)
          .filter(Boolean) as ScanError[];
        this.store.setResults(scan.runId, results);
        this.store.setErrors(scan.runId, errors);

        this.emitEvent(scan.runId, "scan://completed", {
          runId: scan.runId,
          sequence: scan.seq++,
          total: scan.symbols.length,
          succeeded: scan.succeeded,
          failed: scan.failed,
        } as ScanCompletedEvent);

        this.activeScans.delete(scan.runId);
      }
    },
  };

  events = {
    subscribe: async (
      eventType: ScanEventType,
      handler: (payload: ScanEventPayload) => void,
    ): Promise<() => void> => {
      if (!this.subscribers.has(eventType)) {
        this.subscribers.set(eventType, new Set());
      }
      this.subscribers.get(eventType)!.add(handler);
      return () => {
        const set = this.subscribers.get(eventType);
        if (set) {
          set.delete(handler);
        }
      };
    },
  };

  private emitEvent(
    runId: string,
    eventType: ScanEventType,
    payload: ScanEventPayload,
  ): void {
    const handlers = this.subscribers.get(eventType);
    if (handlers) {
      for (const h of handlers) {
        try {
          h(payload);
        } catch {
          // subscriber error — don't crash the mock
        }
      }
    }
  }
}

function delay(ms: number): Promise<void> {
  return new Promise((resolve) => {
    globalThis.setTimeout(resolve, ms);
  });
}

export { MockBackendClient };
