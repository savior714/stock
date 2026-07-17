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
