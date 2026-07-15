use crate::error::AppResult;

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
/// Returns three vectors (lower, middle, upper), each of the same length as input.
/// Warm-up positions are `None`.
///
/// Stub — implementation in C-04.
pub fn calculate_bollinger(
    _closes: &[f64],
    _period: u32,
) -> AppResult<(IndicatorOutput, IndicatorOutput, IndicatorOutput)> {
    validate_period(_period)?;
    assert_lengths_match(&[_closes.len()])?;
    let out = empty_output(_closes.len());
    Ok((out.clone(), out.clone(), out))
}
