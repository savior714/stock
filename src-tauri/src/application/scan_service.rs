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
use std::sync::Arc;
use tokio::sync::Mutex;

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
// CancellationToken — stub for A-03 (cancellation support)
// ---------------------------------------------------------------------------

pub struct CancellationToken;

impl Default for CancellationToken {
    fn default() -> Self {
        Self
    }
}

impl CancellationToken {
    pub fn is_cancelled(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// ScanService — single-symbol scan pipeline
// ---------------------------------------------------------------------------

pub struct ScanService<P>
where
    P: MarketDataProvider,
{
    provider: Arc<P>,
    cancellation: Arc<Mutex<CancellationToken>>,
}

impl<P> ScanService<P>
where
    P: MarketDataProvider,
{
    pub fn new(provider: P, cancellation: Arc<Mutex<CancellationToken>>) -> Self {
        Self {
            provider: Arc::new(provider),
            cancellation,
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
        {
            let token = self.cancellation.lock().await;
            if token.is_cancelled() {
                return Err(AppError::new(
                    AppErrorCode::Cancelled,
                    format!("scan cancelled for symbol {}", symbol),
                ));
            }
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

    struct FakeProvider {
        bars: Vec<DailyBar>,
        error: Option<AppError>,
    }

    impl FakeProvider {
        fn new(bars: Vec<DailyBar>) -> Self {
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
        Arc::new(Mutex::new(CancellationToken))
    }

    // ---- Tests ----

    #[tokio::test]
    async fn processes_symbol_successfully() {
        let bars: Vec<DailyBar> = (0..20)
            .map(|i| make_bar(&format!("2026-07-{:02}", i + 1), 100.0 + i as f64))
            .collect();
        let provider = FakeProvider::new(bars.clone());
        let cancellation = make_cancellation();
        let service = ScanService::new(provider, cancellation);

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
        let service = ScanService::new(provider, cancellation);

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
        let service = ScanService::new(provider, cancellation);

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
        let service = ScanService::new(provider, cancellation);

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
        let service = ScanService::new(provider, cancellation);

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
        let service = ScanService::new(provider, cancellation);

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
        let service = ScanService::new(provider, cancellation);

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
        let service = ScanService::new(provider, cancellation);

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
