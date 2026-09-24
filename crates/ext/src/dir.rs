//! Directory entries with the `filetype` feature: the 8-byte header and
//! name, `rec_len` rules, scanning a block, and the two fixed blocks
//! `format` and `create_dir` write. Insertion and removal allocate, so they
//! live on `ExtFs`.

use crate::superblock::BLOCK_SIZE;
use fs_core::{Error, Result};

pub const FT_UNKNOWN: u8 = 0;
pub const FT_FILE: u8 = 1;
pub const FT_DIR: u8 = 2;
pub const MAX_NAME: usize = 255;

/// One entry as stored. `inode == 0` marks unused space.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DirEntry {
    pub inode: u32,
    pub rec_len: u16,
    pub name_len: u8,
    pub file_type: u8,
    pub name: Vec<u8>,
}

impl DirEntry {
    pub const HEADER: usize = 8;

    /// Decode the entry at the start of `bytes`, which runs to the end of
    /// its block. `CorruptImage` when fewer than 8 bytes remain or when
    /// `rec_len` is not a multiple of 4, is shorter than the header and
    /// name need, or runs past the block.
    pub fn decode(bytes: &[u8]) -> Result<DirEntry> {
        if bytes.len() < Self::HEADER {
            return Err(Error::CorruptImage(format!(
                "directory entry header needs 8 bytes but {} remain in the block",
                bytes.len()
            )));
        }
        let inode = u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let rec_len = u16::from_le_bytes([bytes[4], bytes[5]]);
        let name_len = bytes[6];
        let file_type = bytes[7];
        let needed = record_len(name_len as usize) as usize;
        if !rec_len.is_multiple_of(4)
            || (rec_len as usize) < needed
            || rec_len as usize > bytes.len()
        {
            return Err(Error::CorruptImage(format!(
                "directory entry rec_len {rec_len} is invalid (name_len {name_len} needs {needed}, {} bytes remain in the block)",
                bytes.len()
            )));
        }
        Ok(DirEntry {
            inode,
            rec_len,
            name_len,
            file_type,
            name: bytes[Self::HEADER..Self::HEADER + name_len as usize].to_vec(),
        })
    }

    /// Write the header, the name, and zero padding up to `actual_len()`.
    /// The bytes from there to `rec_len` are left as they are. Panics if
    /// `out` is shorter than `actual_len()`.
    pub fn encode(&self, out: &mut [u8]) {
        let len = self.actual_len() as usize;
        let b = &mut out[..len];
        b.fill(0);
        b[0..4].copy_from_slice(&self.inode.to_le_bytes());
        b[4..6].copy_from_slice(&self.rec_len.to_le_bytes());
        b[6] = self.name_len;
        b[7] = self.file_type;
        b[Self::HEADER..Self::HEADER + self.name.len()].copy_from_slice(&self.name);
    }

    /// The bytes this entry needs: `round_up(8 + name_len, 4)`.
    pub fn actual_len(&self) -> u16 {
        record_len(self.name_len as usize)
    }
}

/// `round_up(8 + name_len, 4)`: the smallest `rec_len` for a name.
pub fn record_len(name_len: usize) -> u16 {
    (DirEntry::HEADER + name_len).next_multiple_of(4) as u16
}

/// Every entry of one directory block as `(offset, entry)`, in order,
/// including unused (`inode == 0`) entries. `CorruptImage` when an entry's
/// `rec_len` is invalid, so the chain cannot run off the block or loop.
pub fn entries_in_block(block: &[u8]) -> Result<Vec<(usize, DirEntry)>> {
    let mut out = Vec::new();
    let mut offset = 0;
    while offset < block.len() {
        let entry = DirEntry::decode(&block[offset..])?;
        let next = offset + entry.rec_len as usize;
        out.push((offset, entry));
        offset = next;
    }
    Ok(out)
}

/// ext names: 1 to 255 bytes of UTF-8 without `/`, `\`, or NUL, and not
/// `.` or `..`; otherwise `InvalidName`.
pub fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || name.len() > MAX_NAME
        || name == "."
        || name == ".."
        || name.contains(['/', '\\', '\0'])
    {
        return Err(Error::InvalidName);
    }
    Ok(())
}

/// A directory's first block: `.` (rec_len 12) pointing at `self_ino`, then
/// `..` pointing at `parent_ino` with its `rec_len` running to the block end.
pub fn dot_entries_block(self_ino: u32, parent_ino: u32) -> [u8; 1024] {
    let mut block = [0u8; BLOCK_SIZE as usize];
    let dot = DirEntry {
        inode: self_ino,
        rec_len: record_len(1),
        name_len: 1,
        file_type: FT_DIR,
        name: b".".to_vec(),
    };
    let dotdot = DirEntry {
        inode: parent_ino,
        rec_len: BLOCK_SIZE as u16 - dot.rec_len,
        name_len: 2,
        file_type: FT_DIR,
        name: b"..".to_vec(),
    };
    dot.encode(&mut block);
    dotdot.encode(&mut block[dot.rec_len as usize..]);
    block
}

/// A block holding one unused entry (inode 0) that spans all 1,024 bytes.
pub fn empty_block() -> [u8; 1024] {
    let mut block = [0u8; BLOCK_SIZE as usize];
    DirEntry {
        inode: 0,
        rec_len: BLOCK_SIZE as u16,
        name_len: 0,
        file_type: FT_UNKNOWN,
        name: Vec::new(),
    }
    .encode(&mut block);
    block
}

#[cfg(test)]
mod tests {
    use super::*;
    use fs_core::Error;

    fn entry(inode: u32, rec_len: u16, name: &str, file_type: u8) -> DirEntry {
        DirEntry {
            inode,
            rec_len,
            name_len: name.len() as u8,
            file_type,
            name: name.as_bytes().to_vec(),
        }
    }

    #[test]
    fn record_len_rounds_header_and_name_up_to_4() {
        assert_eq!(record_len(0), 8);
        assert_eq!(record_len(1), 12);
        assert_eq!(record_len(2), 12);
        assert_eq!(record_len(4), 12);
        assert_eq!(record_len(5), 16);
        assert_eq!(record_len(9), 20);
        assert_eq!(record_len(255), 264);
        assert_eq!(entry(12, 16, "hello", FT_FILE).actual_len(), 16);
    }

    #[test]
    fn encode_decode_round_trip() {
        let e = entry(12, 1000, "hello", FT_FILE);
        let mut b = [0xAAu8; 1000];
        e.encode(&mut b);
        assert_eq!(&b[0..4], &12u32.to_le_bytes());
        assert_eq!(&b[4..6], &1000u16.to_le_bytes());
        assert_eq!((b[6], b[7]), (5, 1));
        assert_eq!(&b[8..13], b"hello");
        assert_eq!(&b[13..16], &[0, 0, 0]);
        assert_eq!(b[16], 0xAA, "bytes past actual_len are untouched");
        assert_eq!(DirEntry::decode(&b).unwrap(), e);
    }

    #[test]
    fn dot_entries_block_holds_dot_and_dotdot() {
        let block = dot_entries_block(2, 2);
        let entries = entries_in_block(&block).unwrap();
        assert_eq!(
            entries,
            vec![
                (0, entry(2, 12, ".", FT_DIR)),
                (12, entry(2, 1012, "..", FT_DIR)),
            ]
        );
        let lf = entries_in_block(&dot_entries_block(11, 2)).unwrap();
        assert_eq!((lf[0].1.inode, lf[1].1.inode), (11, 2));
        assert!(block[24..].iter().all(|&b| b == 0));
    }

    #[test]
    fn empty_block_is_one_unused_entry() {
        let block = empty_block();
        assert_eq!(&block[0..8], &[0, 0, 0, 0, 0x00, 0x04, 0, 0]);
        assert_eq!(
            entries_in_block(&block).unwrap(),
            vec![(0, entry(0, 1024, "", FT_UNKNOWN))]
        );
    }

    #[test]
    fn entries_in_block_walks_split_entries_including_unused_ones() {
        let mut block = [0u8; 1024];
        entry(2, 12, ".", FT_DIR).encode(&mut block[0..]);
        entry(2, 12, "..", FT_DIR).encode(&mut block[12..]);
        entry(0, 16, "gone!", FT_FILE).encode(&mut block[24..]);
        entry(12, 984, "hello", FT_FILE).encode(&mut block[40..]);
        let entries = entries_in_block(&block).unwrap();
        let offsets: Vec<usize> = entries.iter().map(|(o, _)| *o).collect();
        assert_eq!(offsets, vec![0, 12, 24, 40]);
        assert_eq!(entries[2].1.inode, 0);
        assert_eq!(entries[2].1.name, b"gone!");
        assert_eq!(entries[3].1.name, b"hello");
    }

    #[test]
    fn a_bad_rec_len_is_a_corrupt_image() {
        let corrupt = |rec_len: u16, name_len: u8| {
            let mut block = dot_entries_block(2, 2);
            block[12 + 4..12 + 6].copy_from_slice(&rec_len.to_le_bytes());
            block[12 + 6] = name_len;
            matches!(entries_in_block(&block), Err(Error::CorruptImage(_)))
        };
        assert!(corrupt(0, 2), "zero would loop forever");
        assert!(corrupt(1010, 2), "not a multiple of 4");
        assert!(corrupt(1016, 2), "runs past the block");
        assert!(
            corrupt(1008, 2),
            "leaves a 4-byte tail too short for a header"
        );
        assert!(corrupt(12, 200), "shorter than the name");
        assert!(!corrupt(1012, 2));
        assert!(matches!(
            DirEntry::decode(&[0u8; 7]),
            Err(Error::CorruptImage(_))
        ));
    }

    #[test]
    fn validate_name_cases() {
        assert_eq!(validate_name("hello.txt"), Ok(()));
        assert_eq!(validate_name("Hello World"), Ok(()));
        assert_eq!(validate_name("..."), Ok(()));
        assert_eq!(validate_name("caf\u{e9}"), Ok(()));
        assert_eq!(validate_name(&"a".repeat(255)), Ok(()));
        assert_eq!(validate_name(&"\u{e9}".repeat(127)), Ok(()));
        assert_eq!(validate_name(""), Err(Error::InvalidName));
        assert_eq!(validate_name(&"a".repeat(256)), Err(Error::InvalidName));
        assert_eq!(
            validate_name(&"\u{e9}".repeat(128)),
            Err(Error::InvalidName)
        );
        assert_eq!(validate_name("a/b"), Err(Error::InvalidName));
        assert_eq!(validate_name("a\\b"), Err(Error::InvalidName));
        assert_eq!(validate_name("a\0b"), Err(Error::InvalidName));
        assert_eq!(validate_name("."), Err(Error::InvalidName));
        assert_eq!(validate_name(".."), Err(Error::InvalidName));
    }
}
