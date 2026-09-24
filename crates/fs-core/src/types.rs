use std::fmt;

/// A calendar timestamp with second resolution. Filesystems pack it into
/// their own on-disk format (FAT rounds seconds down to an even number).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DateTime {
    pub year: u16,
    pub month: u8,
    pub day: u8,
    pub hour: u8,
    pub minute: u8,
    pub second: u8,
}

impl DateTime {
    pub const fn new(year: u16, month: u8, day: u8, hour: u8, minute: u8, second: u8) -> Self {
        Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        }
    }

    /// Seconds since 1970-01-01T00:00:00, reading the fields as UTC in the
    /// proleptic Gregorian calendar; negative before 1970. Pure arithmetic,
    /// no clock. The fields are not range-checked.
    pub fn to_unix_seconds(&self) -> i64 {
        let days = days_from_civil(
            i64::from(self.year),
            i64::from(self.month),
            i64::from(self.day),
        );
        days * SECONDS_PER_DAY
            + i64::from(self.hour) * 3_600
            + i64::from(self.minute) * 60
            + i64::from(self.second)
    }

    /// The UTC calendar value `secs` seconds after 1970-01-01T00:00:00
    /// (before it when negative). A `DateTime` holds years 0 to 65535, so
    /// `secs` is first clamped to 0000-01-01T00:00:00 ..= 65535-12-31T23:59:59.
    pub fn from_unix_seconds(secs: i64) -> DateTime {
        let secs = secs.clamp(MIN_UNIX_SECONDS, MAX_UNIX_SECONDS);
        let (year, month, day) = civil_from_days(secs.div_euclid(SECONDS_PER_DAY));
        let time = secs.rem_euclid(SECONDS_PER_DAY);
        DateTime::new(
            year as u16,
            month as u8,
            day as u8,
            (time / 3_600) as u8,
            (time % 3_600 / 60) as u8,
            (time % 60) as u8,
        )
    }
}

const SECONDS_PER_DAY: i64 = 86_400;
/// 0000-01-01T00:00:00, the earliest instant a `DateTime` holds.
const MIN_UNIX_SECONDS: i64 = days_from_civil(0, 1, 1) * SECONDS_PER_DAY;
/// 65535-12-31T23:59:59, the latest.
const MAX_UNIX_SECONDS: i64 =
    days_from_civil(65_535, 12, 31) * SECONDS_PER_DAY + SECONDS_PER_DAY - 1;

/// Days from 1970-01-01 to `year-month-day` in the proleptic Gregorian
/// calendar, negative before it. Howard Hinnant's `days_from_civil`: count
/// years from March, so a leap day is the last day of its year; split them
/// into 400-year eras of 146,097 days; locate the day inside its era.
/// `div_euclid` is the floor division Hinnant writes as
/// `(y >= 0 ? y : y - 399) / 400`.
const fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    // January and February end the previous March-based year.
    let y = year - (month <= 2) as i64;
    let era = y.div_euclid(400);
    // Year of era, 0..=399.
    let yoe = y - era * 400;
    // Month counted from March: March = 0 .. February = 11.
    let mp = if month > 2 { month - 3 } else { month + 9 };
    // Day of the March-based year, 0..=365.
    let doy = (153 * mp + 2) / 5 + day - 1;
    // Day of era, 0..=146_096.
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    // 719,468 days run from 0000-03-01 to 1970-01-01.
    era * 146_097 + doe - 719_468
}

/// The inverse of `days_from_civil`, Hinnant's `civil_from_days`:
/// `(year, month, day)` for a day count relative to 1970-01-01.
const fn civil_from_days(days: i64) -> (i64, i64, i64) {
    // Days since 0000-03-01.
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    // Day of era, 0..=146_096.
    let doe = z - era * 146_097;
    // Year of era, 0..=399: the corrections undo the leap days before `doe`.
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    // Day of the March-based year, 0..=365.
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    // Month counted from March: March = 0 .. February = 11.
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    // January and February belong to the next civil year.
    let year = yoe + era * 400 + (month <= 2) as i64;
    (year, month, day)
}

impl Default for DateTime {
    /// The FAT epoch, 1980-01-01 00:00:00, so tests are deterministic.
    fn default() -> Self {
        Self::new(1980, 1, 1, 0, 0, 0)
    }
}

impl fmt::Display for DateTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
            self.year, self.month, self.day, self.hour, self.minute, self.second
        )
    }
}

/// What `list_dir` and `stat` report for one entry, in filesystem-neutral terms.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EntryInfo {
    /// Display name: the long name when one exists, otherwise the short name.
    pub name: String,
    pub is_dir: bool,
    pub size: u64,
    pub created: Option<DateTime>,
    pub modified: Option<DateTime>,
    /// FAT stores only a date here; time fields are zero.
    pub accessed: Option<DateTime>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_datetime_is_fat_epoch() {
        assert_eq!(DateTime::default(), DateTime::new(1980, 1, 1, 0, 0, 0));
    }

    #[test]
    fn datetime_display() {
        assert_eq!(
            DateTime::new(2026, 9, 21, 7, 5, 9).to_string(),
            "2026-09-21 07:05:09"
        );
    }

    /// Known instants: (calendar value, seconds since 1970-01-01T00:00:00 UTC).
    const KNOWN: [(DateTime, i64); 5] = [
        (DateTime::new(1970, 1, 1, 0, 0, 0), 0),
        (DateTime::new(1980, 1, 1, 0, 0, 0), 315_532_800),
        (DateTime::new(2000, 2, 29, 12, 0, 0), 951_825_600),
        (DateTime::new(2038, 1, 19, 3, 14, 7), 2_147_483_647),
        (DateTime::new(1969, 12, 31, 23, 59, 59), -1),
    ];

    #[test]
    fn to_unix_seconds_matches_known_instants() {
        for (dt, secs) in KNOWN {
            assert_eq!(dt.to_unix_seconds(), secs, "{dt}");
        }
    }

    #[test]
    fn from_unix_seconds_matches_known_instants() {
        for (dt, secs) in KNOWN {
            assert_eq!(DateTime::from_unix_seconds(secs), dt, "{secs}");
        }
    }

    #[test]
    fn unix_seconds_round_trip_over_a_table_of_dates() {
        let dates = [
            DateTime::new(0, 1, 1, 0, 0, 0),
            DateTime::new(1, 1, 1, 0, 0, 0),
            DateTime::new(1600, 2, 29, 23, 59, 59),
            DateTime::new(1700, 3, 1, 0, 0, 0),
            DateTime::new(1899, 12, 31, 23, 59, 59),
            DateTime::new(1900, 2, 28, 12, 30, 0),
            DateTime::new(1900, 3, 1, 0, 0, 0),
            DateTime::new(1969, 1, 1, 0, 0, 0),
            DateTime::new(1970, 1, 2, 0, 0, 0),
            DateTime::new(1972, 2, 29, 6, 7, 8),
            DateTime::new(1999, 12, 31, 23, 59, 59),
            DateTime::new(2000, 1, 1, 0, 0, 0),
            DateTime::new(2000, 3, 1, 0, 0, 0),
            DateTime::new(2024, 2, 29, 18, 45, 1),
            DateTime::new(2026, 9, 23, 10, 11, 12),
            DateTime::new(2100, 2, 28, 23, 59, 59),
            DateTime::new(2100, 3, 1, 0, 0, 0),
            DateTime::new(2106, 2, 7, 6, 28, 15),
            DateTime::new(2400, 2, 29, 0, 0, 0),
            DateTime::new(9999, 12, 31, 23, 59, 59),
            DateTime::new(65535, 12, 31, 23, 59, 59),
        ];
        for dt in dates {
            assert_eq!(
                DateTime::from_unix_seconds(dt.to_unix_seconds()),
                dt,
                "{dt}"
            );
        }
        for secs in [
            -86_401,
            -86_400,
            -1,
            0,
            1,
            59,
            86_399,
            86_400,
            1_000_000_000,
        ] {
            assert_eq!(DateTime::from_unix_seconds(secs).to_unix_seconds(), secs);
        }
    }

    #[test]
    fn u32_max_seconds_is_early_2106() {
        assert_eq!(
            DateTime::from_unix_seconds(i64::from(u32::MAX)),
            DateTime::new(2106, 2, 7, 6, 28, 15)
        );
    }

    #[test]
    fn from_unix_seconds_clamps_to_the_representable_years() {
        assert_eq!(
            DateTime::from_unix_seconds(i64::MIN),
            DateTime::new(0, 1, 1, 0, 0, 0)
        );
        assert_eq!(
            DateTime::from_unix_seconds(i64::MAX),
            DateTime::new(65535, 12, 31, 23, 59, 59)
        );
    }
}
