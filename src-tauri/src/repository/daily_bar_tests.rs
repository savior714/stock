use super::*;
use crate::db::Database;
use crate::error::AppErrorCode;

#[allow(clippy::too_many_arguments)]
fn bar(
    symbol: &str,
    trade_date: &str,
    price_basis: PriceBasis,
    open: f64,
    high: f64,
    low: f64,
    close: f64,
    volume: u64,
) -> DailyBar {
    DailyBar {
        symbol: Symbol::new(symbol).expect("symbol must be valid"),
        trade_date: trade_date.to_string(),
        price_basis,
        open,
        high,
        low,
        close,
        volume,
    }
}

fn bar_raw(symbol: &str, trade_date: &str) -> DailyBar {
    bar(
        symbol,
        trade_date,
        PriceBasis::Raw,
        100.0,
        104.0,
        99.0,
        102.0,
        1_000,
    )
}

/// Insert a minimal instrument row to satisfy the daily_bars FK constraint.
fn insert_instrument(database: &mut Database, symbol: &str) {
    database
        .connection_mut()
        .execute(
            "INSERT OR IGNORE INTO instruments (symbol, provider_symbol, asset_type, is_active)
             VALUES (?1, ?2, 'stock', 1)",
            rusqlite::params![symbol, symbol],
        )
        .expect("instrument must insert");
}

#[test]
fn upserts_new_bars() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    insert_instrument(&mut database, "AAPL");
    let mut repository = DailyBarRepository::new(&mut database);

    let bars = vec![
        bar_raw("AAPL", "2026-07-12"),
        bar(
            "AAPL",
            "2026-07-13",
            PriceBasis::Raw,
            101.0,
            105.0,
            100.0,
            103.0,
            2_000,
        ),
        bar_raw("AAPL", "2026-07-14"),
    ];

    repository.upsert_batch(&bars).expect("upsert must succeed");

    let loaded = repository
        .load_range(
            &Symbol::new("AAPL").expect("symbol must be valid"),
            "2026-07-12",
            "2026-07-14",
        )
        .expect("load must succeed");

    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded[0].trade_date, "2026-07-12");
    assert_eq!(loaded[1].trade_date, "2026-07-13");
    assert_eq!(loaded[1].close, 103.0);
    assert_eq!(loaded[2].trade_date, "2026-07-14");
}

#[test]
fn upsert_overwrites_existing_date() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    insert_instrument(&mut database, "AAPL");
    let mut repository = DailyBarRepository::new(&mut database);

    // Insert initial bar
    repository
        .upsert_batch(&[bar_raw("AAPL", "2026-07-13")])
        .expect("initial upsert must succeed");

    // Overwrite same date with different values
    let updated = bar(
        "AAPL",
        "2026-07-13",
        PriceBasis::Raw,
        200.0,
        210.0,
        195.0,
        205.0,
        5_000,
    );
    repository
        .upsert_batch(&[updated])
        .expect("overwrite upsert must succeed");

    let loaded = repository
        .load_range(
            &Symbol::new("AAPL").expect("symbol must be valid"),
            "2026-07-13",
            "2026-07-13",
        )
        .expect("load must succeed");

    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].open, 200.0);
    assert_eq!(loaded[0].high, 210.0);
    assert_eq!(loaded[0].low, 195.0);
    assert_eq!(loaded[0].close, 205.0);
    assert_eq!(loaded[0].volume, 5_000);
}

#[test]
fn date_range_returns_empty_for_unknown_symbol() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let repository = DailyBarRepository::new(&mut database);

    let result = repository
        .date_range(&Symbol::new("GOOGL").expect("symbol must be valid"))
        .expect("query must succeed");

    assert!(result.is_none());
}

#[test]
fn date_range_returns_correct_span() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    insert_instrument(&mut database, "AAPL");
    let mut repository = DailyBarRepository::new(&mut database);

    let bars = vec![
        bar_raw("AAPL", "2026-07-10"),
        bar_raw("AAPL", "2026-07-15"),
        bar_raw("AAPL", "2026-07-20"),
    ];
    repository.upsert_batch(&bars).expect("upsert must succeed");

    let range = repository
        .date_range(&Symbol::new("AAPL").expect("symbol must be valid"))
        .expect("query must succeed")
        .expect("range must exist");

    assert_eq!(range.0, "2026-07-10");
    assert_eq!(range.1, "2026-07-20");
}

#[test]
fn load_range_orders_ascending() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    insert_instrument(&mut database, "AAPL");
    let mut repository = DailyBarRepository::new(&mut database);

    // Insert in non-chronological order
    let bars = vec![
        bar_raw("AAPL", "2026-07-15"),
        bar_raw("AAPL", "2026-07-10"),
        bar_raw("AAPL", "2026-07-20"),
    ];
    repository.upsert_batch(&bars).expect("upsert must succeed");

    let loaded = repository
        .load_range(
            &Symbol::new("AAPL").expect("symbol must be valid"),
            "2026-07-01",
            "2026-07-31",
        )
        .expect("load must succeed");

    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded[0].trade_date, "2026-07-10");
    assert_eq!(loaded[1].trade_date, "2026-07-15");
    assert_eq!(loaded[2].trade_date, "2026-07-20");
}

#[test]
fn load_range_filters_by_dates() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    insert_instrument(&mut database, "AAPL");
    let mut repository = DailyBarRepository::new(&mut database);

    let bars = vec![
        bar_raw("AAPL", "2026-07-10"),
        bar_raw("AAPL", "2026-07-13"),
        bar_raw("AAPL", "2026-07-15"),
        bar_raw("AAPL", "2026-07-20"),
    ];
    repository.upsert_batch(&bars).expect("upsert must succeed");

    let loaded = repository
        .load_range(
            &Symbol::new("AAPL").expect("symbol must be valid"),
            "2026-07-12",
            "2026-07-16",
        )
        .expect("load must succeed");

    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].trade_date, "2026-07-13");
    assert_eq!(loaded[1].trade_date, "2026-07-15");
}

#[test]
fn rejects_mixed_price_basis() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = DailyBarRepository::new(&mut database);

    let bars = vec![
        bar(
            "AAPL",
            "2026-07-13",
            PriceBasis::Raw,
            100.0,
            104.0,
            99.0,
            102.0,
            1_000,
        ),
        bar(
            "AAPL",
            "2026-07-14",
            PriceBasis::SplitAdjusted,
            101.0,
            105.0,
            100.0,
            103.0,
            2_000,
        ),
    ];

    let error = repository
        .upsert_batch(&bars)
        .expect_err("mixed basis must fail");

    assert_eq!(error.code, AppErrorCode::Validation);
}

#[test]
fn rejects_invalid_bar() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = DailyBarRepository::new(&mut database);

    // Bar with negative price
    let bars = vec![bar(
        "AAPL",
        "2026-07-13",
        PriceBasis::Raw,
        -10.0,
        104.0,
        99.0,
        102.0,
        1_000,
    )];

    let error = repository
        .upsert_batch(&bars)
        .expect_err("invalid bar must fail");

    assert_eq!(error.code, AppErrorCode::InvalidMarketData);
}
