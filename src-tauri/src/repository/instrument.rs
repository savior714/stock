use crate::db::Database;
use crate::domain::{AssetType, Instrument, Symbol};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension};

pub struct InstrumentRepository<'connection> {
    connection: &'connection mut Connection,
}

impl<'connection> InstrumentRepository<'connection> {
    pub fn new(database: &'connection mut Database) -> Self {
        Self {
            connection: database.connection_mut(),
        }
    }

    pub fn upsert(&mut self, instrument: &Instrument) -> AppResult<()> {
        self.connection
            .execute(
                "INSERT INTO instruments (symbol, provider_symbol, asset_type, exchange, is_active)
                 VALUES (?1, ?2, ?3, ?4, ?5)
                 ON CONFLICT(symbol) DO UPDATE SET
                   provider_symbol = excluded.provider_symbol,
                   asset_type = excluded.asset_type,
                   exchange = excluded.exchange,
                   is_active = excluded.is_active,
                   updated_at = CURRENT_TIMESTAMP",
                params![
                    instrument.symbol.as_str(),
                    &instrument.provider_symbol,
                    asset_type_db_value(instrument.asset_type),
                    instrument.exchange.as_deref(),
                    i64::from(instrument.is_active),
                ],
            )
            .map_err(|error| db_error("failed to upsert Instrument", error))?;
        Ok(())
    }

    pub fn get(&self, symbol: &Symbol) -> AppResult<Instrument> {
        let row = self
            .connection
            .query_row(
                "SELECT symbol, provider_symbol, asset_type, exchange, is_active
                 FROM instruments WHERE symbol = ?1",
                params![symbol.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, i64>(4)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| db_error("failed to load Instrument", error))?
            .ok_or_else(|| AppError::not_found("Instrument not found"))?;

        Ok(Instrument {
            symbol: Symbol::new(&row.0)?,
            provider_symbol: row.1,
            asset_type: parse_asset_type(&row.2)?,
            exchange: row.3,
            is_active: row.4 != 0,
        })
    }

    pub fn list_active(&self) -> AppResult<Vec<Instrument>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT symbol, provider_symbol, asset_type, exchange, is_active
                 FROM instruments
                 WHERE is_active = 1
                 ORDER BY lower(symbol), symbol",
            )
            .map_err(|error| db_error("failed to prepare Instruments list query", error))?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, Option<String>>(3)?,
                    row.get::<_, i64>(4)?,
                ))
            })
            .map_err(|error| db_error("failed to list active Instruments", error))?;

        let raw_rows = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| db_error("failed to read Instrument rows", error))?;

        raw_rows
            .into_iter()
            .map(
                |(symbol_str, provider_symbol, asset_type_str, exchange, is_active)| {
                    Ok(Instrument {
                        symbol: Symbol::new(&symbol_str)?,
                        provider_symbol,
                        asset_type: parse_asset_type(&asset_type_str)?,
                        exchange,
                        is_active: is_active != 0,
                    })
                },
            )
            .collect()
    }
}

fn asset_type_db_value(asset_type: AssetType) -> &'static str {
    match asset_type {
        AssetType::Stock => "stock",
        AssetType::Etf => "etf",
        AssetType::Adr => "adr",
    }
}

fn parse_asset_type(value: &str) -> AppResult<AssetType> {
    match value {
        "stock" => Ok(AssetType::Stock),
        "etf" => Ok(AssetType::Etf),
        "adr" => Ok(AssetType::Adr),
        _ => Err(AppError::database(
            "invalid asset type stored in database",
            value.to_string(),
        )),
    }
}

fn db_error(message: &'static str, error: rusqlite::Error) -> AppError {
    AppError::database(message, error.to_string())
}

#[cfg(test)]
#[path = "instrument_tests.rs"]
mod tests;
