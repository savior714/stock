import { describe, expect, it } from "vitest";

import { parseThemeValue, reconcileSelectedId } from "./scanner-utils";

// ── reconcileSelectedId ──────────────────────────────────────────

describe("reconcileSelectedId", () => {
  it("선택 ID가 목록에 존재하면 해당 ID를 유지한다", () => {
    const items = [{ id: "a" }, { id: "b" }, { id: "c" }];
    expect(reconcileSelectedId("b", items)).toBe("b");
  });

  it("선택 ID가 삭제됐으면 빈 문자열로 정리한다", () => {
    const items = [{ id: "a" }, { id: "c" }];
    expect(reconcileSelectedId("b", items)).toBe("");
  });

  it("빈 선택 ID는 빈 문자열로 유지한다", () => {
    const items = [{ id: "a" }, { id: "b" }];
    expect(reconcileSelectedId("", items)).toBe("");
  });

  it("빈 목록에서 ID를 선택하면 빈 문자열을 반환한다", () => {
    expect(reconcileSelectedId("any", [])).toBe("");
  });

  it("Watchlist와 Preset에 모두 적용할 수 있다", () => {
    type Watchlist = { id: string; name: string };
    type Preset = { id: string; name: string };

    const watchlists: Watchlist[] = [{ id: "w1", name: "Tech" }];
    const presets: Preset[] = [{ id: "p1", name: "RSI Cross" }];

    expect(reconcileSelectedId("w1", watchlists)).toBe("w1");
    expect(reconcileSelectedId("w2", watchlists)).toBe("");
    expect(reconcileSelectedId("p1", presets)).toBe("p1");
    expect(reconcileSelectedId("p2", presets)).toBe("");
  });
});

// ── parseThemeValue ──────────────────────────────────────────────

describe("parseThemeValue", () => {
  it('light 값은 light를 반환한다', () => {
    expect(parseThemeValue("light")).toBe("light");
  });

  it('dark 값은 dark를 반환한다', () => {
    expect(parseThemeValue("dark")).toBe("dark");
  });

  it('system 값은 system를 반환한다', () => {
    expect(parseThemeValue("system")).toBe("system");
  });

  it("null은 light로 fallback한다", () => {
    expect(parseThemeValue(null)).toBe("light");
  });

  it("잘못된 값은 light로 fallback한다", () => {
    expect(parseThemeValue("")).toBe("light");
    expect(parseThemeValue("darkmode")).toBe("light");
    expect(parseThemeValue("Dark")).toBe("light");
    expect(parseThemeValue("  light  ")).toBe("light");
  });
});
