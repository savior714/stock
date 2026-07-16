import { invoke } from "@tauri-apps/api/core";

import type {
  ScanError,
  ScanResult,
  ScanRunDetail,
  ScanRunSummary,
  StartScanRequest,
} from "./types";

export async function startScan(request: StartScanRequest): Promise<string> {
  return invoke<string>("start_scan", { request });
}

export async function listScanRuns(limit?: number): Promise<ScanRunSummary[]> {
  return invoke<ScanRunSummary[]>("list_scan_runs", { limit });
}

export async function getScanRun(runId: string): Promise<ScanRunDetail> {
  return invoke<ScanRunDetail>("get_scan_run", { runId });
}

export async function getScanResults(
  runId: string,
  filter?: "and" | "or" | undefined,
): Promise<ScanResult[]> {
  return invoke<ScanResult[]>("get_scan_results", { runId, filter });
}

export async function getScanErrors(runId: string): Promise<ScanError[]> {
  return invoke<ScanError[]>("get_scan_errors", { runId });
}

export async function cancelScan(runId: string): Promise<void> {
  return invoke<void>("cancel_scan", { runId });
}
