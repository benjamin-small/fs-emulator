#![allow(dead_code)]
//! Helpers the ext3 integration tests share. Each test binary that says
//! `mod common;` compiles all of them, so the ones it does not use would
//! otherwise be dead code.

use ext::{ExtFormatOptions, ExtFs, JournalMode, JournalOptions};

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
