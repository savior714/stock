use crate::application::scan_service::{
    DatabaseOps, DatabaseOpsExtended, ScanRunCreateInput, ScanService,
};
use crate::domain::{
    DailyBar, ScanError, ScanPreset, ScanPresetId, ScanResult, ScanRunId, SignalCondition, Symbol,
    WatchlistId,
};
use crate::error::{AppErrorCode, AppResult};
use crate::provider::MarketDataProvider;
use crate::state::AppState;

/// Acquires the application database mutex only for each repository operation.
/// Network and indicator work therefore never hold the SQLite lock.
pub struct SharedDatabaseOps {
    state: AppState,
}

impl SharedDatabaseOps {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }
}

impl DatabaseOps for SharedDatabaseOps {
    fn date_range(&mut self, symbol: &Symbol) -> AppResult<Option<(String, String)>> {
        self.state
            .with_database(|database| DatabaseOps::date_range(database, symbol))
    }

    fn upsert_bars(&mut self, bars: &[DailyBar]) -> AppResult<()> {
        self.state
            .with_database(|database| DatabaseOps::upsert_bars(database, bars))
    }

    fn load_bars(&mut self, symbol: &Symbol, start: &str, end: &str) -> AppResult<Vec<DailyBar>> {
        self.state
            .with_database(|database| DatabaseOps::load_bars(database, symbol, start, end))
    }
}

impl DatabaseOpsExtended for SharedDatabaseOps {
    fn load_watchlist(&mut self, id: &WatchlistId) -> AppResult<(Vec<Symbol>, String)> {
        self.state
            .with_database(|database| DatabaseOpsExtended::load_watchlist(database, id))
    }

    fn load_preset_conditions(&mut self, id: &ScanPresetId) -> AppResult<Vec<SignalCondition>> {
        self.state
            .with_database(|database| DatabaseOpsExtended::load_preset_conditions(database, id))
    }

    fn create_scan_run(&mut self, input: &ScanRunCreateInput) -> AppResult<ScanRunId> {
        self.state
            .with_database(|database| DatabaseOpsExtended::create_scan_run(database, input))
    }

    fn start_scan_run(&mut self, id: &ScanRunId) -> AppResult<()> {
        self.state
            .with_database(|database| DatabaseOpsExtended::start_scan_run(database, id))
    }

    fn save_scan_result(&mut self, result: &ScanResult) -> AppResult<()> {
        self.state
            .with_database(|database| DatabaseOpsExtended::save_scan_result(database, result))
    }

    fn save_scan_error(&mut self, error: &ScanError) -> AppResult<()> {
        self.state
            .with_database(|database| DatabaseOpsExtended::save_scan_error(database, error))
    }

    fn update_scan_progress(
        &mut self,
        id: &ScanRunId,
        succeeded: u32,
        failed: u32,
    ) -> AppResult<()> {
        self.state.with_database(|database| {
            DatabaseOpsExtended::update_scan_progress(database, id, succeeded, failed)
        })
    }

    fn mark_scan_completed(&mut self, id: &ScanRunId, base_date: Option<&str>) -> AppResult<()> {
        self.state.with_database(|database| {
            DatabaseOpsExtended::mark_scan_completed(database, id, base_date)
        })
    }

    fn mark_scan_cancelled(&mut self, id: &ScanRunId) -> AppResult<()> {
        self.state
            .with_database(|database| DatabaseOpsExtended::mark_scan_cancelled(database, id))
    }

    fn mark_scan_failed(&mut self, id: &ScanRunId) -> AppResult<()> {
        self.state
            .with_database(|database| DatabaseOpsExtended::mark_scan_failed(database, id))
    }

    fn update_stale_flags(&mut self, id: &ScanRunId, base_date: &str) -> AppResult<()> {
        self.state.with_database(|database| {
            DatabaseOpsExtended::update_stale_flags(database, id, base_date)
        })
    }

    fn is_cancelled(&mut self) -> bool {
        false
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedScanResult {
    pub run_id: ScanRunId,
    pub total_symbols: u32,
    pub succeeded_symbols: u32,
    pub failed_symbols: u32,
    pub base_trade_date: Option<String>,
}

/// Executes an already-created run.
///
/// Results, errors, progress, and terminal status are always written against
/// the supplied `run_id`; this function never creates or starts another run.
pub async fn execute_prepared_scan<P>(
    service: &ScanService<P>,
    run_id: ScanRunId,
    preset_id: ScanPresetId,
    symbols: Vec<Symbol>,
    conditions: Vec<SignalCondition>,
    database: &mut dyn DatabaseOpsExtended,
) -> AppResult<PreparedScanResult>
where
    P: MarketDataProvider + 'static,
{
    let total_symbols = symbols.len() as u32;
    let preset = ScanPreset {
        id: preset_id,
        name: String::new(),
        conditions,
    };
    let mut succeeded_symbols = 0_u32;
    let mut failed_symbols = 0_u32;
    let mut base_trade_date: Option<String> = None;
    let mut cancelled = false;

    for symbol in symbols {
        match service
            .process_single_symbol(symbol.clone(), &preset, &run_id, database)
            .await
        {
            Ok(result) => {
                database.save_scan_result(&result)?;
                succeeded_symbols += 1;

                match &base_trade_date {
                    Some(current) if current >= &result.trade_date => {}
                    _ => base_trade_date = Some(result.trade_date),
                }
            }
            Err(error) if error.code == AppErrorCode::Cancelled => {
                cancelled = true;
                break;
            }
            Err(error) => {
                database.save_scan_error(&ScanError {
                    run_id: run_id.clone(),
                    symbol: Some(symbol.as_str().to_string()),
                    code: format!("{:?}", error.code),
                    message: error.message,
                    detail: error.detail,
                    retryable: error.retryable,
                    attempt: 1,
                })?;
                failed_symbols += 1;
            }
        }

        database.update_scan_progress(&run_id, succeeded_symbols, failed_symbols)?;
    }

    if let Some(date) = base_trade_date.as_deref() {
        database.update_stale_flags(&run_id, date)?;
    }

    if cancelled {
        database.mark_scan_cancelled(&run_id)?;
    } else if failed_symbols == total_symbols {
        database.mark_scan_failed(&run_id)?;
    } else {
        database.mark_scan_completed(&run_id, base_trade_date.as_deref())?;
    }

    database.update_scan_progress(&run_id, succeeded_symbols, failed_symbols)?;

    Ok(PreparedScanResult {
        run_id,
        total_symbols,
        succeeded_symbols,
        failed_symbols,
        base_trade_date,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{IndicatorKind, PriceBasis, SignalConditionId, SignalSide, TriggerMode};
    use crate::error::AppError;
    use crate::state::CancellationRegistry;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    #[derive(Clone)]
    struct FakeProvider {
        bars: Vec<DailyBar>,
    }

    #[async_trait::async_trait]
    impl MarketDataProvider for FakeProvider {
        async fn fetch_daily_bars(
            &self,
            _symbol: &Symbol,
            _range: &crate::provider::DateRange,
        ) -> AppResult<Vec<DailyBar>> {
            Ok(self.bars.clone())
        }
    }

    struct FakeDatabaseOps {
        bars: Vec<DailyBar>,
        create_calls: u32,
        start_calls: u32,
        saved_results: Vec<ScanResult>,
        saved_errors: Vec<ScanError>,
        progress: Vec<(ScanRunId, u32, u32)>,
        completed_run: Option<ScanRunId>,
        cancelled_run: Option<ScanRunId>,
        failed_run: Option<ScanRunId>,
    }

    impl FakeDatabaseOps {
        fn new(bars: Vec<DailyBar>) -> Self {
            Self {
                bars,
                create_calls: 0,
                start_calls: 0,
                saved_results: Vec::new(),
                saved_errors: Vec::new(),
                progress: Vec::new(),
                completed_run: None,
                cancelled_run: None,
                failed_run: None,
            }
        }
    }

    impl DatabaseOps for FakeDatabaseOps {
        fn date_range(&mut self, _symbol: &Symbol) -> AppResult<Option<(String, String)>> {
            Ok(None)
        }

        fn upsert_bars(&mut self, _bars: &[DailyBar]) -> AppResult<()> {
            Ok(())
        }

        fn load_bars(
            &mut self,
            _symbol: &Symbol,
            _start: &str,
            _end: &str,
        ) -> AppResult<Vec<DailyBar>> {
            Ok(self.bars.clone())
        }
    }

    impl DatabaseOpsExtended for FakeDatabaseOps {
        fn load_watchlist(&mut self, _id: &WatchlistId) -> AppResult<(Vec<Symbol>, String)> {
            Err(AppError::internal("unexpected load_watchlist", "not used"))
        }

        fn load_preset_conditions(
            &mut self,
            _id: &ScanPresetId,
        ) -> AppResult<Vec<SignalCondition>> {
            Err(AppError::internal(
                "unexpected load_preset_conditions",
                "not used",
            ))
        }

        fn create_scan_run(&mut self, _input: &ScanRunCreateInput) -> AppResult<ScanRunId> {
            self.create_calls += 1;
            Err(AppError::internal("unexpected create_scan_run", "not used"))
        }

        fn start_scan_run(&mut self, _id: &ScanRunId) -> AppResult<()> {
            self.start_calls += 1;
            Err(AppError::internal("unexpected start_scan_run", "not used"))
        }

        fn save_scan_result(&mut self, result: &ScanResult) -> AppResult<()> {
            self.saved_results.push(result.clone());
            Ok(())
        }

        fn save_scan_error(&mut self, error: &ScanError) -> AppResult<()> {
            self.saved_errors.push(error.clone());
            Ok(())
        }

        fn update_scan_progress(
            &mut self,
            id: &ScanRunId,
            succeeded: u32,
            failed: u32,
        ) -> AppResult<()> {
            self.progress.push((id.clone(), succeeded, failed));
            Ok(())
        }

        fn mark_scan_completed(
            &mut self,
            id: &ScanRunId,
            _base_date: Option<&str>,
        ) -> AppResult<()> {
            self.completed_run = Some(id.clone());
            Ok(())
        }

        fn mark_scan_cancelled(&mut self, id: &ScanRunId) -> AppResult<()> {
            self.cancelled_run = Some(id.clone());
            Ok(())
        }

        fn mark_scan_failed(&mut self, id: &ScanRunId) -> AppResult<()> {
            self.failed_run = Some(id.clone());
            Ok(())
        }

        fn update_stale_flags(&mut self, _id: &ScanRunId, _base_date: &str) -> AppResult<()> {
            Ok(())
        }

        fn is_cancelled(&mut self) -> bool {
            false
        }
    }

    fn bars(symbol: &Symbol) -> Vec<DailyBar> {
        [100.0, 101.0, 102.0, 103.0]
            .into_iter()
            .enumerate()
            .map(|(index, close)| DailyBar {
                symbol: symbol.clone(),
                trade_date: format!("2026-07-{:02}", index + 1),
                price_basis: PriceBasis::Raw,
                open: close,
                high: close + 1.0,
                low: close - 1.0,
                close,
                volume: 1_000,
            })
            .collect()
    }

    fn condition() -> SignalCondition {
        SignalCondition {
            id: SignalConditionId::new("rsi:lower").expect("valid condition id"),
            indicator: IndicatorKind::Rsi,
            side: SignalSide::Lower,
            period: 2,
            threshold: Some(100.0),
            parameters: serde_json::json!({}),
            trigger_mode: TriggerMode::Current,
            enabled: true,
            sort_order: 0,
        }
    }

    async fn service(
        provider: FakeProvider,
        run_id: &ScanRunId,
        cancelled: bool,
    ) -> ScanService<FakeProvider> {
        let registry = Arc::new(CancellationRegistry::new());
        registry.register(run_id).await;
        if cancelled {
            registry.cancel(run_id).await;
        }
        let token = registry.get(run_id).await.expect("registered token");
        ScanService::new(provider, Arc::new(Mutex::new(token)), registry)
    }

    #[tokio::test]
    async fn executes_only_the_supplied_run() {
        let run_id = ScanRunId::new("run-prepared-1").expect("valid run id");
        let preset_id = ScanPresetId::new("preset-1").expect("valid preset id");
        let symbol = Symbol::new("AAPL").expect("valid symbol");
        let provider = FakeProvider {
            bars: bars(&symbol),
        };
        let service = service(provider, &run_id, false).await;
        let mut database = FakeDatabaseOps::new(bars(&symbol));

        let result = execute_prepared_scan(
            &service,
            run_id.clone(),
            preset_id,
            vec![symbol],
            vec![condition()],
            &mut database,
        )
        .await
        .expect("prepared scan must complete");

        assert_eq!(result.run_id, run_id);
        assert_eq!(result.succeeded_symbols, 1);
        assert_eq!(database.create_calls, 0);
        assert_eq!(database.start_calls, 0);
        assert_eq!(database.saved_results.len(), 1);
        assert_eq!(database.saved_results[0].run_id, result.run_id);
        assert_eq!(database.completed_run, Some(result.run_id.clone()));
        assert!(database.cancelled_run.is_none());
        assert!(database.failed_run.is_none());
        assert_eq!(database.progress.last(), Some(&(result.run_id, 1, 0)));
    }

    #[tokio::test]
    async fn cancellation_marks_the_supplied_run_without_creating_another() {
        let run_id = ScanRunId::new("run-prepared-2").expect("valid run id");
        let preset_id = ScanPresetId::new("preset-2").expect("valid preset id");
        let symbol = Symbol::new("MSFT").expect("valid symbol");
        let provider = FakeProvider {
            bars: bars(&symbol),
        };
        let service = service(provider, &run_id, true).await;
        let mut database = FakeDatabaseOps::new(bars(&symbol));

        let result = execute_prepared_scan(
            &service,
            run_id.clone(),
            preset_id,
            vec![symbol],
            vec![condition()],
            &mut database,
        )
        .await
        .expect("cancelled scan must finalize");

        assert_eq!(result.run_id, run_id);
        assert_eq!(result.succeeded_symbols, 0);
        assert_eq!(result.failed_symbols, 0);
        assert_eq!(database.create_calls, 0);
        assert_eq!(database.start_calls, 0);
        assert!(database.saved_results.is_empty());
        assert!(database.saved_errors.is_empty());
        assert_eq!(database.cancelled_run, Some(result.run_id));
        assert!(database.completed_run.is_none());
        assert!(database.failed_run.is_none());
    }
}
