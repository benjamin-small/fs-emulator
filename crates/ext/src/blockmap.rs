//! A file's logical-to-physical block mapping through the 12 direct
//! pointers, the single-indirect block, and the double-indirect block. This
//! is the read side; allocation and freeing join it in later tasks.

use crate::inode::{Inode, DIND_BLOCK, DIRECT_BLOCKS, IND_BLOCK};
use crate::superblock::BLOCK_SIZE;
use fs_core::Disk;

/// Block pointers in one 1 KiB indirect block.
pub const PTRS_PER_BLOCK: u32 = 256;
/// Direct + single-indirect + double-indirect: 65,804 blocks.
pub const MAX_FILE_BLOCKS: u32 = 12 + 256 + 65_536;

/// First logical block mapped through the single-indirect block.
const IND_FIRST: u32 = DIRECT_BLOCKS as u32;
/// First logical block mapped through the double-indirect block.
const DIND_FIRST: u32 = IND_FIRST + PTRS_PER_BLOCK;

/// Data blocks for `len` bytes: `ceil(len / 1024)`, saturating at `u32::MAX`.
pub fn blocks_needed(len: u64) -> u32 {
    u32::try_from(len.div_ceil(BLOCK_SIZE as u64)).unwrap_or(u32::MAX)
}

/// Indirect blocks a file of `data_blocks` blocks needs (for counts up to
/// `MAX_FILE_BLOCKS`): none for 12 or fewer, the single-indirect block up to
/// 268, then the double-indirect block and one second-level block per 256.
pub fn indirect_blocks_needed(data_blocks: u32) -> u32 {
    if data_blocks <= IND_FIRST {
        0
    } else if data_blocks <= DIND_FIRST {
        1
    } else {
        2 + (data_blocks - DIND_FIRST).div_ceil(PTRS_PER_BLOCK)
    }
}

fn disk_blocks(disk: &Disk) -> u64 {
    disk.len() as u64 / BLOCK_SIZE as u64
}

/// A pointer this crate can follow: non-zero and inside the disk.
fn followable(disk: &Disk, block: u32) -> bool {
    block != 0 && (block as u64) < disk_blocks(disk)
}

/// The 256 little-endian pointers of an indirect block. Block 0 and blocks
/// past the end of the disk read as all zeros (unmapped), so a corrupt
/// pointer never panics.
pub fn read_pointers(disk: &Disk, block: u32) -> [u32; 256] {
    let mut ptrs = [0u32; 256];
    if !followable(disk, block) {
        return ptrs;
    }
    let bytes = disk.read(block as usize * BLOCK_SIZE as usize, BLOCK_SIZE as usize);
    for (i, ptr) in ptrs.iter_mut().enumerate() {
        *ptr = u32::from_le_bytes([
            bytes[i * 4],
            bytes[i * 4 + 1],
            bytes[i * 4 + 2],
            bytes[i * 4 + 3],
        ]);
    }
    ptrs
}

/// The physical block holding logical block `logical` of `inode`, or `None`
/// when it is unmapped (a zero pointer), past `MAX_FILE_BLOCKS`, or reached
/// through or at a pointer past the end of the disk. Ignores `i_size`.
pub fn logical_to_physical(disk: &Disk, inode: &Inode, logical: u32) -> Option<u32> {
    let ptr = if logical < IND_FIRST {
        inode.block[logical as usize]
    } else if logical < DIND_FIRST {
        let ind = inode.block[IND_BLOCK];
        if !followable(disk, ind) {
            return None;
        }
        read_pointers(disk, ind)[(logical - IND_FIRST) as usize]
    } else if logical < MAX_FILE_BLOCKS {
        let dind = inode.block[DIND_BLOCK];
        if !followable(disk, dind) {
            return None;
        }
        let rel = logical - DIND_FIRST;
        let mid = read_pointers(disk, dind)[(rel / PTRS_PER_BLOCK) as usize];
        if !followable(disk, mid) {
            return None;
        }
        read_pointers(disk, mid)[(rel % PTRS_PER_BLOCK) as usize]
    } else {
        return None;
    };
    followable(disk, ptr).then_some(ptr)
}

/// The data blocks of `inode` in logical order, `blocks_needed(i_size)` of
/// them (capped at `MAX_FILE_BLOCKS`). Stops early at the first unmapped
/// block: this crate never writes holes, so one means a damaged mapping.
/// Reads each indirect block once.
pub fn file_blocks(disk: &Disk, inode: &Inode) -> Vec<u32> {
    let want = blocks_needed(inode.size as u64).min(MAX_FILE_BLOCKS) as usize;
    let mut out = Vec::with_capacity(want);
    // Push `ptr` if more blocks are wanted and it is mapped; false stops the walk.
    let take = |out: &mut Vec<u32>, ptr: u32| -> bool {
        if out.len() == want || !followable(disk, ptr) {
            return false;
        }
        out.push(ptr);
        true
    };
    for &ptr in &inode.block[..DIRECT_BLOCKS] {
        if !take(&mut out, ptr) {
            return out;
        }
    }
    if out.len() == want || !followable(disk, inode.block[IND_BLOCK]) {
        return out;
    }
    for ptr in read_pointers(disk, inode.block[IND_BLOCK]) {
        if !take(&mut out, ptr) {
            return out;
        }
    }
    if out.len() == want || !followable(disk, inode.block[DIND_BLOCK]) {
        return out;
    }
    for mid in read_pointers(disk, inode.block[DIND_BLOCK]) {
        if out.len() == want || !followable(disk, mid) {
            return out;
        }
        for ptr in read_pointers(disk, mid) {
            if !take(&mut out, ptr) {
                return out;
            }
        }
    }
    out
}

/// The indirect blocks that map `inode`'s `blocks_needed(i_size)` data
/// blocks, as `(block, level)` in the order they are referenced: the
/// single-indirect block (level 1), then the double-indirect block
/// (level 2) followed by each second-level block it uses (level 1).
/// Unmapped or out-of-disk pointers are skipped. For a well-formed inode
/// the length is `indirect_blocks_needed(blocks_needed(i_size))`.
pub fn indirect_blocks(disk: &Disk, inode: &Inode) -> Vec<(u32, u8)> {
    let want = blocks_needed(inode.size as u64).min(MAX_FILE_BLOCKS);
    let mut out = Vec::new();
    if want > IND_FIRST && followable(disk, inode.block[IND_BLOCK]) {
        out.push((inode.block[IND_BLOCK], 1));
    }
    let dind = inode.block[DIND_BLOCK];
    if want > DIND_FIRST && followable(disk, dind) {
        out.push((dind, 2));
        let mids = (want - DIND_FIRST).div_ceil(PTRS_PER_BLOCK) as usize;
        for mid in read_pointers(disk, dind).into_iter().take(mids) {
            if followable(disk, mid) {
                out.push((mid, 1));
            }
        }
    }
    out
}

/// Point `inode` at `data`: logical block `i` becomes `data[i]`. Sets the 12
/// direct pointers; allocates from `goal_group`, in one `alloc_blocks` call,
/// each single-indirect, double-indirect, and second-level block the list
/// needs that the inode does not already hold; writes every pointer block in
/// full (unused slots zero); sets `inode.blocks` to the data plus pointer
/// blocks in 512-byte units. Pointer blocks the inode already holds are
/// reused, so a file that grows keeps them; a caller that shrinks a file
/// releases the ones it no longer needs first. `FileTooLarge` past
/// `MAX_FILE_BLOCKS` and `DiskFull` from the allocator both return before
/// anything is written or `inode` changes. Emits only the allocator's
/// events; `ExtFs::map_file_blocks` reports the pointer blocks.
pub fn map_blocks(
    ctx: &mut crate::alloc::AllocCtx,
    inode: &mut crate::inode::Inode,
    data: &[u32],
    goal_group: u32,
) -> fs_core::Result<()> {
    use crate::alloc::alloc_blocks;
    use crate::inode::{DIND_BLOCK, DIRECT_BLOCKS, IND_BLOCK};
    use crate::superblock::BLOCK_SIZE;
    use fs_core::Error;

    let n = u32::try_from(data.len()).map_err(|_| Error::FileTooLarge)?;
    if n > MAX_FILE_BLOCKS {
        return Err(Error::FileTooLarge);
    }
    let per_block = PTRS_PER_BLOCK as usize;
    let single_end = DIRECT_BLOCKS + per_block;
    let needs_single = data.len() > DIRECT_BLOCKS;
    let children = data.len().saturating_sub(single_end).div_ceil(per_block);
    let mut child_ptrs = [0u32; PTRS_PER_BLOCK as usize];
    if inode.block[DIND_BLOCK] != 0 {
        child_ptrs = read_pointers(ctx.disk, inode.block[DIND_BLOCK]);
    }
    let new_single = needs_single && inode.block[IND_BLOCK] == 0;
    let new_double = children > 0 && inode.block[DIND_BLOCK] == 0;
    let new_children = child_ptrs[..children].iter().filter(|&&p| p == 0).count();
    let missing = u32::from(new_single) + u32::from(new_double) + new_children as u32;
    let mut fresh = alloc_blocks(ctx, goal_group, missing)?.into_iter();
    let mut next = || {
        fresh
            .next()
            .expect("alloc_blocks returns exactly the count asked for")
    };
    if new_single {
        inode.block[IND_BLOCK] = next();
    }
    if new_double {
        inode.block[DIND_BLOCK] = next();
    }
    for p in child_ptrs[..children].iter_mut().filter(|p| **p == 0) {
        *p = next();
    }
    for (i, slot) in inode.block[..DIRECT_BLOCKS].iter_mut().enumerate() {
        *slot = data.get(i).copied().unwrap_or(0);
    }
    if needs_single {
        let end = data.len().min(single_end);
        write_pointer_block(ctx.disk, inode.block[IND_BLOCK], &data[DIRECT_BLOCKS..end]);
    } else {
        inode.block[IND_BLOCK] = 0;
    }
    if children > 0 {
        for (k, &child) in child_ptrs[..children].iter().enumerate() {
            let start = single_end + k * per_block;
            let end = data.len().min(start + per_block);
            write_pointer_block(ctx.disk, child, &data[start..end]);
        }
        write_pointer_block(ctx.disk, inode.block[DIND_BLOCK], &child_ptrs[..children]);
    } else {
        inode.block[DIND_BLOCK] = 0;
    }
    inode.blocks = (n + indirect_blocks_needed(n)) * (BLOCK_SIZE / 512);
    Ok(())
}

/// One pointer block: `pointers` little-endian from byte 0, zeros after.
fn write_pointer_block(disk: &mut fs_core::Disk, block: u32, pointers: &[u32]) {
    let mut bytes = [0u8; 1024];
    for (chunk, p) in bytes.chunks_exact_mut(4).zip(pointers) {
        chunk.copy_from_slice(&p.to_le_bytes());
    }
    disk.write(block as usize * 1024, &bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::inode::Inode;
    use fs_core::Disk;

    fn put_pointers(disk: &mut Disk, block: u32, ptrs: &[u32]) {
        let bytes: Vec<u8> = ptrs.iter().flat_map(|p| p.to_le_bytes()).collect();
        disk.write(block as usize * 1024, &bytes);
    }

    /// A 2,048-block disk and a 526-block file: direct blocks 100..=111,
    /// single-indirect block 20 mapping 1000..=1255, double-indirect block
    /// 21 whose entries 22 and 23 map 1300..=1555 and 1600..=1601.
    fn hand_built() -> (Disk, Inode) {
        let mut disk = Disk::new(1024, 2048);
        let single: Vec<u32> = (1000..1256).collect();
        put_pointers(&mut disk, 20, &single);
        put_pointers(&mut disk, 21, &[22, 23]);
        let second: Vec<u32> = (1300..1556).collect();
        put_pointers(&mut disk, 22, &second);
        put_pointers(&mut disk, 23, &[1600, 1601]);
        let mut inode = Inode::new_file(0);
        for (i, ptr) in inode.block.iter_mut().enumerate().take(12) {
            *ptr = 100 + i as u32;
        }
        inode.block[12] = 20;
        inode.block[13] = 21;
        inode.size = 526 * 1024;
        (disk, inode)
    }

    #[test]
    fn blocks_needed_rounds_up_to_whole_blocks() {
        assert_eq!(blocks_needed(0), 0);
        assert_eq!(blocks_needed(1), 1);
        assert_eq!(blocks_needed(1024), 1);
        assert_eq!(blocks_needed(1025), 2);
        assert_eq!(blocks_needed(12 * 1024), 12);
        assert_eq!(blocks_needed(12 * 1024 + 1), 13);
        assert_eq!(blocks_needed(268 * 1024), 268);
        assert_eq!(blocks_needed(268 * 1024 + 1), 269);
        assert_eq!(blocks_needed(65_804 * 1024), 65_804);
        assert_eq!(blocks_needed(u64::MAX), u32::MAX);
        assert_eq!(MAX_FILE_BLOCKS, 65_804);
    }

    #[test]
    fn indirect_blocks_needed_at_the_boundaries() {
        assert_eq!(indirect_blocks_needed(0), 0);
        assert_eq!(indirect_blocks_needed(1), 0);
        assert_eq!(indirect_blocks_needed(12), 0);
        assert_eq!(indirect_blocks_needed(13), 1);
        assert_eq!(indirect_blocks_needed(268), 1);
        assert_eq!(indirect_blocks_needed(269), 3);
        assert_eq!(indirect_blocks_needed(524), 3);
        assert_eq!(indirect_blocks_needed(525), 4);
        assert_eq!(indirect_blocks_needed(65_804), 258);
    }

    #[test]
    fn logical_to_physical_follows_direct_single_and_double_indirect() {
        let (disk, inode) = hand_built();
        assert_eq!(logical_to_physical(&disk, &inode, 0), Some(100));
        assert_eq!(logical_to_physical(&disk, &inode, 11), Some(111));
        assert_eq!(logical_to_physical(&disk, &inode, 12), Some(1000));
        assert_eq!(logical_to_physical(&disk, &inode, 267), Some(1255));
        assert_eq!(logical_to_physical(&disk, &inode, 268), Some(1300));
        assert_eq!(logical_to_physical(&disk, &inode, 523), Some(1555));
        assert_eq!(logical_to_physical(&disk, &inode, 524), Some(1600));
        assert_eq!(logical_to_physical(&disk, &inode, 525), Some(1601));
        assert_eq!(logical_to_physical(&disk, &inode, 526), None);
        assert_eq!(logical_to_physical(&disk, &inode, 780), None);
        assert_eq!(logical_to_physical(&disk, &inode, MAX_FILE_BLOCKS), None);
    }

    #[test]
    fn unmapped_and_out_of_disk_pointers_read_as_none() {
        let (disk, mut inode) = hand_built();
        inode.block[0] = 5000;
        assert_eq!(logical_to_physical(&disk, &inode, 0), None);
        inode.block[12] = 0;
        assert_eq!(logical_to_physical(&disk, &inode, 12), None);
        inode.block[13] = 9999;
        assert_eq!(logical_to_physical(&disk, &inode, 268), None);
        assert_eq!(read_pointers(&disk, 0), [0u32; 256]);
        assert_eq!(read_pointers(&disk, 2048), [0u32; 256]);
        assert_eq!(read_pointers(&disk, 23)[..3], [1600, 1601, 0]);
    }

    #[test]
    fn file_blocks_lists_every_data_block_in_order() {
        let (disk, mut inode) = hand_built();
        let blocks = file_blocks(&disk, &inode);
        assert_eq!(blocks.len(), 526);
        assert_eq!(blocks[..12], (100..112).collect::<Vec<u32>>()[..]);
        assert_eq!(blocks[12..268], (1000..1256).collect::<Vec<u32>>()[..]);
        assert_eq!(blocks[268..524], (1300..1556).collect::<Vec<u32>>()[..]);
        assert_eq!(blocks[524..], [1600, 1601]);
        for (logical, &physical) in blocks.iter().enumerate() {
            assert_eq!(
                logical_to_physical(&disk, &inode, logical as u32),
                Some(physical)
            );
        }
        inode.size = 13 * 1024 - 1;
        assert_eq!(file_blocks(&disk, &inode).len(), 13);
        assert_eq!(file_blocks(&disk, &inode)[12], 1000);
        inode.size = 0;
        assert!(file_blocks(&disk, &inode).is_empty());
        inode.size = 5 * 1024;
        inode.block[3] = 0;
        assert_eq!(file_blocks(&disk, &inode), vec![100, 101, 102]);
    }

    #[test]
    fn indirect_blocks_lists_what_the_size_needs_in_reference_order() {
        let (disk, mut inode) = hand_built();
        assert_eq!(
            indirect_blocks(&disk, &inode),
            vec![(20, 1), (21, 2), (22, 1), (23, 1)]
        );
        assert_eq!(
            indirect_blocks(&disk, &inode).len() as u32,
            indirect_blocks_needed(526)
        );
        inode.size = 269 * 1024;
        assert_eq!(
            indirect_blocks(&disk, &inode),
            vec![(20, 1), (21, 2), (22, 1)]
        );
        inode.size = 268 * 1024;
        assert_eq!(indirect_blocks(&disk, &inode), vec![(20, 1)]);
        inode.size = 12 * 1024;
        assert!(indirect_blocks(&disk, &inode).is_empty());
    }
}
