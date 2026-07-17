import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

import type { BackendClient } from "./types";

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
  StartScanRequest,
} from "@/features/scans/types";

class TauriBackendClient implements BackendClient {
  watchlists = {
    list: (): Promise<WatchlistSummary[]> => invoke("list_watchlists"),

    get: (id: string): Promise<WatchlistDetail> =>
      invoke<WatchlistDetail>("get_watchlist", { id }),

    create: (input: WatchlistInput): Promise<WatchlistDetail> =>
      invoke<WatchlistDetail>("create_watchlist", { request: input }),

    update: (
      id: string,
      input: WatchlistInput,
    ): Promise<WatchlistDetail> =>
      invoke<WatchlistDetail>("update_watchlist", {
        request: { id, ...input },
      }),

    delete: (id: string): Promise<void> =>
      invoke<void>("delete_watchlist", { id }),
  };

  presets = {
    list: (): Promise<ScanPresetSummary[]> =>
      invoke("list_scan_presets"),

    get: (id: string): Promise<ScanPresetDetail> =>
      invoke<ScanPresetDetail>("get_scan_preset", { id }),

    create: (input: ScanPresetInput): Promise<ScanPresetDetail> =>
      invoke<ScanPresetDetail>("create_scan_preset", { request: input }),

    update: (
      id: string,
      input: ScanPresetInput,
    ): Promise<ScanPresetDetail> =>
      invoke<ScanPresetDetail>("update_scan_preset", {
        request: { id, ...input },
      }),

    delete: (id: string): Promise<void> =>
      invoke<void>("delete_scan_preset", { id }),
  };

  scans = {
    start: (request: StartScanRequest): Promise<string> =>
      invoke<string>("start_scan", {
        request: {
          watchlist_id: request.watchlistId,
          preset_id: request.presetId,
        },
      }),

    listRuns: (limit?: number): Promise<ScanRunSummary[]> =>
      invoke<ScanRunSummary[]>("list_scan_runs", { limit }),

    getRun: (runId: string): Promise<ScanRunDetail> =>
      invoke<ScanRunDetail>("get_scan_run", { runId }),

    getResults: (
      runId: string,
      filter?: "and" | "or",
    ): Promise<ScanResult[]> =>
      invoke<ScanResult[]>("get_scan_results", { runId, filter }),

    getErrors: (runId: string): Promise<ScanError[]> =>
      invoke<ScanError[]>("get_scan_errors", { runId }),

    cancel: (runId: string): Promise<void> =>
      invoke<void>("cancel_scan", { runId }),
  };

  events = {
    subscribe: (
      eventType: ScanEventType,
      handler: (payload: ScanEventPayload) => void,
    ): Promise<() => void> =>
      listen<ScanEventPayload>(eventType, (event) => {
        handler(event.payload);
      }),
  };
}

export { TauriBackendClient };
