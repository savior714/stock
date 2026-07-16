use super::*;
use crate::db::Database;

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

fn make_error(run_id: &ScanRunId, symbol: Option<&str>, code: &str, retryable: bool) -> ScanError {
    ScanError {
        run_id: run_id.clone(),
        symbol: symbol.map(str::to_string),
        code: code.to_string(),
        message: "test error".to_string(),
        detail: None,
        retryable,
        attempt: 1,
    }
}

#[test]
fn appends_and_retrieves_errors() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanErrorRepository::new(&mut database);

    repo.append(&make_error(
        &run_id,
        Some("AAPL"),
        "provider_unavailable",
        true,
    ))
    .expect("ok");
    repo.append(&make_error(&run_id, Some("MSFT"), "invalid_data", false))
        .expect("ok");

    let errors = repo.get_by_run(&run_id).expect("ok");
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].symbol.as_deref(), Some("AAPL"));
    assert_eq!(errors[1].symbol.as_deref(), Some("MSFT"));
}

#[test]
fn multiple_errors_for_same_symbol() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanErrorRepository::new(&mut database);

    let mut first = make_error(&run_id, Some("AAPL"), "rate_limited", true);
    first.attempt = 1;
    repo.append(&first).expect("ok");

    let mut second = make_error(&run_id, Some("AAPL"), "rate_limited", true);
    second.attempt = 2;
    repo.append(&second).expect("ok");

    let errors = repo.get_by_run(&run_id).expect("ok");
    assert_eq!(errors.len(), 2);
    assert_eq!(errors[0].attempt, 1);
    assert_eq!(errors[1].attempt, 2);
}

#[test]
fn returns_retryable_symbols() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanErrorRepository::new(&mut database);

    repo.append(&make_error(&run_id, Some("AAPL"), "rate_limited", true))
        .expect("ok");
    repo.append(&make_error(&run_id, Some("MSFT"), "rate_limited", true))
        .expect("ok");
    repo.append(&make_error(&run_id, Some("GOOGL"), "invalid_data", false))
        .expect("ok");

    let symbols = repo.get_retryable_symbols(&run_id).expect("ok");
    assert_eq!(symbols, vec!["AAPL", "MSFT"]);
}

#[test]
fn retryable_run_level_error_is_not_a_symbol_target() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanErrorRepository::new(&mut database);

    repo.append(&make_error(&run_id, None, "provider_unavailable", true))
        .expect("run-level error must append");
    repo.append(&make_error(&run_id, Some("AAPL"), "rate_limited", true))
        .expect("symbol error must append");

    assert_eq!(
        repo.get_retryable_symbols(&run_id).expect("query must succeed"),
        vec!["AAPL"]
    );
    assert_eq!(repo.count_retryable(&run_id).expect("count must succeed"), 1);
}

#[test]
fn counts_retryable_symbols() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanErrorRepository::new(&mut database);

    let mut first = make_error(&run_id, Some("AAPL"), "rate_limited", true);
    first.attempt = 1;
    repo.append(&first).expect("ok");

    let mut second = make_error(&run_id, Some("AAPL"), "rate_limited", true);
    second.attempt = 2;
    repo.append(&second).expect("ok");

    repo.append(&make_error(&run_id, Some("MSFT"), "rate_limited", true))
        .expect("ok");
    repo.append(&make_error(&run_id, Some("GOOGL"), "invalid_data", false))
        .expect("ok");

    assert_eq!(repo.count_retryable(&run_id).expect("ok"), 2);
}

#[test]
fn handles_null_symbol() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanErrorRepository::new(&mut database);

    repo.append(&make_error(&run_id, None, "internal", false))
        .expect("ok");

    let errors = repo.get_by_run(&run_id).expect("ok");
    assert_eq!(errors.len(), 1);
    assert!(errors[0].symbol.is_none());
}

#[test]
fn preserves_detail_field() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanErrorRepository::new(&mut database);

    let error = ScanError {
        run_id: run_id.clone(),
        symbol: Some("AAPL".to_string()),
        code: "provider_error".to_string(),
        message: "request failed".to_string(),
        detail: Some("connection timeout after 30s".to_string()),
        retryable: true,
        attempt: 1,
    };

    repo.append(&error).expect("ok");
    let retrieved = repo.get_by_run(&run_id).expect("ok");
    assert_eq!(
        retrieved[0].detail,
        Some("connection timeout after 30s".to_string())
    );
}

#[test]
fn empty_run_returns_no_errors() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let repo = ScanErrorRepository::new(&mut database);

    assert!(repo.get_by_run(&run_id).expect("ok").is_empty());
}

#[test]
fn errors_ordered_by_symbol() {
    let mut database = make_database();
    let run_id = insert_scan_run(&mut database);
    let mut repo = ScanErrorRepository::new(&mut database);

    repo.append(&make_error(&run_id, Some("ZM"), "error", false))
        .expect("ok");
    repo.append(&make_error(&run_id, Some("AAPL"), "error", false))
        .expect("ok");
    repo.append(&make_error(&run_id, Some("MSFT"), "error", false))
        .expect("ok");

    let errors = repo.get_by_run(&run_id).expect("ok");
    assert_eq!(errors[0].symbol.as_deref(), Some("AAPL"));
    assert_eq!(errors[1].symbol.as_deref(), Some("MSFT"));
    assert_eq!(errors[2].symbol.as_deref(), Some("ZM"));
}
