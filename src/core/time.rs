//! Simulation timestamps use integer nanoseconds.
//!
//! The prepared v1 kernel uses unsigned nanoseconds. The fight runtime uses Go's signed
//! `time.Duration` semantics, including negative cooldown starts and `NeverExpires`.

pub(crate) const SECOND: u64 = 1_000_000_000;

/// One second as a Go `time.Duration`.
pub(crate) const NS_PER_SECOND: i64 = 1_000_000_000;
pub(crate) const NS_PER_MILLISECOND: i64 = 1_000_000;
/// Go `NeverExpires`.
pub(crate) const NEVER_EXPIRES: i64 = i64::MAX;
/// Go `startingCDTime`: a timer reset to this is always ready.
pub(crate) const STARTING_CD_TIME: i64 = -60 * 60 * NS_PER_SECOND;

/// Go `Duration.Seconds`.
pub(crate) fn seconds(duration: i64) -> f64 {
    let whole = duration / NS_PER_SECOND;
    let fraction = duration % NS_PER_SECOND;
    whole as f64 + fraction as f64 / 1e9
}

/// Go `DurationFromSeconds`: a float product truncated toward zero.
pub(crate) fn from_seconds(seconds: f64) -> i64 {
    (NS_PER_SECOND as f64 * seconds) as i64
}

/// Go `Duration.Round`: halves round away from zero.
pub(crate) fn round(duration: i64, multiple: i64) -> i64 {
    if multiple <= 0 {
        return duration;
    }
    let less_than_half = |x: i64| (x as u64).wrapping_add(x as u64) < multiple as u64;
    let mut remainder = duration % multiple;
    if duration < 0 {
        remainder = -remainder;
        if less_than_half(remainder) {
            return duration + remainder;
        }
        return duration
            .checked_sub(multiple - remainder)
            .unwrap_or(i64::MIN);
    }
    if less_than_half(remainder) {
        return duration - remainder;
    }
    duration
        .checked_add(multiple - remainder)
        .unwrap_or(i64::MAX)
}

/// Go `Duration.Milliseconds`.
pub(crate) fn milliseconds(duration: i64) -> i64 {
    duration / NS_PER_MILLISECOND
}

/// Go `Duration.String`, used by debug logs.
pub(crate) fn go_string(duration: i64) -> String {
    let negative = duration < 0;
    let mut u = duration.unsigned_abs();
    if u < NS_PER_SECOND as u64 {
        let (precision, unit) = match u {
            0 => return "0s".into(),
            1..=999 => (0, "ns"),
            1_000..=999_999 => (3, "\u{b5}s"),
            _ => (6, "ms"),
        };
        let (whole, fraction) = format_fraction(u, precision);
        return format!("{}{whole}{fraction}{unit}", if negative { "-" } else { "" });
    }
    let (seconds_whole, seconds_fraction) = format_fraction(u, 9);
    u = seconds_whole.parse::<u64>().unwrap_or(0);
    let mut text = format!("{}{seconds_fraction}s", u % 60);
    u /= 60;
    if u > 0 {
        text = format!("{}m{text}", u % 60);
        u /= 60;
        if u > 0 {
            text = format!("{u}h{text}");
        }
    }
    if negative {
        text.insert(0, '-');
    }
    text
}

/// Go `fmtFrac`: split off `precision` decimal digits, dropping trailing zeros.
fn format_fraction(value: u64, precision: u32) -> (String, String) {
    let scale = 10u64.pow(precision);
    let whole = value / scale;
    let mut fraction = value % scale;
    let mut digits = String::new();
    let mut printed = false;
    for _ in 0..precision {
        let digit = fraction % 10;
        printed = printed || digit != 0;
        if printed {
            digits.insert(0, char::from(b'0' + digit as u8));
        }
        fraction /= 10;
    }
    let fraction = if printed {
        format!(".{digits}")
    } else {
        String::new()
    };
    (whole.to_string(), fraction)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_strings_match_go() {
        assert_eq!(go_string(0), "0s");
        assert_eq!(go_string(2_500_000_000), "2.5s");
        assert_eq!(go_string(1_500_000_000), "1.5s");
        assert_eq!(go_string(60 * NS_PER_SECOND), "1m0s");
        assert_eq!(go_string(714_285_714), "714.285714ms");
        assert_eq!(go_string(1_500), "1.5\u{b5}s");
        assert_eq!(go_string(12), "12ns");
        assert_eq!(go_string(-90 * NS_PER_SECOND), "-1m30s");
        assert_eq!(go_string(3_723 * NS_PER_SECOND), "1h2m3s");
    }

    #[test]
    fn rounding_and_seconds_match_go() {
        assert_eq!(round(2_499_500_000, NS_PER_MILLISECOND), 2_500_000_000);
        assert_eq!(round(2_499_499_999, NS_PER_MILLISECOND), 2_499_000_000);
        assert_eq!(round(-1_500_000, NS_PER_MILLISECOND), -2_000_000);
        assert_eq!(seconds(119_898_691_254), 119.898691254);
        assert_eq!(from_seconds(0.8), 800_000_000);
    }
}
