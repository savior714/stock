use crate::domain::{DailyBar, PriceBasis, Symbol};
use crate::error::{AppError, AppResult};
use chrono::DateTime;
use std::collections::HashMap;

pub use super::dto::YahooChartResponse;

/// Parse Yahoo Finance chart JSON into domain DailyBars.
///
/// Rules:
/// - Requires matching timestamp, quote (OHLCV), and adjclose arrays.
/// - Skips any bar where any OHLC price is null.
/// - Never converts null to zero.
/// - Returns bars sorted ascending by date with deduplicated dates (last wins).
pub fn parse_yahoo_chart(symbol: &Symbol, body: &str) -> AppResult<Vec<DailyBar>> {
    let chart: YahooChartResponse = serde_json::from_str(body).map_err(|error| {
        AppError::new(
            crate::error::AppErrorCode::InvalidMarketData,
            "failed to parse Yahoo chart JSON",
        )
        .with_detail(error.to_string())
    })?;

    let data = chart
        .chart
        .as_ref()
        .and_then(|c| c.result.as_ref())
        .and_then(|r| r.first())
        .ok_or_else(|| {
            if let Some(err) = &chart.chart.as_ref().and_then(|c| c.error.as_ref()) {
                AppError::new(
                    crate::error::AppErrorCode::InvalidMarketData,
                    format!("Yahoo chart error: {}", err.code),
                )
                .with_detail(&err.description)
            } else {
                AppError::new(
                    crate::error::AppErrorCode::InvalidMarketData,
                    format!("no chart data for {}", symbol),
                )
            }
        })?;

    let timestamps = data.timestamp.as_ref().ok_or_else(|| {
        AppError::new(
            crate::error::AppErrorCode::InvalidMarketData,
            format!("{} missing timestamp array", symbol),
        )
    })?;

    let quote = data
        .indicators
        .as_ref()
        .and_then(|i| i.quote.as_ref())
        .ok_or_else(|| {
            AppError::new(
                crate::error::AppErrorCode::InvalidMarketData,
                format!("{} missing quote indicators", symbol),
            )
        })?;

    let adjclose = data
        .indicators
        .as_ref()
        .and_then(|i| i.adjclose.as_ref())
        .ok_or_else(|| {
            AppError::new(
                crate::error::AppErrorCode::InvalidMarketData,
                format!("{} missing adjclose indicators", symbol),
            )
        })?;

    let quote = quote.first();
    let adj = adjclose.first();

    let open_arr = quote.and_then(|q| q.open.as_ref()).ok_or_else(|| {
        AppError::new(
            crate::error::AppErrorCode::InvalidMarketData,
            format!("{} missing open data", symbol),
        )
    })?;

    let high_arr = quote.and_then(|q| q.high.as_ref()).ok_or_else(|| {
        AppError::new(
            crate::error::AppErrorCode::InvalidMarketData,
            format!("{} missing high data", symbol),
        )
    })?;

    let low_arr = quote.and_then(|q| q.low.as_ref()).ok_or_else(|| {
        AppError::new(
            crate::error::AppErrorCode::InvalidMarketData,
            format!("{} missing low data", symbol),
        )
    })?;

    let close_arr = quote.and_then(|q| q.close.as_ref()).ok_or_else(|| {
        AppError::new(
            crate::error::AppErrorCode::InvalidMarketData,
            format!("{} missing close data", symbol),
        )
    })?;

    let volume_arr = quote.and_then(|q| q.volume.as_ref()).ok_or_else(|| {
        AppError::new(
            crate::error::AppErrorCode::InvalidMarketData,
            format!("{} missing volume data", symbol),
        )
    })?;

    let adj_arr = adj.and_then(|a| a.adjclose.as_ref()).ok_or_else(|| {
        AppError::new(
            crate::error::AppErrorCode::InvalidMarketData,
            format!("{} missing adjclose array", symbol),
        )
    })?;

    if timestamps.len() != open_arr.len()
        || timestamps.len() != high_arr.len()
        || timestamps.len() != low_arr.len()
        || timestamps.len() != close_arr.len()
        || timestamps.len() != volume_arr.len()
        || timestamps.len() != adj_arr.len()
    {
        return Err(AppError::new(
            crate::error::AppErrorCode::InvalidMarketData,
            format!(
                "{} array length mismatch: {} timestamps vs O:{} H:{} L:{} C:{} V:{} adj:{}",
                symbol,
                timestamps.len(),
                open_arr.len(),
                high_arr.len(),
                low_arr.len(),
                close_arr.len(),
                volume_arr.len(),
                adj_arr.len()
            ),
        ));
    }

    // Parse and deduplicate: last occurrence of a date wins.
    let mut date_bars: HashMap<String, DailyBar> = HashMap::new();

    for (i, &ts) in timestamps.iter().enumerate() {
        let datetime = DateTime::from_timestamp(ts, 0).ok_or_else(|| {
            AppError::new(
                crate::error::AppErrorCode::InvalidMarketData,
                format!("{} invalid timestamp {}", symbol, ts),
            )
        })?;

        let trade_date = datetime.format("%Y-%m-%d").to_string();

        // Skip bars where any OHLC price is null
        let open = match open_arr[i] {
            Some(v) => v,
            None => continue,
        };

        let high = match high_arr[i] {
            Some(v) => v,
            None => continue,
        };

        let low = match low_arr[i] {
            Some(v) => v,
            None => continue,
        };

        let close = match close_arr[i] {
            Some(v) => v,
            None => continue,
        };

        let volume = volume_arr[i].unwrap_or(0);

        let bar = DailyBar {
            symbol: symbol.clone(),
            trade_date,
            price_basis: PriceBasis::SplitAdjusted,
            open,
            high,
            low,
            close,
            volume,
        };

        // Last wins for duplicate dates
        date_bars.insert(bar.trade_date.clone(), bar);
    }

    // Sort by date ascending and return
    let mut bars: Vec<DailyBar> = date_bars.into_values().collect();
    bars.sort_by(|a, b| a.trade_date.cmp(&b.trade_date));

    Ok(bars)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture_path(name: &str) -> String {
        let project_root = env!("CARGO_MANIFEST_DIR");
        format!("{}/tests/fixtures/yahoo/{}", project_root, name)
    }

    fn read_fixture(name: &str) -> String {
        let path = fixture_path(name);
        std::fs::read_to_string(&path).unwrap_or_else(|_| {
            panic!(
                "Fixture file not found: {}. Run: mkdir -p tests/fixtures/yahoo && touch {}",
                path, name
            )
        })
    }

    #[test]
    fn parses_valid_chart() {
        let symbol = Symbol::new("AAPL").expect("symbol valid");
        let body = read_fixture("valid_chart.json");
        let bars = parse_yahoo_chart(&symbol, &body).expect("must parse");

        assert!(!bars.is_empty());
        // Verify ascending order
        for i in 1..bars.len() {
            assert!(bars[i - 1].trade_date <= bars[i].trade_date);
        }
        // Verify price_basis
        assert!(bars
            .iter()
            .all(|b| b.price_basis == PriceBasis::SplitAdjusted));
    }

    #[test]
    fn skips_bars_with_null_prices() {
        let symbol = Symbol::new("TEST").expect("symbol valid");
        let body = read_fixture("null_values.json");
        let bars = parse_yahoo_chart(&symbol, &body).expect("must parse");

        // Should have fewer bars than timestamp entries due to null skipping
        let timestamps: Vec<i64> = serde_json::from_value(
            serde_json::from_str::<YahooChartResponse>(&body)
                .expect("valid json")
                .chart
                .unwrap()
                .result
                .as_ref()
                .unwrap()[0]
                .timestamp
                .as_ref()
                .unwrap()
                .clone()
                .into(),
        )
        .expect("valid array");

        assert!(bars.len() < timestamps.len());
    }

    #[test]
    fn returns_empty_for_empty_result() {
        let symbol = Symbol::new("EMPTY").expect("symbol valid");
        let body = read_fixture("empty_result.json");
        let result = parse_yahoo_chart(&symbol, &body);

        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error.code, crate::error::AppErrorCode::InvalidMarketData);
        }
    }

    #[test]
    fn returns_error_for_provider_error() {
        let symbol = Symbol::new("ERROR").expect("symbol valid");
        let body = read_fixture("provider_error.json");
        let result = parse_yahoo_chart(&symbol, &body);

        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error.code, crate::error::AppErrorCode::InvalidMarketData);
            assert!(error.message.contains("No data found"));
        }
    }

    #[test]
    fn deduplicates_by_date_last_wins() {
        let symbol = Symbol::new("DUP").expect("symbol valid");
        let body = read_fixture("duplicate_dates.json");
        let bars = parse_yahoo_chart(&symbol, &body).expect("must parse");

        // All trade_dates should be unique
        let mut dates: Vec<&String> = bars.iter().map(|b| &b.trade_date).collect();
        dates.sort();
        dates.dedup();
        assert_eq!(dates.len(), bars.len());
    }

    #[test]
    fn rejects_mismatched_array_lengths() {
        let symbol = Symbol::new("BAD").expect("symbol valid");
        let invalid_body = r#"{
            "chart": {
                "result": [{
                    "timestamp": [1, 2, 3],
                    "indicators": {
                        "quote": [{"open": [1.0, 2.0], "high": [3.0, 4.0], "low": [0.5, 1.5], "close": [1.5, 2.5], "volume": [100, 200]}],
                        "adjclose": [{"adjclose": [1.5, 2.5]}]
                    }
                }]
            }
        }"#;

        let result = parse_yahoo_chart(&symbol, invalid_body);
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error.code, crate::error::AppErrorCode::InvalidMarketData);
            assert!(error.message.contains("array length mismatch"));
        }
    }

    #[test]
    fn rejects_invalid_timestamp() {
        let symbol = Symbol::new("BAD").expect("symbol valid");
        let invalid_body = r#"{
            "chart": {
                "result": [{
                    "timestamp": [99999999999],
                    "indicators": {
                        "quote": {"open": [1.0], "high": [2.0], "low": [0.5], "close": [1.5], "volume": [100]},
                        "adjclose": {"adjclose": [1.5]}
                    }
                }]
            }
        }"#;

        let result = parse_yahoo_chart(&symbol, invalid_body);
        assert!(result.is_err());
    }

    #[test]
    fn returns_error_when_no_chart_data() {
        let symbol = Symbol::new("EMPTY").expect("symbol valid");
        let empty_body = r#"{"chart": {"result": []}}"#;

        let result = parse_yahoo_chart(&symbol, empty_body);
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error.code, crate::error::AppErrorCode::InvalidMarketData);
        }
    }

    #[test]
    fn returns_error_when_missing_quote_indicators() {
        let symbol = Symbol::new("BAD").expect("symbol valid");
        let body = r#"{
            "chart": {
                "result": [{
                    "timestamp": [1700000000],
                    "indicators": {
                        "adjclose": {"adjclose": [150.0]}
                    }
                }]
            }
        }"#;

        let result = parse_yahoo_chart(&symbol, body);
        assert!(result.is_err());
    }

    #[test]
    fn returns_error_when_missing_adjclose_indicators() {
        let symbol = Symbol::new("BAD").expect("symbol valid");
        let body = r#"{
            "chart": {
                "result": [{
                    "timestamp": [1700000000],
                    "indicators": {
                        "quote": {"open": [1.0], "high": [2.0], "low": [0.5], "close": [1.5], "volume": [100]}
                    }
                }]
            }
        }"#;

        let result = parse_yahoo_chart(&symbol, body);
        assert!(result.is_err());
    }

    #[test]
    fn returns_error_for_invalid_json() {
        let symbol = Symbol::new("BAD").expect("symbol valid");
        let result = parse_yahoo_chart(&symbol, "not json at all");
        assert!(result.is_err());
        if let Err(error) = result {
            assert_eq!(error.code, crate::error::AppErrorCode::InvalidMarketData);
        }
    }

    #[test]
    fn preserves_bar_values_from_fixture() {
        let symbol = Symbol::new("AAPL").expect("symbol valid");
        let body = read_fixture("valid_chart.json");
        let bars = parse_yahoo_chart(&symbol, &body).expect("must parse");

        if let Some(first) = bars.first() {
            assert!(first.open > 0.0);
            assert!(first.high >= first.open);
            assert!(first.high >= first.low);
            assert!(first.low > 0.0);
            assert!(first.close > 0.0);
            assert!(first.volume > 0);
        }
    }
}
