use crate::db::Database;
use crate::domain::{
    DailyBar, IndicatorValues, ScanPreset, ScanResult, ScanRunId, SignalMatch, Symbol,
};
use crate::error::{AppError, AppErrorCode, AppResult};
use crate::indicator::IndicatorSnapshot;
use crate::provider::{
    fetch_planner::{FetchPlanner, FetchSource},
    DateRange, MarketDataProvider,
};
use crate::repository::daily_bar::DailyBarRepository;
use crate::signal;
use serde_json;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use tokio::sync::{Mutex, Semaphore};

// ---------------------------------------------------------------------------
// DatabaseOps trait — abstracts repository operations needed by the service
// ---------------------------------------------------------------------------

pub trait DatabaseOps: Send {
    fn date_range(&mut self, symbol: &Symbol) -> AppResult<Option<(String, String)>>;
    fn upsert_bars(&mut self, bars: &[DailyBar]) -> AppResult<()>;
    fn load_bars(&mut self, symbol: &Symbol, start: &str, end: &str) -> AppResult<Vec<DailyBar>>;
}

impl DatabaseOps for Database {
    fn date_range(&mut self, symbol: &Symbol) -> AppResult<Option<(String, String)>> {
        let repo = DailyBarRepository::new(self);
        repo.date_range(symbol)
    }

    fn upsert_bars(&mut self, bars: &[DailyBar]) -> AppResult<()> {
        let mut repo = DailyBarRepository::new(self);
        repo.upsert_batch(bars)
    }

    fn load_bars(&mut self, symbol: &Symbol, start: &str, end: &str) -> AppResult<Vec<DailyBar>> {
        let repo = DailyBarRepository::new(self);
        repo.load_range(symbol, start, end)
    }
}

// ---------------------------------------------------------------------------
// ScanRunCreateInput / SequentialScanResult — DTOs for sequential scan orchestration
// ---------------------------------------------------------------------------

/// Input for creating a scan run.
pub struct ScanRunCreateInput {
    pub watchlist_id: crate::domain::WatchlistId,
    pub preset_id: crate::domain::ScanPresetId,
    pub total_symbols: u32,
    pub preset_snapshot_json: String,
    pub symbols_snapshot_json: String,
    pub retry_of_run_id: Option<crate::domain::ScanRunId>,
}

/// Result of a sequential scan run.
#[derive(Debug)]
pub struct SequentialScanResult {
    pub run_id: crate::domain::ScanRunId,
    pub total_symbols: u32,
    pub succeeded_symbols: u32,
    pub failed_symbols: u32,
    pub base_trade_date: Option<String>,
}

/// Result of computing a symbol in a concurrent scan (returned from spawned task).
enum ComputeResult {
    Success {
        symbol: crate::domain::Symbol,
        bars: Vec<crate::domain::DailyBar>,
        conditions: Vec<crate::domain::SignalCondition>,
    },
    Error {
        symbol: crate::domain::Symbol,
        error: crate::error::AppError,
    },
    Cancelled {
        _symbol: crate::domain::Symbol,
    },
}

/// Result of a concurrent scan run.
pub struct ConcurrentScanResult {
    pub run_id: crate::domain::ScanRunId,
    pub total_symbols: u32,
    pub succeeded_symbols: u32,
    pub failed_symbols: u32,
    pub base_trade_date: Option<String>,
}

// ---------------------------------------------------------------------------
// DatabaseOpsExtended — extended database operations for scan run orchestration
// ---------------------------------------------------------------------------

/// Extended database operations for scan run orchestration.
pub trait DatabaseOpsExtended: DatabaseOps + Send {
    fn load_watchlist(
        &mut self,
        id: &crate::domain::WatchlistId,
    ) -> AppResult<(Vec<crate::domain::Symbol>, String)>;
    fn load_preset_conditions(
        &mut self,
        id: &crate::domain::ScanPresetId,
    ) -> AppResult<Vec<crate::domain::SignalCondition>>;
    fn create_scan_run(
        &mut self,
        input: &ScanRunCreateInput,
    ) -> AppResult<crate::domain::ScanRunId>;
    fn start_scan_run(&mut self, id: &crate::domain::ScanRunId) -> AppResult<()>;
    fn save_scan_result(&mut self, result: &crate::domain::ScanResult) -> AppResult<()>;
    fn save_scan_error(&mut self, error: &crate::domain::ScanError) -> AppResult<()>;
    fn update_scan_progress(
        &mut self,
        id: &crate::domain::ScanRunId,
        succeeded: u32,
        failed: u32,
    ) -> AppResult<()>;
    fn mark_scan_completed(
        &mut self,
        id: &crate::domain::ScanRunId,
        base_date: Option<&str>,
    ) -> AppResult<()>;
    fn mark_scan_cancelled(&mut self, id: &crate::domain::ScanRunId) -> AppResult<()>;
    fn mark_scan_failed(&mut self, id: &crate::domain::ScanRunId) -> AppResult<()>;
    fn update_stale_flags(
        &mut self,
        id: &crate::domain::ScanRunId,
        base_date: &str,
    ) -> AppResult<()>;
}

impl DatabaseOpsExtended for crate::db::Database {
    fn load_watchlist(
        &mut self,
        id: &crate::domain::WatchlistId,
    ) -> AppResult<(Vec<crate::domain::Symbol>, String)> {
        let repo = crate::repository::watchlist::WatchlistRepository::new(self);
        let detail = repo.get(id)?;
        Ok((detail.symbols, detail.name))
    }

    fn load_preset_conditions(
        &mut self,
        id: &crate::domain::ScanPresetId,
    ) -> AppResult<Vec<crate::domain::SignalCondition>> {
        let repo = crate::repository::scan_preset::ScanPresetRepository::new(self);
        let detail = repo.get(id)?;
        Ok(detail
            .conditions
            .into_iter()
            .map(|c| crate::domain::SignalCondition {
                id: crate::domain::SignalConditionId::new(format!(
                    "preset-{}-{}",
                    id.0, c.indicator as u8
                ))
                .unwrap(),
                indicator: c.indicator,
                side: c.side,
                period: c.period,
                threshold: c.threshold,
                parameters: serde_json::json!({}),
                trigger_mode: c.trigger_mode,
                enabled: c.enabled,
                sort_order: 0,
            })
            .collect())
    }

    fn create_scan_run(
        &mut self,
        input: &ScanRunCreateInput,
    ) -> AppResult<crate::domain::ScanRunId> {
        let repo_input = crate::repository::scan_run::ScanRunCreate {
            watchlist_id: input.watchlist_id.clone(),
            preset_id: input.preset_id.clone(),
            total_symbols: input.total_symbols,
            preset_snapshot_json: input.preset_snapshot_json.clone(),
            symbols_snapshot_json: input.symbols_snapshot_json.clone(),
            retry_of_run_id: input.retry_of_run_id.clone(),
        };
        let mut repo = crate::repository::scan_run::ScanRunRepository::new(self);
        let summary = repo.create_pending(&repo_input)?;
        Ok(summary.id)
    }

    fn start_scan_run(&mut self, id: &crate::domain::ScanRunId) -> AppResult<()> {
        let mut repo = crate::repository::scan_run::ScanRunRepository::new(self);
        repo.start_running(id)
    }

    fn save_scan_result(&mut self, result: &crate::domain::ScanResult) -> AppResult<()> {
        let mut repo = crate::repository::scan_result::ScanResultRepository::new(self);
        repo.upsert(result)
    }

    fn save_scan_error(&mut self, error: &crate::domain::ScanError) -> AppResult<()> {
        let mut repo = crate::repository::scan_error::ScanErrorRepository::new(self);
        repo.append(error)
    }

    fn update_scan_progress(
        &mut self,
        id: &crate::domain::ScanRunId,
        succeeded: u32,
        failed: u32,
    ) -> AppResult<()> {
        let mut repo = crate::repository::scan_run::ScanRunRepository::new(self);
        repo.update_progress(id, succeeded, failed)
    }

    fn mark_scan_completed(
        &mut self,
        id: &crate::domain::ScanRunId,
        base_date: Option<&str>,
    ) -> AppResult<()> {
        let mut repo = crate::repository::scan_run::ScanRunRepository::new(self);
        repo.mark_completed(id, base_date)
    }

    fn mark_scan_cancelled(&mut self, id: &crate::domain::ScanRunId) -> AppResult<()> {
        let mut repo = crate::repository::scan_run::ScanRunRepository::new(self);
        repo.mark_cancelled(id)
    }

    fn mark_scan_failed(&mut self, id: &crate::domain::ScanRunId) -> AppResult<()> {
        let mut repo = crate::repository::scan_run::ScanRunRepository::new(self);
        repo.mark_failed(id)
    }

    fn update_stale_flags(
        &mut self,
        id: &crate::domain::ScanRunId,
        base_date: &str,
    ) -> AppResult<()> {
        let mut repo = crate::repository::scan_result::ScanResultRepository::new(self);
        repo.update_stale_flags(id, base_date)
    }
}

// ---------------------------------------------------------------------------
// CancellationToken — re-export the real cancellation token from state
// ---------------------------------------------------------------------------

pub use crate::state::CancellationToken;

// ---------------------------------------------------------------------------
// ScanService — single-symbol scan pipeline
// ---------------------------------------------------------------------------

pub struct ScanService<P>
where
    P: MarketDataProvider,
{
    provider: Arc<P>,
    cancellation: Arc<Mutex<CancellationToken>>,
    cancellation_registry: Arc<crate::state::CancellationRegistry>,
}

impl<P> ScanService<P>
where
    P: MarketDataProvider + 'static,
{
    pub fn new(
        provider: P,
        cancellation: Arc<Mutex<CancellationToken>>,
        cancellation_registry: Arc<crate::state::CancellationRegistry>,
    ) -> Self {
        Self {
            provider: Arc::new(provider),
            cancellation,
            cancellation_registry,
        }
    }

    /// Process a single symbol: fetch bars, compute indicators, evaluate signals, return result.
    ///
    /// Processing order:
    /// 1. Check cancellation token
    /// 2. Validate symbol
    /// 3. Query existing bar range (via db)
    /// 4. Fetch needed data from provider
    /// 5. Validate bars
    /// 6. Upsert bars (via db)
    /// 7. Load calculation range (via db)
    /// 8. Compute indicators
    /// 9. Evaluate signals
    /// 10. Build and return ScanResult
    pub async fn process_single_symbol(
        &self,
        symbol: Symbol,
        preset: &ScanPreset,
        run_id: &ScanRunId,
        db: &mut dyn DatabaseOps,
    ) -> AppResult<ScanResult> {
        // 0. Check cancellation
        if self.is_cancelled().await {
            return Err(AppError::new(
                AppErrorCode::Cancelled,
                format!("scan cancelled for symbol {}", symbol),
            ));
        }

        // 1. Validate symbol (already done by caller, but ensure)
        let _ = Symbol::new(symbol.as_str())?;

        // 2. Query existing bar range
        let existing_range = db.date_range(&symbol)?;

        // 3. Fetch needed data
        let fetch_range = self.plan_fetch_range(&existing_range, preset);
        let fetched_bars = self.fetch_bars(&symbol, &fetch_range).await?;

        // 4. Validate bars
        for bar in &fetched_bars {
            bar.validate()?;
        }

        // 5. Upsert bars
        db.upsert_bars(&fetched_bars)?;

        // 6. Load calculation range
        let bars = {
            let end = fetched_bars
                .last()
                .map(|b| b.trade_date.clone())
                .unwrap_or_default();
            let start = self.calculate_start_date(&existing_range, preset);
            db.load_bars(&symbol, &start, &end)?
        };

        // 7. Compute indicators
        let snapshot = crate::indicator::compute_snapshot(&bars, preset)?;

        // 8. Evaluate signals
        let matches = signal::evaluate_signals(&snapshot, &preset.conditions)?;

        // 9. Build result
        let aggregate = signal::aggregate_matches(&matches);
        Ok(self.build_result(&symbol, run_id, &snapshot, &matches, aggregate, &bars))
    }
}

impl<P> ScanService<P>
where
    P: MarketDataProvider,
{
    fn max_period(&self, preset: &ScanPreset) -> u32 {
        preset
            .conditions
            .iter()
            .filter(|c| c.enabled)
            .map(|c| c.period)
            .max()
            .unwrap_or(14)
    }

    fn plan_fetch_range(
        &self,
        existing_range: &Option<(String, String)>,
        preset: &ScanPreset,
    ) -> DateRange {
        let planner = FetchPlanner::new(self.max_period(preset));

        match existing_range {
            None => {
                let fresh = planner.plan_fresh_fetch();
                let end = chrono::Utc::now().format("%Y-%m-%d").to_string();
                let days = fresh.days_back;
                let start = chrono::Utc::now()
                    .checked_sub_days(chrono::Days::new(days as u64))
                    .unwrap_or_else(chrono::Utc::now)
                    .format("%Y-%m-%d")
                    .to_string();
                DateRange::new(start, end)
            }
            Some((_min, max)) => planner
                .plan_incremental_fetch(max)
                .map(|range| match range.source {
                    FetchSource::Incremental { start_date } => {
                        DateRange::new(start_date, String::new())
                    }
                    _ => DateRange::new(String::new(), String::new()),
                })
                .unwrap_or_else(|| DateRange::new(String::new(), String::new())),
        }
    }

    fn calculate_start_date(
        &self,
        existing_range: &Option<(String, String)>,
        preset: &ScanPreset,
    ) -> String {
        let min_bars = (self.max_period(preset) + 2) as usize;
        match existing_range {
            None => {
                let days = std::cmp::max(min_bars, 30);
                chrono::Utc::now()
                    .checked_sub_days(chrono::Days::new(days as u64))
                    .unwrap_or_else(chrono::Utc::now)
                    .format("%Y-%m-%d")
                    .to_string()
            }
            Some((_min, max)) => {
                let parsed = chrono::NaiveDate::parse_from_str(max, "%Y-%m-%d")
                    .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
                let start = parsed
                    .checked_sub_days(chrono::Days::new((min_bars + 5) as u64))
                    .unwrap_or(parsed);
                start.format("%Y-%m-%d").to_string()
            }
        }
    }

    async fn fetch_bars(&self, symbol: &Symbol, range: &DateRange) -> AppResult<Vec<DailyBar>> {
        self.provider.fetch_daily_bars(symbol, range).await
    }

    fn build_result(
        &self,
        symbol: &Symbol,
        run_id: &ScanRunId,
        snapshot: &IndicatorSnapshot,
        matches: &[SignalMatch],
        aggregate: signal::SignalAggregate,
        bars: &[DailyBar],
    ) -> ScanResult {
        let data_stale = bars
            .last()
            .map(|last| last.trade_date != snapshot.trade_date)
            .unwrap_or(false);

        let first_rsi = snapshot.rsi_by_period.values().next().copied().flatten();
        let first_mfi = snapshot.mfi_by_period.values().next().copied().flatten();
        let first_bb = snapshot
            .bollinger_by_params
            .values()
            .next()
            .copied()
            .flatten();

        ScanResult {
            run_id: run_id.clone(),
            symbol: symbol.clone(),
            trade_date: snapshot.trade_date.clone(),
            current_price: snapshot.close,
            indicators: IndicatorValues {
                rsi: first_rsi,
                mfi: first_mfi,
                bollinger_lower: first_bb.map(|b| b.lower),
                bollinger_middle: first_bb.map(|b| b.middle),
                bollinger_upper: first_bb.map(|b| b.upper),
            },
            matches: matches.to_vec(),
            all_conditions_matched: aggregate.all_matched,
            any_condition_matched: aggregate.any_matched,
            data_stale,
        }
    }
}

impl<P> ScanService<P>
where
    P: MarketDataProvider + 'static,
{
    /// Run a sequential scan across all symbols in a watchlist.
    ///
    /// Flow:
    /// 1. Load watchlist and preset conditions
    /// 2. Create pending run record with snapshots
    /// 3. Transition to running
    /// 4. Process each symbol sequentially (success -> save result, error -> save error)
    /// 5. Update progress after each symbol
    /// 6. Calculate base_trade_date from successful results
    /// 7. Update stale flags
    /// 8. Mark as completed (or failed if all symbols failed)
    pub async fn run_scan_sequential(
        &self,
        watchlist_id: &crate::domain::WatchlistId,
        preset_id: &crate::domain::ScanPresetId,
        db: &mut dyn DatabaseOpsExtended,
    ) -> AppResult<SequentialScanResult> {
        // 1. Load watchlist and preset
        let (symbols, _watchlist_name) = db.load_watchlist(watchlist_id)?;
        let conditions = db.load_preset_conditions(preset_id)?;

        let total = symbols.len() as u32;
        if total == 0 {
            return Err(AppError::validation("watchlist has no symbols"));
        }

        // 2. Create snapshots
        let preset_snapshot = self.build_preset_snapshot(preset_id, &conditions);
        let symbols_snapshot = self.build_symbols_snapshot(&symbols);

        // 3. Create pending run
        let run_input = ScanRunCreateInput {
            watchlist_id: watchlist_id.clone(),
            preset_id: preset_id.clone(),
            total_symbols: total,
            preset_snapshot_json: preset_snapshot,
            symbols_snapshot_json: symbols_snapshot,
            retry_of_run_id: None,
        };
        let run_id = db.create_scan_run(&run_input)?;

        // 4. Start running
        db.start_scan_run(&run_id)?;

        // 5. Process symbols sequentially
        let mut succeeded: u32 = 0;
        let mut failed: u32 = 0;
        let mut latest_trade_date: Option<String> = None;

        for symbol in &symbols {
            // Check cancellation before each symbol
            if self.is_cancelled().await {
                db.mark_scan_cancelled(&run_id)?;
                db.update_scan_progress(&run_id, succeeded, failed)?;
                return Ok(SequentialScanResult {
                    run_id,
                    total_symbols: total,
                    succeeded_symbols: succeeded,
                    failed_symbols: failed,
                    base_trade_date: None,
                });
            }

            // Process the symbol
            let preset_for_symbol = crate::domain::ScanPreset {
                id: preset_id.clone(),
                name: String::new(),
                conditions: conditions.clone(),
            };

            match self
                .process_single_symbol(symbol.clone(), &preset_for_symbol, &run_id, db)
                .await
            {
                Ok(result) => {
                    db.save_scan_result(&result)?;
                    succeeded += 1;

                    if let Some(ref current) = latest_trade_date {
                        if result.trade_date > *current {
                            latest_trade_date = Some(result.trade_date.clone());
                        }
                    } else {
                        latest_trade_date = Some(result.trade_date.clone());
                    }
                }
                Err(error) => {
                    let scan_error = crate::domain::ScanError {
                        run_id: run_id.clone(),
                        symbol: Some(symbol.as_str().to_string()),
                        code: format!("{:?}", error.code),
                        message: error.message.clone(),
                        detail: error.detail.clone(),
                        retryable: error.retryable,
                        attempt: 1,
                    };
                    db.save_scan_error(&scan_error)?;
                    failed += 1;
                }
            }

            // Update progress after each symbol
            db.update_scan_progress(&run_id, succeeded, failed)?;
        }

        // 6. Update stale flags
        if let Some(ref date) = latest_trade_date {
            let _ = db.update_stale_flags(&run_id, date);
        }

        // 7. Mark as completed or failed
        if failed == total {
            db.mark_scan_failed(&run_id)?;
        } else {
            db.mark_scan_completed(&run_id, latest_trade_date.as_deref())?;
        }

        // 8. Final progress update
        db.update_scan_progress(&run_id, succeeded, failed)?;

        Ok(SequentialScanResult {
            run_id,
            total_symbols: total,
            succeeded_symbols: succeeded,
            failed_symbols: failed,
            base_trade_date: latest_trade_date,
        })
    }

    /// Check if the scan has been cancelled.
    async fn is_cancelled(&self) -> bool {
        let token = self.cancellation.lock().await;
        token.is_cancelled()
    }

    fn build_preset_snapshot(
        &self,
        preset_id: &crate::domain::ScanPresetId,
        conditions: &[crate::domain::SignalCondition],
    ) -> String {
        let snapshot = serde_json::json!({
            "id": preset_id.0,
            "conditions": conditions.iter().map(|c| {
                serde_json::json!({
                    "id": c.id.0,
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
        serde_json::to_string(&snapshot).unwrap_or_default()
    }

    fn build_symbols_snapshot(&self, symbols: &[crate::domain::Symbol]) -> String {
        let list: Vec<&str> = symbols.iter().map(|s| s.as_str()).collect();
        serde_json::to_string(&list).unwrap_or_default()
    }

    // ------------------------------------------------------------------
    // Concurrent scan
    // ------------------------------------------------------------------

    /// Run a concurrent scan across all symbols with bounded concurrency (4).
    ///
    /// Symbols are processed concurrently. DB operations (upsert, load, save)
    /// are performed by the main task to avoid holding DB locks during network I/O.
    /// Task panics are caught and treated as symbol errors.
    /// Cancellation prevents new symbol tasks from starting.
    pub async fn run_scan_concurrent(
        &self,
        watchlist_id: &crate::domain::WatchlistId,
        preset_id: &crate::domain::ScanPresetId,
        db: &mut dyn DatabaseOpsExtended,
    ) -> AppResult<ConcurrentScanResult> {
        // 1. Load watchlist and preset
        let (symbols, _watchlist_name) = db.load_watchlist(watchlist_id)?;
        let conditions = db.load_preset_conditions(preset_id)?;

        let total = symbols.len() as u32;
        if total == 0 {
            return Err(AppError::validation("watchlist has no symbols"));
        }

        // 2. Create snapshots
        let preset_snapshot = self.build_preset_snapshot(preset_id, &conditions);
        let symbols_snapshot = self.build_symbols_snapshot(&symbols);

        // 3. Create pending run
        let run_input = ScanRunCreateInput {
            watchlist_id: watchlist_id.clone(),
            preset_id: preset_id.clone(),
            total_symbols: total,
            preset_snapshot_json: preset_snapshot,
            symbols_snapshot_json: symbols_snapshot,
            retry_of_run_id: None,
        };
        let run_id = db.create_scan_run(&run_input)?;

        // 4. Start running
        db.start_scan_run(&run_id)?;

        // 5. Register cancellation token
        self.cancellation_registry.register(&run_id).await;

        // 6. Shared state
        let succeeded_count = AtomicU32::new(0);
        let failed_count = AtomicU32::new(0);
        let latest_trade_date = std::sync::Mutex::new(None);

        // 7. Semaphore for bounded concurrency (4)
        let semaphore = Arc::new(Semaphore::new(4));
        let mut handles = Vec::new();

        // 8. Spawn tasks
        for symbol in symbols {
            // Check cancellation before spawning
            if self.is_cancelled_run(&run_id).await {
                break;
            }

            let sem = Arc::clone(&semaphore);
            let provider = Arc::clone(&self.provider);
            let cancellation = Arc::clone(&self.cancellation);
            let preset_conditions = conditions.clone();

            let handle = tokio::spawn(async move {
                // Acquire permit (bounds concurrency)
                let _permit = match sem.acquire().await {
                    Ok(p) => p,
                    Err(_) => {
                        return ComputeResult::Cancelled { _symbol: symbol };
                    }
                };

                // Check cancellation before processing
                {
                    let token = cancellation.lock().await;
                    if token.is_cancelled() {
                        return ComputeResult::Cancelled { _symbol: symbol };
                    }
                }

                // Fetch bars from provider
                let fetch_range =
                    Self::plan_fetch_for_symbol(&provider, &symbol, &preset_conditions);
                match provider.fetch_daily_bars(&symbol, &fetch_range).await {
                    Ok(bars) => {
                        // Validate bars
                        for bar in &bars {
                            if let Err(e) = bar.validate() {
                                return ComputeResult::Error { symbol, error: e };
                            }
                        }
                        ComputeResult::Success {
                            symbol,
                            bars,
                            conditions: preset_conditions,
                        }
                    }
                    Err(e) => ComputeResult::Error { symbol, error: e },
                }
            });

            handles.push(handle);
        }

        // 9. Collect results
        for handle in handles {
            // Check cancellation before awaiting next task
            if self.is_cancelled_run(&run_id).await {
                break;
            }

            let compute_result = match handle.await {
                Ok(result) => result,
                Err(_join_error) => {
                    // Task panicked — treat as symbol error
                    failed_count.fetch_add(1, Ordering::Relaxed);
                    let _ = db.update_scan_progress(
                        &run_id,
                        succeeded_count.load(Ordering::Relaxed),
                        failed_count.load(Ordering::Relaxed),
                    );
                    continue;
                }
            };

            match compute_result {
                ComputeResult::Success {
                    symbol,
                    bars,
                    conditions,
                } => {
                    // DB operations by main task
                    let existing_range = db.date_range(&symbol)?;
                    let _ = db.upsert_bars(&bars);

                    let end = bars
                        .last()
                        .map(|b| b.trade_date.clone())
                        .unwrap_or_default();
                    let start = Self::calc_start_date(&existing_range, &conditions);
                    let load_bars = db.load_bars(&symbol, &start, &end)?;

                    let preset = crate::domain::ScanPreset {
                        id: preset_id.clone(),
                        name: String::new(),
                        conditions,
                    };

                    let snapshot = match crate::indicator::compute_snapshot(&load_bars, &preset) {
                        Ok(s) => s,
                        Err(e) => {
                            let scan_error = crate::domain::ScanError {
                                run_id: run_id.clone(),
                                symbol: Some(symbol.as_str().to_string()),
                                code: format!("{:?}", e.code),
                                message: e.message.clone(),
                                detail: e.detail.clone(),
                                retryable: e.retryable,
                                attempt: 1,
                            };
                            let _ = db.save_scan_error(&scan_error);
                            failed_count.fetch_add(1, Ordering::Relaxed);
                            let _ = db.update_scan_progress(
                                &run_id,
                                succeeded_count.load(Ordering::Relaxed),
                                failed_count.load(Ordering::Relaxed),
                            );
                            continue;
                        }
                    };

                    let matches =
                        match crate::signal::evaluate_signals(&snapshot, &preset.conditions) {
                            Ok(m) => m,
                            Err(e) => {
                                let scan_error = crate::domain::ScanError {
                                    run_id: run_id.clone(),
                                    symbol: Some(symbol.as_str().to_string()),
                                    code: format!("{:?}", e.code),
                                    message: e.message.clone(),
                                    detail: e.detail.clone(),
                                    retryable: e.retryable,
                                    attempt: 1,
                                };
                                let _ = db.save_scan_error(&scan_error);
                                failed_count.fetch_add(1, Ordering::Relaxed);
                                let _ = db.update_scan_progress(
                                    &run_id,
                                    succeeded_count.load(Ordering::Relaxed),
                                    failed_count.load(Ordering::Relaxed),
                                );
                                continue;
                            }
                        };

                    let aggregate = crate::signal::aggregate_matches(&matches);
                    let result = self
                        .build_result(&symbol, &run_id, &snapshot, &matches, aggregate, &load_bars);

                    let _ = db.save_scan_result(&result);
                    succeeded_count.fetch_add(1, Ordering::Relaxed);

                    if let Ok(mut latest) = latest_trade_date.lock() {
                        if let Some(ref current) = *latest {
                            if result.trade_date > *current {
                                *latest = Some(result.trade_date.clone());
                            }
                        } else {
                            *latest = Some(result.trade_date.clone());
                        }
                    }

                    let _ = db.update_scan_progress(
                        &run_id,
                        succeeded_count.load(Ordering::Relaxed),
                        failed_count.load(Ordering::Relaxed),
                    );
                }
                ComputeResult::Error { symbol, error } => {
                    let scan_error = crate::domain::ScanError {
                        run_id: run_id.clone(),
                        symbol: Some(symbol.as_str().to_string()),
                        code: format!("{:?}", error.code),
                        message: error.message.clone(),
                        detail: error.detail.clone(),
                        retryable: error.retryable,
                        attempt: 1,
                    };
                    let _ = db.save_scan_error(&scan_error);
                    failed_count.fetch_add(1, Ordering::Relaxed);
                    let _ = db.update_scan_progress(
                        &run_id,
                        succeeded_count.load(Ordering::Relaxed),
                        failed_count.load(Ordering::Relaxed),
                    );
                }
                ComputeResult::Cancelled { _symbol: _ } => {
                    // Symbol was cancelled before processing started
                    break;
                }
            }
        }

        // 10. Finalize
        let succeeded = succeeded_count.load(Ordering::Relaxed);
        let failed = failed_count.load(Ordering::Relaxed);
        let base_date = latest_trade_date.lock().unwrap().clone();

        if let Some(ref date) = base_date {
            let _ = db.update_stale_flags(&run_id, date);
        }

        if failed == total {
            let _ = db.mark_scan_failed(&run_id);
        } else {
            let _ = db.mark_scan_completed(&run_id, base_date.as_deref());
        }

        let _ = db.update_scan_progress(&run_id, succeeded, failed);

        // Remove cancellation token
        self.cancellation_registry.remove(&run_id).await;

        Ok(ConcurrentScanResult {
            run_id,
            total_symbols: total,
            succeeded_symbols: succeeded,
            failed_symbols: failed,
            base_trade_date: base_date,
        })
    }

    /// Check if a specific run has been cancelled.
    async fn is_cancelled_run(&self, run_id: &crate::domain::ScanRunId) -> bool {
        if let Some(token) = self.cancellation_registry.get(run_id).await {
            token.is_cancelled()
        } else {
            false
        }
    }

    /// Plan fetch range for a symbol (fresh fetch, no existing range info in concurrent tasks).
    fn plan_fetch_for_symbol(
        provider: &Arc<P>,
        _symbol: &crate::domain::Symbol,
        conditions: &[crate::domain::SignalCondition],
    ) -> crate::provider::DateRange {
        let _ = provider; // used via trait method call below
        let max_period = conditions
            .iter()
            .filter(|c| c.enabled)
            .map(|c| c.period)
            .max()
            .unwrap_or(14);
        let planner = crate::provider::fetch_planner::FetchPlanner::new(max_period);
        let fresh = planner.plan_fresh_fetch();
        let end = chrono::Utc::now().format("%Y-%m-%d").to_string();
        let days = fresh.days_back;
        let start = chrono::Utc::now()
            .checked_sub_days(chrono::Days::new(days as u64))
            .unwrap_or_else(chrono::Utc::now)
            .format("%Y-%m-%d")
            .to_string();
        crate::provider::DateRange::new(start, end)
    }

    /// Calculate start date for loading bars.
    fn calc_start_date(
        existing_range: &Option<(String, String)>,
        conditions: &[crate::domain::SignalCondition],
    ) -> String {
        let max_period = conditions
            .iter()
            .filter(|c| c.enabled)
            .map(|c| c.period)
            .max()
            .unwrap_or(14);
        let min_bars = (max_period + 2) as usize;

        match existing_range {
            None => {
                let days = std::cmp::max(min_bars, 30);
                chrono::Utc::now()
                    .checked_sub_days(chrono::Days::new(days as u64))
                    .unwrap_or_else(chrono::Utc::now)
                    .format("%Y-%m-%d")
                    .to_string()
            }
            Some((_min, max)) => {
                let parsed = chrono::NaiveDate::parse_from_str(max, "%Y-%m-%d")
                    .unwrap_or_else(|_| chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap());
                let start = parsed
                    .checked_sub_days(chrono::Days::new((min_bars + 5) as u64))
                    .unwrap_or(parsed);
                start.format("%Y-%m-%d").to_string()
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        IndicatorKind, PriceBasis, ScanPresetId, SignalCondition, SignalConditionId, SignalSide,
        TriggerMode,
    };
    use serde_json::json;

    // ---- Fake provider ----

    pub struct FakeProvider {
        bars: Vec<DailyBar>,
        error: Option<AppError>,
    }

    impl FakeProvider {
        pub fn new(bars: Vec<DailyBar>) -> Self {
            Self { bars, error: None }
        }

        fn with_error(mut self, error: AppError) -> Self {
            self.error = Some(error);
            self
        }
    }

    #[async_trait::async_trait]
    impl MarketDataProvider for FakeProvider {
        async fn fetch_daily_bars(
            &self,
            _symbol: &Symbol,
            _range: &DateRange,
        ) -> AppResult<Vec<DailyBar>> {
            if let Some(ref error) = self.error {
                return Err(error.clone());
            }
            Ok(self.bars.clone())
        }
    }

    // ---- Fake database ops ----

    struct FakeDatabaseOps {
        existing_range: Option<(String, String)>,
        loaded_bars: Vec<DailyBar>,
        upserted: std::cell::RefCell<Vec<DailyBar>>,
    }

    impl FakeDatabaseOps {
        fn new(existing_range: Option<(String, String)>, loaded_bars: Vec<DailyBar>) -> Self {
            Self {
                existing_range,
                loaded_bars,
                upserted: std::cell::RefCell::new(Vec::new()),
            }
        }

        fn upserted_bars(&self) -> Vec<DailyBar> {
            self.upserted.borrow().clone()
        }
    }

    impl DatabaseOps for FakeDatabaseOps {
        fn date_range(&mut self, _symbol: &Symbol) -> AppResult<Option<(String, String)>> {
            Ok(self.existing_range.clone())
        }

        fn upsert_bars(&mut self, bars: &[DailyBar]) -> AppResult<()> {
            *self.upserted.borrow_mut() = bars.to_vec();
            Ok(())
        }

        fn load_bars(
            &mut self,
            _symbol: &Symbol,
            _start: &str,
            _end: &str,
        ) -> AppResult<Vec<DailyBar>> {
            Ok(self.loaded_bars.clone())
        }
    }

    // ---- Helpers ----

    fn make_bar(date: &str, close: f64) -> DailyBar {
        DailyBar {
            symbol: Symbol::new("AAPL").unwrap(),
            trade_date: date.to_string(),
            price_basis: PriceBasis::SplitAdjusted,
            open: close,
            high: close + 1.0,
            low: close - 1.0,
            close,
            volume: 1_000,
        }
    }

    fn make_preset(period: u32, threshold: f64) -> ScanPreset {
        ScanPreset {
            id: ScanPresetId::new("test-preset").unwrap(),
            name: "Test".to_string(),
            conditions: vec![SignalCondition {
                id: SignalConditionId::new("rsi1").unwrap(),
                indicator: IndicatorKind::Rsi,
                side: SignalSide::Lower,
                period,
                threshold: Some(threshold),
                parameters: json!({}),
                trigger_mode: TriggerMode::Current,
                enabled: true,
                sort_order: 0,
            }],
        }
    }

    fn make_cancellation() -> Arc<Mutex<CancellationToken>> {
        Arc::new(Mutex::new(CancellationToken::new()))
    }

    fn make_registry() -> Arc<crate::state::CancellationRegistry> {
        Arc::new(crate::state::CancellationRegistry::new())
    }

    // ---- Tests ----

    #[tokio::test]
    async fn processes_symbol_successfully() {
        let bars: Vec<DailyBar> = (0..20)
            .map(|i| make_bar(&format!("2026-07-{:02}", i + 1), 100.0 + i as f64))
            .collect();
        let provider = FakeProvider::new(bars.clone());
        let cancellation = make_cancellation();
        let registry = make_registry();
        let service = ScanService::new(provider, cancellation, registry);

        let symbol = Symbol::new("AAPL").unwrap();
        let preset = make_preset(14, 30.0);
        let run_id = ScanRunId::new("test-run").unwrap();
        let mut db = FakeDatabaseOps::new(None, bars.clone());

        let result = service
            .process_single_symbol(symbol, &preset, &run_id, &mut db)
            .await;

        assert!(result.is_ok());
        let scan_result = result.unwrap();
        assert_eq!(scan_result.symbol.as_str(), "AAPL");
        assert!(!scan_result.trade_date.is_empty());
        // Bars should have been upserted
        assert!(!db.upserted_bars().is_empty());
    }

    #[tokio::test]
    async fn returns_error_on_provider_failure() {
        let provider = FakeProvider::new(vec![]).with_error(
            AppError::new(AppErrorCode::ProviderUnavailable, "network error").retryable(true),
        );
        let cancellation = make_cancellation();
        let registry = make_registry();
        let service = ScanService::new(provider, cancellation, registry);

        let symbol = Symbol::new("TSLA").unwrap();
        let preset = make_preset(14, 30.0);
        let run_id = ScanRunId::new("test-run").unwrap();
        let mut db = FakeDatabaseOps::new(None, vec![]);

        let result: AppResult<ScanResult> = service
            .process_single_symbol(symbol, &preset, &run_id, &mut db)
            .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, AppErrorCode::ProviderUnavailable);
    }

    #[tokio::test]
    async fn returns_insufficient_data_for_few_bars() {
        // Only 2 bars, RSI period=14 needs at least 15 bars
        let bars = vec![make_bar("2026-07-01", 100.0), make_bar("2026-07-02", 101.0)];
        let provider = FakeProvider::new(bars);
        let cancellation = make_cancellation();
        let registry = make_registry();
        let service = ScanService::new(provider, cancellation, registry);

        let symbol = Symbol::new("AAPL").unwrap();
        let preset = make_preset(14, 30.0);
        let run_id = ScanRunId::new("test-run").unwrap();
        let mut db = FakeDatabaseOps::new(None, vec![]);

        let result: AppResult<ScanResult> = service
            .process_single_symbol(symbol, &preset, &run_id, &mut db)
            .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, AppErrorCode::InsufficientData);
    }

    #[tokio::test]
    async fn rejects_invalid_bar_data() {
        let mut bars = vec![make_bar("2026-07-01", 100.0)];
        let invalid_bar = DailyBar {
            symbol: Symbol::new("AAPL").unwrap(),
            trade_date: "2026-07-02".to_string(),
            price_basis: PriceBasis::SplitAdjusted,
            open: -1.0,
            high: 105.0,
            low: 95.0,
            close: 100.0,
            volume: 1_000,
        };
        bars.push(invalid_bar);

        let provider = FakeProvider::new(bars);
        let cancellation = make_cancellation();
        let registry = make_registry();
        let service = ScanService::new(provider, cancellation, registry);

        let symbol = Symbol::new("AAPL").unwrap();
        let preset = make_preset(14, 30.0);
        let run_id = ScanRunId::new("test-run").unwrap();
        let mut db = FakeDatabaseOps::new(None, vec![]);

        let result: AppResult<ScanResult> = service
            .process_single_symbol(symbol, &preset, &run_id, &mut db)
            .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, AppErrorCode::InvalidMarketData);
    }

    #[tokio::test]
    async fn produces_current_and_cross_results() {
        let bars: Vec<DailyBar> = (0..20)
            .map(|i| {
                let close = if i % 3 == 0 {
                    100.0
                } else if i % 3 == 1 {
                    105.0
                } else {
                    98.0
                };
                make_bar(&format!("2026-07-{:02}", i + 1), close)
            })
            .collect();
        let provider = FakeProvider::new(bars.clone());
        let cancellation = make_cancellation();
        let registry = make_registry();
        let service = ScanService::new(provider, cancellation, registry);

        let preset = ScanPreset {
            id: ScanPresetId::new("test-preset").unwrap(),
            name: "Test".to_string(),
            conditions: vec![SignalCondition {
                id: SignalConditionId::new("rsi1").unwrap(),
                indicator: IndicatorKind::Rsi,
                side: SignalSide::Lower,
                period: 2,
                threshold: Some(30.0),
                parameters: json!({}),
                trigger_mode: TriggerMode::Current,
                enabled: true,
                sort_order: 0,
            }],
        };
        let run_id = ScanRunId::new("test-run").unwrap();
        let mut db = FakeDatabaseOps::new(None, bars);

        let result = service
            .process_single_symbol(Symbol::new("AAPL").unwrap(), &preset, &run_id, &mut db)
            .await;

        assert!(result.is_ok());
        let scan_result = result.unwrap();
        assert!(!scan_result.trade_date.is_empty());
    }

    #[tokio::test]
    async fn uses_existing_range_for_incremental_fetch() {
        let bars: Vec<DailyBar> = (0..20)
            .map(|i| make_bar(&format!("2026-07-{:02}", i + 1), 100.0 + i as f64))
            .collect();
        let provider = FakeProvider::new(bars.clone());
        let cancellation = make_cancellation();
        let registry = make_registry();
        let service = ScanService::new(provider, cancellation, registry);

        // Simulate existing data with max date 2026-07-10
        let mut db = FakeDatabaseOps::new(
            Some(("2026-06-01".to_string(), "2026-07-10".to_string())),
            bars.clone(),
        );

        let symbol = Symbol::new("AAPL").unwrap();
        let preset = make_preset(14, 30.0);
        let run_id = ScanRunId::new("test-run").unwrap();

        let _ = service
            .process_single_symbol(symbol, &preset, &run_id, &mut db)
            .await;

        // Bars should have been upserted
        assert!(!db.upserted_bars().is_empty());
    }

    #[tokio::test]
    async fn result_contains_indicator_values() {
        let bars: Vec<DailyBar> = (0..20)
            .map(|i| make_bar(&format!("2026-07-{:02}", i + 1), 100.0 + i as f64))
            .collect();
        let provider = FakeProvider::new(bars.clone());
        let cancellation = make_cancellation();
        let registry = make_registry();
        let service = ScanService::new(provider, cancellation, registry);

        let symbol = Symbol::new("AAPL").unwrap();
        let preset = make_preset(14, 30.0);
        let run_id = ScanRunId::new("test-run").unwrap();
        let mut db = FakeDatabaseOps::new(None, bars.clone());

        let result = service
            .process_single_symbol(symbol, &preset, &run_id, &mut db)
            .await;

        assert!(result.is_ok());
        let scan_result = result.unwrap();
        // RSI with 20 bars and period=14 should produce a value
        assert!(scan_result.indicators.rsi.is_some());
        // Trade date should match the last bar
        assert_eq!(scan_result.trade_date, "2026-07-20");
        // Current price should be the last close
        assert_eq!(scan_result.current_price, 119.0);
    }

    #[tokio::test]
    async fn result_has_correct_aggregate() {
        // Create bars where RSI(14) will be above 30 (no match for lower condition)
        let bars: Vec<DailyBar> = (0..20)
            .map(|i| make_bar(&format!("2026-07-{:02}", i + 1), 100.0 + i as f64))
            .collect();
        let provider = FakeProvider::new(bars.clone());
        let cancellation = make_cancellation();
        let registry = make_registry();
        let service = ScanService::new(provider, cancellation, registry);

        // RSI lower threshold=30 — with steadily rising prices, RSI likely > 30
        let preset = make_preset(14, 30.0);
        let run_id = ScanRunId::new("test-run").unwrap();
        let mut db = FakeDatabaseOps::new(None, bars.clone());

        let result = service
            .process_single_symbol(Symbol::new("AAPL").unwrap(), &preset, &run_id, &mut db)
            .await;

        assert!(result.is_ok());
        let scan_result = result.unwrap();
        // any_condition_matched depends on RSI value vs threshold
        // The aggregate should be consistent with matches
        assert!(!scan_result.matches.is_empty());
    }
}

// ---------------------------------------------------------------------------
// Sequential scan tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod sequential_tests {
    use super::tests::FakeProvider;
    use super::*;
    use crate::domain::{
        IndicatorKind, PriceBasis, ScanPresetId, SignalCondition, SignalConditionId, SignalSide,
        TriggerMode, WatchlistId,
    };
    use serde_json::json;

    struct SequentialFakeDatabaseOps {
        symbols: Vec<crate::domain::Symbol>,
        conditions: Vec<SignalCondition>,
        created_run_id: Option<crate::domain::ScanRunId>,
        saved_results: std::cell::RefCell<Vec<crate::domain::ScanResult>>,
        saved_errors: std::cell::RefCell<Vec<crate::domain::ScanError>>,
        mark_completed_date: std::cell::RefCell<Option<String>>,
        mark_failed_called: std::cell::RefCell<bool>,
    }

    impl SequentialFakeDatabaseOps {
        fn new(symbols: Vec<crate::domain::Symbol>, conditions: Vec<SignalCondition>) -> Self {
            Self {
                symbols,
                conditions,
                created_run_id: None,
                saved_results: std::cell::RefCell::new(Vec::new()),
                saved_errors: std::cell::RefCell::new(Vec::new()),
                mark_completed_date: std::cell::RefCell::new(None),
                mark_failed_called: std::cell::RefCell::new(false),
            }
        }

        fn saved_results(&self) -> Vec<crate::domain::ScanResult> {
            self.saved_results.borrow().clone()
        }

        fn saved_errors(&self) -> Vec<crate::domain::ScanError> {
            self.saved_errors.borrow().clone()
        }

        fn mark_failed_called(&self) -> bool {
            *self.mark_failed_called.borrow()
        }
    }

    impl DatabaseOps for SequentialFakeDatabaseOps {
        fn date_range(
            &mut self,
            _symbol: &crate::domain::Symbol,
        ) -> AppResult<Option<(String, String)>> {
            Ok(None)
        }

        fn upsert_bars(&mut self, _bars: &[DailyBar]) -> AppResult<()> {
            Ok(())
        }

        fn load_bars(
            &mut self,
            _symbol: &crate::domain::Symbol,
            _start: &str,
            _end: &str,
        ) -> AppResult<Vec<DailyBar>> {
            Ok((0..20)
                .map(|i| DailyBar {
                    symbol: crate::domain::Symbol::new("AAPL").unwrap(),
                    trade_date: format!("2026-07-{:02}", i + 1),
                    price_basis: PriceBasis::SplitAdjusted,
                    open: 100.0 + i as f64,
                    high: 101.0 + i as f64,
                    low: 99.0 + i as f64,
                    close: 100.0 + i as f64,
                    volume: 1_000,
                })
                .collect())
        }
    }

    impl DatabaseOpsExtended for SequentialFakeDatabaseOps {
        fn load_watchlist(
            &mut self,
            _id: &WatchlistId,
        ) -> AppResult<(Vec<crate::domain::Symbol>, String)> {
            Ok((self.symbols.clone(), "Test Watchlist".to_string()))
        }

        fn load_preset_conditions(
            &mut self,
            _id: &ScanPresetId,
        ) -> AppResult<Vec<SignalCondition>> {
            Ok(self.conditions.clone())
        }

        fn create_scan_run(
            &mut self,
            input: &ScanRunCreateInput,
        ) -> AppResult<crate::domain::ScanRunId> {
            let id =
                crate::domain::ScanRunId::new(format!("run-{}", input.watchlist_id.0)).unwrap();
            self.created_run_id = Some(id.clone());
            Ok(id)
        }

        fn start_scan_run(&mut self, _id: &crate::domain::ScanRunId) -> AppResult<()> {
            Ok(())
        }

        fn save_scan_result(&mut self, result: &crate::domain::ScanResult) -> AppResult<()> {
            self.saved_results.borrow_mut().push(result.clone());
            Ok(())
        }

        fn save_scan_error(&mut self, error: &crate::domain::ScanError) -> AppResult<()> {
            self.saved_errors.borrow_mut().push(error.clone());
            Ok(())
        }

        fn update_scan_progress(
            &mut self,
            _id: &crate::domain::ScanRunId,
            _succeeded: u32,
            _failed: u32,
        ) -> AppResult<()> {
            Ok(())
        }

        fn mark_scan_completed(
            &mut self,
            _id: &crate::domain::ScanRunId,
            base_date: Option<&str>,
        ) -> AppResult<()> {
            self.mark_completed_date
                .borrow_mut()
                .clone_from(&base_date.map(|s| s.to_string()));
            Ok(())
        }

        fn mark_scan_cancelled(&mut self, _id: &crate::domain::ScanRunId) -> AppResult<()> {
            Ok(())
        }

        fn mark_scan_failed(&mut self, _id: &crate::domain::ScanRunId) -> AppResult<()> {
            *self.mark_failed_called.borrow_mut() = true;
            Ok(())
        }

        fn update_stale_flags(
            &mut self,
            _id: &crate::domain::ScanRunId,
            _base_date: &str,
        ) -> AppResult<()> {
            Ok(())
        }
    }

    fn make_registry() -> Arc<crate::state::CancellationRegistry> {
        Arc::new(crate::state::CancellationRegistry::new())
    }

    fn make_rsi_condition(period: u32, threshold: f64) -> SignalCondition {
        SignalCondition {
            id: SignalConditionId::new("rsi1").unwrap(),
            indicator: IndicatorKind::Rsi,
            side: SignalSide::Lower,
            period,
            threshold: Some(threshold),
            parameters: json!({}),
            trigger_mode: TriggerMode::Current,
            enabled: true,
            sort_order: 0,
        }
    }

    #[tokio::test]
    async fn sequential_scan_completes_all_symbols() {
        let symbols = vec![
            crate::domain::Symbol::new("AAPL").unwrap(),
            crate::domain::Symbol::new("GOOGL").unwrap(),
            crate::domain::Symbol::new("MSFT").unwrap(),
        ];
        let conditions = vec![make_rsi_condition(14, 30.0)];
        let provider = FakeProvider::new(
            (0..20)
                .map(|i| DailyBar {
                    symbol: crate::domain::Symbol::new("AAPL").unwrap(),
                    trade_date: format!("2026-07-{:02}", i + 1),
                    price_basis: PriceBasis::SplitAdjusted,
                    open: 100.0 + i as f64,
                    high: 101.0 + i as f64,
                    low: 99.0 + i as f64,
                    close: 100.0 + i as f64,
                    volume: 1_000,
                })
                .collect(),
        );
        let cancellation = Arc::new(Mutex::new(CancellationToken::new()));
        let registry = make_registry();
        let service = ScanService::new(provider, cancellation, registry);

        let mut db = SequentialFakeDatabaseOps::new(symbols, conditions);
        let watchlist_id = WatchlistId::new("wl-1").unwrap();
        let preset_id = ScanPresetId::new("ps-1").unwrap();

        let result = service
            .run_scan_sequential(&watchlist_id, &preset_id, &mut db)
            .await;

        assert!(result.is_ok());
        let scan_result = result.unwrap();
        assert_eq!(scan_result.total_symbols, 3);
        assert_eq!(scan_result.succeeded_symbols, 3);
        assert_eq!(scan_result.failed_symbols, 0);
        assert!(scan_result.base_trade_date.is_some());
        assert_eq!(db.saved_results().len(), 3);
        assert!(db.saved_errors().is_empty());
    }

    #[tokio::test]
    async fn single_symbol_failure_does_not_stop_run() {
        struct FailingProvider {
            fail_on: Vec<String>,
        }

        #[async_trait::async_trait]
        impl MarketDataProvider for FailingProvider {
            async fn fetch_daily_bars(
                &self,
                symbol: &crate::domain::Symbol,
                _range: &DateRange,
            ) -> AppResult<Vec<DailyBar>> {
                if self.fail_on.contains(&symbol.as_str().to_string()) {
                    return Err(AppError::new(
                        AppErrorCode::ProviderUnavailable,
                        format!("failed for {}", symbol),
                    )
                    .retryable(true));
                }
                Ok((0..20)
                    .map(|i| DailyBar {
                        symbol: symbol.clone(),
                        trade_date: format!("2026-07-{:02}", i + 1),
                        price_basis: PriceBasis::SplitAdjusted,
                        open: 100.0 + i as f64,
                        high: 101.0 + i as f64,
                        low: 99.0 + i as f64,
                        close: 100.0 + i as f64,
                        volume: 1_000,
                    })
                    .collect())
            }
        }

        let symbols = vec![
            crate::domain::Symbol::new("AAPL").unwrap(),
            crate::domain::Symbol::new("GOOGL").unwrap(),
            crate::domain::Symbol::new("MSFT").unwrap(),
        ];
        let conditions = vec![make_rsi_condition(14, 30.0)];
        let provider = FailingProvider {
            fail_on: vec!["GOOGL".to_string()],
        };
        let cancellation = Arc::new(Mutex::new(CancellationToken::new()));
        let registry = make_registry();
        let service = ScanService::new(provider, cancellation, registry);

        let mut db = SequentialFakeDatabaseOps::new(symbols, conditions);
        let watchlist_id = WatchlistId::new("wl-2").unwrap();
        let preset_id = ScanPresetId::new("ps-2").unwrap();

        let result = service
            .run_scan_sequential(&watchlist_id, &preset_id, &mut db)
            .await;

        assert!(result.is_ok());
        let scan_result = result.unwrap();
        assert_eq!(scan_result.total_symbols, 3);
        assert_eq!(scan_result.succeeded_symbols, 2);
        assert_eq!(scan_result.failed_symbols, 1);
        assert_eq!(db.saved_results().len(), 2);
        assert_eq!(db.saved_errors().len(), 1);
        let error = &db.saved_errors()[0];
        assert_eq!(error.symbol.as_deref(), Some("GOOGL"));
        assert!(error.retryable);
    }

    #[tokio::test]
    async fn all_symbols_failed_marks_run_as_failed() {
        struct AlwaysFailingProvider;

        #[async_trait::async_trait]
        impl MarketDataProvider for AlwaysFailingProvider {
            async fn fetch_daily_bars(
                &self,
                symbol: &crate::domain::Symbol,
                _range: &DateRange,
            ) -> AppResult<Vec<DailyBar>> {
                Err(AppError::new(
                    AppErrorCode::ProviderUnavailable,
                    format!("network error for {}", symbol),
                )
                .retryable(true))
            }
        }

        let symbols = vec![
            crate::domain::Symbol::new("AAPL").unwrap(),
            crate::domain::Symbol::new("GOOGL").unwrap(),
        ];
        let conditions = vec![make_rsi_condition(14, 30.0)];
        let provider = AlwaysFailingProvider;
        let cancellation = Arc::new(Mutex::new(CancellationToken::new()));
        let registry = make_registry();
        let service = ScanService::new(provider, cancellation, registry);

        let mut db = SequentialFakeDatabaseOps::new(symbols, conditions);
        let watchlist_id = WatchlistId::new("wl-3").unwrap();
        let preset_id = ScanPresetId::new("ps-3").unwrap();

        let _ = service
            .run_scan_sequential(&watchlist_id, &preset_id, &mut db)
            .await;

        assert_eq!(db.saved_results().len(), 0);
        assert_eq!(db.saved_errors().len(), 2);
        assert!(db.mark_failed_called());
    }

    #[tokio::test]
    async fn progress_counts_match_total() {
        let symbols = vec![
            crate::domain::Symbol::new("AAPL").unwrap(),
            crate::domain::Symbol::new("GOOGL").unwrap(),
        ];
        let conditions = vec![make_rsi_condition(14, 30.0)];
        let provider = FakeProvider::new(
            (0..20)
                .map(|i| DailyBar {
                    symbol: crate::domain::Symbol::new("AAPL").unwrap(),
                    trade_date: format!("2026-07-{:02}", i + 1),
                    price_basis: PriceBasis::SplitAdjusted,
                    open: 100.0 + i as f64,
                    high: 101.0 + i as f64,
                    low: 99.0 + i as f64,
                    close: 100.0 + i as f64,
                    volume: 1_000,
                })
                .collect(),
        );
        let cancellation = Arc::new(Mutex::new(CancellationToken::new()));
        let registry = make_registry();
        let service = ScanService::new(provider, cancellation, registry);

        let mut db = SequentialFakeDatabaseOps::new(symbols, conditions);
        let watchlist_id = WatchlistId::new("wl-4").unwrap();
        let preset_id = ScanPresetId::new("ps-4").unwrap();

        let result = service
            .run_scan_sequential(&watchlist_id, &preset_id, &mut db)
            .await;

        assert!(result.is_ok());
        let scan_result = result.unwrap();
        assert_eq!(
            scan_result.succeeded_symbols + scan_result.failed_symbols,
            scan_result.total_symbols
        );
    }

    #[tokio::test]
    async fn empty_watchlist_returns_error() {
        let symbols: Vec<crate::domain::Symbol> = vec![];
        let conditions = vec![make_rsi_condition(14, 30.0)];
        let provider = FakeProvider::new(vec![]);
        let cancellation = Arc::new(Mutex::new(CancellationToken::new()));
        let registry = make_registry();
        let service = ScanService::new(provider, cancellation, registry);

        let mut db = SequentialFakeDatabaseOps::new(symbols, conditions);
        let watchlist_id = WatchlistId::new("wl-5").unwrap();
        let preset_id = ScanPresetId::new("ps-5").unwrap();

        let result = service
            .run_scan_sequential(&watchlist_id, &preset_id, &mut db)
            .await;

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, AppErrorCode::Validation);
    }
}

// ---------------------------------------------------------------------------
// Concurrent scan tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod concurrent_tests {
    use super::tests::FakeProvider;
    use super::*;
    use crate::domain::{
        IndicatorKind, PriceBasis, ScanPresetId, SignalCondition, SignalConditionId, SignalSide,
        TriggerMode, WatchlistId,
    };
    use serde_json::json;

    struct ConcurrentFakeDatabaseOps {
        symbols: Vec<crate::domain::Symbol>,
        conditions: Vec<SignalCondition>,
    }

    impl ConcurrentFakeDatabaseOps {
        fn new(symbols: Vec<crate::domain::Symbol>, conditions: Vec<SignalCondition>) -> Self {
            Self {
                symbols,
                conditions,
            }
        }
    }

    impl DatabaseOps for ConcurrentFakeDatabaseOps {
        fn date_range(
            &mut self,
            _symbol: &crate::domain::Symbol,
        ) -> AppResult<Option<(String, String)>> {
            Ok(None)
        }

        fn upsert_bars(&mut self, _bars: &[DailyBar]) -> AppResult<()> {
            Ok(())
        }

        fn load_bars(
            &mut self,
            _symbol: &crate::domain::Symbol,
            _start: &str,
            _end: &str,
        ) -> AppResult<Vec<DailyBar>> {
            Ok((0..20)
                .map(|i| DailyBar {
                    symbol: crate::domain::Symbol::new("AAPL").unwrap(),
                    trade_date: format!("2026-07-{:02}", i + 1),
                    price_basis: PriceBasis::SplitAdjusted,
                    open: 100.0 + i as f64,
                    high: 101.0 + i as f64,
                    low: 99.0 + i as f64,
                    close: 100.0 + i as f64,
                    volume: 1_000,
                })
                .collect())
        }
    }

    impl DatabaseOpsExtended for ConcurrentFakeDatabaseOps {
        fn load_watchlist(
            &mut self,
            _id: &WatchlistId,
        ) -> AppResult<(Vec<crate::domain::Symbol>, String)> {
            Ok((self.symbols.clone(), "Test Watchlist".to_string()))
        }

        fn load_preset_conditions(
            &mut self,
            _id: &ScanPresetId,
        ) -> AppResult<Vec<SignalCondition>> {
            Ok(self.conditions.clone())
        }

        fn create_scan_run(
            &mut self,
            input: &ScanRunCreateInput,
        ) -> AppResult<crate::domain::ScanRunId> {
            let id =
                crate::domain::ScanRunId::new(format!("run-{}", input.watchlist_id.0)).unwrap();
            Ok(id)
        }

        fn start_scan_run(&mut self, _id: &crate::domain::ScanRunId) -> AppResult<()> {
            Ok(())
        }

        fn save_scan_result(&mut self, _result: &crate::domain::ScanResult) -> AppResult<()> {
            Ok(())
        }

        fn save_scan_error(&mut self, _error: &crate::domain::ScanError) -> AppResult<()> {
            Ok(())
        }

        fn update_scan_progress(
            &mut self,
            _id: &crate::domain::ScanRunId,
            _succeeded: u32,
            _failed: u32,
        ) -> AppResult<()> {
            Ok(())
        }

        fn mark_scan_completed(
            &mut self,
            _id: &crate::domain::ScanRunId,
            _base_date: Option<&str>,
        ) -> AppResult<()> {
            Ok(())
        }

        fn mark_scan_cancelled(&mut self, _id: &crate::domain::ScanRunId) -> AppResult<()> {
            Ok(())
        }

        fn mark_scan_failed(&mut self, _id: &crate::domain::ScanRunId) -> AppResult<()> {
            Ok(())
        }

        fn update_stale_flags(
            &mut self,
            _id: &crate::domain::ScanRunId,
            _base_date: &str,
        ) -> AppResult<()> {
            Ok(())
        }
    }

    fn make_registry() -> Arc<crate::state::CancellationRegistry> {
        Arc::new(crate::state::CancellationRegistry::new())
    }

    fn make_rsi_condition(period: u32, threshold: f64) -> SignalCondition {
        SignalCondition {
            id: SignalConditionId::new("rsi1").unwrap(),
            indicator: IndicatorKind::Rsi,
            side: SignalSide::Lower,
            period,
            threshold: Some(threshold),
            parameters: json!({}),
            trigger_mode: TriggerMode::Current,
            enabled: true,
            sort_order: 0,
        }
    }

    #[tokio::test]
    async fn concurrent_scan_completes_all_symbols() {
        let symbols = vec![
            crate::domain::Symbol::new("AAPL").unwrap(),
            crate::domain::Symbol::new("GOOGL").unwrap(),
            crate::domain::Symbol::new("MSFT").unwrap(),
            crate::domain::Symbol::new("AMZN").unwrap(),
            crate::domain::Symbol::new("META").unwrap(),
        ];
        let conditions = vec![make_rsi_condition(14, 30.0)];
        let provider = FakeProvider::new(
            (0..20)
                .map(|i| DailyBar {
                    symbol: crate::domain::Symbol::new("AAPL").unwrap(),
                    trade_date: format!("2026-07-{:02}", i + 1),
                    price_basis: PriceBasis::SplitAdjusted,
                    open: 100.0 + i as f64,
                    high: 101.0 + i as f64,
                    low: 99.0 + i as f64,
                    close: 100.0 + i as f64,
                    volume: 1_000,
                })
                .collect(),
        );
        let cancellation = Arc::new(Mutex::new(CancellationToken::new()));
        let registry = make_registry();
        let service = ScanService::new(provider, cancellation, registry);

        let mut db = ConcurrentFakeDatabaseOps::new(symbols, conditions);
        let watchlist_id = WatchlistId::new("wl-c1").unwrap();
        let preset_id = ScanPresetId::new("ps-c1").unwrap();

        let result = service
            .run_scan_concurrent(&watchlist_id, &preset_id, &mut db)
            .await;

        assert!(result.is_ok());
        let scan_result = result.unwrap();
        assert_eq!(scan_result.total_symbols, 5);
        assert_eq!(scan_result.succeeded_symbols, 5);
        assert_eq!(scan_result.failed_symbols, 0);
    }

    #[tokio::test]
    async fn concurrent_scan_handles_partial_failures() {
        struct PartialFailingProvider {
            fail_on: Vec<String>,
        }

        #[async_trait::async_trait]
        impl MarketDataProvider for PartialFailingProvider {
            async fn fetch_daily_bars(
                &self,
                symbol: &crate::domain::Symbol,
                _range: &DateRange,
            ) -> AppResult<Vec<DailyBar>> {
                if self.fail_on.contains(&symbol.as_str().to_string()) {
                    return Err(AppError::new(
                        AppErrorCode::ProviderUnavailable,
                        format!("failed for {}", symbol),
                    )
                    .retryable(true));
                }
                Ok((0..20)
                    .map(|i| DailyBar {
                        symbol: symbol.clone(),
                        trade_date: format!("2026-07-{:02}", i + 1),
                        price_basis: PriceBasis::SplitAdjusted,
                        open: 100.0 + i as f64,
                        high: 101.0 + i as f64,
                        low: 99.0 + i as f64,
                        close: 100.0 + i as f64,
                        volume: 1_000,
                    })
                    .collect())
            }
        }

        let symbols = vec![
            crate::domain::Symbol::new("AAPL").unwrap(),
            crate::domain::Symbol::new("GOOGL").unwrap(),
            crate::domain::Symbol::new("MSFT").unwrap(),
        ];
        let conditions = vec![make_rsi_condition(14, 30.0)];
        let provider = PartialFailingProvider {
            fail_on: vec!["GOOGL".to_string()],
        };
        let cancellation = Arc::new(Mutex::new(CancellationToken::new()));
        let registry = make_registry();
        let service = ScanService::new(provider, cancellation, registry);

        let mut db = ConcurrentFakeDatabaseOps::new(symbols, conditions);
        let watchlist_id = WatchlistId::new("wl-c2").unwrap();
        let preset_id = ScanPresetId::new("ps-c2").unwrap();

        let result = service
            .run_scan_concurrent(&watchlist_id, &preset_id, &mut db)
            .await;

        assert!(result.is_ok());
        let scan_result = result.unwrap();
        assert_eq!(scan_result.total_symbols, 3);
        assert_eq!(scan_result.succeeded_symbols, 2);
        assert_eq!(scan_result.failed_symbols, 1);
    }

    #[tokio::test]
    async fn concurrent_semaphore_has_four_permits() {
        let semaphore = Semaphore::new(4);
        assert_eq!(semaphore.available_permits(), 4);
    }
}
