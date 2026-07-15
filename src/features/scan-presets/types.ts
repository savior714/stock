export type IndicatorKind = "rsi" | "mfi" | "bollinger";
export type SignalSide = "lower" | "upper";
export type TriggerMode = "current" | "cross";

export type ScanPresetSummary = {
  id: string;
  name: string;
  enabledConditionCount: number;
};

export type ScanConditionDetail = {
  indicator: IndicatorKind;
  side: SignalSide;
  period: number;
  threshold: number | null;
  stdDevMultiplier: number | null;
  triggerMode: TriggerMode;
  enabled: boolean;
};

export type ScanPresetDetail = {
  id: string;
  name: string;
  conditions: ScanConditionDetail[];
};

export type ScanConditionWrite = {
  indicator: IndicatorKind;
  side: SignalSide;
  period: number;
  threshold: number | null;
  stdDevMultiplier: number | null;
  triggerMode: TriggerMode;
  enabled: boolean;
};

export type ScanPresetInput = {
  name: string;
  conditions: ScanConditionWrite[];
};

export type ScanPresetFormState = {
  id: string | null;
  name: string;
  conditions: ScanConditionWrite[];
};

export const FIXED_CONDITION_SLOTS: Array<{ indicator: IndicatorKind; side: SignalSide }> = [
  { indicator: "rsi", side: "lower" },
  { indicator: "rsi", side: "upper" },
  { indicator: "mfi", side: "lower" },
  { indicator: "mfi", side: "upper" },
  { indicator: "bollinger", side: "lower" },
  { indicator: "bollinger", side: "upper" },
];
