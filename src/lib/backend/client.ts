import type { BackendClient } from "./types";
import { resolveBackendMode } from "./resolve-client";
import { TauriBackendClient } from "./tauri-client";
import { MockBackendClient } from "./mock-client";

let cachedClient: BackendClient | null = null;
let cachedMode: "mock" | "tauri" | null = null;

function createClient(): BackendClient {
  const mode = resolveBackendMode();
  if (mode === "tauri") {
    return new TauriBackendClient();
  }
  return new MockBackendClient();
}

export function getBackendClient(): BackendClient {
  const mode = resolveBackendMode();
  if (cachedClient && cachedMode === mode) {
    return cachedClient;
  }
  cachedClient = createClient();
  cachedMode = mode;
  return cachedClient;
}

export function setBackendClientForTest(client: BackendClient): void {
  cachedClient = client;
  cachedMode = null;
}

export function resetBackendClientForTest(): void {
  cachedClient = null;
  cachedMode = null;
}
