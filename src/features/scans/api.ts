import { getBackendClient } from "@/lib/backend/client";

import type {
  ScanError,
  ScanResult,
  ScanRunDetail,
  ScanRunSummary,
  StartScanRequest,
} from "./types";

export async function startScan(request: StartScanRequest): Promise<string> {
  return getBackendClient().scans.start(request);
}

export async function retryScan(runId: string): Promise<string> {
  return getBackendClient().scans.retry(runId);
}

export async function listScanRuns(limit?: number): Promise<ScanRunSummary[]> {
  return getBackendClient().scans.listRuns(limit);
}

export async function getScanRun(runId: string): Promise<ScanRunDetail> {
  return getBackendClient().scans.getRun(runId);
}

export async function getScanResults(
  runId: string,
  filter?: "and" | "or" | undefined,
): Promise<ScanResult[]> {
  return getBackendClient().scans.getResults(runId, filter);
}

export async function getScanErrors(runId: string): Promise<ScanError[]> {
  return getBackendClient().scans.getErrors(runId);
}

export async function cancelScan(runId: string): Promise<void> {
  return getBackendClient().scans.cancel(runId);
}
