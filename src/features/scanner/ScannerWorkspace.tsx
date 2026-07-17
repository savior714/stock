"use client";

import ScanRunSetup from "@/features/scans/ScanRunSetup";
import type { WatchlistSummary } from "@/features/watchlists/types";
import type { ScanPresetSummary } from "@/features/scan-presets/types";

type ScannerWorkspaceProps = {
  selectedWatchlistId: string;
  selectedPresetId: string;
  onPresetIdChange: (id: string) => void;
  onOpenWatchlistDrawer: () => void;
  onOpenPresetDrawer: () => void;
  watchlists: WatchlistSummary[];
  presets: ScanPresetSummary[];
  watchlistExists: boolean;
  presetExists: boolean;
  presetsLoading: boolean;
  presetsError: string | null;
};

export default function ScannerWorkspace({
  selectedWatchlistId,
  selectedPresetId,
  onPresetIdChange,
  onOpenWatchlistDrawer,
  onOpenPresetDrawer,
  watchlists,
  presets,
  watchlistExists,
  presetExists,
  presetsLoading,
  presetsError,
}: ScannerWorkspaceProps) {
  if (!selectedWatchlistId || !watchlistExists) {
    return (
      <div className="empty-state">
        <h3>스캔할 Watchlist를 선택하십시오.</h3>
        <p>왼쪽 사이드바에서 Watchlist를 선택하거나, + 버튼을 눌러 새 Watchlist를 생성하십시오.</p>
      </div>
    );
  }

  return (
    <ScanRunSetup
      selectedWatchlistId={selectedWatchlistId}
      selectedPresetId={selectedPresetId}
      onPresetIdChange={onPresetIdChange}
      watchlists={watchlists}
      presets={presets}
      presetsLoading={presetsLoading}
      presetsError={presetsError}
      onOpenPresetDrawer={onOpenPresetDrawer}
      presetExists={presetExists}
      watchlistExists={watchlistExists}
    />
  );
}
