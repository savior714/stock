use crate::db::Database;
use crate::domain::{DailyBar, PriceBasis, Symbol};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection};
use std::collections::HashMap;

pub struct DailyBarRepository<'connection> {
    connection: &'connection mut Connection,
}

impl<'connection> DailyBarRepository<'connection> {
    pub fn new(database: &'connection mut Database) -> Self {
        Self {
            connection: database.connection_mut(),
        }
    }

    /// Batch upsert multiple bars in one transaction.
    /// Duplicate (symbol, trade_date) is overwritten.
    /// Rejects if bars for the same symbol mix different price_basis values.
    pub fn upsert_batch(&mut self, bars: &[DailyBar]) -> AppResult<()> {
        if bars.is_empty() {
            return Ok(());
        }

        // Validate all bars before any DB operation
        for bar in bars {
            bar.validate()?;
        }

        // Check that all bars for the same symbol share the same price_basis
        let mut basis_map: HashMap<&str, PriceBasis> = HashMap::new();
        for bar in bars {
            if let Some(&existing) = basis_map.get(bar.symbol.as_str()) {
                if existing != bar.price_basis {
                    return Err(AppError::validation(format!(
                        "bars for {} mix different price_basis values",
                        bar.symbol
                    )));
                }
            } else {
                basis_map.insert(bar.symbol.as_str(), bar.price_basis);
            }
        }

        let transaction = self
            .connection
            .transaction()
            .map_err(|error| db_error("failed to start DailyBar upsert transaction", error))?;

        for bar in bars {
            transaction
                .execute(
                    "INSERT INTO daily_bars (symbol, trade_date, price_basis, open, high, low, close, volume)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(symbol, trade_date) DO UPDATE SET
                       open = excluded.open,
                       high = excluded.high,
                       low = excluded.low,
                       close = excluded.close,
                       volume = excluded.volume,
                       fetched_at = CURRENT_TIMESTAMP",
                    params![
                        bar.symbol.as_str(),
                        &bar.trade_date,
                        price_basis_db_value(bar.price_basis),
                        bar.open,
                        bar.high,
                        bar.low,
                        bar.close,
                        bar.volume as i64,
                    ],
                )
                .map_err(|error| db_error("failed to upsert DailyBar", error))?;
        }

        transaction
            .commit()
            .map_err(|error| db_error("failed to commit DailyBar upsert", error))?;

        Ok(())
    }

    /// Get earliest and latest trade_date for a symbol.
    pub fn date_range(&self, symbol: &Symbol) -> AppResult<Option<(String, String)>> {
        let (min_date, max_date): (Option<String>, Option<String>) = self
            .connection
            .query_row(
                "SELECT MIN(trade_date), MAX(trade_date) FROM daily_bars WHERE symbol = ?1",
                params![symbol.as_str()],
                |row| {
                    Ok((
                        row.get::<_, Option<String>>(0)?,
                        row.get::<_, Option<String>>(1)?,
                    ))
                },
            )
            .map_err(|error| db_error("failed to read DailyBar date range", error))?;

        match (min_date, max_date) {
            (Some(min), Some(max)) => Ok(Some((min, max))),
            _ => Ok(None),
        }
    }

    /// Load bars for a symbol within an inclusive date range, ordered ascending by trade_date.
    pub fn load_range(
        &self,
        symbol: &Symbol,
        start_date: &str,
        end_date: &str,
    ) -> AppResult<Vec<DailyBar>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT symbol, trade_date, price_basis, open, high, low, close, volume
                 FROM daily_bars
                 WHERE symbol = ?1 AND trade_date >= ?2 AND trade_date <= ?3
                 ORDER BY trade_date ASC",
            )
            .map_err(|error| db_error("failed to prepare DailyBar range query", error))?;

        let rows = statement
            .query_map(params![symbol.as_str(), start_date, end_date], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, f64>(3)?,
                    row.get::<_, f64>(4)?,
                    row.get::<_, f64>(5)?,
                    row.get::<_, f64>(6)?,
                    row.get::<_, i64>(7)?,
                ))
            })
            .map_err(|error| db_error("failed to load DailyBar range", error))?;

        let raw_rows = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| db_error("failed to read DailyBar rows", error))?;

        raw_rows
            .into_iter()
            .map(
                |(symbol_str, trade_date, price_basis_str, open, high, low, close, volume)| {
                    Ok(DailyBar {
                        symbol: Symbol::new(&symbol_str)?,
                        trade_date,
                        price_basis: parse_price_basis(&price_basis_str)?,
                        open,
                        high,
                        low,
                        close,
                        volume: volume as u64,
                    })
                },
            )
            .collect()
    }
}

fn price_basis_db_value(price_basis: PriceBasis) -> &'static str {
    match price_basis {
        PriceBasis::Raw => "raw",
        PriceBasis::SplitAdjusted => "split_adjusted",
    }
}

fn parse_price_basis(value: &str) -> AppResult<PriceBasis> {
    match value {
        "raw" => Ok(PriceBasis::Raw),
        "split_adjusted" => Ok(PriceBasis::SplitAdjusted),
        _ => Err(AppError::database(
            "invalid price_basis stored in database",
            value.to_string(),
        )),
    }
}

fn db_error(message: &'static str, error: rusqlite::Error) -> AppError {
    AppError::database(message, error.to_string())
}

#[cfg(test)]
#[path = "daily_bar_tests.rs"]
mod tests;
