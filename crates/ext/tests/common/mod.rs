#![allow(dead_code)]
//! Helpers the ext3 integration tests share. Each test binary that says
//! `mod common;` compiles all of them, so the ones it does not use would
//! otherwise be dead code.

use ext::{BlockRole, ExtFormatOptions, ExtFs, JournalMode, JournalOptions};
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
