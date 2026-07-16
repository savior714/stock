use crate::db::Database;
use crate::domain::{IndicatorValues, ScanResult, ScanRunId, SignalMatch, Symbol};
use crate::error::{AppError, AppResult};
use rusqlite::params;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultMatchFilter {
    None,
    And,
    Or,
}

pub struct ScanResultRepository<'connection> {
    connection: &'connection mut rusqlite::Connection,
}

impl<'connection> ScanResultRepository<'connection> {
    pub fn new(database: &'connection mut Database) -> Self {
        Self {
            connection: database.connection_mut(),
        }
    }

    pub fn upsert(&mut self, result: &ScanResult) -> AppResult<()> {
        let signal_json = serde_json::to_string(&result.matches).map_err(|error| {
            AppError::internal("failed to encode SignalMatch JSON", error.to_string())
        })?;

        let matched_count = result.matches.iter().filter(|m| m.matched).count() as i64;

        self.connection
            .execute(
                "INSERT INTO scan_results (
                    run_id, symbol, trade_date, current_price,
                    rsi, mfi, bollinger_lower, bollinger_middle, bollinger_upper,
                    signal_flags_json, matched_condition_count,
                    all_conditions_matched, any_condition_matched, data_stale
                ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)
                ON CONFLICT(run_id, symbol) DO UPDATE SET
                    trade_date = excluded.trade_date,
                    current_price = excluded.current_price,
                    rsi = excluded.rsi,
                    mfi = excluded.mfi,
                    bollinger_lower = excluded.bollinger_lower,
                    bollinger_middle = excluded.bollinger_middle,
                    bollinger_upper = excluded.bollinger_upper,
                    signal_flags_json = excluded.signal_flags_json,
                    matched_condition_count = excluded.matched_condition_count,
                    all_conditions_matched = excluded.all_conditions_matched,
                    any_condition_matched = excluded.any_condition_matched,
                    data_stale = excluded.data_stale",
                params![
                    result.run_id.0.as_str(),
                    result.symbol.as_str(),
                    &result.trade_date,
                    result.current_price,
                    result.indicators.rsi,
                    result.indicators.mfi,
                    result.indicators.bollinger_lower,
                    result.indicators.bollinger_middle,
                    result.indicators.bollinger_upper,
                    signal_json,
                    matched_count,
                    bool_to_int(result.all_conditions_matched),
                    bool_to_int(result.any_condition_matched),
                    bool_to_int(result.data_stale),
                ],
            )
            .map_err(|error| db_error("failed to upsert ScanResult", error))?;

        Ok(())
    }

    pub fn get_by_run(
        &self,
        run_id: &ScanRunId,
        filter: ResultMatchFilter,
    ) -> AppResult<Vec<ScanResult>> {
        let where_clause = match filter {
            ResultMatchFilter::And => "WHERE all_conditions_matched = 1 AND run_id = ?1",
            ResultMatchFilter::Or => "WHERE any_condition_matched = 1 AND run_id = ?1",
            ResultMatchFilter::None => "WHERE run_id = ?1",
        };

        let query = format!(
            "SELECT run_id, symbol, trade_date, current_price, \
             rsi, mfi, bollinger_lower, bollinger_middle, bollinger_upper, \
             signal_flags_json, all_conditions_matched, any_condition_matched, data_stale \
             FROM scan_results {where_clause} \
             ORDER BY symbol COLLATE NOCASE"
        );

        let mut statement = self
            .connection
            .prepare(&query)
            .map_err(|error| db_error("failed to prepare ScanResult query", error))?;

        let raw_rows = statement
            .query_map(params![run_id.0.as_str()], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, f64>(3)?,
                    row.get::<_, Option<f64>>(4)?,
                    row.get::<_, Option<f64>>(5)?,
                    row.get::<_, Option<f64>>(6)?,
                    row.get::<_, Option<f64>>(7)?,
                    row.get::<_, Option<f64>>(8)?,
                    row.get::<_, String>(9)?,
                    row.get::<_, i32>(10)?,
                    row.get::<_, i32>(11)?,
                    row.get::<_, i32>(12)?,
                ))
            })
            .map_err(|error| db_error("failed to query ScanResults", error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| db_error("failed to read ScanResult rows", error))?;

        raw_rows
            .into_iter()
            .map(
                |(
                    run_id_str,
                    symbol_str,
                    trade_date,
                    current_price,
                    rsi,
                    mfi,
                    bollinger_lower,
                    bollinger_middle,
                    bollinger_upper,
                    signal_json,
                    all_matched,
                    any_matched,
                    stale,
                )| {
                    let indicators = IndicatorValues {
                        rsi,
                        mfi,
                        bollinger_lower,
                        bollinger_middle,
                        bollinger_upper,
                    };

                    let matches: Vec<SignalMatch> =
                        serde_json::from_str(&signal_json).map_err(|error| {
                            AppError::internal(
                                "failed to decode SignalMatch JSON",
                                error.to_string(),
                            )
                        })?;

                    Ok(ScanResult {
                        run_id: ScanRunId::new(run_id_str)?,
                        symbol: Symbol::new(&symbol_str)?,
                        trade_date,
                        current_price,
                        indicators,
                        matches,
                        all_conditions_matched: all_matched == 1,
                        any_condition_matched: any_matched == 1,
                        data_stale: stale == 1,
                    })
                },
            )
            .collect()
    }

    pub fn update_stale_flags(
        &mut self,
        run_id: &ScanRunId,
        base_trade_date: &str,
    ) -> AppResult<()> {
        self.connection
            .execute(
                "UPDATE scan_results
                 SET data_stale = CASE
                     WHEN trade_date < ?1 THEN 1
                     ELSE 0
                 END
                 WHERE run_id = ?2",
                params![base_trade_date, run_id.0.as_str()],
            )
            .map_err(|error| db_error("failed to update stale flags", error))?;

        Ok(())
    }
}

fn bool_to_int(value: bool) -> i32 {
    if value {
        1
    } else {
        0
    }
}

fn db_error(message: &'static str, error: rusqlite::Error) -> AppError {
    AppError::database(message, error.to_string())
}

#[cfg(test)]
#[path = "scan_result_tests.rs"]
mod tests;
