use crate::db::Database;
use crate::domain::{ScanPresetId, ScanRunId, ScanRunStatus, WatchlistId};
use crate::error::{AppError, AppResult};
use rusqlite::{params, OptionalExtension};
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ScanRunCreate {
    pub watchlist_id: WatchlistId,
    pub preset_id: ScanPresetId,
    pub total_symbols: u32,
    pub preset_snapshot_json: String,
    pub symbols_snapshot_json: String,
    pub retry_of_run_id: Option<ScanRunId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRunSummary {
    pub id: ScanRunId,
    pub watchlist_id: WatchlistId,
    pub preset_id: ScanPresetId,
    pub status: ScanRunStatus,
    pub total_symbols: u32,
    pub succeeded_symbols: u32,
    pub failed_symbols: u32,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRunDetail {
    pub id: ScanRunId,
    pub watchlist_id: WatchlistId,
    pub preset_id: ScanPresetId,
    pub status: ScanRunStatus,
    pub base_trade_date: Option<String>,
    pub total_symbols: u32,
    pub succeeded_symbols: u32,
    pub failed_symbols: u32,
    pub started_at: Option<String>,
    pub finished_at: Option<String>,
    pub preset_snapshot_json: serde_json::Value,
    pub symbols_snapshot_json: serde_json::Value,
    pub retry_of_run_id: Option<ScanRunId>,
}

pub struct ScanRunRepository<'connection> {
    connection: &'connection mut rusqlite::Connection,
}

impl<'connection> ScanRunRepository<'connection> {
    pub fn new(database: &'connection mut Database) -> Self {
        Self {
            connection: database.connection_mut(),
        }
    }

    pub fn create_pending(&mut self, input: &ScanRunCreate) -> AppResult<ScanRunSummary> {
        let transaction = self
            .connection
            .transaction()
            .map_err(|error| db_error("failed to start ScanRun transaction", error))?;

        let raw_id: String = transaction
            .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
            .map_err(|error| db_error("failed to generate ScanRun id", error))?;
        let id = ScanRunId::new(raw_id)?;

        transaction
            .execute(
                "INSERT INTO scan_runs (
                    id, watchlist_id, preset_id, status, total_symbols,
                    preset_snapshot_json, symbols_snapshot_json, retry_of_run_id
                ) VALUES (?1, ?2, ?3, 'pending', ?4, ?5, ?6, ?7)",
                params![
                    id.0.as_str(),
                    input.watchlist_id.0.as_str(),
                    input.preset_id.0.as_str(),
                    input.total_symbols as i64,
                    &input.preset_snapshot_json,
                    &input.symbols_snapshot_json,
                    input.retry_of_run_id.as_ref().map(|r| r.0.as_str()),
                ],
            )
            .map_err(|error| db_error("failed to create ScanRun", error))?;

        transaction
            .commit()
            .map_err(|error| db_error("failed to commit ScanRun creation", error))?;

        self.get_summary(&id)
    }

    pub fn start_running(&mut self, id: &ScanRunId) -> AppResult<()> {
        validate_transition(self.connection, id, ScanRunStatus::Running)?;

        let changed = self
            .connection
            .execute(
                "UPDATE scan_runs SET status = 'running', started_at = CURRENT_TIMESTAMP WHERE id = ?1",
                params![id.0.as_str()],
            )
            .map_err(|error| db_error("failed to start ScanRun", error))?;

        if changed == 0 {
            return Err(AppError::not_found("ScanRun not found"));
        }
        Ok(())
    }

    pub fn update_progress(
        &mut self,
        id: &ScanRunId,
        succeeded: u32,
        failed: u32,
    ) -> AppResult<()> {
        ensure_status(self.connection, id, ScanRunStatus::Running)?;

        let changed = self
            .connection
            .execute(
                "UPDATE scan_runs
                 SET succeeded_symbols = ?1, failed_symbols = ?2
                 WHERE id = ?3",
                params![succeeded as i64, failed as i64, id.0.as_str()],
            )
            .map_err(|error| db_error("failed to update ScanRun progress", error))?;

        if changed == 0 {
            return Err(AppError::not_found("ScanRun not found"));
        }
        Ok(())
    }

    pub fn mark_completed(
        &mut self,
        id: &ScanRunId,
        base_trade_date: Option<&str>,
    ) -> AppResult<()> {
        ensure_status(self.connection, id, ScanRunStatus::Running)?;

        let changed = self
            .connection
            .execute(
                "UPDATE scan_runs
                 SET status = 'completed', base_trade_date = ?1, finished_at = CURRENT_TIMESTAMP
                 WHERE id = ?2",
                params![base_trade_date, id.0.as_str()],
            )
            .map_err(|error| db_error("failed to complete ScanRun", error))?;

        if changed == 0 {
            return Err(AppError::not_found("ScanRun not found"));
        }
        Ok(())
    }

    pub fn mark_cancelled(&mut self, id: &ScanRunId) -> AppResult<()> {
        ensure_status(self.connection, id, ScanRunStatus::Running)?;

        let changed = self
            .connection
            .execute(
                "UPDATE scan_runs
                 SET status = 'cancelled', finished_at = CURRENT_TIMESTAMP
                 WHERE id = ?1",
                params![id.0.as_str()],
            )
            .map_err(|error| db_error("failed to cancel ScanRun", error))?;

        if changed == 0 {
            return Err(AppError::not_found("ScanRun not found"));
        }
        Ok(())
    }

    pub fn mark_failed(&mut self, id: &ScanRunId) -> AppResult<()> {
        ensure_status(self.connection, id, ScanRunStatus::Running)?;

        let changed = self
            .connection
            .execute(
                "UPDATE scan_runs
                 SET status = 'failed', finished_at = CURRENT_TIMESTAMP
                 WHERE id = ?1",
                params![id.0.as_str()],
            )
            .map_err(|error| db_error("failed to fail ScanRun", error))?;

        if changed == 0 {
            return Err(AppError::not_found("ScanRun not found"));
        }
        Ok(())
    }

    pub fn list_recent(&self, limit: u32) -> AppResult<Vec<ScanRunSummary>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, watchlist_id, preset_id, status, total_symbols,
                       succeeded_symbols, failed_symbols, started_at, finished_at
                 FROM scan_runs
                 ORDER BY created_at DESC, rowid DESC
                 LIMIT ?1",
            )
            .map_err(|error| db_error("failed to prepare ScanRun list query", error))?;

        let raw_rows = statement
            .query_map([limit as i64], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            })
            .map_err(|error| db_error("failed to list ScanRuns", error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| db_error("failed to read ScanRun rows", error))?;

        raw_rows
            .into_iter()
            .map(
                |(
                    id_str,
                    wl_id,
                    ps_id,
                    status_str,
                    total,
                    succeeded,
                    failed,
                    started_at,
                    finished_at,
                )| {
                    Ok(ScanRunSummary {
                        id: ScanRunId::new(id_str)?,
                        watchlist_id: WatchlistId::new(wl_id)?,
                        preset_id: ScanPresetId::new(ps_id)?,
                        status: parse_scan_run_status(&status_str)?,
                        total_symbols: total as u32,
                        succeeded_symbols: succeeded as u32,
                        failed_symbols: failed as u32,
                        started_at,
                        finished_at,
                    })
                },
            )
            .collect()
    }

    pub fn get(&self, id: &ScanRunId) -> AppResult<ScanRunDetail> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, watchlist_id, preset_id, status, base_trade_date,
                       total_symbols, succeeded_symbols, failed_symbols,
                       started_at, finished_at, preset_snapshot_json,
                       symbols_snapshot_json, retry_of_run_id
                 FROM scan_runs WHERE id = ?1",
            )
            .map_err(|error| db_error("failed to prepare ScanRun detail query", error))?;

        let row = statement
            .query_row(params![id.0.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, i64>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, Option<String>>(9)?,
                    row.get::<_, String>(10)?,
                    row.get::<_, String>(11)?,
                    row.get::<_, Option<String>>(12)?,
                ))
            })
            .optional()
            .map_err(|error| db_error("failed to load ScanRun", error))?
            .ok_or_else(|| AppError::not_found("ScanRun not found"))?;

        Ok(ScanRunDetail {
            id: ScanRunId::new(row.0)?,
            watchlist_id: WatchlistId::new(row.1)?,
            preset_id: ScanPresetId::new(row.2)?,
            status: parse_scan_run_status(&row.3)?,
            base_trade_date: row.4,
            total_symbols: row.5 as u32,
            succeeded_symbols: row.6 as u32,
            failed_symbols: row.7 as u32,
            started_at: row.8,
            finished_at: row.9,
            preset_snapshot_json: serde_json::from_str(&row.10).map_err(|error| {
                AppError::internal("failed to parse preset snapshot JSON", error.to_string())
            })?,
            symbols_snapshot_json: serde_json::from_str(&row.11).map_err(|error| {
                AppError::internal("failed to parse symbols snapshot JSON", error.to_string())
            })?,
            retry_of_run_id: row.12.map(|s| ScanRunId::new(s).unwrap()),
        })
    }

    fn get_summary(&self, id: &ScanRunId) -> AppResult<ScanRunSummary> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT id, watchlist_id, preset_id, status, total_symbols,
                       succeeded_symbols, failed_symbols, started_at, finished_at
                 FROM scan_runs WHERE id = ?1",
            )
            .map_err(|error| db_error("failed to prepare ScanRun summary query", error))?;

        let row = statement
            .query_row(params![id.0.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, i64>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, Option<String>>(8)?,
                ))
            })
            .optional()
            .map_err(|error| db_error("failed to load ScanRun summary", error))?
            .ok_or_else(|| AppError::not_found("ScanRun not found"))?;

        Ok(ScanRunSummary {
            id: ScanRunId::new(row.0)?,
            watchlist_id: WatchlistId::new(row.1)?,
            preset_id: ScanPresetId::new(row.2)?,
            status: parse_scan_run_status(&row.3)?,
            total_symbols: row.4 as u32,
            succeeded_symbols: row.5 as u32,
            failed_symbols: row.6 as u32,
            started_at: row.7,
            finished_at: row.8,
        })
    }
}

fn parse_scan_run_status(value: &str) -> AppResult<ScanRunStatus> {
    match value {
        "pending" => Ok(ScanRunStatus::Pending),
        "running" => Ok(ScanRunStatus::Running),
        "completed" => Ok(ScanRunStatus::Completed),
        "cancelled" => Ok(ScanRunStatus::Cancelled),
        "failed" => Ok(ScanRunStatus::Failed),
        other => Err(AppError::database(
            "invalid ScanRun status stored in database",
            other.to_string(),
        )),
    }
}

fn status_db_value(status: ScanRunStatus) -> &'static str {
    match status {
        ScanRunStatus::Pending => "pending",
        ScanRunStatus::Running => "running",
        ScanRunStatus::Completed => "completed",
        ScanRunStatus::Cancelled => "cancelled",
        ScanRunStatus::Failed => "failed",
    }
}

fn validate_transition(
    connection: &rusqlite::Connection,
    id: &ScanRunId,
    expected: ScanRunStatus,
) -> AppResult<()> {
    let current: String = connection
        .query_row(
            "SELECT status FROM scan_runs WHERE id = ?1",
            params![id.0.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| db_error("failed to read ScanRun status", error))?
        .ok_or_else(|| AppError::not_found("ScanRun not found"))?;

    let current_status = parse_scan_run_status(&current)?;

    let allowed = match current_status {
        ScanRunStatus::Pending => vec![ScanRunStatus::Running],
        ScanRunStatus::Running => {
            vec![
                ScanRunStatus::Completed,
                ScanRunStatus::Cancelled,
                ScanRunStatus::Failed,
            ]
        }
        _ => {
            return Err(AppError::validation(format!(
                "ScanRun is in terminal state {} and cannot be modified",
                status_db_value(current_status)
            )));
        }
    };

    if !allowed.contains(&expected) {
        return Err(AppError::validation(format!(
            "cannot transition ScanRun from {} to {}",
            status_db_value(current_status),
            status_db_value(expected)
        )));
    }

    Ok(())
}

fn ensure_status(
    connection: &rusqlite::Connection,
    id: &ScanRunId,
    expected: ScanRunStatus,
) -> AppResult<()> {
    let current: String = connection
        .query_row(
            "SELECT status FROM scan_runs WHERE id = ?1",
            params![id.0.as_str()],
            |row| row.get(0),
        )
        .optional()
        .map_err(|error| db_error("failed to read ScanRun status", error))?
        .ok_or_else(|| AppError::not_found("ScanRun not found"))?;

    let current_status = parse_scan_run_status(&current)?;
    if current_status != expected {
        return Err(AppError::validation(format!(
            "ScanRun must be in {} state, currently {}",
            status_db_value(expected),
            status_db_value(current_status)
        )));
    }

    Ok(())
}

fn db_error(message: &'static str, error: rusqlite::Error) -> AppError {
    AppError::database(message, error.to_string())
}

#[cfg(test)]
#[path = "scan_run_tests.rs"]
mod tests;
