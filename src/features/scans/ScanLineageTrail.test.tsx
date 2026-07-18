import { describe, expect, it, vi } from "vitest";
import { render, screen, fireEvent } from "@/test/render";
import ScanLineageTrail from "./ScanLineageTrail";
import type { ScanRunDetail } from "./types";

function makeRun(id: string, retryOfRunId: string | null = null): ScanRunDetail {
  return {
    id,
    watchlistId: "wl-1",
    presetId: "preset-1",
    status: "completed",
    baseTradeDate: "2025-07-17",
    totalSymbols: 5,
    succeededSymbols: 5,
    failedSymbols: 0,
    startedAt: "2025-07-17T00:00:00Z",
    finishedAt: "2025-07-17T00:01:00Z",
    presetSnapshotJson: {},
    symbolsSnapshotJson: [],
    retryOfRunId,
  };
}

describe("ScanLineageTrail", () => {
  it("renders Original → Retry 1 → Retry 2 trail", () => {
    const original = makeRun("run-a");
    const retry1 = makeRun("run-b", "run-a");
    const retry2 = makeRun("run-c", "run-b");

    render(
      <ScanLineageTrail
        runs={[original, retry1, retry2]}
        currentRunId="run-c"
      />,
    );

    expect(screen.getByRole("navigation", { name: /scan lineage/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /original/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /retry 1/i })).toBeInTheDocument();
    expect(screen.getByRole("button", { name: /retry 2/i })).toBeInTheDocument();
  });

  it("disables current node", () => {
    const original = makeRun("run-a");
    const retry1 = makeRun("run-b", "run-a");

    render(
      <ScanLineageTrail
        runs={[original, retry1]}
        currentRunId="run-b"
        onRunSelect={() => {}}
      />,
    );

    const currentButton = screen.getByRole("button", { name: /retry 1/i });
    expect(currentButton).toBeDisabled();
  });

  it("sets aria-current on current node", () => {
    const original = makeRun("run-a");
    const retry1 = makeRun("run-b", "run-a");

    render(
      <ScanLineageTrail
        runs={[original, retry1]}
        currentRunId="run-b"
      />,
    );

    const currentButton = screen.getByRole("button", { name: /retry 1/i });
    expect(currentButton).toHaveAttribute("aria-current", "step");
  });

  it("calls onRunSelect with correct run when clicking previous node", () => {
    const original = makeRun("run-a");
    const retry1 = makeRun("run-b", "run-a");
    const retry2 = makeRun("run-c", "run-b");
    const handler = vi.fn();

    render(
      <ScanLineageTrail
        runs={[original, retry1, retry2]}
        currentRunId="run-c"
        onRunSelect={handler}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /original/i }));
    expect(handler).toHaveBeenCalledWith(original);
    expect(handler).toHaveBeenCalledTimes(1);
  });

  it("does not call onRunSelect when clicking current node", () => {
    const original = makeRun("run-a");
    const retry1 = makeRun("run-b", "run-a");
    const handler = vi.fn();

    render(
      <ScanLineageTrail
        runs={[original, retry1]}
        currentRunId="run-b"
        onRunSelect={handler}
      />,
    );

    fireEvent.click(screen.getByRole("button", { name: /retry 1/i }));
    expect(handler).not.toHaveBeenCalled();
  });

  it("hides trail for single run without retry parent", () => {
    const original = makeRun("run-a");

    const { container } = render(
      <ScanLineageTrail
        runs={[original]}
        currentRunId="run-a"
      />,
    );

    expect(container.firstChild).toBeNull();
  });

  it("shows full run ID in title", () => {
    const original = makeRun("run-a");
    const retry1 = makeRun("run-b", "run-a");

    render(
      <ScanLineageTrail
        runs={[original, retry1]}
        currentRunId="run-b"
      />,
    );

    const originalButton = screen.getByRole("button", { name: /original/i });
    expect(originalButton).toHaveAttribute("title", "Run ID: run-a");
  });
});
