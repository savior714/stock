use crate::error::{AppError, AppResult};
use rusqlite::Connection;
use std::fs;
use std::path::Path;

const INITIAL_MIGRATION: &str = include_str!("../../migrations/0001_initial.sql");
const LATEST_SCHEMA_VERSION: i64 = 1;

pub struct Database {
    connection: Connection,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> AppResult<Self> {
        let path = path.as_ref();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| {
                AppError::database("failed to create database directory", error.to_string())
            })?;
        }

        let connection = Connection::open(path)
            .map_err(|error| AppError::database("failed to open SQLite database", error.to_string()))?;
        Self::initialize(connection)
    }

    pub fn open_in_memory() -> AppResult<Self> {
        let connection = Connection::open_in_memory().map_err(|error| {
            AppError::database("failed to open in-memory SQLite database", error.to_string())
        })?;
        Self::initialize(connection)
    }

    pub fn connection(&self) -> &Connection {
        &self.connection
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

        let current_version: i64 = connection
            .query_row("PRAGMA user_version", [], |row| row.get(0))
            .map_err(|error| AppError::database("failed to read schema version", error.to_string()))?;

        match current_version {
            0 => connection.execute_batch(INITIAL_MIGRATION).map_err(|error| {
                AppError::database("failed to apply initial SQLite migration", error.to_string())
            })?,
            LATEST_SCHEMA_VERSION => {}
            newer => {
                return Err(AppError::database(
                    "database schema is newer than this application",
                    format!("found version {newer}, supported version {LATEST_SCHEMA_VERSION}"),
                ));
            }
        }

        Ok(Self { connection })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initializes_schema_and_enables_foreign_keys() {
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

        assert_eq!(database.schema_version().expect("version must exist"), 1);
        assert_eq!(foreign_keys, 1);
        assert_eq!(watchlists_table, "watchlists");
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
