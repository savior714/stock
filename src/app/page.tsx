"use client";

import { useState } from "react";

import ScanPresetWorkspace from "@/features/scan-presets/ScanPresetWorkspace";
import WatchlistWorkspace from "@/features/watchlists/WatchlistWorkspace";

const sections = ["Watchlists", "Scan Settings", "Results", "Logs"] as const;
type Section = (typeof sections)[number];

export default function Home() {
  const [active, setActive] = useState<Section>("Watchlists");

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div>
          <p className="eyebrow">macOS daily scanner</p>
          <h1>Stock</h1>
        </div>
        <nav aria-label="Primary navigation">
          {sections.map((section) => (
            <button
              key={section}
              className={active === section ? "nav-button active" : "nav-button"}
              onClick={() => setActive(section)}
            >
              {section}
            </button>
          ))}
        </nav>
      </aside>

      <section className="workspace">
        <header className="workspace-header">
          <div>
            <p className="eyebrow">Personal stock scanner</p>
            <h2>{active}</h2>
          </div>
          <button className="scan-button" disabled>
            Scan unavailable
          </button>
        </header>

        {active === "Watchlists" ? (
          <WatchlistWorkspace />
        ) : active === "Scan Settings" ? (
          <ScanPresetWorkspace />
        ) : (
          <div className="empty-state">
            <h3>{active} module</h3>
            <p>이 영역은 다음 milestone에서 SQLite 기반 도메인 기능과 연결됩니다.</p>
          </div>
        )}
      </section>
    </main>
  );
}
