"use client";

import { resolveBackendMode } from "@/lib/backend/resolve-client";
import styles from "./MockBadge.module.css";

export default function MockBadge() {
  if (resolveBackendMode() !== "mock") {
    return null;
  }

  return (
    <div className={styles.badge} data-testid="mock-badge">
      Mock
    </div>
  );
}
