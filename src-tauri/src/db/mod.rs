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
const HISTORICAL_RESOURCES_MIGRATION: &str =
    include_str!("../../migrations/0005_scan_run_historical_resources.sql");
const LATEST_SCHEMA_VERSION: i64 = 5;

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
            current_version = 4;
        }

        if current_version == 4 {
            connection
                .execute_batch(HISTORICAL_RESOURCES_MIGRATION)
                .map_err(|error| {
                    AppError::database(
                        "failed to apply scan run historical resources migration",
                        error.to_string(),
                    )
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

        assert_eq!(database.schema_version().expect("version must exist"), 5);
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

        assert_eq!(database.schema_version().expect("version must exist"), 5);
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

        let database =
            Database::initialize(connection).expect("database must migrate v2->v3->v4->v5");

        // Verify version reached 5
        assert_eq!(database.schema_version().expect("version must exist"), 5);

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

    #[test]
    fn initializes_latest_schema_version_five() {
        let database = Database::open_in_memory().expect("database must initialize");
        assert_eq!(database.schema_version().expect("version must exist"), 5);
    }

    #[test]
    fn migrates_v4_to_v5_preserving_data() {
        // Build a v4 database manually
        let connection = Connection::open_in_memory().expect("database must open");
        connection
            .execute_batch(INITIAL_MIGRATION)
            .expect("v1 migration must apply");
        connection
            .execute_batch(CONDITION_TRIGGER_MODE_MIGRATION)
            .expect("v2 migration must apply");
        connection
            .execute_batch(SCAN_RUN_SNAPSHOTS_MIGRATION)
            .expect("v3 migration must apply");
        connection
            .execute_batch(LEGACY_IMPORT_MIGRATION)
            .expect("v4 migration must apply");

        // Insert data to verify preservation
        let conn = &connection;
        conn.execute(
            "INSERT INTO watchlists (id, name) VALUES ('wl-mig', 'Migrate Watchlist')",
            [],
        )
        .expect("watchlist must insert");
        conn.execute(
            "INSERT INTO scan_presets (id, name, trigger_mode) VALUES ('ps-mig', 'Migrate Preset', 'current')",
            [],
        ).expect("preset must insert");
        conn.execute(
            "INSERT INTO instruments (symbol, provider_symbol, asset_type) VALUES ('AAPL', 'AAPL', 'stock')",
            [],
        ).ok();
        conn.execute(
            "INSERT INTO instruments (symbol, provider_symbol, asset_type) VALUES ('MSFT', 'MSFT', 'stock')",
            [],
        ).ok();

        // Create a scan run with snapshots
        conn.execute(
            "INSERT INTO scan_runs (id, watchlist_id, preset_id, status, total_symbols, \
             preset_snapshot_json, symbols_snapshot_json, retry_of_run_id) \
             VALUES ('run-mig-1', 'wl-mig', 'ps-mig', 'completed', 2, \
             '{\"name\":\"Test\"}', '[\"AAPL\",\"MSFT\"]', NULL)",
            [],
        )
        .expect("scan run must insert");

        // Create a child retry run
        conn.execute(
            "INSERT INTO scan_runs (id, watchlist_id, preset_id, status, total_symbols, \
             preset_snapshot_json, symbols_snapshot_json, retry_of_run_id) \
             VALUES ('run-mig-2', 'wl-mig', 'ps-mig', 'pending', 1, \
             '{\"name\":\"Retry\"}', '[\"AAPL\"]', 'run-mig-1')",
            [],
        )
        .expect("retry run must insert");

        // Create scan results and errors
        conn.execute(
            "INSERT INTO scan_results (run_id, symbol, trade_date, current_price, \
             all_conditions_matched, any_condition_matched) \
             VALUES ('run-mig-1', 'AAPL', '2026-07-15', 150.0, 1, 1)",
            [],
        )
        .expect("scan result must insert");
        conn.execute(
            "INSERT INTO scan_errors (run_id, symbol, code, message, retryable) \
             VALUES ('run-mig-1', 'MSFT', 'NETWORK_RETRY', 'retryable error', 1)",
            [],
        )
        .expect("scan error must insert");

        // Now migrate to v5
        let database = Database::initialize(connection).expect("v4->v5 migration must succeed");

        // Verify version reached 5
        assert_eq!(database.schema_version().expect("version must exist"), 5);

        // Verify ScanRun data preserved
        let run: (String, String, String, String) = database
            .connection()
            .query_row(
                "SELECT id, watchlist_id, preset_id, status FROM scan_runs WHERE id = 'run-mig-1'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("run must exist");
        assert_eq!(run.0, "run-mig-1");
        assert_eq!(run.1, "wl-mig");
        assert_eq!(run.2, "ps-mig");
        assert_eq!(run.3, "completed");

        // Verify snapshot JSON preserved
        let preset_snap: String = database
            .connection()
            .query_row(
                "SELECT preset_snapshot_json FROM scan_runs WHERE id = 'run-mig-1'",
                [],
                |row| row.get(0),
            )
            .expect("preset snapshot must exist");
        assert_eq!(preset_snap, "{\"name\":\"Test\"}");

        let symbols_snap: String = database
            .connection()
            .query_row(
                "SELECT symbols_snapshot_json FROM scan_runs WHERE id = 'run-mig-1'",
                [],
                |row| row.get(0),
            )
            .expect("symbols snapshot must exist");
        assert_eq!(symbols_snap, "[\"AAPL\",\"MSFT\"]");

        // Verify retry_of_run_id preserved
        let retry_run: String = database
            .connection()
            .query_row(
                "SELECT retry_of_run_id FROM scan_runs WHERE id = 'run-mig-2'",
                [],
                |row| row.get(0),
            )
            .expect("retry run must exist");
        assert_eq!(retry_run, "run-mig-1");

        // Verify ScanResults preserved
        let result_count: i64 = database
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM scan_results WHERE run_id = 'run-mig-1'",
                [],
                |row| row.get(0),
            )
            .expect("result count must exist");
        assert_eq!(result_count, 1);

        // Verify ScanErrors preserved
        let error_count: i64 = database
            .connection()
            .query_row(
                "SELECT COUNT(*) FROM scan_errors WHERE run_id = 'run-mig-1'",
                [],
                |row| row.get(0),
            )
            .expect("error count must exist");
        assert_eq!(error_count, 1);
    }

    #[test]
    fn allows_watchlist_and_preset_deletion_after_migration() {
        let connection = Connection::open_in_memory().expect("database must open");
        connection
            .execute_batch(INITIAL_MIGRATION)
            .expect("v1 migration must apply");
        connection
            .execute_batch(CONDITION_TRIGGER_MODE_MIGRATION)
            .expect("v2 migration must apply");
        connection
            .execute_batch(SCAN_RUN_SNAPSHOTS_MIGRATION)
            .expect("v3 migration must apply");
        connection
            .execute_batch(LEGACY_IMPORT_MIGRATION)
            .expect("v4 migration must apply");
        connection
            .execute_batch(HISTORICAL_RESOURCES_MIGRATION)
            .expect("v4->v5 migration must apply");

        let conn = &connection;
        conn.execute(
            "INSERT INTO watchlists (id, name) VALUES ('wl-del', 'Delete Me')",
            [],
        )
        .expect("watchlist must insert");
        conn.execute(
            "INSERT INTO scan_presets (id, name, trigger_mode) VALUES ('ps-del', 'Delete Me Preset', 'current')",
            [],
        ).expect("preset must insert");
        conn.execute(
            "INSERT INTO scan_runs (id, watchlist_id, preset_id, status, total_symbols, \
             preset_snapshot_json, symbols_snapshot_json) \
             VALUES ('run-del', 'wl-del', 'ps-del', 'completed', 1, '{}', '[]')",
            [],
        )
        .expect("run must insert");

        // Delete watchlist and preset (no FK constraint to block this)
        conn.execute("DELETE FROM watchlists WHERE id = 'wl-del'", [])
            .expect("watchlist delete must succeed");
        conn.execute("DELETE FROM scan_presets WHERE id = 'ps-del'", [])
            .expect("preset delete must succeed");

        // Run should still be queryable
        let run: String = conn
            .query_row("SELECT id FROM scan_runs WHERE id = 'run-del'", [], |row| {
                row.get(0)
            })
            .expect("run must still exist after resource deletion");
        assert_eq!(run, "run-del");
    }

    #[test]
    fn foreign_key_enforcement_reenabled_after_migration() {
        let connection = Connection::open_in_memory().expect("database must open");
        connection
            .execute_batch(INITIAL_MIGRATION)
            .expect("v1 migration must apply");
        connection
            .execute_batch(CONDITION_TRIGGER_MODE_MIGRATION)
            .expect("v2 migration must apply");
        connection
            .execute_batch(SCAN_RUN_SNAPSHOTS_MIGRATION)
            .expect("v3 migration must apply");
        connection
            .execute_batch(LEGACY_IMPORT_MIGRATION)
            .expect("v4 migration must apply");
        connection
            .execute_batch(HISTORICAL_RESOURCES_MIGRATION)
            .expect("v4->v5 migration must apply");

        // Verify foreign keys are enforced
        let fk: i64 = connection
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .expect("foreign_keys pragma must be readable");
        assert_eq!(fk, 1);
    }

    #[test]
    fn foreign_key_check_clean_after_migration() {
        let connection = Connection::open_in_memory().expect("database must open");
        connection
            .execute_batch(INITIAL_MIGRATION)
            .expect("v1 migration must apply");
        connection
            .execute_batch(CONDITION_TRIGGER_MODE_MIGRATION)
            .expect("v2 migration must apply");
        connection
            .execute_batch(SCAN_RUN_SNAPSHOTS_MIGRATION)
            .expect("v3 migration must apply");
        connection
            .execute_batch(LEGACY_IMPORT_MIGRATION)
            .expect("v4 migration must apply");
        connection
            .execute_batch(HISTORICAL_RESOURCES_MIGRATION)
            .expect("v4->v5 migration must apply");

        // PRAGMA foreign_key_check should return no rows (clean)
        let mut stmt = connection
            .prepare("PRAGMA foreign_key_check")
            .expect("foreign_key_check must prepare");
        let rows = stmt
            .query_map([], |row| row.get::<_, String>(0))
            .expect("foreign_key_check must run");
        let orphans: Vec<String> = rows.map(|r| r.expect("row must be readable")).collect();
        assert!(
            orphans.is_empty(),
            "no foreign key violations expected, found: {:?}",
            orphans
        );
    }
}
