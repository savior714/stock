"use client";

import { type ChangeEvent, type FocusEvent, useCallback } from "react";

import type { ScanConditionWrite } from "./types";

type ScanConditionCardProps = {
  condition: ScanConditionWrite;
  disabled: boolean;
  error?: string;
  onChange: (next: ScanConditionWrite) => void;
};

const INDICATOR_LABELS: Record<string, string> = {
  rsi: "RSI",
  mfi: "MFI",
  bollinger: "Bollinger",
};

const SIDE_LABELS: Record<string, string> = {
  lower: "Lower",
  upper: "Upper",
};

export function ScanConditionCard({
  condition,
  disabled,
  error,
  onChange,
}: ScanConditionCardProps) {
  const indicatorLabel = INDICATOR_LABELS[condition.indicator] ?? condition.indicator;
  const sideLabel = SIDE_LABELS[condition.side] ?? condition.side;

  const handleEnabledChange = useCallback(
    (e: ChangeEvent<HTMLInputElement>) => {
      onChange({ ...condition, enabled: e.target.checked });
    },
    [condition, onChange],
  );

  const handlePeriodChange = useCallback(
    (e: ChangeEvent<HTMLInputElement>) => {
      const val = e.target.value;
      if (val === "") {
        onChange({ ...condition, period: 0 });
        return;
      }
      const num = Number(val);
      if (!Number.isNaN(num)) {
        onChange({ ...condition, period: num });
      }
    },
    [condition, onChange],
  );

  const handlePeriodBlur = useCallback(
    (_e: FocusEvent<HTMLInputElement>) => {
      let period = condition.period;
      if (period < 2) period = 2;
      if (period > 500) period = 500;
      if (period !== condition.period) {
        onChange({ ...condition, period });
      }
    },
    [condition, onChange],
  );

  const handleThresholdChange = useCallback(
    (e: ChangeEvent<HTMLInputElement>) => {
      const val = e.target.value;
      if (val === "") {
        onChange({ ...condition, threshold: null });
        return;
      }
      const num = Number(val);
      if (!Number.isNaN(num)) {
        onChange({ ...condition, threshold: num });
      }
    },
    [condition, onChange],
  );

  const handleThresholdBlur = useCallback(
    (_e: FocusEvent<HTMLInputElement>) => {
      if (condition.threshold === null) return;
      let val = condition.threshold;
      if (val < 0) val = 0;
      if (val > 100) val = 100;
      if (val !== condition.threshold) {
        onChange({ ...condition, threshold: val });
      }
    },
    [condition, onChange],
  );

  const handleMultiplierChange = useCallback(
    (e: ChangeEvent<HTMLInputElement>) => {
      const val = e.target.value;
      if (val === "") {
        onChange({ ...condition, stdDevMultiplier: null });
        return;
      }
      const num = Number(val);
      if (!Number.isNaN(num)) {
        onChange({ ...condition, stdDevMultiplier: num });
      }
    },
    [condition, onChange],
  );

  const handleMultiplierBlur = useCallback(
    (_e: FocusEvent<HTMLInputElement>) => {
      if (condition.stdDevMultiplier === null) return;
      let val = condition.stdDevMultiplier;
      if (val < 0.1) val = 0.1;
      if (val > 10) val = 10;
      if (val !== condition.stdDevMultiplier) {
        onChange({ ...condition, stdDevMultiplier: val });
      }
    },
    [condition, onChange],
  );

  const handleTriggerModeChange = useCallback(
    (mode: "current" | "cross") => {
      onChange({ ...condition, triggerMode: mode });
    },
    [condition, onChange],
  );

  const isBollinger = condition.indicator === "bollinger";

  return (
    <div
      className={`condition-card${disabled ? " disabled" : ""}${!condition.enabled ? " disabled" : ""}`}
    >
      <div className="condition-card-header">
        <div>
          <span className="condition-indicator">{indicatorLabel}</span>{" "}
          <span className="condition-side">{sideLabel}</span>
        </div>
        <label className="condition-toggle">
          <input
            type="checkbox"
            checked={condition.enabled}
            disabled={disabled}
            onChange={handleEnabledChange}
          />
          <span>활성화</span>
        </label>
      </div>

      <div className="condition-fields">
        <label>
          Period
          <input
            type="number"
            min={2}
            max={500}
            step={1}
            value={condition.period}
            disabled={disabled || !condition.enabled}
            onChange={handlePeriodChange}
            onBlur={handlePeriodBlur}
          />
        </label>

        {!isBollinger ? (
          <label>
            Threshold
            <input
              type="number"
              min={0}
              max={100}
              step={1}
              value={condition.threshold ?? ""}
              disabled={disabled || !condition.enabled}
              onChange={handleThresholdChange}
              onBlur={handleThresholdBlur}
            />
          </label>
        ) : (
          <label>
            표준편차 배수
            <input
              type="number"
              min={0.1}
              max={10}
              step={0.1}
              value={condition.stdDevMultiplier ?? ""}
              disabled={disabled || !condition.enabled}
              onChange={handleMultiplierChange}
              onBlur={handleMultiplierBlur}
            />
          </label>
        )}

        <div className="trigger-mode-control">
          <span className="trigger-mode-label">트리거</span>
          <div className="trigger-mode-buttons">
            <button
              type="button"
              className={`trigger-mode-button${condition.triggerMode === "current" ? " active" : ""}`}
              disabled={disabled || !condition.enabled}
              onClick={() => handleTriggerModeChange("current")}
            >
              Current
            </button>
            <button
              type="button"
              className={`trigger-mode-button${condition.triggerMode === "cross" ? " active" : ""}`}
              disabled={disabled || !condition.enabled}
              onClick={() => handleTriggerModeChange("cross")}
            >
              Cross
            </button>
          </div>
        </div>
      </div>

      {error ? <div className="field-error">{error}</div> : null}
    </div>
  );
}
