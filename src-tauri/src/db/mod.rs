use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::fs;
use std::path::Path;

const INITIAL_MIGRATION: &str = include_str!("../../migrations/0001_initial.sql");
const CONDITION_TRIGGER_MODE_MIGRATION: &str =
    include_str!("../../migrations/0002_condition_trigger_modes.sql");
const SCAN_RUN_SNAPSHOTS_MIGRATION: &str =
    include_str!("../../migrations/0003_scan_run_snapshots.sql");
const LEGACY_IMPORT_MIGRATION: &str = include_str!("../../migrations/0004_legacy_import.sql");
const LATEST_SCHEMA_VERSION: i64 = 4;

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|error| {
                AppError::database("failed to create database directory", error.to_string())
            })?;
        }

        let connection = Connection::open(path).map_err(|error| {
            AppError::database("failed to open SQLite database", error.to_string())
        })?;
        Self::initialize(connection)
    }

    pub fn open_in_memory() -> AppResult<Self> {
        let connection = Connection::open_in_memory().map_err(|error| {
            AppError::database(
                "failed to open in-memory SQLite database",
                error.to_string(),
            )
        })?;
        Self::initialize(connection)
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
    }

    pub(crate) fn connection_mut(&mut self) -> &mut Connection {
        &mut self.connection
    }

    pub fn schema_version(&self) -> AppResult<i64> {
        self.connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| AppError::database("failed to read schema version", error.to_string()))
    }

    fn initialize(connection: Connection) -> AppResult<Self> {
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA journal_mode = WAL;
                 PRAGMA synchronous = NORMAL;
                 PRAGMA busy_timeout = 5000;",
            )
            .map_err(|error| AppError::database("failed to configure SQLite", error.to_string()))?;

        let mut current_version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| {
                AppError::database("failed to read schema version", error.to_string())
            })?;

        if current_version > LATEST_SCHEMA_VERSION {
            return Err(AppError::database(
                "database schema is newer than this application",
                format!(
                    "found version {current_version}, supported version {LATEST_SCHEMA_VERSION}"
                ),
            ));
        }

        if current_version == 0 {
            connection
                .execute_batch(INITIAL_MIGRATION)
                .map_err(|error| {
                    AppError::database(
                        "failed to apply initial SQLite migration",
                        error.to_string(),
                    )
                })?;
            current_version = 1;
        }

        if current_version == 1 {
            connection
                .execute_batch(CONDITION_TRIGGER_MODE_MIGRATION)
                .map_err(|error| {
                    AppError::database(
                        "failed to apply condition trigger mode migration",
                        error.to_string(),
                    )
                })?;
            current_version = 2;
        }

        if current_version == 2 {
            connection
                .execute_batch(SCAN_RUN_SNAPSHOTS_MIGRATION)
                .map_err(|error| {
                    AppError::database(
                        "failed to apply scan run snapshots migration",
                        error.to_string(),
                    )
                })?;
            current_version = 3;
        }

        if current_version == 3 {
            connection
                .execute_batch(LEGACY_IMPORT_MIGRATION)
                .map_err(|error| {
                    AppError::database("failed to apply legacy import migration", error.to_string())
                })?;
        }

        Ok(Self { connection })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn get_column_name(connection: &Connection, table: &str, column: &str) -> Option<String> {
        let mut stmt = connection
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("pragma query must prepare");
        let mut rows = stmt.query([]).expect("pragma query must execute");
        while let Some(row) = rows.next().expect("row must be readable") {
            let name: String = row.get(1).expect("name must be readable");
            if name == column {
                return Some(name);
            }
        }
        None
    }

    fn get_column_default(connection: &Connection, table: &str, column: &str) -> Option<String> {
        let mut stmt = connection
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("pragma query must prepare");
        let mut rows = stmt.query([]).expect("pragma query must execute");
        while let Some(row) = rows.next().expect("row must be readable") {
            let name: String = row.get(1).expect("name must be readable");
            if name == column {
                return row.get(4).ok();
            }
        }
        None
    }

    #[test]
    fn initializes_latest_schema_and_enables_foreign_keys() {
        let database = Database::open_in_memory().expect("database must initialize");
        let foreign_keys: i64 = database
            .connection()
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign key pragma must be readable");
        let watchlists_table: String = database
            .connection()
            .query_row(
                "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'watchlists'",
                [],
                |row| row.get(0),
            )
            .expect("watchlists table must exist");
        let default_condition_count: i64 = database
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM scan_preset_conditions
                 WHERE preset_id = 'default-legacy-triple'",
                [],
                |row| row.get(0),
            )
            .expect("default preset conditions must be readable");

        // Verify v3 snapshot columns exist on scan_runs
        let preset_snapshot_col =
            get_column_name(database.connection(), "scan_runs", "preset_snapshot_json")
                .expect("preset_snapshot_json column must exist");
        let symbols_snapshot_col =
            get_column_name(database.connection(), "scan_runs", "symbols_snapshot_json")
                .expect("symbols_snapshot_json column must exist");

        assert_eq!(database.schema_version().expect("version must exist"), 4);
        assert_eq!(foreign_keys, 1);
        assert_eq!(watchlists_table, "watchlists");

        // Verify Legacy Import watchlist was created
        let legacy_wl: String = database
            .connection()
            .query_row(
                "SELECT name FROM watchlists WHERE id = 'legacy-import-0000-0000-0000-000000000000'",
                [],
                |row| row.get(0),
            )
            .expect("Legacy Import watchlist must exist");
        assert_eq!(legacy_wl, "Legacy Import");

        // Verify symbols were imported
        let symbol_count: i64 = database
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM watchlist_symbols WHERE watchlist_id = 'legacy-import-0000-0000-0000-000000000000'",
                [],
                |row| row.get(0),
            )
            .expect("symbol count must be readable");
        assert_eq!(symbol_count, 374);
        assert_eq!(default_condition_count, 6);
        assert_eq!(preset_snapshot_col, "preset_snapshot_json");
        assert_eq!(symbols_snapshot_col, "symbols_snapshot_json");
    }

    #[test]
    fn migrates_existing_version_one_database() {
        let connection = Connection::open_in_memory().expect("database must open");
        connection
            .execute_batch(INITIAL_MIGRATION)
            .expect("version one migration must apply");
        connection
            .execute(
                "INSERT INTO scan_presets (id, name, trigger_mode)
                 VALUES ('legacy', 'Legacy', 'cross')",
                [],
            )
            .expect("legacy preset must insert");
        connection
            .execute(
                "INSERT INTO scan_preset_conditions (
                    id, preset_id, indicator, side, period, threshold
                 ) VALUES ('legacy-rsi', 'legacy', 'rsi', 'lower', 14, 30)",
                [],
            )
            .expect("legacy condition must insert");

        let database = Database::initialize(connection).expect("database must migrate");
        let trigger_mode: String = database
            .connection()
            .query_row(
                "SELECT trigger_mode FROM scan_preset_conditions WHERE id = 'legacy-rsi'",
                [],
                |row| row.get(0),
            )
            .expect("trigger mode must exist");

        // v4 legacy import columns must also exist after full migration chain
        let retry_of_col = get_column_name(database.connection(), "scan_runs", "retry_of_run_id")
            .expect("retry_of_run_id column must exist after v1->v2->v3->v4");

        assert_eq!(database.schema_version().expect("version must exist"), 4);
        assert_eq!(trigger_mode, "cross");
        assert_eq!(retry_of_col, "retry_of_run_id");
    }

    #[test]
    fn migrates_existing_version_two_database() {
        let connection = Connection::open_in_memory().expect("database must open");
        // Apply v1 then v2 manually to simulate existing v2 database
        connection
            .execute_batch(INITIAL_MIGRATION)
            .expect("v1 migration must apply");
        connection
            .execute_batch(CONDITION_TRIGGER_MODE_MIGRATION)
            .expect("v2 migration must apply");

        // Insert data to verify preservation through v3 migration
        connection
            .execute(
                "INSERT INTO watchlists (id, name) VALUES ('wl-1', 'Test Watchlist')",
                [],
            )
            .expect("watchlist must insert");
        connection
            .execute(
                "INSERT INTO scan_presets (id, name, trigger_mode) VALUES ('ps-1', 'Test Preset', 'cross')",
                [],
            )
            .expect("preset must insert");

        let database = Database::initialize(connection).expect("database must migrate v2->v3->v4");

        // Verify version reached 4
        assert_eq!(database.schema_version().expect("version must exist"), 4);

        // Verify existing data preserved
        let watchlist_name: String = database
            .connection()
            .query_row("SELECT name FROM watchlists WHERE id = 'wl-1'", [], |row| {
                row.get(0)
            })
            .expect("watchlist must be preserved");
        let preset_name: String = database
            .connection()
            .query_row(
                "SELECT name FROM scan_presets WHERE id = 'ps-1'",
                [],
                |row| row.get(0),
            )
            .expect("preset must be preserved");
        assert_eq!(watchlist_name, "Test Watchlist");
        assert_eq!(preset_name, "Test Preset");

        // Verify v3 columns exist with correct defaults
        let preset_snapshot_default =
            get_column_default(database.connection(), "scan_runs", "preset_snapshot_json")
                .expect("preset_snapshot_json default must exist");
        assert_eq!(preset_snapshot_default, "'{}'");
    }

    #[test]
    fn rejects_newer_schema() {
        let connection = Connection::open_in_memory().expect("database must open");
        connection
            .execute_batch(
                "PRAGMA foreign_keys = ON;
                 PRAGMA user_version = 99;",
            )
            .expect("user_version must be set");

        let result = Database::initialize(connection);
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error.code, crate::error::AppErrorCode::Database);
        } else {
            panic!("expected error for newer schema");
        }
    }

    #[test]
    fn rejects_orphan_watchlist_symbol() {
        let database = Database::open_in_memory().expect("database must initialize");
        let result = database.connection().execute(
            "INSERT INTO watchlist_symbols (watchlist_id, symbol) VALUES ('missing', 'AAPL')",
            [],
        );

        assert!(result.is_err());
    }
}
