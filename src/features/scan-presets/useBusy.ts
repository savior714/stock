"use no side effects";

import { useCallback, useRef, useState } from "react";

/**
 * React 18+ batches all setState calls within the same async function.
 * This hook provides a busy flag that can be "unlocked" outside the current batch
 * by scheduling the unlock via setTimeout(0).
 *
 * Returns [isBusy, lock, unlock] where:
 * - isBusy: boolean state (safe to read in render)
 * - lock:   () => void — sets isBusy = true
 * - unlock: () => void — schedules isBusy = false in next macrotask
 */
export function useBusy() {
  const [isBusy, setIsBusy] = useState(false);
  const pendingRef = useRef<(() => void) | null>(null);

  const lock = useCallback(() => {
    setIsBusy(true);
  }, []);

  const unlock = useCallback(() => {
    if (pendingRef.current) {
      clearTimeout(pendingRef.current as unknown as number);
    }
    pendingRef.current = () => {
      setIsBusy(false);
      pendingRef.current = null;
    };
    setTimeout(pendingRef.current, 0);
  }, []);

  return [isBusy, lock, unlock] as const;
}
