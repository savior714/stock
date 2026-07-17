import { getBackendClient } from "@/lib/backend/client";

import type { WatchlistDetail, WatchlistInput, WatchlistSummary } from "./types";

export async function listWatchlists(): Promise<WatchlistSummary[]> {
  return getBackendClient().watchlists.list();
}

export async function getWatchlist(id: string): Promise<WatchlistDetail> {
  return getBackendClient().watchlists.get(id);
}

export async function createWatchlist(input: WatchlistInput): Promise<WatchlistDetail> {
  return getBackendClient().watchlists.create(input);
}

export async function updateWatchlist(
  id: string,
  input: WatchlistInput,
): Promise<WatchlistDetail> {
  return getBackendClient().watchlists.update(id, input);
}

export async function deleteWatchlist(id: string): Promise<void> {
  return getBackendClient().watchlists.delete(id);
}

export { parseSymbols, filterSymbols, removeSymbolsBySearch } from "./model";
