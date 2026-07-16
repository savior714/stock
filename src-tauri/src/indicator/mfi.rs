use crate::error::{AppError, AppResult};

use super::{assert_lengths_match, empty_output, validate_period, IndicatorOutput};

/// Calculate Money Flow Index (MFI) from a fixed rolling window.
///
/// - Typical Price = `(high + low + close) / 3`
/// - Raw Money Flow = `typical_price * volume`
/// - Typical price > previous → positive flow; < previous → negative flow; equal → neither.
/// - Each value uses the most recent `period` classified money-flow entries.
/// - Negative flow = 0, positive > 0 → 100.
/// - Both zero → 50.
/// - First valid value is at index `period`; the preceding positions are `None`.
pub fn calculate_mfi(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    volumes: &[u64],
    period: u32,
) -> AppResult<IndicatorOutput> {
    validate_period(period)?;
    assert_lengths_match(&[highs.len(), lows.len(), closes.len(), volumes.len()])?;

    if highs
        .iter()
        .chain(lows)
        .chain(closes)
        .any(|value| !value.is_finite())
    {
        return Err(AppError::validation(
            "MFI price inputs must be finite (no NaN or infinity)",
        ));
    }

    let period = period as usize;
    let len = highs.len();
    let mut output = empty_output(len);

    if len < period + 1 {
        return Ok(output);
    }

    let typical_prices: Vec<f64> = highs
        .iter()
        .zip(lows)
        .zip(closes)
        .map(|((&high, &low), &close)| (high + low + close) / 3.0)
        .collect();

    let mut positive_flow = vec![0.0; len];
    let mut negative_flow = vec![0.0; len];

    for index in 1..len {
        let raw_flow = typical_prices[index] * volumes[index] as f64;
        if !raw_flow.is_finite() {
            return Err(AppError::validation("MFI raw money flow must be finite"));
        }

        if typical_prices[index] > typical_prices[index - 1] {
            positive_flow[index] = raw_flow;
        } else if typical_prices[index] < typical_prices[index - 1] {
            negative_flow[index] = raw_flow;
        }
    }

    let mut positive_sum = positive_flow[1..=period].iter().sum::<f64>();
    let mut negative_sum = negative_flow[1..=period].iter().sum::<f64>();
    output[period] = Some(compute_mfi_value(positive_sum, negative_sum));

    for index in (period + 1)..len {
        let expired_index = index - period;
        positive_sum += positive_flow[index] - positive_flow[expired_index];
        negative_sum += negative_flow[index] - negative_flow[expired_index];
        output[index] = Some(compute_mfi_value(positive_sum, negative_sum));
    }

    Ok(output)
}

fn compute_mfi_value(positive_sum: f64, negative_sum: f64) -> f64 {
    if negative_sum == 0.0 {
        if positive_sum > 0.0 {
            return 100.0;
        }
        return 50.0;
    }

    let ratio = positive_sum / negative_sum;
    100.0 - (100.0 / (1.0 + ratio))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positive_only_mfi_is_one_hundred() {
        let highs: Vec<f64> = (10..=30).map(|value| value as f64).collect();
        let lows: Vec<f64> = (0..=20).map(|value| value as f64).collect();
        let closes: Vec<f64> = (5..=25).map(|value| value as f64).collect();
        let volumes = vec![100_u64; 21];
        let result = calculate_mfi(&highs, &lows, &closes, &volumes, 5).unwrap();

        for value in result.iter().take(5) {
            assert!(value.is_none());
        }
        for value in result.iter().skip(5) {
            assert_eq!(*value, Some(100.0));
        }
    }

    #[test]
    fn negative_only_mfi_is_zero() {
        let highs: Vec<f64> = (0..=20).map(|value| (30 - value) as f64).collect();
        let lows: Vec<f64> = (0..=20).map(|value| (20 - value) as f64).collect();
        let closes: Vec<f64> = (0..=20).map(|value| (25 - value) as f64).collect();
        let volumes = vec![100_u64; 21];
        let result = calculate_mfi(&highs, &lows, &closes, &volumes, 5).unwrap();

        for value in result.iter().take(5) {
            assert!(value.is_none());
        }
        for value in result.iter().skip(5) {
            assert_eq!(*value, Some(0.0));
        }
    }

    #[test]
    fn flat_or_zero_volume_mfi_is_fifty() {
        let highs = vec![10.0; 10];
        let lows = vec![8.0; 10];
        let closes = vec![9.0; 10];
        let volumes = vec![0_u64; 10];
        let result = calculate_mfi(&highs, &lows, &closes, &volumes, 3).unwrap();

        for value in result.iter().skip(3) {
            assert_eq!(*value, Some(50.0));
        }
    }

    #[test]
    fn rolling_window_expires_old_positive_flow() {
        let highs = vec![11.0, 13.0, 15.0, 14.0, 13.0];
        let lows = vec![9.0, 11.0, 13.0, 12.0, 11.0];
        let closes = vec![10.0, 12.0, 14.0, 13.0, 12.0];
        let volumes = vec![100_u64; 5];
        let result = calculate_mfi(&highs, &lows, &closes, &volumes, 2).unwrap();

        assert_eq!(result[2], Some(100.0));
        let expected_middle = 1_400.0 / (1_400.0 + 1_300.0) * 100.0;
        assert!((result[3].unwrap() - expected_middle).abs() < 1e-9);
        assert_eq!(result[4], Some(0.0));
    }

    #[test]
    fn known_fixture_period_three() {
        let highs = vec![10.0, 12.0, 11.0, 13.0, 12.0, 14.0];
        let lows = vec![8.0, 10.0, 9.0, 11.0, 10.0, 12.0];
        let closes = vec![9.0, 11.0, 10.0, 12.0, 11.0, 13.0];
        let volumes = vec![100_u64; 6];
        let result = calculate_mfi(&highs, &lows, &closes, &volumes, 3).unwrap();

        assert!(result[0].is_none());
        assert!(result[1].is_none());
        assert!(result[2].is_none());
        assert!((result[3].unwrap() - 69.696_969_696_969_7).abs() < 1e-9);
        assert!((result[4].unwrap() - 36.363_636_363_636_37).abs() < 1e-9);
        assert!((result[5].unwrap() - 69.444_444_444_444_44).abs() < 1e-9);
    }

    #[test]
    fn warmup_boundary_period_fourteen() {
        let count = 20;
        let highs: Vec<f64> = (0..count).map(|value| 100.0 + value as f64 * 2.0).collect();
        let lows: Vec<f64> = (0..count).map(|value| 90.0 + value as f64 * 2.0).collect();
        let closes: Vec<f64> = (0..count).map(|value| 95.0 + value as f64 * 2.0).collect();
        let volumes = vec![100_u64; count];
        let result = calculate_mfi(&highs, &lows, &closes, &volumes, 14).unwrap();

        for value in result.iter().take(14) {
            assert!(value.is_none());
        }
        assert!(result[14].is_some());
    }

    #[test]
    fn insufficient_data_returns_all_none() {
        let highs = vec![10.0; 5];
        let lows = vec![8.0; 5];
        let closes = vec![9.0; 5];
        let volumes = vec![100_u64; 5];
        let result = calculate_mfi(&highs, &lows, &closes, &volumes, 5).unwrap();

        assert!(result.iter().all(Option::is_none));
    }

    #[test]
    fn rejects_mismatched_or_non_finite_inputs() {
        assert!(calculate_mfi(&[10.0; 4], &[8.0; 4], &[9.0; 3], &[100; 4], 2).is_err());
        assert!(calculate_mfi(
            &[10.0, f64::NAN, 12.0],
            &[8.0, 9.0, 10.0],
            &[9.0, 10.0, 11.0],
            &[100; 3],
            2,
        )
        .is_err());
    }
}
