import { listen } from "@tauri-apps/api/event";

import type { ScanEventPayload, ScanEventType } from "./types";

type EventCallback = (payload: ScanEventPayload) => void;

let eventListeners: Array<() => void> = [];

export async function subscribeScanEvent(
  eventType: ScanEventType,
  callback: EventCallback,
): Promise<() => void> {
  const unsubscribe = await listen<ScanEventPayload>(eventType, (event) => {
    callback(event.payload);
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
