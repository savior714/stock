import { describe, expect, it } from "vitest";

import { parseThemeValue, reconcileSelectedId } from "./scanner-utils";
import { resolveTheme } from "./theme";

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

// ── resolveTheme ─────────────────────────────────────────────────

describe("resolveTheme", () => {
  it('light 모드일 때 light를 반환한다', () => {
    expect(resolveTheme("light", null)).toBe("light");
  });

  it('dark 모드일 때 dark를 반환한다', () => {
    expect(resolveTheme("dark", null)).toBe("dark");
  });

  it('system + dark media일 때 dark를 반환한다', () => {
    const mql = { matches: true } as MediaQueryList;
    expect(resolveTheme("system", mql)).toBe("dark");
  });

  it('system + light media일 때 light를 반환한다', () => {
    const mql = { matches: false } as MediaQueryList;
    expect(resolveTheme("system", mql)).toBe("light");
  });

  it('system + null media일 때 light를 반환한다', () => {
    expect(resolveTheme("system", null)).toBe("light");
  });
});

// ── canStart logic ───────────────────────────────────────────────

describe("scan canStart logic", () => {
  type Item = { id: string };

  function computeCanStart(
    selectedWatchlistId: string,
    selectedPresetId: string,
    watchlists: Item[],
    presets: Item[],
    isRunning: boolean,
    isLoading: boolean,
  ): boolean {
    const watchlistExists = watchlists.some((w) => w.id === selectedWatchlistId);
    const presetExists = presets.some((p) => p.id === selectedPresetId);
    return !!(
      selectedWatchlistId &&
      selectedPresetId &&
      watchlistExists &&
      presetExists &&
      !isRunning &&
      !isLoading
    );
  }

  it("Watchlist가 목록에 없으면 false", () => {
    expect(computeCanStart("w1", "p1", [], [{ id: "p1" }], false, false)).toBe(false);
  });

  it("Preset이 목록에 없으면 false", () => {
    expect(computeCanStart("w1", "p1", [{ id: "w1" }], [], false, false)).toBe(false);
  });

  it("둘 다 존재하면 true", () => {
    expect(computeCanStart("w1", "p1", [{ id: "w1" }], [{ id: "p1" }], false, false)).toBe(true);
  });

  it("실행 중이면 false", () => {
    expect(computeCanStart("w1", "p1", [{ id: "w1" }], [{ id: "p1" }], true, false)).toBe(false);
  });

  it("로딩 중이면 false", () => {
    expect(computeCanStart("w1", "p1", [{ id: "w1" }], [{ id: "p1" }], false, true)).toBe(false);
  });

  it("Watchlist ID가 비어 있으면 false", () => {
    expect(computeCanStart("", "p1", [{ id: "w1" }], [{ id: "p1" }], false, false)).toBe(false);
  });

  it("Preset ID가 비어 있으면 false", () => {
    expect(computeCanStart("w1", "", [{ id: "w1" }], [{ id: "p1" }], false, false)).toBe(false);
  });

  it("둘 다 비어 있으면 false", () => {
    expect(computeCanStart("", "", [], [], false, false)).toBe(false);
  });
});
