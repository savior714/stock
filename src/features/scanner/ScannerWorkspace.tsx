"use client";

import ScanRunSetup from "@/features/scans/ScanRunSetup";
import type { WatchlistSummary } from "@/features/watchlists/types";
import type { ScanPresetSummary } from "@/features/scan-presets/types";

type ScannerWorkspaceProps = {
  selectedWatchlistId: string;
  onWatchlistIdChange: (id: string) => void;
  selectedPresetId: string;
  onPresetIdChange: (id: string) => void;
  onOpenWatchlistDrawer: () => void;
  onOpenPresetDrawer: () => void;
  watchlists: WatchlistSummary[];
  presets: ScanPresetSummary[];
  watchlistExists: boolean;
  presetExists: boolean;
};

export default function ScannerWorkspace({
  selectedWatchlistId,
  onWatchlistIdChange,
  selectedPresetId,
  onPresetIdChange,
  onOpenWatchlistDrawer,
  onOpenPresetDrawer,
  watchlists,
  presets,
  watchlistExists,
  presetExists,
}: ScannerWorkspaceProps) {
  const selectedWatchlist = watchlists.find(
    (w) => w.id === selectedWatchlistId,
  );

  return (
    <div>
      {!selectedWatchlistId || !watchlistExists ? (
        <div className="empty-state">
          <h3>스캔할 Watchlist를 선택하십시오.</h3>
          <p>왼쪽 사이드바에서 Watchlist를 선택하거나, + 버튼을 눌러 새 Watchlist를 생성하십시오.</p>
        </div>
      ) : !selectedPresetId || !presetExists ? (
        <div className="empty-state">
          <h3>스캔할 Preset을 선택하십시오.</h3>
          <p>Preset 관리에서 스캔 조건을 설정하거나 편집하십시오.</p>
        </div>
      ) : (
        <ScanRunSetup
          selectedWatchlistId={selectedWatchlistId}
          onWatchlistIdChange={onWatchlistIdChange}
          selectedPresetId={selectedPresetId}
          onPresetIdChange={onPresetIdChange}
          watchlists={watchlists}
          presets={presets}
          onOpenPresetDrawer={onOpenPresetDrawer}
          presetExists={presetExists}
          watchlistExists={watchlistExists}
        />
      )}
    </div>
  );
}
