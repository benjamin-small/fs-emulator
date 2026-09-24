//! Block groups: the geometry derived from the options or a superblock, the
//! per-group layout (backup superblock and descriptors under `sparse_super`,
//! block bitmap, inode bitmap, inode table, data), and the 32-byte group
//! descriptor codec.

use crate::superblock::{
    default_inodes_per_group, has_superblock, inodes_per_group_is_valid, ExtFormatOptions,
    Superblock, BLOCKS_PER_GROUP, BLOCK_SIZE, INODE_SIZE, MAX_BLOCKS, MIN_BLOCKS, RESERVED_INODES,
};
use fs_core::{Error, Result};

/// Where one group's structures live, in absolute block numbers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroupLayout {
    pub index: u32,
    pub first_block: u32,
    /// 8,192, or the remainder for the last group.
    pub block_count: u32,
    pub has_superblock: bool,
    pub superblock_block: Option<u32>,
    /// The first block of this group's descriptor table copy; the table is
    /// `Geometry::descriptor_blocks` long.
    pub descriptors_block: Option<u32>,
    pub block_bitmap: u32,
    pub inode_bitmap: u32,
    /// The first of `Geometry::inode_table_blocks` blocks.
    pub inode_table: u32,
    pub first_data: u32,
}

/// Every number the other layers derive from the superblock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Geometry {
    pub block_size: u32,
    pub first_data_block: u32,
    pub blocks_per_group: u32,
    pub total_blocks: u32,
    pub groups: u32,
    pub inodes_per_group: u32,
    pub inodes_count: u32,
    pub inode_table_blocks: u32,
    pub descriptor_blocks: u32,
    pub groups_layout: Vec<GroupLayout>,
}

impl Geometry {
    /// The geometry `format` lays down; `InvalidGeometry` per spec section 3.
    pub fn from_options(o: &ExtFormatOptions) -> Result<Geometry> {
        if !(MIN_BLOCKS..=MAX_BLOCKS).contains(&o.total_blocks) {
            return Err(Error::InvalidGeometry(format!(
                "total_blocks {} must be in {MIN_BLOCKS}..={MAX_BLOCKS}",
                o.total_blocks
            )));
        }
        let groups = group_count(o.total_blocks);
        let ipg = match o.inodes_per_group {
            Some(ipg) if !inodes_per_group_is_valid(ipg) => {
                return Err(Error::InvalidGeometry(format!(
                    "inodes_per_group {ipg} must be a multiple of 8 in 16..=8192"
                )))
            }
            Some(ipg) => ipg,
            None => default_inodes_per_group(o.total_blocks, groups),
        };
        Self::build(o.total_blocks, ipg).map_err(Error::InvalidGeometry)
    }

    /// The geometry a superblock describes; `CorruptImage` when the numbers
    /// cannot be laid out or disagree with each other.
    pub fn from_superblock(sb: &Superblock) -> Result<Geometry> {
        if !(MIN_BLOCKS..=MAX_BLOCKS).contains(&sb.blocks_count) {
            return Err(Error::CorruptImage(format!(
                "block count {} is outside {MIN_BLOCKS}..={MAX_BLOCKS}",
                sb.blocks_count
            )));
        }
        if !inodes_per_group_is_valid(sb.inodes_per_group) {
            return Err(Error::CorruptImage(format!(
                "{} inodes per group is not a multiple of 8 in 16..=8192",
                sb.inodes_per_group
            )));
        }
        let geo = Self::build(sb.blocks_count, sb.inodes_per_group).map_err(Error::CorruptImage)?;
        if sb.inodes_count != geo.inodes_count {
            return Err(Error::CorruptImage(format!(
                "superblock counts {} inodes but {} groups of {} make {}",
                sb.inodes_count, geo.groups, geo.inodes_per_group, geo.inodes_count
            )));
        }
        Ok(geo)
    }

    /// Lay out `total_blocks` (already in range) with `ipg` inodes per group
    /// (already valid). Fails when a group is too short for its metadata.
    fn build(total_blocks: u32, ipg: u32) -> std::result::Result<Geometry, String> {
        let groups = group_count(total_blocks);
        let inode_table_blocks = ipg * INODE_SIZE as u32 / BLOCK_SIZE;
        let descriptor_blocks = (groups * GroupDescriptor::SIZE as u32).div_ceil(BLOCK_SIZE);
        let mut groups_layout = Vec::with_capacity(groups as usize);
        for index in 0..groups {
            let first_block = 1 + index * BLOCKS_PER_GROUP;
            let block_count = (total_blocks - first_block).min(BLOCKS_PER_GROUP);
            let has_sb = has_superblock(index);
            let block_bitmap = if has_sb {
                first_block + 1 + descriptor_blocks
            } else {
                first_block
            };
            let first_data = block_bitmap + 2 + inode_table_blocks;
            if first_data - first_block > block_count {
                return Err(format!(
                    "group {index} has {block_count} blocks but its metadata needs {}",
                    first_data - first_block
                ));
            }
            groups_layout.push(GroupLayout {
                index,
                first_block,
                block_count,
                has_superblock: has_sb,
                superblock_block: has_sb.then_some(first_block),
                descriptors_block: has_sb.then_some(first_block + 1),
                block_bitmap,
                inode_bitmap: block_bitmap + 1,
                inode_table: block_bitmap + 2,
                first_data,
            });
        }
        Ok(Geometry {
            block_size: BLOCK_SIZE,
            first_data_block: 1,
            blocks_per_group: BLOCKS_PER_GROUP,
            total_blocks,
            groups,
            inodes_per_group: ipg,
            inodes_count: ipg * groups,
            inode_table_blocks,
            descriptor_blocks,
            groups_layout,
        })
    }

    /// The group a block belongs to; block 0 (the boot block) reports group 0.
    pub fn group_of_block(&self, block: u32) -> u32 {
        block.saturating_sub(self.first_data_block) / self.blocks_per_group
    }

    /// `(ino - 1) / inodes_per_group`. Panics for inode 0.
    pub fn group_of_inode(&self, ino: u32) -> u32 {
        (ino - 1) / self.inodes_per_group
    }

    /// The block holding inode `ino` and the byte offset of its 128 bytes
    /// within that block. Panics for inode 0 or past `inodes_count`.
    pub fn inode_location(&self, ino: u32) -> (u32, usize) {
        assert!(
            (1..=self.inodes_count).contains(&ino),
            "inode {ino} is outside 1..={}",
            self.inodes_count
        );
        let index = (ino - 1) % self.inodes_per_group;
        let byte = index as usize * INODE_SIZE as usize;
        let table = self.group(self.group_of_inode(ino)).inode_table;
        let block_size = self.block_size as usize;
        (table + (byte / block_size) as u32, byte % block_size)
    }

    /// Panics when `index >= groups`.
    pub fn group(&self, index: u32) -> &GroupLayout {
        &self.groups_layout[index as usize]
    }

    /// Superblock, descriptor table, both bitmaps, and the inode table, as
    /// present in the group: everything before `first_data`.
    pub fn metadata_blocks_in_group(&self, index: u32) -> u32 {
        let g = self.group(index);
        g.first_data - g.first_block
    }
}

/// `ceil((total_blocks - 1) / 8192)`: block 0 is outside every group.
fn group_count(total_blocks: u32) -> u32 {
    (total_blocks - 1).div_ceil(BLOCKS_PER_GROUP)
}

/// One 32-byte entry of the group descriptor table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GroupDescriptor {
    pub block_bitmap: u32,
    pub inode_bitmap: u32,
    pub inode_table: u32,
    pub free_blocks_count: u16,
    pub free_inodes_count: u16,
    pub used_dirs_count: u16,
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

impl GroupDescriptor {
    pub const SIZE: usize = 32;

    /// Read the first `SIZE` bytes. Panics if `bytes` is shorter.
    pub fn decode(bytes: &[u8]) -> Self {
        let b = &bytes[..Self::SIZE];
        GroupDescriptor {
            block_bitmap: u32_at(b, 0),
            inode_bitmap: u32_at(b, 4),
            inode_table: u32_at(b, 8),
            free_blocks_count: u16_at(b, 12),
            free_inodes_count: u16_at(b, 14),
            used_dirs_count: u16_at(b, 16),
        }
    }

    /// Write the first `SIZE` bytes of `out`; `bg_pad` and the reserved
    /// bytes 18..32 are zeroed. Panics if `out` is shorter.
    pub fn encode(&self, out: &mut [u8]) {
        let b = &mut out[..Self::SIZE];
        b.fill(0);
        b[0..4].copy_from_slice(&self.block_bitmap.to_le_bytes());
        b[4..8].copy_from_slice(&self.inode_bitmap.to_le_bytes());
        b[8..12].copy_from_slice(&self.inode_table.to_le_bytes());
        b[12..14].copy_from_slice(&self.free_blocks_count.to_le_bytes());
        b[14..16].copy_from_slice(&self.free_inodes_count.to_le_bytes());
        b[16..18].copy_from_slice(&self.used_dirs_count.to_le_bytes());
    }

    /// A group's descriptor before `format` creates anything: every
    /// non-metadata block free, every inode free except the reserved ones
    /// in group 0, no directories.
    pub fn initial(gl: &GroupLayout, geo: &Geometry) -> Self {
        let reserved = if gl.index == 0 { RESERVED_INODES } else { 0 };
        GroupDescriptor {
            block_bitmap: gl.block_bitmap,
            inode_bitmap: gl.inode_bitmap,
            inode_table: gl.inode_table,
            free_blocks_count: (gl.block_count - (gl.first_data - gl.first_block)) as u16,
            free_inodes_count: (geo.inodes_per_group - reserved) as u16,
            used_dirs_count: 0,
        }
    }
}

/// Decode `groups` consecutive descriptors. Panics if `bytes` is shorter
/// than `groups * 32`.
pub fn decode_table(bytes: &[u8], groups: u32) -> Vec<GroupDescriptor> {
    (0..groups as usize)
        .map(|i| GroupDescriptor::decode(&bytes[i * GroupDescriptor::SIZE..]))
        .collect()
}

/// Encode every descriptor at `i * 32`; bytes past the last one are left
/// as they are.
pub fn encode_table(gds: &[GroupDescriptor], out: &mut [u8]) {
    for (i, gd) in gds.iter().enumerate() {
        gd.encode(&mut out[i * GroupDescriptor::SIZE..]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::superblock::{ExtFormatOptions, Superblock};
    use fs_core::Error;

    fn geo(total_blocks: u32) -> Geometry {
        Geometry::from_options(&ExtFormatOptions {
            total_blocks,
            ..Default::default()
        })
        .unwrap()
    }

    #[test]
    fn default_disk_matches_spec_section_3() {
        let g = Geometry::from_options(&ExtFormatOptions::default()).unwrap();
        assert_eq!(g.block_size, 1024);
        assert_eq!(g.first_data_block, 1);
        assert_eq!(g.blocks_per_group, 8192);
        assert_eq!(g.total_blocks, 16_384);
        assert_eq!(g.groups, 2);
        assert_eq!(g.inodes_per_group, 512);
        assert_eq!(g.inodes_count, 1024);
        assert_eq!(g.inode_table_blocks, 64);
        assert_eq!(g.descriptor_blocks, 1);
        assert_eq!(
            g.group(0),
            &GroupLayout {
                index: 0,
                first_block: 1,
                block_count: 8192,
                has_superblock: true,
                superblock_block: Some(1),
                descriptors_block: Some(2),
                block_bitmap: 3,
                inode_bitmap: 4,
                inode_table: 5,
                first_data: 69,
            }
        );
        assert_eq!(
            g.group(1),
            &GroupLayout {
                index: 1,
                first_block: 8193,
                block_count: 8191,
                has_superblock: true,
                superblock_block: Some(8193),
                descriptors_block: Some(8194),
                block_bitmap: 8195,
                inode_bitmap: 8196,
                inode_table: 8197,
                first_data: 8261,
            }
        );
        // The inode table of group 1 ends at 8260: 64 blocks from 8197.
        assert_eq!(g.group(1).inode_table + g.inode_table_blocks - 1, 8260);
        assert_eq!(g.metadata_blocks_in_group(0), 68);
        assert_eq!(g.metadata_blocks_in_group(1), 68);
        let gd0 = GroupDescriptor::initial(g.group(0), &g);
        let gd1 = GroupDescriptor::initial(g.group(1), &g);
        assert_eq!(
            gd0,
            GroupDescriptor {
                block_bitmap: 3,
                inode_bitmap: 4,
                inode_table: 5,
                free_blocks_count: 8124,
                free_inodes_count: 502,
                used_dirs_count: 0,
            }
        );
        assert_eq!((gd1.free_blocks_count, gd1.free_inodes_count), (8123, 512));
        assert_eq!(g.group_of_block(0), 0);
        assert_eq!(g.group_of_block(1), 0);
        assert_eq!(g.group_of_block(8192), 0);
        assert_eq!(g.group_of_block(8193), 1);
        assert_eq!(g.group_of_block(16_383), 1);
    }

    #[test]
    fn inode_location_finds_the_table_slot() {
        let g = geo(16_384);
        assert_eq!(g.inode_location(1), (5, 0));
        assert_eq!(g.inode_location(2), (5, 128));
        assert_eq!(g.inode_location(11), (6, 256));
        assert_eq!(g.inode_location(512), (68, 896));
        assert_eq!(g.inode_location(513), (8197, 0));
        assert_eq!(g.inode_location(1024), (8260, 896));
        assert_eq!(g.group_of_inode(512), 0);
        assert_eq!(g.group_of_inode(513), 1);
    }

    #[test]
    #[should_panic(expected = "outside")]
    fn inode_location_panics_past_the_last_inode() {
        geo(16_384).inode_location(1025);
    }

    #[test]
    fn a_64_block_disk_is_one_group() {
        let g = geo(64);
        assert_eq!(g.groups, 1);
        assert_eq!(g.inodes_per_group, 16);
        assert_eq!(g.inodes_count, 16);
        assert_eq!(g.inode_table_blocks, 2);
        assert_eq!(
            g.group(0),
            &GroupLayout {
                index: 0,
                first_block: 1,
                block_count: 63,
                has_superblock: true,
                superblock_block: Some(1),
                descriptors_block: Some(2),
                block_bitmap: 3,
                inode_bitmap: 4,
                inode_table: 5,
                first_data: 7,
            }
        );
        let gd = GroupDescriptor::initial(g.group(0), &g);
        assert_eq!((gd.free_blocks_count, gd.free_inodes_count), (57, 6));
    }

    #[test]
    fn a_262144_block_disk_is_32_groups_with_sparse_backups() {
        let g = geo(262_144);
        assert_eq!(g.groups, 32);
        assert_eq!(g.inodes_per_group, 512);
        assert_eq!(g.inodes_count, 16_384);
        assert_eq!(g.descriptor_blocks, 1);
        let backups: Vec<u32> = g
            .groups_layout
            .iter()
            .filter(|gl| gl.has_superblock)
            .map(|gl| gl.index)
            .collect();
        assert_eq!(backups, vec![0, 1, 3, 5, 7, 9, 25, 27]);
        let g2 = g.group(2);
        assert_eq!((g2.first_block, g2.block_count), (16_385, 8192));
        assert_eq!((g2.superblock_block, g2.descriptors_block), (None, None));
        assert_eq!(
            (g2.block_bitmap, g2.inode_bitmap, g2.inode_table),
            (16_385, 16_386, 16_387)
        );
        assert_eq!(g2.first_data, 16_451);
        assert_eq!(g.metadata_blocks_in_group(2), 66);
        let g27 = g.group(27);
        assert_eq!(g27.superblock_block, Some(1 + 27 * 8192));
        assert_eq!(g27.descriptors_block, Some(2 + 27 * 8192));
        assert_eq!(g27.first_data, 1 + 27 * 8192 + 68);
        let last = g.group(31);
        assert_eq!((last.first_block, last.block_count), (253_953, 8191));
        assert!(!last.has_superblock);
        assert_eq!(last.first_data, 254_019);
        let gd = GroupDescriptor::initial(last, &g);
        assert_eq!((gd.free_blocks_count, gd.free_inodes_count), (8125, 512));
        assert_eq!(g.inode_location(16_384), (254_018, 896));
    }

    #[test]
    fn from_options_rejects_invalid_geometry() {
        let bad = |o: ExtFormatOptions| {
            matches!(Geometry::from_options(&o), Err(Error::InvalidGeometry(_)))
        };
        let d = ExtFormatOptions::default;
        assert!(bad(ExtFormatOptions {
            total_blocks: 63,
            ..d()
        }));
        assert!(bad(ExtFormatOptions {
            total_blocks: 262_145,
            ..d()
        }));
        assert!(bad(ExtFormatOptions {
            inodes_per_group: Some(8),
            ..d()
        }));
        assert!(bad(ExtFormatOptions {
            inodes_per_group: Some(20),
            ..d()
        }));
        assert!(bad(ExtFormatOptions {
            inodes_per_group: Some(8200),
            ..d()
        }));
        // A last group of 7 blocks cannot hold its superblock copy, the
        // descriptors, two bitmaps, and a 32-block inode table.
        assert!(bad(ExtFormatOptions {
            total_blocks: 8200,
            ..d()
        }));
        let g = Geometry::from_options(&ExtFormatOptions {
            inodes_per_group: Some(8192),
            ..d()
        })
        .unwrap();
        assert_eq!(g.inode_table_blocks, 1024);
        assert_eq!(g.group(0).first_data, 1029);
    }

    #[test]
    fn from_superblock_agrees_with_from_options_and_checks_the_numbers() {
        let o = ExtFormatOptions::default();
        let g = Geometry::from_options(&o).unwrap();
        let sb = Superblock::new(&o, &g, 0);
        assert_eq!(Geometry::from_superblock(&sb).unwrap(), g);
        let corrupt =
            |sb: Superblock| matches!(Geometry::from_superblock(&sb), Err(Error::CorruptImage(_)));
        assert!(corrupt(Superblock {
            inodes_count: 1000,
            ..sb.clone()
        }));
        assert!(corrupt(Superblock {
            blocks_count: 10,
            ..sb.clone()
        }));
        assert!(corrupt(Superblock {
            blocks_count: 8200,
            inodes_count: 1024,
            ..sb.clone()
        }));
        assert!(corrupt(Superblock {
            inodes_per_group: 7,
            ..sb
        }));
    }

    #[test]
    fn descriptor_encode_decode_round_trip() {
        let gd = GroupDescriptor {
            block_bitmap: 8195,
            inode_bitmap: 8196,
            inode_table: 8197,
            free_blocks_count: 8123,
            free_inodes_count: 512,
            used_dirs_count: 3,
        };
        let mut b = [0xAAu8; 32];
        gd.encode(&mut b);
        assert_eq!(&b[0..4], &8195u32.to_le_bytes());
        assert_eq!(&b[4..8], &8196u32.to_le_bytes());
        assert_eq!(&b[8..12], &8197u32.to_le_bytes());
        assert_eq!(&b[12..14], &8123u16.to_le_bytes());
        assert_eq!(&b[14..16], &512u16.to_le_bytes());
        assert_eq!(&b[16..18], &3u16.to_le_bytes());
        assert_eq!(&b[18..32], &[0u8; 14]);
        assert_eq!(GroupDescriptor::decode(&b), gd);
    }

    #[test]
    fn descriptor_tables_round_trip() {
        let g = geo(16_384);
        let gds: Vec<GroupDescriptor> = g
            .groups_layout
            .iter()
            .map(|gl| GroupDescriptor::initial(gl, &g))
            .collect();
        let mut block = [0u8; 1024];
        encode_table(&gds, &mut block);
        assert_eq!(&block[32..36], &8195u32.to_le_bytes());
        assert!(block[64..].iter().all(|&b| b == 0));
        assert_eq!(decode_table(&block, 2), gds);
    }
}
