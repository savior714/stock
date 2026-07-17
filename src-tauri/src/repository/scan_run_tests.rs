use super::*;
use crate::db::Database;
use crate::error::AppErrorCode;

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
}

fn make_watchlist_id() -> WatchlistId {
    WatchlistId::new("wl-0000000000000001").expect("valid id")
}

fn make_preset_id() -> ScanPresetId {
    ScanPresetId::new("ps-0000000000000001").expect("valid id")
}

fn make_create_input() -> ScanRunCreate {
    ScanRunCreate {
        watchlist_id: make_watchlist_id(),
        preset_id: make_preset_id(),
        total_symbols: 10,
        preset_snapshot_json: r#"{"name":"Test"}"#.to_string(),
        symbols_snapshot_json: r#"["AAPL","MSFT"]"#.to_string(),
        retry_of_run_id: None,
    }
}

#[test]
fn creates_pending_run_with_snapshots() {
    let mut database = make_database();
    let mut repo = ScanRunRepository::new(&mut database);
    let input = make_create_input();

    let summary = repo.create_pending(&input).expect("must create");

    assert_eq!(summary.status, ScanRunStatus::Pending);
    assert_eq!(summary.total_symbols, 10);
    assert_eq!(summary.succeeded_symbols, 0);
    assert_eq!(summary.failed_symbols, 0);
    assert!(summary.started_at.is_none());
    assert!(summary.finished_at.is_none());
}

#[test]
fn transitions_pending_to_running() {
    let mut database = make_database();
    let mut repo = ScanRunRepository::new(&mut database);
    let input = make_create_input();
    let summary = repo.create_pending(&input).expect("must create");

    repo.start_running(&summary.id).expect("must start");

    let updated = repo.get_summary(&summary.id).expect("must read");
    assert_eq!(updated.status, ScanRunStatus::Running);
    assert!(updated.started_at.is_some());
}

#[test]
fn rejects_invalid_transition_pending_to_completed() {
    let mut database = make_database();
    let mut repo = ScanRunRepository::new(&mut database);
    let input = make_create_input();
    let summary = repo.create_pending(&input).expect("must create");

    let err = repo
        .mark_completed(&summary.id, None)
        .expect_err("must reject");
    assert_eq!(err.code, AppErrorCode::Validation);
}

#[test]
fn updates_progress_in_running_state() {
    let mut database = make_database();
    let mut repo = ScanRunRepository::new(&mut database);
    let input = make_create_input();
    let summary = repo.create_pending(&input).expect("must create");
    repo.start_running(&summary.id).expect("must start");

    repo.update_progress(&summary.id, 3, 1)
        .expect("must update");

    let updated = repo.get_summary(&summary.id).expect("must read");
    assert_eq!(updated.succeeded_symbols, 3);
    assert_eq!(updated.failed_symbols, 1);
}

#[test]
fn rejects_progress_update_when_not_running() {
    let mut database = make_database();
    let mut repo = ScanRunRepository::new(&mut database);
    let input = make_create_input();
    let summary = repo.create_pending(&input).expect("must create");

    let err = repo
        .update_progress(&summary.id, 1, 0)
        .expect_err("must reject");
    assert_eq!(err.code, AppErrorCode::Validation);
}

#[test]
fn marks_completed_with_base_date() {
    let mut database = make_database();
    let mut repo = ScanRunRepository::new(&mut database);
    let input = make_create_input();
    let summary = repo.create_pending(&input).expect("must create");
    repo.start_running(&summary.id).expect("must start");

    repo.mark_completed(&summary.id, Some("2026-07-15"))
        .expect("must complete");

    let detail = repo.get(&summary.id).expect("must read");
    assert_eq!(detail.status, ScanRunStatus::Completed);
    assert_eq!(detail.base_trade_date, Some("2026-07-15".to_string()));
    assert!(detail.finished_at.is_some());
}

#[test]
fn marks_cancelled() {
    let mut database = make_database();
    let mut repo = ScanRunRepository::new(&mut database);
    let input = make_create_input();
    let summary = repo.create_pending(&input).expect("must create");
    repo.start_running(&summary.id).expect("must start");

    repo.mark_cancelled(&summary.id).expect("must cancel");

    let updated = repo.get_summary(&summary.id).expect("must read");
    assert_eq!(updated.status, ScanRunStatus::Cancelled);
    assert!(updated.finished_at.is_some());
}

#[test]
fn marks_failed() {
    let mut database = make_database();
    let mut repo = ScanRunRepository::new(&mut database);
    let input = make_create_input();
    let summary = repo.create_pending(&input).expect("must create");
    repo.start_running(&summary.id).expect("must start");

    repo.mark_failed(&summary.id).expect("must fail");

    let updated = repo.get_summary(&summary.id).expect("must read");
    assert_eq!(updated.status, ScanRunStatus::Failed);
}

#[test]
fn rejects_transition_from_terminal_state() {
    let mut database = make_database();
    let mut repo = ScanRunRepository::new(&mut database);
    let input = make_create_input();
    let summary = repo.create_pending(&input).expect("must create");
    repo.start_running(&summary.id).expect("must start");
    repo.mark_completed(&summary.id, None)
        .expect("must complete");

    let err = repo.start_running(&summary.id).expect_err("must reject");
    assert_eq!(err.code, AppErrorCode::Validation);
}

#[test]
fn lists_recent_runs_ordered_by_creation() {
    let mut database = make_database();
    let mut repo = ScanRunRepository::new(&mut database);

    for i in 0..5 {
        let input = ScanRunCreate {
            watchlist_id: make_watchlist_id(),
            preset_id: make_preset_id(),
            total_symbols: (i + 1) * 10,
            preset_snapshot_json: format!(r#"{{"n":{i}}}"#),
            symbols_snapshot_json: "[]".to_string(),
            retry_of_run_id: None,
        };
        repo.create_pending(&input).expect("must create");
    }

    let recent = repo.list_recent(3).expect("must list");
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0].total_symbols, 50);
    assert_eq!(recent[1].total_symbols, 40);
    assert_eq!(recent[2].total_symbols, 30);
}

#[test]
fn get_detail_preserves_snapshots() {
    let mut database = make_database();
    let mut repo = ScanRunRepository::new(&mut database);
    let input = make_create_input();
    let summary = repo.create_pending(&input).expect("must create");

    let detail = repo.get(&summary.id).expect("must read");
    assert_eq!(detail.preset_snapshot_json["name"], "Test");
    assert!(detail.symbols_snapshot_json.is_array());
    assert_eq!(detail.symbols_snapshot_json.as_array().unwrap().len(), 2);
}

#[test]
fn get_nonexistent_returns_not_found() {
    let mut database = make_database();
    let repo = ScanRunRepository::new(&mut database);
    let fake_id = ScanRunId::new("0000000000000000").expect("valid id");

    let err = repo.get(&fake_id).expect_err("must reject");
    assert_eq!(err.code, AppErrorCode::NotFound);
}

#[test]
fn preserves_retry_of_run_id() {
    let mut database = make_database();
    let mut repo = ScanRunRepository::new(&mut database);

    let parent_input = make_create_input();
    let parent = repo.create_pending(&parent_input).expect("must create");

    let retry_input = ScanRunCreate {
        watchlist_id: make_watchlist_id(),
        preset_id: make_preset_id(),
        total_symbols: 3,
        preset_snapshot_json: r#"{"name":"Retry"}"#.to_string(),
        symbols_snapshot_json: r#"["GOOGL"]"#.to_string(),
        retry_of_run_id: Some(parent.id.clone()),
    };

    let child = repo.create_pending(&retry_input).expect("must create");
    let detail = repo.get(&child.id).expect("must read");

    assert!(detail.retry_of_run_id.is_some());
    assert_eq!(detail.retry_of_run_id.unwrap().0, parent.id.0);
}

#[test]
fn rejects_start_running_on_nonexistent() {
    let mut database = make_database();
    let mut repo = ScanRunRepository::new(&mut database);
    let fake_id = ScanRunId::new("0000000000000000").expect("valid id");

    let err = repo.start_running(&fake_id).expect_err("must reject");
    assert_eq!(err.code, AppErrorCode::NotFound);
}

#[test]
fn full_lifecycle_pending_running_completed() {
    let mut database = make_database();
    let mut repo = ScanRunRepository::new(&mut database);
    let input = make_create_input();

    let summary = repo.create_pending(&input).expect("must create");
    assert_eq!(summary.status, ScanRunStatus::Pending);

    repo.start_running(&summary.id).expect("must start");
    let s = repo.get_summary(&summary.id).expect("must read");
    assert_eq!(s.status, ScanRunStatus::Running);

    repo.update_progress(&summary.id, 8, 2)
        .expect("must update");
    let s = repo.get_summary(&summary.id).expect("must read");
    assert_eq!(s.succeeded_symbols, 8);
    assert_eq!(s.failed_symbols, 2);

    repo.mark_completed(&summary.id, Some("2026-07-15"))
        .expect("must complete");
    let s = repo.get_summary(&summary.id).expect("must read");
    assert_eq!(s.status, ScanRunStatus::Completed);
}

#[test]
fn list_respects_limit() {
    let mut database = make_database();
    let mut repo = ScanRunRepository::new(&mut database);

    for _ in 0..10 {
        repo.create_pending(&make_create_input())
            .expect("must create");
    }

    let results = repo.list_recent(0).expect("must list");
    assert!(results.is_empty());

    let results = repo.list_recent(5).expect("must list");
    assert_eq!(results.len(), 5);

    let results = repo.list_recent(100).expect("must list");
    assert_eq!(results.len(), 10);
}
