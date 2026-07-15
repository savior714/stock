use crate::db::Database;
use crate::error::{AppError, AppResult};
use std::sync::Mutex;

pub struct AppState {
    database: Mutex<Database>,
}

impl AppState {
    pub fn new(database: Database) -> Self {
        Self {
            database: Mutex::new(database),
        }
    }

    pub fn with_database<T>(
        &self,
        operation: impl FnOnce(&mut Database) -> AppResult<T>,
    ) -> AppResult<T> {
        let mut database = self.database.lock().map_err(|error| {
            AppError::internal("failed to lock application database", error.to_string())
        })?;

        operation(&mut database)
    }
}
