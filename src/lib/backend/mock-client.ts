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
      const runSummary = this.store.createRun(request, "pending");
      const runId = runSummary.id;

      const presetId = request.presetId;
      const resultsData = FIXED_RESULTS[presetId];
      const symbols = resultsData ? Object.keys(resultsData) : [];
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

      this.store.updateRun(runId, { status: "running" });
      this.emitEvent(runId, "scan://started", { runId, sequence: 1 });

      return runId;
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
