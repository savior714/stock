use crate::domain::WatchlistId;
use crate::error::AppResult;
use crate::repository::watchlist::{
    WatchlistDetail, WatchlistRepository, WatchlistSummary, WatchlistWrite,
};
use crate::state::AppState;
use serde::Deserialize;
use tauri::State;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWatchlistRequest {
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub symbols: Vec<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateWatchlistRequest {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    #[serde(default)]
    pub symbols: Vec<String>,
}

#[tauri::command]
pub fn list_watchlists(state: State<'_, AppState>) -> AppResult<Vec<WatchlistSummary>> {
    state.with_database(|database| WatchlistRepository::new(database).list())
}

#[tauri::command]
pub fn get_watchlist(id: String, state: State<'_, AppState>) -> AppResult<WatchlistDetail> {
    let id = WatchlistId::new(id)?;
    state.with_database(|database| WatchlistRepository::new(database).get(&id))
}

#[tauri::command]
pub fn create_watchlist(
    request: CreateWatchlistRequest,
    state: State<'_, AppState>,
) -> AppResult<WatchlistDetail> {
    state.with_database(|database| {
        WatchlistRepository::new(database).create(WatchlistWrite {
            name: request.name,
            description: request.description,
            symbols: request.symbols,
        })
    })
}

#[tauri::command]
pub fn update_watchlist(
    request: UpdateWatchlistRequest,
    state: State<'_, AppState>,
) -> AppResult<WatchlistDetail> {
    let id = WatchlistId::new(request.id)?;
    state.with_database(|database| {
        WatchlistRepository::new(database).update(
            &id,
            WatchlistWrite {
                name: request.name,
                description: request.description,
                symbols: request.symbols,
            },
        )
    })
}

#[tauri::command]
pub fn delete_watchlist(id: String, state: State<'_, AppState>) -> AppResult<()> {
    let id = WatchlistId::new(id)?;
    state.with_database(|database| WatchlistRepository::new(database).delete(&id))
}
