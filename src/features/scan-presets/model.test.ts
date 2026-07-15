import { describe, expect, it } from "vitest";

import { FIXED_CONDITION_SLOTS } from "./types";
import { emptyPresetForm } from "./defaults";
import { conditionKey, detailToForm, formToInput, validateForm } from "./model";
import type { ScanConditionDetail, ScanPresetDetail, ScanPresetFormState } from "./types";

// ── emptyPresetForm ─────────────────────────────────────────────

describe("emptyPresetForm", () => {
  it("6개 slot과 기본값을 포함한다", () => {
    const form = emptyPresetForm();

    expect(form.id).toBe(null);
    expect(form.name).toBe("");
    expect(form.conditions).toHaveLength(6);

    // FIXED_CONDITION_SLOTS 순서와 일치
    for (let i = 0; i < FIXED_CONDITION_SLOTS.length; i++) {
      const slot = FIXED_CONDITION_SLOTS[i];
      expect(form.conditions[i].indicator).toBe(slot.indicator);
      expect(form.conditions[i].side).toBe(slot.side);
    }

    // rsi:lower 기본값
    expect(form.conditions[0]).toMatchObject({
      indicator: "rsi",
      side: "lower",
      period: 14,
      threshold: 30,
      stdDevMultiplier: null,
      triggerMode: "current",
      enabled: true,
    });

    // rsi:upper 기본값
    expect(form.conditions[1]).toMatchObject({
      indicator: "rsi",
      side: "upper",
      period: 14,
      threshold: 70,
      stdDevMultiplier: null,
      triggerMode: "current",
      enabled: false,
    });

    // mfi:lower 기본값
    expect(form.conditions[2]).toMatchObject({
      indicator: "mfi",
      side: "lower",
      period: 14,
      threshold: 30,
      stdDevMultiplier: null,
      triggerMode: "current",
      enabled: true,
    });

    // mfi:upper 기본값
    expect(form.conditions[3]).toMatchObject({
      indicator: "mfi",
      side: "upper",
      period: 14,
      threshold: 70,
      stdDevMultiplier: null,
      triggerMode: "current",
      enabled: false,
    });

    // bollinger:lower 기본값
    expect(form.conditions[4]).toMatchObject({
      indicator: "bollinger",
      side: "lower",
      period: 20,
      threshold: null,
      stdDevMultiplier: 1.0,
      triggerMode: "current",
      enabled: true,
    });

    // bollinger:upper 기본값
    expect(form.conditions[5]).toMatchObject({
      indicator: "bollinger",
      side: "upper",
      period: 20,
      threshold: null,
      stdDevMultiplier: 1.0,
      triggerMode: "current",
      enabled: false,
    });
  });
});

// ── detailToForm ────────────────────────────────────────────────

describe("detailToForm", () => {
  function makeDetail(conditions: ScanConditionDetail[]): ScanPresetDetail {
    return { id: "test-1", name: "Test Preset", conditions };
  }

  it("condition 순서가 달라도 6개 slot에 정확히 매핑된다", () => {
    // FIXED_CONDITION_SLOTS와 다른 순서로 condition 전달
    const shuffled: ScanConditionDetail[] = [
      { indicator: "bollinger", side: "upper", period: 20, threshold: null, stdDevMultiplier: 2.0, triggerMode: "cross", enabled: true },
      { indicator: "rsi", side: "lower", period: 14, threshold: 25, stdDevMultiplier: null, triggerMode: "current", enabled: true },
      { indicator: "mfi", side: "upper", period: 10, threshold: 75, stdDevMultiplier: null, triggerMode: "current", enabled: false },
      { indicator: "bollinger", side: "lower", period: 20, threshold: null, stdDevMultiplier: 1.5, triggerMode: "current", enabled: true },
      { indicator: "rsi", side: "upper", period: 14, threshold: 65, stdDevMultiplier: null, triggerMode: "cross", enabled: false },
      { indicator: "mfi", side: "lower", period: 10, threshold: 20, stdDevMultiplier: null, triggerMode: "current", enabled: true },
    ];

    const form = detailToForm(makeDetail(shuffled));

    // FIXED_CONDITION_SLOTS 순서로 정렬되어 반환
    expect(form.conditions[0].indicator).toBe("rsi");
    expect(form.conditions[0].side).toBe("lower");
    expect(form.conditions[0].threshold).toBe(25);

    expect(form.conditions[1].indicator).toBe("rsi");
    expect(form.conditions[1].side).toBe("upper");
    expect(form.conditions[1].threshold).toBe(65);

    expect(form.conditions[2].indicator).toBe("mfi");
    expect(form.conditions[2].side).toBe("lower");
    expect(form.conditions[2].threshold).toBe(20);

    expect(form.conditions[3].indicator).toBe("mfi");
    expect(form.conditions[3].side).toBe("upper");
    expect(form.conditions[3].threshold).toBe(75);

    expect(form.conditions[4].indicator).toBe("bollinger");
    expect(form.conditions[4].side).toBe("lower");
    expect(form.conditions[4].stdDevMultiplier).toBe(1.5);

    expect(form.conditions[5].indicator).toBe("bollinger");
    expect(form.conditions[5].side).toBe("upper");
    expect(form.conditions[5].stdDevMultiplier).toBe(2.0);
  });

  it("누락 condition slot이 있으면 에러를 던진다", () => {
    // bollinger:upper 누락 (5개만 전달)
    const incomplete: ScanConditionDetail[] = [
      { indicator: "rsi", side: "lower", period: 14, threshold: 30, stdDevMultiplier: null, triggerMode: "current", enabled: true },
      { indicator: "rsi", side: "upper", period: 14, threshold: 70, stdDevMultiplier: null, triggerMode: "current", enabled: false },
      { indicator: "mfi", side: "lower", period: 14, threshold: 30, stdDevMultiplier: null, triggerMode: "current", enabled: true },
      { indicator: "mfi", side: "upper", period: 14, threshold: 70, stdDevMultiplier: null, triggerMode: "current", enabled: false },
      { indicator: "bollinger", side: "lower", period: 20, threshold: null, stdDevMultiplier: 1.0, triggerMode: "current", enabled: true },
    ];

    expect(() => detailToForm(makeDetail(incomplete))).toThrow("조건 슬롯이 없습니다: bollinger:upper");
  });
});

// ── validateForm ────────────────────────────────────────────────

describe("validateForm", () => {
  function makeForm(overrides?: Partial<ScanPresetFormState>): ScanPresetFormState {
    const base = emptyPresetForm();
    return { ...base, ...overrides };
  }

  it("name이 공백만이면 name error", () => {
    const errors = validateForm({ ...emptyPresetForm(), name: "   " });
    expect(errors.name).toBeDefined();
  });

  it("name이 80자를 초과하면 name error", () => {
    const longName = "a".repeat(81);
    const errors = validateForm({ ...emptyPresetForm(), name: longName });
    expect(errors.name).toBeDefined();
  });

  it("모든 condition이 disabled면 conditions error", () => {
    const allDisabled = emptyPresetForm().conditions.map((c) => ({ ...c, enabled: false }));
    const errors = validateForm({ ...emptyPresetForm(), conditions: allDisabled });
    expect(errors.conditions).toBeDefined();
  });

  it("RSI threshold가 0 미만이면 threshold error", () => {
    const conditions = emptyPresetForm().conditions.map((c) =>
      c.indicator === "rsi" && c.side === "lower" ? { ...c, threshold: -1 } : c,
    );
    const errors = validateForm({ ...emptyPresetForm(), conditions });
    expect(errors.conditionErrors["rsi:lower"]?.threshold).toBeDefined();
  });

  it("MFI threshold가 100을 초과하면 threshold error", () => {
    const conditions = emptyPresetForm().conditions.map((c) =>
      c.indicator === "mfi" && c.side === "lower" ? { ...c, threshold: 101 } : c,
    );
    const errors = validateForm({ ...emptyPresetForm(), conditions });
    expect(errors.conditionErrors["mfi:lower"]?.threshold).toBeDefined();
  });

  it("Bollinger stdDevMultiplier가 범위 밖이면 error", () => {
    const conditions = emptyPresetForm().conditions.map((c) =>
      c.indicator === "bollinger" && c.side === "lower" ? { ...c, stdDevMultiplier: 0.05 } : c,
    );
    const errors = validateForm({ ...emptyPresetForm(), conditions });
    expect(errors.conditionErrors["bollinger:lower"]?.stdDevMultiplier).toBeDefined();
  });
});

// ── formToInput ─────────────────────────────────────────────────

describe("formToInput", () => {
  it("name의 앞뒤 공백이 제거된다", () => {
    const form = { ...emptyPresetForm(), name: "  My Preset  " };
    const input = formToInput(form);
    expect(input.name).toBe("My Preset");
  });
});

// ── conditionKey ────────────────────────────────────────────────

describe("conditionKey", () => {
  it("indicator와 side를 콜론으로 결합한다", () => {
    expect(conditionKey({ indicator: "rsi", side: "lower" })).toBe("rsi:lower");
    expect(conditionKey({ indicator: "bollinger", side: "upper" })).toBe("bollinger:upper");
  });
});
