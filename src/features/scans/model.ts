import type { ScanResult } from "./types";

export type ResultFilter = {
  matchMode?: "and" | "or" | "none";
  includeStale?: boolean;
  symbolFilter?: string;
};

export type ResultSort = {
  field:
    | "symbol"
    | "tradeDate"
    | "currentPrice"
    | "rsi"
    | "mfi"
    | "bollingerLower"
    | "bollingerMiddle"
    | "bollingerUpper"
    | "matchedCount";
  direction: "asc" | "desc";
};

export function filterResults(
  results: ScanResult[],
  filter: ResultFilter,
): ScanResult[] {
  let filtered = [...results];

  if (filter.includeStale === false) {
    filtered = filtered.filter((r) => !r.dataStale);
  }

  if (filter.symbolFilter) {
    const query = filter.symbolFilter.toUpperCase();
    filtered = filtered.filter((r) =>
      r.symbol.toUpperCase().includes(query),
    );
  }

  return filtered;
}

export function sortResults(
  results: ScanResult[],
  sort: ResultSort,
): ScanResult[] {
  const sorted = [...results];
  const { field, direction } = sort;
  const multiplier = direction === "asc" ? 1 : -1;

  sorted.sort((a, b) => {
    let comparison = 0;

    switch (field) {
      case "symbol":
        comparison = a.symbol.localeCompare(b.symbol);
        break;
      case "tradeDate":
        comparison = a.tradeDate.localeCompare(b.tradeDate);
        break;
      case "currentPrice":
        comparison = a.currentPrice - b.currentPrice;
        break;
      case "rsi":
        comparison = (a.rsi ?? -1) - (b.rsi ?? -1);
        break;
      case "mfi":
        comparison = (a.mfi ?? -1) - (b.mfi ?? -1);
        break;
      case "bollingerLower":
        comparison = (a.bollingerLower ?? -1) - (b.bollingerLower ?? -1);
        break;
      case "bollingerMiddle":
        comparison = (a.bollingerMiddle ?? -1) - (b.bollingerMiddle ?? -1);
        break;
      case "bollingerUpper":
        comparison = (a.bollingerUpper ?? -1) - (b.bollingerUpper ?? -1);
        break;
      case "matchedCount":
        const aMatched = [
          a.allConditionsMatched,
          a.anyConditionMatched,
        ].filter(Boolean).length;
        const bMatched = [
          b.allConditionsMatched,
          b.anyConditionMatched,
        ].filter(Boolean).length;
        comparison = aMatched - bMatched;
        break;
    }

    return comparison * multiplier;
  });

  return sorted;
}

export function filterByMatchMode(
  results: ScanResult[],
  matchMode: "and" | "or" | "none",
): ScanResult[] {
  switch (matchMode) {
    case "and":
      return results.filter((r) => r.allConditionsMatched);
    case "or":
      return results.filter((r) => r.anyConditionMatched);
    case "none":
    default:
      return results;
  }
}
