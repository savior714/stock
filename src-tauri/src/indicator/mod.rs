pub mod bollinger;
pub mod mfi;
pub mod rsi;

use crate::error::{AppError, AppResult};

/// Indicator output: same length as input, warm-up positions are `None`.
pub type IndicatorOutput = Vec<Option<f64>>;

/// Validate that `period` is at least 1.
pub fn validate_period(period: u32) -> AppResult<()> {
    if period < 1 {
        return Err(AppError::validation("indicator period must be at least 1"));
    }
    Ok(())
}

/// Verify that all input slices have the same length.
/// Accepts mixed types by comparing usize lengths directly.
pub fn assert_lengths_match(lengths: &[usize]) -> AppResult<()> {
    if lengths.is_empty() {
        return Err(AppError::validation("at least one input slice is required"));
    }
    let expected_len = lengths[0];
    if expected_len == 0 {
        return Err(AppError::validation("input must not be empty"));
    }
    for (i, &len) in lengths.iter().enumerate().skip(1) {
        if len != expected_len {
            return Err(AppError::validation(format!(
                "input slice {} length {} does not match first slice length {}",
                i, len, expected_len
            )));
        }
    }
    Ok(())
}

/// Create an output vector of the same length as input, all `None`.
pub fn empty_output(len: usize) -> IndicatorOutput {
    vec![None; len]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_period_zero() {
        assert!(validate_period(0).is_err());
    }

    #[test]
    fn accepts_period_one() {
        assert!(validate_period(1).is_ok());
    }

    #[test]
    fn accepts_period_greater_than_one() {
        assert!(validate_period(14).is_ok());
    }

    #[test]
    fn rejects_empty_lengths() {
        assert!(assert_lengths_match(&[]).is_err());
    }

    #[test]
    fn rejects_zero_length() {
        assert!(assert_lengths_match(&[0]).is_err());
    }

    #[test]
    fn accepts_equal_lengths() {
        assert!(assert_lengths_match(&[3, 3, 3, 3]).is_ok());
    }

    #[test]
    fn rejects_mismatched_lengths() {
        assert!(assert_lengths_match(&[2, 3]).is_err());
    }

    #[test]
    fn empty_output_matches_input_length() {
        let output = empty_output(5);
        assert_eq!(output.len(), 5);
        assert!(output.iter().all(|v| v.is_none()));
    }
}
