use crate::db::Database;
use crate::domain::{ScanError, ScanRunId};
use crate::error::{AppError, AppResult};
use rusqlite::params;

pub struct ScanErrorRepository<'connection> {
    connection: &'connection mut rusqlite::Connection,
}

impl<'connection> ScanErrorRepository<'connection> {
    pub fn new(database: &'connection mut Database) -> Self {
        Self {
            connection: database.connection_mut(),
        }
    }

    pub fn append(&mut self, error: &ScanError) -> AppResult<()> {
        self.connection
            .execute(
                "INSERT INTO scan_errors (
                    run_id, symbol, code, message, detail, retryable, attempt
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
                params![
                    error.run_id.0.as_str(),
                    error.symbol.as_deref(),
                    &error.code,
                    &error.message,
                    error.detail.as_deref(),
                    bool_to_int(error.retryable),
                    error.attempt as i64,
                ],
            )
            .map_err(|error| db_error("failed to append ScanError", error))?;

        Ok(())
    }

    pub fn get_by_run(&self, run_id: &ScanRunId) -> AppResult<Vec<ScanError>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT run_id, symbol, code, message, detail, retryable, attempt
                 FROM scan_errors
                 WHERE run_id = ?1
                 ORDER BY symbol COLLATE NOCASE, attempt",
            )
            .map_err(|error| db_error("failed to prepare ScanError query", error))?;

        let raw_rows = statement
            .query_map(params![run_id.0.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i32>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            })
            .map_err(|error| db_error("failed to query ScanErrors", error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| db_error("failed to read ScanError rows", error))?;

        raw_rows
            .into_iter()
            .map(
                |(run_id_str, symbol, code, message, detail, retryable, attempt)| {
                    Ok(ScanError {
                        run_id: ScanRunId::new(run_id_str)?,
                        symbol,
                        code,
                        message,
                        detail,
                        retryable: retryable == 1,
                        attempt: attempt as u32,
                    })
                },
            )
            .collect()
    }

    /// Return only symbol-scoped retryable failures.
    /// Run-level errors have a NULL symbol and cannot be retried as individual symbols.
    pub fn get_retryable_symbols(&self, run_id: &ScanRunId) -> AppResult<Vec<String>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT DISTINCT symbol FROM scan_errors
                 WHERE run_id = ?1
                   AND retryable = 1
                   AND symbol IS NOT NULL
                 ORDER BY symbol COLLATE NOCASE",
            )
            .map_err(|error| db_error("failed to prepare retryable symbols query", error))?;

        let rows = statement
            .query_map(params![run_id.0.as_str()], |row| row.get::<_, String>(0))
            .map_err(|error| db_error("failed to query retryable symbols", error))?;

        rows.collect::<Result<Vec<_>, _>>()
            .map_err(|error| db_error("failed to read retryable symbols", error))
    }

    pub fn count_retryable(&self, run_id: &ScanRunId) -> AppResult<u32> {
        let count: i64 = self
            .connection
            .query_row(
                "SELECT COUNT(DISTINCT symbol) FROM scan_errors
                 WHERE run_id = ?1
                   AND retryable = 1
                   AND symbol IS NOT NULL",
                params![run_id.0.as_str()],
                |row| row.get(0),
            )
            .map_err(|error| db_error("failed to count retryable errors", error))?;

        Ok(count as u32)
    }
}

fn bool_to_int(value: bool) -> i32 {
    if value { 1 } else { 0 }
}

fn db_error(message: &'static str, error: rusqlite::Error) -> AppError {
    AppError::database(message, error.to_string())
}

#[cfg(test)]
#[path = "scan_error_tests.rs"]
mod tests;
