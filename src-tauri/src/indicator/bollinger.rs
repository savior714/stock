use crate::error::{AppError, AppResult};

use super::{assert_lengths_match, empty_output, validate_period, IndicatorOutput};

/// Bollinger Bands result for a single bar.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BollingerBands {
    pub lower: Option<f64>,
    pub middle: Option<f64>,
    pub upper: Option<f64>,
}

/// Calculate Bollinger Bands.
///
/// - Middle line: SMA (Simple Moving Average) of `close` over `period`.
/// - Standard deviation: population standard deviation (`ddof = 0`).
/// - Upper band = middle + `multiplier` * stddev
/// - Lower band = middle − `multiplier` * stddev
///
/// Returns three `IndicatorOutput` vectors (lower, middle, upper), each the same
/// length as input. Warm-up positions (`0..period−1`) are `None`; first valid
/// value is at index `period` (0-indexed).
///
/// Input: `close` slice, `period`, `multiplier`.
/// Output: `(lower, middle, upper)` triple.
pub fn calculate_bollinger(
    closes: &[f64],
    period: u32,
    multiplier: f64,
) -> AppResult<(IndicatorOutput, IndicatorOutput, IndicatorOutput)> {
    validate_period(period)?;
    assert_lengths_match(&[closes.len()])?;

    // Reject non-finite values.
    if closes.iter().any(|v| !v.is_finite()) {
        return Err(AppError::validation(
            "close values must be finite (no NaN or infinity)",
        ));
    }

    let period = period as usize;
    let len = closes.len();

    let mut lower = empty_output(len);
    let mut middle = empty_output(len);
    let mut upper = empty_output(len);

    // First valid value is at index `period`; need at least `period + 1` closes.
    if len <= period {
        return Ok((lower, middle, upper));
    }

    for i in period..len {
        let window = &closes[i - period..i];
        let sum: f64 = window.iter().sum();
        let mean = sum / period as f64;

        let variance: f64 = window.iter().map(|&v| (v - mean).powi(2)).sum::<f64>() / period as f64;
        let stddev = variance.sqrt();

        let mid = mean;
        let up = mid + multiplier * stddev;
        let lo = mid - multiplier * stddev;

        middle[i] = Some(mid);
        upper[i] = Some(up);
        lower[i] = Some(lo);
    }

    Ok((lower, middle, upper))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Constant series: all bands collapse to the constant value ----

    #[test]
    fn constant_series_all_equal() {
        let closes = vec![100.0; 30];
        let (lower, middle, upper) = calculate_bollinger(&closes, 5, 2.0).unwrap();

        assert_eq!(lower.len(), 30);
        assert_eq!(middle.len(), 30);
        assert_eq!(upper.len(), 30);

        // Warm-up: indices 0..4 are None
        for i in 0..5 {
            assert!(lower[i].is_none(), "lower[{}] should be None", i);
            assert!(middle[i].is_none(), "middle[{}] should be None", i);
            assert!(upper[i].is_none(), "upper[{}] should be None", i);
        }

        // From index 5 onward: all three bands = 100
        for i in 5..30 {
            assert!(
                (lower[i].unwrap() - 100.0).abs() < 1e-9,
                "lower[{}] expected 100",
                i
            );
            assert!(
                (middle[i].unwrap() - 100.0).abs() < 1e-9,
                "middle[{}] expected 100",
                i
            );
            assert!(
                (upper[i].unwrap() - 100.0).abs() < 1e-9,
                "upper[{}] expected 100",
                i
            );
        }
    }

    // ---- Length exactly equal to period: all None (need period+1 for first value) ----

    #[test]
    fn length_equals_period_all_none() {
        let closes = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let (lower, middle, upper) = calculate_bollinger(&closes, 5, 2.0).unwrap();

        assert!(lower.iter().all(|v| v.is_none()));
        assert!(middle.iter().all(|v| v.is_none()));
        assert!(upper.iter().all(|v| v.is_none()));
    }

    // ---- Length = period + 1: only last index is Some ----

    #[test]
    fn length_period_plus_one_last_only() {
        // 6 closes, period=5 → first valid at index 5
        let closes = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0];
        let (lower, middle, upper) = calculate_bollinger(&closes, 5, 2.0).unwrap();

        assert_eq!(lower.len(), 6);
        for i in 0..5 {
            assert!(lower[i].is_none());
            assert!(middle[i].is_none());
            assert!(upper[i].is_none());
        }
        assert!(lower[5].is_some());
        assert!(middle[5].is_some());
        assert!(upper[5].is_some());

        // window = [10, 20, 30, 40, 50], mean = 30
        // stddev = sqrt(((10-30)^2+(20-30)^2+(30-30)^2+(40-30)^2+(50-30)^2)/5)
        // = sqrt((400+100+0+100+400)/5) = sqrt(200) ≈ 14.1421
        let expected_mean = 30.0;
        let expected_stddev = 200.0_f64.sqrt();
        assert!((middle[5].unwrap() - expected_mean).abs() < 1e-9);
        assert!((upper[5].unwrap() - (expected_mean + 2.0 * expected_stddev)).abs() < 1e-9);
        assert!((lower[5].unwrap() - (expected_mean - 2.0 * expected_stddev)).abs() < 1e-9);
    }

    // ---- Different multipliers produce different band widths ----

    #[test]
    fn multiplier_1_vs_2() {
        // 6 closes, period=5 → one value at index 5
        let closes = vec![10.0, 20.0, 30.0, 40.0, 50.0, 60.0];
        let (lo1, mid1, up1) = calculate_bollinger(&closes, 5, 1.0).unwrap();
        let (lo2, _mid2, up2) = calculate_bollinger(&closes, 5, 2.0).unwrap();

        // Middle lines are identical
        assert!((mid1[5].unwrap() - _mid2[5].unwrap()).abs() < 1e-9);

        // multiplier=2 band is wider
        let width_1 = up1[5].unwrap() - lo1[5].unwrap();
        let width_2 = up2[5].unwrap() - lo2[5].unwrap();
        assert!(
            (width_2 - 2.0 * width_1).abs() < 1e-9,
            "width_2 should be 2x width_1"
        );
    }

    // ---- Non-finite input rejection ----

    #[test]
    fn rejects_nan() {
        let closes = vec![10.0, f64::NAN, 30.0];
        assert!(calculate_bollinger(&closes, 2, 2.0).is_err());
    }

    #[test]
    fn rejects_infinity() {
        let closes = vec![10.0, f64::INFINITY, 30.0];
        assert!(calculate_bollinger(&closes, 2, 2.0).is_err());
    }

    #[test]
    fn rejects_neg_infinity() {
        let closes = vec![10.0, f64::NEG_INFINITY, 30.0];
        assert!(calculate_bollinger(&closes, 2, 2.0).is_err());
    }

    // ---- Warm-up boundary: period=20, indices 0..19 are None ----

    #[test]
    fn warmup_boundary_period_20() {
        let closes: Vec<f64> = (0..25).map(|v| 100.0 + v as f64).collect();
        let (lower, middle, upper) = calculate_bollinger(&closes, 20, 2.0).unwrap();

        assert_eq!(lower.len(), 25);
        for i in 0..20 {
            assert!(lower[i].is_none(), "lower[{}] should be None", i);
            assert!(middle[i].is_none(), "middle[{}] should be None", i);
            assert!(upper[i].is_none(), "upper[{}] should be None", i);
        }
        // Index 20 is the first valid value
        assert!(lower[20].is_some(), "lower[20] should be Some");
        assert!(middle[20].is_some(), "middle[20] should be Some");
        assert!(upper[20].is_some(), "upper[20] should be Some");
    }

    // ---- Hand-calculated fixture: period=3, multiplier=1.0 ----

    #[test]
    fn known_fixture_period_3_multiplier_1() {
        // closes = [2, 4, 6, 8, 10, 12], period=3, multiplier=1.0
        // First valid at index 3.
        //
        // i=3: window=[2,4,6], mean=4, var=8/3, std=sqrt(8/3)≈1.633
        // i=4: window=[4,6,8], mean=6, var=8/3
        // i=5: window=[6,8,10], mean=8, var=8/3
        let closes = vec![2.0, 4.0, 6.0, 8.0, 10.0, 12.0];
        let (lower, middle, upper) = calculate_bollinger(&closes, 3, 1.0).unwrap();

        // Warm-up: indices 0..2 are None
        for i in 0..3 {
            assert!(lower[i].is_none());
            assert!(middle[i].is_none());
            assert!(upper[i].is_none());
        }

        let expected_std = (8.0_f64 / 3.0_f64).sqrt(); // ≈1.63299

        // i=3: mean=4
        assert!((middle[3].unwrap() - 4.0).abs() < 1e-9);
        assert!((upper[3].unwrap() - (4.0 + expected_std)).abs() < 1e-9);
        assert!((lower[3].unwrap() - (4.0 - expected_std)).abs() < 1e-9);

        // i=4: mean=6
        assert!((middle[4].unwrap() - 6.0).abs() < 1e-9);
        assert!((upper[4].unwrap() - (6.0 + expected_std)).abs() < 1e-9);
        assert!((lower[4].unwrap() - (6.0 - expected_std)).abs() < 1e-9);

        // i=5: mean=8
        assert!((middle[5].unwrap() - 8.0).abs() < 1e-9);
        assert!((upper[5].unwrap() - (8.0 + expected_std)).abs() < 1e-9);
        assert!((lower[5].unwrap() - (8.0 - expected_std)).abs() < 1e-9);
    }

    // ---- Insufficient data: fewer closes than period ----

    #[test]
    fn insufficient_data_all_none() {
        let closes = vec![1.0, 2.0, 3.0];
        let (lower, middle, upper) = calculate_bollinger(&closes, 5, 2.0).unwrap();
        assert!(lower.iter().all(|v| v.is_none()));
        assert!(middle.iter().all(|v| v.is_none()));
        assert!(upper.iter().all(|v| v.is_none()));
    }
}
