use crate::error::AppResult;

use super::{assert_lengths_match, empty_output, validate_period, IndicatorOutput};

/// Calculate Relative Strength Index (RSI).
///
/// Returns a vector of the same length as input.
/// Warm-up positions (first `period` elements) are `None`.
///
/// Stub — implementation in C-02.
pub fn calculate_rsi(_closes: &[f64], _period: u32) -> AppResult<IndicatorOutput> {
    validate_period(_period)?;
    assert_lengths_match(&[_closes.len()])?;
    Ok(empty_output(_closes.len()))
}
