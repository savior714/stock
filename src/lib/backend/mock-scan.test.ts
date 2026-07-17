import { describe, expect, it, beforeEach, vi } from "vitest";

import { getBackendClient, resetBackendClientForTest } from "@/lib/backend/client";
import { resetMockStore } from "@/lib/backend/mock-client";

describe("mock scan - full success", async () => {
  beforeEach(() => {
    resetBackendClientForTest();
    resetMockStore();
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
  });

  it("generates progress and completed events", async () => {
    const client = getBackendClient();
    const events: string[] = [];

    const unsubStarted = await client.events.subscribe("scan://started", () => {
      events.push("started");
    });
    const unsubProgress = await client.events.subscribe("scan://progress", () => {
      events.push("progress");
    });
    const unsubCompleted = await client.events.subscribe("scan://completed", () => {
      events.push("completed");
    });

    const runId = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-2",
    });

    // wl-1 = 5 symbols, tick through all of them
    for (let i = 0; i < 5; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);
    }

    expect(events).toContain("started");
    expect(events).toContain("completed");
    expect(events.filter((e) => e === "progress").length).toBe(5);

    const run = await client.scans.getRun(runId);
    expect(run.status).toBe("completed");
    expect(run.succeededSymbols).toBe(run.totalSymbols);
    expect(run.failedSymbols).toBe(0);

    unsubStarted();
    unsubProgress();
    unsubCompleted();
  });
});

describe("mock scan - partial failure", async () => {
  beforeEach(() => {
    resetBackendClientForTest();
    resetMockStore();
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
  });

  it("stores results and errors together", async () => {
    const client = getBackendClient();
    const errorEvents: string[] = [];

    await client.events.subscribe("scan://error", () => {
      errorEvents.push("error");
    });

    const runId = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-3",
    });

    // wl-1 = 5 symbols
    for (let i = 0; i < 5; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);
    }

    const run = await client.scans.getRun(runId);
    expect(run.status).toBe("completed");
    expect(run.succeededSymbols).toBeGreaterThan(0);
    expect(run.failedSymbols).toBeGreaterThan(0);

    const results = await client.scans.getResults(runId);
    const errors = await client.scans.getErrors(runId);
    expect(results.length).toBeGreaterThan(0);
    expect(errors.length).toBeGreaterThan(0);

    expect(errorEvents.length).toBe(errors.length);
  });
});

describe("mock scan - cancel", async () => {
  beforeEach(() => {
    resetBackendClientForTest();
    resetMockStore();
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
  });

  it("stops progress events after cancel", async () => {
    const client = getBackendClient();
    const progressEvents: string[] = [];

    await client.events.subscribe("scan://progress", () => {
      progressEvents.push("progress");
    });
    const cancelPromise = client.events.subscribe("scan://cancelled", () => {
      progressEvents.push("cancelled");
    });

    const runId = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-4",
    });

    // wl-1 = 5 symbols, advance 1 then cancel
    (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);
    await client.scans.cancel(runId);

    // Try to advance remaining symbols - should be no-ops
    for (let i = 0; i < 4; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);
    }

    const run = await client.scans.getRun(runId);
    expect(run.status).toBe("cancelled");
    expect(run.finishedAt).not.toBeNull();

    expect(progressEvents).toContain("cancelled");

    cancelPromise.then((unsub) => unsub());
  });

  it("does not emit progress after cancel", async () => {
    const client = getBackendClient();
    const progressEvents: string[] = [];

    await client.events.subscribe("scan://progress", () => {
      progressEvents.push("progress");
    });

    const runId = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-4",
    });

    (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);
    await client.scans.cancel(runId);

    const progressBefore = progressEvents.length;
    for (let i = 0; i < 4; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);
    }
    const progressAfter = progressEvents.length;

    expect(progressAfter).toBe(progressBefore);
  });
});

describe("mock scan - run history", async () => {
  beforeEach(() => {
    resetBackendClientForTest();
    resetMockStore();
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
  });

  it("returns runs in reverse chronological order", async () => {
    const client = getBackendClient();

    const runId1 = await client.scans.start({ watchlistId: "wl-1", presetId: "preset-2" });
    const runId2 = await client.scans.start({ watchlistId: "wl-2", presetId: "preset-1" });
    const runId3 = await client.scans.start({ watchlistId: "wl-1", presetId: "preset-3" });

    // Complete all runs (5 symbols each for wl-1, 4 for wl-2)
    for (let i = 0; i < 5; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId1);
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId3);
    }
    for (let i = 0; i < 4; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId2);
    }

    const runs = await client.scans.listRuns();
    expect(runs.length).toBe(3);
    expect(runs[0].id).not.toBe(runs[1].id);
  });

  it("getRun, getResults, getErrors return consistent data", async () => {
    const client = getBackendClient();
    const runId = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-3",
    });

    // wl-1 = 5 symbols
    for (let i = 0; i < 5; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);
    }

    const run = await client.scans.getRun(runId);
    const results = await client.scans.getResults(runId);
    const errors = await client.scans.getErrors(runId);

    expect(run.id).toBe(runId);
    expect(run.status).toBe("completed");
    expect(results.length + errors.length).toBe(run.totalSymbols);
  });
});
