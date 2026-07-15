"use client";

import { useCallback, useEffect, useState } from "react";

import { errorMessage } from "@/features/watchlists/api";
import { listScanPresets } from "./api";
import type { ScanPresetSummary } from "./types";

export default function ScanPresetWorkspace() {
  const [presets, setPresets] = useState<ScanPresetSummary[]>([]);
  const [isLoading, setIsLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);

  const refreshList = useCallback(async () => {
    try {
      const rows = await listScanPresets();
      setPresets(rows);
    } catch (loadError) {
      setError(errorMessage(loadError));
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
          setError(errorMessage(loadError));
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

  return (
    <div className="scan-preset-layout">
      <section className="panel scan-preset-browser" aria-busy={isLoading}>
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Scan conditions</p>
            <h3>Presets</h3>
          </div>
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
            <div key={preset.id} className="scan-preset-item">
              <span>
                <strong>{preset.name}</strong>
              </span>
              <b>{preset.enabledConditionCount}</b>
            </div>
          ))}
        </div>
      </section>

      <section className="panel editor-panel">
        <div className="panel-heading">
          <div>
            <p className="eyebrow">Preset detail</p>
            <h3>상세 정보</h3>
          </div>
        </div>

        <div className="compact-empty">
          <strong>조건 카드 구현 예정</strong>
          <span>좌측 목록에서 Preset을 선택하면 상세 정보를 표시합니다.</span>
        </div>
      </section>
    </div>
  );
}
