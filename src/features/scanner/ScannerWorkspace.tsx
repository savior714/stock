"use client";

import { useCallback, useEffect, useState } from "react";

import ScanPresetWorkspace from "@/features/scan-presets/ScanPresetWorkspace";
import ScanRunSetup from "@/features/scans/ScanRunSetup";
import WatchlistWorkspace from "@/features/watchlists/WatchlistWorkspace";

import styles from "./ScannerWorkspace.module.css";

type DrawerView = "watchlists" | "presets";

const DRAWER_TITLES: Record<DrawerView, string> = {
  watchlists: "Watchlists 관리",
  presets: "Scan Presets 관리",
};

export default function ScannerWorkspace() {
  const [drawer, setDrawer] = useState<DrawerView | null>(null);
  const [setupVersion, setSetupVersion] = useState(0);

  const closeDrawer = useCallback(() => {
    setDrawer(null);
    setSetupVersion((version) => version + 1);
  }, []);

  useEffect(() => {
    if (!drawer) {
      return;
    }

    const previousOverflow = document.body.style.overflow;
    document.body.style.overflow = "hidden";

    const handleKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        closeDrawer();
      }
    };

    window.addEventListener("keydown", handleKeyDown);
    return () => {
      document.body.style.overflow = previousOverflow;
      window.removeEventListener("keydown", handleKeyDown);
    };
  }, [drawer, closeDrawer]);

  return (
    <>
      <div className={styles.workspace}>
        <section className={`panel ${styles.resourcePanel}`}>
          <div className={styles.resourceHeader}>
            <div>
              <p className="eyebrow">Scanner resources</p>
              <h3>분석 대상과 조건 관리</h3>
            </div>
            <p className={styles.resourceDescription}>
              스캔에 사용할 Watchlist와 Preset을 Drawer에서 관리합니다.
            </p>
          </div>

          <div className={styles.resourceActions}>
            <button
              className={styles.resourceButton}
              type="button"
              onClick={() => setDrawer("watchlists")}
            >
              <span>
                <strong>Watchlists</strong>
                <small>분석 종목을 추가하거나 삭제합니다.</small>
              </span>
              <b>관리</b>
            </button>

            <button
              className={styles.resourceButton}
              type="button"
              onClick={() => setDrawer("presets")}
            >
              <span>
                <strong>Scan Presets</strong>
                <small>RSI, MFI, Bollinger 조건을 구성합니다.</small>
              </span>
              <b>관리</b>
            </button>
          </div>
        </section>

        <ScanRunSetup key={setupVersion} />
      </div>

      {drawer ? (
        <div
          className={styles.backdrop}
          onMouseDown={(event) => {
            if (event.target === event.currentTarget) {
              closeDrawer();
            }
          }}
        >
          <aside
            className={styles.drawer}
            role="dialog"
            aria-modal="true"
            aria-labelledby="scanner-drawer-title"
          >
            <header className={styles.drawerHeader}>
              <div>
                <p className="eyebrow">Scanner management</p>
                <h3 id="scanner-drawer-title">{DRAWER_TITLES[drawer]}</h3>
              </div>
              <button
                className={styles.closeButton}
                type="button"
                onClick={closeDrawer}
                aria-label="관리 Drawer 닫기"
              >
                ×
              </button>
            </header>

            <div className={styles.drawerTabs} role="tablist" aria-label="관리 대상">
              <button
                className={`${styles.drawerTab} ${
                  drawer === "watchlists" ? styles.activeDrawerTab : ""
                }`}
                type="button"
                role="tab"
                aria-selected={drawer === "watchlists"}
                onClick={() => setDrawer("watchlists")}
              >
                Watchlists
              </button>
              <button
                className={`${styles.drawerTab} ${
                  drawer === "presets" ? styles.activeDrawerTab : ""
                }`}
                type="button"
                role="tab"
                aria-selected={drawer === "presets"}
                onClick={() => setDrawer("presets")}
              >
                Scan Presets
              </button>
            </div>

            <div className={styles.drawerBody}>
              {drawer === "watchlists" ? <WatchlistWorkspace /> : <ScanPresetWorkspace />}
            </div>
          </aside>
        </div>
      ) : null}
    </>
  );
}
