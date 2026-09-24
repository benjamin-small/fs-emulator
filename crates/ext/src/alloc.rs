//! Goal-based inode and block allocation over the bitmaps (spec section 5).
//!
//! Inodes go in the goal group (the parent directory's) when one is free
//! there, else in the following groups, wrapping; group 0 is scanned from
//! `FIRST_INO - 1` because inodes 1 to 10 are reserved. Blocks come from the
//! goal group (the inode's) from its first data block, then the following
//! groups, wrapping, so one request may straddle groups. Every allocation
//! sets the bitmap bits, lowers the descriptor's and the superblock's free
//! counts, and writes those bytes in the same operation; only the primary
//! superblock and descriptor table are rewritten (backups are written at
//! format only). A request that cannot be met returns `DiskFull` before any
//! byte is written.

use crate::bitmap;
use crate::events::{BitmapKind, ExtEvent};
use crate::group::{Geometry, GroupDescriptor};
use crate::inode::Inode;
use crate::superblock::{Superblock, BLOCK_SIZE, FIRST_INO, SUPERBLOCK_OFFSET};
use fs_core::{Disk, Error, Result};
use std::collections::BTreeMap;

const BS: usize = BLOCK_SIZE as usize;

/// Everything an allocation reads and writes, borrowed from `ExtFs` for one
/// call (`ExtFs::ctx`).
pub struct AllocCtx<'a> {
    pub disk: &'a mut Disk,
    pub sb: &'a mut Superblock,
    pub gds: &'a mut Vec<GroupDescriptor>,
    pub geo: &'a Geometry,
}

/// Group indices in allocation order: `goal`, then the following groups,
/// wrapping. An out-of-range goal starts at group 0.
fn groups_from(geo: &Geometry, goal: u32) -> impl Iterator<Item = u32> {
    let groups = geo.groups;
    let goal = if goal < groups { goal } else { 0 };
    (0..groups).map(move |i| (goal + i) % groups)
}

/// Allocate one inode, preferring `goal_group`, and return its number.
/// Sets its bitmap bit, lowers the group's and the superblock's free inode
/// counts (and raises `bg_used_dirs_count` for a directory), and writes the
/// counters. `DiskFull`, with nothing written, when the superblock counts no
/// free inode or no bitmap has one.
pub fn alloc_inode(ctx: &mut AllocCtx, goal_group: u32, is_dir: bool) -> Result<u32> {
    if ctx.sb.free_inodes_count == 0 {
        return Err(Error::DiskFull);
    }
    let geo = ctx.geo;
    let per_group = geo.inodes_per_group as usize;
    let found = groups_from(geo, goal_group).find_map(|group| {
        let from = if group == 0 {
            FIRST_INO as usize - 1
        } else {
            0
        };
        let bits = ctx
            .disk
            .read(geo.group(group).inode_bitmap as usize * BS, BS);
        bitmap::find_clear(bits, from, per_group).map(|bit| (group, bit))
    });
    let Some((group, bit)) = found else {
        return Err(Error::DiskFull);
    };
    let ino = group * geo.inodes_per_group + bit as u32 + 1;
    let byte_at = geo.group(group).inode_bitmap as usize * BS + bit / 8;
    let mut byte = [ctx.disk.read(byte_at, 1)[0]];
    bitmap::set(&mut byte, bit % 8);
    ctx.disk.write(byte_at, &byte);
    let (block, offset) = geo.inode_location(ino);
    let slot = block as usize * BS + offset;
    ctx.disk.event(Box::new(ExtEvent::InodeAllocated {
        inode: ino,
        group,
        range: slot..slot + Inode::SIZE,
    }));
    ctx.disk.event(Box::new(ExtEvent::BitmapUpdated {
        group,
        which: BitmapKind::Inodes,
        range: byte_at..byte_at + 1,
    }));
    let gd = &mut ctx.gds[group as usize];
    gd.free_inodes_count = gd.free_inodes_count.saturating_sub(1);
    if is_dir {
        gd.used_dirs_count = gd.used_dirs_count.saturating_add(1);
    }
    ctx.sb.free_inodes_count -= 1;
    write_counters(ctx);
    Ok(ino)
}

/// Allocate `count` blocks, the goal group first (from its first data
/// block), then the following groups, wrapping; the result may straddle
/// groups and is in allocation order. The whole request is planned before
/// anything is written, so `DiskFull` (the superblock counts fewer free
/// blocks, or the bitmaps hold fewer) writes nothing. Emits one
/// `BlocksAllocated` per contiguous run and one `BitmapUpdated` per group.
pub fn alloc_blocks(ctx: &mut AllocCtx, goal_group: u32, count: u32) -> Result<Vec<u32>> {
    if count == 0 {
        return Ok(Vec::new());
    }
    if ctx.sb.free_blocks_count < count {
        return Err(Error::DiskFull);
    }
    let geo = ctx.geo;
    let mut plan: Vec<(u32, Vec<usize>)> = Vec::new();
    let mut remaining = count as usize;
    for group in groups_from(geo, goal_group) {
        if remaining == 0 {
            break;
        }
        let gl = geo.group(group);
        let bits = ctx.disk.read(gl.block_bitmap as usize * BS, BS);
        let upto = gl.block_count as usize;
        let mut from = (gl.first_data - gl.first_block) as usize;
        let mut taken = Vec::new();
        while taken.len() < remaining {
            match bitmap::find_clear(bits, from, upto) {
                Some(bit) => {
                    taken.push(bit);
                    from = bit + 1;
                }
                None => break,
            }
        }
        remaining -= taken.len();
        if !taken.is_empty() {
            plan.push((group, taken));
        }
    }
    if remaining > 0 {
        return Err(Error::DiskFull);
    }
    let mut out = Vec::with_capacity(count as usize);
    for (group, bits) in plan {
        let gl = geo.group(group);
        let base = gl.block_bitmap as usize * BS;
        let lo = bits[0] / 8;
        let hi = bits[bits.len() - 1] / 8;
        let mut bytes = ctx.disk.read(base + lo, hi - lo + 1).to_vec();
        for &bit in &bits {
            bitmap::set(&mut bytes, bit - lo * 8);
        }
        ctx.disk.write(base + lo, &bytes);
        let blocks: Vec<u32> = bits.iter().map(|&b| gl.first_block + b as u32).collect();
        for (start, len) in contiguous_runs(&blocks) {
            let first = blocks[start];
            ctx.disk.event(Box::new(ExtEvent::BlocksAllocated {
                first,
                count: len as u32,
                group,
                range: first as usize * BS..(first as usize + len) * BS,
            }));
        }
        ctx.disk.event(Box::new(ExtEvent::BitmapUpdated {
            group,
            which: BitmapKind::Blocks,
            range: base + lo..base + hi + 1,
        }));
        let gd = &mut ctx.gds[group as usize];
        gd.free_blocks_count = gd.free_blocks_count.saturating_sub(blocks.len() as u16);
        out.extend(blocks);
    }
    ctx.sb.free_blocks_count -= count;
    write_counters(ctx);
    Ok(out)
}

/// Write the cached counters to the primary copies: the superblock's free
/// block and inode counts (offset 12) and `s_wtime` (offset 48), and the
/// counted fields (offset 12: free blocks, free inodes, used directories)
/// of every descriptor whose bytes differ. Only bytes that change are
/// written. When any counted bytes changed, one `CountersUpdated` reports
/// the superblock and descriptors together (spec section 7), its range
/// running from the superblock's counts to the end of the last counted
/// bytes written.
pub fn write_counters(ctx: &mut AllocCtx) {
    let free_blocks = ctx.sb.free_blocks_count;
    let free_inodes = ctx.sb.free_inodes_count;
    let mut counts = [0u8; 8];
    counts[..4].copy_from_slice(&free_blocks.to_le_bytes());
    counts[4..].copy_from_slice(&free_inodes.to_le_bytes());
    let start = SUPERBLOCK_OFFSET + 12;
    let mut end = None;
    if ctx.disk.read(start, 8) != counts.as_slice() {
        ctx.disk.write(start, &counts);
        end = Some(start + 8);
    }
    let at = SUPERBLOCK_OFFSET + 48;
    let wtime = ctx.sb.wtime.to_le_bytes();
    if ctx.disk.read(at, 4) != wtime.as_slice() {
        ctx.disk.write(at, &wtime);
    }
    let table = ctx
        .geo
        .group(0)
        .descriptors_block
        .expect("group 0 holds the primary descriptor table") as usize
        * BS;
    for (i, gd) in ctx.gds.iter().enumerate() {
        let at = table + i * GroupDescriptor::SIZE + 12;
        let mut bytes = [0u8; 6];
        bytes[0..2].copy_from_slice(&gd.free_blocks_count.to_le_bytes());
        bytes[2..4].copy_from_slice(&gd.free_inodes_count.to_le_bytes());
        bytes[4..6].copy_from_slice(&gd.used_dirs_count.to_le_bytes());
        if ctx.disk.read(at, 6) != bytes.as_slice() {
            ctx.disk.write(at, &bytes);
            end = Some(at + 6);
        }
    }
    if let Some(end) = end {
        ctx.disk.event(Box::new(ExtEvent::CountersUpdated {
            free_blocks,
            free_inodes,
            range: start..end,
        }));
    }
}

/// Maximal runs of consecutive block numbers, as (index of the run's first
/// element in `blocks`, run length).
pub fn contiguous_runs(blocks: &[u32]) -> Vec<(usize, usize)> {
    let mut runs: Vec<(usize, usize)> = Vec::new();
    for (i, &block) in blocks.iter().enumerate() {
        match runs.last_mut() {
            Some((start, len)) if blocks[*start] + *len as u32 == block => *len += 1,
            _ => runs.push((i, 1)),
        }
    }
    runs
}

/// Return blocks to their groups (spec section 5): clear each block's bit,
/// raise the descriptor's and the superblock's free counts, and write the
/// counters. The blocks' bytes stay as they are (remnants). A block outside
/// the groups, or whose bit is already clear, is skipped, so the counts
/// never run ahead of the bitmap. Events: one `BitmapUpdated` per group
/// touched and one `BlocksFreed` per run of consecutive blocks.
pub fn free_blocks(ctx: &mut AllocCtx, blocks: &[u32]) {
    let geo = ctx.geo;
    let mut sorted: Vec<u32> = blocks
        .iter()
        .copied()
        .filter(|b| (geo.first_data_block..geo.total_blocks).contains(b))
        .collect();
    sorted.sort_unstable();
    sorted.dedup();
    let mut by_group: BTreeMap<u32, Vec<u32>> = BTreeMap::new();
    for &b in &sorted {
        by_group.entry(geo.group_of_block(b)).or_default().push(b);
    }
    let mut freed = Vec::new();
    for (g, members) in by_group {
        let gl = geo.group(g);
        let base = gl.block_bitmap as usize * BS;
        let mut bits = ctx.disk.read(base, BS).to_vec();
        let (mut lo, mut hi) = (usize::MAX, 0);
        for b in members {
            let bit = (b - gl.first_block) as usize;
            if !bitmap::is_set(&bits, bit) {
                continue;
            }
            bitmap::clear(&mut bits, bit);
            lo = lo.min(bit / 8);
            hi = hi.max(bit / 8 + 1);
            freed.push(b);
            ctx.gds[g as usize].free_blocks_count =
                ctx.gds[g as usize].free_blocks_count.saturating_add(1);
            ctx.sb.free_blocks_count = ctx.sb.free_blocks_count.saturating_add(1);
        }
        if lo < hi {
            ctx.disk.write(base + lo, &bits[lo..hi]);
            ctx.disk.event(Box::new(ExtEvent::BitmapUpdated {
                group: g,
                which: BitmapKind::Blocks,
                range: base + lo..base + hi,
            }));
        }
    }
    for (start, len) in contiguous_runs(&freed) {
        let first = freed[start];
        ctx.disk.event(Box::new(ExtEvent::BlocksFreed {
            first,
            count: len as u32,
            range: first as usize * BS..(first as usize + len) * BS,
        }));
    }
    if !freed.is_empty() {
        write_counters(ctx);
    }
}

/// Return an inode to its group (spec section 5): clear its bit, raise the
/// free-inode counts, lower `bg_used_dirs_count` for a directory, and write
/// the counters. The inode's own bytes are the caller's business (delete
/// leaves them as remnants). An inode whose bit is already clear is left
/// alone.
pub fn free_inode(ctx: &mut AllocCtx, ino: u32, is_dir: bool) {
    let geo = ctx.geo;
    if !(1..=geo.inodes_count).contains(&ino) {
        return;
    }
    let g = geo.group_of_inode(ino);
    let bit = ((ino - 1) % geo.inodes_per_group) as usize;
    let off = geo.group(g).inode_bitmap as usize * BS + bit / 8;
    let mut byte = [ctx.disk.read(off, 1)[0]];
    if !bitmap::is_set(&byte, bit % 8) {
        return;
    }
    bitmap::clear(&mut byte, bit % 8);
    ctx.disk.write(off, &byte);
    ctx.disk.event(Box::new(ExtEvent::BitmapUpdated {
        group: g,
        which: BitmapKind::Inodes,
        range: off..off + 1,
    }));
    let (block, within) = geo.inode_location(ino);
    let at = block as usize * BS + within;
    ctx.disk.event(Box::new(ExtEvent::InodeFreed {
        inode: ino,
        range: at..at + Inode::SIZE,
    }));
    let gd = &mut ctx.gds[g as usize];
    gd.free_inodes_count = gd.free_inodes_count.saturating_add(1);
    if is_dir {
        gd.used_dirs_count = gd.used_dirs_count.saturating_sub(1);
    }
    ctx.sb.free_inodes_count = ctx.sb.free_inodes_count.saturating_add(1);
    write_counters(ctx);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::superblock::{ExtFormatOptions, RESERVED_INODES};
    use fs_core::OpRecord;

    /// A volume's metadata and nothing else: bitmaps with the metadata
    /// blocks, the padding bits and the reserved inodes 1..=10 set, counts
    /// to match, no root directory. Inode 11 is free here.
    struct Blank {
        disk: Disk,
        sb: Superblock,
        gds: Vec<GroupDescriptor>,
        geo: Geometry,
    }

    impl Blank {
        fn new(total_blocks: u32) -> Blank {
            let options = ExtFormatOptions {
                total_blocks,
                ..Default::default()
            };
            let geo = Geometry::from_options(&options).unwrap();
            let mut disk = Disk::new(BS, u64::from(total_blocks));
            let gds: Vec<GroupDescriptor> = geo
                .groups_layout
                .iter()
                .map(|gl| GroupDescriptor::initial(gl, &geo))
                .collect();
            let mut sb = Superblock::new(&options, &geo, 0);
            sb.free_blocks_count = gds.iter().map(|g| u32::from(g.free_blocks_count)).sum();
            sb.free_inodes_count = gds.iter().map(|g| u32::from(g.free_inodes_count)).sum();
            for gl in &geo.groups_layout {
                let mut blocks = [0u8; BS];
                bitmap::set_range(&mut blocks, 0, (gl.first_data - gl.first_block) as usize);
                bitmap::set_range(&mut blocks, gl.block_count as usize, BS * 8);
                disk.write(gl.block_bitmap as usize * BS, &blocks);
                let mut inodes = [0u8; BS];
                if gl.index == 0 {
                    bitmap::set_range(&mut inodes, 0, RESERVED_INODES as usize);
                }
                bitmap::set_range(&mut inodes, geo.inodes_per_group as usize, BS * 8);
                disk.write(gl.inode_bitmap as usize * BS, &inodes);
            }
            Blank { disk, sb, gds, geo }
        }

        fn ctx(&mut self) -> AllocCtx<'_> {
            AllocCtx {
                disk: &mut self.disk,
                sb: &mut self.sb,
                gds: &mut self.gds,
                geo: &self.geo,
            }
        }

        fn bits(&self, group: u32, which: BitmapKind) -> &[u8] {
            let gl = self.geo.group(group);
            let block = match which {
                BitmapKind::Blocks => gl.block_bitmap,
                BitmapKind::Inodes => gl.inode_bitmap,
            };
            self.disk.read(block as usize * BS, BS)
        }
    }

    fn count(rec: &OpRecord, kind: &str) -> usize {
        rec.event_kinds().iter().filter(|k| **k == kind).count()
    }

    #[test]
    fn inodes_start_at_first_ino_in_group_0_and_at_bit_0_elsewhere() {
        let mut v = Blank::new(16_384);
        assert_eq!(v.geo.inodes_per_group, 512);
        v.disk.begin_op("t");
        assert_eq!(alloc_inode(&mut v.ctx(), 0, false), Ok(11));
        assert_eq!(alloc_inode(&mut v.ctx(), 0, true), Ok(12));
        assert_eq!(alloc_inode(&mut v.ctx(), 1, false), Ok(513));
        let rec = v.disk.end_op();
        assert_eq!(count(&rec, "inode_allocated"), 3);
        assert_eq!(count(&rec, "bitmap_updated"), 3);
        // One `CountersUpdated` per allocation: superblock and descriptor together.
        assert_eq!(count(&rec, "counters_updated"), 3);
        let counters = rec
            .events
            .iter()
            .filter(|e| e.kind() == "counters_updated")
            .map(|e| e.region().unwrap())
            .collect::<Vec<_>>();
        let table = v.geo.group(0).descriptors_block.unwrap() as usize * BS;
        let gd0_end = table + 12 + 6;
        let gd1_end = table + GroupDescriptor::SIZE + 12 + 6;
        // `Blank` never wrote the table, so the first write covers both
        // descriptors; the second changes only group 0's, the third group 1's.
        assert_eq!(counters[0], SUPERBLOCK_OFFSET + 12..gd1_end);
        assert_eq!(counters[1], SUPERBLOCK_OFFSET + 12..gd0_end);
        assert_eq!(counters[2], SUPERBLOCK_OFFSET + 12..gd1_end);
        assert!(bitmap::is_set(v.bits(0, BitmapKind::Inodes), 10));
        assert!(bitmap::is_set(v.bits(0, BitmapKind::Inodes), 11));
        assert!(!bitmap::is_set(v.bits(0, BitmapKind::Inodes), 12));
        assert!(bitmap::is_set(v.bits(1, BitmapKind::Inodes), 0));
        assert_eq!(
            (v.gds[0].free_inodes_count, v.gds[0].used_dirs_count),
            (500, 1)
        );
        assert_eq!(
            (v.gds[1].free_inodes_count, v.gds[1].used_dirs_count),
            (511, 0)
        );
        assert_eq!(v.sb.free_inodes_count, 1_011);
        // The primary superblock and descriptor table carry the same counts.
        let on_disk = Superblock::decode(v.disk.read(SUPERBLOCK_OFFSET, Superblock::LEN));
        assert_eq!(on_disk.free_inodes_count, 1_011);
        let gd0 = GroupDescriptor::decode(v.disk.read(table, GroupDescriptor::SIZE));
        assert_eq!((gd0.free_inodes_count, gd0.used_dirs_count), (500, 1));
        let gd1 = GroupDescriptor::decode(
            v.disk
                .read(table + GroupDescriptor::SIZE, GroupDescriptor::SIZE),
        );
        assert_eq!(gd1.free_inodes_count, 511);
    }

    #[test]
    fn a_full_goal_group_wraps_to_the_next_group_with_a_free_inode() {
        let mut v = Blank::new(16_384);
        let at = v.geo.group(1).inode_bitmap as usize * BS;
        v.disk.write(at, &[0xFF; BS]);
        v.gds[1].free_inodes_count = 0;
        v.sb.free_inodes_count -= 512;
        assert_eq!(alloc_inode(&mut v.ctx(), 1, false), Ok(11));
        assert_eq!(v.gds[0].free_inodes_count, 501);
        assert_eq!(v.gds[1].free_inodes_count, 0);
    }

    #[test]
    fn inode_exhaustion_is_disk_full_and_writes_nothing() {
        let mut v = Blank::new(64);
        assert_eq!(v.geo.inodes_per_group, 16);
        for expected in 11..=16 {
            assert_eq!(alloc_inode(&mut v.ctx(), 0, false), Ok(expected));
        }
        assert_eq!(v.sb.free_inodes_count, 0);
        let image = v.disk.as_bytes().to_vec();
        v.disk.begin_op("t");
        assert_eq!(alloc_inode(&mut v.ctx(), 0, true), Err(Error::DiskFull));
        // A counter that claims a free inode the bitmap does not have is refused too.
        v.sb.free_inodes_count = 1;
        assert_eq!(alloc_inode(&mut v.ctx(), 0, false), Err(Error::DiskFull));
        let rec = v.disk.end_op();
        assert!(rec.changes.is_empty() && rec.events.is_empty());
        assert_eq!(v.disk.as_bytes(), &image[..]);
    }

    #[test]
    fn blocks_come_from_the_goal_group_then_wrap_and_may_straddle() {
        let mut v = Blank::new(16_384);
        let g0 = v.geo.group(0).first_data;
        let g1 = v.geo.group(1).first_data;
        let last1 = v.geo.group(1).first_block + v.geo.group(1).block_count - 1;
        let free = v.sb.free_blocks_count;
        v.disk.begin_op("t");
        assert_eq!(
            alloc_blocks(&mut v.ctx(), 0, 3),
            Ok(vec![g0, g0 + 1, g0 + 2])
        );
        assert_eq!(alloc_blocks(&mut v.ctx(), 1, 2), Ok(vec![g1, g1 + 1]));
        let rest = u32::from(v.gds[1].free_blocks_count);
        let got = alloc_blocks(&mut v.ctx(), 1, rest + 2).unwrap();
        let rec = v.disk.end_op();
        assert_eq!(got.len() as u32, rest + 2);
        assert_eq!(got[0], g1 + 2);
        assert_eq!(got[rest as usize - 1], last1);
        assert_eq!(&got[rest as usize..], &[g0 + 3, g0 + 4]);
        assert_eq!(v.gds[1].free_blocks_count, 0);
        assert_eq!(v.sb.free_blocks_count, free - 3 - 2 - rest - 2);
        assert_eq!(count(&rec, "blocks_allocated"), 4);
        let bit = (g0 + 4 - v.geo.group(0).first_block) as usize;
        assert!(bitmap::is_set(v.bits(0, BitmapKind::Blocks), bit));
        assert!(!bitmap::is_set(v.bits(0, BitmapKind::Blocks), bit + 1));
    }

    #[test]
    fn a_block_request_that_cannot_be_met_is_disk_full_and_writes_nothing() {
        let mut v = Blank::new(64);
        let free = v.sb.free_blocks_count;
        assert_eq!(free, 57);
        let image = v.disk.as_bytes().to_vec();
        v.disk.begin_op("t");
        assert_eq!(
            alloc_blocks(&mut v.ctx(), 0, free + 1),
            Err(Error::DiskFull)
        );
        assert_eq!(alloc_blocks(&mut v.ctx(), 0, 0), Ok(Vec::new()));
        let rec = v.disk.end_op();
        assert!(rec.changes.is_empty() && rec.events.is_empty());
        assert_eq!(v.disk.as_bytes(), &image[..]);
        assert_eq!(
            alloc_blocks(&mut v.ctx(), 0, free).map(|b| b.len()),
            Ok(free as usize)
        );
        assert_eq!((v.sb.free_blocks_count, v.gds[0].free_blocks_count), (0, 0));
        // A counter that claims a free block the bitmap does not have is refused too.
        v.sb.free_blocks_count = 1;
        let image = v.disk.as_bytes().to_vec();
        assert_eq!(alloc_blocks(&mut v.ctx(), 0, 1), Err(Error::DiskFull));
        assert_eq!(v.disk.as_bytes(), &image[..]);
    }

    #[test]
    fn contiguous_runs_split_on_gaps() {
        assert_eq!(
            contiguous_runs(&[5, 6, 7, 9, 10, 20]),
            vec![(0, 3), (3, 2), (5, 1)]
        );
        assert!(contiguous_runs(&[]).is_empty());
    }

    use crate::blockmap::{self, MAX_FILE_BLOCKS};

    /// Allocate `n` data blocks from group 0 and map them into a new file inode.
    fn mapped(v: &mut Blank, n: u32) -> (Inode, Vec<u32>) {
        let data = alloc_blocks(&mut v.ctx(), 0, n).unwrap();
        let mut inode = Inode::new_file(0);
        inode.size = n * BLOCK_SIZE;
        blockmap::map_blocks(&mut v.ctx(), &mut inode, &data, 0).unwrap();
        (inode, data)
    }

    fn pointers(v: &Blank, block: u32) -> [u32; 256] {
        blockmap::read_pointers(&v.disk, block)
    }

    #[test]
    fn twelve_blocks_map_directly() {
        let mut v = Blank::new(16_384);
        let free = v.sb.free_blocks_count;
        let (inode, data) = mapped(&mut v, 12);
        assert_eq!(&inode.block[..12], &data[..]);
        assert_eq!(&inode.block[12..], &[0, 0, 0]);
        assert_eq!(inode.blocks, 24);
        assert_eq!(v.sb.free_blocks_count, free - 12);
        assert_eq!(blockmap::file_blocks(&v.disk, &inode), data);
        assert!(blockmap::indirect_blocks(&v.disk, &inode).is_empty());
    }

    #[test]
    fn the_thirteenth_block_needs_a_single_indirect_block() {
        let mut v = Blank::new(16_384);
        let free = v.sb.free_blocks_count;
        let (inode, data) = mapped(&mut v, 13);
        let single = inode.block[12];
        assert_ne!(single, 0);
        assert_eq!(inode.block[13], 0);
        assert_eq!(inode.blocks, (13 + 1) * 2);
        assert_eq!(v.sb.free_blocks_count, free - 14);
        let p = pointers(&v, single);
        assert_eq!(p[0], data[12]);
        assert!(p[1..].iter().all(|&x| x == 0));
        assert_eq!(
            blockmap::logical_to_physical(&v.disk, &inode, 12),
            Some(data[12])
        );
    }

    #[test]
    fn blocks_268_and_269_straddle_the_double_indirect_boundary() {
        let mut v = Blank::new(16_384);
        let (inode, data) = mapped(&mut v, 268);
        assert_eq!(inode.block[13], 0);
        assert_eq!(inode.blocks, (268 + 1) * 2);
        assert_eq!(pointers(&v, inode.block[12])[255], data[267]);
        let (inode, data) = mapped(&mut v, 269);
        let double = inode.block[13];
        assert_ne!(double, 0);
        assert_eq!(inode.blocks, (269 + 3) * 2);
        let children = pointers(&v, double);
        assert_ne!(children[0], 0);
        assert!(children[1..].iter().all(|&x| x == 0));
        assert_eq!(pointers(&v, children[0])[0], data[268]);
        assert_eq!(
            blockmap::logical_to_physical(&v.disk, &inode, 268),
            Some(data[268])
        );
        assert_eq!(blockmap::file_blocks(&v.disk, &inode), data);
        assert_eq!(blockmap::indirect_blocks(&v.disk, &inode).len(), 3);
    }

    #[test]
    fn growing_reuses_the_indirect_block_already_mapped() {
        let mut v = Blank::new(16_384);
        let (mut inode, mut data) = mapped(&mut v, 20);
        let single = inode.block[12];
        data.extend(alloc_blocks(&mut v.ctx(), 0, 10).unwrap());
        let free = v.sb.free_blocks_count;
        inode.size = 30 * BLOCK_SIZE;
        blockmap::map_blocks(&mut v.ctx(), &mut inode, &data, 0).unwrap();
        assert_eq!(inode.block[12], single);
        assert_eq!(v.sb.free_blocks_count, free);
        assert_eq!(inode.blocks, (30 + 1) * 2);
        assert_eq!(blockmap::file_blocks(&v.disk, &inode), data);
    }

    #[test]
    fn mapping_fails_before_writing_when_too_large_or_short_of_indirect_blocks() {
        let mut v = Blank::new(64);
        let mut inode = Inode::new_file(0);
        let too_many = vec![7u32; MAX_FILE_BLOCKS as usize + 1];
        assert_eq!(
            blockmap::map_blocks(&mut v.ctx(), &mut inode, &too_many, 0),
            Err(Error::FileTooLarge)
        );
        // Every free block becomes data, so the single-indirect block cannot be had.
        let data = alloc_blocks(&mut v.ctx(), 0, 57).unwrap();
        let image = v.disk.as_bytes().to_vec();
        inode.size = 57 * BLOCK_SIZE;
        let before = inode.clone();
        assert_eq!(
            blockmap::map_blocks(&mut v.ctx(), &mut inode, &data, 0),
            Err(Error::DiskFull)
        );
        assert_eq!(inode, before);
        assert_eq!(v.disk.as_bytes(), &image[..]);
    }
}
