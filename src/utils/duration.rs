//! Duration parsing utilities.

use std::time::Duration;
use crate::constants::time;

/// Parses a duration string like "30s", "5m", "1h" into a Duration.
///
/// # Arguments
/// * `s` - Duration string (e.g., "30s", "5m", "1h")
///
/// # Returns
/// * `Ok(Duration)` on success
/// * `Err(&str)` with error message on failure
pub fn parse_duration(s: &str) -> Result<Duration, &'static str> {
    let s = s.trim();
    let unit = s.chars().last().ok_or("Empty duration")?;
    let value: u64 = s[..s.len()-1]
        .parse()
        .map_err(|_| "Invalid number in duration")?;

    match unit {
        's' => Ok(Duration::from_secs(value)),
        'm' => Ok(Duration::from_secs(value * time::SECONDS_PER_MINUTE)),
        'h' => Ok(Duration::from_secs(value * time::SECONDS_PER_HOUR)),
        _ => Err("Invalid duration unit")
    }
}
