import { describe, expect, it } from "vitest";

import { parseSymbols } from "./api";

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
