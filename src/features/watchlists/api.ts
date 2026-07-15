import { invoke } from "@tauri-apps/api/core";

import type { WatchlistDetail, WatchlistInput, WatchlistSummary } from "./types";

export async function listWatchlists(): Promise<WatchlistSummary[]> {
  return invoke<WatchlistSummary[]>("list_watchlists");
}

export async function getWatchlist(id: string): Promise<WatchlistDetail> {
  return invoke<WatchlistDetail>("get_watchlist", { id });
}

export async function createWatchlist(input: WatchlistInput): Promise<WatchlistDetail> {
  return invoke<WatchlistDetail>("create_watchlist", { request: input });
}

export async function updateWatchlist(
  id: string,
  input: WatchlistInput,
): Promise<WatchlistDetail> {
  return invoke<WatchlistDetail>("update_watchlist", {
    request: { id, ...input },
  });
}

export async function deleteWatchlist(id: string): Promise<void> {
  return invoke<void>("delete_watchlist", { id });
}

export function parseSymbols(value: string): string[] {
  const seen = new Set<string>();

  return value
    .split(/[\s,;]+/)
    .map((symbol) => symbol.trim().toUpperCase())
    .filter((symbol) => symbol.length > 0)
    .filter((symbol) => {
      if (seen.has(symbol)) {
        return false;
      }
      seen.add(symbol);
      return true;
    });
}
