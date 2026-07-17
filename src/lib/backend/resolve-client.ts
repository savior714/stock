import type { BackendClient } from "./types";

function isTauriRuntime(): boolean {
  return (
    typeof window !== "undefined" &&
    "__TAURI_INTERNALS__" in window
  );
}

function resolveBackendMode(): "mock" | "tauri" {
  if (typeof process === "undefined" || !process.env?.NEXT_PUBLIC_STOCK_BACKEND) {
    return isTauriRuntime() ? "tauri" : "mock";
  }

  const mode = process.env.NEXT_PUBLIC_STOCK_BACKEND;

  if (mode === "mock") return "mock";
  if (mode === "tauri") return "tauri";
  if (mode === "auto") return isTauriRuntime() ? "tauri" : "mock";

  console.warn(
    `[stock] unknown NEXT_PUBLIC_STOCK_BACKEND="${mode}", defaulting to "mock"`,
  );
  return "mock";
}

export { resolveBackendMode, isTauriRuntime };
