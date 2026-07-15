use crate::domain::{
    BollingerKey, BollingerValue, IndicatorKind, SignalCondition, SignalMatch, SignalSide,
    TriggerMode,
};
use crate::error::{AppError, AppErrorCode, AppResult};
use crate::indicator::IndicatorSnapshot;

/// Evaluate current-mode signals for all enabled conditions in the preset.
///
/// Returns a `Vec<SignalMatch>`, one per enabled condition.
/// If any condition's indicator value is `None` (warm-up), returns `InsufficientData` error.
pub fn evaluate_current(
    snapshot: &IndicatorSnapshot,
    conditions: &[SignalCondition],
) -> AppResult<Vec<SignalMatch>> {
    let mut matches = Vec::with_capacity(conditions.len());

    for condition in conditions {
        if !condition.enabled {
            continue;
        }

        let matched = evaluate_condition(snapshot, condition)?;

        matches.push(SignalMatch {
            condition_id: condition.id.clone(),
            matched,
            newly_crossed: false, // current mode never crosses
        });
    }

    Ok(matches)
}

fn evaluate_condition(
    snapshot: &IndicatorSnapshot,
    condition: &SignalCondition,
) -> AppResult<bool> {
    let value = match condition.indicator {
        IndicatorKind::Rsi => snapshot
            .rsi_by_period
            .get(&condition.period)
            .copied()
            .flatten()
            .ok_or_else(|| {
                AppError::new(
                    AppErrorCode::InsufficientData,
                    format!("RSI(period={}) not available (warm-up)", condition.period),
                )
            })?,
        IndicatorKind::Mfi => snapshot
            .mfi_by_period
            .get(&condition.period)
            .copied()
            .flatten()
            .ok_or_else(|| {
                AppError::new(
                    AppErrorCode::InsufficientData,
                    format!("MFI(period={}) not available (warm-up)", condition.period),
                )
            })?,
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
            let bands = snapshot
                .bollinger_by_params
                .get(&key)
                .copied()
                .flatten()
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorCode::InsufficientData,
                        format!(
                            "Bollinger(period={}, multiplier={}) not available (warm-up)",
                            condition.period, multiplier
                        ),
                    )
                })?;

            return evaluate_bollinger(snapshot.close, &bands, condition.side);
        }
    };

    let threshold = condition.threshold.unwrap_or(0.0);
    evaluate_threshold(value, threshold, condition.side)
}

fn evaluate_threshold(value: f64, threshold: f64, side: SignalSide) -> AppResult<bool> {
    let matched = match side {
        SignalSide::Lower => value <= threshold,
        SignalSide::Upper => value >= threshold,
    };
    Ok(matched)
}

fn evaluate_bollinger(close: f64, bands: &BollingerValue, side: SignalSide) -> AppResult<bool> {
    let matched = match side {
        SignalSide::Lower => close <= bands.lower,
        SignalSide::Upper => close >= bands.upper,
    };
    Ok(matched)
}

/// Evaluate cross-mode signals for all enabled conditions.
///
/// Lower cross: `previous > threshold && current <= threshold`
/// Upper cross: `previous < threshold && current >= threshold`
/// Bollinger cross uses previous bar's bands and close vs. current bar's bands and close.
/// If previous value is `None`, returns `InsufficientData`.
pub fn evaluate_cross(
    snapshot: &IndicatorSnapshot,
    conditions: &[SignalCondition],
) -> AppResult<Vec<SignalMatch>> {
    let mut matches = Vec::with_capacity(conditions.len());

    for condition in conditions {
        if !condition.enabled {
            continue;
        }

        let matched = evaluate_condition_cross(snapshot, condition)?;

        matches.push(SignalMatch {
            condition_id: condition.id.clone(),
            matched,
            newly_crossed: matched, // in cross mode, matched == newly_crossed
        });
    }

    Ok(matches)
}

fn evaluate_condition_cross(
    snapshot: &IndicatorSnapshot,
    condition: &SignalCondition,
) -> AppResult<bool> {
    match condition.indicator {
        IndicatorKind::Rsi => {
            let current = snapshot
                .rsi_by_period
                .get(&condition.period)
                .copied()
                .flatten()
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorCode::InsufficientData,
                        format!(
                            "RSI(period={}) current value not available",
                            condition.period
                        ),
                    )
                })?;
            let previous = snapshot
                .prev_rsi_by_period
                .get(&condition.period)
                .copied()
                .flatten()
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorCode::InsufficientData,
                        format!(
                            "RSI(period={}) previous value not available",
                            condition.period
                        ),
                    )
                })?;
            let threshold = condition.threshold.unwrap_or(0.0);
            evaluate_threshold_cross(current, previous, threshold, condition.side)
        }
        IndicatorKind::Mfi => {
            let current = snapshot
                .mfi_by_period
                .get(&condition.period)
                .copied()
                .flatten()
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorCode::InsufficientData,
                        format!(
                            "MFI(period={}) current value not available",
                            condition.period
                        ),
                    )
                })?;
            let previous = snapshot
                .prev_mfi_by_period
                .get(&condition.period)
                .copied()
                .flatten()
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorCode::InsufficientData,
                        format!(
                            "MFI(period={}) previous value not available",
                            condition.period
                        ),
                    )
                })?;
            let threshold = condition.threshold.unwrap_or(0.0);
            evaluate_threshold_cross(current, previous, threshold, condition.side)
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
            let current_bands = snapshot
                .bollinger_by_params
                .get(&key)
                .copied()
                .flatten()
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorCode::InsufficientData,
                        format!(
                            "Bollinger(period={}, multiplier={}) current bands not available",
                            condition.period, multiplier
                        ),
                    )
                })?;
            let prev_bands = snapshot
                .prev_bollinger_by_params
                .get(&key)
                .copied()
                .flatten()
                .ok_or_else(|| {
                    AppError::new(
                        AppErrorCode::InsufficientData,
                        format!(
                            "Bollinger(period={}, multiplier={}) previous bands not available",
                            condition.period, multiplier
                        ),
                    )
                })?;
            let prev_close = snapshot.prev_close.ok_or_else(|| {
                AppError::new(
                    AppErrorCode::InsufficientData,
                    "previous close not available for Bollinger cross",
                )
            })?;

            evaluate_bollinger_cross(
                snapshot.close,
                &current_bands,
                prev_close,
                &prev_bands,
                condition.side,
            )
        }
    }
}

fn evaluate_threshold_cross(
    current: f64,
    previous: f64,
    threshold: f64,
    side: SignalSide,
) -> AppResult<bool> {
    let crossed = match side {
        SignalSide::Lower => previous > threshold && current <= threshold,
        SignalSide::Upper => previous < threshold && current >= threshold,
    };
    Ok(crossed)
}

fn evaluate_bollinger_cross(
    current_close: f64,
    current_bands: &BollingerValue,
    prev_close: f64,
    prev_bands: &BollingerValue,
    side: SignalSide,
) -> AppResult<bool> {
    let crossed = match side {
        SignalSide::Lower => prev_close > prev_bands.lower && current_close <= current_bands.lower,
        SignalSide::Upper => prev_close < prev_bands.upper && current_close >= current_bands.upper,
    };
    Ok(crossed)
}

/// Aggregate signal matches based on the preset's logic.
///
/// - AND: all active conditions must match
/// - OR: at least one active condition must match
/// - 0 active conditions: all_matched=false, any_matched=false
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalAggregate {
    pub all_matched: bool,
    pub any_matched: bool,
}

pub fn aggregate_matches(matches: &[SignalMatch]) -> SignalAggregate {
    let all_matched = if matches.is_empty() {
        false
    } else {
        matches.iter().all(|m| m.matched)
    };

    let any_matched = matches.iter().any(|m| m.matched);

    SignalAggregate {
        all_matched,
        any_matched,
    }
}

/// Unified signal evaluator that dispatches to current or cross mode per condition.
///
/// Each condition's `trigger_mode` decides which evaluator to use, allowing mixed-mode presets.
pub fn evaluate_signals(
    snapshot: &IndicatorSnapshot,
    conditions: &[SignalCondition],
) -> AppResult<Vec<SignalMatch>> {
    let mut matches = Vec::new();

    for condition in conditions {
        if !condition.enabled {
            continue;
        }

        let (matched, newly_crossed) = match condition.trigger_mode {
            TriggerMode::Current => {
                let matched = evaluate_condition(snapshot, condition)?;
                (matched, false)
            }
            TriggerMode::Cross => {
                let matched = evaluate_condition_cross(snapshot, condition)?;
                (matched, matched) // in cross mode, matched implies newly crossed
            }
        };

        matches.push(SignalMatch {
            condition_id: condition.id.clone(),
            matched,
            newly_crossed,
        });
    }

    Ok(matches)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{SignalConditionId, TriggerMode};
    use serde_json::json;
    use std::collections::HashMap;

    fn make_snapshot(
        rsi_14: Option<f64>,
        mfi_14: Option<f64>,
        bb_value: Option<BollingerValue>,
        close: f64,
    ) -> IndicatorSnapshot {
        let mut rsi_by_period = HashMap::new();
        if rsi_14.is_some() {
            rsi_by_period.insert(14, rsi_14);
        } else {
            rsi_by_period.insert(14, None);
        }

        let mut mfi_by_period = HashMap::new();
        if mfi_14.is_some() {
            mfi_by_period.insert(14, mfi_14);
        } else {
            mfi_by_period.insert(14, None);
        }

        let mut bollinger_by_params = HashMap::new();
        let key = BollingerKey {
            period: 20,
            multiplier: 2.0,
        };
        if bb_value.is_some() {
            bollinger_by_params.insert(key, bb_value);
        } else {
            bollinger_by_params.insert(key, None);
        }

        IndicatorSnapshot {
            trade_date: "2026-07-14".to_string(),
            close,
            rsi_by_period,
            mfi_by_period,
            bollinger_by_params,
            prev_close: None,
            prev_rsi_by_period: HashMap::new(),
            prev_mfi_by_period: HashMap::new(),
            prev_bollinger_by_params: HashMap::new(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn make_snapshot_with_prev(
        rsi_14: Option<f64>,
        prev_rsi_14: Option<f64>,
        mfi_14: Option<f64>,
        prev_mfi_14: Option<f64>,
        bb_value: Option<BollingerValue>,
        prev_bb_value: Option<BollingerValue>,
        close: f64,
        prev_close: Option<f64>,
    ) -> IndicatorSnapshot {
        let mut rsi_by_period = HashMap::new();
        rsi_by_period.insert(14, rsi_14);

        let mut prev_rsi_by_period = HashMap::new();
        prev_rsi_by_period.insert(14, prev_rsi_14);

        let mut mfi_by_period = HashMap::new();
        mfi_by_period.insert(14, mfi_14);

        let mut prev_mfi_by_period = HashMap::new();
        prev_mfi_by_period.insert(14, prev_mfi_14);

        let mut bollinger_by_params = HashMap::new();
        let key = BollingerKey {
            period: 20,
            multiplier: 2.0,
        };
        bollinger_by_params.insert(key, bb_value);

        let mut prev_bollinger_by_params = HashMap::new();
        prev_bollinger_by_params.insert(key, prev_bb_value);

        IndicatorSnapshot {
            trade_date: "2026-07-14".to_string(),
            close,
            rsi_by_period,
            mfi_by_period,
            bollinger_by_params,
            prev_close,
            prev_rsi_by_period,
            prev_mfi_by_period,
            prev_bollinger_by_params,
        }
    }

    fn rsi_lower_condition(
        id: &str,
        period: u32,
        threshold: f64,
        enabled: bool,
    ) -> SignalCondition {
        SignalCondition {
            id: SignalConditionId::new(id).unwrap(),
            indicator: IndicatorKind::Rsi,
            side: SignalSide::Lower,
            period,
            threshold: Some(threshold),
            parameters: json!({}),
            trigger_mode: TriggerMode::Current,
            enabled,
            sort_order: 0,
        }
    }

    fn rsi_upper_condition(
        id: &str,
        period: u32,
        threshold: f64,
        enabled: bool,
    ) -> SignalCondition {
        SignalCondition {
            id: SignalConditionId::new(id).unwrap(),
            indicator: IndicatorKind::Rsi,
            side: SignalSide::Upper,
            period,
            threshold: Some(threshold),
            parameters: json!({}),
            trigger_mode: TriggerMode::Current,
            enabled,
            sort_order: 0,
        }
    }

    fn rsi_lower_cross_condition(
        id: &str,
        period: u32,
        threshold: f64,
        enabled: bool,
    ) -> SignalCondition {
        SignalCondition {
            id: SignalConditionId::new(id).unwrap(),
            indicator: IndicatorKind::Rsi,
            side: SignalSide::Lower,
            period,
            threshold: Some(threshold),
            parameters: json!({}),
            trigger_mode: TriggerMode::Cross,
            enabled,
            sort_order: 0,
        }
    }

    fn rsi_upper_cross_condition(
        id: &str,
        period: u32,
        threshold: f64,
        enabled: bool,
    ) -> SignalCondition {
        SignalCondition {
            id: SignalConditionId::new(id).unwrap(),
            indicator: IndicatorKind::Rsi,
            side: SignalSide::Upper,
            period,
            threshold: Some(threshold),
            parameters: json!({}),
            trigger_mode: TriggerMode::Cross,
            enabled,
            sort_order: 0,
        }
    }

    fn mfi_upper_condition(
        id: &str,
        period: u32,
        threshold: f64,
        enabled: bool,
    ) -> SignalCondition {
        SignalCondition {
            id: SignalConditionId::new(id).unwrap(),
            indicator: IndicatorKind::Mfi,
            side: SignalSide::Upper,
            period,
            threshold: Some(threshold),
            parameters: json!({}),
            trigger_mode: TriggerMode::Current,
            enabled,
            sort_order: 0,
        }
    }

    fn bb_lower_condition(
        id: &str,
        period: u32,
        multiplier: f64,
        enabled: bool,
    ) -> SignalCondition {
        SignalCondition {
            id: SignalConditionId::new(id).unwrap(),
            indicator: IndicatorKind::Bollinger,
            side: SignalSide::Lower,
            period,
            threshold: None,
            parameters: json!({ "stdDevMultiplier": multiplier }),
            trigger_mode: TriggerMode::Current,
            enabled,
            sort_order: 0,
        }
    }

    fn bb_upper_condition(
        id: &str,
        period: u32,
        multiplier: f64,
        enabled: bool,
    ) -> SignalCondition {
        SignalCondition {
            id: SignalConditionId::new(id).unwrap(),
            indicator: IndicatorKind::Bollinger,
            side: SignalSide::Upper,
            period,
            threshold: None,
            parameters: json!({ "stdDevMultiplier": multiplier }),
            trigger_mode: TriggerMode::Current,
            enabled,
            sort_order: 0,
        }
    }

    fn bb_lower_cross_condition(
        id: &str,
        period: u32,
        multiplier: f64,
        enabled: bool,
    ) -> SignalCondition {
        SignalCondition {
            id: SignalConditionId::new(id).unwrap(),
            indicator: IndicatorKind::Bollinger,
            side: SignalSide::Lower,
            period,
            threshold: None,
            parameters: json!({ "stdDevMultiplier": multiplier }),
            trigger_mode: TriggerMode::Cross,
            enabled,
            sort_order: 0,
        }
    }

    fn bb_upper_cross_condition(
        id: &str,
        period: u32,
        multiplier: f64,
        enabled: bool,
    ) -> SignalCondition {
        SignalCondition {
            id: SignalConditionId::new(id).unwrap(),
            indicator: IndicatorKind::Bollinger,
            side: SignalSide::Upper,
            period,
            threshold: None,
            parameters: json!({ "stdDevMultiplier": multiplier }),
            trigger_mode: TriggerMode::Cross,
            enabled,
            sort_order: 0,
        }
    }

    // ---- Test 1: threshold와 정확히 같은 값 ----

    #[test]
    fn rsi_exact_threshold_lower_matches() {
        let snapshot = make_snapshot(Some(30.0), None, None, 100.0);
        let conditions = vec![rsi_lower_condition("c1", 14, 30.0, true)];

        let matches = evaluate_current(&snapshot, &conditions).unwrap();

        assert_eq!(matches.len(), 1);
        assert!(matches[0].matched);
        assert!(!matches[0].newly_crossed);
    }

    // ---- Test 2: upper/lower 대칭 ----

    #[test]
    fn rsi_exact_threshold_upper_matches() {
        let snapshot = make_snapshot(Some(70.0), None, None, 100.0);
        let conditions = vec![rsi_upper_condition("c1", 14, 70.0, true)];

        let matches = evaluate_current(&snapshot, &conditions).unwrap();

        assert_eq!(matches.len(), 1);
        assert!(matches[0].matched);
    }

    // ---- Test 3: 활성/비활성 혼합 ----

    #[test]
    fn disabled_condition_excluded_from_results() {
        let snapshot = make_snapshot(Some(25.0), None, None, 100.0);
        let conditions = vec![
            rsi_lower_condition("c1", 14, 30.0, true),
            rsi_lower_condition("c2", 14, 30.0, false),
        ];

        let matches = evaluate_current(&snapshot, &conditions).unwrap();

        // Only the enabled condition appears
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].condition_id.0, "c1");
        assert!(matches[0].matched); // 25 <= 30
    }

    // ---- Test 4: warm-up None → InsufficientData ----

    #[test]
    fn rsi_warmup_returns_insufficient_data() {
        let snapshot = make_snapshot(None, None, None, 100.0);
        let conditions = vec![rsi_lower_condition("c1", 14, 30.0, true)];

        let result = evaluate_current(&snapshot, &conditions);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, AppErrorCode::InsufficientData);
    }

    // ---- Test 5: Bollinger lower touch ----

    #[test]
    fn bollinger_lower_touch_matches() {
        let bands = BollingerValue {
            lower: 95.0,
            middle: 100.0,
            upper: 105.0,
        };
        let snapshot = make_snapshot(None, None, Some(bands), 95.0);
        let conditions = vec![bb_lower_condition("c1", 20, 2.0, true)];

        let matches = evaluate_current(&snapshot, &conditions).unwrap();

        assert_eq!(matches.len(), 1);
        assert!(matches[0].matched); // close == lower
    }

    // ---- Test 6: Bollinger upper touch ----

    #[test]
    fn bollinger_upper_touch_matches() {
        let bands = BollingerValue {
            lower: 95.0,
            middle: 100.0,
            upper: 105.0,
        };
        let snapshot = make_snapshot(None, None, Some(bands), 105.0);
        let conditions = vec![bb_upper_condition("c1", 20, 2.0, true)];

        let matches = evaluate_current(&snapshot, &conditions).unwrap();

        assert_eq!(matches.len(), 1);
        assert!(matches[0].matched); // close == upper
    }

    // ---- Test 7: Bollinger no touch ----

    #[test]
    fn bollinger_no_touch_does_not_match() {
        let bands = BollingerValue {
            lower: 95.0,
            middle: 100.0,
            upper: 105.0,
        };
        let snapshot = make_snapshot(None, None, Some(bands), 100.0);
        let conditions = vec![
            bb_lower_condition("c1", 20, 2.0, true),
            bb_upper_condition("c2", 20, 2.0, true),
        ];

        let matches = evaluate_current(&snapshot, &conditions).unwrap();

        assert_eq!(matches.len(), 2);
        assert!(!matches[0].matched); // 100 > 95, not lower
        assert!(!matches[1].matched); // 100 < 105, not upper
    }

    // ---- Test 8: RSI lower no match ----

    #[test]
    fn rsi_lower_no_match() {
        let snapshot = make_snapshot(Some(40.0), None, None, 100.0);
        let conditions = vec![rsi_lower_condition("c1", 14, 30.0, true)];

        let matches = evaluate_current(&snapshot, &conditions).unwrap();

        assert_eq!(matches.len(), 1);
        assert!(!matches[0].matched); // 40 > 30, not lower
    }

    // ---- Test 9: MFI upper match ----

    #[test]
    fn mfi_upper_matches() {
        let snapshot = make_snapshot(None, Some(80.0), None, 100.0);
        let conditions = vec![mfi_upper_condition("c1", 14, 70.0, true)];

        let matches = evaluate_current(&snapshot, &conditions).unwrap();

        assert_eq!(matches.len(), 1);
        assert!(matches[0].matched); // 80 >= 70
    }

    // ---- Cross tests ----

    // Test 10: RSI lower cross: prev=35, current=28, threshold=30
    #[test]
    fn rsi_lower_cross_matches() {
        let snapshot = make_snapshot_with_prev(
            Some(28.0),
            Some(35.0),
            None,
            None,
            None,
            None,
            100.0,
            Some(99.0),
        );
        let conditions = vec![rsi_lower_cross_condition("c1", 14, 30.0, true)];

        let matches = evaluate_cross(&snapshot, &conditions).unwrap();

        assert_eq!(matches.len(), 1);
        assert!(matches[0].matched); // 35 > 30 && 28 <= 30
        assert!(matches[0].newly_crossed);
    }

    // Test 11: RSI upper cross: prev=65, current=75, threshold=70
    #[test]
    fn rsi_upper_cross_matches() {
        let snapshot = make_snapshot_with_prev(
            Some(75.0),
            Some(65.0),
            None,
            None,
            None,
            None,
            100.0,
            Some(99.0),
        );
        let conditions = vec![rsi_upper_cross_condition("c1", 14, 70.0, true)];

        let matches = evaluate_cross(&snapshot, &conditions).unwrap();

        assert_eq!(matches.len(), 1);
        assert!(matches[0].matched); // 65 < 70 && 75 >= 70
        assert!(matches[0].newly_crossed);
    }

    // Test 12: RSI lower no cross: prev=25, current=28, threshold=30
    #[test]
    fn rsi_lower_no_cross() {
        let snapshot = make_snapshot_with_prev(
            Some(28.0),
            Some(25.0),
            None,
            None,
            None,
            None,
            100.0,
            Some(99.0),
        );
        let conditions = vec![rsi_lower_cross_condition("c1", 14, 30.0, true)];

        let matches = evaluate_cross(&snapshot, &conditions).unwrap();

        assert_eq!(matches.len(), 1);
        assert!(!matches[0].matched); // prev was already below threshold
        assert!(!matches[0].newly_crossed);
    }

    // Test 13: Bollinger lower cross
    #[test]
    fn bollinger_lower_cross_matches() {
        let current_bands = BollingerValue {
            lower: 95.0,
            middle: 100.0,
            upper: 105.0,
        };
        let prev_bands = BollingerValue {
            lower: 94.0,
            middle: 99.0,
            upper: 104.0,
        };
        // prev_close=96 > prev_lower=94, current_close=94 <= current_lower=95
        let snapshot = make_snapshot_with_prev(
            None,
            None,
            None,
            None,
            Some(current_bands),
            Some(prev_bands),
            94.0,
            Some(96.0),
        );
        let conditions = vec![bb_lower_cross_condition("c1", 20, 2.0, true)];

        let matches = evaluate_cross(&snapshot, &conditions).unwrap();

        assert_eq!(matches.len(), 1);
        assert!(matches[0].matched);
        assert!(matches[0].newly_crossed);
    }

    // Test 14: Bollinger upper cross
    #[test]
    fn bollinger_upper_cross_matches() {
        let current_bands = BollingerValue {
            lower: 95.0,
            middle: 100.0,
            upper: 105.0,
        };
        let prev_bands = BollingerValue {
            lower: 96.0,
            middle: 101.0,
            upper: 106.0,
        };
        // prev_close=104 < prev_upper=106, current_close=106 >= current_upper=105
        let snapshot = make_snapshot_with_prev(
            None,
            None,
            None,
            None,
            Some(current_bands),
            Some(prev_bands),
            106.0,
            Some(104.0),
        );
        let conditions = vec![bb_upper_cross_condition("c1", 20, 2.0, true)];

        let matches = evaluate_cross(&snapshot, &conditions).unwrap();

        assert_eq!(matches.len(), 1);
        assert!(matches[0].matched);
        assert!(matches[0].newly_crossed);
    }

    // Test 15: Previous warm-up None → InsufficientData
    #[test]
    fn cross_prev_none_returns_insufficient_data() {
        // current is Some but prev is None
        let snapshot =
            make_snapshot_with_prev(Some(28.0), None, None, None, None, None, 100.0, Some(99.0));
        let conditions = vec![rsi_lower_cross_condition("c1", 14, 30.0, true)];

        let result = evaluate_cross(&snapshot, &conditions);

        assert!(result.is_err());
        assert_eq!(result.unwrap_err().code, AppErrorCode::InsufficientData);
    }

    // ---- Aggregate tests ----

    // Test 16: Aggregate AND: 2개 모두 match → all_matched=true
    #[test]
    fn aggregate_and_all_match() {
        let matches = vec![
            SignalMatch {
                condition_id: SignalConditionId::new("c1").unwrap(),
                matched: true,
                newly_crossed: false,
            },
            SignalMatch {
                condition_id: SignalConditionId::new("c2").unwrap(),
                matched: true,
                newly_crossed: false,
            },
        ];

        let aggregate = aggregate_matches(&matches);

        assert!(aggregate.all_matched);
        assert!(aggregate.any_matched);
    }

    // Test 17: Aggregate OR: 1개만 match → any_matched=true
    #[test]
    fn aggregate_or_one_match() {
        let matches = vec![
            SignalMatch {
                condition_id: SignalConditionId::new("c1").unwrap(),
                matched: true,
                newly_crossed: false,
            },
            SignalMatch {
                condition_id: SignalConditionId::new("c2").unwrap(),
                matched: false,
                newly_crossed: false,
            },
        ];

        let aggregate = aggregate_matches(&matches);

        assert!(!aggregate.all_matched);
        assert!(aggregate.any_matched);
    }

    // Test 18: Aggregate AND fail: 1개 miss → all_matched=false
    #[test]
    fn aggregate_and_one_miss() {
        let matches = vec![
            SignalMatch {
                condition_id: SignalConditionId::new("c1").unwrap(),
                matched: false,
                newly_crossed: false,
            },
            SignalMatch {
                condition_id: SignalConditionId::new("c2").unwrap(),
                matched: true,
                newly_crossed: false,
            },
        ];

        let aggregate = aggregate_matches(&matches);

        assert!(!aggregate.all_matched);
        assert!(aggregate.any_matched);
    }

    // Test 19: Aggregate empty → all=false, any=false
    #[test]
    fn aggregate_empty() {
        let matches: Vec<SignalMatch> = vec![];

        let aggregate = aggregate_matches(&matches);

        assert!(!aggregate.all_matched);
        assert!(!aggregate.any_matched);
    }

    // ---- Mixed trigger mode tests ----

    // Test 20: Mixed current + cross preset
    #[test]
    fn evaluate_signals_mixed_modes() {
        // c1: Current mode, RSI(14) lower, threshold=30 → current=25 <= 30 → matched
        // c2: Cross mode, RSI(9) upper, threshold=70 → prev=65<70 && current=75>=70 → crossed
        let mut rsi_by_period = HashMap::new();
        rsi_by_period.insert(14, Some(25.0));
        rsi_by_period.insert(9, Some(75.0));

        let mut prev_rsi_by_period = HashMap::new();
        prev_rsi_by_period.insert(14, Some(30.0));
        prev_rsi_by_period.insert(9, Some(65.0));

        let snapshot = IndicatorSnapshot {
            trade_date: "2026-07-14".to_string(),
            close: 100.0,
            rsi_by_period,
            mfi_by_period: HashMap::new(),
            bollinger_by_params: HashMap::new(),
            prev_close: Some(99.0),
            prev_rsi_by_period,
            prev_mfi_by_period: HashMap::new(),
            prev_bollinger_by_params: HashMap::new(),
        };
        let conditions = vec![
            rsi_lower_condition("c1", 14, 30.0, true), // Current: 25 <= 30 → true
            rsi_upper_cross_condition("c2", 9, 70.0, true), // Cross: 65<70 && 75>=70 → true
        ];

        let matches = evaluate_signals(&snapshot, &conditions).unwrap();

        assert_eq!(matches.len(), 2);
        // c1: current mode, matched, not newly_crossed
        assert_eq!(matches[0].condition_id.0, "c1");
        assert!(matches[0].matched);
        assert!(!matches[0].newly_crossed);
        // c2: cross mode, matched, newly_crossed
        assert_eq!(matches[1].condition_id.0, "c2");
        assert!(matches[1].matched);
        assert!(matches[1].newly_crossed);
    }

    // Test 21: evaluate_signals with disabled condition
    #[test]
    fn evaluate_signals_skips_disabled() {
        let snapshot = make_snapshot(Some(25.0), None, None, 100.0);
        let conditions = vec![
            rsi_lower_condition("c1", 14, 30.0, true),
            rsi_upper_condition("c2", 14, 70.0, false),
        ];

        let matches = evaluate_signals(&snapshot, &conditions).unwrap();

        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].condition_id.0, "c1");
    }
}
