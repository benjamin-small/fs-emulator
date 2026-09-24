//! The post-pass transaction builder (spec section 5): run the unchanged
//! ext2 body under a draft operation, classify the blocks it touched, take
//! their after-images, put the disk back, and re-emit the honest write
//! sequence of section 5.2 as the recorded operation, stopping at an armed
//! crash phase (section 6).

use super::state::JournalState;
use super::{
    descriptor_blocks_needed, encode_commit, encode_descriptor, escape, needs_escape, CrashPhase,
    JournalMode, JournalSuperblock, Tag, FEATURE_INCOMPAT_RECOVER, TAGS_PER_DESCRIPTOR, TAG_ESCAPE,
};
use crate::events::{ExtEvent, JournalWriteKind};
use crate::fs::{BlockOwner, BlockRole, ExtFs};
use crate::group;
use crate::superblock::{Superblock, BLOCK_SIZE, SUPERBLOCK_OFFSET};
use fs_core::{ByteChange, Disk, Error, OpRecord, Result};
use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

const BS: usize = BLOCK_SIZE as usize;
/// `s_feature_incompat` in the ext superblock.
const INCOMPAT_OFFSET: usize = 0x60;
/// The block that holds the primary superblock (1 KiB blocks).
const PRIMARY_SUPERBLOCK_BLOCK: u32 = (SUPERBLOCK_OFFSET / BS) as u32;

/// What a draft touched, split at block boundaries (spec 5.1 step 3).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct Classified {
    /// The portions of draft changes that lie in data blocks, in draft
    /// order: what ordered mode writes home at step 2.
    pub data_changes: Vec<ByteChange>,
    /// The data blocks touched.
    pub data_blocks: BTreeSet<u32>,
    /// Every other block touched.
    pub metadata_blocks: BTreeSet<u32>,
}

impl Classified {
    /// The tagged set in ascending block order: the metadata blocks in
    /// ordered mode, metadata and data blocks together in data mode.
    pub fn tagged(&self, mode: JournalMode) -> Vec<u32> {
        match mode {
            JournalMode::Ordered => self.metadata_blocks.iter().copied().collect(),
            JournalMode::Data => self
                .metadata_blocks
                .union(&self.data_blocks)
                .copied()
                .collect(),
        }
    }
}

/// Split every change at 1024-byte block boundaries and sort the pieces: a
/// block is data when `owners` lists it with `BlockRole::Data`, metadata
/// otherwise (superblock and descriptor copies, bitmaps, inode tables,
/// directory and indirect blocks, anything else).
pub(crate) fn classify(changes: &[ByteChange], owners: &BTreeMap<u32, BlockOwner>) -> Classified {
    let mut out = Classified::default();
    for change in changes {
        let end = change.offset + change.after.len();
        let mut at = change.offset;
        while at < end {
            let block = (at / BS) as u32;
            let stop = end.min((at / BS + 1) * BS);
            let is_data = owners
                .get(&block)
                .is_some_and(|owner| owner.role == BlockRole::Data);
            if is_data {
                let (from, to) = (at - change.offset, stop - change.offset);
                out.data_changes.push(ByteChange {
                    offset: at,
                    before: change.before[from..to].to_vec(),
                    after: change.after[from..to].to_vec(),
                });
                out.data_blocks.insert(block);
            } else {
                out.metadata_blocks.insert(block);
            }
            at = stop;
        }
    }
    out
}

/// The 1024-byte after-image of every tagged block as `disk` holds it,
/// with `needs_recovery` set in the primary superblock's image (spec 5.1
/// step 4), so the checkpoint and a replay write the same bytes.
pub(crate) fn after_images(disk: &Disk, tagged: &[u32]) -> Vec<Vec<u8>> {
    tagged
        .iter()
        .map(|&block| {
            let mut image = disk.read(block as usize * BS, BS).to_vec();
            if block == PRIMARY_SUPERBLOCK_BLOCK {
                let at = SUPERBLOCK_OFFSET % BS + INCOMPAT_OFFSET;
                let flags = le32(&image[at..at + 4]) | FEATURE_INCOMPAT_RECOVER;
                image[at..at + 4].copy_from_slice(&flags.to_le_bytes());
            }
            image
        })
        .collect()
}

fn le32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

/// Put every draft change back in reverse order, outside any operation.
fn undo(disk: &mut Disk, draft: &OpRecord) {
    for change in draft.changes.iter().rev() {
        disk.write(change.offset, &change.before);
    }
}

/// One transaction ready to emit.
struct Transaction {
    tid: u32,
    uuid: [u8; 16],
    commit_sec: u64,
    /// Step 2's writes: the data portions in ordered mode, none in data mode.
    data_changes: Vec<ByteChange>,
    tagged: Vec<u32>,
    images: Vec<Vec<u8>>,
}

/// Where the emission stopped.
enum Outcome {
    /// Every step ran; the next transaction starts at `head`.
    Completed { head: u32 },
    /// The armed phase stopped it after the change at `range`.
    Crashed {
        phase: CrashPhase,
        range: Range<usize>,
    },
}

/// Writes under the open operation, remembering the last range written.
struct Emitter<'a> {
    disk: &'a mut Disk,
    last: Range<usize>,
}

impl Emitter<'_> {
    fn write(&mut self, offset: usize, bytes: &[u8]) -> Range<usize> {
        self.disk.write(offset, bytes);
        self.last = offset..offset + bytes.len();
        self.last.clone()
    }

    fn event(&mut self, event: ExtEvent) {
        self.disk.event(Box::new(event));
    }

    /// Set or clear `needs_recovery` in the primary superblock: the four
    /// bytes of `s_feature_incompat` (steps 1 and 8).
    fn recovery_flag(&mut self, set: bool) -> Range<usize> {
        let at = SUPERBLOCK_OFFSET + INCOMPAT_OFFSET;
        let flags = le32(self.disk.read(at, 4));
        let flags = if set {
            flags | FEATURE_INCOMPAT_RECOVER
        } else {
            flags & !FEATURE_INCOMPAT_RECOVER
        };
        self.write(at, &flags.to_le_bytes())
    }

    /// `s_sequence` and `s_start` of the journal superblock, 8 bytes at
    /// 0x18 of journal index 0 (steps 3 and 7).
    fn journal_superblock(&mut self, j: &JournalState, sequence: u32, start: u32) -> Range<usize> {
        let mut bytes = [0u8; 8];
        bytes[..4].copy_from_slice(&sequence.to_be_bytes());
        bytes[4..].copy_from_slice(&start.to_be_bytes());
        self.write(j.offset(0) + JournalSuperblock::SEQUENCE_OFFSET, &bytes)
    }
}

/// Emit spec 5.2 steps 1 to 8 for `txn` under the open operation, stopping
/// where `j.armed` says (spec section 6).
fn emit(disk: &mut Disk, j: &JournalState, txn: &Transaction) -> Outcome {
    let mut out = Emitter { disk, last: 0..0 };
    let tid = txn.tid;
    let crashed = |phase, range| Outcome::Crashed { phase, range };

    // 1. needs_recovery in the primary superblock.
    let range = out.recovery_flag(true);
    out.event(ExtEvent::RecoveryFlagSet { range });
    // 2. Ordered mode: the file data home, as the draft wrote it.
    for change in &txn.data_changes {
        out.write(change.offset, &change.after);
    }
    // 3. The journal superblock names the transaction.
    let start = j.head;
    let range = out.journal_superblock(j, tid, start);
    out.event(ExtEvent::TransactionStarted {
        tid,
        journal_block: start,
        tagged: txn.tagged.len() as u32,
        range,
    });
    // 4. Descriptor blocks, each followed by the copies it tags.
    let mut index = start;
    let mut copy_index = Vec::with_capacity(txn.tagged.len());
    for d in 0..descriptor_blocks_needed(txn.tagged.len()) {
        let part = d * TAGS_PER_DESCRIPTOR..txn.tagged.len().min((d + 1) * TAGS_PER_DESCRIPTOR);
        let copies: Vec<(Tag, Vec<u8>)> = txn.tagged[part.clone()]
            .iter()
            .zip(&txn.images[part])
            .map(|(&home, image)| {
                let mut copy = image.clone();
                let flags = if needs_escape(&copy) {
                    escape(&mut copy);
                    TAG_ESCAPE
                } else {
                    0
                };
                (Tag { block: home, flags }, copy)
            })
            .collect();
        let tags: Vec<Tag> = copies.iter().map(|(tag, _)| *tag).collect();
        let range = out.write(j.offset(index), &encode_descriptor(tid, &txn.uuid, &tags));
        out.event(ExtEvent::JournalBlockWritten {
            tid,
            kind: JournalWriteKind::Descriptor,
            index,
            block: j.physical(index),
            range,
        });
        for (tag, copy) in copies {
            index = j.wrap(index + 1);
            let range = out.write(j.offset(index), &copy);
            out.event(ExtEvent::JournalBlockWritten {
                tid,
                kind: JournalWriteKind::Copy {
                    home: tag.block,
                    escaped: tag.flags & TAG_ESCAPE != 0,
                },
                index,
                block: j.physical(index),
                range,
            });
            copy_index.push(index);
        }
        index = j.wrap(index + 1);
    }
    if j.armed == Some(CrashPhase::BeforeCommit) {
        return crashed(CrashPhase::BeforeCommit, out.last);
    }
    // 5. The commit block.
    let range = out.write(j.offset(index), &encode_commit(tid, txn.commit_sec));
    out.event(ExtEvent::JournalBlockWritten {
        tid,
        kind: JournalWriteKind::Commit,
        index,
        block: j.physical(index),
        range,
    });
    let head = j.wrap(index + 1);
    if j.armed == Some(CrashPhase::AfterCommit) {
        return crashed(CrashPhase::AfterCommit, out.last);
    }
    // 6. Checkpoint: every after-image home, whole blocks, in tag order.
    for ((&home, image), &from_index) in txn.tagged.iter().zip(&txn.images).zip(&copy_index) {
        let range = out.write(home as usize * BS, image);
        out.event(ExtEvent::Checkpointed {
            tid,
            home,
            from_index,
            range,
        });
        if j.armed == Some(CrashPhase::DuringCheckpoint) {
            return crashed(CrashPhase::DuringCheckpoint, out.last);
        }
    }
    // 7. The journal is empty again. JBD2 tids wrap on overflow.
    let next_tid = tid.wrapping_add(1);
    let range = out.journal_superblock(j, next_tid, 0);
    out.event(ExtEvent::JournalEmptied {
        next_sequence: next_tid,
        range,
    });
    // 8. needs_recovery cleared.
    let range = out.recovery_flag(false);
    out.event(ExtEvent::RecoveryFlagCleared { range });
    Outcome::Completed { head }
}

/// A mutation on an ext3 volume whose journal is `j` (spec 5.1 steps 2 to
/// 7): the draft, the classification, the size check, the restore, and the
/// emitted sequence. Step 1, the `NeedsRecovery` refusal, is each
/// mutation's `ensure_recovered` before its own checks. A crash is a
/// successful operation whose record stops at the armed phase. `j` goes
/// back into `fs.journal` with the new `head` and `sequence`, or marked as
/// needing recovery.
pub(crate) fn run_journaled(
    fs: &mut ExtFs,
    mut j: JournalState,
    op: String,
    body: impl FnOnce(&mut ExtFs) -> Result<()>,
) -> Result<OpRecord> {
    // 2. The draft.
    let saved = (fs.sb.clone(), fs.gds.clone(), fs.journal.clone());
    fs.disk.begin_op(op.clone());
    let result = body(fs);
    let draft = fs.disk.end_op();
    let rollback = |fs: &mut ExtFs, draft: &OpRecord, e: Error| {
        undo(&mut fs.disk, draft);
        (fs.sb, fs.gds, fs.journal) = saved;
        Err(e)
    };
    if let Err(e) = result {
        return rollback(fs, &draft, e);
    }
    // 3 and 4. Classify on the post-body disk and take the after-images.
    let classified = classify(&draft.changes, &fs.block_owners());
    let tagged = classified.tagged(j.mode);
    let images = after_images(&fs.disk, &tagged);
    // 5. The size limit.
    let n = tagged.len() as u32;
    if n > j.max_transaction() {
        let e = Error::Unsupported(format!(
            "transaction of {n} blocks exceeds the journal's maximum of {}",
            j.max_transaction()
        ));
        return rollback(fs, &draft, e);
    }
    // 6. Back to the pre-body bytes; `sb` and `gds` keep the body's values.
    undo(&mut fs.disk, &draft);
    // 7. The real operation.
    let name = match j.armed {
        Some(phase) => format!("{op} (crashed {phase})"),
        None => op,
    };
    fs.disk.begin_op(name);
    for event in draft.events {
        fs.disk.event(event);
    }
    let txn = Transaction {
        tid: j.sequence,
        uuid: fs.sb.uuid,
        commit_sec: fs.now.to_unix_seconds().max(1) as u64,
        data_changes: match j.mode {
            JournalMode::Ordered => classified.data_changes,
            JournalMode::Data => Vec::new(),
        },
        tagged,
        images,
    };
    let outcome = emit(&mut fs.disk, &j, &txn);
    if let Outcome::Crashed { phase, range } = &outcome {
        fs.disk.event(Box::new(ExtEvent::Crashed {
            phase: *phase,
            range: range.clone(),
        }));
    }
    let record = fs_core::finish_op(&mut fs.disk, &mut fs.history, Ok(()))?;
    match outcome {
        Outcome::Completed { head } => {
            j.head = head;
            j.sequence = txn.tid.wrapping_add(1);
        }
        Outcome::Crashed { .. } => {
            j.needs_recovery = true;
            j.armed = None;
            reload_metadata(fs);
        }
    }
    fs.journal = Some(j);
    Ok(record)
}

/// Re-decode the cached superblock and descriptors from the disk after a
/// crash, so they describe the raw on-disk state.
fn reload_metadata(fs: &mut ExtFs) {
    let bytes = fs.disk.as_bytes();
    fs.sb = Superblock::decode(&bytes[SUPERBLOCK_OFFSET..SUPERBLOCK_OFFSET + Superblock::LEN]);
    let table = fs.geo.group(0).descriptors_block.unwrap_or(2) as usize * BS;
    let len = fs.geo.descriptor_blocks as usize * BS;
    fs.gds = group::decode_table(&bytes[table..table + len], fs.geo.groups);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owner(role: BlockRole) -> BlockOwner {
        BlockOwner {
            inode: 12,
            path: "/f".into(),
            role,
        }
    }

    fn change(offset: usize, len: usize, before: u8, after: u8) -> ByteChange {
        ByteChange {
            offset,
            before: vec![before; len],
            after: vec![after; len],
        }
    }

    #[test]
    fn a_change_across_a_directory_and_a_data_block_splits_in_two() {
        let owners = BTreeMap::from([
            (100, owner(BlockRole::Directory)),
            (101, owner(BlockRole::Data)),
        ]);
        let spanning = ByteChange {
            offset: 101 * BS - 4,
            before: vec![0, 0, 0, 0, 0, 0, 0, 0],
            after: vec![1, 2, 3, 4, 5, 6, 7, 8],
        };
        let c = classify(&[spanning], &owners);
        assert_eq!(
            c.data_changes,
            vec![ByteChange {
                offset: 101 * BS,
                before: vec![0; 4],
                after: vec![5, 6, 7, 8],
            }]
        );
        assert_eq!(c.data_blocks, BTreeSet::from([101]));
        assert_eq!(c.metadata_blocks, BTreeSet::from([100]));
        assert_eq!(c.tagged(JournalMode::Ordered), vec![100]);
        assert_eq!(c.tagged(JournalMode::Data), vec![100, 101]);
    }

    #[test]
    fn unowned_indirect_and_directory_blocks_are_metadata_in_block_order() {
        let owners = BTreeMap::from([
            (69, owner(BlockRole::Directory)),
            (1114, owner(BlockRole::Indirect)),
            (1111, owner(BlockRole::Data)),
            (1112, owner(BlockRole::Data)),
            (1113, owner(BlockRole::Data)),
        ]);
        let changes = [
            // Data 1111..1114 in one run, then inode 12 (block 6 of the
            // inode table), a bitmap, the superblock counters, the pointer
            // block, the directory.
            change(1111 * BS, 3 * BS, 0, 7),
            change(5 * BS + 11 * 128, 128, 0, 1),
            change(3 * BS + 138, 1, 0, 0xFF),
            change(SUPERBLOCK_OFFSET + 12, 8, 0, 2),
            change(1114 * BS, BS, 0, 3),
            change(69 * BS + 24, 20, 0, 4),
        ];
        let c = classify(&changes, &owners);
        assert_eq!(
            c.data_changes
                .iter()
                .map(|d| (d.offset, d.after.len()))
                .collect::<Vec<_>>(),
            vec![(1111 * BS, BS), (1112 * BS, BS), (1113 * BS, BS)]
        );
        assert_eq!(c.tagged(JournalMode::Ordered), vec![1, 3, 6, 69, 1114]);
        assert_eq!(
            c.tagged(JournalMode::Data),
            vec![1, 3, 6, 69, 1111, 1112, 1113, 1114]
        );
    }

    #[test]
    fn a_data_block_written_twice_keeps_both_portions_in_draft_order() {
        let owners = BTreeMap::from([(200, owner(BlockRole::Data))]);
        let changes = [
            change(200 * BS, 10, 0, 1),
            change(200 * BS + 10, BS - 10, 0, 0),
        ];
        let c = classify(&changes, &owners);
        assert_eq!(c.data_changes, changes.to_vec());
        assert_eq!(c.data_blocks, BTreeSet::from([200]));
        assert!(c.metadata_blocks.is_empty());
        assert!(c.tagged(JournalMode::Ordered).is_empty());
    }

    #[test]
    fn after_images_set_needs_recovery_only_in_the_primary_superblock() {
        let mut disk = Disk::new(BS, 8);
        disk.write(
            SUPERBLOCK_OFFSET + INCOMPAT_OFFSET,
            &0x0002u32.to_le_bytes(),
        );
        disk.write(5 * BS, &[9; BS]);
        disk.write(6 * BS + INCOMPAT_OFFSET, &0x0002u32.to_le_bytes());
        let images = after_images(&disk, &[1, 5, 6]);
        assert_eq!(images.len(), 3);
        assert!(images.iter().all(|image| image.len() == BS));
        let mut want = disk.read(SUPERBLOCK_OFFSET, BS).to_vec();
        want[INCOMPAT_OFFSET] = 0x06;
        assert_eq!(images[0], want);
        assert_eq!(images[1], vec![9; BS]);
        assert_eq!(images[2], disk.read(6 * BS, BS));
        // The disk itself is untouched.
        assert_eq!(disk.read(SUPERBLOCK_OFFSET + INCOMPAT_OFFSET, 1), &[0x02]);
    }
}
