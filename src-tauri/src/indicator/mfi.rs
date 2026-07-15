use crate::error::AppResult;

use super::{assert_lengths_match, empty_output, validate_period, IndicatorOutput};

/// Calculate Money Flow Index (MFI).
///
/// Returns a vector of the same length as input.
/// Warm-up positions (first `period` elements) are `None`.
///
/// Stub — implementation in C-03.
pub fn calculate_mfi(
    _highs: &[f64],
    _lows: &[f64],
    _closes: &[f64],
    _volumes: &[u64],
    _period: u32,
) -> AppResult<IndicatorOutput> {
    validate_period(_period)?;
    assert_lengths_match(&[_highs.len(), _lows.len(), _closes.len(), _volumes.len()])?;
    Ok(empty_output(_highs.len()))
}
