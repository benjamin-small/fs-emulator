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
