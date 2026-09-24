//! The journal as the explorer sees it (spec section 7): what every journal
//! block holds, whether it belongs to the live transaction or is a stale
//! leftover of an earlier one, and the annotations `annotate_sector` shows
//! for a journal data block.

use super::recovery::{parse_block, LogBlock, LogEntry, LogWalk};
use super::state::JournalState;
use super::{
    be32_at, COMMIT_SEC_OFFSET, HEADER_LEN, TAG_BYTES, TAG_DELETED, TAG_ESCAPE, TAG_LAST,
    TAG_SAME_UUID,
};
use crate::fs::{note, uuid};
use crate::superblock::BLOCK_SIZE;
use fs_core::{Annotation, DateTime, Disk};

const BS: usize = BLOCK_SIZE as usize;

/// What one journal block holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalBlockKind {
    /// Journal index 0.
    Superblock,
    Descriptor,
    /// The logged copy of home block `home`; `escaped` when its tag sets
    /// `ESCAPE` (the copy's first four bytes were the magic and are zero).
    Copy {
        home: u32,
        escaped: bool,
    },
    Commit,
    Revoke,
    /// No journal header and no descriptor claims it.
    Unused,
}

/// One journal index: its physical block, what it holds, the transaction it
/// belongs to, and whether that transaction is over (`stale`: a leftover
/// the journal no longer needs) or is the live one `s_start` names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JournalBlock {
    pub index: u32,
    pub block: u32,
    pub kind: JournalBlockKind,
    pub tid: Option<u32>,
    pub stale: bool,
}

/// A journal index's classification while the walk runs.
type Slot = Option<(JournalBlockKind, Option<u32>, bool)>;

/// The classification in progress: one slot per journal index.
struct Walk<'a> {
    disk: &'a Disk,
    journal: &'a JournalState,
    slots: Vec<Slot>,
}

impl Walk<'_> {
    fn set(&mut self, index: u32, kind: JournalBlockKind, tid: Option<u32>, stale: bool) {
        self.slots[index as usize] = Some((kind, tid, stale));
    }

    fn taken(&self, index: u32) -> bool {
        self.slots[index as usize].is_some()
    }

    /// Mark a log block: a descriptor also claims the copies its tags name,
    /// as far as they are not taken yet. Returns `false` when a
    /// descriptor's tags do not parse or one of its copies was taken.
    fn mark(&mut self, entry: LogEntry, stale: bool) -> bool {
        let (index, tid) = (entry.index, Some(entry.sequence));
        match entry.block {
            LogBlock::Descriptor(tags) => {
                self.set(index, JournalBlockKind::Descriptor, tid, stale);
                let Ok(tags) = tags else {
                    return false;
                };
                for (at, tag) in tags {
                    if self.taken(at) {
                        return false;
                    }
                    let kind = JournalBlockKind::Copy {
                        home: tag.block,
                        escaped: tag.flags & TAG_ESCAPE != 0,
                    };
                    self.set(at, kind, tid, stale);
                }
            }
            LogBlock::Commit => self.set(index, JournalBlockKind::Commit, tid, stale),
            LogBlock::Revoke => self.set(index, JournalBlockKind::Revoke, tid, stale),
        }
        true
    }

    /// The live transactions from `start`: recovery's log walk (spec
    /// section 6), except that a revoke block is part of the live
    /// transaction instead of an error. The parse ends where the walk ends,
    /// at a block already classified, or at a descriptor `mark` refuses.
    fn live(&mut self, start: u32, sequence: u32) {
        for entry in LogWalk::new(self.disk, self.journal, start, sequence) {
            if self.taken(entry.index) || !self.mark(entry, false) {
                return;
            }
        }
    }

    /// Every other log block, in index order: stale descriptors (claiming
    /// their copies), commits, and revoke blocks. A header of any other
    /// type (a superblock inside the log) stays unused.
    fn stale(&mut self) {
        for index in self.journal.first..self.journal.maxlen {
            if self.taken(index) {
                continue;
            }
            if let Some(entry) = parse_block(self.disk, self.journal, index) {
                self.mark(entry, true);
            }
        }
    }
}

/// Classify every journal index in order (spec section 7): index 0 is the
/// superblock; the live transaction from `s_start`, if any, comes first
/// (`stale: false`); then every other log block is a stale leftover, a
/// stale descriptor claiming the following blocks that are not live or
/// claimed yet as its copies; the rest is unused.
pub(crate) fn classify(disk: &Disk, journal: &JournalState) -> Vec<JournalBlock> {
    let mut walk = Walk {
        disk,
        journal,
        slots: vec![None; journal.maxlen as usize],
    };
    walk.set(0, JournalBlockKind::Superblock, None, false);
    if let Ok(jsb) = journal.superblock(disk) {
        walk.live(jsb.start, jsb.sequence);
    }
    walk.stale();
    walk.slots
        .into_iter()
        .enumerate()
        .map(|(index, slot)| {
            let (kind, tid, stale) = slot.unwrap_or((JournalBlockKind::Unused, None, false));
            JournalBlock {
                index: index as u32,
                block: journal.blocks[index],
                kind,
                tid,
                stale,
            }
        })
        .collect()
}

/// The journal superblock fields `annotate` names: offset, length, label,
/// and whether the value shows in hex. `users` is `s_nr_users`.
const SUPERBLOCK_FIELDS: [(usize, usize, &str, bool); 9] = [
    (0x00, 4, "magic", true),
    (0x04, 4, "block type", false),
    (0x0C, 4, "block size", false),
    (0x10, 4, "maxlen", false),
    (0x14, 4, "first", false),
    (0x18, 4, "sequence", false),
    (0x1C, 4, "start", false),
    (0x30, 16, "uuid", true),
    (0x40, 4, "users", false),
];

/// The tag flags by name, in bit order.
const TAG_FLAG_NAMES: [(u16, &str); 4] = [
    (TAG_ESCAPE, "escape"),
    (TAG_SAME_UUID, "same_uuid"),
    (TAG_DELETED, "deleted"),
    (TAG_LAST, "last"),
];

/// A journal block's annotations (spec section 7), `bytes` being its 1024
/// bytes and `entry` what `classify` says it holds.
pub(crate) fn annotate(bytes: &[u8], entry: &JournalBlock) -> Vec<Annotation> {
    match entry.kind {
        JournalBlockKind::Superblock => SUPERBLOCK_FIELDS
            .iter()
            .map(|&(offset, len, label, hex)| {
                let field = &bytes[offset..offset + len];
                let value = match (len, hex) {
                    (16, _) => uuid(field),
                    (_, true) => format!("0x{:08X}", be32_at(field, 0)),
                    (_, false) => be32_at(field, 0).to_string(),
                };
                note(offset..offset + len, label, value)
            })
            .collect(),
        JournalBlockKind::Descriptor => {
            let mut notes = header_notes(bytes);
            notes.extend(tag_notes(bytes));
            notes
        }
        JournalBlockKind::Commit => {
            let mut notes = header_notes(bytes);
            let at = COMMIT_SEC_OFFSET;
            let mut sec = [0u8; 8];
            sec.copy_from_slice(&bytes[at..at + 8]);
            let secs = u64::from_be_bytes(sec);
            let date = DateTime::from_unix_seconds(i64::try_from(secs).unwrap_or(i64::MAX));
            notes.push(note(at..at + 8, "commit time", format!("{secs} ({date})")));
            notes
        }
        JournalBlockKind::Revoke => header_notes(bytes),
        JournalBlockKind::Copy { home, escaped } => {
            let tid = entry.tid.unwrap_or_default();
            let mut value = format!("copy of block {home} for transaction {tid}");
            if entry.stale {
                value.push_str(" (stale)");
            }
            if escaped {
                value.push_str(" (escaped)");
            }
            vec![note(0..BS, "journal copy", value)]
        }
        JournalBlockKind::Unused => vec![note(0..BS, "journal", "unused journal block")],
    }
}

/// The 12-byte header: magic (hex), block type, sequence.
fn header_notes(bytes: &[u8]) -> Vec<Annotation> {
    vec![
        note(0..4, "magic", format!("0x{:08X}", be32_at(bytes, 0))),
        note(4..8, "block type", be32_at(bytes, 4).to_string()),
        note(8..12, "sequence", be32_at(bytes, 8).to_string()),
    ]
}

/// A descriptor's tags as `tag k` (from 1) over their 8 bytes, each uuid
/// (after the first tag and any other tag without `SAME_UUID`) as `uuid`,
/// up to the tag with `LAST` or the end of the block.
fn tag_notes(bytes: &[u8]) -> Vec<Annotation> {
    let mut notes = Vec::new();
    let mut pos = HEADER_LEN;
    let mut k = 1;
    while pos + TAG_BYTES <= bytes.len() {
        let block = be32_at(bytes, pos);
        let flags = u16::from_be_bytes([bytes[pos + 6], bytes[pos + 7]]);
        let value = format!("block {block}, flags {}", flag_names(flags));
        notes.push(note(pos..pos + TAG_BYTES, format!("tag {k}"), value));
        pos += TAG_BYTES;
        if flags & TAG_SAME_UUID == 0 && pos + 16 <= bytes.len() {
            notes.push(note(pos..pos + 16, "uuid", uuid(&bytes[pos..pos + 16])));
            pos += 16;
        }
        if flags & TAG_LAST != 0 {
            break;
        }
        k += 1;
    }
    notes
}

/// The set flags by name joined with ` | ` (an unnamed bit in hex), or
/// `none`.
fn flag_names(flags: u16) -> String {
    let mut names: Vec<String> = TAG_FLAG_NAMES
        .iter()
        .filter(|&&(bit, _)| flags & bit != 0)
        .map(|&(_, name)| name.to_string())
        .collect();
    let other = flags & !(TAG_ESCAPE | TAG_SAME_UUID | TAG_DELETED | TAG_LAST);
    if other != 0 {
        names.push(format!("0x{other:04X}"));
    }
    if names.is_empty() {
        "none".to_string()
    } else {
        names.join(" | ")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::journal::{
        encode_commit, encode_descriptor, write_header, JournalMode, JournalSuperblock, Tag,
        BLOCKTYPE_DESCRIPTOR, BLOCKTYPE_REVOKE, BLOCKTYPE_SUPERBLOCK_V2,
    };
    use JournalBlockKind::{Commit, Descriptor, Revoke, Superblock, Unused};

    /// A 16-block journal whose index `i` is physical block `10 + i`, on a
    /// 64-block disk, with a fresh journal superblock.
    fn journal() -> (Disk, JournalState) {
        let mut disk = Disk::new(BS, 64);
        let j = JournalState {
            blocks: (10..26).collect(),
            maxlen: 16,
            first: 1,
            sequence: 1,
            head: 1,
            mode: JournalMode::Ordered,
            needs_recovery: false,
            armed: None,
        };
        let mut sb = [0u8; BS];
        JournalSuperblock::new(16, [7; 16]).encode(&mut sb);
        disk.write(j.offset(0), &sb);
        (disk, j)
    }

    /// Point `s_start` and `s_sequence` at a live transaction (0: empty).
    fn set_start(disk: &mut Disk, j: &JournalState, start: u32, sequence: u32) {
        let at = j.offset(0) + JournalSuperblock::SEQUENCE_OFFSET;
        disk.write(at, &sequence.to_be_bytes());
        disk.write(at + 4, &start.to_be_bytes());
    }

    /// Log transaction `tid` from `start`: a descriptor, one copy per home
    /// (the copy of home `h` is the byte `h` repeated), and a commit when
    /// `commit`. Returns the index after the last block written.
    fn log(
        disk: &mut Disk,
        j: &JournalState,
        start: u32,
        tid: u32,
        homes: &[u32],
        commit: bool,
    ) -> u32 {
        let tags: Vec<Tag> = homes.iter().map(|&block| Tag { block, flags: 0 }).collect();
        disk.write(j.offset(start), &encode_descriptor(tid, &[7; 16], &tags));
        let mut at = start;
        for &home in homes {
            at = j.wrap(at + 1);
            disk.write(j.offset(at), &[home as u8; BS]);
        }
        at = j.wrap(at + 1);
        if commit {
            disk.write(j.offset(at), &encode_commit(tid, 1_000));
            at = j.wrap(at + 1);
        }
        at
    }

    fn copy(home: u32) -> JournalBlockKind {
        JournalBlockKind::Copy {
            home,
            escaped: false,
        }
    }

    /// `(kind, tid, stale)` per index.
    fn summary(disk: &Disk, j: &JournalState) -> Vec<(JournalBlockKind, Option<u32>, bool)> {
        classify(disk, j)
            .into_iter()
            .map(|b| (b.kind, b.tid, b.stale))
            .collect()
    }

    fn unused() -> (JournalBlockKind, Option<u32>, bool) {
        (Unused, None, false)
    }

    #[test]
    fn a_fresh_journal_is_its_superblock_and_unused_blocks() {
        let (disk, j) = journal();
        let blocks = classify(&disk, &j);
        assert_eq!(blocks.len(), 16);
        assert_eq!(
            blocks[0],
            JournalBlock {
                index: 0,
                block: 10,
                kind: Superblock,
                tid: None,
                stale: false,
            }
        );
        for (i, b) in blocks.iter().enumerate().skip(1) {
            assert_eq!((b.index, b.block), (i as u32, 10 + i as u32));
            assert_eq!((b.kind, b.tid, b.stale), unused());
        }
    }

    #[test]
    fn a_finished_transaction_is_stale_and_the_live_one_is_not() {
        let (mut disk, j) = journal();
        log(&mut disk, &j, 1, 3, &[5, 6, 7], true);
        let mut want = vec![(Superblock, None, false)];
        want.push((Descriptor, Some(3), true));
        want.extend([5, 6, 7].map(|h| (copy(h), Some(3), true)));
        want.push((Commit, Some(3), true));
        want.extend(vec![unused(); 10]);
        assert_eq!(summary(&disk, &j), want);
        // The same blocks named by `s_start` are the live transaction.
        set_start(&mut disk, &j, 1, 3);
        let live: Vec<_> = want
            .iter()
            .map(|&(kind, tid, _)| (kind, tid, false))
            .collect();
        assert_eq!(summary(&disk, &j), live);
    }

    #[test]
    fn the_live_parse_follows_the_sequence_and_stops_at_a_mismatch() {
        let (mut disk, j) = journal();
        // Transaction 3 committed, 4 logged but not committed, then a
        // leftover descriptor of transaction 1.
        let next = log(&mut disk, &j, 1, 3, &[5], true);
        let next = log(&mut disk, &j, next, 4, &[6, 7], false);
        assert_eq!(next, 7);
        log(&mut disk, &j, next, 1, &[8], true);
        set_start(&mut disk, &j, 1, 3);
        let s = summary(&disk, &j);
        assert_eq!(
            s[1..4],
            [
                (Descriptor, Some(3), false),
                (copy(5), Some(3), false),
                (Commit, Some(3), false)
            ]
        );
        assert_eq!(
            s[4..7],
            [
                (Descriptor, Some(4), false),
                (copy(6), Some(4), false),
                (copy(7), Some(4), false)
            ]
        );
        assert_eq!(
            s[7..10],
            [
                (Descriptor, Some(1), true),
                (copy(8), Some(1), true),
                (Commit, Some(1), true)
            ]
        );
        assert!(s[10..].iter().all(|&e| e == unused()));
    }

    #[test]
    fn a_descriptor_near_the_end_claims_its_copies_across_the_wrap() {
        for live in [false, true] {
            let (mut disk, j) = journal();
            // Descriptor at 14, copies at 15, 1, 2, commit at 3.
            assert_eq!(log(&mut disk, &j, 14, 9, &[5, 6, 7], true), 4);
            if live {
                set_start(&mut disk, &j, 14, 9);
            }
            let stale = !live;
            let s = summary(&disk, &j);
            assert_eq!(s[14], (Descriptor, Some(9), stale), "live {live}");
            assert_eq!(s[15], (copy(5), Some(9), stale));
            assert_eq!(s[1], (copy(6), Some(9), stale));
            assert_eq!(s[2], (copy(7), Some(9), stale));
            assert_eq!(s[3], (Commit, Some(9), stale));
            assert!(s[4..14].iter().all(|&e| e == unused()));
        }
    }

    #[test]
    fn a_stale_descriptor_stops_claiming_at_the_live_transaction() {
        let (mut disk, j) = journal();
        // A leftover descriptor at 2 names four copies (3..=6) and its
        // commit is at 7, but the live transaction 5 has since been logged
        // at 4..=6.
        log(&mut disk, &j, 2, 4, &[20, 21, 22, 23], true);
        log(&mut disk, &j, 4, 5, &[30], true);
        set_start(&mut disk, &j, 4, 5);
        let s = summary(&disk, &j);
        assert_eq!(s[1], unused());
        assert_eq!(s[2], (Descriptor, Some(4), true));
        assert_eq!(s[3], (copy(20), Some(4), true));
        assert_eq!(s[4], (Descriptor, Some(5), false));
        assert_eq!(s[5], (copy(30), Some(5), false));
        assert_eq!(s[6], (Commit, Some(5), false));
        assert_eq!(s[7], (Commit, Some(4), true));
        assert!(s[8..].iter().all(|&e| e == unused()));
    }

    #[test]
    fn escape_flags_revoke_blocks_and_foreign_headers_are_reported() {
        let (mut disk, j) = journal();
        let tags = [Tag {
            block: 40,
            flags: TAG_ESCAPE,
        }];
        disk.write(j.offset(1), &encode_descriptor(2, &[7; 16], &tags));
        let mut revoke = [0u8; BS];
        write_header(&mut revoke, BLOCKTYPE_REVOKE, 2);
        disk.write(j.offset(3), &revoke);
        let mut foreign = [0u8; BS];
        write_header(&mut foreign, BLOCKTYPE_SUPERBLOCK_V2, 0);
        disk.write(j.offset(4), &foreign);
        let s = summary(&disk, &j);
        let escaped = JournalBlockKind::Copy {
            home: 40,
            escaped: true,
        };
        assert_eq!(s[2], (escaped, Some(2), true));
        assert_eq!(s[3], (Revoke, Some(2), true));
        assert_eq!(s[4], unused());
        // Live, a revoke block is part of the transaction.
        set_start(&mut disk, &j, 1, 2);
        let s = summary(&disk, &j);
        assert_eq!(
            s[1..4],
            [
                (Descriptor, Some(2), false),
                (escaped, Some(2), false),
                (Revoke, Some(2), false)
            ]
        );
    }

    #[test]
    fn a_broken_journal_superblock_leaves_only_stale_blocks() {
        let (mut disk, j) = journal();
        log(&mut disk, &j, 1, 3, &[5], true);
        set_start(&mut disk, &j, 1, 3);
        disk.write(j.offset(0), &[0; 4]);
        let s = summary(&disk, &j);
        assert_eq!(s[0], (Superblock, None, false));
        assert_eq!(s[1], (Descriptor, Some(3), true));
        // An `s_start` outside the log is ignored too.
        let (mut disk, j) = journal();
        log(&mut disk, &j, 1, 3, &[5], true);
        set_start(&mut disk, &j, 16, 3);
        assert_eq!(summary(&disk, &j)[1], (Descriptor, Some(3), true));
    }

    #[test]
    fn every_tag_without_same_uuid_is_followed_by_a_uuid_and_every_flag_is_named() {
        // A foreign descriptor: tag 1 (deleted) and tag 2 (an unknown bit
        // and last) both lack SAME_UUID, so a uuid follows each.
        let mut bytes = [0u8; BS];
        write_header(&mut bytes, BLOCKTYPE_DESCRIPTOR, 6);
        bytes[12..16].copy_from_slice(&7u32.to_be_bytes());
        bytes[18..20].copy_from_slice(&TAG_DELETED.to_be_bytes());
        bytes[20..36].fill(0xAB);
        bytes[36..40].copy_from_slice(&8u32.to_be_bytes());
        bytes[42..44].copy_from_slice(&(0x10 | TAG_LAST).to_be_bytes());
        bytes[44..60].fill(0xCD);
        let entry = JournalBlock {
            index: 1,
            block: 11,
            kind: Descriptor,
            tid: Some(6),
            stale: true,
        };
        let notes: Vec<(String, String, std::ops::Range<usize>)> = annotate(&bytes, &entry)
            .into_iter()
            .map(|a| (a.label, a.value, a.range))
            .collect();
        let first = "abababab-abab-abab-abab-abababababab";
        let second = "cdcdcdcd-cdcd-cdcd-cdcd-cdcdcdcdcdcd";
        assert_eq!(
            notes[3..],
            [
                ("tag 1".into(), "block 7, flags deleted".into(), 12..20),
                ("uuid".into(), first.into(), 20..36),
                (
                    "tag 2".into(),
                    "block 8, flags last | 0x0010".into(),
                    36..44
                ),
                ("uuid".into(), second.into(), 44..60),
            ]
        );
        assert_eq!(flag_names(0), "none");
        assert_eq!(flag_names(0x000F), "escape | same_uuid | deleted | last");
    }
}
