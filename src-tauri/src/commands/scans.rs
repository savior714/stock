use crate::domain::{ScanRunStatus, WatchlistId};
use crate::error::{AppError, AppResult};
use crate::provider::retry::RetryConcurrentProvider;
use crate::provider::yahoo::YahooMarketDataProvider;
use crate::state::{AppState, CancellationRegistry, CancellationToken};
use serde::Deserialize;
use std::sync::Arc;
use tokio::sync::Mutex;

// ---------------------------------------------------------------------------
// Request DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct StartScanRequest {
    pub watchlist_id: String,
    pub preset_id: String,
}

// ---------------------------------------------------------------------------
// Response DTOs
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRunSummaryDto {
    pub id: String,
    pub watchlist_id: String,
    pub preset_id: String,
    pub status: String,
    pub total_symbols: u32,
    pub succeeded_symbols: u32,
    pub failed_symbols: u32,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRunDetailDto {
    pub id: String,
    pub watchlist_id: String,
    pub preset_id: String,
    pub status: String,
    pub base_trade_date: Option<String>,
    pub total_symbols: u32,
    pub succeeded_symbols: u32,
    pub failed_symbols: u32,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub preset_snapshot_json: serde_json::Value,
    pub symbols_snapshot_json: serde_json::Value,
    pub retry_of_run_id: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResultDto {
    pub symbol: String,
    pub trade_date: String,
    pub current_price: f64,
    pub rsi: Option<f64>,
    pub mfi: Option<f64>,
    pub bollinger_lower: Option<f64>,
    pub bollinger_middle: Option<f64>,
    pub bollinger_upper: Option<f64>,
    pub all_conditions_matched: bool,
    pub any_condition_matched: bool,
    pub data_stale: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanErrorDto {
    pub symbol: Option<String>,
    pub code: String,
    pub message: String,
    pub detail: Option<String>,
    pub retryable: bool,
    pub attempt: u32,
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn status_to_string(status: ScanRunStatus) -> String {
    match status {
        ScanRunStatus::Pending => "pending".to_string(),
        ScanRunStatus::Running => "running".to_string(),
        ScanRunStatus::Completed => "completed".to_string(),
        ScanRunStatus::Cancelled => "cancelled".to_string(),
        ScanRunStatus::Failed => "failed".to_string(),
    }
}

// ---------------------------------------------------------------------------
// Cancellation guard — removes token on drop
// ---------------------------------------------------------------------------

struct CancelGuard {
    run_id: crate::domain::ScanRunId,
    registry: Arc<CancellationRegistry>,
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        let rt = tokio::runtime::Handle::current();
        rt.block_on(async {
            let _ = self.registry.remove(&self.run_id).await;
        });
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

/// Start a scan run in the background. Returns run ID immediately.
#[tauri::command]
pub async fn start_scan(
    state: tauri::State<'_, AppState>,
    request: StartScanRequest,
) -> AppResult<String> {
    let watchlist_id = WatchlistId::new(&request.watchlist_id)?;
    let preset_id = crate::domain::ScanPresetId::new(&request.preset_id)?;

    // 1. Create run record (brief DB lock)
    let run_id = state.with_database(|db| {
        // Load watchlist for symbols
        let wl = {
            let wl_repo = crate::repository::watchlist::WatchlistRepository::new(db);
            wl_repo.get(&watchlist_id)?
        };

        // Load preset for conditions
        let ps = {
            let ps_repo = crate::repository::scan_preset::ScanPresetRepository::new(db);
            ps_repo.get(&preset_id)?
        };

        let mut repo = crate::repository::scan_run::ScanRunRepository::new(db);

        let total = wl.symbols.len() as u32;

        // Build preset snapshot
        let preset_snapshot = serde_json::json!({
            "id": ps.id.0,
            "name": ps.name,
            "conditions": ps.conditions.iter().map(|c| {
                serde_json::json!({
                    // Condition IDs are synthetic; use indicator+side as key
                    "indicator": match c.indicator {
                        crate::domain::IndicatorKind::Rsi => "rsi",
                        crate::domain::IndicatorKind::Mfi => "mfi",
                        crate::domain::IndicatorKind::Bollinger => "bollinger",
                    },
                    "side": match c.side {
                        crate::domain::SignalSide::Lower => "lower",
                        crate::domain::SignalSide::Upper => "upper",
                    },
                    "period": c.period,
                    "threshold": c.threshold,
                    "trigger_mode": match c.trigger_mode {
                        crate::domain::TriggerMode::Current => "current",
                        crate::domain::TriggerMode::Cross => "cross",
                    },
                    "enabled": c.enabled,
                })
            }).collect::<Vec<_>>(),
        });

        let symbols_snapshot: Vec<&str> = wl.symbols.iter().map(|s| s.as_str()).collect();

        let run_input = crate::repository::scan_run::ScanRunCreate {
            watchlist_id: watchlist_id.clone(),
            preset_id: preset_id.clone(),
            total_symbols: total,
            preset_snapshot_json: serde_json::to_string(&preset_snapshot).unwrap_or_default(),
            symbols_snapshot_json: serde_json::to_string(&symbols_snapshot).unwrap_or_default(),
            retry_of_run_id: None,
        };

        let summary = repo.create_pending(&run_input)?;
        repo.start_running(&summary.id)?;

        Ok::<_, AppError>(summary.id)
    })?;

    // 2. Register cancellation token
    state.cancellation_registry().register(&run_id).await;

    // 3. Spawn background task
    let cancel_registry = state.cancellation_registry().clone();
    let app_state_for_task = Arc::new(AppState::clone(state.inner()));
    let run_id_for_task = run_id.clone();
    let preset_id_for_task = preset_id.clone();
    let watchlist_id_for_task = watchlist_id.clone();

    tauri::async_runtime::spawn(async move {
        let cancel_registry_for_service = Arc::clone(&cancel_registry);
        let _guard = CancelGuard {
            run_id: run_id_for_task.clone(),
            registry: cancel_registry,
        };

        let result = app_state_for_task.with_database(|db| {
            let mut ops = crate::application::scan_service::ScanDbOpsWrapper::new(db);
            let provider = RetryConcurrentProvider::new(YahooMarketDataProvider::new());
            let cancellation = Arc::new(Mutex::new(CancellationToken::new()));
            let service = crate::application::ScanService::new(
                provider,
                cancellation,
                Arc::clone(&cancel_registry_for_service),
            );
            // We need to await run_scan_concurrent, but we're in a sync closure
            // So we spawn it and wait for the future
            let watchlist_id = watchlist_id_for_task.clone();
            let preset_id = preset_id_for_task.clone();
            tauri::async_runtime::block_on(service.run_scan_concurrent(
                &watchlist_id,
                &preset_id,
                &mut ops,
            ))
        });

        match result {
            Ok(_) => {}
            Err(e) => {
                eprintln!("Scan failed for run {}: {}", run_id_for_task.0, e);
            }
        }
    });

    Ok(run_id.0)
}

/// List recent scan runs.
#[tauri::command]
pub async fn list_scan_runs(
    state: tauri::State<'_, AppState>,
    limit: Option<u32>,
) -> AppResult<Vec<ScanRunSummaryDto>> {
    let limit = limit.unwrap_or(20);
    let runs = state.with_database(|db| {
        let repo = crate::repository::scan_run::ScanRunRepository::new(db);
        let summaries = repo.list_recent(limit)?;
        Ok(summaries
            .into_iter()
            .map(|s| ScanRunSummaryDto {
                id: s.id.0,
                watchlist_id: s.watchlist_id.0,
                preset_id: s.preset_id.0,
                status: status_to_string(s.status),
                total_symbols: s.total_symbols,
                succeeded_symbols: s.succeeded_symbols,
                failed_symbols: s.failed_symbols,
                started_at: s.started_at,
                finished_at: s.finished_at,
            })
            .collect::<Vec<_>>())
    })?;
    Ok(runs)
}

/// Get scan run detail.
#[tauri::command]
pub async fn get_scan_run(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> AppResult<ScanRunDetailDto> {
    let scan_run_id = crate::domain::ScanRunId::new(&run_id)?;
    let detail = state.with_database(|db| {
        let repo = crate::repository::scan_run::ScanRunRepository::new(db);
        let d = repo.get(&scan_run_id)?;
        Ok(ScanRunDetailDto {
            id: d.id.0,
            watchlist_id: d.watchlist_id.0,
            preset_id: d.preset_id.0,
            status: status_to_string(d.status),
            base_trade_date: d.base_trade_date,
            total_symbols: d.total_symbols,
            succeeded_symbols: d.succeeded_symbols,
            failed_symbols: d.failed_symbols,
            started_at: d.started_at,
            finished_at: d.finished_at,
            preset_snapshot_json: d.preset_snapshot_json,
            symbols_snapshot_json: d.symbols_snapshot_json,
            retry_of_run_id: d.retry_of_run_id.map(|r| r.0),
        })
    })?;
    Ok(detail)
}

/// Get scan results for a run.
#[tauri::command]
pub async fn get_scan_results(
    state: tauri::State<'_, AppState>,
    run_id: String,
    filter: Option<String>,
) -> AppResult<Vec<ScanResultDto>> {
    let scan_run_id = crate::domain::ScanRunId::new(&run_id)?;
    let match_filter = match filter.as_deref() {
        Some("and") => crate::repository::scan_result::ResultMatchFilter::And,
        Some("or") => crate::repository::scan_result::ResultMatchFilter::Or,
        _ => crate::repository::scan_result::ResultMatchFilter::None,
    };
    let results = state.with_database(|db| {
        let repo = crate::repository::scan_result::ScanResultRepository::new(db);
        let items = repo.get_by_run(&scan_run_id, match_filter)?;
        Ok(items
            .into_iter()
            .map(|r| ScanResultDto {
                symbol: r.symbol.as_str().to_string(),
                trade_date: r.trade_date,
                current_price: r.current_price,
                rsi: r.indicators.rsi,
                mfi: r.indicators.mfi,
                bollinger_lower: r.indicators.bollinger_lower,
                bollinger_middle: r.indicators.bollinger_middle,
                bollinger_upper: r.indicators.bollinger_upper,
                all_conditions_matched: r.all_conditions_matched,
                any_condition_matched: r.any_condition_matched,
                data_stale: r.data_stale,
            })
            .collect::<Vec<_>>())
    })?;
    Ok(results)
}

/// Get scan errors for a run.
#[tauri::command]
pub async fn get_scan_errors(
    state: tauri::State<'_, AppState>,
    run_id: String,
) -> AppResult<Vec<ScanErrorDto>> {
    let scan_run_id = crate::domain::ScanRunId::new(&run_id)?;
    let errors = state.with_database(|db| {
        let repo = crate::repository::scan_error::ScanErrorRepository::new(db);
        let items = repo.get_by_run(&scan_run_id)?;
        Ok(items
            .into_iter()
            .map(|e| ScanErrorDto {
                symbol: e.symbol,
                code: e.code,
                message: e.message,
                detail: e.detail,
                retryable: e.retryable,
                attempt: e.attempt,
            })
            .collect::<Vec<_>>())
    })?;
    Ok(errors)
}

/// Cancel a running scan.
#[tauri::command]
pub async fn cancel_scan(state: tauri::State<'_, AppState>, run_id: String) -> AppResult<()> {
    let scan_run_id = crate::domain::ScanRunId::new(&run_id)?;

    // First, try to cancel via cancellation registry
    let cancelled = state.cancellation_registry().cancel(&scan_run_id).await;

    if !cancelled {
        // Scan may have already finished; try to mark as cancelled via DB
        state.with_database(|db| {
            let mut repo = crate::repository::scan_run::ScanRunRepository::new(db);
            repo.mark_cancelled(&scan_run_id).map_err(|e| {
                AppError::new(
                    crate::error::AppErrorCode::Validation,
                    format!("cannot cancel scan run: {}", e),
                )
            })
        })?;
    }

    Ok(())
}
