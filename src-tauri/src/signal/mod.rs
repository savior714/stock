use crate::domain::{
    BollingerKey, BollingerValue, IndicatorKind, SignalCondition, SignalMatch, SignalSide,
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
}
