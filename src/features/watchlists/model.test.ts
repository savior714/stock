import { describe, expect, it } from "vitest";

import { addSymbols, parseSymbols, removeSymbol, restoreSymbol } from "./model";

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
