use super::*;
use crate::db::Database;
use crate::error::AppErrorCode;

fn instrument(symbol: &str, asset_type: AssetType) -> Instrument {
    Instrument {
        symbol: Symbol::new(symbol).expect("symbol must be valid"),
        provider_symbol: Symbol::new(symbol)
            .expect("symbol must be valid")
            .provider_symbol(),
        asset_type,
        exchange: Some("NASDAQ".to_string()),
        is_active: true,
    }
}

fn instrument_with_exchange(
    symbol: &str,
    asset_type: AssetType,
    exchange: Option<&str>,
) -> Instrument {
    Instrument {
        symbol: Symbol::new(symbol).expect("symbol must be valid"),
        provider_symbol: Symbol::new(symbol)
            .expect("symbol must be valid")
            .provider_symbol(),
        asset_type,
        exchange: exchange.map(|e| e.to_string()),
        is_active: true,
    }
}

#[test]
fn upserts_new_instrument() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = InstrumentRepository::new(&mut database);

    let instrument = instrument("AAPL", AssetType::Stock);
    repository.upsert(&instrument).expect("upsert must succeed");

    let loaded = repository
        .get(&Symbol::new("AAPL").expect("symbol must be valid"))
        .expect("instrument must exist");

    assert_eq!(loaded.symbol.as_str(), "AAPL");
    assert_eq!(loaded.provider_symbol, "AAPL");
    assert_eq!(loaded.asset_type, AssetType::Stock);
    assert_eq!(loaded.exchange.as_deref(), Some("NASDAQ"));
    assert!(loaded.is_active);
}

#[test]
fn upsert_updates_existing_instrument() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = InstrumentRepository::new(&mut database);

    // Insert initial instrument
    repository
        .upsert(&instrument("MSFT", AssetType::Stock))
        .expect("initial upsert must succeed");

    // Update with different values
    let updated = Instrument {
        symbol: Symbol::new("MSFT").expect("symbol must be valid"),
        provider_symbol: "MSFT".to_string(),
        asset_type: AssetType::Etf,
        exchange: None,
        is_active: false,
    };
    repository
        .upsert(&updated)
        .expect("update upsert must succeed");

    let loaded = repository
        .get(&Symbol::new("MSFT").expect("symbol must be valid"))
        .expect("instrument must exist");

    assert_eq!(loaded.asset_type, AssetType::Etf);
    assert_eq!(loaded.exchange, None);
    assert!(!loaded.is_active);
}

#[test]
fn gets_existing_instrument() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = InstrumentRepository::new(&mut database);

    repository
        .upsert(&instrument("TSLA", AssetType::Stock))
        .expect("upsert must succeed");

    let loaded = repository
        .get(&Symbol::new("TSLA").expect("symbol must be valid"))
        .expect("instrument must exist");

    assert_eq!(loaded.symbol.as_str(), "TSLA");
}

#[test]
fn get_rejects_invalid_symbol() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let _repository = InstrumentRepository::new(&mut database);

    // Symbol::new validates before reaching the repository
    let error = Symbol::new("AAPL/USD").expect_err("invalid symbol must fail");

    assert_eq!(error.code, AppErrorCode::Validation);
}

#[test]
fn get_returns_not_found_for_missing_symbol() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let repository = InstrumentRepository::new(&mut database);

    let error = repository
        .get(&Symbol::new("GOOGL").expect("symbol must be valid"))
        .expect_err("missing instrument must fail");

    assert_eq!(error.code, AppErrorCode::NotFound);
}

#[test]
fn list_active_excludes_inactive() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = InstrumentRepository::new(&mut database);

    // Insert active instrument
    repository
        .upsert(&instrument("AAPL", AssetType::Stock))
        .expect("active upsert must succeed");

    // Insert inactive instrument
    let inactive = Instrument {
        symbol: Symbol::new("MSFT").expect("symbol must be valid"),
        provider_symbol: "MSFT".to_string(),
        asset_type: AssetType::Stock,
        exchange: Some("NASDAQ".to_string()),
        is_active: false,
    };
    repository
        .upsert(&inactive)
        .expect("inactive upsert must succeed");

    let active = repository.list_active().expect("list must succeed");

    assert_eq!(active.len(), 1);
    assert_eq!(active[0].symbol.as_str(), "AAPL");
}

#[test]
fn list_active_orders_by_symbol() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = InstrumentRepository::new(&mut database);

    // Insert in non-alphabetical order
    repository
        .upsert(&instrument_with_exchange(
            "ZM",
            AssetType::Stock,
            Some("NYSE"),
        ))
        .expect("upsert must succeed");
    repository
        .upsert(&instrument_with_exchange(
            "AAPL",
            AssetType::Stock,
            Some("NASDAQ"),
        ))
        .expect("upsert must succeed");
    repository
        .upsert(&instrument_with_exchange(
            "brk.b",
            AssetType::Stock,
            Some("NYSE"),
        ))
        .expect("upsert must succeed");

    let list = repository.list_active().expect("list must succeed");

    assert_eq!(list.len(), 3);
    assert_eq!(list[0].symbol.as_str(), "AAPL");
    assert_eq!(list[1].symbol.as_str(), "BRK.B");
    assert_eq!(list[2].symbol.as_str(), "ZM");
}
