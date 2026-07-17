pub mod bollinger;
pub mod mfi;
pub mod rsi;

use crate::domain::{BollingerKey, BollingerValue, DailyBar, IndicatorKind, ScanPreset};
use crate::error::{AppError, AppResult};
use std::collections::HashMap;

/// Indicator output: same length as input, warm-up positions are `None`.
pub type IndicatorOutput = Vec<Option<f64>>;

/// Snapshot of indicator values for a single symbol at the latest trade date.
#[derive(Debug, Clone)]
pub struct IndicatorSnapshot {
    pub trade_date: String,
    pub close: f64,
    /// RSI values keyed by period. Only periods requested by the preset are computed.
    pub rsi_by_period: HashMap<u32, Option<f64>>,
    /// MFI values keyed by period. Only periods requested by the preset are computed.
    pub mfi_by_period: HashMap<u32, Option<f64>>,
    /// Bollinger Bands keyed by (period, multiplier). Only requested params are computed.
    pub bollinger_by_params: HashMap<BollingerKey, Option<BollingerValue>>,
    // Previous bar values (for cross detection)
    pub prev_close: Option<f64>,
    pub prev_rsi_by_period: HashMap<u32, Option<f64>>,
    pub prev_mfi_by_period: HashMap<u32, Option<f64>>,
    pub prev_bollinger_by_params: HashMap<BollingerKey, Option<BollingerValue>>,
}

/// Compute indicator snapshot from daily bars and preset conditions.
///
/// Extracts the unique indicator parameters required by the preset, computes each
/// indicator once, and returns current/previous values for signal evaluation.
pub fn compute_snapshot(bars: &[DailyBar], preset: &ScanPreset) -> AppResult<IndicatorSnapshot> {
    if bars.is_empty() {
        return Err(AppError::new(
            crate::error::AppErrorCode::InsufficientData,
            "no bars available for snapshot",
        ));
    }

    let len = bars.len();
    let last_idx = len - 1;

    // prev_idx available for cross-mode signal evaluation.
    let prev_idx = if len >= 2 { Some(len - 2) } else { None };

    // Extract unique parameters from enabled conditions
    let mut rsi_periods: Vec<u32> = Vec::new();
    let mut mfi_periods: Vec<u32> = Vec::new();
    let mut bollinger_keys: Vec<BollingerKey> = Vec::new();

    for condition in &preset.conditions {
        if !condition.enabled {
            continue;
        }
        match condition.indicator {
            IndicatorKind::Rsi => {
                if !rsi_periods.contains(&condition.period) {
                    rsi_periods.push(condition.period);
                }
            }
            IndicatorKind::Mfi => {
                if !mfi_periods.contains(&condition.period) {
                    mfi_periods.push(condition.period);
                }
            }
            IndicatorKind::Bollinger => {
                let multiplier = condition
                    .parameters
                    .get("stdDevMultiplier")
                    .and_then(|v| v.as_f64())
                    .unwrap_or(2.0);
                let key = BollingerKey {
                    period: condition.period,
                    multiplier,
                };
                if !bollinger_keys.contains(&key) {
                    bollinger_keys.push(key);
                }
            }
        }
    }

    // Extract OHLCV slices
    let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
    let highs: Vec<f64> = bars.iter().map(|b| b.high).collect();
    let lows: Vec<f64> = bars.iter().map(|b| b.low).collect();
    let volumes: Vec<u64> = bars.iter().map(|b| b.volume).collect();

    // Compute RSI
    let mut rsi_by_period: HashMap<u32, Option<f64>> = HashMap::new();
    let mut prev_rsi_by_period: HashMap<u32, Option<f64>> = HashMap::new();
    for &period in &rsi_periods {
        let output = rsi::calculate_rsi(&closes, period)?;
        let value = extract_value(&output, last_idx);
        rsi_by_period.insert(period, value);
        if let Some(pi) = prev_idx {
            let prev_value = extract_value(&output, pi);
            prev_rsi_by_period.insert(period, prev_value);
        }
    }

    // Compute MFI
    let mut mfi_by_period: HashMap<u32, Option<f64>> = HashMap::new();
    let mut prev_mfi_by_period: HashMap<u32, Option<f64>> = HashMap::new();
    for &period in &mfi_periods {
        let output = mfi::calculate_mfi(&highs, &lows, &closes, &volumes, period)?;
        let value = extract_value(&output, last_idx);
        mfi_by_period.insert(period, value);
        if let Some(pi) = prev_idx {
            let prev_value = extract_value(&output, pi);
            prev_mfi_by_period.insert(period, prev_value);
        }
    }

    // Compute Bollinger Bands
    let mut bollinger_by_params: HashMap<BollingerKey, Option<BollingerValue>> = HashMap::new();
    let mut prev_bollinger_by_params: HashMap<BollingerKey, Option<BollingerValue>> =
        HashMap::new();
    for key in &bollinger_keys {
        let (lower, middle, upper) =
            bollinger::calculate_bollinger(&closes, key.period, key.multiplier)?;
        let value = extract_bollinger(&lower, &middle, &upper, last_idx);
        bollinger_by_params.insert(*key, value);
        if let Some(pi) = prev_idx {
            let prev_value = extract_bollinger(&lower, &middle, &upper, pi);
            prev_bollinger_by_params.insert(*key, prev_value);
        }
    }

    let prev_close = prev_idx.map(|pi| bars[pi].close);

    Ok(IndicatorSnapshot {
        trade_date: bars[last_idx].trade_date.clone(),
        close: bars[last_idx].close,
        rsi_by_period,
        mfi_by_period,
        bollinger_by_params,
        prev_close,
        prev_rsi_by_period,
        prev_mfi_by_period,
        prev_bollinger_by_params,
    })
}

/// Extract value at the given index from an `IndicatorOutput`.
fn extract_value(output: &[Option<f64>], idx: usize) -> Option<f64> {
    output.get(idx).and_then(|v| *v)
}

/// Extract Bollinger value at the given index.
fn extract_bollinger(
    lower: &[Option<f64>],
    middle: &[Option<f64>],
    upper: &[Option<f64>],
    idx: usize,
) -> Option<BollingerValue> {
    match (
        lower.get(idx).and_then(|v| *v),
        middle.get(idx).and_then(|v| *v),
        upper.get(idx).and_then(|v| *v),
    ) {
        (Some(l), Some(m), Some(u)) => Some(BollingerValue {
            lower: l,
            middle: m,
            upper: u,
        }),
        _ => None,
    }
}

/// Validate that `period` is at least 1.
pub fn validate_period(period: u32) -> AppResult<()> {
    if period < 1 {
        return Err(AppError::validation("indicator period must be at least 1"));
    }
    Ok(())
}

/// Verify that all input slices have the same length.
/// Accepts mixed types by comparing usize lengths directly.
pub fn assert_lengths_match(lengths: &[usize]) -> AppResult<()> {
    if lengths.is_empty() {
        return Err(AppError::validation("at least one input slice is required"));
    }
    let expected_len = lengths[0];
    if expected_len == 0 {
        return Err(AppError::validation("input must not be empty"));
    }
    for (i, &len) in lengths.iter().enumerate().skip(1) {
        if len != expected_len {
            return Err(AppError::validation(format!(
                "input slice {i} length {len} does not match first slice length {expected_len}"
            )));
        }
    }
    Ok(())
}

/// Create an output vector of the same length as input, all `None`.
pub fn empty_output(len: usize) -> IndicatorOutput {
    vec![None; len]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        PriceBasis, ScanPresetId, SignalCondition, SignalConditionId, Symbol, TriggerMode,
    };
    use serde_json::json;

    fn make_bar(date: &str, close: f64) -> DailyBar {
        DailyBar {
            symbol: Symbol::new("AAPL").unwrap(),
            trade_date: date.to_string(),
            price_basis: PriceBasis::SplitAdjusted,
            open: close,
            high: close + 1.0,
            low: close - 1.0,
            close,
            volume: 1_000,
        }
    }

    fn make_preset(conditions: Vec<SignalCondition>) -> ScanPreset {
        ScanPreset {
            id: ScanPresetId::new("test").unwrap(),
            name: "Test".to_string(),
            conditions,
        }
    }

    fn make_rsi_condition(period: u32) -> SignalCondition {
        SignalCondition {
            id: SignalConditionId::new("rsi1").unwrap(),
            indicator: IndicatorKind::Rsi,
            side: crate::domain::SignalSide::Lower,
            period,
            threshold: Some(30.0),
            parameters: json!({}),
            trigger_mode: TriggerMode::Current,
            enabled: true,
            sort_order: 0,
        }
    }

    fn make_mfi_condition(period: u32) -> SignalCondition {
        SignalCondition {
            id: SignalConditionId::new("mfi1").unwrap(),
            indicator: IndicatorKind::Mfi,
            side: crate::domain::SignalSide::Upper,
            period,
            threshold: Some(70.0),
            parameters: json!({}),
            trigger_mode: TriggerMode::Current,
            enabled: true,
            sort_order: 0,
        }
    }

    fn make_bollinger_condition(period: u32, multiplier: f64) -> SignalCondition {
        SignalCondition {
            id: SignalConditionId::new("bb1").unwrap(),
            indicator: IndicatorKind::Bollinger,
            side: crate::domain::SignalSide::Lower,
            period,
            threshold: None,
            parameters: json!({ "stdDevMultiplier": multiplier }),
            trigger_mode: TriggerMode::Current,
            enabled: true,
            sort_order: 0,
        }
    }

    // ---- Utility tests (preserved) ----

    #[test]
    fn rejects_period_zero() {
        assert!(validate_period(0).is_err());
    }

    #[test]
    fn accepts_period_one() {
        assert!(validate_period(1).is_ok());
    }

    #[test]
    fn accepts_period_greater_than_one() {
        assert!(validate_period(14).is_ok());
    }

    #[test]
    fn rejects_empty_lengths() {
        assert!(assert_lengths_match(&[]).is_err());
    }

    #[test]
    fn rejects_zero_length() {
        assert!(assert_lengths_match(&[0]).is_err());
    }

    #[test]
    fn accepts_equal_lengths() {
        assert!(assert_lengths_match(&[3, 3, 3, 3]).is_ok());
    }

    #[test]
    fn rejects_mismatched_lengths() {
        assert!(assert_lengths_match(&[2, 3]).is_err());
    }

    #[test]
    fn empty_output_matches_input_length() {
        let output = empty_output(5);
        assert_eq!(output.len(), 5);
        assert!(output.iter().all(|v| v.is_none()));
    }

    // ---- compute_snapshot tests ----

    fn make_bars(n: usize) -> Vec<DailyBar> {
        (0..n)
            .map(|i| make_bar(&format!("2026-07-{:02}", i + 1), 100.0 + i as f64))
            .collect()
    }

    #[test]
    fn rsi_only_preset_fills_rsi() {
        let bars = make_bars(20);
        let preset = make_preset(vec![make_rsi_condition(14)]);

        let snapshot = compute_snapshot(&bars, &preset).unwrap();

        assert_eq!(snapshot.trade_date, "2026-07-20");
        assert_eq!(snapshot.close, 119.0);
        assert!(snapshot.rsi_by_period.contains_key(&14));
        assert!(snapshot.rsi_by_period[&14].is_some());
        assert!(snapshot.mfi_by_period.is_empty());
        assert!(snapshot.bollinger_by_params.is_empty());

        // Previous values: prev_close is Some, prev_rsi has period 14
        assert!(snapshot.prev_close.is_some());
        assert_eq!(snapshot.prev_close.unwrap(), 118.0);
        assert!(snapshot.prev_rsi_by_period.contains_key(&14));
        assert!(snapshot.prev_mfi_by_period.is_empty());
        assert!(snapshot.prev_bollinger_by_params.is_empty());
    }

    #[test]
    fn mfi_only_preset_fills_mfi() {
        let bars = make_bars(20);
        let preset = make_preset(vec![make_mfi_condition(14)]);

        let snapshot = compute_snapshot(&bars, &preset).unwrap();

        assert!(snapshot.rsi_by_period.is_empty());
        assert!(snapshot.mfi_by_period.contains_key(&14));
        assert!(snapshot.mfi_by_period[&14].is_some());
        assert!(snapshot.bollinger_by_params.is_empty());

        // Previous values
        assert!(snapshot.prev_close.is_some());
        assert!(snapshot.prev_mfi_by_period.contains_key(&14));
        assert!(snapshot.prev_rsi_by_period.is_empty());
        assert!(snapshot.prev_bollinger_by_params.is_empty());
    }

    #[test]
    fn bollinger_only_preset_fills_bollinger() {
        let bars = make_bars(20);
        let preset = make_preset(vec![make_bollinger_condition(14, 2.0)]);

        let snapshot = compute_snapshot(&bars, &preset).unwrap();

        assert!(snapshot.rsi_by_period.is_empty());
        assert!(snapshot.mfi_by_period.is_empty());

        let key = BollingerKey {
            period: 14,
            multiplier: 2.0,
        };
        assert!(snapshot.bollinger_by_params.contains_key(&key));
        let val = snapshot.bollinger_by_params[&key].as_ref().unwrap();
        // Constant-ish series: all three bands close together
        assert!(val.lower <= val.middle);
        assert!(val.middle <= val.upper);

        // Previous values
        assert!(snapshot.prev_close.is_some());
        assert!(snapshot.prev_bollinger_by_params.contains_key(&key));
        assert!(snapshot.prev_rsi_by_period.is_empty());
        assert!(snapshot.prev_mfi_by_period.is_empty());
    }

    #[test]
    fn mixed_preset_fills_all() {
        let bars = make_bars(30);
        let preset = make_preset(vec![
            make_rsi_condition(14),
            make_mfi_condition(14),
            make_bollinger_condition(20, 2.0),
        ]);

        let snapshot = compute_snapshot(&bars, &preset).unwrap();

        assert!(snapshot.rsi_by_period.contains_key(&14));
        assert!(snapshot.mfi_by_period.contains_key(&14));

        let key = BollingerKey {
            period: 20,
            multiplier: 2.0,
        };
        assert!(snapshot.bollinger_by_params.contains_key(&key));

        // All have values
        assert!(snapshot.rsi_by_period[&14].is_some());
        assert!(snapshot.mfi_by_period[&14].is_some());
        assert!(snapshot.bollinger_by_params[&key].is_some());

        // Previous values also populated
        assert!(snapshot.prev_close.is_some());
        assert!(snapshot.prev_rsi_by_period.contains_key(&14));
        assert!(snapshot.prev_mfi_by_period.contains_key(&14));
        assert!(snapshot.prev_bollinger_by_params.contains_key(&key));
    }

    #[test]
    fn same_period_dedup_computed_once() {
        // Two RSI conditions with same period → only one entry in HashMap
        let bars = make_bars(20);
        let cond1 = make_rsi_condition(14);
        let mut cond2 = make_rsi_condition(14);
        cond2.id = SignalConditionId::new("rsi2").unwrap();
        let preset = make_preset(vec![cond1, cond2]);

        let snapshot = compute_snapshot(&bars, &preset).unwrap();

        // Only one entry for period 14
        assert_eq!(snapshot.rsi_by_period.len(), 1);
        assert!(snapshot.rsi_by_period.contains_key(&14));
        // Same for prev
        assert_eq!(snapshot.prev_rsi_by_period.len(), 1);
    }

    #[test]
    fn empty_bars_returns_insufficient_data() {
        let preset = make_preset(vec![make_rsi_condition(14)]);
        let result = compute_snapshot(&[], &preset);

        assert!(result.is_err());
        assert_eq!(
            result.unwrap_err().code,
            crate::error::AppErrorCode::InsufficientData
        );
    }

    #[test]
    fn cross_previous_value_extracted_correctly() {
        // With 20 bars, last_idx=19. RSI period=2: first valid at index 2.
        // Both index 18 (prev) and 19 (current) must have values for cross detection.
        // Use oscillating prices so prev and current differ.
        let bars: Vec<DailyBar> = (0..20)
            .map(|i| {
                let close = if i % 3 == 0 {
                    100.0
                } else if i % 3 == 1 {
                    105.0
                } else {
                    98.0
                };
                make_bar(&format!("2026-07-{:02}", i + 1), close)
            })
            .collect();
        let preset = make_preset(vec![make_rsi_condition(2)]);

        let snapshot = compute_snapshot(&bars, &preset).unwrap();

        // Current value (last bar) is Some
        assert!(snapshot.rsi_by_period[&2].is_some());

        // Previous value is also Some (extracted from index 18)
        assert!(snapshot.prev_rsi_by_period[&2].is_some());

        // Verify by re-computing: both prev (18) and current (19) are Some
        let closes: Vec<f64> = bars.iter().map(|b| b.close).collect();
        let output = rsi::calculate_rsi(&closes, 2).unwrap();
        assert!(output[18].is_some());
        assert!(output[19].is_some());
        // Oscillating prices produce different RSI at consecutive indices
        assert_ne!(output[18].unwrap(), output[19].unwrap());

        // Snapshot prev value matches index 18
        assert_eq!(snapshot.prev_rsi_by_period[&2], output[18]);
    }

    #[test]
    fn disabled_condition_not_computed() {
        let bars = make_bars(20);
        let mut cond = make_rsi_condition(14);
        cond.enabled = false;
        let preset = make_preset(vec![cond]);

        let snapshot = compute_snapshot(&bars, &preset).unwrap();

        // Disabled condition's period should not appear
        assert!(snapshot.rsi_by_period.is_empty());
        assert!(snapshot.prev_rsi_by_period.is_empty());
    }

    #[test]
    fn single_bar_no_previous() {
        let bars = vec![make_bar("2026-07-01", 100.0)];
        let preset = make_preset(vec![make_rsi_condition(2)]);

        // period=2 needs at least 3 bars for one value; we have 1 → all None
        let snapshot = compute_snapshot(&bars, &preset).unwrap();

        assert_eq!(snapshot.trade_date, "2026-07-01");
        assert_eq!(snapshot.close, 100.0);
        // RSI period=2 with only 1 bar: insufficient data, all None
        assert!(snapshot.rsi_by_period[&2].is_none());

        // No previous bar: prev_close is None, prev HashMaps are empty
        assert!(snapshot.prev_close.is_none());
        assert!(snapshot.prev_rsi_by_period.is_empty());
    }

    #[test]
    fn bollinger_same_period_different_multiplier_two_entries() {
        let bars = make_bars(20);
        let cond1 = make_bollinger_condition(14, 1.5);
        let mut cond2 = make_bollinger_condition(14, 2.0);
        cond2.id = SignalConditionId::new("bb2").unwrap();
        let preset = make_preset(vec![cond1, cond2]);

        let snapshot = compute_snapshot(&bars, &preset).unwrap();

        assert_eq!(snapshot.bollinger_by_params.len(), 2);

        let key1 = BollingerKey {
            period: 14,
            multiplier: 1.5,
        };
        let key2 = BollingerKey {
            period: 14,
            multiplier: 2.0,
        };

        assert!(snapshot.bollinger_by_params.contains_key(&key1));
        assert!(snapshot.bollinger_by_params.contains_key(&key2));

        // multiplier=2.0 band is wider
        let v1 = snapshot.bollinger_by_params[&key1].as_ref().unwrap();
        let v2 = snapshot.bollinger_by_params[&key2].as_ref().unwrap();
        let width1 = v1.upper - v1.lower;
        let width2 = v2.upper - v2.lower;
        assert!(width2 > width1, "multiplier 2.0 band should be wider");

        // Previous values also have two entries
        assert_eq!(snapshot.prev_bollinger_by_params.len(), 2);
        assert!(snapshot.prev_bollinger_by_params.contains_key(&key1));
        assert!(snapshot.prev_bollinger_by_params.contains_key(&key2));
    }
}
