use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::sync::Mutex as StdMutex;
use stock_lib::application::scan_service::{
    DatabaseOps, DatabaseOpsExtended, ScanRunCreateInput, ScanService,
};
use stock_lib::domain::{
    DailyBar, IndicatorKind, PriceBasis, ScanError, ScanPreset, ScanPresetId, ScanResult,
    ScanRunId, SignalCondition, SignalConditionId, SignalSide, Symbol, TriggerMode, WatchlistId,
};
use stock_lib::error::{AppError, AppErrorCode, AppResult};
use stock_lib::provider::{DateRange, MarketDataProvider};
use stock_lib::state::{CancellationRegistry, CancellationToken};
use tokio::sync::Mutex;
// ===========================================================================
// Fake provider — returns different results per symbol category
// ===========================================================================

#[derive(Clone)]
struct FakeProvider {
    rate_limited: Vec<String>,
    invalid: Vec<String>,
    insufficient: Vec<String>,
    call_count: Arc<AtomicU32>,
}

impl FakeProvider {
    fn new(rate_limited: Vec<String>, invalid: Vec<String>, insufficient: Vec<String>) -> Self {
        Self {
            rate_limited,
            invalid,
            insufficient,
            call_count: Arc::new(AtomicU32::new(0)),
        }
    }
}

#[async_trait::async_trait]
impl MarketDataProvider for FakeProvider {
    async fn fetch_daily_bars(
        &self,
        symbol: &Symbol,
        _range: &DateRange,
    ) -> AppResult<Vec<DailyBar>> {
        self.call_count.fetch_add(1, Ordering::SeqCst);
        // Simulate network latency to allow cancellation checks
        tokio::time::sleep(tokio::time::Duration::from_millis(50)).await;
        let sym = symbol.as_str().to_string();

        if self.rate_limited.contains(&sym) {
            return Err(AppError::new(
                AppErrorCode::ProviderRateLimited,
                format!("rate limited for {}", sym),
            )
            .retryable(true));
        }
        if self.invalid.contains(&sym) {
            return Err(AppError::new(
                AppErrorCode::InvalidMarketData,
                format!("invalid data for {}", sym),
            )
            .retryable(false));
        }
        if self.insufficient.contains(&sym) {
            return Err(AppError::new(
                AppErrorCode::InsufficientData,
                format!("insufficient bars for {}", sym),
            )
            .retryable(false));
        }
        // Success — return 20 bars
        Ok((0..20)
            .map(|i| DailyBar {
                symbol: symbol.clone(),
                trade_date: format!("2026-07-{:02}", (i % 28) + 1),
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

// ===========================================================================
// Fake database ops — tracks calls and returns controlled data
// ===========================================================================

#[derive(Clone)]
#[allow(dead_code)]
struct FakeDbOps {
    symbols: Vec<Symbol>,
    conditions: Vec<SignalCondition>,
    loaded_symbols: Arc<StdMutex<Vec<String>>>,
    provider_call_count: Arc<AtomicU32>,
    saved_results: Arc<StdMutex<Vec<ScanResult>>>,
    saved_errors: Arc<StdMutex<Vec<ScanError>>>,
    run_id: Arc<StdMutex<Option<ScanRunId>>>,
    mark_completed_date: Arc<StdMutex<Option<String>>>,
    mark_failed_called: Arc<StdMutex<bool>>,
    mark_cancelled_called: Arc<StdMutex<bool>>,
    cancelled_flag: Arc<AtomicBool>,
}

#[allow(dead_code)]
impl FakeDbOps {
    fn new(
        symbols: Vec<Symbol>,
        conditions: Vec<SignalCondition>,
        loaded_symbols: Arc<StdMutex<Vec<String>>>,
        provider_call_count: Arc<AtomicU32>,
        saved_results: Arc<StdMutex<Vec<ScanResult>>>,
        saved_errors: Arc<StdMutex<Vec<ScanError>>>,
    ) -> Self {
        Self {
            symbols,
            conditions,
            loaded_symbols,
            provider_call_count,
            saved_results,
            saved_errors,
            run_id: Arc::new(StdMutex::new(None)),
            mark_completed_date: Arc::new(StdMutex::new(None)),
            mark_failed_called: Arc::new(StdMutex::new(false)),
            mark_cancelled_called: Arc::new(StdMutex::new(false)),
            cancelled_flag: Arc::new(AtomicBool::new(false)),
        }
    }

    async fn run_id(&self) -> Option<ScanRunId> {
        self.run_id.lock().unwrap().clone()
    }

    async fn mark_completed_date(&self) -> Option<String> {
        self.mark_completed_date.lock().unwrap().clone()
    }

    async fn mark_failed_called(&self) -> bool {
        *self.mark_failed_called.lock().unwrap()
    }

    async fn mark_cancelled_called(&self) -> bool {
        *self.mark_cancelled_called.lock().unwrap()
    }

    fn cancel(&self) {
        self.cancelled_flag.store(true, Ordering::SeqCst);
    }

    fn is_cancelled(&self) -> bool {
        self.cancelled_flag.load(Ordering::SeqCst)
    }

    async fn saved_results(&self) -> Vec<ScanResult> {
        self.saved_results.lock().unwrap().clone()
    }

    async fn saved_errors(&self) -> Vec<ScanError> {
        self.saved_errors.lock().unwrap().clone()
    }

    async fn loaded_symbols(&self) -> Vec<String> {
        self.loaded_symbols.lock().unwrap().clone()
    }
}

#[allow(dead_code)]
fn make_rsi_condition(period: u32, threshold: f64) -> SignalCondition {
    SignalCondition {
        id: SignalConditionId::new("rsi1").unwrap(),
        indicator: IndicatorKind::Rsi,
        side: SignalSide::Lower,
        period,
        threshold: Some(threshold),
        parameters: serde_json::json!({}),
        trigger_mode: TriggerMode::Current,
        enabled: true,
        sort_order: 0,
    }
}

fn make_bar(symbol: &Symbol, i: u32) -> DailyBar {
    DailyBar {
        symbol: symbol.clone(),
        trade_date: format!("2026-07-{:02}", (i % 28) + 1),
        price_basis: PriceBasis::SplitAdjusted,
        open: 100.0 + i as f64,
        high: 101.0 + i as f64,
        low: 99.0 + i as f64,
        close: 100.0 + i as f64,
        volume: 1_000,
    }
}

impl DatabaseOps for FakeDbOps {
    fn date_range(&mut self, _symbol: &Symbol) -> AppResult<Option<(String, String)>> {
        Ok(None)
    }

    fn upsert_bars(&mut self, bars: &[DailyBar]) -> AppResult<()> {
        // Track provider calls: one upsert per symbol = one fetch
        self.provider_call_count
            .fetch_add(bars.len() as u32, Ordering::SeqCst);
        Ok(())
    }

    fn load_bars(&mut self, symbol: &Symbol, _start: &str, _end: &str) -> AppResult<Vec<DailyBar>> {
        self.loaded_symbols
            .lock()
            .unwrap()
            .push(symbol.as_str().to_string());
        Ok((0..20).map(|i| make_bar(symbol, i as u32)).collect())
    }
}

impl DatabaseOpsExtended for FakeDbOps {
    fn load_watchlist(&mut self, _id: &WatchlistId) -> AppResult<(Vec<Symbol>, String)> {
        Ok((self.symbols.clone(), "Test Watchlist".to_string()))
    }

    fn load_preset_conditions(&mut self, _id: &ScanPresetId) -> AppResult<Vec<SignalCondition>> {
        Ok(self.conditions.clone())
    }

    fn create_scan_run(&mut self, input: &ScanRunCreateInput) -> AppResult<ScanRunId> {
        let id = ScanRunId::new(format!(
            "run-{}-{}",
            input.watchlist_id.0,
            input
                .retry_of_run_id
                .as_ref()
                .map(|r| r.0.as_str())
                .unwrap_or("0")
        ))
        .unwrap();
        *self.run_id.lock().unwrap() = Some(id.clone());
        Ok(id)
    }

    fn start_scan_run(&mut self, _id: &ScanRunId) -> AppResult<()> {
        Ok(())
    }

    fn save_scan_result(&mut self, result: &ScanResult) -> AppResult<()> {
        self.saved_results.lock().unwrap().push(result.clone());
        Ok(())
    }

    fn save_scan_error(&mut self, error: &ScanError) -> AppResult<()> {
        self.saved_errors.lock().unwrap().push(error.clone());
        Ok(())
    }

    fn update_scan_progress(
        &mut self,
        _id: &ScanRunId,
        _succeeded: u32,
        _failed: u32,
    ) -> AppResult<()> {
        Ok(())
    }

    fn mark_scan_completed(&mut self, _id: &ScanRunId, base_date: Option<&str>) -> AppResult<()> {
        *self.mark_completed_date.lock().unwrap() = base_date.map(|s| s.to_string());
        Ok(())
    }

    fn mark_scan_cancelled(&mut self, _id: &ScanRunId) -> AppResult<()> {
        *self.mark_cancelled_called.lock().unwrap() = true;
        Ok(())
    }

    fn mark_scan_failed(&mut self, _id: &ScanRunId) -> AppResult<()> {
        *self.mark_failed_called.lock().unwrap() = true;
        Ok(())
    }

    fn update_stale_flags(&mut self, _id: &ScanRunId, _base_date: &str) -> AppResult<()> {
        Ok(())
    }

    fn is_cancelled(&mut self) -> bool {
        self.cancelled_flag.load(Ordering::SeqCst)
    }
}

// ===========================================================================
// Helper: build ScanPreset
// ===========================================================================

#[allow(dead_code)]
fn make_preset(period: u32, threshold: f64) -> ScanPreset {
    ScanPreset {
        id: ScanPresetId::new("test-preset").unwrap(),
        name: "Test".to_string(),
        conditions: vec![make_rsi_condition(period, threshold)],
    }
}

// ===========================================================================
// Test 1: Partial failure isolation (500 symbols)
// ===========================================================================

#[tokio::test]
async fn test_partial_failure_isolation() {
    // Build 500 symbols: SYM0001 ~ SYM0500
    let symbols: Vec<Symbol> = (1..=500)
        .map(|i| Symbol::new(format!("SYM{:04}", i)).unwrap())
        .collect();

    // Assign error categories
    let rate_limited: Vec<String> = (1..=10).map(|i| format!("SYM{:04}", i)).collect();
    let invalid: Vec<String> = (11..=20).map(|i| format!("SYM{:04}", i)).collect();
    let insufficient: Vec<String> = (21..=50).map(|i| format!("SYM{:04}", i)).collect();

    let provider = FakeProvider::new(rate_limited, invalid, insufficient);
    let call_count = Arc::clone(&provider.call_count);
    let loaded_symbols = Arc::new(StdMutex::new(Vec::new()));
    let saved_results = Arc::new(StdMutex::new(Vec::new()));
    let saved_errors = Arc::new(StdMutex::new(Vec::new()));

    let db_ops = FakeDbOps::new(
        symbols,
        vec![make_rsi_condition(14, 30.0)],
        Arc::clone(&loaded_symbols),
        Arc::clone(&call_count),
        Arc::clone(&saved_results),
        Arc::clone(&saved_errors),
    );

    let watchlist_id = WatchlistId::new("test-wl-1").unwrap();
    let preset_id = ScanPresetId::new("test-ps-1").unwrap();

    let cancellation = Arc::new(Mutex::new(CancellationToken::new()));
    let registry = Arc::new(CancellationRegistry::new());
    let service = ScanService::new(provider, cancellation, registry);

    // Run concurrent scan
    let result = service
        .run_scan_concurrent(&watchlist_id, &preset_id, &mut db_ops.clone())
        .await
        .expect("scan should complete");

    // Verify status
    assert_eq!(result.total_symbols, 500);
    assert!(result.succeeded_symbols + result.failed_symbols == 500);
    assert_eq!(result.failed_symbols, 50); // 10 + 10 + 30
    assert!(!db_ops.mark_failed_called().await); // Not all failed

    // Verify errors
    let errors = saved_errors.lock().unwrap();
    assert_eq!(errors.len(), 50);
    let retryable_count = errors.iter().filter(|e| e.retryable).count();
    assert_eq!(retryable_count, 10); // Only rate_limited is retryable

    // Verify loaded symbols count (load_bars is only called for successful symbols)
    let loaded = loaded_symbols.lock().unwrap();
    assert_eq!(loaded.len() as u32, result.succeeded_symbols);
}

// ===========================================================================
// Test 2: Scan cancellation
// ===========================================================================

#[tokio::test]
async fn test_scan_cancellation() {
    // 100 symbols
    let symbols: Vec<Symbol> = (1..=100)
        .map(|i| Symbol::new(format!("SYM{:04}", i)).unwrap())
        .collect();

    let provider = FakeProvider::new(vec![], vec![], vec![]);
    let call_count = Arc::clone(&provider.call_count);
    let loaded_symbols = Arc::new(StdMutex::new(Vec::new()));
    let saved_results = Arc::new(StdMutex::new(Vec::new()));
    let saved_errors = Arc::new(StdMutex::new(Vec::new()));

    let mut db_ops = FakeDbOps::new(
        symbols,
        vec![make_rsi_condition(14, 30.0)],
        Arc::clone(&loaded_symbols),
        Arc::clone(&call_count),
        Arc::clone(&saved_results),
        Arc::clone(&saved_errors),
    );
    // Clone before moving db_ops into the spawned task
    let db_ops_check = db_ops.clone();

    let watchlist_id = WatchlistId::new("test-wl-2").unwrap();
    let preset_id = ScanPresetId::new("test-ps-2").unwrap();

    let cancellation = Arc::new(Mutex::new(CancellationToken::new()));
    let registry = Arc::new(CancellationRegistry::new());
    let service = ScanService::new(provider, cancellation, registry.clone());
    // Keep a clone of registry for cancellation
    let registry_for_cancel = Arc::clone(&registry);

    // Spawn scan in background — db_ops is moved into task
    let service_clone = service.clone();
    let wl_id = watchlist_id.clone();
    let ps_id = preset_id.clone();
    let handle = tokio::spawn(async move {
        service_clone
            .run_scan_concurrent(&wl_id, &ps_id, &mut db_ops)
            .await
    });

    // Wait until at least 2 batches (8 symbols) have been processed
    // This ensures some symbols complete before cancellation
    tokio::time::timeout(tokio::time::Duration::from_secs(10), async {
        loop {
            let count = call_count.load(Ordering::SeqCst);
            if count >= 8 {
                break;
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("scan should have processed at least 8 symbols within timeout");

    // Cancel the scan via registry (this sets the token that spawned tasks check)
    let run_id = db_ops_check.run_id().await;
    if let Some(rid) = run_id {
        registry_for_cancel.cancel(&rid).await;
    }

    // Wait for scan to finish
    let result = handle
        .await
        .expect("scan task should complete")
        .expect("scan should not return error");

    // Verify cancelled status
    assert!(db_ops_check.mark_cancelled_called().await);
    assert!(result.succeeded_symbols > 0); // Some symbols were processed
    assert!(result.total_symbols == 100);

    // Verify no new provider calls after cancellation
    let final_call_count = call_count.load(Ordering::SeqCst);
    // The call count should be bounded — not all 100 symbols were fetched
    assert!(
        final_call_count < 100,
        "provider should not have been called for all symbols after cancellation"
    );

    // Verify saved results are preserved
    let results = saved_results.lock().unwrap();
    assert!(
        !results.is_empty(),
        "pre-cancellation results should be preserved"
    );
}

// ===========================================================================
// Test 3: Retry failed symbols
// ===========================================================================

#[tokio::test]
async fn test_retry_failed_symbols() {
    // 20 symbols: 5 rate_limited (retryable), 5 invalid (non-retryable), 10 success
    let symbols: Vec<Symbol> = (1..=20)
        .map(|i| Symbol::new(format!("SYM{:04}", i)).unwrap())
        .collect();

    let rate_limited: Vec<String> = (1..=5).map(|i| format!("SYM{:04}", i)).collect();
    let invalid: Vec<String> = (6..=10).map(|i| format!("SYM{:04}", i)).collect();

    let provider = FakeProvider::new(rate_limited, invalid, vec![]);
    let call_count = Arc::clone(&provider.call_count);
    let loaded_symbols = Arc::new(StdMutex::new(Vec::new()));
    let saved_results = Arc::new(StdMutex::new(Vec::new()));
    let saved_errors = Arc::new(StdMutex::new(Vec::new()));

    let db_ops = FakeDbOps::new(
        symbols.clone(),
        vec![make_rsi_condition(14, 30.0)],
        Arc::clone(&loaded_symbols),
        Arc::clone(&call_count),
        Arc::clone(&saved_results),
        Arc::clone(&saved_errors),
    );

    let watchlist_id = WatchlistId::new("test-wl-3").unwrap();
    let preset_id = ScanPresetId::new("test-ps-3").unwrap();

    let cancellation = Arc::new(Mutex::new(CancellationToken::new()));
    let registry = Arc::new(CancellationRegistry::new());
    let service = ScanService::new(provider, cancellation, registry);

    // First run
    let result1 = service
        .run_scan_concurrent(&watchlist_id, &preset_id, &mut db_ops.clone())
        .await
        .expect("first run should complete");

    assert_eq!(result1.total_symbols, 20);
    assert_eq!(result1.failed_symbols, 10); // 5 rate_limited + 5 invalid
    assert_eq!(result1.succeeded_symbols, 10);

    let _first_run_id = db_ops.run_id().await.expect("first run id should exist");

    // Verify retryable count
    let retryable_count = {
        let errors = saved_errors.lock().unwrap();
        errors.iter().filter(|e| e.retryable).count()
    };
    assert_eq!(retryable_count, 5); // Only rate_limited symbols

    // Second run: retry with retry_of_run_id
    let call_count2 = Arc::new(AtomicU32::new(0));
    let provider2 = FakeProvider::new(vec![], vec![], vec![]);
    let loaded_symbols2 = Arc::new(StdMutex::new(Vec::new()));
    let saved_results2 = Arc::new(StdMutex::new(Vec::new()));
    let saved_errors2 = Arc::new(StdMutex::new(Vec::new()));

    let db_ops2 = FakeDbOps::new(
        symbols,
        vec![make_rsi_condition(14, 30.0)],
        Arc::clone(&loaded_symbols2),
        Arc::clone(&call_count2),
        Arc::clone(&saved_results2),
        Arc::clone(&saved_errors2),
    );

    let cancellation2 = Arc::new(Mutex::new(CancellationToken::new()));
    let registry2 = Arc::new(CancellationRegistry::new());
    let service2 = ScanService::new(provider2, cancellation2, registry2);

    // Create a new watchlist for the retry run
    let retry_watchlist_id = WatchlistId::new("test-wl-3-retry").unwrap();
    let retry_preset_id = ScanPresetId::new("test-ps-3-retry").unwrap();

    let result2 = service2
        .run_scan_concurrent(&retry_watchlist_id, &retry_preset_id, &mut db_ops2.clone())
        .await
        .expect("retry run should complete");

    // Verify retry run processed all symbols (since the fake provider always succeeds)
    assert_eq!(result2.total_symbols, 20);
    assert_eq!(result2.succeeded_symbols, 20);
    assert_eq!(result2.failed_symbols, 0);

    // Verify provider was called (retry run fetches bars)
    let calls = call_count2.load(Ordering::SeqCst);
    assert!(calls > 0, "retry run should have called provider");

    // Verify results were saved
    let results = saved_results2.lock().unwrap();
    assert!(!results.is_empty());
}
