import type {
  IndicatorKind,
  ScanConditionDetail,
  ScanConditionWrite,
  ScanPresetDetail,
  ScanPresetFormState,
  ScanPresetInput,
  SignalSide,
} from "./types";
import { FIXED_CONDITION_SLOTS } from "./types";

// ── Condition Key ───────────────────────────────────────────────

export type ConditionKey = string; // ex: "rsi:lower"

export function conditionKey(condition: {
  indicator: IndicatorKind;
  side: SignalSide;
}): ConditionKey {
  return `${condition.indicator}:${condition.side}`;
}

// ── Form Errors ─────────────────────────────────────────────────

export type ScanPresetFormErrors = {
  name?: string;
  conditions?: string;
  conditionErrors: Partial<
    Record<
      ConditionKey,
      {
        period?: string;
        threshold?: string;
        stdDevMultiplier?: string;
      }
    >
  >;
};

/** Convert structured errors to flat Record<string, string> for UI state. */
export function flattenFormErrors(errors: ScanPresetFormErrors): Record<string, string> {
  const flat: Record<string, string> = {};

  if (errors.name) {
    flat.name = errors.name;
  }

  if (errors.conditions) {
    flat.conditions = errors.conditions;
  }

  for (const [key, fieldErrors] of Object.entries(errors.conditionErrors)) {
    if (!fieldErrors) {
      continue;
    }
    if (fieldErrors.period) {
      flat[`${key}:period`] = fieldErrors.period;
    }
    if (fieldErrors.threshold) {
      flat[`${key}:threshold`] = fieldErrors.threshold;
    }
    if (fieldErrors.stdDevMultiplier) {
      flat[`${key}:stdDevMultiplier`] = fieldErrors.stdDevMultiplier;
    }
  }

  return flat;
}

// ── Detail → Form ───────────────────────────────────────────────

export function detailToForm(detail: ScanPresetDetail): ScanPresetFormState {
  const conditionsByKey = new Map<string, ScanConditionDetail>(
    detail.conditions.map((condition) => [conditionKey(condition), condition]),
  );

  return {
    id: detail.id,
    name: detail.name,
    conditions: FIXED_CONDITION_SLOTS.map((slot) => {
      const key = conditionKey(slot);
      const condition = conditionsByKey.get(key);

      if (!condition) {
        throw new Error(`조건 슬롯이 없습니다: ${key}`);
      }

      return { ...condition };
    }),
  };
}

// ── Validation ──────────────────────────────────────────────────

export function validateForm(form: ScanPresetFormState): ScanPresetFormErrors {
  const errors: ScanPresetFormErrors = { conditionErrors: {} };

  const name = form.name.trim();

  if (!name) {
    errors.name = "Preset 이름을 입력하십시오.";
  } else if (name.length > 80) {
    errors.name = "Preset 이름은 80자 이하여야 합니다.";
  }

  if (!form.conditions.some((condition) => condition.enabled)) {
    errors.conditions = "최소 한 개 조건을 활성화해야 합니다.";
  }

  for (const condition of form.conditions) {
    const key = conditionKey(condition);
    const conditionError: ScanPresetFormErrors["conditionErrors"][string] = {};

    if (condition.period < 2 || condition.period > 500) {
      conditionError.period = "Period은 2~500 사이여야 합니다.";
    }

    if (condition.indicator === "rsi" || condition.indicator === "mfi") {
      if (condition.threshold === null) {
        conditionError.threshold = "Threshold을 입력하십시오.";
      } else if (condition.threshold < 0 || condition.threshold > 100) {
        conditionError.threshold = "Threshold은 0~100 사이여야 합니다.";
      }
    }

    if (condition.indicator === "bollinger") {
      if (condition.stdDevMultiplier === null) {
        conditionError.stdDevMultiplier = "표준편차 배수를 입력하십시오.";
      } else if (condition.stdDevMultiplier < 0.1 || condition.stdDevMultiplier > 10) {
        conditionError.stdDevMultiplier = "배수는 0.1~10 사이여야 합니다.";
      }
    }

    if (Object.keys(conditionError).length > 0) {
      errors.conditionErrors[key] = conditionError;
    }
  }

  return errors;
}

// ── Form → API Input ────────────────────────────────────────────

export function formToInput(form: ScanPresetFormState): ScanPresetInput {
  return {
    name: form.name.trim(),
    conditions: form.conditions,
  };
}

// ── Condition Replacement ───────────────────────────────────────

/** Replace a condition in the conditions array by key. */
export function replaceCondition(
  conditions: ScanConditionWrite[],
  key: ConditionKey,
  nextCondition: ScanConditionWrite,
): ScanConditionWrite[] {
  return conditions.map((condition) =>
    conditionKey(condition) === key ? nextCondition : condition,
  );
}
