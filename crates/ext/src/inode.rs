//! The 128-byte revision-1 inode: modes, times, link count, the 512-byte
//! block count, and the 15 block pointers.

/// `0100644`: a regular file, rw-r--r--.
pub const MODE_FILE: u16 = 0x81A4;
/// `040755`: a directory, rwxr-xr-x.
pub const MODE_DIR: u16 = 0x41ED;
/// `040700`: `lost+found`, rwx------.
pub const MODE_LOST_FOUND: u16 = 0x41C0;
pub const MODE_TYPE_MASK: u16 = 0xF000;
pub const MODE_TYPE_FILE: u16 = 0x8000;
pub const MODE_TYPE_DIR: u16 = 0x4000;
pub const DIRECT_BLOCKS: usize = 12;
pub const IND_BLOCK: usize = 12;
pub const DIND_BLOCK: usize = 13;
/// Never used by this crate; always 0.
pub const TIND_BLOCK: usize = 14;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Inode {
    pub mode: u16,
    pub uid: u16,
    pub size: u32,
    pub atime: u32,
    pub ctime: u32,
    pub mtime: u32,
    pub dtime: u32,
    pub gid: u16,
    pub links_count: u16,
    /// In 512-byte units, counting data and indirect blocks.
    pub blocks: u32,
    pub flags: u32,
    pub block: [u32; 15],
    pub generation: u32,
    pub file_acl: u32,
    pub dir_acl: u32,
    pub faddr: u32,
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

impl Inode {
    pub const SIZE: usize = 128;

    /// Read the first `SIZE` bytes; `i_osd1` and `osd2` are ignored.
    /// Panics if `bytes` is shorter than `SIZE`.
    pub fn decode(bytes: &[u8]) -> Inode {
        let b = &bytes[..Self::SIZE];
        let mut block = [0u32; 15];
        for (i, ptr) in block.iter_mut().enumerate() {
            *ptr = u32_at(b, 40 + i * 4);
        }
        Inode {
            mode: u16_at(b, 0),
            uid: u16_at(b, 2),
            size: u32_at(b, 4),
            atime: u32_at(b, 8),
            ctime: u32_at(b, 12),
            mtime: u32_at(b, 16),
            dtime: u32_at(b, 20),
            gid: u16_at(b, 24),
            links_count: u16_at(b, 26),
            blocks: u32_at(b, 28),
            flags: u32_at(b, 32),
            block,
            generation: u32_at(b, 100),
            file_acl: u32_at(b, 104),
            dir_acl: u32_at(b, 108),
            faddr: u32_at(b, 112),
        }
    }

    /// Write the first `SIZE` bytes of `out`; `i_osd1` (36..40) and `osd2`
    /// (116..128) are zeroed. Panics if `out` is shorter than `SIZE`.
    pub fn encode(&self, out: &mut [u8]) {
        let b = &mut out[..Self::SIZE];
        b.fill(0);
        b[0..2].copy_from_slice(&self.mode.to_le_bytes());
        b[2..4].copy_from_slice(&self.uid.to_le_bytes());
        b[4..8].copy_from_slice(&self.size.to_le_bytes());
        b[8..12].copy_from_slice(&self.atime.to_le_bytes());
        b[12..16].copy_from_slice(&self.ctime.to_le_bytes());
        b[16..20].copy_from_slice(&self.mtime.to_le_bytes());
        b[20..24].copy_from_slice(&self.dtime.to_le_bytes());
        b[24..26].copy_from_slice(&self.gid.to_le_bytes());
        b[26..28].copy_from_slice(&self.links_count.to_le_bytes());
        b[28..32].copy_from_slice(&self.blocks.to_le_bytes());
        b[32..36].copy_from_slice(&self.flags.to_le_bytes());
        for (i, ptr) in self.block.iter().enumerate() {
            b[40 + i * 4..44 + i * 4].copy_from_slice(&ptr.to_le_bytes());
        }
        b[100..104].copy_from_slice(&self.generation.to_le_bytes());
        b[104..108].copy_from_slice(&self.file_acl.to_le_bytes());
        b[108..112].copy_from_slice(&self.dir_acl.to_le_bytes());
        b[112..116].copy_from_slice(&self.faddr.to_le_bytes());
    }

    pub fn is_dir(&self) -> bool {
        self.mode & MODE_TYPE_MASK == MODE_TYPE_DIR
    }

    pub fn is_file(&self) -> bool {
        self.mode & MODE_TYPE_MASK == MODE_TYPE_FILE
    }

    /// Never used: no mode and no links. A deleted inode keeps its mode
    /// (remnants) and is told free by its bitmap bit, not by this.
    pub fn is_free(&self) -> bool {
        self.mode == 0 && self.links_count == 0
    }

    /// A new empty regular file: mode 0100644, one link, all three times `now`.
    pub fn new_file(now: u32) -> Inode {
        Inode {
            mode: MODE_FILE,
            links_count: 1,
            atime: now,
            ctime: now,
            mtime: now,
            ..Inode::default()
        }
    }

    /// A new directory with `mode` (`MODE_DIR` or `MODE_LOST_FOUND`), two
    /// links (its entry in the parent and its own `.`), no blocks yet.
    pub fn new_dir(now: u32, mode: u16) -> Inode {
        Inode {
            mode,
            links_count: 2,
            atime: now,
            ctime: now,
            mtime: now,
            ..Inode::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encode_writes_the_spec_offsets_and_decode_round_trips() {
        let mut block = [0u32; 15];
        for (i, ptr) in block.iter_mut().enumerate().take(14) {
            *ptr = 1000 + i as u32;
        }
        let inode = Inode {
            mode: MODE_FILE,
            uid: 0,
            size: 300 * 1024,
            atime: 1,
            ctime: 2,
            mtime: 3,
            dtime: 4,
            gid: 0,
            links_count: 1,
            blocks: 606,
            flags: 0,
            block,
            generation: 5,
            file_acl: 6,
            dir_acl: 7,
            faddr: 8,
        };
        let mut b = [0xAAu8; 128];
        inode.encode(&mut b);
        assert_eq!(&b[0..2], &[0xA4, 0x81]);
        assert_eq!(&b[4..8], &(300u32 * 1024).to_le_bytes());
        assert_eq!(&b[8..12], &1u32.to_le_bytes());
        assert_eq!(&b[12..16], &2u32.to_le_bytes());
        assert_eq!(&b[16..20], &3u32.to_le_bytes());
        assert_eq!(&b[20..24], &4u32.to_le_bytes());
        assert_eq!(&b[26..28], &1u16.to_le_bytes());
        assert_eq!(&b[28..32], &606u32.to_le_bytes());
        assert_eq!(&b[36..40], &[0u8; 4]);
        assert_eq!(&b[40..44], &1000u32.to_le_bytes());
        assert_eq!(&b[88..92], &1012u32.to_le_bytes());
        assert_eq!(&b[92..96], &1013u32.to_le_bytes());
        assert_eq!(&b[96..100], &[0u8; 4]);
        assert_eq!(&b[100..104], &5u32.to_le_bytes());
        assert_eq!(&b[112..116], &8u32.to_le_bytes());
        assert_eq!(&b[116..128], &[0u8; 12]);
        assert_eq!(Inode::decode(&b), inode);
    }

    #[test]
    fn new_file_and_new_dir_set_mode_links_and_times() {
        let f = Inode::new_file(315_532_800);
        assert_eq!(f.mode, 0o100644);
        assert_eq!(f.links_count, 1);
        assert_eq!(
            (f.atime, f.ctime, f.mtime, f.dtime),
            (315_532_800, 315_532_800, 315_532_800, 0)
        );
        assert_eq!((f.size, f.blocks, f.block), (0, 0, [0; 15]));
        assert!(f.is_file() && !f.is_dir() && !f.is_free());
        let d = Inode::new_dir(7, MODE_DIR);
        assert_eq!(d.mode, 0o40755);
        assert_eq!(d.links_count, 2);
        assert_eq!((d.atime, d.ctime, d.mtime), (7, 7, 7));
        assert!(d.is_dir() && !d.is_file());
        let lf = Inode::new_dir(7, MODE_LOST_FOUND);
        assert_eq!(lf.mode, 0o40700);
        assert!(lf.is_dir());
    }

    #[test]
    fn is_free_means_no_mode_and_no_links() {
        assert!(Inode::default().is_free());
        assert!(Inode::decode(&[0u8; 128]).is_free());
        let deleted = Inode {
            links_count: 0,
            dtime: 9,
            ..Inode::new_file(1)
        };
        assert!(!deleted.is_free());
        assert!(!deleted.is_dir() && deleted.is_file());
    }
}
