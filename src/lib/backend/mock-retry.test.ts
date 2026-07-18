import { describe, expect, it, beforeEach, vi } from "vitest";

import { getBackendClient, resetBackendClientForTest } from "@/lib/backend/client";
import { resetMockStore } from "@/lib/backend/mock-client";

describe("mock retry - basic flow", () => {
  beforeEach(() => {
    resetBackendClientForTest();
    resetMockStore();
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
  });

  it("retries only retryable symbols", async () => {
    const client = getBackendClient();

    // Run scan with preset-3 (partial failure: GOOGL=retryable, AMD=permanent)
    const runId = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-3",
    });

    // Complete all 9 symbols
    for (let i = 0; i < 9; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);
    }

    const run = await client.scans.getRun(runId);
    expect(run.status).toBe("completed");
    expect(run.failedSymbols).toBeGreaterThan(0);

    // Get errors
    const errors = await client.scans.getErrors(runId);
    const retryableErrors = errors.filter((e) => e.retryable && e.symbol !== null);
    expect(retryableErrors.length).toBeGreaterThan(0);

    // Retry
    const retryRunId = await client.scans.retry(runId);
    expect(retryRunId).not.toBe(runId);

    // Verify retry run detail
    const retryRun = await client.scans.getRun(retryRunId);
    expect(retryRun.retryOfRunId).toBe(runId);
    expect(retryRun.totalSymbols).toBe(retryableErrors.length);
    expect(retryRun.presetSnapshotJson).not.toEqual({});
    expect(retryRun.symbolsSnapshotJson.length).toBe(retryableErrors.length);

    // Complete retry run
    for (let i = 0; i < retryableErrors.length; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(retryRunId);
    }

    const finalRetry = await client.scans.getRun(retryRunId);
    expect(finalRetry.status).toBe("completed");
  });

  it("preserves original watchlist/preset ids after resource deletion", async () => {
    const client = getBackendClient();

    const runId = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-3",
    });

    for (let i = 0; i < 9; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);
    }

    const originalRun = await client.scans.getRun(runId);
    const originalWlId = originalRun.watchlistId;
    const originalPsId = originalRun.presetId;

    // Delete watchlist and preset
    (client as unknown as { watchlists: { delete: (id: string) => Promise<void> } }).watchlists.delete(originalWlId);
    (client as unknown as { presets: { delete: (id: string) => Promise<void> } }).presets.delete(originalPsId);

    // Retry should still work using snapshot data
    const retryRunId = await client.scans.retry(runId);
    const retryRun = await client.scans.getRun(retryRunId);
    expect(retryRun.watchlistId).toBe(originalWlId);
    expect(retryRun.presetId).toBe(originalPsId);
  });

  it("rejects retry when no retryable symbols", async () => {
    const client = getBackendClient();

    const runId = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-2", // All success
    });

    for (let i = 0; i < 9; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);
    }

    const run = await client.scans.getRun(runId);
    expect(run.status).toBe("completed");
    expect(run.failedSymbols).toBe(0);

    // Retry should fail - no retryable errors
    await expect(client.scans.retry(runId)).rejects.toThrow(/no retryable/i);
  });

  it("rejects retry on non-terminal status", async () => {
    const client = getBackendClient();

    const runId = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-3",
    });

    // Don't complete the run - it's still running
    await expect(client.scans.retry(runId)).rejects.toThrow(/running/);
  });

  it("retry of retry references previous run", async () => {
    const client = getBackendClient();

    // First run
    const runId1 = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-3",
    });
    for (let i = 0; i < 5; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId1);
    }

    // First retry
    const runId2 = await client.scans.retry(runId1);
    const retryRun2 = await client.scans.getRun(runId2);
    const retry2Count = retryRun2.totalSymbols;
    for (let i = 0; i < retry2Count; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId2);
    }

    // Second retry
    const runId3 = await client.scans.retry(runId2);

    const r3 = await client.scans.getRun(runId3);
    expect(r3.retryOfRunId).toBe(runId2);
  });

  it("uses watchlist symbols not fixture symbols", async () => {
    const client = getBackendClient();

    // wl-2 has 4 symbols: AMD, TSM, AVGO, COST
    const runId = await client.scans.start({
      watchlistId: "wl-2",
      presetId: "preset-2", // All success
    });

    for (let i = 0; i < 4; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);
    }

    const run = await client.scans.getRun(runId);
    expect(run.totalSymbols).toBe(4);
    expect((run.symbolsSnapshotJson as string[]).length).toBe(4);
  });
});

describe("mock retry - result invariants", () => {
  beforeEach(() => {
    resetBackendClientForTest();
    resetMockStore();
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
  });

  it("retry generated results satisfy anyConditionMatched invariant", async () => {
    const client = getBackendClient();

    const runId = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-3",
    });
    for (let i = 0; i < 9; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);
    }

    const errors = await client.scans.getErrors(runId);
    const retryableErrors = errors.filter((e) => e.retryable && e.symbol !== null);
    const retryRunId = await client.scans.retry(runId);

    for (let i = 0; i < retryableErrors.length; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(retryRunId);
    }

    const results = await client.scans.getResults(retryRunId);
    for (const r of results) {
      expect(r.anyConditionMatched).toBe(r.matchedConditionCount > 0);
    }
  });

  it("retry results: no count 0 with OR true", async () => {
    const client = getBackendClient();

    const runId = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-3",
    });
    for (let i = 0; i < 9; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);
    }

    const errors = await client.scans.getErrors(runId);
    const retryableErrors = errors.filter((e) => e.retryable && e.symbol !== null);
    const retryRunId = await client.scans.retry(runId);

    for (let i = 0; i < retryableErrors.length; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(retryRunId);
    }

    const results = await client.scans.getResults(retryRunId);
    const violations = results.filter((r) => r.matchedConditionCount === 0 && r.anyConditionMatched);
    expect(violations).toHaveLength(0);
  });
});
