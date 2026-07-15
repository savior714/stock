import type { ScanConditionWrite, ScanPresetFormState } from "./types";
import { FIXED_CONDITION_SLOTS } from "./types";

const DEFAULTS: Record<string, Omit<ScanConditionWrite, "indicator" | "side">> = {
  "rsi:lower": {
    period: 14,
    threshold: 30,
    stdDevMultiplier: null,
    triggerMode: "current",
    enabled: true,
  },
  "rsi:upper": {
    period: 14,
    threshold: 70,
    stdDevMultiplier: null,
    triggerMode: "current",
    enabled: false,
  },
  "mfi:lower": {
    period: 14,
    threshold: 30,
    stdDevMultiplier: null,
    triggerMode: "current",
    enabled: true,
  },
  "mfi:upper": {
    period: 14,
    threshold: 70,
    stdDevMultiplier: null,
    triggerMode: "current",
    enabled: false,
  },
  "bollinger:lower": {
    period: 20,
    threshold: null,
    stdDevMultiplier: 1.0,
    triggerMode: "current",
    enabled: true,
  },
  "bollinger:upper": {
    period: 20,
    threshold: null,
    stdDevMultiplier: 1.0,
    triggerMode: "current",
    enabled: false,
  },
};

function slotKey(slot: { indicator: string; side: string }): string {
  return `${slot.indicator}:${slot.side}`;
}

function defaultCondition(slot: { indicator: string; side: string }): ScanConditionWrite {
  const key = slotKey(slot);
  const def = DEFAULTS[key];

  if (!def) {
    throw new Error(`Unknown condition slot: ${key}`);
  }

  return {
    indicator: slot.indicator as ScanConditionWrite["indicator"],
    side: slot.side as ScanConditionWrite["side"],
    ...def,
  };
}

export function defaultConditions(): ScanConditionWrite[] {
  return FIXED_CONDITION_SLOTS.map(defaultCondition);
}

export function emptyPresetForm(): ScanPresetFormState {
  return {
    id: null,
    name: "",
    conditions: defaultConditions(),
  };
}
