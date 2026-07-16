export type ScanRunStatus = "pending" | "running" | "completed" | "cancelled" | "failed";

export type ScanRunSummary = {
  id: string;
  watchlistId: string;
  presetId: string;
  status: ScanRunStatus;
  totalSymbols: number;
  succeededSymbols: number;
  failedSymbols: number;
  startedAt: string | null;
  finishedAt: string | null;
};

export type ScanRunDetail = {
  id: string;
  watchlistId: string;
  presetId: string;
  status: ScanRunStatus;
  baseTradeDate: string | null;
  totalSymbols: number;
  succeededSymbols: number;
  failedSymbols: number;
  startedAt: string | null;
  finishedAt: string | null;
  presetSnapshotJson: Record<string, unknown>;
  symbolsSnapshotJson: unknown[];
  retryOfRunId: string | null;
};

export type ScanResult = {
  symbol: string;
  tradeDate: string;
  currentPrice: number;
  rsi: number | null;
  mfi: number | null;
  bollingerLower: number | null;
  bollingerMiddle: number | null;
  bollingerUpper: number | null;
  allConditionsMatched: boolean;
  anyConditionMatched: boolean;
  dataStale: boolean;
};

export type ScanError = {
  symbol: string | null;
  code: string;
  message: string;
  detail: string | null;
  retryable: boolean;
  attempt: number;
};

export type StartScanRequest = {
  watchlistId: string;
  presetId: string;
};

export type ScanStartedEvent = {
  runId: string;
  sequence: number;
};

export type ScanProgressEvent = {
  runId: string;
  sequence: number;
  completed: number;
  total: number;
  succeeded: number;
  failed: number;
  currentSymbol?: string;
};

export type ScanResultEvent = {
  runId: string;
  sequence: number;
  symbol: string;
  success: boolean;
};

export type ScanErrorEvent = {
  runId: string;
  sequence: number;
  symbol: string | null;
  code: string;
  message: string;
};

export type ScanCompletedEvent = {
  runId: string;
  sequence: number;
  total: number;
  succeeded: number;
  failed: number;
};

export type ScanCancelledEvent = {
  runId: string;
  sequence: number;
};

export type ScanEventPayload =
  | ScanStartedEvent
  | ScanProgressEvent
  | ScanResultEvent
  | ScanErrorEvent
  | ScanCompletedEvent
  | ScanCancelledEvent;

export type ScanEventType =
  | "scan://started"
  | "scan://progress"
  | "scan://result"
  | "scan://error"
  | "scan://completed"
  | "scan://cancelled";
