//! The journal as a mounted volume sees it: `JournalState` (the resolved
//! block map and the session's sequence, head, mode, and flags), opening it
//! from an image with the checks of spec section 4, the format-time writer
//! of spec sections 2 and 3, and the `JournalInfo` summary.

use super::{
    default_journal_blocks, wrap, CrashPhase, JournalMode, JournalOptions, JournalSuperblock,
    FEATURE_COMPAT_HAS_JOURNAL, FEATURE_INCOMPAT_RECOVER, JOURNAL_INO, MIN_JOURNAL_BLOCKS,
};
use crate::alloc::{self, AllocCtx};
use crate::blockmap;
use crate::fs::{stamp, ExtFs};
use crate::group::Geometry;
use crate::inode::Inode;
use crate::superblock::{Superblock, BLOCK_SIZE};
use fs_core::{Disk, Error, Result};

const BS: usize = BLOCK_SIZE as usize;
/// `0100600`: a regular file, rw-------, as mke2fs makes the journal inode.
const MODE_JOURNAL: u16 = 0x8180;
/// `s_jnl_backup_type` when `s_jnl_blocks` holds the journal inode's
/// `i_block` (e2fsprogs' `EXT3_JNL_BACKUP_BLOCKS`).
const JNL_BACKUP_BLOCKS: u8 = 1;

/// The journal of a mounted ext3 volume.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalState {
    /// The physical block of journal index `i`; `len() == maxlen`.
    pub blocks: Vec<u32>,
    pub maxlen: u32,
    /// The first log index (`s_first`, 1).
    pub first: u32,
    /// The tid the next transaction takes.
    pub sequence: u32,
    /// The journal index the next transaction starts at.
    pub head: u32,
    pub mode: JournalMode,
    /// The ext superblock's `needs_recovery` flag.
    pub needs_recovery: bool,
    pub armed: Option<CrashPhase>,
}

impl JournalState {
    /// Open the journal of a volume whose superblock sets `has_journal`,
    /// checking spec section 4 items 1 to 6 in order and returning the
    /// first failure. Never replays: `sequence` is `s_sequence`, `head` is
    /// `s_first`, nothing is armed.
    pub fn open(disk: &Disk, sb: &Superblock, geo: &Geometry) -> Result<JournalState> {
        // 1. An internal journal on inode 8.
        if sb.journal_inum != JOURNAL_INO {
            return Err(Error::Unsupported(format!(
                "journal inode {} (only the reserved inode 8 is supported)",
                sb.journal_inum
            )));
        }
        if sb.journal_uuid != [0; 16] || sb.journal_dev != 0 {
            return Err(Error::Unsupported("external journal".into()));
        }
        // 2. Inode 8: a regular file of whole blocks, mapped inside the volume.
        let (block, offset) = geo.inode_location(JOURNAL_INO);
        let inode = Inode::decode(disk.read(block as usize * BS + offset, Inode::SIZE));
        if !inode.is_file() {
            return Err(Error::CorruptImage(format!(
                "journal inode 8 is not a regular file (mode 0x{:04X})",
                inode.mode
            )));
        }
        if !inode.size.is_multiple_of(BLOCK_SIZE) {
            return Err(Error::CorruptImage(format!(
                "journal inode 8 has size {}, not a whole number of blocks",
                inode.size
            )));
        }
        // `file_blocks` stops at the first pointer that is zero or past the
        // disk, so a short list is a hole or a block outside the volume.
        let want = inode.size / BLOCK_SIZE;
        let mut blocks = blockmap::file_blocks(disk, &inode);
        let inside = blocks.iter().take_while(|&&b| b < geo.total_blocks).count() as u32;
        if inside < want {
            return Err(Error::CorruptImage(format!(
                "journal inode 8 maps only {inside} of its {want} blocks inside the volume"
            )));
        }
        if want < MIN_JOURNAL_BLOCKS {
            return Err(Error::Unsupported(format!(
                "journal of {want} blocks is below the 1024-block minimum"
            )));
        }
        blocks.truncate(want as usize);
        // 3 and 4. The journal superblock: magic, version, geometry, features.
        let jsb = JournalSuperblock::decode(disk.read(blocks[0] as usize * BS, BS))?;
        JournalSuperblock {
            start: 0,
            ..jsb.clone()
        }
        .validate(want)?;
        // 5. The journal mode bits of the ext superblock.
        let mode = JournalMode::from_mount_opts(sb.default_mount_opts)?;
        // 6. `s_start` (the only check `validate` has left).
        jsb.validate(want)?;
        Ok(JournalState {
            blocks,
            maxlen: want,
            first: jsb.first,
            sequence: jsb.sequence,
            head: jsb.first,
            mode,
            needs_recovery: sb.feature_incompat & FEATURE_INCOMPAT_RECOVER != 0,
            armed: None,
        })
    }

    /// The physical block of journal index `index`. Panics past `maxlen`.
    pub fn physical(&self, index: u32) -> u32 {
        self.blocks[index as usize]
    }

    /// The byte offset of journal index `index` on the disk.
    pub fn offset(&self, index: u32) -> usize {
        self.physical(index) as usize * BS
    }

    /// The most blocks one transaction may tag: jbd2's
    /// `j_max_transaction_buffers`, a quarter of the journal.
    pub fn max_transaction(&self) -> u32 {
        self.maxlen / 4
    }

    /// The journal superblock as the disk holds it now.
    pub fn superblock(&self, disk: &Disk) -> Result<JournalSuperblock> {
        JournalSuperblock::decode(disk.read(self.offset(0), BS))
    }

    /// The ring rule for this journal: `journal::wrap(index, first, maxlen)`.
    pub fn wrap(&self, index: u32) -> u32 {
        wrap(index, self.first, self.maxlen)
    }
}

/// The journal size for `options` on a volume of `total_blocks` whose
/// freshly formatted ext2 layout leaves `free_blocks` free (spec section
/// 3): mke2fs's table, or the explicit size within mke2fs's two bounds.
fn journal_size(total_blocks: u32, free_blocks: u32, options: &JournalOptions) -> Result<u32> {
    let too_big = || Error::InvalidGeometry("journal size too big for the volume".into());
    let Some(default) = default_journal_blocks(total_blocks) else {
        return Err(Error::InvalidGeometry(
            "a journal needs at least 2048 blocks".into(),
        ));
    };
    let blocks = match options.blocks {
        None => default,
        Some(blocks) if blocks < MIN_JOURNAL_BLOCKS => {
            return Err(Error::InvalidGeometry(
                "journal size must be at least 1024 blocks".into(),
            ))
        }
        Some(blocks) if blocks > free_blocks / 2 || blocks > blockmap::MAX_FILE_BLOCKS => {
            return Err(too_big())
        }
        Some(blocks) => blocks,
    };
    // A default-sized journal on a volume whose inode tables leave too
    // little room: the data and pointer blocks must fit.
    if blocks + blockmap::indirect_blocks_needed(blocks) > free_blocks {
        return Err(too_big());
    }
    Ok(blocks)
}

/// Lay the journal down during format (spec sections 2 and 3), after the
/// root and `lost+found` exist: check the size, fill inode 8's slot (inode
/// 8 is already counted used, as inodes 1..=11 are at format), allocate its
/// data blocks from the group of block `(total_blocks - first_data_block) /
/// 2` and then its pointer blocks after them (`blockmap::map_blocks`, the
/// slice-2 rule), and write the journal superblock to journal index 0.
/// Indexes 1 and up are fresh allocations of a zero-filled disk, so they
/// are already zero. The cached superblock gains the section-2 fields and
/// the new counters; `format` then writes every superblock copy.
pub(crate) fn write_journal(fs: &mut ExtFs, options: &JournalOptions) -> Result<JournalState> {
    let maxlen = journal_size(fs.geo.total_blocks, fs.sb.free_blocks_count, options)?;
    let goal = fs
        .geo
        .group_of_block((fs.geo.total_blocks - fs.geo.first_data_block) / 2);
    let secs = stamp(fs.now);
    let mut inode = Inode {
        mode: MODE_JOURNAL,
        links_count: 1,
        size: maxlen * BLOCK_SIZE,
        atime: secs,
        ctime: secs,
        mtime: secs,
        ..Inode::default()
    };
    let blocks = {
        let mut ctx = AllocCtx {
            disk: &mut fs.disk,
            sb: &mut fs.sb,
            gds: &mut fs.gds,
            geo: &fs.geo,
        };
        let blocks = alloc::alloc_blocks(&mut ctx, goal, maxlen)?;
        blockmap::map_blocks(&mut ctx, &mut inode, &blocks, goal)?;
        blocks
    };
    let (block, offset) = fs.geo.inode_location(JOURNAL_INO);
    let mut slot = [0u8; Inode::SIZE];
    inode.encode(&mut slot);
    fs.disk.write(block as usize * BS + offset, &slot);

    let mut jsb = [0u8; JournalSuperblock::LEN];
    JournalSuperblock::new(maxlen, fs.sb.uuid).encode(&mut jsb);
    fs.disk.write(blocks[0] as usize * BS, &jsb);

    let sb = &mut fs.sb;
    sb.feature_compat |= FEATURE_COMPAT_HAS_JOURNAL;
    sb.journal_inum = JOURNAL_INO;
    sb.default_mount_opts = options.mode.mount_opts();
    sb.jnl_backup_type = JNL_BACKUP_BLOCKS;
    sb.jnl_blocks[..15].copy_from_slice(&inode.block);
    sb.jnl_blocks[15] = 0;
    sb.jnl_blocks[16] = inode.size;
    Ok(JournalState {
        blocks,
        maxlen,
        first: 1,
        sequence: 1,
        head: 1,
        mode: options.mode,
        needs_recovery: false,
        armed: None,
    })
}

/// What `ExtFs::journal_info` reports (spec section 7).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalInfo {
    /// Always 8.
    pub inode: u32,
    pub maxlen: u32,
    /// The physical block of journal index 0.
    pub first_block: u32,
    /// The tid the next transaction takes.
    pub sequence: u32,
    /// `s_start` as the disk holds it: 0 when the journal is empty.
    pub start: u32,
    pub head: u32,
    pub mode: JournalMode,
    pub needs_recovery: bool,
    /// `maxlen / 4`.
    pub max_transaction: u32,
}
