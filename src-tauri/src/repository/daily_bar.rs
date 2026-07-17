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
    /// Rejects when a symbol mixes price bases inside the batch or with stored rows.
    pub fn upsert_batch(&mut self, bars: &[DailyBar]) -> AppResult<()> {
        if bars.is_empty() {
            return Ok(());
        }

        for bar in bars {
            bar.validate()?;
            validate_trade_date(&bar.trade_date)?;
        }

        let mut basis_by_symbol: HashMap<&str, PriceBasis> = HashMap::new();
        for bar in bars {
            if let Some(&existing) = basis_by_symbol.get(bar.symbol.as_str()) {
                if existing != bar.price_basis {
                    return Err(AppError::validation(format!(
                        "bars for {} mix different price_basis values",
                        bar.symbol
                    )));
                }
            } else {
                basis_by_symbol.insert(bar.symbol.as_str(), bar.price_basis);
            }
        }

        let transaction = self
            .connection
            .transaction()
            .map_err(|error| db_error("failed to start DailyBar upsert transaction", error))?;

        for (&symbol, &incoming_basis) in &basis_by_symbol {
            let (basis_count, stored_basis): (i64, Option<String>) = transaction
                .query_row(
                    "SELECT COUNT(DISTINCT price_basis), MIN(price_basis)
                     FROM daily_bars
                     WHERE symbol = ?1",
                    params![symbol],
                    |row| Ok((row.get(0)?, row.get(1)?)),
                )
                .map_err(|error| db_error("failed to inspect stored DailyBar price basis", error))?;

            if basis_count > 1 {
                return Err(AppError::database(
                    "stored DailyBar rows mix price_basis values",
                    symbol.to_string(),
                ));
            }

            if let Some(stored_basis) = stored_basis {
                let stored_basis = parse_price_basis(&stored_basis)?;
                if stored_basis != incoming_basis {
                    return Err(AppError::validation(format!(
                        "bars for {symbol} use {}, but stored rows use {}",
                        price_basis_db_value(incoming_basis),
                        price_basis_db_value(stored_basis)
                    )));
                }
            }
        }

        for bar in bars {
            transaction
                .execute(
                    "INSERT INTO daily_bars (symbol, trade_date, price_basis, open, high, low, close, volume)
                     VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
                     ON CONFLICT(symbol, trade_date) DO UPDATE SET
                       price_basis = excluded.price_basis,
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
        validate_trade_date(start_date)?;
        validate_trade_date(end_date)?;
        if start_date > end_date {
            return Err(AppError::validation(
                "DailyBar range start_date must not be after end_date",
            ));
        }

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
                    validate_trade_date(&trade_date)?;
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

fn validate_trade_date(value: &str) -> AppResult<()> {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return Err(AppError::validation("trade_date must use YYYY-MM-DD"));
    }

    let year = value[0..4]
        .parse::<u32>()
        .map_err(|_| AppError::validation("trade_date year must be numeric"))?;
    let month = value[5..7]
        .parse::<u32>()
        .map_err(|_| AppError::validation("trade_date month must be numeric"))?;
    let day = value[8..10]
        .parse::<u32>()
        .map_err(|_| AppError::validation("trade_date day must be numeric"))?;

    if year == 0 || !(1..=12).contains(&month) {
        return Err(AppError::validation("trade_date is not a valid calendar date"));
    }

    let leap_year = year % 4 == 0 && (year % 100 != 0 || year % 400 == 0);
    let max_day = match month {
        2 if leap_year => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    };

    if day == 0 || day > max_day {
        return Err(AppError::validation("trade_date is not a valid calendar date"));
    }

    Ok(())
}

fn db_error(message: &'static str, error: rusqlite::Error) -> AppError {
    AppError::database(message, error.to_string())
}

#[cfg(test)]
#[path = "daily_bar_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "daily_bar_basis_tests.rs"]
mod basis_tests;
