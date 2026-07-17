export const MAX_WATCHLIST_SYMBOLS = 500;

export type AddSymbolsResult = {
  symbols: string[];
  added: string[];
  duplicates: string[];
  omittedCount: number;
};

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

export function addSymbols(
  currentSymbols: string[],
  value: string,
  limit = MAX_WATCHLIST_SYMBOLS,
): AddSymbolsResult {
  const parsed = parseSymbols(value);
  const existing = new Set(currentSymbols);
  const duplicates = parsed.filter((symbol) => existing.has(symbol));
  const candidates = parsed.filter((symbol) => !existing.has(symbol));
  const availableSlots = Math.max(0, limit - currentSymbols.length);
  const added = candidates.slice(0, availableSlots);

  return {
    symbols: [...currentSymbols, ...added],
    added,
    duplicates,
    omittedCount: candidates.length - added.length,
  };
}

export function removeSymbol(symbols: string[], symbol: string): string[] {
  return symbols.filter((candidate) => candidate !== symbol);
}

export function restoreSymbol(symbols: string[], symbol: string, index: number): string[] {
  if (symbols.includes(symbol)) {
    return symbols;
  }

  const restored = [...symbols];
  const insertionIndex = Math.max(0, Math.min(index, restored.length));
  restored.splice(insertionIndex, 0, symbol);
  return restored;
}
