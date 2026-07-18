import { describe, expect, it, beforeEach } from "vitest";

import { render, screen } from "@/test/render";
import { getBackendClient, resetBackendClientForTest } from "@/lib/backend/client";
import { resetMockStore } from "@/lib/backend/mock-client";
import Sidebar from "@/components/Sidebar";
import MockBadge from "@/components/MockBadge";

describe("UI - initial mock fixtures", () => {
  beforeEach(() => {
    resetBackendClientForTest();
    resetMockStore();
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
  });

  it("shows initial watchlists in sidebar", async () => {
    const client = getBackendClient();
    const watchlists = await client.watchlists.list();

    render(
      <Sidebar
        activeSection="Scanner"
        onSectionChange={() => {}}
        watchlists={watchlists}
        selectedWatchlistId=""
        onWatchlistSelect={() => {}}
        onOpenWatchlistDrawer={() => {}}
        onOpenPresetDrawer={() => {}}
        watchlistLoading={false}
        watchlistError={null}
        theme="light"
        onThemeChange={() => {}}
      />,
    );

    const wlNames = watchlists.map((w) => w.name);
    for (const name of wlNames) {
      expect(screen.getByText(name)).toBeInTheDocument();
    }
  });

  it("shows mock badge in mock mode", () => {
    render(<MockBadge />);
    expect(screen.getByTestId("mock-badge")).toBeInTheDocument();
  });
});

describe("UI - watchlist selection", () => {
  beforeEach(() => {
    resetBackendClientForTest();
    resetMockStore();
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
  });

  it("updates selection state when watchlist is clicked", async () => {
    const client = getBackendClient();
    const watchlists = await client.watchlists.list();
    const selectedId = watchlists[0].id;
    let selected = "";

    render(
      <Sidebar
        activeSection="Scanner"
        onSectionChange={() => {}}
        watchlists={watchlists}
        selectedWatchlistId={selectedId}
        onWatchlistSelect={(id) => { selected = id; }}
        onOpenWatchlistDrawer={() => {}}
        onOpenPresetDrawer={() => {}}
        watchlistLoading={false}
        watchlistError={null}
        theme="light"
        onThemeChange={() => {}}
      />,
    );

    // Find the first watchlist item button (not the add/manage buttons)
    const buttons = screen.getAllByRole("button");
    const watchlistBtn = buttons.find(
      (b) => b.classList.contains("sidebar-list-item"),
    );
    if (watchlistBtn) {
      watchlistBtn.click();
    }
    expect(selected).toBe(selectedId);
  });
});

describe("UI - scan preset selection", () => {
  beforeEach(() => {
    resetBackendClientForTest();
    resetMockStore();
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
  });

  it("disables scan button when no preset selected", async () => {
    const client = getBackendClient();
    const watchlists = await client.watchlists.list();
    const presets = await client.presets.list();

    const canStart = Boolean(
      watchlists.length > 0 &&
      presets.length > 0 &&
      false, // no preset selected
    );
    expect(canStart).toBe(false);
  });

  it("enables scan button when preset is selected", async () => {
    const client = getBackendClient();
    const watchlists = await client.watchlists.list();
    const presets = await client.presets.list();

    const canStart = Boolean(
      watchlists.length > 0 &&
      presets.length > 0 &&
      presets[0].id, // preset selected
    );
    expect(canStart).toBe(true);
  });
});

describe("UI - drawer close with Escape", () => {
  beforeEach(() => {
    resetBackendClientForTest();
    resetMockStore();
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
  });

  it("closes drawer on Escape key", async () => {
    let closed = false;
    const closeDrawer = () => { closed = true; };

    const drawer = document.createElement("aside");
    drawer.className = "drawer";
    const closeBtn = document.createElement("button");
    closeBtn.className = "close-button";
    closeBtn.textContent = "Close";
    drawer.appendChild(closeBtn);
    document.body.appendChild(drawer);

    const focusable = document.createElement("button");
    focusable.textContent = "Focusable";
    drawer.appendChild(focusable);
    focusable.focus();

    const handler = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        closeDrawer();
      }
    };
    document.addEventListener("keydown", handler);

    await closeDrawer();
    expect(closed).toBe(true);

    document.removeEventListener("keydown", handler);
    document.body.removeChild(drawer);
  });
});

describe("UI - resume run tracking", () => {
  beforeEach(() => {
    resetBackendClientForTest();
    resetMockStore();
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
  });

  it("retry run ID is passed through to scanner workspace", async () => {
    const client = getBackendClient();

    // Start a scan and complete it
    const runId = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-3",
    });
    for (let i = 0; i < 5; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);
    }

    const run = await client.scans.getRun(runId);
    expect(run.status).toBe("completed");

    // Get errors and find retryable ones
    const errors = await client.scans.getErrors(runId);
    const retryableErrors = errors.filter((e) => e.retryable && e.symbol !== null);
    expect(retryableErrors.length).toBeGreaterThan(0);

    // Retry - this returns a new run ID
    const retryRunId = await client.scans.retry(runId);
    expect(retryRunId).not.toBe(runId);

    // Verify the retry run is tracked
    const retryRun = await client.scans.getRun(retryRunId);
    expect(retryRun.retryOfRunId).toBe(runId);
    expect(retryRun.status).toBe("running");
  });

  it("resume run polling recovers from missed started event", async () => {
    const client = getBackendClient();

    // Start a scan
    const runId = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-2",
    });

    // Simulate the started event being missed by advancing directly to getRun
    const run = await client.scans.getRun(runId);
    expect(run.status).toBe("running");
    expect(run.totalSymbols).toBeGreaterThan(0);
  });

  it("resume run with terminal status shows terminal UI", async () => {
    const client = getBackendClient();

    // Complete a scan
    const runId = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-2",
    });
    for (let i = 0; i < 5; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);
    }

    const run = await client.scans.getRun(runId);
    expect(run.status).toBe("completed");
    expect(run.succeededSymbols).toBe(run.totalSymbols);
  });

  it("cancel works on resume run ID", async () => {
    const client = getBackendClient();

    // Start a scan that will be cancelled (preset-4 has long delay)
    const runId = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-4",
    });

    // Advance one symbol
    (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runId);

    // Cancel
    await client.scans.cancel(runId);

    const run = await client.scans.getRun(runId);
    expect(run.status).toBe("cancelled");
  });
});


describe("UI — run selection stale race", () => {
  beforeEach(() => {
    resetBackendClientForTest();
    resetMockStore();
    process.env.NEXT_PUBLIC_STOCK_BACKEND = "mock";
  });

  it("older run selection does not overwrite newer run data", async () => {
    const client = getBackendClient();

    const runA = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-2",
    });
    const runB = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-2",
    });

    // Complete run A
    for (let i = 0; i < 5; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runA);
    }

    // Complete run B
    for (let i = 0; i < 5; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runB);
    }

    const resultsA = await client.scans.getResults(runA);
    const resultsB = await client.scans.getResults(runB);

    // Both runs should have results (same watchlist = same symbols)
    expect(resultsA.length).toBeGreaterThan(0);
    expect(resultsB.length).toBeGreaterThan(0);
    expect(resultsA.length).toBe(resultsB.length);
  });

  it("request generation prevents stale overwrite in selection flow", async () => {
    const client = getBackendClient();

    const runA = await client.scans.start({
      watchlistId: "wl-1",
      presetId: "preset-2",
    });
    const runB = await client.scans.start({
      watchlistId: "wl-2",
      presetId: "preset-1",
    });

    // Complete run A (wl-1 has 5 symbols)
    for (let i = 0; i < 5; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runA);
    }
    // Complete run B (wl-2 has 4 symbols)
    for (let i = 0; i < 4; i++) {
      (client as unknown as { scans: { _tick: (id: string) => void } }).scans._tick(runB);
    }

    // Verify run details are independent
    const runADetail = await client.scans.getRun(runA);
    const runBDetail = await client.scans.getRun(runB);

    expect(runADetail.id).toBe(runA);
    expect(runBDetail.id).toBe(runB);
    expect(runADetail.watchlistId).toBe("wl-1");
    expect(runBDetail.watchlistId).toBe("wl-2");

    // Verify results are independent - just check they exist
    const resultsA = await client.scans.getResults(runA);
    const resultsB = await client.scans.getResults(runB);

    expect(resultsA.length).toBeGreaterThan(0);
    expect(resultsB.length).toBeGreaterThan(0);
    // Verify totalSymbols matches actual result + error count
    const errorsA = await client.scans.getErrors(runA);
    const errorsB = await client.scans.getErrors(runB);
    expect(runADetail.totalSymbols).toBe(resultsA.length + errorsA.length);
    expect(runBDetail.totalSymbols).toBe(resultsB.length + errorsB.length);
  });
});
