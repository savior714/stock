use crate::error::AppResult;

use super::{assert_lengths_match, empty_output, validate_period, IndicatorOutput};

/// Calculate Money Flow Index (MFI) using Wilder smoothing.
///
/// - Typical Price = `(high + low + close) / 3`
/// - Raw Money Flow = `typical_price * volume` (f64)
/// - Typical price > previous → positive flow; < previous → negative flow; equal → neither.
/// - Wilder smoothing (same as RSI).
/// - Negative flow = 0, positive > 0 → 100.
/// - Both zero → 50.
/// - First valid value at index `period` (0-indexed); indices `0..period` are `None`.
///
/// Input: `high`, `low`, `close` (each `&[f64]`), `volume: &[u64]`, `period: u32`.
/// Output: `IndicatorOutput` of same length as input.
pub fn calculate_mfi(
    highs: &[f64],
    lows: &[f64],
    closes: &[f64],
    volumes: &[u64],
    period: u32,
) -> AppResult<IndicatorOutput> {
    validate_period(period)?;
    assert_lengths_match(&[highs.len(), lows.len(), closes.len(), volumes.len()])?;

    let period = period as usize;
    let len = highs.len();
    let mut output = empty_output(len);

    // Need at least period + 1 bars to compute one MFI value.
    if len < period + 1 {
        return Ok(output);
    }

    // Compute typical prices
    let typical: Vec<f64> = highs
        .iter()
        .zip(lows.iter())
        .zip(closes.iter())
        .map(|((&h, &l), &c)| (h + l + c) / 3.0)
        .collect();

    // Compute raw money flow and classify into positive/negative
    let mut pos_flow: Vec<f64> = vec![0.0; len];
    let mut neg_flow: Vec<f64> = vec![0.0; len];

    for i in 1..len {
        let raw = typical[i] * volumes[i] as f64;
        if typical[i] > typical[i - 1] {
            pos_flow[i] = raw;
        } else if typical[i] < typical[i - 1] {
            neg_flow[i] = raw;
        }
        // equal → neither
    }

    // First average: simple mean of first `period` flow values (indices 1..=period)
    let mut avg_pos = pos_flow[1..=period].iter().sum::<f64>() / period as f64;
    let mut avg_neg = neg_flow[1..=period].iter().sum::<f64>() / period as f64;

    // MFI at index `period`
    output[period] = Some(compute_mfi_value(avg_pos, avg_neg));

    // Wilder smoothing for subsequent indices
    for i in (period + 1)..len {
        avg_pos = (avg_pos * (period - 1) as f64 + pos_flow[i]) / period as f64;
        avg_neg = (avg_neg * (period - 1) as f64 + neg_flow[i]) / period as f64;
        output[i] = Some(compute_mfi_value(avg_pos, avg_neg));
    }

    Ok(output)
}

/// Compute MFI value from smoothed positive and negative money flow.
fn compute_mfi_value(avg_pos: f64, avg_neg: f64) -> f64 {
    if avg_neg == 0.0 {
        if avg_pos > 0.0 {
            return 100.0;
        }
        return 50.0; // both zero
    }
    let ratio = avg_pos / avg_neg;
    100.0 - (100.0 / (1.0 + ratio))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- positive-only: typical price keeps rising → 100 ----

    #[test]
    fn positive_only_mfi_100() {
        // Monotonically increasing typical price → all positive flow
        let highs: Vec<f64> = (10..=30).map(|v| v as f64).collect();
        let lows: Vec<f64> = (0..=20).map(|v| v as f64).collect();
        let closes: Vec<f64> = (5..=25).map(|v| v as f64).collect();
        let volumes = vec![100u64; 21];
        let period: usize = 5;

        let result = calculate_mfi(&highs, &lows, &closes, &volumes, 5).unwrap();
        assert_eq!(result.len(), 21);

        for (i, val) in result.iter().enumerate().take(period) {
            assert!(val.is_none(), "index {} should be None", i);
        }
        for (i, val) in result.iter().enumerate().skip(period) {
            let v = val.unwrap();
            assert!(
                (v - 100.0).abs() < 1e-9,
                "index {} expected 100, got {}",
                i,
                v
            );
        }
    }

    // ---- negative-only: typical price keeps falling → 0 ----

    #[test]
    fn negative_only_mfi_0() {
        // Monotonically decreasing typical price → all negative flow
        let highs: Vec<f64> = (0..=20).map(|v| (30 - v) as f64).collect();
        let lows: Vec<f64> = (0..=20).map(|v| (20 - v) as f64).collect();
        let closes: Vec<f64> = (0..=20).map(|v| (25 - v) as f64).collect();
        let volumes = vec![100u64; 21];
        let period: usize = 5;

        let result = calculate_mfi(&highs, &lows, &closes, &volumes, 5).unwrap();
        for (i, val) in result.iter().enumerate().take(period) {
            assert!(val.is_none(), "index {} should be None", i);
        }
        for (i, val) in result.iter().enumerate().skip(period) {
            let v = val.unwrap();
            assert!((v - 0.0).abs() < 1e-9, "index {} expected 0, got {}", i, v);
        }
    }

    // ---- flat: typical price unchanged → 50 ----

    #[test]
    fn flat_price_mfi_50() {
        let highs = vec![10.0; 30];
        let lows = vec![8.0; 30];
        let closes = vec![9.0; 30];
        let volumes = vec![100u64; 30];
        let period: usize = 5;

        let result = calculate_mfi(&highs, &lows, &closes, &volumes, 5).unwrap();
        for val in result.iter().take(period) {
            assert!(val.is_none());
        }
        for (i, val) in result.iter().enumerate().skip(period) {
            let v = val.unwrap();
            assert!(
                (v - 50.0).abs() < 1e-9,
                "index {} expected 50, got {}",
                i,
                v
            );
        }
    }

    // ---- zero volume: positive/negative both 0 → 50 ----

    #[test]
    fn zero_volume_mfi_50() {
        // Price changes but volume is always 0 → raw money flow = 0
        let highs = vec![10.0, 12.0, 11.0, 13.0, 12.0, 14.0];
        let lows = vec![8.0, 10.0, 9.0, 11.0, 10.0, 12.0];
        let closes = vec![9.0, 11.0, 10.0, 12.0, 11.0, 13.0];
        let volumes = vec![0u64; 6];
        let period: usize = 2;

        let result = calculate_mfi(&highs, &lows, &closes, &volumes, 2).unwrap();
        for val in result.iter().take(period) {
            assert!(val.is_none());
        }
        for (i, val) in result.iter().enumerate().skip(period) {
            let v = val.unwrap();
            assert!(
                (v - 50.0).abs() < 1e-9,
                "index {} expected 50 (zero volume), got {}",
                i,
                v
            );
        }
    }

    // ---- warm-up boundary: period=14, index 0..13 are None ----

    #[test]
    fn warmup_boundary_period_14() {
        let n = 20;
        let highs: Vec<f64> = (0..n).map(|v| 100.0 + v as f64 * 2.0).collect();
        let lows: Vec<f64> = (0..n).map(|v| 90.0 + v as f64 * 2.0).collect();
        let closes: Vec<f64> = (0..n).map(|v| 95.0 + v as f64 * 2.0).collect();
        let volumes = vec![100u64; n];

        let result = calculate_mfi(&highs, &lows, &closes, &volumes, 14).unwrap();
        assert_eq!(result.len(), n);
        for (i, val) in result.iter().enumerate().take(14) {
            assert!(val.is_none(), "index {} should be None", i);
        }
        assert!(result[14].is_some(), "index 14 should be Some");
        assert!(result[15].is_some());
    }

    // ---- slice length mismatch → error ----

    #[test]
    fn rejects_mismatched_lengths() {
        let highs = vec![10.0; 15];
        let lows = vec![8.0; 15];
        let closes = vec![9.0; 14]; // one short
        let volumes = vec![100u64; 15];

        assert!(calculate_mfi(&highs, &lows, &closes, &volumes, 5).is_err());
    }

    // ---- known fixture (period=3, hand-calculated) ----

    #[test]
    fn known_fixture_period_3() {
        // highs  = [10, 12, 11, 13, 12, 14]
        // lows   = [8,  10, 9,  11, 10, 12]
        // closes = [9,  11, 10, 12, 11, 13]
        // volumes = [100; 6]
        //
        // typical = [9, 11, 10, 12, 11, 13]
        // raw     = [900, 1100, 1000, 1200, 1100, 1300]
        // change:  i=1 up, i=2 down, i=3 up, i=4 down, i=5 up
        // pos: [0, 1100, 0, 1200, 0, 1300]
        // neg: [0, 0, 1000, 0, 1100, 0]
        //
        // period=3 (floating-point, verified by running):
        //   i=3: 69.697
        //   i=4: 46.465
        //   i=5: 66.349

        let highs = vec![10.0, 12.0, 11.0, 13.0, 12.0, 14.0];
        let lows = vec![8.0, 10.0, 9.0, 11.0, 10.0, 12.0];
        let closes = vec![9.0, 11.0, 10.0, 12.0, 11.0, 13.0];
        let volumes = vec![100u64; 6];

        let result = calculate_mfi(&highs, &lows, &closes, &volumes, 3).unwrap();

        assert!(result[0].is_none());
        assert!(result[1].is_none());
        assert!(result[2].is_none());

        assert!((result[3].unwrap() - 69.697).abs() < 1e-2);
        assert!((result[4].unwrap() - 46.465).abs() < 1e-2);
        assert!((result[5].unwrap() - 66.349).abs() < 1e-2);
    }

    // ---- insufficient data returns all None ----

    #[test]
    fn insufficient_data_returns_all_none() {
        // period=5 needs at least 6 bars, but we only have 5
        let highs = vec![10.0; 5];
        let lows = vec![8.0; 5];
        let closes = vec![9.0; 5];
        let volumes = vec![100u64; 5];

        let result = calculate_mfi(&highs, &lows, &closes, &volumes, 5).unwrap();
        assert!(result.iter().all(|v| v.is_none()));
    }

    // ---- exactly period+1 bars → one valid value ----

    #[test]
    fn exactly_period_plus_one() {
        // period=2, 3 bars → one MFI value at index 2
        let highs = vec![10.0, 12.0, 11.0];
        let lows = vec![8.0, 10.0, 9.0];
        let closes = vec![9.0, 11.0, 10.0];
        let volumes = vec![100u64; 3];

        let result = calculate_mfi(&highs, &lows, &closes, &volumes, 2).unwrap();
        assert!(result[0].is_none());
        assert!(result[1].is_none());
        assert!(result[2].is_some());
    }
}
