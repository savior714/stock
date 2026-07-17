"use client";

import type { WatchlistSummary } from "@/features/watchlists/types";
import type { ThemeMode } from "@/lib/theme";

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
  theme: ThemeMode;
  onThemeChange: (theme: ThemeMode) => void;
};

const NAV_ITEMS: Array<{ value: "Scanner" | "Results" | "Logs"; label: string }> = [
  { value: "Scanner", label: "Scanner" },
  { value: "Results", label: "Results" },
  { value: "Logs", label: "Logs" },
];

const THEME_OPTIONS: Array<{ value: ThemeMode; label: string }> = [
  { value: "light", label: "Light" },
  { value: "system", label: "System" },
  { value: "dark", label: "Dark" },
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
  theme,
  onThemeChange,
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
            aria-current={activeSection === item.value ? "page" : undefined}
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
              aria-label="새 Watchlist 만들기"
              title="새 Watchlist 만들기"
            >
              +
            </button>
            <button
              className="sidebar-manage-btn"
              type="button"
              onClick={onOpenPresetDrawer}
              aria-label="Preset 관리"
              title="Preset 관리"
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
            <span>+ 버튼을 눌러 새 Watchlist를 만드십시오.</span>
          </div>
        ) : (
          <ul className="sidebar-list" aria-label="Watchlist 목록">
            {watchlists.map((wl) => (
              <li key={wl.id}>
                <button
                  className={`sidebar-list-item${selectedWatchlistId === wl.id ? " active" : ""}`}
                  type="button"
                  onClick={() => onWatchlistSelect(wl.id)}
                  aria-pressed={selectedWatchlistId === wl.id}
                >
                  <span>
                    <strong>{wl.name}</strong>
                    {wl.description && <small>{wl.description}</small>}
                  </span>
                  <b>{wl.symbolCount}</b>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>

      <div className="sidebar-theme">
        <label htmlFor="theme-select">Theme</label>
        <select
          id="theme-select"
          value={theme}
          onChange={(e) => onThemeChange(e.target.value as ThemeMode)}
        >
          {THEME_OPTIONS.map((opt) => (
            <option key={opt.value} value={opt.value}>
              {opt.label}
            </option>
          ))}
        </select>
      </div>
    </aside>
  );
}
