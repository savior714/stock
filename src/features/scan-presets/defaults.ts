import type { ScanConditionWrite, ScanPresetFormState } from "./types";
import { FIXED_CONDITION_SLOTS } from "./types";

const DEFAULT_RSI_THRESHOLD = 30;
const DEFAULT_MFI_THRESHOLD = 30;
const DEFAULT_BOLLINGER_MULTIPLIER = 2.0;
const DEFAULT_PERIOD = 14;

function defaultCondition(slot: { indicator: string; side: string }): ScanConditionWrite {
  const base = {
    indicator: slot.indicator as ScanConditionWrite["indicator"],
    side: slot.side as ScanConditionWrite["side"],
    period: DEFAULT_PERIOD,
    triggerMode: "current" as const,
    enabled: false,
  };

  if (slot.indicator === "rsi") {
    return { ...base, threshold: DEFAULT_RSI_THRESHOLD, stdDevMultiplier: null };
  }
  if (slot.indicator === "mfi") {
    return { ...base, threshold: DEFAULT_MFI_THRESHOLD, stdDevMultiplier: null };
  }
  // bollinger
  return { ...base, threshold: null, stdDevMultiplier: DEFAULT_BOLLINGER_MULTIPLIER };
}

export function defaultConditions(): ScanConditionWrite[] {
  return FIXED_CONDITION_SLOTS.map(defaultCondition);
}

export const emptyPresetForm = (): ScanPresetFormState => ({
  id: null,
  name: "",
  conditions: defaultConditions(),
});
