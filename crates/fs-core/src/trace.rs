//! What one filesystem operation did to the disk: every byte written and
//! every semantic event the filesystem chose to report.

use std::fmt;
use std::ops::Range;

/// A semantic event emitted by a filesystem during an operation, such as
/// "allocated cluster 5". Each filesystem defines its own enum implementing
/// this trait; the core never knows about clusters or inodes.
pub trait Event: fmt::Display + fmt::Debug {
    /// A stable machine-readable label, e.g. `"cluster_allocated"`.
    fn kind(&self) -> &'static str;
    /// The absolute byte range on disk this event concerns, if any.
    fn region(&self) -> Option<Range<usize>>;
    fn clone_box(&self) -> Box<dyn Event>;
}

impl Clone for Box<dyn Event> {
    fn clone(&self) -> Self {
        self.clone_box()
    }
}

/// The one event a filesystem-agnostic raw write reports: `len` bytes
/// written at absolute `offset`. Kind `raw_write`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawWrite {
    pub offset: usize,
    pub len: usize,
}

impl fmt::Display for RawWrite {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "wrote {} raw bytes at 0x{:x}", self.len, self.offset)
    }
}

impl Event for RawWrite {
    fn kind(&self) -> &'static str {
        "raw_write"
    }
    fn region(&self) -> Option<Range<usize>> {
        Some(self.offset..self.offset + self.len)
    }
    fn clone_box(&self) -> Box<dyn Event> {
        Box::new(self.clone())
    }
}

/// One contiguous write: the bytes at `offset` before and after.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ByteChange {
    pub offset: usize,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}

/// Everything one operation did, in order.
#[derive(Debug, Clone)]
pub struct OpRecord {
    /// e.g. `"create_file /DOCS/NOTES.TXT"`
    pub op: String,
    pub changes: Vec<ByteChange>,
    pub events: Vec<Box<dyn Event>>,
}

impl OpRecord {
    pub fn new(op: impl Into<String>) -> Self {
        Self {
            op: op.into(),
            changes: Vec::new(),
            events: Vec::new(),
        }
    }

    pub fn event_kinds(&self) -> Vec<&'static str> {
        self.events.iter().map(|e| e.kind()).collect()
    }

    /// Sorted, de-duplicated sector numbers touched by `changes`.
    pub fn changed_sectors(&self, sector_size: usize) -> Vec<u64> {
        let mut sectors: Vec<u64> = self
            .changes
            .iter()
            .flat_map(|c| {
                let first = c.offset / sector_size;
                let last = (c.offset + c.after.len().max(1) - 1) / sector_size;
                first..=last
            })
            .map(|s| s as u64)
            .collect();
        sectors.sort_unstable();
        sectors.dedup();
        sectors
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_write_event_has_kind_region_text_and_clones() {
        let event = RawWrite {
            offset: 512,
            len: 3,
        };
        assert_eq!(event.kind(), "raw_write");
        assert_eq!(event.region(), Some(512..515));
        assert_eq!(event.to_string(), "wrote 3 raw bytes at 0x200");
        let boxed: Box<dyn Event> = Box::new(event.clone());
        let copy = boxed.clone();
        assert_eq!(copy.kind(), "raw_write");
        assert_eq!(copy.region(), Some(512..515));
        let mut record = OpRecord::new("write_raw 0x200 +3");
        record.events.push(boxed);
        assert_eq!(record.event_kinds(), vec!["raw_write"]);
    }

    #[test]
    fn empty_raw_write_has_an_empty_region() {
        let event = RawWrite { offset: 4, len: 0 };
        assert_eq!(event.region(), Some(4..4));
        assert_eq!(event.to_string(), "wrote 0 raw bytes at 0x4");
    }
}
