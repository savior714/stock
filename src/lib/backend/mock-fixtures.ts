import type { WatchlistSummary } from "@/features/watchlists/types";
import type { ScanPresetSummary } from "@/features/scan-presets/types";
import type { ScanResult, ScanError, ScanResultEvent } from "@/features/scans/types";

export const INITIAL_WATCHLISTS: Array<WatchlistSummary & { _symbols?: string[] }> = [
  {
    id: "wl-1",
    name: "미국 대형주",
    description: "메저리 대형주 위주",
    symbolCount: 5,
    _symbols: ["AAPL", "MSFT", "NVDA", "AMZN", "GOOGL"],
  },
  {
    id: "wl-2",
    name: "저점 관찰",
    description: "저점 접근 종목",
    symbolCount: 4,
    _symbols: ["AMD", "TSM", "AVGO", "COST"],
  },
];

export const INITIAL_PRESETS: ScanPresetSummary[] = [
  {
    id: "preset-1",
    name: "기본 저점 스캔",
    enabledConditionCount: 3,
  },
  {
    id: "preset-2",
    name: "[MOCK] 전체 성공",
    enabledConditionCount: 1,
  },
  {
    id: "preset-3",
    name: "[MOCK] 부분 실패",
    enabledConditionCount: 1,
  },
  {
    id: "preset-4",
    name: "[MOCK] 느린 실행",
    enabledConditionCount: 1,
  },
];

const ALL_SYMBOLS = ["AAPL", "MSFT", "NVDA", "AMZN", "GOOGL", "AMD", "TSM", "AVGO", "COST"];

const FIXED_TRADE_DATE = "2025-07-17";

type FixtureEntry = Omit<ScanResultEvent, "runId" | "sequence"> & {
  result: ScanResult | null;
  error: ScanError | null;
};

type FixtureData = Record<string, Record<string, FixtureEntry>>;

const FIXED_RESULTS: FixtureData = {
  "preset-2": {},
  "preset-3": {},
  "preset-4": {},
};

// preset-2: all success
["AAPL", "MSFT", "NVDA", "AMZN", "GOOGL", "AMD", "TSM", "AVGO", "COST"].forEach((sym, i) => {
  FIXED_RESULTS["preset-2"][sym] = {
    symbol: sym,
    success: true,
    result: {
      symbol: sym,
      tradeDate: FIXED_TRADE_DATE,
      currentPrice: 100 + i * 15,
      rsi: 30 + i * 5,
      mfi: 25 + i * 6,
      bollingerLower: 90 + i * 10,
      bollingerMiddle: 100 + i * 12,
      bollingerUpper: 110 + i * 14,
      allConditionsMatched: i % 2 === 0,
      anyConditionMatched: true,
      dataStale: false,
    },
    error: null,
  };
});

// preset-3: partial failure
["AAPL", "MSFT", "NVDA"].forEach((sym, i) => {
  FIXED_RESULTS["preset-3"][sym] = {
    symbol: sym,
    success: true,
    result: {
      symbol: sym,
      tradeDate: FIXED_TRADE_DATE,
      currentPrice: 150 + i * 20,
      rsi: 35 + i * 4,
      mfi: 30 + i * 5,
      bollingerLower: 130 + i * 10,
      bollingerMiddle: 150 + i * 15,
      bollingerUpper: 170 + i * 20,
      allConditionsMatched: i === 0,
      anyConditionMatched: true,
      dataStale: false,
    },
    error: null,
  };
});
FIXED_RESULTS["preset-3"]["GOOGL"] = {
  symbol: "GOOGL",
  success: false,
  result: null as unknown as ScanResult,
  error: {
    symbol: "GOOGL",
    code: "NETWORK_RETRY",
    message: "Yahoo Finance API retry limit exceeded",
    detail: "temporary network error",
    retryable: true,
    attempt: 3,
  },
};
FIXED_RESULTS["preset-3"]["AMD"] = {
  symbol: "AMD",
  success: false,
  result: null as unknown as ScanResult,
  error: {
    symbol: "AMD",
    code: "DATA_NOT_FOUND",
    message: "No chart data available for this symbol",
    detail: "permanent data error",
    retryable: false,
    attempt: 1,
  },
};
["TSM", "AVGO", "COST"].forEach((sym, i) => {
  FIXED_RESULTS["preset-3"][sym] = {
    symbol: sym,
    success: true,
    result: {
      symbol: sym,
      tradeDate: FIXED_TRADE_DATE,
      currentPrice: 80 + i * 10,
      rsi: 40 + i * 3,
      mfi: 35 + i * 4,
      bollingerLower: 70 + i * 8,
      bollingerMiddle: 80 + i * 10,
      bollingerUpper: 90 + i * 12,
      allConditionsMatched: false,
      anyConditionMatched: i === 0,
      dataStale: false,
    },
    error: null,
  };
});

// preset-4: slow execution (same data as preset-2 but with longer delays)
Object.assign(FIXED_RESULTS["preset-4"], FIXED_RESULTS["preset-2"]);

export { FIXED_RESULTS };
