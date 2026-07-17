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
  ScanError,
  ScanEventPayload,
  ScanEventType,
  ScanResult,
  ScanRunDetail,
  ScanRunSummary,
  ScanRunStatus,
  StartScanRequest,
} from "@/features/scans/types";

import { INITIAL_WATCHLISTS, INITIAL_PRESETS } from "./mock-fixtures";

const STORAGE_KEY = "stock.mock.backend.v2";

interface StoredState {
  version: 2;
  watchlists: WatchlistSummary[];
  watchlistDetails: Record<string, Omit<WatchlistDetail, "id">>;
  presets: ScanPresetSummary[];
  presetDetails: Record<string, Omit<ScanPresetDetail, "id">>;
  runs: ScanRunSummary[];
  runDetails: Record<string, Omit<ScanRunDetail, "id">>;
  results: Record<string, ScanResult[]>;
  errors: Record<string, ScanError[]>;
  nextWlId: number;
  nextPresetId: number;
  nextRunId: number;
}

function createInitialState(): StoredState {
  return {
    version: 2,
    watchlists: INITIAL_WATCHLISTS,
    watchlistDetails: Object.fromEntries(
      INITIAL_WATCHLISTS.map((wl) => [wl.id, {
        name: wl.name,
        description: wl.description,
        symbols: wl._symbols ?? [],
      }]),
    ),
    presets: INITIAL_PRESETS,
    presetDetails: Object.fromEntries(
      INITIAL_PRESETS.map((p) => [p.id, {
        name: p.name,
        conditions: [],
      }]),
    ),
    runs: [],
    runDetails: {},
    results: {},
    errors: {},
    nextWlId: 100,
    nextPresetId: 100,
    nextRunId: 100,
  };
}

function loadState(): StoredState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) return createInitialState();
    const parsed = JSON.parse(raw) as StoredState;
    if (parsed.version !== 2) return createInitialState();
    return parsed;
  } catch {
    return createInitialState();
  }
}

function saveState(state: StoredState): void {
  try {
    localStorage.setItem(STORAGE_KEY, JSON.stringify(state));
  } catch {
    // storage full or unavailable — silently ignore
  }
}

export interface MockStore {
  watchlists: WatchlistSummary[];
  presets: ScanPresetSummary[];
  runs: ScanRunSummary[];
  getWatchlistDetail(id: string): WatchlistDetail | undefined;
  getPresetDetail(id: string): ScanPresetDetail | undefined;
  getRunDetail(runId: string): ScanRunDetail | undefined;
  getResults(runId: string): ScanResult[];
  getErrors(runId: string): ScanError[];
  createWatchlist(input: WatchlistInput): WatchlistDetail;
  updateWatchlist(id: string, input: WatchlistInput): WatchlistDetail;
  deleteWatchlist(id: string): void;
  createPreset(input: ScanPresetInput): ScanPresetDetail;
  updatePreset(id: string, input: ScanPresetInput): ScanPresetDetail;
  deletePreset(id: string): void;
  createRun(request: StartScanRequest, status: ScanRunStatus, extra?: Partial<ScanRunDetail>): ScanRunSummary;
  updateRun(runId: string, updates: Partial<ScanRunSummary> & Partial<ScanRunDetail>): void;
  setResults(runId: string, results: ScanResult[]): void;
  setErrors(runId: string, errors: ScanError[]): void;
  reset(): void;
}

let cached: StoredState | null = null;

function getState(): StoredState {
  if (!cached) {
    cached = loadState();
  }
  return cached;
}

function invalidateCache(): void {
  cached = null;
}

export function createMockStore(): MockStore {
  let state = getState();

  function persist(): void {
    saveState(state);
    invalidateCache();
  }

  return {
    get watchlists() {
      return state.watchlists;
    },

    get presets() {
      return state.presets;
    },

    get runs() {
      return state.runs;
    },

    getWatchlistDetail(id: string): WatchlistDetail | undefined {
      const base = state.watchlistDetails[id];
      if (!base) return undefined;
      return { id, ...base };
    },

    getPresetDetail(id: string): ScanPresetDetail | undefined {
      const base = state.presetDetails[id];
      if (!base) return undefined;
      return { id, ...base };
    },

    getRunDetail(runId: string): ScanRunDetail | undefined {
      const base = state.runDetails[runId];
      if (!base) return undefined;
      return { id: runId, ...base };
    },

    getResults(runId: string): ScanResult[] {
      return state.results[runId] ?? [];
    },

    getErrors(runId: string): ScanError[] {
      return state.errors[runId] ?? [];
    },

    createWatchlist(input: WatchlistInput): WatchlistDetail {
      const id = `mock-wl-${state.nextWlId++}`;
      const summary: WatchlistSummary = {
        id,
        name: input.name,
        description: input.description,
        symbolCount: input.symbols.length,
      };
      state.watchlists.push(summary);
      state.watchlistDetails[id] = {
        name: input.name,
        description: input.description,
        symbols: [...input.symbols],
      };
      persist();
      return { id, ...state.watchlistDetails[id] };
    },

    updateWatchlist(id: string, input: WatchlistInput): WatchlistDetail {
      const existing = state.watchlistDetails[id];
      if (!existing) throw new Error(`Watchlist ${id} not found`);
      const updated = { ...existing, name: input.name, description: input.description, symbols: [...input.symbols] };
      state.watchlistDetails[id] = updated;
      const wlIdx = state.watchlists.findIndex((w) => w.id === id);
      if (wlIdx >= 0) {
        state.watchlists[wlIdx] = {
          ...state.watchlists[wlIdx],
          name: input.name,
          description: input.description,
          symbolCount: input.symbols.length,
        };
      }
      persist();
      return { id, ...updated };
    },

    deleteWatchlist(id: string): void {
      state.watchlists = state.watchlists.filter((w) => w.id !== id);
      delete state.watchlistDetails[id];
      persist();
    },

    createPreset(input: ScanPresetInput): ScanPresetDetail {
      const id = `mock-preset-${state.nextPresetId++}`;
      const enabledCount = input.conditions.filter((c) => c.enabled).length;
      const summary: ScanPresetSummary = {
        id,
        name: input.name,
        enabledConditionCount: enabledCount,
      };
      state.presets.push(summary);
      state.presetDetails[id] = {
        name: input.name,
        conditions: input.conditions.map((c) => ({
          ...c,
          threshold: c.threshold ?? null,
          stdDevMultiplier: c.stdDevMultiplier ?? null,
        })),
      };
      persist();
      return { id, ...state.presetDetails[id] };
    },

    updatePreset(id: string, input: ScanPresetInput): ScanPresetDetail {
      const existing = state.presetDetails[id];
      if (!existing) throw new Error(`Preset ${id} not found`);
      const enabledCount = input.conditions.filter((c) => c.enabled).length;
      const updated = {
        ...existing,
        name: input.name,
        conditions: input.conditions.map((c) => ({
          ...c,
          threshold: c.threshold ?? null,
          stdDevMultiplier: c.stdDevMultiplier ?? null,
        })),
      };
      state.presetDetails[id] = updated;
      const pIdx = state.presets.findIndex((p) => p.id === id);
      if (pIdx >= 0) {
        state.presets[pIdx] = {
          ...state.presets[pIdx],
          name: input.name,
          enabledConditionCount: enabledCount,
        };
      }
      persist();
      return { id, ...updated };
    },

    deletePreset(id: string): void {
      state.presets = state.presets.filter((p) => p.id !== id);
      delete state.presetDetails[id];
      persist();
    },

    createRun(
      request: StartScanRequest,
      status: ScanRunStatus,
      extra: Partial<ScanRunDetail> = {},
    ): ScanRunSummary {
      const now = new Date().toISOString();
      const runId = `mock-run-${state.nextRunId++}`;
      const watchlist = state.watchlists.find((w) => w.id === request.watchlistId);
      const totalSymbols = extra.totalSymbols ?? watchlist?.symbolCount ?? 0;
      const summary: ScanRunSummary = {
        id: runId,
        watchlistId: request.watchlistId,
        presetId: request.presetId,
        status,
        totalSymbols,
        succeededSymbols: 0,
        failedSymbols: 0,
        startedAt: status === "running" || status === "completed" || status === "cancelled" || status === "failed" ? now : null,
        finishedAt: status === "completed" || status === "cancelled" || status === "failed" ? now : null,
      };
      state.runs.unshift(summary);
      state.runDetails[runId] = {
        watchlistId: request.watchlistId,
        presetId: request.presetId,
        status,
        baseTradeDate: null,
        totalSymbols,
        succeededSymbols: 0,
        failedSymbols: 0,
        startedAt: summary.startedAt,
        finishedAt: summary.finishedAt,
        presetSnapshotJson: extra.presetSnapshotJson ?? {},
        symbolsSnapshotJson: extra.symbolsSnapshotJson ?? [],
        retryOfRunId: extra.retryOfRunId ?? null,
        ...extra,
      };
      persist();
      return summary;
    },

    updateRun(
      runId: string,
      updates: Partial<ScanRunSummary> & Partial<ScanRunDetail>,
    ): void {
      const existing = state.runDetails[runId];
      if (!existing) return;
      Object.assign(existing, updates);
      const runIdx = state.runs.findIndex((r) => r.id === runId);
      if (runIdx >= 0) {
        Object.assign(state.runs[runIdx], updates);
      }
      persist();
    },

    setResults(runId: string, results: ScanResult[]): void {
      state.results[runId] = results;
      persist();
    },

    setErrors(runId: string, errors: ScanError[]): void {
      state.errors[runId] = errors;
      persist();
    },

    reset(): void {
      state = createInitialState();
      persist();
    },
  };
}
