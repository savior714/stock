use super::*;
use crate::db::Database;
use crate::domain::SignalConditionId;

fn make_database() -> Database {
    let database = Database::open_in_memory().expect("database must initialize");
    insert_fk_records(&database);
    database
}

fn insert_fk_records(database: &Database) {
    let conn = database.connection();
    conn.execute(
        "INSERT INTO watchlists (id, name) VALUES ('wl-0000000000000001', 'Test Watchlist')",
        [],
    )
    .expect("watchlist must insert");
    conn.execute(
        "INSERT INTO scan_presets (id, name, trigger_mode) VALUES ('ps-0000000000000001', 'Test Preset', 'current')",
        [],
    )
    .expect("preset must insert");
    for symbol in ["AAPL", "MSFT", "GOOGL", "ZM"] {
        conn.execute(
            &format!("INSERT OR IGNORE INTO instruments (symbol, provider_symbol, asset_type) VALUES ('{symbol}', '{symbol}', 'stock')"),
            [],
        )
        .ok();
    }
}

fn insert_scan_run(database: &mut Database) -> ScanRunId {
    use crate::repository::scan_run::{ScanRunCreate, ScanRunRepository};
    let mut repo = ScanRunRepository::new(database);
    let input = ScanRunCreate {
        watchlist_id: crate::domain::WatchlistId::new("wl-0000000000000001").expect("valid"),
        preset_id: crate::domain::ScanPresetId::new("ps-0000000000000001").expect("valid"),
        total_symbols: 3,
        preset_snapshot_json: "{}".to_string(),
        symbols_snapshot_json: "[]".to_string(),
        retry_of_run_id: None,
    };
    let summary = repo.create_pending(&input).expect("must create");
    summary.id
}

fn make_result(
    run_id: &ScanRunId,
    symbol: &str,
    price: f64,
    all_match: bool,
    any_match: bool,
) -> ScanResult {
    let matches = vec![SignalMatch {
        condition_id: SignalConditionId::new("cond-0000000000000001").expect("valid"),
        matched: all_match,
        newly_crossed: false,
    }];
    ScanResult {
        run_id: run_id.clone(),
        symbol: Symbol::new(symbol).expect("valid symbol"),
        trade_date: "2026-07-15".to_string(),
        current_price: price,
        indicators: IndicatorValues {
            rsi: Some(25.0),
            mfi: Some(20.0),
            bollinger_lower: Some(95.0),
            bollinger_middle: Some(100.0),
            bollinger_upper: Some(105.0),
        },
        matches,
        matched_condition_count: if all_match { 1 } else { 0 },
        all_conditions_matched: all_match,
        any_condition_matched: any_match,
        data_stale: false,
    }
}

#[test]
fn upserts_and_retrieves_result() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanResultRepository::new(&mut database);

    let result = make_result(&run_id, "AAPL", 150.0, true, true);
    repo.upsert(&result).expect("must upsert");

    let results = repo
        .get_by_run(&run_id, ResultMatchFilter::None)
        .expect("must list");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].symbol.as_str(), "AAPL");
    assert_eq!(results[0].current_price, 150.0);
}

#[test]
fn upsert_overwrites_existing() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanResultRepository::new(&mut database);

    let r1 = make_result(&run_id, "AAPL", 150.0, true, true);
    repo.upsert(&r1).expect("must upsert");

    let mut r2 = r1.clone();
    r2.current_price = 160.0;
    r2.indicators.rsi = Some(30.0);
    repo.upsert(&r2).expect("must overwrite");

    let results = repo
        .get_by_run(&run_id, ResultMatchFilter::None)
        .expect("must list");
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].current_price, 160.0);
    assert_eq!(results[0].indicators.rsi, Some(30.0));
}

#[test]
fn filters_by_and_match() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanResultRepository::new(&mut database);

    repo.upsert(&make_result(&run_id, "AAPL", 150.0, true, true))
        .expect("ok");
    repo.upsert(&make_result(&run_id, "MSFT", 300.0, false, true))
        .expect("ok");
    repo.upsert(&make_result(&run_id, "GOOGL", 120.0, false, false))
        .expect("ok");

    let and_results = repo
        .get_by_run(&run_id, ResultMatchFilter::And)
        .expect("ok");
    assert_eq!(and_results.len(), 1);
    assert_eq!(and_results[0].symbol.as_str(), "AAPL");
}

#[test]
fn filters_by_or_match() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanResultRepository::new(&mut database);

    repo.upsert(&make_result(&run_id, "AAPL", 150.0, true, true))
        .expect("ok");
    repo.upsert(&make_result(&run_id, "MSFT", 300.0, false, true))
        .expect("ok");
    repo.upsert(&make_result(&run_id, "GOOGL", 120.0, false, false))
        .expect("ok");

    let or_results = repo.get_by_run(&run_id, ResultMatchFilter::Or).expect("ok");
    assert_eq!(or_results.len(), 2);
    assert_eq!(or_results[0].symbol.as_str(), "AAPL");
    assert_eq!(or_results[1].symbol.as_str(), "MSFT");
}

#[test]
fn no_filter_returns_all() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanResultRepository::new(&mut database);

    repo.upsert(&make_result(&run_id, "AAPL", 150.0, true, true))
        .expect("ok");
    repo.upsert(&make_result(&run_id, "MSFT", 300.0, false, true))
        .expect("ok");
    repo.upsert(&make_result(&run_id, "GOOGL", 120.0, false, false))
        .expect("ok");

    let all = repo
        .get_by_run(&run_id, ResultMatchFilter::None)
        .expect("ok");
    assert_eq!(all.len(), 3);
}

#[test]
fn results_ordered_by_symbol() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanResultRepository::new(&mut database);

    repo.upsert(&make_result(&run_id, "ZM", 50.0, false, false))
        .expect("ok");
    repo.upsert(&make_result(&run_id, "AAPL", 150.0, false, false))
        .expect("ok");
    repo.upsert(&make_result(&run_id, "MSFT", 300.0, false, false))
        .expect("ok");

    let results = repo
        .get_by_run(&run_id, ResultMatchFilter::None)
        .expect("ok");
    assert_eq!(results[0].symbol.as_str(), "AAPL");
    assert_eq!(results[1].symbol.as_str(), "MSFT");
    assert_eq!(results[2].symbol.as_str(), "ZM");
}

#[test]
fn updates_stale_flags() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanResultRepository::new(&mut database);

    let mut stale_result = make_result(&run_id, "AAPL", 150.0, true, true);
    stale_result.trade_date = "2026-07-14".to_string();
    repo.upsert(&stale_result).expect("ok");

    let mut fresh_result = make_result(&run_id, "MSFT", 300.0, false, true);
    fresh_result.trade_date = "2026-07-15".to_string();
    repo.upsert(&fresh_result).expect("ok");

    repo.update_stale_flags(&run_id, "2026-07-15").expect("ok");

    let results = repo
        .get_by_run(&run_id, ResultMatchFilter::None)
        .expect("ok");
    let aapl = results
        .iter()
        .find(|r| r.symbol.as_str() == "AAPL")
        .unwrap();
    let msft = results
        .iter()
        .find(|r| r.symbol.as_str() == "MSFT")
        .unwrap();

    assert!(aapl.data_stale, "AAPL should be stale (older date)");
    assert!(!msft.data_stale, "MSFT should not be stale (base date)");
}

#[test]
fn preserves_null_indicators() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanResultRepository::new(&mut database);

    let result = ScanResult {
        run_id: run_id.clone(),
        symbol: Symbol::new("AAPL").expect("valid"),
        trade_date: "2026-07-15".to_string(),
        current_price: 150.0,
        indicators: IndicatorValues {
            rsi: None,
            mfi: None,
            bollinger_lower: None,
            bollinger_middle: None,
            bollinger_upper: None,
        },
        matches: vec![],
        matched_condition_count: 0,
        all_conditions_matched: false,
        any_condition_matched: false,
        data_stale: false,
    };

    repo.upsert(&result).expect("ok");
    let retrieved = repo
        .get_by_run(&run_id, ResultMatchFilter::None)
        .expect("ok");
    assert!(retrieved[0].indicators.rsi.is_none());
    assert!(retrieved[0].indicators.mfi.is_none());
}

#[test]
fn encodes_and_decodes_signal_matches() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanResultRepository::new(&mut database);

    let result = ScanResult {
        run_id: run_id.clone(),
        symbol: Symbol::new("AAPL").expect("valid"),
        trade_date: "2026-07-15".to_string(),
        current_price: 150.0,
        indicators: IndicatorValues {
            rsi: Some(25.0),
            mfi: None,
            bollinger_lower: None,
            bollinger_middle: None,
            bollinger_upper: None,
        },
        matches: vec![
            SignalMatch {
                condition_id: SignalConditionId::new("c1").expect("valid"),
                matched: true,
                newly_crossed: true,
            },
            SignalMatch {
                condition_id: SignalConditionId::new("c2").expect("valid"),
                matched: false,
                newly_crossed: false,
            },
        ],
        matched_condition_count: 1,
        all_conditions_matched: false,
        any_condition_matched: true,
        data_stale: false,
    };

    repo.upsert(&result).expect("ok");
    let retrieved = repo
        .get_by_run(&run_id, ResultMatchFilter::None)
        .expect("ok");
    assert_eq!(retrieved[0].matches.len(), 2);
    assert!(retrieved[0].matches[0].matched);
    assert!(retrieved[0].matches[0].newly_crossed);
    assert!(!retrieved[0].matches[1].matched);
}

#[test]
fn empty_run_returns_no_results() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let repo = ScanResultRepository::new(&mut database);

    let results = repo
        .get_by_run(&run_id, ResultMatchFilter::None)
        .expect("ok");
    assert!(results.is_empty());
}

#[test]
fn preserves_matched_condition_count_zero() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanResultRepository::new(&mut database);

    let result = make_result(&run_id, "AAPL", 150.0, false, false);
    assert_eq!(result.matched_condition_count, 0);
    repo.upsert(&result).expect("ok");

    let retrieved = repo
        .get_by_run(&run_id, ResultMatchFilter::None)
        .expect("ok");
    assert_eq!(retrieved[0].matched_condition_count, 0);
}

#[test]
fn preserves_matched_condition_count_positive() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanResultRepository::new(&mut database);

    let result = make_result(&run_id, "AAPL", 150.0, true, true);
    assert_eq!(result.matched_condition_count, 1);
    repo.upsert(&result).expect("ok");

    let retrieved = repo
        .get_by_run(&run_id, ResultMatchFilter::None)
        .expect("ok");
    assert_eq!(retrieved[0].matched_condition_count, 1);
}

#[test]
fn preserves_count_across_multiple_results() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanResultRepository::new(&mut database);

    repo.upsert(&make_result(&run_id, "AAPL", 150.0, true, true))
        .expect("ok");
    repo.upsert(&make_result(&run_id, "MSFT", 300.0, false, true))
        .expect("ok");
    repo.upsert(&make_result(&run_id, "GOOGL", 120.0, true, true))
        .expect("ok");

    let results = repo
        .get_by_run(&run_id, ResultMatchFilter::None)
        .expect("ok");
    assert_eq!(results.len(), 3);
    // Results are ordered by symbol COLLATE NOCASE: AAPL, GOOGL, MSFT
    assert_eq!(results[0].symbol.as_str(), "AAPL");
    assert_eq!(results[0].matched_condition_count, 1);
    assert_eq!(results[1].symbol.as_str(), "GOOGL");
    assert_eq!(results[1].matched_condition_count, 1);
    assert_eq!(results[2].symbol.as_str(), "MSFT");
    assert_eq!(results[2].matched_condition_count, 0);
}

#[test]
fn preserves_count_after_filtering() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanResultRepository::new(&mut database);

    repo.upsert(&make_result(&run_id, "AAPL", 150.0, true, true))
        .expect("ok");
    repo.upsert(&make_result(&run_id, "MSFT", 300.0, false, true))
        .expect("ok");

    let and_results = repo
        .get_by_run(&run_id, ResultMatchFilter::And)
        .expect("ok");
    assert_eq!(and_results.len(), 1);
    assert_eq!(and_results[0].matched_condition_count, 1);
}
