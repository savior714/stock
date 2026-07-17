"use client";

import { useCallback, useEffect, useState } from "react";

import ScanRunSetup from "@/features/scans/ScanRunSetup";
import type { WatchlistSummary } from "@/features/watchlists/types";

type ScannerWorkspaceProps = {
  selectedWatchlistId: string;
  onWatchlistIdChange: (id: string) => void;
  selectedPresetId: string;
  onPresetIdChange: (id: string) => void;
  onOpenWatchlistDrawer: () => void;
  onOpenPresetDrawer: () => void;
  watchlists: WatchlistSummary[];
};

export default function ScannerWorkspace({
  selectedWatchlistId,
  onWatchlistIdChange,
  selectedPresetId,
  onPresetIdChange,
  onOpenWatchlistDrawer,
  onOpenPresetDrawer,
  watchlists,
}: ScannerWorkspaceProps) {
  const [setupVersion, setSetupVersion] = useState(0);

  const handleDrawerClose = useCallback(() => {
    setSetupVersion((v) => v + 1);
  }, []);

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        handleDrawerClose();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [handleDrawerClose]);

  const selectedWatchlist = watchlists.find(
    (w) => w.id === selectedWatchlistId,
  );

  const hasSelection = selectedWatchlistId && selectedPresetId;

  return (
    <div>
      {!selectedWatchlistId ? (
        <div className="empty-state">
          <h3>스캔할 Watchlist를 선택하십시오.</h3>
          <p>왼쪽 사이드바에서 Watchlist를 선택하거나, + 버튼을 눌러 새 Watchlist를 생성하십시오.</p>
        </div>
      ) : (
        <ScanRunSetup
          key={setupVersion}
          selectedWatchlistId={selectedWatchlistId}
          onWatchlistIdChange={onWatchlistIdChange}
          selectedPresetId={selectedPresetId}
          onPresetIdChange={onPresetIdChange}
          watchlists={watchlists}
          onDrawerOpen={handleDrawerClose}
        />
      )}
    </div>
  );
}
