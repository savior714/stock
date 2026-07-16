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
/// length as input. Warm-up positions (`0..period-1`) are `None`; the first valid
/// value is at index `period - 1` and includes the close at that index.
pub fn calculate_bollinger(
    closes: &[f64],
    period: u32,
    multiplier: f64,
) -> AppResult<(IndicatorOutput, IndicatorOutput, IndicatorOutput)> {
    validate_period(period)?;
    assert_lengths_match(&[closes.len()])?;

    if closes.iter().any(|value| !value.is_finite()) {
        return Err(AppError::validation(
            "close values must be finite (no NaN or infinity)",
        ));
    }
    if !multiplier.is_finite() || multiplier <= 0.0 {
        return Err(AppError::validation(
            "Bollinger multiplier must be finite and greater than zero",
        ));
    }

    let period = period as usize;
    let len = closes.len();

    let mut lower = empty_output(len);
    let mut middle = empty_output(len);
    let mut upper = empty_output(len);

    if len < period {
        return Ok((lower, middle, upper));
    }

    for index in (period - 1)..len {
        let window = &closes[(index + 1 - period)..=index];
        let mean = window.iter().sum::<f64>() / period as f64;
        let variance = window
            .iter()
            .map(|value| (value - mean).powi(2))
            .sum::<f64>()
            / period as f64;
        let stddev = variance.sqrt();

        middle[index] = Some(mean);
        upper[index] = Some(mean + multiplier * stddev);
        lower[index] = Some(mean - multiplier * stddev);
    }

    Ok((lower, middle, upper))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn constant_series_all_equal_from_period_minus_one() {
        let closes = vec![100.0; 30];
        let (lower, middle, upper) = calculate_bollinger(&closes, 5, 2.0).unwrap();

        for index in 0..4 {
            assert!(lower[index].is_none());
            assert!(middle[index].is_none());
            assert!(upper[index].is_none());
        }

        for index in 4..30 {
            assert_eq!(lower[index], Some(100.0));
            assert_eq!(middle[index], Some(100.0));
            assert_eq!(upper[index], Some(100.0));
        }
    }

    #[test]
    fn length_equal_to_period_has_one_value() {
        let closes = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let (lower, middle, upper) = calculate_bollinger(&closes, 5, 2.0).unwrap();

        for index in 0..4 {
            assert!(lower[index].is_none());
            assert!(middle[index].is_none());
            assert!(upper[index].is_none());
        }

        let expected_stddev = 200.0_f64.sqrt();
        assert!((middle[4].unwrap() - 30.0).abs() < 1e-9);
        assert!((upper[4].unwrap() - (30.0 + 2.0 * expected_stddev)).abs() < 1e-9);
        assert!((lower[4].unwrap() - (30.0 - 2.0 * expected_stddev)).abs() < 1e-9);
    }

    #[test]
    fn rolling_window_includes_current_close() {
        let closes = vec![10.0, 20.0, 30.0, 40.0];
        let (_, middle, _) = calculate_bollinger(&closes, 3, 1.0).unwrap();

        assert!(middle[0].is_none());
        assert!(middle[1].is_none());
        assert_eq!(middle[2], Some(20.0));
        assert_eq!(middle[3], Some(30.0));
    }

    #[test]
    fn multiplier_changes_band_width() {
        let closes = vec![10.0, 20.0, 30.0, 40.0, 50.0];
        let (lower_one, middle_one, upper_one) =
            calculate_bollinger(&closes, 5, 1.0).unwrap();
        let (lower_two, middle_two, upper_two) =
            calculate_bollinger(&closes, 5, 2.0).unwrap();

        assert_eq!(middle_one[4], middle_two[4]);
        let width_one = upper_one[4].unwrap() - lower_one[4].unwrap();
        let width_two = upper_two[4].unwrap() - lower_two[4].unwrap();
        assert!((width_two - 2.0 * width_one).abs() < 1e-9);
    }

    #[test]
    fn warmup_boundary_period_twenty() {
        let closes: Vec<f64> = (0..25).map(|value| 100.0 + value as f64).collect();
        let (lower, middle, upper) = calculate_bollinger(&closes, 20, 2.0).unwrap();

        for index in 0..19 {
            assert!(lower[index].is_none());
            assert!(middle[index].is_none());
            assert!(upper[index].is_none());
        }
        assert!(lower[19].is_some());
        assert!(middle[19].is_some());
        assert!(upper[19].is_some());
    }

    #[test]
    fn known_fixture_period_three() {
        let closes = vec![2.0, 4.0, 6.0, 8.0, 10.0, 12.0];
        let (lower, middle, upper) = calculate_bollinger(&closes, 3, 1.0).unwrap();
        let expected_stddev = (8.0_f64 / 3.0).sqrt();

        assert!(middle[0].is_none());
        assert!(middle[1].is_none());

        for (index, expected_mean) in [(2, 4.0), (3, 6.0), (4, 8.0), (5, 10.0)] {
            assert!((middle[index].unwrap() - expected_mean).abs() < 1e-9);
            assert!((upper[index].unwrap() - (expected_mean + expected_stddev)).abs() < 1e-9);
            assert!((lower[index].unwrap() - (expected_mean - expected_stddev)).abs() < 1e-9);
        }
    }

    #[test]
    fn insufficient_data_returns_all_none() {
        let closes = vec![1.0, 2.0, 3.0];
        let (lower, middle, upper) = calculate_bollinger(&closes, 5, 2.0).unwrap();

        assert!(lower.iter().all(Option::is_none));
        assert!(middle.iter().all(Option::is_none));
        assert!(upper.iter().all(Option::is_none));
    }

    #[test]
    fn rejects_non_finite_close() {
        assert!(calculate_bollinger(&[10.0, f64::NAN, 30.0], 2, 2.0).is_err());
        assert!(calculate_bollinger(&[10.0, f64::INFINITY, 30.0], 2, 2.0).is_err());
    }

    #[test]
    fn rejects_invalid_multiplier() {
        assert!(calculate_bollinger(&[10.0, 20.0], 2, 0.0).is_err());
        assert!(calculate_bollinger(&[10.0, 20.0], 2, f64::NAN).is_err());
    }
}
