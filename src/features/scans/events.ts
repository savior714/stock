import { listen } from "@tauri-apps/api/event";

import type { ScanEventPayload, ScanEventType } from "./types";

type EventCallback<T = ScanEventPayload> = (payload: T) => void;

let eventListeners: Array<() => void> = [];

export async function subscribeScanEvent<T extends ScanEventType>(
  eventType: T,
  callback: EventCallback<Extract<ScanEventPayload, { runId: string }>>,
): Promise<() => void> {
  const unsubscribe = await listen<ScanEventPayload>(eventType, (event) => {
    callback(event.payload as Extract<ScanEventPayload, { runId: string }>);
  });

  eventListeners.push(unsubscribe);

  return () => {
    unsubscribe();
    eventListeners = eventListeners.filter((l) => l !== unsubscribe);
  };
}

export function unsubscribeAll(): void {
  for (const listener of eventListeners) {
    listener();
  }
  eventListeners = [];
}
