/**
 * Reconcile a selected ID against a list of items.
 * Returns the selected ID if it exists in the list, otherwise returns an empty string.
 */
export function reconcileSelectedId<T extends { id: string }>(
  selectedId: string,
  items: T[],
): string {
  if (!selectedId) return "";
  return items.some((item) => item.id === selectedId) ? selectedId : "";
}

/**
 * Parse and validate a theme string from localStorage.
 * Returns "light" for any invalid or missing value.
 */
export function parseThemeValue(raw: string | null): "light" | "dark" | "system" {
  if (raw === "light" || raw === "dark" || raw === "system") {
    return raw;
  }
  return "light";
}

export function resolveTheme(mode: "light" | "dark" | "system", mql: MediaQueryList | null): "light" | "dark" {
  if (mode === "system") {
    return mql && mql.matches ? "dark" : "light";
  }
  return mode;
}

export type CanStartScanInput = {
  selectedWatchlistId: string;
  selectedPresetId: string;
  watchlistExists: boolean;
  presetExists: boolean;
  isRunning: boolean;
  isLoading: boolean;
};

export function canStartScan(input: CanStartScanInput): boolean {
  return Boolean(
    input.selectedWatchlistId
    && input.selectedPresetId
    && input.watchlistExists
    && input.presetExists
    && !input.isRunning
    && !input.isLoading,
  );
}
