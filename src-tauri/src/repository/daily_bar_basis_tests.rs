use super::*;
use crate::domain::{AssetType, Instrument};
use crate::error::AppErrorCode;

fn insert_instrument(database: &mut Database, symbol: &str) {
    let instrument = Instrument {
        symbol: Symbol::new(symbol).expect("symbol must be valid"),
        provider_symbol: symbol.to_string(),
        asset_type: AssetType::Stock,
        exchange: None,
        is_active: true,
    };

    database
        .connection_mut()
        .execute(
            "INSERT INTO instruments (symbol, provider_symbol, asset_type, is_active)
             VALUES (?1, ?2, 'stock', 1)",
            params![instrument.symbol.as_str(), instrument.provider_symbol],
        )
        .expect("instrument must insert");
}

fn bar(symbol: &str, trade_date: &str, price_basis: PriceBasis, close: f64) -> DailyBar {
    DailyBar {
        symbol: Symbol::new(symbol).expect("symbol must be valid"),
        trade_date: trade_date.to_string(),
        price_basis,
        open: close,
        high: close + 1.0,
        low: close - 1.0,
        close,
        volume: 1_000,
    }
}

#[test]
fn rejects_basis_different_from_stored_rows_without_mutation() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    insert_instrument(&mut database, "AAPL");

    {
        let mut repository = DailyBarRepository::new(&mut database);
        repository
            .upsert_batch(&[bar("AAPL", "2026-07-15", PriceBasis::Raw, 100.0)])
            .expect("initial raw bar must insert");

        let error = repository
            .upsert_batch(&[bar(
                "AAPL",
                "2026-07-15",
                PriceBasis::SplitAdjusted,
                50.0,
            )])
            .expect_err("basis mismatch must fail");
        assert_eq!(error.code, AppErrorCode::Validation);
    }

    let (basis, close): (String, f64) = database
        .connection()
        .query_row(
            "SELECT price_basis, close FROM daily_bars
             WHERE symbol = 'AAPL' AND trade_date = '2026-07-15'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .expect("stored bar must remain readable");

    assert_eq!(basis, "raw");
    assert_eq!(close, 100.0);
}

#[test]
fn rejects_preexisting_mixed_basis_series() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    insert_instrument(&mut database, "AAPL");
    database
        .connection_mut()
        .execute_batch(
            "INSERT INTO daily_bars (
                 symbol, trade_date, price_basis, open, high, low, close, volume
             ) VALUES
                 ('AAPL', '2026-07-14', 'raw', 100, 101, 99, 100, 1000),
                 ('AAPL', '2026-07-15', 'split_adjusted', 50, 51, 49, 50, 2000);",
        )
        .expect("mixed fixture must insert");

    let mut repository = DailyBarRepository::new(&mut database);
    let error = repository
        .upsert_batch(&[bar("AAPL", "2026-07-16", PriceBasis::Raw, 102.0)])
        .expect_err("mixed stored basis must fail");

    assert_eq!(error.code, AppErrorCode::Database);
}
