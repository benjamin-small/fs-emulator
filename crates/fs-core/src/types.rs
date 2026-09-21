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
}
