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
use std::collections::HashSet;
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

#[derive(Debug)]
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

    let prepared =
        state.with_database(|database| prepare_scan(database, &watchlist_id, &preset_id))?;

    launch_prepared_scan(state, prepared).await
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

// ---------------------------------------------------------------------------
// Retry — snapshot-based symbol retry
// ---------------------------------------------------------------------------

/// Prepare a retry scan from an existing run's snapshot data.
/// Does not create or start a new run; returns the prepared inputs only.
fn prepare_retry_scan(
    database: &mut Database,
    original_run_id: &ScanRunId,
) -> AppResult<PreparedScan> {
    let repo = crate::repository::scan_run::ScanRunRepository::new(database);
    let original = repo.get(original_run_id)?;

    // Only completed/failed runs can be retried
    if original.status != ScanRunStatus::Completed && original.status != ScanRunStatus::Failed {
        return Err(AppError::validation(format!(
            "scan run {} is in {} state and cannot be retried",
            original_run_id.0,
            match original.status {
                ScanRunStatus::Pending => "pending",
                ScanRunStatus::Running => "running",
                ScanRunStatus::Completed => "completed",
                ScanRunStatus::Cancelled => "cancelled",
                ScanRunStatus::Failed => "failed",
            }
        )));
    }

    // Deserialize preset snapshot
    let preset: ScanPreset = serde_json::from_value(original.preset_snapshot_json.clone())
        .map_err(|error| {
            AppError::internal(
                "failed to deserialize preset snapshot JSON",
                error.to_string(),
            )
        })?;

    // Validate preset snapshot ID matches original run preset ID
    if preset.id != original.preset_id {
        return Err(AppError::internal(
            "scan preset snapshot ID does not match the original run",
            format!(
                "run_id={}, stored_preset_id={}, snapshot_preset_id={}",
                original_run_id.0, original.preset_id.0, preset.id.0,
            ),
        ));
    }

    // Validate preset has at least one enabled condition
    if !preset.conditions.iter().any(|c| c.enabled) {
        return Err(AppError::validation(
            "preset snapshot has no enabled conditions",
        ));
    }

    // Deserialize symbol snapshot
    let symbols: Vec<Symbol> = serde_json::from_value(original.symbols_snapshot_json.clone())
        .map_err(|error| {
            AppError::internal(
                "failed to deserialize symbols snapshot JSON",
                error.to_string(),
            )
        })?;

    if symbols.is_empty() {
        return Err(AppError::validation(
            "symbols snapshot is empty — cannot retry",
        ));
    }

    // Build a set of original symbols for intersection
    let original_symbols: Vec<Symbol> = symbols.clone();
    let original_symbol_names: Vec<&str> = original_symbols.iter().map(|s| s.as_str()).collect();

    // Get retryable symbols (distinct, non-null, retryable=true)
    let error_repo = crate::repository::scan_error::ScanErrorRepository::new(database);
    let retryable_raw = error_repo.get_retryable_symbols(original_run_id)?;

    // Intersect with original symbols, preserving original order
    let retryable_set: HashSet<&str> = retryable_raw.iter().map(|s| s.as_str()).collect();
    let mut retry_symbols: Vec<Symbol> = symbols
        .into_iter()
        .filter(|s| retryable_set.contains(s.as_str()))
        .collect::<HashSet<_>>()
        .into_iter()
        .collect();

    // Re-sort to match original snapshot order
    retry_symbols.sort_by(|a, b| {
        let oa = original_symbol_names.iter().position(|s| *s == a.as_str());
        let ob = original_symbol_names.iter().position(|s| *s == b.as_str());
        oa.cmp(&ob)
    });

    if retry_symbols.is_empty() {
        return Err(AppError::validation(
            "no retryable symbol errors found for this run",
        ));
    }

    // The preset snapshot already contains SignalCondition objects.
    // Convert them directly (they were serialized from the same type).
    let conditions: Vec<SignalCondition> = preset
        .conditions
        .clone()
        .into_iter()
        .enumerate()
        .map(|(index, mut condition)| {
            condition.sort_order = index as i64;
            condition
        })
        .collect();

    // Serialize snapshots for the new run
    let preset_snapshot = serde_json::to_string(&preset).map_err(|error| {
        AppError::internal("failed to serialize preset snapshot", error.to_string())
    })?;
    let symbols_snapshot = serde_json::to_string(&retry_symbols).map_err(|error| {
        AppError::internal(
            "failed to serialize retry symbol snapshot",
            error.to_string(),
        )
    })?;

    let input = crate::repository::scan_run::ScanRunCreate {
        watchlist_id: original.watchlist_id.clone(),
        preset_id: original.preset_id.clone(),
        total_symbols: retry_symbols.len() as u32,
        preset_snapshot_json: preset_snapshot,
        symbols_snapshot_json: symbols_snapshot,
        retry_of_run_id: Some(original_run_id.clone()),
    };

    let mut run_repo = crate::repository::scan_run::ScanRunRepository::new(database);
    let summary = run_repo.create_pending(&input)?;

    Ok(PreparedScan {
        run_id: summary.id,
        preset_id: preset.id,
        symbols: retry_symbols,
        conditions,
    })
}

/// Launch a prepared scan: register cancellation, transition to running, spawn background task.
async fn launch_prepared_scan(
    state: tauri::State<'_, AppState>,
    prepared: PreparedScan,
) -> AppResult<String> {
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

/// Retry failed symbols from a completed/failed scan run.
/// Uses the original run's snapshot data, not live Watchlist/Preset resources.
#[tauri::command]
pub async fn retry_scan(state: tauri::State<'_, AppState>, run_id: String) -> AppResult<String> {
    let scan_run_id = ScanRunId::new(&run_id)?;

    let prepared = state.with_database(|database| prepare_retry_scan(database, &scan_run_id))?;

    launch_prepared_scan(state, prepared).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TriggerMode;
    use crate::error::AppErrorCode;

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

    // ------------------------------------------------------------------
    // Retry helper tests
    // ------------------------------------------------------------------

    fn make_retry_db() -> (Database, ScanRunId) {
        let db = Database::open_in_memory().expect("db must init");
        let conn = db.connection();

        conn.execute(
            "INSERT INTO watchlists (id, name) VALUES ('wl-retry', 'Retry WL')",
            [],
        )
        .expect("watchlist must insert");
        conn.execute(
            "INSERT INTO scan_presets (id, name, trigger_mode) VALUES ('ps-retry', 'Retry PS', 'current')",
            [],
        ).expect("preset must insert");
        conn.execute(
            "INSERT INTO instruments (symbol, provider_symbol, asset_type) VALUES ('AAPL', 'AAPL', 'stock')",
            [],
        ).ok();
        conn.execute(
            "INSERT INTO instruments (symbol, provider_symbol, asset_type) VALUES ('MSFT', 'MSFT', 'stock')",
            [],
        ).ok();
        conn.execute(
            "INSERT INTO instruments (symbol, provider_symbol, asset_type) VALUES ('GOOGL', 'GOOGL', 'stock')",
            [],
        ).ok();
        conn.execute(
            "INSERT INTO instruments (symbol, provider_symbol, asset_type) VALUES ('AMZN', 'AMZN', 'stock')",
            [],
        ).ok();

        let run_id = ScanRunId::new("run-retry-1").expect("valid id");
        let preset_with_condition = serde_json::json!({
            "id": "ps-retry",
            "name": "Retry",
            "conditions": [{
                "id": "rsi:lower",
                "indicator": "rsi",
                "side": "lower",
                "period": 14,
                "threshold": 30.0,
                "parameters": {},
                "triggerMode": "current",
                "enabled": true,
                "sortOrder": 0
            }]
        });
        conn.execute(
            "INSERT INTO scan_runs (id, watchlist_id, preset_id, status, total_symbols, \
             preset_snapshot_json, symbols_snapshot_json) \
             VALUES (?, 'wl-retry', 'ps-retry', 'completed', 4, ?, \
             '[\"AAPL\",\"MSFT\",\"GOOGL\",\"AMZN\"]')",
            [
                &run_id.0,
                &serde_json::to_string(&preset_with_condition).unwrap(),
            ],
        )
        .expect("run must insert");

        (db, run_id)
    }

    #[test]
    fn extracts_retryable_symbols_only() {
        let (mut db, run_id) = make_retry_db();
        let conn = db.connection_mut();
        conn.execute(
            "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-retry-1', 'AAPL', 'NETWORK_RETRY', 'retryable', 1)",
            [],
        )
        .ok();
        conn.execute(
            "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-retry-1', 'MSFT', 'DATA_NOT_FOUND', 'permanent', 0)",
            [],
        )
        .ok();
        conn.execute(
            "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-retry-1', 'GOOGL', 'NETWORK_RETRY', 'retryable', 1)",
            [],
        )
        .ok();

        let prepared = prepare_retry_scan(&mut db, &run_id).expect("must prepare");
        let symbols: Vec<&str> = prepared.symbols.iter().map(|s| s.as_str()).collect();
        assert_eq!(symbols, vec!["AAPL", "GOOGL"]);
    }

    #[test]
    fn excludes_null_symbol_errors() {
        let (mut db, run_id) = make_retry_db();
        let conn = db.connection_mut();
        // Run-level error (null symbol) should not be a retry target
        conn.execute(
            "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-retry-1', NULL, 'GLOBAL_ERR', 'global', 1)",
            [],
        )
        .ok();

        let err = prepare_retry_scan(&mut db, &run_id).unwrap_err();
        assert_eq!(err.code, AppErrorCode::Validation);
    }

    #[test]
    fn returns_validation_when_no_retryable_symbols() {
        let (mut db, run_id) = make_retry_db();
        // Only permanent errors
        let conn = db.connection_mut();
        conn.execute(
            "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-retry-1', 'AAPL', 'DATA_NOT_FOUND', 'permanent', 0)",
            [],
        )
        .ok();

        let err = prepare_retry_scan(&mut db, &run_id).unwrap_err();
        assert_eq!(err.code, AppErrorCode::Validation);
    }

    #[test]
    fn preserves_original_symbol_order() {
        let (mut db, run_id) = make_retry_db();
        let conn = db.connection_mut();
        // Add retryable errors in reverse order
        conn.execute(
            "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-retry-1', 'AMZN', 'NETWORK_RETRY', 'retryable', 1)",
            [],
        )
        .ok();
        conn.execute(
            "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-retry-1', 'GOOGL', 'NETWORK_RETRY', 'retryable', 1)",
            [],
        )
        .ok();

        let prepared = prepare_retry_scan(&mut db, &run_id).expect("must prepare");
        let symbols: Vec<&str> = prepared.symbols.iter().map(|s| s.as_str()).collect();
        // Should preserve original order: GOOGL before AMZN
        assert_eq!(symbols, vec!["GOOGL", "AMZN"]);
    }

    #[test]
    fn deduplicates_retryable_symbols() {
        let (mut db, run_id) = make_retry_db();
        let conn = db.connection_mut();
        // Multiple errors for same symbol
        conn.execute(
            "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-retry-1', 'AAPL', 'ERR1', 'retryable', 1)",
            [],
        )
        .ok();
        conn.execute(
            "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-retry-1', 'AAPL', 'ERR2', 'retryable', 1)",
            [],
        )
        .ok();

        let prepared = prepare_retry_scan(&mut db, &run_id).expect("must prepare");
        let symbols: Vec<&str> = prepared.symbols.iter().map(|s| s.as_str()).collect();
        assert_eq!(symbols, vec!["AAPL"]);
    }

    #[test]
    fn excludes_symbols_not_in_original_snapshot() {
        let (mut db, run_id) = make_retry_db();
        let conn = db.connection_mut();
        // Error for a symbol not in the original snapshot
        conn.execute(
            "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-retry-1', 'TSLA', 'NETWORK_RETRY', 'retryable', 1)",
            [],
        )
        .ok();
        // Also add a retryable error for an original symbol
        conn.execute(
            "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-retry-1', 'AAPL', 'NETWORK_RETRY', 'retryable', 1)",
            [],
        )
        .ok();

        let prepared = prepare_retry_scan(&mut db, &run_id).expect("must prepare");
        let symbols: Vec<&str> = prepared.symbols.iter().map(|s| s.as_str()).collect();
        // TSLA should be excluded (not in original snapshot)
        assert!(!symbols.contains(&"TSLA"));
        // AAPL should be included (in original snapshot)
        assert!(symbols.contains(&"AAPL"));
    }

    #[test]
    fn rejects_non_terminal_status() {
        let (mut db, run_id) = make_retry_db();
        let conn = db.connection_mut();
        conn.execute(
            "UPDATE scan_runs SET status = 'pending' WHERE id = 'run-retry-1'",
            [],
        )
        .expect("must update");

        let err = prepare_retry_scan(&mut db, &run_id).unwrap_err();
        assert_eq!(err.code, AppErrorCode::Validation);
    }

    #[test]
    fn saves_retry_of_run_id() {
        let (mut db, run_id) = make_retry_db();
        db.connection()
            .execute(
                "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-retry-1', 'AAPL', 'NETWORK_RETRY', 'retryable', 1)",
                [],
            )
            .ok();

        let prepared = prepare_retry_scan(&mut db, &run_id).expect("must prepare");
        let new_run_id = prepared.run_id.0.clone();

        // Verify the new run has retry_of_run_id set
        let child: Option<String> = db
            .connection()
            .query_row(
                "SELECT retry_of_run_id FROM scan_runs WHERE id = ?",
                [&new_run_id],
                |row| row.get(0),
            )
            .ok();
        assert_eq!(child, Some(run_id.0));
    }

    #[test]
    fn total_symbols_matches_retry_subset() {
        let (mut db, run_id) = make_retry_db();
        let conn = db.connection();
        conn.execute(
            "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-retry-1', 'AAPL', 'NETWORK_RETRY', 'retryable', 1)",
            [],
        )
        .ok();
        conn.execute(
            "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-retry-1', 'GOOGL', 'NETWORK_RETRY', 'retryable', 1)",
            [],
        )
        .ok();

        let prepared = prepare_retry_scan(&mut db, &run_id).expect("must prepare");
        assert_eq!(prepared.symbols.len(), 2);
    }

    // ------------------------------------------------------------------
    // Preset snapshot ID validation
    // ------------------------------------------------------------------

    #[test]
    fn preset_snapshot_id_mismatch_returns_internal_error() {
        let (mut db, run_id) = make_retry_db();
        let conn = db.connection();

        // Insert a run with a mismatched preset snapshot ID
        let mismatched_preset = serde_json::json!({
            "id": "ps-different",
            "name": "Different",
            "conditions": [{
                "id": "rsi:lower",
                "indicator": "rsi",
                "side": "lower",
                "period": 14,
                "threshold": 30.0,
                "parameters": {},
                "triggerMode": "current",
                "enabled": true,
                "sortOrder": 0
            }]
        });
        conn.execute(
            "UPDATE scan_runs SET preset_snapshot_json = ? WHERE id = 'run-retry-1'",
            [&serde_json::to_string(&mismatched_preset).unwrap()],
        )
        .expect("must update");

        let err = prepare_retry_scan(&mut db, &run_id).unwrap_err();
        assert_eq!(err.code, AppErrorCode::Internal);
    }

    #[test]
    fn preset_snapshot_id_mismatch_prevents_run_creation() {
        let (mut db, run_id) = make_retry_db();

        {
            let conn = db.connection();
            // Insert a run with a mismatched preset snapshot ID
            let mismatched_preset = serde_json::json!({
                "id": "ps-different",
                "name": "Different",
                "conditions": [{
                    "id": "rsi:lower",
                    "indicator": "rsi",
                    "side": "lower",
                    "period": 14,
                    "threshold": 30.0,
                    "parameters": {},
                    "triggerMode": "current",
                    "enabled": true,
                    "sortOrder": 0
                }]
            });
            conn.execute(
                "UPDATE scan_runs SET preset_snapshot_json = ? WHERE id = 'run-retry-1'",
                [&serde_json::to_string(&mismatched_preset).unwrap()],
            )
            .expect("must update");
        }

        // Should fail — no new run should be created
        let result = prepare_retry_scan(&mut db, &run_id);
        assert!(result.is_err());

        // Verify no new run was created
        let count: i64 = db
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM scan_runs WHERE retry_of_run_id = 'run-retry-1'",
                [],
                |row| row.get(0),
            )
            .expect("must count");
        assert_eq!(count, 0);
    }

    #[test]
    fn matching_preset_snapshot_id_succeeds() {
        let (mut db, run_id) = make_retry_db();
        let conn = db.connection();
        conn.execute(
            "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-retry-1', 'AAPL', 'NETWORK_RETRY', 'retryable', 1)",
            [],
        )
        .ok();

        // Should succeed with matching preset ID
        let prepared = prepare_retry_scan(&mut db, &run_id).expect("must prepare");
        assert_eq!(prepared.preset_id.0, "ps-retry");
    }
}
