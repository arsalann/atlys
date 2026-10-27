//! Time bounds and RFC 3339 rendering for record JSON.

/// Latest allowed timestamp: 9999-12-31T23:59:59.999Z, in Unix epoch milliseconds.
pub const MAX_MS: i64 = 253_402_300_799_999;

/// Renders Unix epoch milliseconds as RFC 3339 UTC with millisecond
/// precision, for example `2026-10-22T06:48:00.000Z`. `ms` must be in
/// `0..=MAX_MS`.
pub fn rfc3339(ms: i64) -> String {
    debug_assert!((0..=MAX_MS).contains(&ms));
    let days = ms.div_euclid(86_400_000);
    let rem = ms.rem_euclid(86_400_000);
    let (y, m, d) = civil_from_days(days);
    format!(
        "{y:04}-{m:02}-{d:02}T{:02}:{:02}:{:02}.{:03}Z",
        rem / 3_600_000,
        rem / 60_000 % 60,
        rem / 1000 % 60,
        rem % 1000
    )
}

/// Days since 1970-01-01 to a proleptic Gregorian date (H. Hinnant's algorithm).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_known_instants() {
        assert_eq!(rfc3339(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(rfc3339(951_782_400_000), "2000-02-29T00:00:00.000Z");
        assert_eq!(rfc3339(1_761_115_680_123), "2025-10-22T06:48:00.123Z");
        assert_eq!(rfc3339(4_107_542_399_999), "2100-02-28T23:59:59.999Z");
        assert_eq!(rfc3339(MAX_MS), "9999-12-31T23:59:59.999Z");
    }
}
