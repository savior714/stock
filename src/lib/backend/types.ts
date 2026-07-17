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

export interface BackendClient {
  watchlists: {
    list(): Promise<WatchlistSummary[]>;
    get(id: string): Promise<WatchlistDetail>;
    create(input: WatchlistInput): Promise<WatchlistDetail>;
    update(id: string, input: WatchlistInput): Promise<WatchlistDetail>;
    delete(id: string): Promise<void>;
  };

  presets: {
    list(): Promise<ScanPresetSummary[]>;
    get(id: string): Promise<ScanPresetDetail>;
    create(input: ScanPresetInput): Promise<ScanPresetDetail>;
    update(id: string, input: ScanPresetInput): Promise<ScanPresetDetail>;
    delete(id: string): Promise<void>;
  };

  scans: {
    start(request: StartScanRequest): Promise<string>;
    retry(runId: string): Promise<string>;
    listRuns(limit?: number): Promise<ScanRunSummary[]>;
    getRun(runId: string): Promise<ScanRunDetail>;
    getResults(
      runId: string,
      filter?: "and" | "or",
    ): Promise<ScanResult[]>;
    getErrors(runId: string): Promise<ScanError[]>;
    cancel(runId: string): Promise<void>;
  };

  events: {
    subscribe(
      eventType: ScanEventType,
      handler: (payload: ScanEventPayload) => void,
    ): Promise<() => void>;
  };
}
