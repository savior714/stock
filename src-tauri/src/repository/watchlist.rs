use crate::db::Database;
use crate::domain::{Symbol, WatchlistId};
use crate::error::{AppError, AppResult};
use rusqlite::{params, Connection, OptionalExtension, Transaction};
use serde::Serialize;
use std::collections::HashSet;

const MAX_WATCHLIST_NAME_LENGTH: usize = 80;
const MAX_DESCRIPTION_LENGTH: usize = 500;
const MAX_SYMBOLS_PER_WATCHLIST: usize = 500;

#[derive(Debug, Clone)]
pub struct WatchlistWrite {
    pub name: String,
    pub description: Option<String>,
    pub symbols: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistSummary {
    pub id: WatchlistId,
    pub name: String,
    pub description: Option<String>,
    pub symbol_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistDetail {
    pub id: WatchlistId,
    pub name: String,
    pub description: Option<String>,
    pub symbols: Vec<Symbol>,
}

struct NormalizedWatchlistWrite {
    name: String,
    description: Option<String>,
    symbols: Vec<Symbol>,
}

pub struct WatchlistRepository<'connection> {
    connection: &'connection mut Connection,
}

impl<'connection> WatchlistRepository<'connection> {
    pub fn new(database: &'connection mut Database) -> Self {
        Self {
            connection: database.connection_mut(),
        }
    }

    pub fn list(&self) -> AppResult<Vec<WatchlistSummary>> {
        let mut statement = self
            .connection
            .prepare(
                "SELECT w.id, w.name, w.description, COUNT(ws.symbol)
                 FROM watchlists w
                 LEFT JOIN watchlist_symbols ws ON ws.watchlist_id = w.id
                 GROUP BY w.id, w.name, w.description
                 ORDER BY lower(w.name), w.id",
            )
            .map_err(|error| db_error("failed to prepare Watchlist list query", error))?;

        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(|error| db_error("failed to list Watchlists", error))?;

        let raw_rows = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| db_error("failed to read Watchlist rows", error))?;

        raw_rows
            .into_iter()
            .map(|(id, name, description, symbol_count)| {
                Ok(WatchlistSummary {
                    id: WatchlistId::new(id)?,
                    name,
                    description,
                    symbol_count: symbol_count as u32,
                })
            })
            .collect()
    }

    pub fn get(&self, id: &WatchlistId) -> AppResult<WatchlistDetail> {
        let watchlist = self
            .connection
            .query_row(
                "SELECT id, name, description FROM watchlists WHERE id = ?1",
                params![id.0.as_str()],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| db_error("failed to load Watchlist", error))?
            .ok_or_else(|| AppError::not_found("Watchlist not found"))?;

        let mut statement = self
            .connection
            .prepare(
                "SELECT symbol FROM watchlist_symbols
                 WHERE watchlist_id = ?1
                 ORDER BY sort_order, symbol",
            )
            .map_err(|error| db_error("failed to prepare Watchlist symbol query", error))?;
        let rows = statement
            .query_map(params![id.0.as_str()], |row| row.get::<_, String>(0))
            .map_err(|error| db_error("failed to load Watchlist symbols", error))?;
        let raw_symbols = rows
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| db_error("failed to read Watchlist symbols", error))?;
        let symbols = raw_symbols
            .into_iter()
            .map(Symbol::new)
            .collect::<AppResult<Vec<_>>>()?;

        Ok(WatchlistDetail {
            id: WatchlistId::new(watchlist.0)?,
            name: watchlist.1,
            description: watchlist.2,
            symbols,
        })
    }

    pub fn create(&mut self, input: WatchlistWrite) -> AppResult<WatchlistDetail> {
        let normalized = normalize_write(input)?;
        if self.name_exists(&normalized.name, None)? {
            return Err(AppError::conflict("a Watchlist with this name already exists"));
        }

        let transaction = self
            .connection
            .transaction()
            .map_err(|error| db_error("failed to start Watchlist transaction", error))?;
        let raw_id: String = transaction
            .query_row("SELECT lower(hex(randomblob(16)))", [], |row| row.get(0))
            .map_err(|error| db_error("failed to generate Watchlist id", error))?;
        let id = WatchlistId::new(raw_id)?;

        transaction
            .execute(
                "INSERT INTO watchlists (id, name, description) VALUES (?1, ?2, ?3)",
                params![id.0.as_str(), &normalized.name, normalized.description.as_deref()],
            )
            .map_err(|error| db_error("failed to create Watchlist", error))?;
        replace_members(&transaction, &id, &normalized.symbols)?;
        transaction
            .commit()
            .map_err(|error| db_error("failed to commit Watchlist creation", error))?;

        self.get(&id)
    }

    pub fn update(
        &mut self,
        id: &WatchlistId,
        input: WatchlistWrite,
    ) -> AppResult<WatchlistDetail> {
        let normalized = normalize_write(input)?;
        if self.name_exists(&normalized.name, Some(id))? {
            return Err(AppError::conflict("a Watchlist with this name already exists"));
        }

        let transaction = self
            .connection
            .transaction()
            .map_err(|error| db_error("failed to start Watchlist transaction", error))?;
        let changed = transaction
            .execute(
                "UPDATE watchlists
                 SET name = ?1, description = ?2, updated_at = CURRENT_TIMESTAMP
                 WHERE id = ?3",
                params![&normalized.name, normalized.description.as_deref(), id.0.as_str()],
            )
            .map_err(|error| db_error("failed to update Watchlist", error))?;
        if changed == 0 {
            return Err(AppError::not_found("Watchlist not found"));
        }

        replace_members(&transaction, id, &normalized.symbols)?;
        transaction
            .commit()
            .map_err(|error| db_error("failed to commit Watchlist update", error))?;

        self.get(id)
    }

    pub fn delete(&mut self, id: &WatchlistId) -> AppResult<()> {
        let changed = self
            .connection
            .execute("DELETE FROM watchlists WHERE id = ?1", params![id.0.as_str()])
            .map_err(|error| db_error("failed to delete Watchlist", error))?;

        if changed == 0 {
            return Err(AppError::not_found("Watchlist not found"));
        }
        Ok(())
    }

    fn name_exists(&self, name: &str, excluding_id: Option<&WatchlistId>) -> AppResult<bool> {
        let count: i64 = match excluding_id {
            Some(id) => self.connection.query_row(
                "SELECT COUNT(*) FROM watchlists WHERE name = ?1 COLLATE NOCASE AND id <> ?2",
                params![name, id.0.as_str()],
                |row| row.get(0),
            ),
            None => self.connection.query_row(
                "SELECT COUNT(*) FROM watchlists WHERE name = ?1 COLLATE NOCASE",
                params![name],
                |row| row.get(0),
            ),
        }
        .map_err(|error| db_error("failed to check Watchlist name", error))?;

        Ok(count > 0)
    }
}

fn normalize_write(input: WatchlistWrite) -> AppResult<NormalizedWatchlistWrite> {
    let name = input.name.trim().to_string();
    if name.is_empty() || name.len() > MAX_WATCHLIST_NAME_LENGTH {
        return Err(AppError::validation(format!(
            "Watchlist name must contain 1 to {MAX_WATCHLIST_NAME_LENGTH} characters"
        )));
    }

    let description = input
        .description
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());
    if description
        .as_ref()
        .is_some_and(|value| value.len() > MAX_DESCRIPTION_LENGTH)
    {
        return Err(AppError::validation(format!(
            "Watchlist description must not exceed {MAX_DESCRIPTION_LENGTH} characters"
        )));
    }

    let mut seen = HashSet::new();
    let mut symbols = Vec::new();
    for raw_symbol in input.symbols {
        let symbol = Symbol::new(raw_symbol)?;
        if seen.insert(symbol.as_str().to_string()) {
            symbols.push(symbol);
        }
    }
    if symbols.len() > MAX_SYMBOLS_PER_WATCHLIST {
        return Err(AppError::validation(format!(
            "a Watchlist may contain at most {MAX_SYMBOLS_PER_WATCHLIST} symbols"
        )));
    }

    Ok(NormalizedWatchlistWrite {
        name,
        description,
        symbols,
    })
}

fn replace_members(
    transaction: &Transaction<'_>,
    id: &WatchlistId,
    symbols: &[Symbol],
) -> AppResult<()> {
    transaction
        .execute(
            "DELETE FROM watchlist_symbols WHERE watchlist_id = ?1",
            params![id.0.as_str()],
        )
        .map_err(|error| db_error("failed to clear Watchlist symbols", error))?;

    for (index, symbol) in symbols.iter().enumerate() {
        transaction
            .execute(
                "INSERT INTO instruments (symbol, provider_symbol, asset_type, is_active)
                 VALUES (?1, ?2, 'stock', 1)
                 ON CONFLICT(symbol) DO UPDATE SET
                   provider_symbol = excluded.provider_symbol,
                   is_active = 1,
                   updated_at = CURRENT_TIMESTAMP",
                params![symbol.as_str(), symbol.provider_symbol()],
            )
            .map_err(|error| db_error("failed to upsert Watchlist instrument", error))?;
        transaction
            .execute(
                "INSERT INTO watchlist_symbols (watchlist_id, symbol, sort_order)
                 VALUES (?1, ?2, ?3)",
                params![id.0.as_str(), symbol.as_str(), index as i64],
            )
            .map_err(|error| db_error("failed to add Watchlist symbol", error))?;
    }
    Ok(())
}

fn db_error(message: &'static str, error: rusqlite::Error) -> AppError {
    AppError::database(message, error.to_string())
}

#[cfg(test)]
#[path = "watchlist_tests.rs"]
mod tests;
