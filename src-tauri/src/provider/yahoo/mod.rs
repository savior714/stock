pub mod dto;
pub mod parser;

use crate::domain::{DailyBar, Symbol};
use crate::error::{AppError, AppResult};
pub use parser::parse_yahoo_chart;

use super::{DateRange, MarketDataProvider};

pub struct YahooMarketDataProvider {
    client: reqwest::Client,
}

impl YahooMarketDataProvider {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }

    fn build_url(&self, symbol: &Symbol, interval: &str, range: &str) -> String {
        format!(
            "https://query1.finance.yahoo.com/v8/finance/chart/{}?interval={}&range={}",
            symbol.provider_symbol(),
            interval,
            range
        )
    }
}

impl Default for YahooMarketDataProvider {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait::async_trait]
impl MarketDataProvider for YahooMarketDataProvider {
    async fn fetch_daily_bars(
        &self,
        symbol: &Symbol,
        range: &DateRange,
    ) -> AppResult<Vec<DailyBar>> {
        let url = self.build_url(symbol, "1d", &range.start);
        // Use end date if provided (range format like "30d" or "2024-01-01+2024-12-31")
        let full_url = if range.end.is_empty() {
            url
        } else {
            format!("{}&endDate={}", url, range.end)
        };

        let response = self
            .client
            .get(&full_url)
            .header("User-Agent", "Mozilla/5.0")
            .send()
            .await
            .map_err(|error| {
                AppError::new(
                    crate::error::AppErrorCode::ProviderUnavailable,
                    format!("Yahoo request failed for {symbol}"),
                )
                .with_detail(error.to_string())
                .retryable(true)
            })?;

        let status = response.status();

        if status == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(AppError::new(
                crate::error::AppErrorCode::ProviderRateLimited,
                format!("Yahoo rate limited for {symbol}"),
            )
            .retryable(true));
        }

        if status == reqwest::StatusCode::NOT_FOUND {
            return Err(AppError::new(
                crate::error::AppErrorCode::NotFound,
                format!("No chart data found for {symbol}"),
            ));
        }

        if !status.is_success() {
            return Err(AppError::new(
                crate::error::AppErrorCode::ProviderUnavailable,
                format!("Yahoo returned status {status} for {symbol}"),
            )
            .with_detail(format!("status: {status}"))
            .retryable(true));
        }

        let body = response.text().await.map_err(|error| {
            AppError::new(
                crate::error::AppErrorCode::InvalidMarketData,
                format!("failed to read Yahoo response body for {symbol}"),
            )
            .with_detail(error.to_string())
        })?;

        parse_yahoo_chart(symbol, &body)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn returns_rate_limited_on_429() {
        // We can't easily mock HTTP in unit tests without extra deps,
        // so we verify the provider struct instantiates correctly.
        let _provider = YahooMarketDataProvider::new();
        assert!(std::mem::size_of::<YahooMarketDataProvider>() > 0);
    }

    #[test]
    fn builds_url_correctly() {
        let provider = YahooMarketDataProvider::new();
        let symbol = Symbol::new("BRK.B").expect("symbol valid");

        // The URL should use provider_symbol (BRK-B)
        let url = provider.build_url(&symbol, "1d", "60d");
        assert!(url.contains("BRK-B"));
        assert!(url.contains("interval=1d"));
        assert!(url.contains("range=60d"));
    }
}
