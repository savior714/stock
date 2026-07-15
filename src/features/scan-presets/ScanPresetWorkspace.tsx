"use client";

import { useCallback, useEffect, useRef, useState } from "react";

import { formatAppError, parseAppError } from "@/lib/app-error";
import {
  createScanPreset,
  deleteScanPreset,
  getScanPreset,
  listScanPresets,
  updateScanPreset,
} from "./api";
import { emptyPresetForm } from "./defaults";
import { ScanConditionCard } from "./ScanConditionCard";
import type {
  IndicatorKind,
  ScanConditionDetail,
  ScanConditionWrite,
  ScanPresetDetail,
  ScanPresetFormState,
  ScanPresetSummary,
  SignalSide,
} from "./types";
import { FIXED_CONDITION_SLOTS } from "./types";

function conditionKey(condition: { indicator: IndicatorKind; side: SignalSide }): string {
  return `${condition.indicator}:${condition.side}`;
}

function detailToForm(detail: ScanPresetDetail): ScanPresetFormState {
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

function validateForm(form: ScanPresetFormState): Record<string, string> {
  const errors: Record<string, string> = {};

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

    if (condition.period < 2 || condition.period > 500) {
      errors[`${key}:period`] = "Period은 2~500 사이여야 합니다.";
    }

    if (condition.indicator === "rsi" || condition.indicator === "mfi") {
      if (condition.threshold === null) {
        errors[`${key}:threshold`] = "Threshold을 입력하십시오.";
      } else if (condition.threshold < 0 || condition.threshold > 100) {
        errors[`${key}:threshold`] = "Threshold은 0~100 사이여야 합니다.";
      }
    }

    if (condition.indicator === "bollinger") {
      if (condition.stdDevMultiplier === null) {
        errors[`${key}:stdDevMultiplier`] = "표준편차 배수를 입력하십시오.";
      } else if (condition.stdDevMultiplier < 0.1 || condition.stdDevMultiplier > 10) {
        errors[`${key}:stdDevMultiplier`] = "배수는 0.1~10 사이여야 합니다.";
      }
    }
  }

  return errors;
}

export default function ScanPresetWorkspace() {
  const [presets, setPresets] = useState<ScanPresetSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [fieldErrors, setFieldErrors] = useState<Record<string, string>>({});
  const [form, setForm] = useState<ScanPresetFormState>(emptyPresetForm());
  const noticeTimerRef = useRef<number | null>(null);
  const [isLoadingDetail, setIsLoadingDetail] = useState(false);
  const [isSaving, setIsSaving] = useState(false);
  const [isDeleting, setIsDeleting] = useState(false);

  const isBusy = isLoadingDetail || isSaving || isDeleting;

  // notice timer cleanup on unmount
  useEffect(() => {
    return () => {
      if (noticeTimerRef.current) {
        clearTimeout(noticeTimerRef.current);
      }
    };
  }, []);

  const clearNotice = useCallback(() => {
    if (noticeTimerRef.current) {
      clearTimeout(noticeTimerRef.current);
    }
    setNotice(null);
  }, []);

  const showNotice = useCallback((message: string) => {
    clearNotice();
    setNotice(message);
    noticeTimerRef.current = window.setTimeout(() => {
      noticeTimerRef.current = null;
      setNotice(null);
    }, 3000);
  }, [clearNotice]);

  const refreshList = useCallback(async () => {
    try {
      const rows = await listScanPresets();
      setPresets(rows);
    } catch (loadError) {
      setError(formatAppError(loadError));
    }
  }, []);

  useEffect(() => {
    let cancelled = false;

    async function load() {
      try {
        const rows = await listScanPresets();
        if (!cancelled) {
          setPresets(rows);
        }
      } catch (loadError) {
        if (!cancelled) {
          setError(formatAppError(loadError));
        }
      } finally {
        if (!cancelled) {
          setIsLoading(false);
        }
      }
    }

    void load();
    return () => {
      cancelled = true;
    };
  }, []);

  const selectPreset = useCallback(
    async (id: string) => {
      setError(null);
      setNotice(null);
      setFieldErrors({});
      setIsLoadingDetail(true);

      try {
        const detail = await getScanPreset(id);
        setForm(detailToForm(detail));
      } catch (loadError) {
        setError(formatAppError(loadError));
      } finally {
        setIsLoadingDetail(false);
      }
    },
    [],
  );

  const startNewPreset = useCallback(() => {
    setForm(emptyPresetForm());
    setError(null);
    setNotice(null);
    setFieldErrors({});
  }, []);

  const updateCondition = useCallback(
    (key: string, nextCondition: ScanConditionWrite) => {
      setForm((current) => ({
        ...current,
        conditions: current.conditions.map((condition) =>
          conditionKey(condition) === key ? nextCondition : condition,
        ),
      }));
    },
    [],
  );

  const handleNameChange = useCallback(
    (e: React.ChangeEvent<HTMLInputElement>) => {
      setForm((current) => ({ ...current, name: e.target.value }));
    },
    [],
  );

  const handleSave = useCallback(async () => {
    if (isBusy) return;

    clearNotice();
    setFieldErrors({});

    const errors = validateForm(form);
    if (Object.keys(errors).length > 0) {
      setFieldErrors(errors);
      return;
    }

    setIsSaving(true);

    const input = {
      name: form.name.trim(),
      conditions: form.conditions,
    };

    try {
      const detail = form.id
        ? await updateScanPreset(form.id, input)
        : await createScanPreset(input);

      setForm(detailToForm(detail));
      await refreshList();
      showNotice(form.id ? "Preset이 저장되었습니다." : "새 Preset이 생성되었습니다.");
    } catch (saveError) {
      const payload = parseAppError(saveError);

      if (payload.code === "conflict") {
        setFieldErrors({ name: payload.message });
      } else if (payload.code === "validation") {
        // Backend validation error — route to relevant field
        if (payload.detail && payload.detail.toLowerCase().includes("condition")) {
          setFieldErrors({ conditions: payload.message });
        } else {
          setFieldErrors({ name: payload.message });
        }
      } else {
        setError(formatAppError(saveError));
      }
    } finally {
      setIsSaving(false);
    }
  }, [form, isBusy, clearNotice, refreshList, showNotice]);

  const handleDelete = useCallback(async () => {
    if (!form.id || isBusy) return;

    if (!window.confirm(`"${form.name}" Preset을 삭제하시겠습니까?`)) {
      return;
    }

    setIsDeleting(true);

    try {
      await deleteScanPreset(form.id);
      setForm(emptyPresetForm());
      await refreshList();
      showNotice("Preset이 삭제되었습니다.");
    } catch (deleteError) {
      setError(formatAppError(deleteError));
    } finally {
      setIsDeleting(false);
    }
  }, [form, isBusy, refreshList, showNotice]);

  return (
    <div className="scan-preset-layout">
      <section className="panel scan-preset-browser" aria-busy={isLoading}>
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Scan conditions</p>
            <h3>Presets</h3>
          </div>
          <button
            className="primary-button"
            type="button"
            onClick={startNewPreset}
            disabled={isBusy}
          >
            새 Preset
          </button>
        </div>

        {isLoading ? <p className="muted">목록을 불러오는 중입니다.</p> : null}
        {error ? <div className="message error-message">{error}</div> : null}
        {!isLoading && !error && presets.length === 0 ? (
          <div className="compact-empty">
            <strong>저장된 Preset이 없습니다.</strong>
            <span>새 Preset을 만들어 스캔 조건을 구성하십시오.</span>
          </div>
        ) : null}

        <div className="scan-preset-items">
          {presets.map((preset) => (
            <button
              key={preset.id}
              type="button"
              className={
                form.id === preset.id ? "scan-preset-item active" : "scan-preset-item"
              }
              onClick={() => void selectPreset(preset.id)}
              disabled={isBusy}
            >
              <span>
                <strong>{preset.name}</strong>
              </span>
              <b>{preset.enabledConditionCount}</b>
            </button>
          ))}
        </div>
      </section>

      <section className="panel editor-panel" aria-busy={isBusy}>
        <div className="panel-heading">
          <div>
            <p className="eyebrow">{form.id ? "Edit Preset" : "New Preset"}</p>
            <h3>{form.id ? form.name || "Preset" : "새 Preset"}</h3>
          </div>
          {form.id ? (
            <button
              className="danger-button"
              type="button"
              onClick={handleDelete}
              disabled={isBusy || isSaving}
            >
              삭제
            </button>
          ) : null}
        </div>

        {notice ? <div className="message success-message">{notice}</div> : null}
        {error ? <div className="message error-message">{error}</div> : null}

        <div className="scan-preset-form">
          <label>
            Preset 이름
            <input
              type="text"
              maxLength={80}
              value={form.name}
              onChange={handleNameChange}
              disabled={isBusy}
              placeholder="이름을 입력하십시오."
            />
          </label>
          {fieldErrors.name ? (
            <div className="field-error">{fieldErrors.name}</div>
          ) : null}

          <div className="condition-grid">
            {form.conditions.map((condition) => (
              <ScanConditionCard
                key={conditionKey(condition)}
                condition={condition}
                disabled={isBusy}
                error={fieldErrors[conditionKey(condition) + ":period"] || fieldErrors[conditionKey(condition) + ":threshold"] || fieldErrors[conditionKey(condition) + ":stdDevMultiplier"]}
                onChange={(next) => updateCondition(conditionKey(condition), next)}
              />
            ))}
          </div>
          {fieldErrors.conditions ? (
            <div className="field-error">{fieldErrors.conditions}</div>
          ) : null}

          <div className="form-actions">
            <button
              className="primary-button strong"
              type="button"
              onClick={handleSave}
              disabled={isBusy || isDeleting}
            >
              {form.id ? "저장" : "생성"}
            </button>
          </div>
        </div>
      </section>
    </div>
  );
}
