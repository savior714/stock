use crate::error::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fmt;

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
        #[serde(transparent)]
        pub struct $name(pub String);

        impl $name {
            pub fn new(value: impl Into<String>) -> AppResult<Self> {
                let value = value.into();
                if value.trim().is_empty() {
                    return Err(AppError::validation(concat!(stringify!($name), " must not be empty")));
                }
                Ok(Self(value))
            }
        }
    };
}

string_id!(WatchlistId);
string_id!(ScanPresetId);
string_id!(SignalConditionId);
string_id!(ScanRunId);

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Symbol(String);

impl Symbol {
    pub fn new(value: impl AsRef<str>) -> AppResult<Self> {
        let normalized = value.as_ref().trim().to_ascii_uppercase();
        let valid_length = (1..=15).contains(&normalized.len());
        let valid_chars = normalized
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '.' | '-'));

        if !valid_length || !valid_chars {
            return Err(AppError::validation(format!(
                "invalid US market symbol: {}",
                value.as_ref()
            )));
        }

        Ok(Self(normalized))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn provider_symbol(&self) -> String {
        self.0.replace('.', "-")
    }
}

impl fmt::Display for Symbol {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssetType {
    Stock,
    Etf,
    Adr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PriceBasis {
    Raw,
    SplitAdjusted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TriggerMode {
    Current,
    Cross,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IndicatorKind {
    Bollinger,
    Rsi,
    Mfi,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SignalSide {
    Lower,
    Upper,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanRunStatus {
    Pending,
    Running,
    Completed,
    Cancelled,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Instrument {
    pub symbol: Symbol,
    pub provider_symbol: String,
    pub asset_type: AssetType,
    pub exchange: Option<String>,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DailyBar {
    pub symbol: Symbol,
    pub trade_date: String,
    pub price_basis: PriceBasis,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: u64,
}

impl DailyBar {
    pub fn validate(&self) -> AppResult<()> {
        if self.trade_date.len() != 10 {
            return Err(AppError::validation("trade_date must use YYYY-MM-DD"));
        }

        let prices = [self.open, self.high, self.low, self.close];
        if prices.iter().any(|price| !price.is_finite() || *price <= 0.0) {
            return Err(AppError::new(
                crate::error::AppErrorCode::InvalidMarketData,
                format!("{} contains a non-positive or non-finite price", self.symbol),
            ));
        }

        if self.high < self.open || self.high < self.low || self.high < self.close {
            return Err(AppError::new(
                crate::error::AppErrorCode::InvalidMarketData,
                format!("{} high is inconsistent with OHLC", self.symbol),
            ));
        }

        if self.low > self.open || self.low > self.high || self.low > self.close {
            return Err(AppError::new(
                crate::error::AppErrorCode::InvalidMarketData,
                format!("{} low is inconsistent with OHLC", self.symbol),
            ));
        }

        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Watchlist {
    pub id: WatchlistId,
    pub name: String,
    pub description: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WatchlistMember {
    pub watchlist_id: WatchlistId,
    pub symbol: Symbol,
    pub sort_order: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalCondition {
    pub id: SignalConditionId,
    pub indicator: IndicatorKind,
    pub side: SignalSide,
    pub period: u32,
    pub threshold: Option<f64>,
    pub parameters: Value,
    pub enabled: bool,
    pub sort_order: i64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanPreset {
    pub id: ScanPresetId,
    pub name: String,
    pub trigger_mode: TriggerMode,
    pub conditions: Vec<SignalCondition>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct IndicatorValues {
    pub rsi: Option<f64>,
    pub mfi: Option<f64>,
    pub bollinger_lower: Option<f64>,
    pub bollinger_middle: Option<f64>,
    pub bollinger_upper: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SignalMatch {
    pub condition_id: SignalConditionId,
    pub matched: bool,
    pub newly_crossed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanRun {
    pub id: ScanRunId,
    pub watchlist_id: WatchlistId,
    pub preset_id: ScanPresetId,
    pub status: ScanRunStatus,
    pub base_trade_date: Option<String>,
    pub total_symbols: u32,
    pub succeeded_symbols: u32,
    pub failed_symbols: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResult {
    pub run_id: ScanRunId,
    pub symbol: Symbol,
    pub trade_date: String,
    pub current_price: f64,
    pub indicators: IndicatorValues,
    pub matches: Vec<SignalMatch>,
    pub all_conditions_matched: bool,
    pub any_condition_matched: bool,
    pub data_stale: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_symbol_and_maps_provider_symbol() {
        let symbol = Symbol::new(" brk.b ").expect("symbol must be valid");

        assert_eq!(symbol.as_str(), "BRK.B");
        assert_eq!(symbol.provider_symbol(), "BRK-B");
    }

    #[test]
    fn rejects_invalid_symbol() {
        assert!(Symbol::new("AAPL/USD").is_err());
    }

    #[test]
    fn validates_consistent_bar() {
        let bar = DailyBar {
            symbol: Symbol::new("AAPL").expect("symbol must be valid"),
            trade_date: "2026-07-14".to_string(),
            price_basis: PriceBasis::SplitAdjusted,
            open: 100.0,
            high: 104.0,
            low: 99.0,
            close: 102.0,
            volume: 1_000,
        };

        assert!(bar.validate().is_ok());
    }
}
