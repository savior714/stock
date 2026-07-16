use super::*;
use crate::db::Database;
use crate::error::AppErrorCode;

fn write(name: &str, symbols: &[&str]) -> WatchlistWrite {
    WatchlistWrite {
        name: name.to_string(),
        description: Some("  test list  ".to_string()),
        symbols: symbols.iter().map(|symbol| symbol.to_string()).collect(),
    }
}

#[test]
fn creates_lists_and_normalizes_symbols() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = WatchlistRepository::new(&mut database);

    let created = repository
        .create(write(" Core Holdings ", &["aapl", " BRK.B ", "AAPL"]))
        .expect("Watchlist must be created");
    let summaries = repository.list().expect("Watchlists must be listed");

    assert_eq!(created.name, "Core Holdings");
    assert_eq!(created.description.as_deref(), Some("test list"));
    assert_eq!(
        created
            .symbols
            .iter()
            .map(Symbol::as_str)
            .collect::<Vec<_>>(),
        vec!["AAPL", "BRK.B"]
    );
    // Legacy import adds 1 watchlist; this test adds 1 more.
    assert!(!summaries.is_empty());
    assert!(summaries.iter().any(|s| s.name == "Core Holdings"));
    assert_eq!(created.symbols.len(), 2);
}

#[test]
fn updates_metadata_and_replaces_members() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = WatchlistRepository::new(&mut database);
    let created = repository
        .create(write("Candidates", &["NVDA", "AMD"]))
        .expect("Watchlist must be created");

    let updated = repository
        .update(
            &created.id,
            WatchlistWrite {
                name: "AI Leaders".to_string(),
                description: None,
                symbols: vec!["MSFT".to_string(), "NVDA".to_string()],
            },
        )
        .expect("Watchlist must be updated");

    assert_eq!(updated.name, "AI Leaders");
    assert_eq!(updated.description, None);
    assert_eq!(
        updated
            .symbols
            .iter()
            .map(Symbol::as_str)
            .collect::<Vec<_>>(),
        vec!["MSFT", "NVDA"]
    );
}

#[test]
fn rejects_case_insensitive_duplicate_names() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = WatchlistRepository::new(&mut database);
    repository
        .create(write("Long Term", &["QQQ"]))
        .expect("first Watchlist must be created");

    let error = repository
        .create(write("long term", &["SPY"]))
        .expect_err("duplicate name must fail");

    assert_eq!(error.code, AppErrorCode::Conflict);
}

#[test]
fn deletes_watchlist_and_cascades_members() {
    let mut database = Database::open_in_memory().expect("database must initialize");
    let mut repository = WatchlistRepository::new(&mut database);
    let created = repository
        .create(write("Temporary", &["TSLA"]))
        .expect("Watchlist must be created");

    repository
        .delete(&created.id)
        .expect("Watchlist must be deleted");
    let error = repository
        .get(&created.id)
        .expect_err("deleted Watchlist must not exist");

    assert_eq!(error.code, AppErrorCode::NotFound);
}
