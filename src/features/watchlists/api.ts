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

export function errorMessage(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }

  if (error && typeof error === "object") {
    const candidate = error as { message?: unknown; detail?: unknown };
    const message = typeof candidate.message === "string" ? candidate.message : null;
    const detail = typeof candidate.detail === "string" ? candidate.detail : null;

    if (message && detail) {
      return `${message}: ${detail}`;
    }
    if (message) {
      return message;
    }
  }

  return "알 수 없는 오류가 발생했습니다.";
}
