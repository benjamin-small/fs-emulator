#![allow(dead_code)]
//! Helpers the ext3 integration tests share. Each test binary that says
//! `mod common;` compiles all of them, so the ones it does not use would
//! otherwise be dead code.

use ext::{BlockRole, ExtFormatOptions, ExtFs, Geometry, JournalMode, JournalOptions};
use fs_core::OpRecord;

/// The default 16 MiB disk formatted as ext3 in `mode`, with mke2fs's
/// journal size (1024 blocks).
pub fn ext3(mode: JournalMode) -> ExtFs {
    ExtFs::format(ExtFormatOptions {
        journal: Some(JournalOptions { blocks: None, mode }),
        ..Default::default()
    })
    .unwrap()
}

/// `len` bytes of the repeating pattern `i % 251`.
pub fn pattern(len: usize) -> Vec<u8> {
    (0..len).map(|i| (i % 251) as u8).collect()
}

/// Blank what only an ext3 volume carries, so its image compares equal to
/// an ext2 image of the same operations: every data block of `fs`'s
/// journal, and in every superblock copy `s_journal_uuid`,
/// `s_journal_inum`, `s_journal_dev` (0xD0..0xE8), `s_jnl_backup_type`
/// (0xFD), `s_default_mount_opts` (0x100), `s_jnl_blocks` (0x10C..0x150),
/// the `has_journal` bit of 0x5C, and the `needs_recovery` bit of 0x60.
pub fn mask_journal(image: &mut [u8], fs: &ExtFs) {
    for (&block, owner) in &fs.block_owners() {
        if owner.role == BlockRole::Journal {
            let at = block as usize * 1024;
            image[at..at + 1024].fill(0);
        }
    }
    for gl in &fs.geometry().groups_layout {
        let Some(block) = gl.superblock_block else {
            continue;
        };
        // Block 1 holds the primary copy at byte 1024; a backup starts its block.
        let sb = block as usize * 1024;
        image[sb + 0xD0..sb + 0xE8].fill(0);
        image[sb + 0xFD] = 0;
        image[sb + 0x100..sb + 0x104].fill(0);
        image[sb + 0x10C..sb + 0x150].fill(0);
        image[sb + 0x5C] &= !0x04;
        image[sb + 0x60] &= !0x04;
    }
}

/// `(offset, len)` of every byte change of `record`, in record order.
pub fn changes_in_order(record: &OpRecord) -> Vec<(usize, usize)> {
    record
        .changes
        .iter()
        .map(|c| (c.offset, c.after.len()))
        .collect()
}

/// The superblock fields e2fsck or a kernel mount may rewrite on its own
/// account, as `(offset, length, name)`: the mount and check bookkeeping,
/// the lifetime write counter, and the reserved tail (`s_reserved`, up to
/// `s_checksum`).
pub const SUPERBLOCK_IGNORED: [(usize, usize, &str); 7] = [
    (0x2C, 4, "s_mtime"),
    (0x30, 4, "s_wtime"),
    (0x34, 2, "s_mnt_count"),
    (0x3A, 2, "s_state"),
    (0x40, 4, "s_lastcheck"),
    (0x178, 8, "s_kbytes_written"),
    (0x284, 0x178, "s_reserved"),
];

/// `ours` and `theirs` hold the same bytes in every block, except that in
/// the superblock copies the fields of `SUPERBLOCK_IGNORED` may differ.
/// The panic names the first differing block and offset; `whose` names
/// where `theirs` came from (`"e2fsck's"`, `"the kernel's"`).
pub fn assert_images_agree(ours: &[u8], theirs: &[u8], geo: &Geometry, whose: &str) {
    assert_eq!(ours.len(), theirs.len(), "the images differ in length");
    let superblocks: Vec<usize> = geo
        .groups_layout
        .iter()
        .filter_map(|g| g.superblock_block)
        .map(|b| b as usize)
        .collect();
    for (block, (a, b)) in ours.chunks(1024).zip(theirs.chunks(1024)).enumerate() {
        let is_superblock = superblocks.contains(&block);
        let ignored = |at: usize| {
            is_superblock
                && SUPERBLOCK_IGNORED
                    .iter()
                    .any(|&(offset, len, _)| (offset..offset + len).contains(&at))
        };
        if let Some(at) = (0..a.len()).find(|&at| a[at] != b[at] && !ignored(at)) {
            panic!(
                "block {block} differs at offset 0x{at:03X}: ours 0x{:02X}, {whose} 0x{:02X}",
                a[at], b[at]
            );
        }
    }
}
