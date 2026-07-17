import { describe, expect, it, vi, beforeEach, afterEach } from "vitest";

import { getBackendClient, resetBackendClientForTest } from "@/lib/backend/client";
import { resolveBackendMode } from "@/lib/backend/resolve-client";
import { MockBackendClient } from "@/lib/backend/mock-client";

describe("backend resolver", () => {
  const originalEnv = process.env.NEXT_PUBLIC_STOCK_BACKEND;
  const originalWindow = (globalThis as Record<string, unknown>).window;

  beforeEach(() => {
    resetBackendClientForTest();
    delete (globalThis as Record<string, unknown>).window;
    delete process.env.NEXT_PUBLIC_STOCK_BACKEND;
  });

  afterEach(() => {
    if (originalEnv !== undefined) {
      process.env.NEXT_PUBLIC_STOCK_BACKEND = originalEnv;
    } else {
      delete process.env.NEXT_PUBLIC_STOCK_BACKEND;
    }
    if (originalWindow !== undefined) {
      (globalThis as Record<string, unknown>).window = originalWindow;
    } else {
      delete (globalThis as Record<string, unknown>).window;
    }
  });

  it("returns mock mode when env is explicitly mock", () => {
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
    expect(resolveBackendMode()).toBe("mock");
  });

  it("returns tauri mode when env is explicitly tauri", () => {
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "tauri";
    expect(resolveBackendMode()).toBe("tauri");
  });

  it("returns mock mode in browser (no Tauri runtime) with auto", () => {
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "auto";
    expect(resolveBackendMode()).toBe("mock");
  });

  it("returns tauri mode when Tauri runtime is detected with auto", () => {
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "auto";
    (globalThis as Record<string, unknown>).window = { __TAURI_INTERNALS__: {} };
    expect(resolveBackendMode()).toBe("tauri");
  });

  it("explicit mock env overrides Tauri runtime detection", () => {
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
    (globalThis as Record<string, unknown>).window = { __TAURI_INTERNALS__: {} };
    expect(resolveBackendMode()).toBe("mock");
  });

  it("returns mock mode for unknown env value with warning", () => {
    const warnSpy = vi.spyOn(console, "warn").mockImplementation(() => {});
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "invalid";
    expect(resolveBackendMode()).toBe("mock");
    expect(warnSpy).toHaveBeenCalledWith(
      '[stock] unknown NEXT_PUBLIC_STOCK_BACKEND="invalid", defaulting to "mock"',
    );
    warnSpy.mockRestore();
  });

  it("getBackendClient returns a client with expected interface", async () => {
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
    const client = getBackendClient();
    expect(client.watchlists).toBeDefined();
    expect(client.presets).toBeDefined();
    expect(client.scans).toBeDefined();
    expect(client.events).toBeDefined();
    expect(typeof client.watchlists.list).toBe("function");
    expect(typeof client.presets.list).toBe("function");
    expect(typeof client.scans.start).toBe("function");
    expect(typeof client.events.subscribe).toBe("function");
  });
});
