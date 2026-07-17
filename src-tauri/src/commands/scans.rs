use crate::application::scan_executor::{execute_prepared_scan, SharedDatabaseOps};
use crate::application::scan_service::DatabaseOpsExtended;
use crate::db::Database;
use crate::domain::{
    IndicatorKind, ScanError, ScanPreset, ScanPresetId, ScanRunId, ScanRunStatus, SignalCondition,
    SignalConditionId, SignalSide, Symbol, WatchlistId,
};
use crate::error::{AppError, AppResult};
use crate::provider::retry::RetryConcurrentProvider;
use crate::provider::yahoo::YahooMarketDataProvider;
use crate::repository::scan_preset::ScanConditionDetail;
use crate::state::{AppState, CancellationRegistry};
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
// Helpers
// ---------------------------------------------------------------------------

struct PreparedScan {
    run_id: ScanRunId,
    preset_id: ScanPresetId,
    symbols: Vec<Symbol>,
    conditions: Vec<SignalCondition>,
}

fn status_to_string(status: ScanRunStatus) -> String {
    match status {
        ScanRunStatus::Pending => "pending".to_string(),
        ScanRunStatus::Running => "running".to_string(),
        ScanRunStatus::Completed => "completed".to_string(),
        ScanRunStatus::Cancelled => "cancelled".to_string(),
        ScanRunStatus::Failed => "failed".to_string(),
    }
}

fn indicator_key(indicator: IndicatorKind) -> &'static str {
    match indicator {
        IndicatorKind::Bollinger => "bollinger",
        IndicatorKind::Rsi => "rsi",
        IndicatorKind::Mfi => "mfi",
    }
}

fn side_key(side: SignalSide) -> &'static str {
    match side {
        SignalSide::Lower => "lower",
        SignalSide::Upper => "upper",
    }
}

fn to_signal_condition(
    preset_id: &ScanPresetId,
    condition: &ScanConditionDetail,
    sort_order: usize,
) -> AppResult<SignalCondition> {
    let parameters = match condition.indicator {
        IndicatorKind::Bollinger => serde_json::json!({
            "stdDevMultiplier": condition.std_dev_multiplier.ok_or_else(|| {
                AppError::database(
                    "Bollinger condition is missing std-dev multiplier",
                    format!("preset {} condition {}", preset_id.0, sort_order),
                )
            })?,
        }),
        _ => serde_json::json!({}),
    };

    Ok(SignalCondition {
        id: SignalConditionId::new(format!(
            "{}:{}:{}",
            preset_id.0,
            indicator_key(condition.indicator),
            side_key(condition.side)
        ))?,
        indicator: condition.indicator,
        side: condition.side,
        period: condition.period,
        threshold: condition.threshold,
        parameters,
        trigger_mode: condition.trigger_mode,
        enabled: condition.enabled,
        sort_order: sort_order as i64,
    })
}

fn prepare_scan(
    database: &mut Database,
    watchlist_id: &WatchlistId,
    preset_id: &ScanPresetId,
) -> AppResult<PreparedScan> {
    let watchlist = {
        let repository = crate::repository::watchlist::WatchlistRepository::new(database);
        repository.get(watchlist_id)?
    };
    if watchlist.symbols.is_empty() {
        return Err(AppError::validation("watchlist has no symbols"));
    }

    let preset_detail = {
        let repository = crate::repository::scan_preset::ScanPresetRepository::new(database);
        repository.get(preset_id)?
    };
    let conditions = preset_detail
        .conditions
        .iter()
        .enumerate()
        .map(|(index, condition)| to_signal_condition(preset_id, condition, index))
        .collect::<AppResult<Vec<_>>>()?;
    if !conditions.iter().any(|condition| condition.enabled) {
        return Err(AppError::validation(
            "scan preset must have at least one enabled condition",
        ));
    }

    let preset_snapshot = serde_json::to_string(&ScanPreset {
        id: preset_detail.id.clone(),
        name: preset_detail.name,
        conditions: conditions.clone(),
    })
    .map_err(|error| {
        AppError::internal(
            "failed to serialize scan preset snapshot",
            error.to_string(),
        )
    })?;
    let symbols_snapshot = serde_json::to_string(&watchlist.symbols).map_err(|error| {
        AppError::internal("failed to serialize symbol snapshot", error.to_string())
    })?;

    let input = crate::repository::scan_run::ScanRunCreate {
        watchlist_id: watchlist_id.clone(),
        preset_id: preset_id.clone(),
        total_symbols: watchlist.symbols.len() as u32,
        preset_snapshot_json: preset_snapshot,
        symbols_snapshot_json: symbols_snapshot,
        retry_of_run_id: None,
    };
    let mut repository = crate::repository::scan_run::ScanRunRepository::new(database);
    let summary = repository.create_pending(&input)?;

    Ok(PreparedScan {
        run_id: summary.id,
        preset_id: preset_id.clone(),
        symbols: watchlist.symbols,
        conditions,
    })
}

// ---------------------------------------------------------------------------
// Cancellation guard — removes token when the background task exits
// ---------------------------------------------------------------------------

struct CancelGuard {
    run_id: ScanRunId,
    registry: Arc<CancellationRegistry>,
}

impl Drop for CancelGuard {
    fn drop(&mut self) {
        let run_id = self.run_id.clone();
        let registry = Arc::clone(&self.registry);
        tauri::async_runtime::spawn(async move {
            registry.remove(&run_id).await;
        });
    }
}

// ---------------------------------------------------------------------------
// Tauri Commands
// ---------------------------------------------------------------------------

/// Start a scan run in the background. Returns the exact run ID used by execution.
#[tauri::command]
pub async fn start_scan(
    state: tauri::State<'_, AppState>,
    request: StartScanRequest,
) -> AppResult<String> {
    let watchlist_id = WatchlistId::new(&request.watchlist_id)?;
    let preset_id = ScanPresetId::new(&request.preset_id)?;

    // Prepare immutable symbols/conditions snapshots and create one pending row.
    let prepared =
        state.with_database(|database| prepare_scan(database, &watchlist_id, &preset_id))?;
    let run_id = prepared.run_id.clone();

    // Register cancellation before exposing the run as running.
    state.cancellation_registry().register(&run_id).await;
    let cancellation = state
        .cancellation_registry()
        .get(&run_id)
        .await
        .ok_or_else(|| {
            AppError::internal("failed to register scan cancellation", run_id.0.clone())
        })?;

    if let Err(error) = state.with_database(|database| {
        let mut repository = crate::repository::scan_run::ScanRunRepository::new(database);
        repository.start_running(&run_id)
    }) {
        state.cancellation_registry().remove(&run_id).await;
        return Err(error);
    }

    let response_run_id = run_id.0.clone();
    let app_state_for_task = AppState::clone(state.inner());
    let cancellation_registry = Arc::clone(state.cancellation_registry());

    tauri::async_runtime::spawn(async move {
        let task_run_id = prepared.run_id.clone();
        let _guard = CancelGuard {
            run_id: task_run_id.clone(),
            registry: Arc::clone(&cancellation_registry),
        };
        let provider = RetryConcurrentProvider::new(YahooMarketDataProvider::new());
        let service = crate::application::ScanService::new(
            provider,
            Arc::new(Mutex::new(cancellation)),
            cancellation_registry,
        );
        let mut database = SharedDatabaseOps::new(app_state_for_task);

        let result = execute_prepared_scan(
            &service,
            task_run_id.clone(),
            prepared.preset_id,
            prepared.symbols,
            prepared.conditions,
            &mut database,
        )
        .await;

        if let Err(error) = result {
            let run_error = ScanError {
                run_id: task_run_id.clone(),
                symbol: None,
                code: format!("{:?}", error.code),
                message: error.message,
                detail: error.detail,
                retryable: error.retryable,
                attempt: 1,
            };
            if let Err(save_error) = database.save_scan_error(&run_error) {
                eprintln!(
                    "Failed to save scan error for run {}: {}",
                    task_run_id.0, save_error
                );
            }
            if let Err(mark_error) = database.mark_scan_failed(&task_run_id) {
                eprintln!(
                    "Failed to mark scan run {} as failed: {}",
                    task_run_id.0, mark_error
                );
            }
        }
    });

    Ok(response_run_id)
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
    let scan_run_id = ScanRunId::new(&run_id)?;
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
    let scan_run_id = ScanRunId::new(&run_id)?;
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
    let scan_run_id = ScanRunId::new(&run_id)?;
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
    let scan_run_id = ScanRunId::new(&run_id)?;

    let cancelled = state.cancellation_registry().cancel(&scan_run_id).await;

    if !cancelled {
        state.with_database(|db| {
            let mut repo = crate::repository::scan_run::ScanRunRepository::new(db);
            repo.mark_cancelled(&scan_run_id).map_err(|e| {
                AppError::new(
                    crate::error::AppErrorCode::Validation,
                    format!("cannot cancel scan run: {e}"),
                )
            })
        })?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TriggerMode;

    fn bollinger_detail(side: SignalSide, multiplier: f64) -> ScanConditionDetail {
        ScanConditionDetail {
            indicator: IndicatorKind::Bollinger,
            side,
            period: 20,
            threshold: None,
            std_dev_multiplier: Some(multiplier),
            trigger_mode: TriggerMode::Current,
            enabled: true,
        }
    }

    #[test]
    fn condition_snapshot_preserves_bollinger_multiplier() {
        let preset_id = ScanPresetId::new("preset-lifecycle").expect("valid preset id");
        let condition =
            to_signal_condition(&preset_id, &bollinger_detail(SignalSide::Lower, 1.5), 0)
                .expect("condition must convert");

        assert_eq!(condition.parameters["stdDevMultiplier"], 1.5);
        assert_eq!(condition.sort_order, 0);
    }

    #[test]
    fn condition_snapshot_ids_are_unique_per_side() {
        let preset_id = ScanPresetId::new("preset-lifecycle").expect("valid preset id");
        let lower = to_signal_condition(&preset_id, &bollinger_detail(SignalSide::Lower, 1.0), 0)
            .expect("lower condition must convert");
        let upper = to_signal_condition(&preset_id, &bollinger_detail(SignalSide::Upper, 1.0), 1)
            .expect("upper condition must convert");

        assert_ne!(lower.id, upper.id);
        assert!(lower.id.0.ends_with(":bollinger:lower"));
        assert!(upper.id.0.ends_with(":bollinger:upper"));
    }
}
