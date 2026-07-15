use crate::domain::ScanPresetId;
use crate::error::AppResult;
use crate::repository::scan_preset::{
    ScanConditionWrite, ScanPresetDetail, ScanPresetRepository, ScanPresetSummary, ScanPresetWrite,
};
use crate::state::AppState;
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateScanPresetRequest {
    pub name: String,
    pub conditions: Vec<ScanConditionWrite>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateScanPresetRequest {
    pub id: String,
    pub name: String,
    pub conditions: Vec<ScanConditionWrite>,
}

#[tauri::command]
pub fn list_scan_presets(state: State<'_, AppState>) -> AppResult<Vec<ScanPresetSummary>> {
    state.with_database(|database| ScanPresetRepository::new(database).list())
}

#[tauri::command]
pub fn get_scan_preset(id: String, state: State<'_, AppState>) -> AppResult<ScanPresetDetail> {
    let id = ScanPresetId::new(id)?;
    state.with_database(|database| ScanPresetRepository::new(database).get(&id))
}

#[tauri::command]
pub fn create_scan_preset(
    request: CreateScanPresetRequest,
    state: State<'_, AppState>,
) -> AppResult<ScanPresetDetail> {
    state.with_database(|database| {
        ScanPresetRepository::new(database).create(ScanPresetWrite {
            name: request.name,
            conditions: request.conditions,
        })
    })
}

#[tauri::command]
pub fn update_scan_preset(
    request: UpdateScanPresetRequest,
    state: State<'_, AppState>,
) -> AppResult<ScanPresetDetail> {
    let id = ScanPresetId::new(request.id)?;
    state.with_database(|database| {
        ScanPresetRepository::new(database).update(
            &id,
            ScanPresetWrite {
                name: request.name,
                conditions: request.conditions,
            },
        )
    })
}

#[tauri::command]
pub fn delete_scan_preset(id: String, state: State<'_, AppState>) -> AppResult<()> {
    let id = ScanPresetId::new(id)?;
    state.with_database(|database| ScanPresetRepository::new(database).delete(&id))
}
