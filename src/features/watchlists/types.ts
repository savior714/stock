export type WatchlistSummary = {
  id: string;
  name: string;
  description: string | null;
  symbolCount: number;
};

export type WatchlistDetail = {
  id: string;
  name: string;
  description: string | null;
  symbols: string[];
};

export type WatchlistInput = {
  name: string;
  description: string | null;
  symbols: string[];
};

export type WatchlistFormState = {
  id: string | null;
  name: string;
  description: string;
  symbolsText: string;
};

export const emptyWatchlistForm = (): WatchlistFormState => ({
  id: null,
  name: "",
  description: "",
  symbolsText: "",
});
