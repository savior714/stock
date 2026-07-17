"use client";

import type { WatchlistSummary } from "@/features/watchlists/types";

type SidebarProps = {
  activeSection: "Scanner" | "Results" | "Logs";
  onSectionChange: (section: "Scanner" | "Results" | "Logs") => void;
  watchlists: WatchlistSummary[];
  selectedWatchlistId: string;
  onWatchlistSelect: (id: string) => void;
  onOpenWatchlistDrawer: () => void;
  onOpenPresetDrawer: () => void;
  watchlistLoading: boolean;
  watchlistError: string | null;
};

const NAV_ITEMS: Array<{ value: "Scanner" | "Results" | "Logs"; label: string }> = [
  { value: "Scanner", label: "Scanner" },
  { value: "Results", label: "Results" },
  { value: "Logs", label: "Logs" },
];

export default function Sidebar({
  activeSection,
  onSectionChange,
  watchlists,
  selectedWatchlistId,
  onWatchlistSelect,
  onOpenWatchlistDrawer,
  onOpenPresetDrawer,
  watchlistLoading,
  watchlistError,
}: SidebarProps) {
  return (
    <aside className="sidebar" aria-label="Main navigation">
      <div className="sidebar-brand">
        <h1>Stock Scanner</h1>
      </div>

      <nav className="sidebar-nav" aria-label="Primary navigation">
        {NAV_ITEMS.map((item) => (
          <button
            key={item.value}
            className={`nav-button${activeSection === item.value ? " active" : ""}`}
            type="button"
            onClick={() => onSectionChange(item.value)}
          >
            {item.label}
          </button>
        ))}
      </nav>

      <div className="sidebar-section">
        <div className="sidebar-section-header">
          <p className="sidebar-section-title">Watchlists</p>
          <div style={{ display: "flex", gap: "4px" }}>
            <button
              className="sidebar-add-btn"
              type="button"
              onClick={onOpenWatchlistDrawer}
              aria-label="Watchlist 추가"
              title="Watchlist 추가"
            >
              +
            </button>
            <button
              className="sidebar-manage-btn"
              type="button"
              onClick={onOpenPresetDrawer}
              aria-label="Watchlist 관리"
              title="관리"
            >
              &middot;&middot;&middot;
            </button>
          </div>
        </div>

        {watchlistLoading ? (
          <div className="sidebar-empty">불러오는 중...</div>
        ) : watchlistError ? (
          <div className="sidebar-error">{watchlistError}</div>
        ) : watchlists.length === 0 ? (
          <div className="sidebar-empty">
            <strong>저장된 Watchlist가 없습니다.</strong>
            <span>+ 버튼을 만들어 분석 대상을 구성하십시오.</span>
          </div>
        ) : (
          <div className="sidebar-list" role="list" aria-label="Watchlist 목록">
            {watchlists.map((wl) => (
              <button
                key={wl.id}
                role="listitem"
                className={`sidebar-list-item${selectedWatchlistId === wl.id ? " active" : ""}`}
                type="button"
                onClick={() => onWatchlistSelect(wl.id)}
              >
                <span>
                  <strong>{wl.name}</strong>
                  {wl.description && <small>{wl.description}</small>}
                </span>
                <b>{wl.symbolCount}</b>
              </button>
            ))}
          </div>
        )}
      </div>
    </aside>
  );
}
