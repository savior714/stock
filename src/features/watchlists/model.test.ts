import { describe, expect, it } from "vitest";

import { addSymbols, filterSymbols, parseSymbols, removeSymbol, removeSymbolsBySearch, restoreSymbol } from "./model";

// ── parseSymbols ────────────────────────────────────────────────

describe("parseSymbols", () => {
  it("여러 줄 입력에서 symbol을 추출하고 대문자로 변환한다", () => {
    const input = "AAPL\nGOOG,msft\tTSLA;amzn";
    const result = parseSymbols(input);
    expect(result).toEqual(["AAPL", "GOOG", "MSFT", "TSLA", "AMZN"]);
  });

  it("중복 symbol은 제거한다", () => {
    const input = "AAPL,aapl,AAPL,goog,GOOG";
    const result = parseSymbols(input);
    expect(result).toEqual(["AAPL", "GOOG"]);
  });

  it("빈 문자열은 빈 배열을 반환한다", () => {
    expect(parseSymbols("")).toEqual([]);
  });

  it("공백과 구분자만 있으면 빈 배열을 반환한다", () => {
    expect(parseSymbols("  , ; \t")).toEqual([]);
  });

  it("trim으로 앞뒤 공백을 제거한다", () => {
    const input = " aapl , goog ";
    const result = parseSymbols(input);
    expect(result).toEqual(["AAPL", "GOOG"]);
  });
});

// ── symbol editing ──────────────────────────────────────────────

describe("addSymbols", () => {
  it("기존 순서를 유지하며 새 symbol만 추가한다", () => {
    const result = addSymbols(["AAPL", "MSFT"], "msft, nvda, qqq");

    expect(result.symbols).toEqual(["AAPL", "MSFT", "NVDA", "QQQ"]);
    expect(result.added).toEqual(["NVDA", "QQQ"]);
    expect(result.duplicates).toEqual(["MSFT"]);
    expect(result.omittedCount).toBe(0);
  });

  it("최대 개수를 넘는 symbol은 제외한다", () => {
    const result = addSymbols(["AAPL"], "MSFT NVDA QQQ", 3);

    expect(result.symbols).toEqual(["AAPL", "MSFT", "NVDA"]);
    expect(result.omittedCount).toBe(1);
  });
});

describe("symbol removal and restore", () => {
  it("symbol을 제거한 뒤 원래 위치에 복원한다", () => {
    const removed = removeSymbol(["AAPL", "MSFT", "NVDA"], "MSFT");
    const restored = restoreSymbol(removed, "MSFT", 1);

    expect(removed).toEqual(["AAPL", "NVDA"]);
    expect(restored).toEqual(["AAPL", "MSFT", "NVDA"]);
  });

  it("이미 존재하는 symbol은 중복 복원하지 않는다", () => {
    expect(restoreSymbol(["AAPL", "MSFT"], "MSFT", 0)).toEqual(["AAPL", "MSFT"]);
  });
});

// ── symbol search & bulk removal ──────────────────────────────────

describe("filterSymbols", () => {
  it("검색어가 없으면 전체 symbol을 반환한다", () => {
    const result = filterSymbols(["AAPL", "MSFT", "NVDA"], "");
    expect(result).toEqual(["AAPL", "MSFT", "NVDA"]);
  });

  it("대소문자를 구분하지 않고 부분 일치하는 symbol만 필터링한다", () => {
    const result = filterSymbols(["AAPL", "MSFT", "NVDA", "TSLA"], "aapl");
    expect(result).toEqual(["AAPL"]);
  });

  it("여러 symbol이 일치하면 모두 반환한다", () => {
    const result = filterSymbols(["AAPL", "GOOGL", "GOOG", "TSLA"], "goog");
    expect(result).toEqual(["GOOGL", "GOOG"]);
  });

  it("일치하는 symbol이 없으면 빈 배열을 반환한다", () => {
    const result = filterSymbols(["AAPL", "MSFT"], "XYZ");
    expect(result).toEqual([]);
  });

  it("검색어 앞뒤 공백을 제거한다", () => {
    const result = filterSymbols(["AAPL", "MSFT"], "  aapl  ");
    expect(result).toEqual(["AAPL"]);
  });
});

describe("removeSymbolsBySearch", () => {
  it("검색어가 없으면 변경 없는 결과를 반환한다", () => {
    const result = removeSymbolsBySearch(["AAPL", "MSFT", "NVDA"], "");
    expect(result.symbols).toEqual(["AAPL", "MSFT", "NVDA"]);
    expect(result.removed).toEqual([]);
  });

  it("검색어에 일치하는 symbol만 제거하고 나머지는 유지한다", () => {
    const result = removeSymbolsBySearch(["AAPL", "MSFT", "NVDA", "TSLA"], "aapl");
    expect(result.symbols).toEqual(["MSFT", "NVDA", "TSLA"]);
    expect(result.removed).toEqual(["AAPL"]);
  });

  it("여러 symbol이 제거되면 모두 removed에 포함된다", () => {
    const result = removeSymbolsBySearch(["AAPL", "GOOGL", "GOOG", "TSLA"], "goog");
    expect(result.symbols).toEqual(["AAPL", "TSLA"]);
    expect(result.removed).toEqual(["GOOGL", "GOOG"]);
  });

  it("일치하는 symbol이 없으면 제거 목록이 비어있다", () => {
    const result = removeSymbolsBySearch(["AAPL", "MSFT"], "XYZ");
    expect(result.symbols).toEqual(["AAPL", "MSFT"]);
    expect(result.removed).toEqual([]);
  });

  it("대소문자를 구분하지 않고 부분 일치로 제거한다", () => {
    const result = removeSymbolsBySearch(["AAPL", "MSFT", "NVDA"], "ms");
    expect(result.symbols).toEqual(["AAPL", "NVDA"]);
    expect(result.removed).toEqual(["MSFT"]);
  });
});
