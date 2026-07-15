use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::fs;
use std::path::Path;

const INITIAL_MIGRATION: &str = include_str!("../../migrations/0001_initial.sql");
const CONDITION_TRIGGER_MODE_MIGRATION: &str =
    include_str!("../../migrations/0002_condition_trigger_modes.sql");
const LATEST_SCHEMA_VERSION: i64 = 2;

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
        }

        Ok(Self { connection })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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

        assert_eq!(database.schema_version().expect("version must exist"), 2);
        assert_eq!(foreign_keys, 1);
        assert_eq!(watchlists_table, "watchlists");
        assert_eq!(default_condition_count, 6);
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

        assert_eq!(database.schema_version().expect("version must exist"), 2);
        assert_eq!(trigger_mode, "cross");
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
