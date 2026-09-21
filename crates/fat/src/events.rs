//! Semantic events a FAT volume reports while it works, for the change log.

use crate::table::FatEntry;
use fs_core::Event;
use std::fmt;
use std::ops::Range;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Short,
    Lfn,
}

/// Every variant carries the absolute byte range it concerns so the UI can
/// highlight it without knowing the geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FatEvent {
    ClusterAllocated {
        cluster: u32,
        range: Range<usize>,
    },
    ClusterFreed {
        cluster: u32,
        range: Range<usize>,
    },
    FatEntrySet {
        fat: u8,
        cluster: u32,
        value: FatEntry,
        range: Range<usize>,
    },
    DirEntryWritten {
        dir: String,
        slot: usize,
        kind: EntryKind,
        range: Range<usize>,
    },
    DirEntryDeleted {
        dir: String,
        slot: usize,
        range: Range<usize>,
    },
    DataWritten {
        cluster: u32,
        bytes: usize,
        range: Range<usize>,
    },
    DirectoryGrown {
        dir: String,
        cluster: u32,
        range: Range<usize>,
    },
}

impl fmt::Display for FatEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FatEvent::ClusterAllocated { cluster, .. } => write!(f, "allocated cluster {cluster}"),
            FatEvent::ClusterFreed { cluster, .. } => write!(f, "freed cluster {cluster}"),
            FatEvent::FatEntrySet {
                fat,
                cluster,
                value,
                ..
            } => {
                write!(f, "FAT {fat}: entry for cluster {cluster} set to {value}")
            }
            FatEvent::DirEntryWritten {
                dir, slot, kind, ..
            } => {
                let kind = match kind {
                    EntryKind::Short => "short",
                    EntryKind::Lfn => "long-name",
                };
                write!(f, "wrote {kind} entry in {dir} slot {slot}")
            }
            FatEvent::DirEntryDeleted { dir, slot, .. } => {
                write!(f, "marked {dir} slot {slot} deleted")
            }
            FatEvent::DataWritten { cluster, bytes, .. } => {
                write!(f, "wrote {bytes} bytes of data to cluster {cluster}")
            }
            FatEvent::DirectoryGrown { dir, cluster, .. } => {
                write!(f, "grew directory {dir} with cluster {cluster}")
            }
        }
    }
}

impl Event for FatEvent {
    fn kind(&self) -> &'static str {
        match self {
            FatEvent::ClusterAllocated { .. } => "cluster_allocated",
            FatEvent::ClusterFreed { .. } => "cluster_freed",
            FatEvent::FatEntrySet { .. } => "fat_entry_set",
            FatEvent::DirEntryWritten { .. } => "dir_entry_written",
            FatEvent::DirEntryDeleted { .. } => "dir_entry_deleted",
            FatEvent::DataWritten { .. } => "data_written",
            FatEvent::DirectoryGrown { .. } => "directory_grown",
        }
    }

    fn region(&self) -> Option<Range<usize>> {
        let r = match self {
            FatEvent::ClusterAllocated { range, .. }
            | FatEvent::ClusterFreed { range, .. }
            | FatEvent::FatEntrySet { range, .. }
            | FatEvent::DirEntryWritten { range, .. }
            | FatEvent::DirEntryDeleted { range, .. }
            | FatEvent::DataWritten { range, .. }
            | FatEvent::DirectoryGrown { range, .. } => range,
        };
        Some(r.clone())
    }

    fn clone_box(&self) -> Box<dyn Event> {
        Box::new(self.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::table::FatEntry;
    use fs_core::Event;

    #[test]
    fn events_have_kinds_regions_and_text() {
        let e = FatEvent::FatEntrySet {
            fat: 1,
            cluster: 5,
            value: FatEntry::EndOfChain,
            range: 100..102,
        };
        assert_eq!(e.kind(), "fat_entry_set");
        assert_eq!(e.region(), Some(100..102));
        assert_eq!(
            e.to_string(),
            "FAT 1: entry for cluster 5 set to end of chain"
        );
        let e = FatEvent::DirEntryWritten {
            dir: "/DOCS".into(),
            slot: 3,
            kind: EntryKind::Lfn,
            range: 0..32,
        };
        assert_eq!(e.to_string(), "wrote long-name entry in /DOCS slot 3");
        let boxed: Box<dyn Event> = Box::new(FatEvent::ClusterAllocated {
            cluster: 2,
            range: 0..4,
        });
        assert_eq!(boxed.clone().kind(), "cluster_allocated");
    }
}
