import { getBackendClient } from "@/lib/backend/client";

import type {
  ScanPresetDetail,
  ScanPresetInput,
  ScanPresetSummary,
} from "./types";

export async function listScanPresets(): Promise<ScanPresetSummary[]> {
  return getBackendClient().presets.list();
}

export async function getScanPreset(id: string): Promise<ScanPresetDetail> {
  return getBackendClient().presets.get(id);
}

export async function createScanPreset(input: ScanPresetInput): Promise<ScanPresetDetail> {
  return getBackendClient().presets.create(input);
}

export async function updateScanPreset(
  id: string,
  input: ScanPresetInput,
): Promise<ScanPresetDetail> {
  return getBackendClient().presets.update(id, input);
}

export async function deleteScanPreset(id: string): Promise<void> {
  return getBackendClient().presets.delete(id);
}
