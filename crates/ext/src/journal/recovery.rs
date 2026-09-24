//! Recovery (spec section 6): the walk of the log from `s_start` that
//! jbd2's scan (`PASS_SCAN`) makes, which `journal_blocks` shares for its
//! live parse (spec section 7); the scan built on it; then the replay that
//! writes every committed copy home, empties the journal, and clears
//! `needs_recovery`. `ExtFs::recover` runs `scan` before it opens the
//! `recover` operation, so every check fails with nothing written, and
//! `replay` inside it.

use super::state::JournalState;
use super::{
    decode_descriptor, read_header, unescape, JournalSuperblock, Tag, BLOCKTYPE_COMMIT,
    BLOCKTYPE_DESCRIPTOR, BLOCKTYPE_REVOKE, FEATURE_INCOMPAT_RECOVER, TAG_ESCAPE,
};
use crate::events::ExtEvent;
use crate::superblock::{BLOCK_SIZE, SUPERBLOCK_OFFSET};
use fs_core::{Disk, Error, Result};

const BS: usize = BLOCK_SIZE as usize;
/// `s_feature_incompat` in the ext superblock.
const INCOMPAT_OFFSET: usize = 0x60;

/// What a log block holds.
#[derive(Debug, PartialEq, Eq)]
pub(crate) enum LogBlock {
    /// A descriptor's tags, each with the journal index of the copy it
    /// names (the ring rule from the descriptor's index), or the error when
    /// the tags do not parse.
    Descriptor(Result<Vec<(u32, Tag)>>),
    Commit,
    Revoke,
}

/// One log block: its journal index, its header sequence, what it holds.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct LogEntry {
    pub index: u32,
    pub sequence: u32,
    pub block: LogBlock,
}

/// Parse journal index `index` as a log block: `None` without the magic or
/// for a block type other than descriptor, commit, or revoke.
pub(crate) fn parse_block(disk: &Disk, journal: &JournalState, index: u32) -> Option<LogEntry> {
    let bytes = disk.read(journal.offset(index), BS);
    let header = read_header(bytes)?;
    let block = match header.blocktype {
        BLOCKTYPE_DESCRIPTOR => LogBlock::Descriptor(decode_descriptor(bytes).map(|tags| {
            let mut at = index;
            tags.into_iter()
                .map(|tag| {
                    at = journal.wrap(at + 1);
                    (at, tag)
                })
                .collect()
        })),
        BLOCKTYPE_COMMIT => LogBlock::Commit,
        BLOCKTYPE_REVOKE => LogBlock::Revoke,
        _ => return None,
    };
    Some(LogEntry {
        index,
        sequence: header.sequence,
        block,
    })
}

/// The log from `start` with `expected = sequence`, as jbd2's scan walks it
/// (spec section 6, step 2): every block with the magic, the expected
/// sequence, and a log block type, in log order. A descriptor's copies are
/// skipped (`pos = wrap(pos + 1 + tags)`), a commit moves `expected` on,
/// and a revoke block is yielded and the walk goes on: what it means is the
/// caller's (`scan` refuses it, the live parse of `journal_blocks` shows
/// it). The walk ends at the first block that does not fit, after a
/// descriptor whose tags do not parse or whose copies run past the end of
/// the log more than once, and when it has come around the ring (more than
/// `maxlen - first` blocks; see `lapped`). Nothing is walked when `start`
/// is 0 (an empty journal) or outside the log.
pub(crate) struct LogWalk<'a> {
    disk: &'a Disk,
    journal: &'a JournalState,
    pos: u32,
    expected: u32,
    walked: u32,
    lapped: bool,
    done: bool,
}

impl<'a> LogWalk<'a> {
    pub(crate) fn new(
        disk: &'a Disk,
        journal: &'a JournalState,
        start: u32,
        sequence: u32,
    ) -> Self {
        LogWalk {
            disk,
            journal,
            pos: start,
            expected: sequence,
            walked: 0,
            lapped: false,
            done: !(journal.first..journal.maxlen).contains(&start),
        }
    }

    /// Whether the walk ended because it came around the ring.
    pub(crate) fn lapped(&self) -> bool {
        self.lapped
    }
}

impl Iterator for LogWalk<'_> {
    type Item = LogEntry;

    fn next(&mut self) -> Option<LogEntry> {
        let lap = self.journal.maxlen - self.journal.first;
        if self.done {
            return None;
        }
        if self.walked > lap {
            self.lapped = true;
            self.done = true;
            return None;
        }
        let entry = parse_block(self.disk, self.journal, self.pos)
            .filter(|entry| entry.sequence == self.expected);
        let Some(entry) = entry else {
            self.done = true;
            return None;
        };
        let len = match &entry.block {
            LogBlock::Descriptor(Ok(tags)) => 1 + tags.len() as u32,
            LogBlock::Descriptor(Err(_)) => {
                self.done = true;
                return Some(entry);
            }
            LogBlock::Commit => {
                self.expected = self.expected.wrapping_add(1);
                1
            }
            LogBlock::Revoke => 1,
        };
        self.walked += len;
        // `wrap` folds one pass past the end of the log, no more.
        if self.pos + len >= self.journal.maxlen + lap {
            self.done = true;
        } else {
            self.pos = self.journal.wrap(self.pos + len);
        }
        Some(entry)
    }
}

/// One transaction the scan found in the log.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ScannedTransaction {
    pub tid: u32,
    /// The journal index of its first descriptor block (of its commit
    /// block for a transaction that tags nothing).
    pub descriptor_index: u32,
    /// Every tag of its descriptor blocks in log order, each with the
    /// journal index of the copy it names.
    pub tags: Vec<(u32, Tag)>,
    /// Whether its commit block followed; an uncommitted transaction is
    /// always the last one found, and recovery discards it.
    pub committed: bool,
}

/// The transactions `LogWalk` finds from `s_start` (spec section 6, step
/// 2): a descriptor's tags join the transaction of its sequence, and a
/// commit closes it. Nothing is found when `s_start` is 0. `Unsupported`
/// for a revoke block; `CorruptImage` for tags that do not parse, a tag at
/// or beyond `total_blocks` (the caller's `geo.total_blocks`, not
/// `disk.sector_count()`: a raw write can shrink `s_blocks_count` below
/// what the disk holds, and `JournalState::open` already checks journal
/// blocks against `geo.total_blocks`, so this must agree with it), a
/// descriptor whose copies would run past the end of the log more than
/// once, or a walk that comes around the ring without ending.
pub(crate) fn scan(
    disk: &Disk,
    journal: &JournalState,
    total_blocks: u32,
) -> Result<Vec<ScannedTransaction>> {
    let jsb = journal.superblock(disk)?;
    let volume = u64::from(total_blocks);
    let lap = journal.maxlen - journal.first;
    let mut found: Vec<ScannedTransaction> = Vec::new();
    let mut walk = LogWalk::new(disk, journal, jsb.start, jsb.sequence);
    for LogEntry {
        index: pos,
        sequence: tid,
        block,
    } in walk.by_ref()
    {
        match block {
            LogBlock::Descriptor(tags) => {
                let tags = tags?;
                let n = tags.len() as u32;
                if pos + 1 + n >= journal.maxlen + lap {
                    return Err(Error::CorruptImage(format!(
                        "journal descriptor at journal block {pos} tags {n} blocks, which run \
                         past the end of the {}-block log more than once",
                        journal.maxlen
                    )));
                }
                if let Some((_, tag)) = tags.iter().find(|(_, tag)| u64::from(tag.block) >= volume)
                {
                    return Err(Error::CorruptImage(format!(
                        "journal descriptor at journal block {pos} tags block {}, outside the \
                         {volume}-block volume",
                        tag.block
                    )));
                }
                match found.last_mut() {
                    Some(t) if t.tid == tid => t.tags.extend(tags),
                    _ => found.push(ScannedTransaction {
                        tid,
                        descriptor_index: pos,
                        tags,
                        committed: false,
                    }),
                }
            }
            LogBlock::Commit => match found.last_mut() {
                Some(t) if t.tid == tid => t.committed = true,
                _ => found.push(ScannedTransaction {
                    tid,
                    descriptor_index: pos,
                    tags: Vec::new(),
                    committed: true,
                }),
            },
            LogBlock::Revoke => return Err(Error::Unsupported("journal revoke records".into())),
        }
    }
    if walk.lapped() {
        return Err(Error::CorruptImage(format!(
            "the journal log from journal block {} runs more than once around the ring",
            jsb.start
        )));
    }
    Ok(found)
}

/// Spec section 6 steps 2 (the event) to 5, under the caller's open
/// operation, for what `scan` found: `RecoveryScanned` over `s_sequence`
/// and `s_start`; every committed transaction in order, each tag in order,
/// its copy written home as a whole block with the magic restored where
/// `ESCAPE` is set (`Replayed`); `TransactionDiscarded` over the
/// descriptor of an uncommitted one; the journal superblock emptied with
/// `s_sequence = expected + 1` (`JournalEmptied`); `needs_recovery`
/// cleared in the primary superblock (`RecoveryFlagCleared`). Returns the
/// new `s_sequence`.
pub(crate) fn replay(
    disk: &mut Disk,
    journal: &JournalState,
    found: &[ScannedTransaction],
) -> Result<u32> {
    let jsb = journal.superblock(disk)?;
    let at = journal.offset(0) + JournalSuperblock::SEQUENCE_OFFSET;
    let committed = found.iter().filter(|t| t.committed).count() as u32;
    let tagged = found
        .iter()
        .filter(|t| t.committed)
        .map(|t| t.tags.len() as u32)
        .sum();
    disk.event(Box::new(ExtEvent::RecoveryScanned {
        start: jsb.start,
        sequence: jsb.sequence,
        committed,
        tagged,
        range: at..at + 8,
    }));
    for t in found {
        if !t.committed {
            let from = journal.offset(t.descriptor_index);
            disk.event(Box::new(ExtEvent::TransactionDiscarded {
                tid: t.tid,
                tagged: t.tags.len() as u32,
                range: from..from + BS,
            }));
            continue;
        }
        for &(index, tag) in &t.tags {
            let mut copy = disk.read(journal.offset(index), BS).to_vec();
            if tag.flags & TAG_ESCAPE != 0 {
                unescape(&mut copy);
            }
            let home = tag.block as usize * BS;
            disk.write(home, &copy);
            disk.event(Box::new(ExtEvent::Replayed {
                tid: t.tid,
                home: tag.block,
                from_index: index,
                range: home..home + BS,
            }));
        }
    }
    // Every commit moved `expected` on by one; jbd2 then takes one more
    // (`++info.end_transaction`), so no stale commit can match again.
    let next_sequence = jsb.sequence.wrapping_add(committed).wrapping_add(1);
    let mut bytes = [0u8; 8];
    bytes[..4].copy_from_slice(&next_sequence.to_be_bytes());
    disk.write(at, &bytes);
    disk.event(Box::new(ExtEvent::JournalEmptied {
        next_sequence,
        range: at..at + 8,
    }));
    let flag = SUPERBLOCK_OFFSET + INCOMPAT_OFFSET;
    let raw = disk.read(flag, 4);
    let flags = u32::from_le_bytes([raw[0], raw[1], raw[2], raw[3]]) & !FEATURE_INCOMPAT_RECOVER;
    disk.write(flag, &flags.to_le_bytes());
    disk.event(Box::new(ExtEvent::RecoveryFlagCleared {
        range: flag..flag + 4,
    }));
    Ok(next_sequence)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::{
        encode_commit, encode_descriptor, write_header, JournalMode, JournalSuperblock, TAG_ESCAPE,
        TAG_LAST, TAG_SAME_UUID,
    };

    /// A 64-block disk whose journal of `maxlen` blocks sits at blocks
    /// 20.., with `s_sequence` and `s_start` as given.
    fn journal(maxlen: u32, sequence: u32, start: u32) -> (Disk, JournalState) {
        let mut disk = Disk::new(BS, 64);
        let mut jsb = JournalSuperblock::new(maxlen, [7; 16]);
        jsb.sequence = sequence;
        jsb.start = start;
        let mut bytes = [0u8; JournalSuperblock::LEN];
        jsb.encode(&mut bytes);
        disk.write(20 * BS, &bytes);
        let state = JournalState {
            blocks: (20..20 + maxlen).collect(),
            maxlen,
            first: 1,
            sequence,
            head: 1,
            mode: JournalMode::Ordered,
            needs_recovery: true,
            armed: None,
        };
        (disk, state)
    }

    fn put(disk: &mut Disk, j: &JournalState, index: u32, block: &[u8]) {
        disk.write(j.offset(index), block);
    }

    fn descriptor(sequence: u32, homes: &[u32]) -> [u8; 1024] {
        let tags: Vec<Tag> = homes.iter().map(|&block| Tag { block, flags: 0 }).collect();
        encode_descriptor(sequence, &[7; 16], &tags)
    }

    fn header(blocktype: u32, sequence: u32) -> [u8; 1024] {
        let mut block = [0u8; 1024];
        write_header(&mut block, blocktype, sequence);
        block
    }

    fn homes(t: &ScannedTransaction) -> Vec<(u32, u32)> {
        t.tags
            .iter()
            .map(|&(index, tag)| (index, tag.block))
            .collect()
    }

    #[test]
    fn the_walk_yields_a_revoke_block_and_ends_after_a_broken_descriptor() {
        let (mut disk, j) = journal(16, 5, 1);
        put(&mut disk, &j, 1, &descriptor(5, &[40]));
        put(&mut disk, &j, 3, &header(BLOCKTYPE_REVOKE, 5));
        put(&mut disk, &j, 4, &encode_commit(5, 1));
        // Transaction 6: a revoke block, then a descriptor whose tags run
        // off the block without a last tag; the commit after it is never
        // reached.
        put(&mut disk, &j, 5, &header(BLOCKTYPE_REVOKE, 6));
        put(&mut disk, &j, 6, &header(BLOCKTYPE_DESCRIPTOR, 6));
        put(&mut disk, &j, 7, &encode_commit(6, 1));
        let mut walk = LogWalk::new(&disk, &j, 1, 5);
        let entries: Vec<LogEntry> = walk.by_ref().collect();
        let tag = Tag {
            block: 40,
            flags: TAG_LAST,
        };
        let entry = |index, sequence, block| LogEntry {
            index,
            sequence,
            block,
        };
        assert_eq!(
            entries[..4],
            [
                entry(1, 5, LogBlock::Descriptor(Ok(vec![(2, tag)]))),
                entry(3, 5, LogBlock::Revoke),
                entry(4, 5, LogBlock::Commit),
                entry(5, 6, LogBlock::Revoke),
            ]
        );
        assert_eq!(entries.len(), 5);
        assert_eq!((entries[4].index, entries[4].sequence), (6, 6));
        assert!(matches!(entries[4].block, LogBlock::Descriptor(Err(_))));
        assert!(!walk.lapped());
    }

    #[test]
    fn an_empty_journal_scans_to_nothing() {
        let (mut disk, j) = journal(16, 5, 0);
        // A descriptor at the first log block is ignored while s_start is 0.
        put(&mut disk, &j, 1, &descriptor(5, &[40]));
        assert_eq!(scan(&disk, &j, 64).unwrap(), vec![]);
    }

    #[test]
    fn a_committed_transaction_then_an_uncommitted_one() {
        let (mut disk, j) = journal(16, 5, 3);
        let tags = [
            Tag {
                block: 40,
                flags: 0,
            },
            Tag {
                block: 41,
                flags: TAG_ESCAPE,
            },
        ];
        put(&mut disk, &j, 3, &encode_descriptor(5, &[7; 16], &tags));
        put(&mut disk, &j, 6, &encode_commit(5, 1));
        put(&mut disk, &j, 7, &descriptor(6, &[42]));
        let found = scan(&disk, &j, 64).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!((found[0].tid, found[0].descriptor_index), (5, 3));
        assert!(found[0].committed);
        assert_eq!(homes(&found[0]), vec![(4, 40), (5, 41)]);
        assert_eq!(
            found[0].tags[1].1.flags,
            TAG_ESCAPE | TAG_SAME_UUID | TAG_LAST
        );
        assert_eq!((found[1].tid, found[1].descriptor_index), (6, 7));
        assert!(!found[1].committed);
        assert_eq!(homes(&found[1]), vec![(8, 42)]);
    }

    #[test]
    fn the_scan_follows_the_ring_past_the_last_block() {
        let (mut disk, j) = journal(16, 5, 14);
        put(&mut disk, &j, 14, &descriptor(5, &[40, 41]));
        put(&mut disk, &j, 2, &encode_commit(5, 1));
        put(&mut disk, &j, 3, &descriptor(6, &[42]));
        put(&mut disk, &j, 5, &encode_commit(6, 1));
        let found = scan(&disk, &j, 64).unwrap();
        assert_eq!(homes(&found[0]), vec![(15, 40), (1, 41)]);
        assert_eq!(homes(&found[1]), vec![(4, 42)]);
        assert!(found.iter().all(|t| t.committed));
    }

    #[test]
    fn two_descriptors_of_one_sequence_make_one_transaction() {
        let (mut disk, j) = journal(16, 5, 1);
        put(&mut disk, &j, 1, &descriptor(5, &[40]));
        put(&mut disk, &j, 3, &descriptor(5, &[41]));
        put(&mut disk, &j, 5, &encode_commit(5, 1));
        let found = scan(&disk, &j, 64).unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].descriptor_index, 1);
        assert_eq!(homes(&found[0]), vec![(2, 40), (4, 41)]);
        assert!(found[0].committed);
    }

    #[test]
    fn a_commit_without_a_descriptor_is_an_empty_committed_transaction() {
        let (mut disk, j) = journal(16, 5, 1);
        put(&mut disk, &j, 1, &encode_commit(5, 1));
        let found = scan(&disk, &j, 64).unwrap();
        assert_eq!(
            found,
            vec![ScannedTransaction {
                tid: 5,
                descriptor_index: 1,
                tags: vec![],
                committed: true,
            }]
        );
    }

    #[test]
    fn a_stale_sequence_an_unknown_type_or_a_missing_magic_ends_the_log() {
        // After transaction 5 commits, the scan expects sequence 6.
        for end in [
            // A stale descriptor.
            descriptor(4, &[42]),
            // A stale revoke block: the sequence is checked first.
            header(BLOCKTYPE_REVOKE, 4),
            // A version-1 superblock header: not a log block type.
            header(3, 6),
            // No magic.
            [0u8; 1024],
        ] {
            let (mut disk, j) = journal(16, 5, 1);
            put(&mut disk, &j, 1, &descriptor(5, &[40]));
            put(&mut disk, &j, 3, &encode_commit(5, 1));
            put(&mut disk, &j, 4, &end);
            let found = scan(&disk, &j, 64).unwrap();
            assert_eq!(found.len(), 1);
            assert!(found[0].committed);
        }
    }

    #[test]
    fn a_revoke_block_of_the_expected_sequence_is_unsupported() {
        let (mut disk, j) = journal(16, 5, 1);
        put(&mut disk, &j, 1, &descriptor(5, &[40]));
        put(&mut disk, &j, 3, &header(BLOCKTYPE_REVOKE, 5));
        assert_eq!(
            scan(&disk, &j, 64),
            Err(Error::Unsupported("journal revoke records".into()))
        );
    }

    #[test]
    fn a_tag_outside_the_volume_is_corrupt() {
        let (mut disk, j) = journal(16, 5, 1);
        put(&mut disk, &j, 1, &descriptor(5, &[40, 64]));
        assert_eq!(
            scan(&disk, &j, 64),
            Err(Error::CorruptImage(
                "journal descriptor at journal block 1 tags block 64, outside the 64-block volume"
                    .into()
            ))
        );
    }

    #[test]
    fn scan_checks_tags_against_the_caller_s_volume_bound_not_the_disk_s() {
        // Minor 4: after a raw write shrinks `s_blocks_count` and the
        // corruption gate adopts it, `geo.total_blocks` is smaller than
        // `disk.sector_count()` (64 in this fixture). `scan` must key off
        // the bound its caller passes, so a tag at `total_blocks - 1` is
        // still in the volume and one at `total_blocks` is not, even
        // though both are well inside the disk.
        let (mut disk, j) = journal(16, 5, 1);
        put(&mut disk, &j, 1, &descriptor(5, &[39]));
        assert_eq!(scan(&disk, &j, 40).unwrap().len(), 1);

        let (mut disk, j) = journal(16, 5, 1);
        put(&mut disk, &j, 1, &descriptor(5, &[40]));
        assert_eq!(
            scan(&disk, &j, 40),
            Err(Error::CorruptImage(
                "journal descriptor at journal block 1 tags block 40, outside the 40-block volume"
                    .into()
            ))
        );
    }

    #[test]
    fn a_descriptor_whose_copies_pass_the_end_twice_is_corrupt() {
        // An 8-block log holds indexes 1..=7; eight copies after index 7
        // would wrap past the end twice.
        let (mut disk, j) = journal(8, 5, 7);
        put(&mut disk, &j, 7, &descriptor(5, &[40; 8]));
        assert_eq!(
            scan(&disk, &j, 64),
            Err(Error::CorruptImage(
                "journal descriptor at journal block 7 tags 8 blocks, which run past the end \
                 of the 8-block log more than once"
                    .into()
            ))
        );
    }

    #[test]
    fn a_log_that_comes_around_the_ring_is_corrupt() {
        // One descriptor of six copies fills the 7-block log and leads back
        // to itself with the same sequence.
        let (mut disk, j) = journal(8, 5, 1);
        put(&mut disk, &j, 1, &descriptor(5, &[40; 6]));
        assert_eq!(
            scan(&disk, &j, 64),
            Err(Error::CorruptImage(
                "the journal log from journal block 1 runs more than once around the ring".into()
            ))
        );
    }

    #[test]
    fn replay_writes_committed_copies_home_and_empties_the_journal() {
        let (mut disk, j) = journal(16, 5, 1);
        disk.write(
            SUPERBLOCK_OFFSET + INCOMPAT_OFFSET,
            &0x0006u32.to_le_bytes(),
        );
        let tags = [
            Tag {
                block: 40,
                flags: 0,
            },
            Tag {
                block: 41,
                flags: TAG_ESCAPE,
            },
        ];
        put(&mut disk, &j, 1, &encode_descriptor(5, &[7; 16], &tags));
        put(&mut disk, &j, 2, &[0xA1; 1024]);
        put(&mut disk, &j, 3, &[0xB1; 1024]);
        put(&mut disk, &j, 4, &encode_commit(5, 1));
        put(&mut disk, &j, 5, &descriptor(6, &[42]));
        put(&mut disk, &j, 6, &[0xC1; 1024]);
        let found = scan(&disk, &j, 64).unwrap();
        disk.begin_op("recover");
        let next = replay(&mut disk, &j, &found).unwrap();
        let rec = disk.end_op();
        assert_eq!(next, 7);
        assert_eq!(disk.read(40 * BS, BS), &[0xA1; 1024][..]);
        let mut escaped = vec![0xB1; 1024];
        escaped[..4].copy_from_slice(&[0xC0, 0x3B, 0x39, 0x98]);
        assert_eq!(disk.read(41 * BS, BS), &escaped[..]);
        assert!(disk.read(42 * BS, BS).iter().all(|&b| b == 0));
        assert_eq!(disk.read(20 * BS + 0x18, 8), &[0, 0, 0, 7, 0, 0, 0, 0][..]);
        assert_eq!(
            disk.read(SUPERBLOCK_OFFSET + INCOMPAT_OFFSET, 4),
            &[2, 0, 0, 0][..]
        );
        let texts: Vec<String> = rec.events.iter().map(|e| e.to_string()).collect();
        assert_eq!(
            texts,
            vec![
                "scanned the journal from block 1, sequence 5: 1 committed transaction, 2 tagged blocks",
                "replayed block 40 from journal block 2 (transaction 5)",
                "replayed block 41 from journal block 3 (transaction 5)",
                "discarded uncommitted transaction 6 (1 tagged block)",
                "journal emptied; next transaction 7",
                "cleared needs_recovery in the superblock",
            ]
        );
        let changes: Vec<(usize, usize)> = rec
            .changes
            .iter()
            .map(|c| (c.offset, c.after.len()))
            .collect();
        assert_eq!(
            changes,
            vec![
                (40 * BS, BS),
                (41 * BS, BS),
                (20 * BS + 0x18, 8),
                (SUPERBLOCK_OFFSET + INCOMPAT_OFFSET, 4),
            ]
        );
        assert_eq!(rec.events[3].region(), Some(25 * BS..26 * BS));
    }

    #[test]
    fn the_sequence_wraps_past_u32_max_instead_of_panicking() {
        // Transaction u32::MAX commits, so the walker's own `expected` must
        // wrap to 0 to find the second transaction; `replay` then wraps
        // `s_sequence` past u32::MAX too.
        let (mut disk, j) = journal(16, u32::MAX, 1);
        put(&mut disk, &j, 1, &descriptor(u32::MAX, &[40]));
        put(&mut disk, &j, 3, &encode_commit(u32::MAX, 1));
        put(&mut disk, &j, 4, &descriptor(0, &[41]));
        put(&mut disk, &j, 6, &encode_commit(0, 1));
        let found = scan(&disk, &j, 64).unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].tid, u32::MAX);
        assert!(found[0].committed);
        assert_eq!(found[1].tid, 0);
        assert!(found[1].committed);
        disk.begin_op("recover");
        let next = replay(&mut disk, &j, &found).unwrap();
        let _ = disk.end_op();
        assert_eq!(next, 2);
        assert_eq!(disk.read(20 * BS + 0x18, 8), &[0, 0, 0, 2, 0, 0, 0, 0][..]);
    }
}
