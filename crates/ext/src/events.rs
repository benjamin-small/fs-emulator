//! Semantic events an ext2 or ext3 volume reports while it works, for the
//! change log. The journal events follow the transaction sequence of the
//! ext3 spec (section 5.2) and the recovery steps of section 6.

use crate::journal::CrashPhase;
use fs_core::Event;
use std::fmt;
use std::ops::Range;

/// Which of a group's two bitmaps changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitmapKind {
    Blocks,
    Inodes,
}

/// What a `JournalBlockWritten` wrote into the log.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalWriteKind {
    Descriptor,
    /// The after-image of home block `home`; `escaped` when its first four
    /// bytes were the journal magic and are zeroed in the copy.
    Copy {
        home: u32,
        escaped: bool,
    },
    Commit,
}

/// Every variant carries the absolute byte range it concerns so the UI can
/// highlight it without knowing the geometry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExtEvent {
    InodeAllocated {
        inode: u32,
        group: u32,
        range: Range<usize>,
    },
    InodeFreed {
        inode: u32,
        range: Range<usize>,
    },
    BlocksAllocated {
        first: u32,
        count: u32,
        group: u32,
        range: Range<usize>,
    },
    BlocksFreed {
        first: u32,
        count: u32,
        range: Range<usize>,
    },
    InodeWritten {
        inode: u32,
        path: String,
        range: Range<usize>,
    },
    DirEntryWritten {
        dir_inode: u32,
        name: String,
        block: u32,
        range: Range<usize>,
    },
    DirEntryRemoved {
        dir_inode: u32,
        name: String,
        range: Range<usize>,
    },
    /// One per contiguous run of data blocks; `block` is the run's first.
    DataWritten {
        inode: u32,
        bytes: usize,
        block: u32,
        range: Range<usize>,
    },
    /// `level` 1 holds data pointers, 2 is the double-indirect block.
    IndirectWritten {
        inode: u32,
        level: u8,
        block: u32,
        range: Range<usize>,
    },
    BitmapUpdated {
        group: u32,
        which: BitmapKind,
        range: Range<usize>,
    },
    /// The superblock and descriptor counters together.
    CountersUpdated {
        free_blocks: u32,
        free_inodes: u32,
        range: Range<usize>,
    },
    /// `needs_recovery` set in the primary superblock (transaction step 1).
    RecoveryFlagSet {
        range: Range<usize>,
    },
    /// `needs_recovery` cleared in the primary superblock (the last step).
    RecoveryFlagCleared {
        range: Range<usize>,
    },
    /// The journal superblock names the transaction: `s_sequence = tid`,
    /// `s_start = journal_block`; `tagged` home blocks follow.
    TransactionStarted {
        tid: u32,
        journal_block: u32,
        tagged: u32,
        range: Range<usize>,
    },
    /// One block written into the log at journal index `index`, physical
    /// block `block`.
    JournalBlockWritten {
        tid: u32,
        kind: JournalWriteKind,
        index: u32,
        block: u32,
        range: Range<usize>,
    },
    /// A tagged block written home from its copy at `from_index`.
    Checkpointed {
        tid: u32,
        home: u32,
        from_index: u32,
        range: Range<usize>,
    },
    /// The journal superblock back at rest: `s_start = 0`, `s_sequence =
    /// next_sequence`.
    JournalEmptied {
        next_sequence: u32,
        range: Range<usize>,
    },
    /// An armed crash stopped the transaction; `range` is the last change's.
    Crashed {
        phase: CrashPhase,
        range: Range<usize>,
    },
    /// Recovery's scan: from journal index `start` (0: the journal is
    /// clean), expecting `sequence`, found `committed` transactions tagging
    /// `tagged` blocks.
    RecoveryScanned {
        start: u32,
        sequence: u32,
        committed: u32,
        tagged: u32,
        range: Range<usize>,
    },
    /// Recovery wrote home block `home` from its copy at `from_index`.
    Replayed {
        tid: u32,
        home: u32,
        from_index: u32,
        range: Range<usize>,
    },
    /// Recovery skipped a transaction that never reached its commit block;
    /// `range` is its descriptor block.
    TransactionDiscarded {
        tid: u32,
        tagged: u32,
        range: Range<usize>,
    },
}

fn plural(count: u32) -> &'static str {
    if count == 1 {
        ""
    } else {
        "s"
    }
}

impl fmt::Display for ExtEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExtEvent::InodeAllocated { inode, group, .. } => {
                write!(f, "allocated inode {inode} in group {group}")
            }
            ExtEvent::InodeFreed { inode, .. } => write!(f, "freed inode {inode}"),
            ExtEvent::BlocksAllocated {
                first,
                count,
                group,
                ..
            } => write!(
                f,
                "allocated {count} block{} starting at {first} in group {group}",
                plural(*count)
            ),
            ExtEvent::BlocksFreed { first, count, .. } => write!(
                f,
                "freed {count} block{} starting at {first}",
                plural(*count)
            ),
            ExtEvent::InodeWritten { inode, path, .. } => {
                write!(f, "wrote inode {inode} for {path}")
            }
            ExtEvent::DirEntryWritten {
                dir_inode,
                name,
                block,
                ..
            } => write!(
                f,
                "wrote entry {name} in directory inode {dir_inode} (block {block})"
            ),
            ExtEvent::DirEntryRemoved {
                dir_inode, name, ..
            } => write!(f, "removed entry {name} from directory inode {dir_inode}"),
            ExtEvent::DataWritten { bytes, block, .. } => {
                write!(f, "wrote {bytes} bytes of data to block {block}")
            }
            ExtEvent::IndirectWritten {
                inode,
                level,
                block,
                ..
            } => {
                let kind = if *level == 2 {
                    "double-indirect"
                } else {
                    "indirect"
                };
                write!(f, "wrote {kind} block {block} of inode {inode}")
            }
            ExtEvent::BitmapUpdated { group, which, .. } => {
                let which = match which {
                    BitmapKind::Blocks => "block",
                    BitmapKind::Inodes => "inode",
                };
                write!(f, "updated the {which} bitmap of group {group}")
            }
            ExtEvent::CountersUpdated {
                free_blocks,
                free_inodes,
                ..
            } => write!(
                f,
                "free counts now {free_blocks} blocks and {free_inodes} inodes"
            ),
            ExtEvent::RecoveryFlagSet { .. } => {
                write!(f, "set needs_recovery in the superblock")
            }
            ExtEvent::RecoveryFlagCleared { .. } => {
                write!(f, "cleared needs_recovery in the superblock")
            }
            ExtEvent::TransactionStarted {
                tid,
                journal_block,
                tagged,
                ..
            } => write!(
                f,
                "started transaction {tid} at journal block {journal_block} ({tagged} tagged block{})",
                plural(*tagged)
            ),
            ExtEvent::JournalBlockWritten {
                tid,
                kind,
                index,
                block,
                ..
            } => match kind {
                JournalWriteKind::Descriptor => write!(
                    f,
                    "wrote descriptor for transaction {tid} at journal block {index} (block {block})"
                ),
                JournalWriteKind::Copy { home, escaped } => {
                    write!(
                        f,
                        "copied block {home} into journal block {index} (block {block})"
                    )?;
                    if *escaped {
                        write!(f, " (escaped)")?;
                    }
                    Ok(())
                }
                JournalWriteKind::Commit => write!(
                    f,
                    "committed transaction {tid} at journal block {index} (block {block})"
                ),
            },
            ExtEvent::Checkpointed {
                home, from_index, ..
            } => write!(f, "checkpointed block {home} from journal block {from_index}"),
            ExtEvent::JournalEmptied { next_sequence, .. } => {
                write!(f, "journal emptied; next transaction {next_sequence}")
            }
            ExtEvent::Crashed { phase, .. } => write!(f, "crashed {phase}"),
            ExtEvent::RecoveryScanned {
                start,
                sequence,
                committed,
                tagged,
                ..
            } => {
                if *start == 0 {
                    write!(f, "journal is clean; nothing to replay")
                } else {
                    write!(
                        f,
                        "scanned the journal from block {start}, sequence {sequence}: \
                         {committed} committed transaction{}, {tagged} tagged block{}",
                        plural(*committed),
                        plural(*tagged)
                    )
                }
            }
            ExtEvent::Replayed {
                tid,
                home,
                from_index,
                ..
            } => write!(
                f,
                "replayed block {home} from journal block {from_index} (transaction {tid})"
            ),
            ExtEvent::TransactionDiscarded { tid, tagged, .. } => write!(
                f,
                "discarded uncommitted transaction {tid} ({tagged} tagged block{})",
                plural(*tagged)
            ),
        }
    }
}

impl Event for ExtEvent {
    fn kind(&self) -> &'static str {
        match self {
            ExtEvent::InodeAllocated { .. } => "inode_allocated",
            ExtEvent::InodeFreed { .. } => "inode_freed",
            ExtEvent::BlocksAllocated { .. } => "blocks_allocated",
            ExtEvent::BlocksFreed { .. } => "blocks_freed",
            ExtEvent::InodeWritten { .. } => "inode_written",
            ExtEvent::DirEntryWritten { .. } => "dir_entry_written",
            ExtEvent::DirEntryRemoved { .. } => "dir_entry_removed",
            ExtEvent::DataWritten { .. } => "data_written",
            ExtEvent::IndirectWritten { .. } => "indirect_written",
            ExtEvent::BitmapUpdated { .. } => "bitmap_updated",
            ExtEvent::CountersUpdated { .. } => "counters_updated",
            ExtEvent::RecoveryFlagSet { .. } => "recovery_flag_set",
            ExtEvent::RecoveryFlagCleared { .. } => "recovery_flag_cleared",
            ExtEvent::TransactionStarted { .. } => "transaction_started",
            ExtEvent::JournalBlockWritten { .. } => "journal_block_written",
            ExtEvent::Checkpointed { .. } => "checkpointed",
            ExtEvent::JournalEmptied { .. } => "journal_emptied",
            ExtEvent::Crashed { .. } => "crashed",
            ExtEvent::RecoveryScanned { .. } => "recovery_scanned",
            ExtEvent::Replayed { .. } => "replayed",
            ExtEvent::TransactionDiscarded { .. } => "transaction_discarded",
        }
    }

    fn region(&self) -> Option<Range<usize>> {
        let r = match self {
            ExtEvent::InodeAllocated { range, .. }
            | ExtEvent::InodeFreed { range, .. }
            | ExtEvent::BlocksAllocated { range, .. }
            | ExtEvent::BlocksFreed { range, .. }
            | ExtEvent::InodeWritten { range, .. }
            | ExtEvent::DirEntryWritten { range, .. }
            | ExtEvent::DirEntryRemoved { range, .. }
            | ExtEvent::DataWritten { range, .. }
            | ExtEvent::IndirectWritten { range, .. }
            | ExtEvent::BitmapUpdated { range, .. }
            | ExtEvent::CountersUpdated { range, .. }
            | ExtEvent::RecoveryFlagSet { range }
            | ExtEvent::RecoveryFlagCleared { range }
            | ExtEvent::TransactionStarted { range, .. }
            | ExtEvent::JournalBlockWritten { range, .. }
            | ExtEvent::Checkpointed { range, .. }
            | ExtEvent::JournalEmptied { range, .. }
            | ExtEvent::Crashed { range, .. }
            | ExtEvent::RecoveryScanned { range, .. }
            | ExtEvent::Replayed { range, .. }
            | ExtEvent::TransactionDiscarded { range, .. } => range,
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
    use fs_core::Event;

    #[test]
    fn every_event_has_its_kind_region_and_text() {
        let cases: Vec<(ExtEvent, &str, &str)> = vec![
            (
                ExtEvent::InodeAllocated {
                    inode: 12,
                    group: 0,
                    range: 4097..4098,
                },
                "inode_allocated",
                "allocated inode 12 in group 0",
            ),
            (
                ExtEvent::InodeFreed {
                    inode: 12,
                    range: 4097..4098,
                },
                "inode_freed",
                "freed inode 12",
            ),
            (
                ExtEvent::BlocksAllocated {
                    first: 69,
                    count: 3,
                    group: 0,
                    range: 3080..3081,
                },
                "blocks_allocated",
                "allocated 3 blocks starting at 69 in group 0",
            ),
            (
                ExtEvent::BlocksAllocated {
                    first: 8261,
                    count: 1,
                    group: 1,
                    range: 3080..3081,
                },
                "blocks_allocated",
                "allocated 1 block starting at 8261 in group 1",
            ),
            (
                ExtEvent::BlocksFreed {
                    first: 69,
                    count: 2,
                    range: 3080..3081,
                },
                "blocks_freed",
                "freed 2 blocks starting at 69",
            ),
            (
                ExtEvent::InodeWritten {
                    inode: 12,
                    path: "/hello.txt".into(),
                    range: 6528..6656,
                },
                "inode_written",
                "wrote inode 12 for /hello.txt",
            ),
            (
                ExtEvent::DirEntryWritten {
                    dir_inode: 2,
                    name: "hello.txt".into(),
                    block: 69,
                    range: 70_680..70_700,
                },
                "dir_entry_written",
                "wrote entry hello.txt in directory inode 2 (block 69)",
            ),
            (
                ExtEvent::DirEntryRemoved {
                    dir_inode: 2,
                    name: "hello.txt".into(),
                    range: 70_680..70_700,
                },
                "dir_entry_removed",
                "removed entry hello.txt from directory inode 2",
            ),
            (
                ExtEvent::DataWritten {
                    inode: 12,
                    bytes: 100,
                    block: 69,
                    range: 70_656..70_756,
                },
                "data_written",
                "wrote 100 bytes of data to block 69",
            ),
            (
                ExtEvent::IndirectWritten {
                    inode: 12,
                    level: 1,
                    block: 81,
                    range: 82_944..83_968,
                },
                "indirect_written",
                "wrote indirect block 81 of inode 12",
            ),
            (
                ExtEvent::IndirectWritten {
                    inode: 12,
                    level: 2,
                    block: 338,
                    range: 346_112..347_136,
                },
                "indirect_written",
                "wrote double-indirect block 338 of inode 12",
            ),
            (
                ExtEvent::BitmapUpdated {
                    group: 1,
                    which: BitmapKind::Blocks,
                    range: 8_389_632..8_389_633,
                },
                "bitmap_updated",
                "updated the block bitmap of group 1",
            ),
            (
                ExtEvent::BitmapUpdated {
                    group: 0,
                    which: BitmapKind::Inodes,
                    range: 4097..4098,
                },
                "bitmap_updated",
                "updated the inode bitmap of group 0",
            ),
            (
                ExtEvent::CountersUpdated {
                    free_blocks: 16_233,
                    free_inodes: 1012,
                    range: 1024..3072,
                },
                "counters_updated",
                "free counts now 16233 blocks and 1012 inodes",
            ),
        ];
        for (event, kind, text) in cases {
            assert_eq!(event.kind(), kind);
            assert_eq!(event.to_string(), text);
            let region = event.region().expect("every ext event has a region");
            assert!(region.start < region.end, "{event:?}");
            let boxed: Box<dyn Event> = Box::new(event.clone());
            let copy = boxed.clone();
            assert_eq!(copy.kind(), kind);
            assert_eq!(copy.region(), Some(region));
        }
        let e = ExtEvent::InodeWritten {
            inode: 2,
            path: "/".into(),
            range: 5248..5376,
        };
        assert_eq!(e.region(), Some(5248..5376));
    }

    #[test]
    fn journal_events_have_their_kind_region_and_text() {
        let copy = |home, escaped| JournalWriteKind::Copy { home, escaped };
        let cases: Vec<(ExtEvent, &str, &str)> = vec![
            (
                ExtEvent::RecoveryFlagSet {
                    range: 1120..1124,
                },
                "recovery_flag_set",
                "set needs_recovery in the superblock",
            ),
            (
                ExtEvent::RecoveryFlagCleared {
                    range: 1120..1124,
                },
                "recovery_flag_cleared",
                "cleared needs_recovery in the superblock",
            ),
            (
                ExtEvent::TransactionStarted {
                    tid: 3,
                    journal_block: 17,
                    tagged: 5,
                    range: 84_016..84_024,
                },
                "transaction_started",
                "started transaction 3 at journal block 17 (5 tagged blocks)",
            ),
            (
                ExtEvent::TransactionStarted {
                    tid: 1,
                    journal_block: 1,
                    tagged: 1,
                    range: 84_016..84_024,
                },
                "transaction_started",
                "started transaction 1 at journal block 1 (1 tagged block)",
            ),
            (
                ExtEvent::JournalBlockWritten {
                    tid: 3,
                    kind: JournalWriteKind::Descriptor,
                    index: 17,
                    block: 98,
                    range: 100_352..101_376,
                },
                "journal_block_written",
                "wrote descriptor for transaction 3 at journal block 17 (block 98)",
            ),
            (
                ExtEvent::JournalBlockWritten {
                    tid: 3,
                    kind: copy(5, false),
                    index: 18,
                    block: 99,
                    range: 101_376..102_400,
                },
                "journal_block_written",
                "copied block 5 into journal block 18 (block 99)",
            ),
            (
                ExtEvent::JournalBlockWritten {
                    tid: 3,
                    kind: copy(1111, true),
                    index: 19,
                    block: 100,
                    range: 102_400..103_424,
                },
                "journal_block_written",
                "copied block 1111 into journal block 19 (block 100) (escaped)",
            ),
            (
                ExtEvent::JournalBlockWritten {
                    tid: 3,
                    kind: JournalWriteKind::Commit,
                    index: 21,
                    block: 102,
                    range: 104_448..105_472,
                },
                "journal_block_written",
                "committed transaction 3 at journal block 21 (block 102)",
            ),
            (
                ExtEvent::Checkpointed {
                    tid: 3,
                    home: 5,
                    from_index: 18,
                    range: 5120..6144,
                },
                "checkpointed",
                "checkpointed block 5 from journal block 18",
            ),
            (
                ExtEvent::JournalEmptied {
                    next_sequence: 4,
                    range: 84_016..84_024,
                },
                "journal_emptied",
                "journal emptied; next transaction 4",
            ),
            (
                ExtEvent::Crashed {
                    phase: CrashPhase::BeforeCommit,
                    range: 101_376..102_400,
                },
                "crashed",
                "crashed before commit",
            ),
            (
                ExtEvent::Crashed {
                    phase: CrashPhase::AfterCommit,
                    range: 104_448..105_472,
                },
                "crashed",
                "crashed after commit",
            ),
            (
                ExtEvent::Crashed {
                    phase: CrashPhase::DuringCheckpoint,
                    range: 1024..2048,
                },
                "crashed",
                "crashed during checkpoint",
            ),
            (
                ExtEvent::RecoveryScanned {
                    start: 0,
                    sequence: 4,
                    committed: 0,
                    tagged: 0,
                    range: 84_016..84_024,
                },
                "recovery_scanned",
                "journal is clean; nothing to replay",
            ),
            (
                ExtEvent::RecoveryScanned {
                    start: 17,
                    sequence: 3,
                    committed: 1,
                    tagged: 5,
                    range: 84_016..84_024,
                },
                "recovery_scanned",
                "scanned the journal from block 17, sequence 3: 1 committed transaction, 5 tagged blocks",
            ),
            (
                ExtEvent::RecoveryScanned {
                    start: 1,
                    sequence: 7,
                    committed: 2,
                    tagged: 1,
                    range: 84_016..84_024,
                },
                "recovery_scanned",
                "scanned the journal from block 1, sequence 7: 2 committed transactions, 1 tagged block",
            ),
            (
                ExtEvent::Replayed {
                    tid: 3,
                    home: 5,
                    from_index: 18,
                    range: 5120..6144,
                },
                "replayed",
                "replayed block 5 from journal block 18 (transaction 3)",
            ),
            (
                ExtEvent::TransactionDiscarded {
                    tid: 3,
                    tagged: 5,
                    range: 100_352..101_376,
                },
                "transaction_discarded",
                "discarded uncommitted transaction 3 (5 tagged blocks)",
            ),
            (
                ExtEvent::TransactionDiscarded {
                    tid: 9,
                    tagged: 1,
                    range: 100_352..101_376,
                },
                "transaction_discarded",
                "discarded uncommitted transaction 9 (1 tagged block)",
            ),
        ];
        for (event, kind, text) in cases {
            assert_eq!(event.kind(), kind);
            assert_eq!(event.to_string(), text);
            let region = event.region().expect("every ext event has a region");
            assert!(region.start < region.end, "{event:?}");
            let boxed: Box<dyn Event> = Box::new(event.clone());
            let copy = boxed.clone();
            assert_eq!(copy.kind(), kind);
            assert_eq!(copy.region(), Some(region));
        }
    }
}
