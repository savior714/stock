use crate::domain::{DailyBar, Symbol};
use crate::error::AppResult;

pub mod fetch_planner;
pub mod retry;
pub mod yahoo;

#[derive(Debug, Clone)]
pub struct DateRange {
    pub start: String,
    pub end: String,
}

impl DateRange {
    pub fn new(start: impl Into<String>, end: impl Into<String>) -> Self {
        Self {
            start: start.into(),
            end: end.into(),
        }
    }
}

#[async_trait::async_trait]
pub trait MarketDataProvider: Send + Sync {
    async fn fetch_daily_bars(
        &self,
        symbol: &Symbol,
        range: &DateRange,
    ) -> AppResult<Vec<DailyBar>>;
}
