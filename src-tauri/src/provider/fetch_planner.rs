/// Calculates the fetch date range needed for indicator computation.
///
/// Rules:
/// - No existing data → request a large initial window (2 years in days)
/// - Existing data → request from day after last saved date + overlap for upsert
/// - Minimum bars = max_period + cross_previous_bar + buffer
#[derive(Debug, Clone)]
pub struct FetchPlanner {
    /// Maximum period across all active conditions (used for RSI/MFI/Bollinger warm-up).
    max_period: u32,
    /// Number of extra days for cross detection (previous bar needed).
    cross_lookback: u32,
    /// Buffer days for overlap with existing data.
    overlap_buffer: u32,
    /// Default range in days when no existing data exists.
    default_range_days: u32,
}

impl FetchPlanner {
    pub fn new(max_period: u32) -> Self {
        Self {
            max_period,
            cross_lookback: 2,
            overlap_buffer: 5,
            default_range_days: 730, // ~2 years for fresh data
        }
    }

    /// Calculate the fetch range when no existing data exists for the symbol.
    pub fn plan_fresh_fetch(&self) -> FetchRange {
        let days = self.default_range_days + self.max_period + self.cross_lookback;
        FetchRange {
            days_back: days as usize,
            source: FetchSource::Fresh,
        }
    }

    /// Calculate the fetch range when existing data exists.
    /// `existing_max_date` is the latest trade_date already in the database.
    pub fn plan_incremental_fetch(&self, existing_max_date: &str) -> Option<FetchRange> {
        // Parse the existing max date and add overlap buffer + cross lookback
        let start_date = increment_date(existing_max_date, self.overlap_buffer as usize + 1);
        Some(FetchRange {
            days_back: 0, // Use start_date instead
            source: FetchSource::Incremental { start_date },
        })
    }

    /// Minimum number of bars needed for calculation including warm-up and cross.
    pub fn min_bars_needed(&self) -> usize {
        (self.max_period + self.cross_lookback) as usize
    }
}

/// The result of fetch planning.
#[derive(Debug, Clone)]
pub struct FetchRange {
    /// Days to look back from today (for fresh fetches).
    pub days_back: usize,
    /// How this range was computed.
    pub source: FetchSource,
}

/// Describes the source of the fetch range.
#[derive(Debug, Clone, PartialEq)]
pub enum FetchSource {
    /// Fresh fetch with no existing data.
    Fresh,
    /// Incremental fetch starting from a specific date.
    Incremental { start_date: String },
}

/// Increments a YYYY-MM-DD date by the given number of days.
fn increment_date(date_str: &str, days: usize) -> String {
    let parsed = chrono::NaiveDate::parse_from_str(date_str, "%Y-%m-%d").unwrap_or_else(|_| {
        // Fallback: return a date far in the past if parsing fails
        chrono::NaiveDate::from_ymd_opt(2000, 1, 1).unwrap()
    });

    let incremented = parsed
        .checked_add_days(chrono::Days::new(days as u64))
        .unwrap_or(parsed);

    incremented.format("%Y-%m-%d").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_fetch_returns_large_range() {
        let planner = FetchPlanner::new(14);
        let range = planner.plan_fresh_fetch();

        assert_eq!(range.source, FetchSource::Fresh);
        // 730 (default) + 14 (max_period) + 2 (cross_lookback) = 746
        assert_eq!(range.days_back, 746);
    }

    #[test]
    fn incremental_fetch_returns_start_date() {
        let planner = FetchPlanner::new(14);
        let range = planner
            .plan_incremental_fetch("2024-07-01")
            .expect("should return range");

        assert_eq!(
            range.source,
            FetchSource::Incremental {
                start_date: "2024-07-07".to_string() // +5 buffer + 1 = +6 days
            }
        );
    }

    #[test]
    fn min_bars_includes_period_and_cross() {
        let planner = FetchPlanner::new(20);
        assert_eq!(planner.min_bars_needed(), 22); // 20 + 2
    }

    #[test]
    fn increments_date_correctly() {
        assert_eq!(increment_date("2024-01-01", 1), "2024-01-02");
        assert_eq!(increment_date("2024-01-01", 31), "2024-02-01");
        assert_eq!(increment_date("2024-12-31", 1), "2025-01-01");
    }

    #[test]
    fn handles_leap_year() {
        assert_eq!(increment_date("2024-02-28", 1), "2024-02-29");
        assert_eq!(increment_date("2024-02-29", 1), "2024-03-01");
    }

    #[test]
    fn fresh_fetch_with_larger_period() {
        let planner = FetchPlanner::new(50);
        let range = planner.plan_fresh_fetch();

        // 730 + 50 + 2 = 782
        assert_eq!(range.days_back, 782);
    }

    #[test]
    fn incremental_with_zero_buffer() {
        let mut planner = FetchPlanner::new(14);
        planner.overlap_buffer = 0;
        let range = planner.plan_incremental_fetch("2024-07-01").unwrap();

        // +0 buffer + 1 cross = +1 day
        assert_eq!(
            range.source,
            FetchSource::Incremental {
                start_date: "2024-07-02".to_string()
            }
        );
    }

    #[test]
    fn invalid_date_fallback() {
        // Invalid date should fallback to 2000-01-01
        let result = increment_date("not-a-date", 1);
        assert_eq!(result, "2000-01-02");
    }
}
