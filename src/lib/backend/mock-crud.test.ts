import { describe, expect, it, beforeEach } from "vitest";

import { getBackendClient, resetBackendClientForTest } from "@/lib/backend/client";
import { resetMockStore } from "@/lib/backend/mock-client";

describe("mock CRUD - watchlists", () => {
  beforeEach(() => {
    resetBackendClientForTest();
    resetMockStore();
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
  });

  it("creates, modifies, and deletes a watchlist", async () => {
    const client = getBackendClient();

    // Create
    const created = await client.watchlists.create({
      name: "Test List",
      description: "test desc",
      symbols: ["AAPL", "MSFT"],
    });
    expect(created.id).toMatch(/^mock-wl-\d+$/);
    expect(created.name).toBe("Test List");
    expect(created.symbols).toEqual(["AAPL", "MSFT"]);

    // List includes it
    const list = await client.watchlists.list();
    expect(list.find((w) => w.id === created.id)).toBeDefined();

    // Update
    const updated = await client.watchlists.update(created.id, {
      name: "Updated List",
      description: "updated desc",
      symbols: ["AAPL", "MSFT", "NVDA"],
    });
    expect(updated.name).toBe("Updated List");
    expect(updated.symbols).toEqual(["AAPL", "MSFT", "NVDA"]);

    // Delete
    await client.watchlists.delete(created.id);
    const afterDelete = await client.watchlists.list();
    expect(afterDelete.find((w) => w.id === created.id)).toBeUndefined();
  });

  it("reconciles selection after deletion", async () => {
    const client = getBackendClient();
    const list = await client.watchlists.list();
    const first = list[0];
    const idToDelete = first.id;

    await client.watchlists.delete(idToDelete);
    const updated = await client.watchlists.list();
    expect(updated.find((w) => w.id === idToDelete)).toBeUndefined();
    expect(updated.length).toBe(list.length - 1);
  });
});

describe("mock CRUD - presets", () => {
  beforeEach(() => {
    resetBackendClientForTest();
    resetMockStore();
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
  });

  it("creates, modifies, and deletes a preset", async () => {
    const client = getBackendClient();

    const created = await client.presets.create({
      name: "Test Preset",
      conditions: [
        {
          indicator: "rsi",
          side: "lower",
          period: 14,
          threshold: 30,
          stdDevMultiplier: null,
          triggerMode: "current",
          enabled: true,
        },
      ],
    });
    expect(created.id).toMatch(/^mock-preset-\d+$/);
    expect(created.name).toBe("Test Preset");

    const list = await client.presets.list();
    expect(list.find((p) => p.id === created.id)).toBeDefined();

    const updated = await client.presets.update(created.id, {
      name: "Updated Preset",
      conditions: [
        {
          indicator: "rsi",
          side: "upper",
          period: 14,
          threshold: 70,
          stdDevMultiplier: null,
          triggerMode: "cross",
          enabled: true,
        },
      ],
    });
    expect(updated.name).toBe("Updated Preset");

    await client.presets.delete(created.id);
    const afterDelete = await client.presets.list();
    expect(afterDelete.find((p) => p.id === created.id)).toBeUndefined();
  });
});

describe("mock store - localStorage recovery", () => {
  beforeEach(() => {
    resetBackendClientForTest();
    resetMockStore();
    localStorage.clear();
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
  });

  it("recovers from corrupted localStorage data", () => {
    localStorage.setItem("stock.mock.backend.v1", "corrupted{data");
    resetMockStore();
    const client = getBackendClient();
    expect(client).toBeDefined();
  });

  it("persists data across store resets", async () => {
    const client = getBackendClient();
    await client.watchlists.create({
      name: "Persisted List",
      description: null,
      symbols: ["AAPL"],
    });

    // Reset and re-create client
    resetMockStore();
    const client2 = getBackendClient();
    const list = await client2.watchlists.list();
    expect(list.find((w) => w.name === "Persisted List")).toBeDefined();
  });
});
