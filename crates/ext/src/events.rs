//! Semantic events an ext2 volume reports while it works, for the change log.

use fs_core::Event;
use std::fmt;
use std::ops::Range;

/// Which of a group's two bitmaps changed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitmapKind {
    Blocks,
    Inodes,
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
            | ExtEvent::CountersUpdated { range, .. } => range,
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
}
