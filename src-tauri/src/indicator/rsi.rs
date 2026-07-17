use crate::error::AppResult;

use super::{assert_lengths_match, empty_output, validate_period, IndicatorOutput};

/// Calculate Relative Strength Index (RSI) using Wilder smoothing.
///
/// - First average gain/loss: simple mean of the first `period` price changes.
/// - Subsequent averages: Wilder smoothing `prev * (period - 1) + current / period`.
/// - Loss = 0, gain > 0 → 100.
/// - Gain = 0, loss = 0 → 50.
/// - First valid value at index `period` (0-indexed); indices `0..period` are `None`.
///
/// Input: `close` slice, `period`.
/// Output: `IndicatorOutput` of same length as input.
pub fn calculate_rsi(closes: &[f64], period: u32) -> AppResult<IndicatorOutput> {
    validate_period(period)?;
    assert_lengths_match(&[closes.len()])?;

    let period = period as usize;
    let len = closes.len();
    let mut output = empty_output(len);

    // Need at least period + 1 closes to compute one RSI value (period changes + first average).
    if len < period + 1 {
        return Ok(output);
    }

    // Compute price changes (close[i] - close[i-1]) for i = 1..len
    let mut gains: Vec<f64> = vec![0.0; len];
    let mut losses: Vec<f64> = vec![0.0; len];

    for i in 1..len {
        let change = closes[i] - closes[i - 1];
        if change > 0.0 {
            gains[i] = change;
        } else {
            losses[i] = (-change).max(0.0);
        }
    }

    // First average: simple mean of first `period` changes (indices 1..=period)
    let mut avg_gain = gains[1..=period].iter().sum::<f64>() / period as f64;
    let mut avg_loss = losses[1..=period].iter().sum::<f64>() / period as f64;

    // RSI at index `period`
    output[period] = Some(compute_rsi_value(avg_gain, avg_loss));

    // Wilder smoothing for subsequent indices
    for i in (period + 1)..len {
        avg_gain = (avg_gain * (period - 1) as f64 + gains[i]) / period as f64;
        avg_loss = (avg_loss * (period - 1) as f64 + losses[i]) / period as f64;
        output[i] = Some(compute_rsi_value(avg_gain, avg_loss));
    }

    Ok(output)
}

/// Compute RSI value from average gain and loss.
fn compute_rsi_value(avg_gain: f64, avg_loss: f64) -> f64 {
    if avg_loss == 0.0 {
        if avg_gain > 0.0 {
            return 100.0;
        }
        return 50.0; // both zero
    }
    let rs = avg_gain / avg_loss;
    100.0 - (100.0 / (1.0 + rs))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Edge cases ----

    #[test]
    fn uptrend_only_rsi_100() {
        // Monotonically increasing: every change is positive → RSI = 100
        let closes: Vec<f64> = (100..=120).map(|v| v as f64).collect();
        let result = calculate_rsi(&closes, 5).unwrap();
        assert_eq!(result.len(), closes.len());
        // First 5 are None, rest are Some(100.0)
        for (i, val) in result.iter().enumerate().take(5) {
            assert!(val.is_none(), "index {i} should be None");
        }
        for (i, val) in result.iter().enumerate().skip(5) {
            let v = val.unwrap();
            assert!(
                (v - 100.0).abs() < 1e-9,
                "index {i} expected 100, got {v}"
            );
        }
    }

    #[test]
    fn downtrend_only_rsi_0() {
        // Monotonically decreasing: every change is negative → RSI = 0
        let closes: Vec<f64> = (0..=20).map(|v| (100 - v) as f64).collect();
        let result = calculate_rsi(&closes, 5).unwrap();
        for (i, val) in result.iter().enumerate().take(5) {
            assert!(val.is_none(), "index {i} should be None");
        }
        for (i, val) in result.iter().enumerate().skip(5) {
            let v = val.unwrap();
            assert!((v - 0.0).abs() < 1e-9, "index {i} expected 0, got {v}");
        }
    }

    #[test]
    fn flat_price_rsi_50() {
        let closes = vec![50.0; 30];
        let result = calculate_rsi(&closes, 5).unwrap();
        for val in result.iter().take(5) {
            assert!(val.is_none());
        }
        for (i, val) in result.iter().enumerate().skip(5) {
            let v = val.unwrap();
            assert!(
                (v - 50.0).abs() < 1e-9,
                "index {i} expected 50, got {v}"
            );
        }
    }

    // ---- Warm-up boundary ----

    #[test]
    fn warmup_boundary_period_14() {
        let closes: Vec<f64> = (0..20).map(|v| 100.0 + v as f64).collect();
        let result = calculate_rsi(&closes, 14).unwrap();
        assert_eq!(result.len(), 20);
        for (i, val) in result.iter().enumerate().take(14) {
            assert!(val.is_none(), "index {i} should be None");
        }
        assert!(result[14].is_some(), "index 14 should be Some");
        assert!(result[15].is_some());
    }

    // ---- Threshold values ----

    #[test]
    fn rsi_exactly_30() {
        // RS = avg_gain / avg_loss. RSI = 100 - 100/(1+RS).
        // RSI = 30 → RS = 1/2.333... ≈ 0.3
        // With period=2, we need avg_gain/avg_loss = 30/70 = 0.42857...
        // Let's construct: changes [+1, -1] → avg_gain=0.5, avg_loss=0.5 → RS=1 → RSI=50
        // For RSI=30: RS = 30/70. avg_gain = 3, avg_loss = 7 → RS=3/7
        // period=2: changes [+3, -7] over 2 periods. close: [100, 103, 96]
        // avg_gain = (3+0)/2 = 1.5, avg_loss = (0+7)/2 = 3.5 → RS = 1.5/3.5 = 3/7
        // RSI = 100 - 100/(1+3/7) = 100 - 100*7/10 = 30
        let closes = vec![100.0, 103.0, 96.0];
        let result = calculate_rsi(&closes, 2).unwrap();
        assert!(result[0].is_none());
        assert!(result[1].is_none());
        let val = result[2].unwrap();
        assert!((val - 30.0).abs() < 1e-6, "expected RSI=30, got {val}");
    }

    #[test]
    fn rsi_exactly_70() {
        // RSI = 70 → RS = 7/3. avg_gain=7, avg_loss=3.
        // period=2: changes [+7, -3] → close: [100, 107, 104]
        // avg_gain = (7+0)/2 = 3.5, avg_loss = (0+3)/2 = 1.5 → RS = 3.5/1.5 = 7/3
        // RSI = 100 - 100/(1+7/3) = 100 - 100*3/10 = 70
        let closes = vec![100.0, 107.0, 104.0];
        let result = calculate_rsi(&closes, 2).unwrap();
        assert!(result[0].is_none());
        assert!(result[1].is_none());
        let val = result[2].unwrap();
        assert!((val - 70.0).abs() < 1e-6, "expected RSI=70, got {val}");
    }

    // ---- Known fixture (period=2, hand-calculated) ----

    #[test]
    fn known_fixture_period_2() {
        // close = [10, 11, 9, 10, 8]
        // changes: [+1, -2, +1, -2]
        // period=2:
        //   i=2: avg_gain=(1+0)/2=0.5, avg_loss=(0+2)/2=1.0 → RS=0.5 → RSI=33.333...
        //   i=3: avg_gain=(0.5*1+1)/2=0.75, avg_loss=(1.0*1+0)/2=0.5 → RS=1.5 → RSI=60.0
        //   i=4: avg_gain=(0.75*1+0)/2=0.375, avg_loss=(0.5*1+2)/2=1.25 → RS=0.3 → RSI=23.0769...
        let closes = vec![10.0, 11.0, 9.0, 10.0, 8.0];
        let result = calculate_rsi(&closes, 2).unwrap();

        assert!(result[0].is_none());
        assert!(result[1].is_none());

        // i=2: RSI = 100 - 100/(1+0.5) = 100 - 66.667 = 33.333
        assert!((result[2].unwrap() - 33.333).abs() < 1e-3);

        // i=3: RSI = 100 - 100/(1+1.5) = 100 - 40.0 = 60.0
        assert!((result[3].unwrap() - 60.0).abs() < 1e-6);

        // i=4: RSI = 100 - 100/(1+0.3) = 100 - 76.923 = 23.077
        assert!((result[4].unwrap() - 23.077).abs() < 1e-3);
    }

    // ---- Insufficient data ----

    #[test]
    fn insufficient_data_returns_all_none() {
        // period=5 needs at least 6 closes, but we only have 5
        let closes = vec![1.0, 2.0, 3.0, 4.0, 5.0];
        let result = calculate_rsi(&closes, 5).unwrap();
        assert!(result.iter().all(|v| v.is_none()));
    }

    #[test]
    fn exactly_period_plus_one() {
        // period=2, 3 closes → exactly one RSI value at index 2
        let closes = vec![10.0, 12.0, 11.0];
        let result = calculate_rsi(&closes, 2).unwrap();
        assert!(result[0].is_none());
        assert!(result[1].is_none());
        assert!(result[2].is_some());
    }
}
