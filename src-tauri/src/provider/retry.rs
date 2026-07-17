use super::{DateRange, MarketDataProvider};
use crate::domain::{DailyBar, Symbol};
use crate::error::{AppError, AppErrorCode, AppResult};
use std::sync::Arc;
use tokio::sync::Semaphore;

const MAX_CONCURRENT: usize = 4;
const MAX_ATTEMPTS: u32 = 3;
const BASE_DELAY_MS: u64 = 500;
const MAX_JITTER_MS: u64 = 1000;

/// A provider wrapper that adds bounded concurrency and retry logic.
pub struct RetryConcurrentProvider<P> {
    inner: P,
    semaphore: Arc<Semaphore>,
}

impl<P: MarketDataProvider> RetryConcurrentProvider<P> {
    pub fn new(inner: P) -> Self {
        Self {
            inner,
            semaphore: Arc::new(Semaphore::new(MAX_CONCURRENT)),
        }
    }

    async fn fetch_with_retry_and_concurrency(
        &self,
        symbol: &Symbol,
        range: &DateRange,
    ) -> AppResult<Vec<DailyBar>> {
        let _permit = self.semaphore.acquire().await.map_err(|error| {
            AppError::internal("concurrency semaphore closed", error.to_string())
        })?;

        let mut last_error: Option<AppError> = None;

        for attempt in 1..=MAX_ATTEMPTS {
            match self.inner.fetch_daily_bars(symbol, range).await {
                Ok(bars) => return Ok(bars),
                Err(error) => {
                    if !error.retryable || attempt == MAX_ATTEMPTS {
                        return Err(error);
                    }
                    last_error = Some(error);
                    let delay = calculate_delay(attempt);
                    tokio::time::sleep(delay).await;
                }
            }
        }

        Err(last_error.unwrap_or_else(|| {
            AppError::new(
                AppErrorCode::ProviderUnavailable,
                format!("{symbol} fetch exhausted all attempts"),
            )
        }))
    }
}

fn calculate_delay(attempt: u32) -> std::time::Duration {
    let exponential = BASE_DELAY_MS * 2u64.pow(attempt - 1);
    let jitter = rand_jitter(MAX_JITTER_MS);
    std::time::Duration::from_millis(exponential + jitter)
}

fn rand_jitter(max_ms: u64) -> u64 {
    // Simple deterministic jitter for testing; in production use a proper RNG
    let max_u32 = max_ms as u32;
    (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .subsec_millis()
        % (max_u32 + 1)) as u64
}

#[async_trait::async_trait]
impl<P: MarketDataProvider + Send + Sync> MarketDataProvider for RetryConcurrentProvider<P> {
    async fn fetch_daily_bars(
        &self,
        symbol: &Symbol,
        range: &DateRange,
    ) -> AppResult<Vec<DailyBar>> {
        self.fetch_with_retry_and_concurrency(symbol, range).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::yahoo::YahooMarketDataProvider;

    #[test]
    fn creates_provider_with_semaphore() {
        let yahoo = YahooMarketDataProvider::new();
        let provider = RetryConcurrentProvider::new(yahoo);
        // Verify the semaphore has the correct permit count
        assert_eq!(provider.semaphore.available_permits(), MAX_CONCURRENT);
    }

    #[test]
    fn calculates_delay_increases_with_attempt() {
        let delay1 = calculate_delay(1);
        let delay2 = calculate_delay(2);
        let delay3 = calculate_delay(3);

        // Each attempt should have at least the exponential base (jitter is small relative)
        assert!(delay2 > delay1);
        assert!(delay3 > delay2);
    }

    #[test]
    fn max_concurrent_is_four() {
        assert_eq!(MAX_CONCURRENT, 4);
    }

    #[test]
    fn max_attempts_is_three() {
        assert_eq!(MAX_ATTEMPTS, 3);
    }
}
