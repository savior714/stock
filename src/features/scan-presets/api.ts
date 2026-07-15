import { invoke } from "@tauri-apps/api/core";

import type {
  ScanPresetDetail,
  ScanPresetInput,
  ScanPresetSummary,
} from "./types";

export async function listScanPresets(): Promise<ScanPresetSummary[]> {
  return invoke<ScanPresetSummary[]>("list_scan_presets");
}

export async function getScanPreset(id: string): Promise<ScanPresetDetail> {
  return invoke<ScanPresetDetail>("get_scan_preset", { id });
}

export async function createScanPreset(input: ScanPresetInput): Promise<ScanPresetDetail> {
  return invoke<ScanPresetDetail>("create_scan_preset", { request: input });
}

export async function updateScanPreset(
  id: string,
  input: ScanPresetInput,
): Promise<ScanPresetDetail> {
  return invoke<ScanPresetDetail>("update_scan_preset", {
    request: { id, ...input },
  });
}

export async function deleteScanPreset(id: string): Promise<void> {
  return invoke<void>("delete_scan_preset", { id });
}
