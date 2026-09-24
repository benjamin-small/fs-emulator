# ext3 journal design (slice 3 of the ext work)

The ext crate gains a JBD2 journal on the reserved inode 8, so an ext volume
can be formatted as ext3, every mutation runs as one journal transaction, a
crash can be armed at a chosen phase of the next transaction, and a recover
operation replays or discards what the journal holds the way a mount does.
Images stay e2fsprogs-clean at every rest state, `debugfs logdump` reads the
transactions, and `e2fsck -fy` recovers a crashed image to exactly the bytes
our own recovery produces. This slice is Rust and wasm only; the explorer
learns ext in slice 4.

Binding for the plan and the implementers. Where a later plan or task text
disagrees with this spec, the spec wins. The slice-2 spec
(`docs/superpowers/specs/2026-09-23-ext2-design.md`) stays binding for
everything it covers; this spec only adds to it and amends the points named
in section 12.

## Decisions (2026-09-24, with the user)

1. **Journal mode: both, switchable at format.** Ordered (metadata only in
   the journal, file data written home first) and full data journaling. The
   choice lives on disk in the superblock's default mount options, the field
   `tune2fs -o journal_data` writes, so it is not emulator-only state.
2. **Crash model: an armed crash point plus a recover operation.** The caller
   arms a phase; the next mutation stops exactly there and succeeds as a
   truncated operation. A separate `recover` operation replays or discards
   like mount does. `e2fsck -fy` on a copy is the oracle.
3. **ext2 stays first class.** `formatExt2` is unchanged; `formatExt3` adds
   the journal. One crate, one `ExtFs`; `fs_type()` says `"ext3"` whenever
   `has_journal` is set.
4. **Mechanism: the post-pass transaction builder.** The ext2 mutation body
   runs unchanged; afterwards the journal layer classifies what it changed,
   restores the disk, and re-emits the honest write sequence as the recorded
   operation. Slice-2 code is untouched apart from `run_mutation`.

## Outcome

- `ExtFs::format` with journal options lays down an ext3 volume that
  `e2fsck -fn` calls clean and `dumpe2fs` describes with the journal inode,
  size, sequence, start, and default mount options this spec states.
- Every mutation on an ext3 volume is one transaction whose byte record shows
  the real order: needs-recovery flag, data home (ordered mode), journal
  superblock, descriptor, block copies, commit, checkpoint, journal emptied,
  flag cleared. After every mutation the image is in the state of a cleanly
  unmounted volume.
- `arm_crash(phase)` makes the next mutation stop at that phase; the volume
  then refuses further mutations with `NeedsRecovery` until `recover()`,
  whose result equals e2fsck's replay byte for byte outside the superblock
  copies.
- The wasm `Volume` exposes `formatExt3`, `armCrash`, `disarmCrash`,
  `needsRecovery`, `recover`, `journalInfo`, and `journalBlocks`; the layout
  carries a `journal` region; `annotate_sector` explains journal blocks.
- ext2 behaviour, every slice-2 test, and the explorer are unchanged.

## Non-goals

- Revoke records, multiple in-flight transactions, lazy checkpointing, and
  journal wrap across a transaction boundary that is still live. One
  transaction is in the journal at a time; a foreign image with several
  committed transactions is replayed (section 6), one with a revoke block is
  `Unsupported`.
- Journal checksums (v2 or v3), 64-bit tags, async commit, fast commit, an
  external journal device, and writeback mode.
- Modelling mounts: mount counts, mount times, the orphan list, and the
  superblock state field are left as slice 2 leaves them.
- Splitting one mutation into several transactions. A mutation whose
  transaction would exceed the journal's maximum fails (section 5).
- Anything in `web/ui` beyond the two type-union additions in section 9.

## Vocabulary

The repository already calls its per-operation byte record "the journal"
(`OpRecord`, `ByteChange`). In this spec and in the code it adds, that thing
is **the operation record**; the JBD2 structure on inode 8 is **the ext
journal**, or just "the journal" inside the `ext::journal` module. A
**transaction** is one mutation's set of journaled blocks with its
descriptor and commit blocks; **tid** is its sequence number; **tagged
blocks** are the home blocks a descriptor lists; **checkpoint** is copying
tagged blocks home; **head** is the next journal block a transaction starts
at. Journal blocks are addressed by their **journal index** (0 for the
journal superblock, 1..maxlen for the log) and map to **physical blocks**
through inode 8's block map.

## 1. Crate and core

- New directory module `crates/ext/src/journal/`: `mod.rs` (constants,
  `JournalMode`, `JournalOptions`, `CrashPhase`, `JournalSuperblock`, the
  descriptor and commit codecs, escaping, the ring), `state.rs`
  (`JournalState`, `open`, the format-time writer, `JournalInfo`), `txn.rs`
  (the transaction builder), `recovery.rs` (scan and replay), `inspect.rs`
  (`JournalBlock`, `JournalBlockKind`, annotations). `crates/ext/src/fs.rs` gains the
  journal-aware `run_mutation`, `arm_crash`, `disarm_crash`,
  `crash_phase`, `needs_recovery`, `recover`, `journal_info`,
  `journal_blocks`. `events.rs` gains the journal events (section 5).
- `fs-core` gains two things and nothing else: `Error::NeedsRecovery`
  (Display `"needs recovery"`) and `RegionKind::Journal`. `FileSystem` is
  unchanged: the journal API is inherent on `ExtFs`, reached by wasm through
  its `ext()` accessor.
- `ExtFs` gains one field, `journal: Option<JournalState>`, `Some` when the
  superblock carries `has_journal`. `JournalState` holds the journal inode's
  block map resolved to `blocks: Vec<u32>` (physical block of each journal
  index, length `maxlen`), `maxlen`, `first` (1), `sequence` (the next tid),
  `head` (the next journal index to write), `mode`, `needs_recovery`, and
  `armed: Option<CrashPhase>`. It is cloned into the rollback snapshot beside
  `sb` and `gds` in `run_mutation` and restored with them on error.
- `fs_type()` returns `"ext3"` when `journal.is_some()`, else `"ext2"`.
- `Superblock` gains the fields `journal_uuid: [u8; 16]` (0xD0),
  `journal_inum: u32` (0xE0), `journal_dev: u32` (0xE4), `last_orphan: u32`
  (0xE8), `default_mount_opts: u32` (0x100), `jnl_backup_type: u8` (0xFD),
  and `jnl_blocks: [u32; 17]` (0x10C), decoded and encoded at those
  offsets. ext2 format writes them all as zero, so every slice-2 image and
  test is byte-identical.

## 2. On-disk format

All JBD2 fields are big-endian. Block size is the volume's 1024.

**Journal superblock** (journal index 0, block type 4, version 2):

| Offset | Field | Value at format |
|---|---|---|
| 0x00 | h_magic | 0xC03B3998 |
| 0x04 | h_blocktype | 4 |
| 0x08 | h_sequence | 0 |
| 0x0C | s_blocksize | 1024 |
| 0x10 | s_maxlen | journal blocks (section 3) |
| 0x14 | s_first | 1 |
| 0x18 | s_sequence | 1 |
| 0x1C | s_start | 0 |
| 0x20 | s_errno | 0 |
| 0x24 | s_feature_compat | 0 |
| 0x28 | s_feature_incompat | 0 |
| 0x2C | s_feature_ro_compat | 0 |
| 0x30 | s_uuid | the volume uuid |
| 0x40 | s_nr_users | 1 |
| 0x44 | s_dynsuper | 0 |
| 0x48 | s_max_transaction | 0 |
| 0x4C | s_max_trans_data | 0 |
| 0x50 | s_checksum_type, padding | 0 |
| 0x54 | s_num_fc_blks | 0 |
| 0x58 | s_head | 0 |
| 0x5C..0xFC | padding | 0 |
| 0xFC | s_checksum | 0 |
| 0x100 | s_users[48] | all zero (mke2fs leaves the user slots empty; only s_nr_users is 1) |

Constants: `JBD2_MAGIC = 0xC03B3998`, block types `DESCRIPTOR = 1`,
`COMMIT = 2`, `SUPERBLOCK_V1 = 3`, `SUPERBLOCK_V2 = 4`, `REVOKE = 5`; tag
flags `ESCAPE = 1`, `SAME_UUID = 2`, `DELETED = 4`, `LAST_TAG = 8`;
`TAG_BYTES = 8`; `TAGS_PER_DESCRIPTOR = 122`: the kernel starts a descriptor
with 1012 bytes of space, spends 8 per tag plus 16 for the uuid after the
first, and closes the block once fewer than 24 bytes remain, which is after
the 122nd tag.

**Descriptor block**: the 12-byte header (magic, type 1, sequence = tid),
then tags of 8 bytes: `t_blocknr` (be32 home block), `t_checksum` (be16,
0), `t_flags` (be16). The first tag of each descriptor block has no
`SAME_UUID` and is followed by the 16-byte volume uuid; every later tag in
that block sets `SAME_UUID`. The last tag of each descriptor block sets
`LAST_TAG`. A tag sets `ESCAPE` when the copied block's first four bytes
equal the journal magic; the copy then has those four bytes zeroed and the
home block keeps them. Unused bytes are zero. A transaction of `n` tagged
blocks uses `ceil(n / 122)` descriptor blocks, each immediately followed by
the copies it tags, in tag order.

**Commit block**: the header (type 2, sequence = tid), then `h_chksum_type`
(u8, 0), `h_chksum_size` (u8, 0), 2 bytes padding, `h_chksum[8]` (32 zero
bytes), `h_commit_sec` (be64, the emulator clock as Unix seconds, floored at
1 like every stamp), `h_commit_nsec` (be32, 0). The rest is zero.

**Copies** are the whole 1024-byte after-image of the home block, escaped as
above.

**ext superblock fields for ext3** (primary and every backup copy):
`s_feature_compat = 0x0004` (has_journal), `s_journal_inum = 8`,
`s_journal_uuid` all zero (internal journal), `s_journal_dev = 0`,
`s_default_mount_opts = 0x0040` for ordered or `0x0020` for data mode
(dumpe2fs prints `journal_data_ordered` / `journal_data`),
`s_jnl_backup_type = 1`, and `s_jnl_blocks[0..15]` = inode 8's `i_block`,
`[15]` = `i_size_high` (0), `[16]` = `i_size`. `s_feature_incompat` is
`0x0002` at rest and `0x0006` (filetype + needs_recovery) between the start
of a transaction and its end (section 5).

**Inode 8**: mode 0x8180 (regular, 0600), uid/gid 0, size = maxlen × 1024,
links 1, flags 0, the same timestamps as the other format-time inodes, and
`i_blocks` counting its data and indirect blocks in 512-byte units. Its
block map is written by the existing `map_file_blocks` path, so the data
blocks come first and the indirect blocks after them (slice-2 rule).

## 3. Format

`ExtFormatOptions` gains `journal: Option<JournalOptions>`, default `None`
(ext2). `JournalOptions { blocks: Option<u32>, mode: JournalMode }` with
`JournalMode::{Ordered, Data}`; `JournalOptions::default()` is `blocks:
None, mode: Ordered`.

Journal size when `blocks` is `None`, mke2fs's table for the volume's block
count: below 32,768 total blocks → 1024; below 262,144 → 4096; at 262,144
→ 8192. A volume below 2048 total blocks cannot carry a journal:
`InvalidGeometry("a journal needs at least 2048 blocks")`. An explicit
`blocks` must be at least 1024 (`InvalidGeometry("journal size must be at
least 1024 blocks")`) and at most half the free blocks of the freshly
formatted ext2 layout (`InvalidGeometry("journal size too big for the
volume")`), the two checks mke2fs makes.

Placement: after the ext2 layout (root, lost+found) is written, format
allocates inode 8 and maps `blocks` data blocks through
`map_file_blocks` with the goal group set to the group of block
`(total_blocks − first_data_block) / 2`, mke2fs's rule. On the default
16,384-block disk that is group 0, so the journal occupies blocks
82..1105 with its five indirect blocks at 1106..1110, and the first free
data block becomes 1111. Format then writes the journal superblock to
journal index 0 and zero-fills journal indexes 1..maxlen (they are fresh
allocations, already zero), writes the ext superblock fields of section 2
to every copy, updates the counters, and records the whole thing in the
single `format` operation as today. Counters after the default ext3 format:
free blocks 16,234 − 1029 = 15,205 (group 0: 8111 − 1029 = 7082, group 1:
8123), free inodes 1013 (inode 8 is reserved and already counted used by
slice 2's rule).

`JournalState` after format: `sequence 1`, `head 1`, `needs_recovery
false`, `armed None`.

## 4. Loading and validation

`Superblock::validate` learns the variants:

- `feature_compat` must be 0 or exactly `has_journal` (0x0004); any other
  bit is `Unsupported` as today.
- `feature_incompat` must be `filetype` optionally with `needs_recovery`
  (0x0004); `needs_recovery` without `has_journal` is
  `CorruptImage("needs_recovery is set but the volume has no journal")`.
- `feature_ro_compat` exactly `sparse_super`, as today.

When `has_journal` is set, `ExtFs::from_image` then opens the journal
(`journal::open(&disk, &sb, &geo)`), which checks in this order and returns
the first failure:

1. `s_journal_inum == 8`, else `Unsupported("journal inode {n} (only the
   reserved inode 8 is supported)")`; `s_journal_uuid` all zero and
   `s_journal_dev == 0`, else `Unsupported("external journal")`.
2. Inode 8 decodes as a regular file whose size is a whole number of
   blocks and whose every mapped block lies inside the volume, else
   `CorruptImage` naming the reason; it maps at least 1024 data blocks,
   else `Unsupported("journal of {n} blocks is below the 1024-block
   minimum")`.
3. Journal index 0 has the JBD2 magic, else `CorruptImage("journal
   superblock magic is 0x{:08X}")`; block type 4, else `Unsupported("journal
   superblock version {type}")` (version 1 included); `s_blocksize == 1024`;
   `s_maxlen == inode 8's data block count`; `s_first == 1`; else
   `CorruptImage` naming the field.
4. All three journal feature words zero, else `Unsupported("journal feature
   {name}")` naming the first set bit by its e2fsprogs name (`journal_64bit`,
   `journal_checksum_v2`, `journal_checksum_v3`, `journal_async_commit`,
   `journal_incompat_revoke`, `fast_commit`; unknown bits print in hex).
5. The journal mode bits of `s_default_mount_opts` (mask 0x0060): 0 or
   0x0040 → `Ordered`; 0x0020 → `Data`; 0x0060 → `Unsupported("writeback
   journaling")`.
6. `s_start` is 0 or inside `first..maxlen`, else `CorruptImage`.

`needs_recovery` is `feature_incompat & 0x0004 != 0` (the ext flag; a set
flag with `s_start == 0` still needs a `recover` to clear it). Loading
never replays. `sequence` = `s_sequence`; `head` = `s_first` (a fresh mount
restarts the log at its first block, as the kernel does without
`journal_cycle_record`); `armed None`.

The corruption gate (slice-2 section 6) extends to the journal: a raw
write touching the primary superblock, the primary descriptors, journal
index 0, inode 8's slot in the inode table, or any of the journal's
indirect blocks re-runs the parse above (which re-resolves the block map),
and a failure leaves the volume in the `CorruptImage` state with the same
message shape.

## 5. Transactions

Only the five path mutations journal (`create_file`, `write_file`,
`delete_file`, `create_dir`, `remove_dir`). `write_raw` and `format` never
do. On an ext2 volume `run_mutation` behaves exactly as slice 2 specifies.

### 5.1 The post-pass

On an ext3 volume `run_mutation` does, in order:

1. If `needs_recovery`, return `Err(Error::NeedsRecovery)`. Each of the
   five mutations makes this check right after the corruption gate and
   before any path, name, existence, or space check, so a crashed volume
   answers `NeedsRecovery` to every mutation, whatever its arguments.
2. Snapshot `sb`, `gds`, and `journal` for rollback. Open the operation
   (`disk.begin_op(name)`), run the body, and take the **draft** record with
   `disk.end_op()` without pushing it to history. On `Err`, restore every
   draft change in reverse (unrecorded), restore the snapshots, and return
   the error, exactly as `finish_op` would.
3. Classify. Compute `owners = block_owners()` on the post-body disk. Split
   every draft change at block boundaries. A block is **data** when
   `owners` lists it with `BlockRole::Data`; every other touched block is
   **metadata** (superblock and descriptor copies, bitmaps, inode table
   blocks, directory blocks, indirect blocks, and any other block the body
   wrote). In ordered mode the tagged set is the metadata blocks; in data
   mode it is metadata and data blocks together. Order the tagged set by
   ascending physical block number.
4. Capture the after-image (1024 bytes) of every tagged block from the
   post-body disk. If the primary superblock block is tagged, set
   `needs_recovery` in its after-image (the in-memory superblock of a
   mounted volume carries the flag; the copy must too, so the checkpoint
   and e2fsck's replay write the same bytes).
5. Size check: `n = tagged.len()`; `descriptors = ceil(n / 122)`; the
   transaction length is `descriptors + n + 1`. If `n > maxlen / 4`
   (jbd2's `j_max_transaction_buffers`, 256 on a 1024-block journal),
   restore the draft changes and snapshots and return
   `Err(Unsupported("transaction of {n} blocks exceeds the journal's
   maximum of {max}"))`. Nothing reaches history.
6. Restore every draft change in reverse (unrecorded). The disk is now at
   its pre-body state; `sb`, `gds` keep the body's values, which are the
   values the completed transaction produces.
7. Open the real operation with the same name, append the draft's events in
   their original order, then emit the sequence of 5.2, stopping at the
   armed phase if any. Close with `finish_op` (`Ok`), which pushes the
   record. After a crash, re-decode `sb` and `gds` from the disk and set
   `journal.needs_recovery = true`; `head` and `sequence` keep their
   pre-transaction values, since 5.2 updates them only after step 8.

### 5.2 The emitted sequence

`tid = journal.sequence`; `start = journal.head`; journal indexes are
assigned per block with the ring rule `wrap(x) = if x >= maxlen { x −
(maxlen − first) } else { x }`, so a transaction may straddle the end of
the log block by block, as jbd2 lays it out.

| Step | Bytes written | Event |
|---|---|---|
| 1 | primary superblock: `s_feature_incompat |= 0x0004` (4 bytes at 1024 + 0x60) | `RecoveryFlagSet { range }` |
| 2 | ordered mode only: every draft change that lies in a data block, replayed in draft order with its original offset and bytes | none (the draft's `DataWritten` events already describe them) |
| 3 | journal index 0: `s_sequence = tid`, `s_start = start` (8 bytes at 0x18) | `TransactionStarted { tid, journal_block: start, tagged: n, range }` |
| 4 | for each descriptor block in turn: the descriptor at the next index, then each of its tagged copies at the following indexes | `JournalBlockWritten { tid, kind: Descriptor, index, block, range }`, then per copy `JournalBlockWritten { tid, kind: Copy { home, escaped }, index, block, range }` |
| 5 | the commit block at the next index | `JournalBlockWritten { tid, kind: Commit, index, block, range }` |
| 6 | checkpoint: each tagged block's after-image written home as a whole block, in tag order | `Checkpointed { tid, home, from_index, range }` |
| 7 | journal index 0: `s_start = 0`, `s_sequence = tid + 1` | `JournalEmptied { next_sequence: tid + 1, range }` |
| 8 | primary superblock: `s_feature_incompat &= !0x0004` | `RecoveryFlagCleared { range }` |

`JournalBlockWritten.block` is the physical block; `index` the journal
index. Every write is a `Disk::write` under the open operation, so the
record's `ByteChange`s are the truth the timeline replays; whole-block
writes stay whole-block even where bytes repeat. After step 8:
`journal.head = wrap(index after the commit block)`, `journal.sequence =
tid + 1`. Step 6 writes the primary superblock's after-image with the flag
set; step 8 then clears it, a real change.

Event kinds (snake_case for the DTO `kind`): `recovery_flag_set`,
`transaction_started`, `journal_block_written`, `checkpointed`,
`journal_emptied`, `recovery_flag_cleared`, `crashed`, `recovery_scanned`,
`replayed`, `transaction_discarded`. Display strings, one line each, e.g.
`wrote descriptor for transaction 3 at journal block 17 (block 98)`,
`copied block 5 into journal block 18 (block 99)`, `committed transaction
3 at journal block 21`, `checkpointed block 5 from journal block 18`,
`journal emptied; next transaction 4`, `crashed after commit`. The exact
strings are pinned by the plan's tests.

### 5.3 What the two modes share and where they differ

| | Ordered | Data |
|---|---|---|
| Data blocks | written home at step 2, never tagged | tagged, copied at step 4, written home at step 6 |
| Tagged set | metadata only | metadata and data |
| Crash before commit | data already home in blocks the on-disk bitmap calls free; recovery discards the transaction and the blocks stay free (orphaned data, no leak into any file) | nothing reached home; recovery discards |
| Crash after commit | recovery replays metadata; the file is complete | recovery replays metadata and data |

## 6. Crash and recovery

`CrashPhase::{BeforeCommit, AfterCommit, DuringCheckpoint}`; Display
`"before commit"`, `"after commit"`, `"during checkpoint"`; DTO strings
`before_commit`, `after_commit`, `during_checkpoint`.

`arm_crash(phase)` sets `journal.armed`; `disarm_crash()` clears it;
`crash_phase()` reads it. On an ext2 volume `arm_crash` returns
`Err(Unsupported("this volume has no journal"))`. Arming while
`needs_recovery` is allowed and has no effect until after `recover`.

When armed, the emission of 5.2 stops: `BeforeCommit` after the last copy
of step 4 (the commit block is never written); `AfterCommit` after step 5;
`DuringCheckpoint` after the first home write of step 6 (a transaction that
tags no blocks cannot happen: the inode table block is always tagged). The
record ends with `Crashed { phase, range }` where `range` is the last
change's range, the operation name gains the suffix ` (crashed before
commit)` / ` (crashed after commit)` / ` (crashed during checkpoint)`, the
armed phase is cleared, and the operation returns `Ok(record)`.

While `needs_recovery`: the five mutations return `Err(NeedsRecovery)`;
`read_file`, `list_dir`, `stat`, `lookup`, `dir_entries`, `block_owners`,
`layout`, `annotate_sector`, and `write_raw` work on the raw on-disk state
(a learner sees the orphaned data or the half-checkpointed metadata);
`corruption()` is unaffected.

`recover()` is a recorded operation named `recover`. It never crashes and
is not subject to `arm_crash`. Steps:

1. If `needs_recovery` is false: push a record with no changes and the
   single event `RecoveryScanned { start: 0, sequence, committed: 0, tagged:
   0, range: 0..0 }` and return it (the terminal can say "journal is clean").
2. Scan (jbd2's `PASS_SCAN`): `pos = s_start`, `expected = s_sequence`. If
   `s_start == 0` the scan finds nothing. Otherwise loop: read journal index
   `pos`; stop when the magic is absent, the block type is not 1, 2, or 5,
   or the header sequence is not `expected`. Type 1: parse the tags
   (stopping at `LAST_TAG`; the uuid follows the first tag and any tag
   without `SAME_UUID`), remember them for `expected`, `pos = wrap(pos + 1 +
   tags)`. Type 2: the transaction `expected` is committed; `expected += 1`,
   `pos = wrap(pos + 1)`. Type 5: return `Err(Unsupported("journal revoke
   records"))` with nothing changed. A descriptor for a sequence that never
   reaches a commit is discarded. Record `RecoveryScanned { start,
   sequence: s_sequence, committed, tagged: total tagged blocks of committed
   transactions, range: 0x18..0x20 of journal index 0 }`.
3. Replay (`PASS_REPLAY`): for each committed transaction in sequence order,
   for each tag in order, write the copy home as a whole block, restoring
   the magic in the first four bytes when `ESCAPE` is set:
   `Replayed { tid, home, from_index, range }`. A discarded descriptor
   records `TransactionDiscarded { tid, tagged }` and writes nothing.
4. Journal index 0: `s_start = 0`, `s_sequence = expected + 1` (jbd2's
   `++info.end_transaction`, so after replaying tid the next sequence is
   tid + 2, and after discarding it is tid + 1; this is what e2fsck writes):
   `JournalEmptied { next_sequence, range }`.
5. Primary superblock: clear `needs_recovery`: `RecoveryFlagCleared`.
6. Re-decode `sb` and `gds` from the disk; `journal.sequence =
   next_sequence`, `journal.head = first`, `journal.needs_recovery = false`.

A tag whose home block lies outside the volume, or a descriptor whose
copies would run past the log more than once, makes `recover` return
`Err(CorruptImage(...))` with nothing changed (the checks happen in the scan
before any write).

## 7. Inspection surface for slice 4

- `journal_info() -> Option<JournalInfo>`: `None` on ext2;
  `JournalInfo { inode: 8, maxlen, first_block (physical block of journal
  index 0), sequence, start (from disk), head, mode, needs_recovery,
  max_transaction (maxlen / 4) }`.
- `journal_blocks() -> Vec<JournalBlock>`, one entry per journal index in
  order: `JournalBlock { index, block (physical), kind, tid: Option<u32>,
  stale: bool }` with `JournalBlockKind::{Superblock, Descriptor, Copy {
  home, escaped }, Commit, Revoke, Unused}`. Classification: if
  `s_start != 0`, first parse the live transaction from `s_start` as the
  scan of section 6 does and mark its blocks (`stale: false`); then walk
  indexes 1..maxlen in order, skipping live blocks, parsing any header with
  the magic (a descriptor claims the following blocks as its copies up to
  the first index already classified, live or stale); everything claimed
  this way is `stale: true`; blocks without a header and unclaimed are
  `Unused`; a superblock-type header inside the log is `Unused`. Index 0 is
  `Superblock`. Unlike `recover`, this classification never fails: a revoke
  block with the expected sequence is reported as a live `Revoke`, and the
  live parse simply ends at the first block that does not fit. The live
  parse and `recover`'s scan share one log walker; only their treatment of
  revoke blocks differs.
- `annotate_sector` for a sector inside a journal block (data blocks of
  inode 8): the superblock's fields by name and value (magic, block type,
  block size, maxlen, first, sequence, start, uuid, users); a descriptor's
  header (magic, type, sequence) and each tag (`tag k`: `block N, flags
  ...` naming set flags); a commit block's header and commit time; a copy:
  one annotation over the whole block, `copy of block N for transaction T`
  with ` (stale)` when stale, plus ` (escaped)` when escaped; an unused
  block: one annotation `unused journal block`. Journal indirect blocks
  keep slice 2's indirect-block annotations.
- `layout()`: each maximal run of consecutive physical blocks among the
  journal's data blocks becomes a `Region { name: "journal", kind:
  RegionKind::Journal }` (`"journal (part k)"` when there is more than one
  run, foreign images only), carved out of the data region it lies in, so
  the group's data region splits around it. Journal indirect blocks stay in
  the data region.
- `block_owners()`: the journal's data and indirect blocks are listed with
  `inode: 8`, `path: "<journal>"`, and roles `BlockRole::Journal` (new) for
  the data blocks and `Indirect` for the indirect blocks. Nothing else in
  the map changes.
- `dir_entries`, `lookup`, `list_dir`, `stat`, `read_file` never expose
  inode 8 (it has no directory entry).

## 8. Wasm

- `formatExt3(options)` takes `Ext3FormatOptions`, the four ext2 keys plus
  `journalBlocks?: number` and `journalMode?: "ordered" | "data"` (default
  ordered); any other key, a non-integer `journalBlocks`, or another mode
  string is `BadArgument`. `formatExt2` keeps `ExtFormatOptions` and
  rejects the two journal keys (`deny_unknown_fields`). Both go through
  `ExtFs::format`.
- `fsType()` returns `"ext3"` for a journaled volume; `fromImage` detection
  is unchanged (the ext magic covers both).
- `armCrash(phase: string)`, `disarmCrash()`, `crashPhase(): string |
  undefined`, `needsRecovery(): boolean`, `recover(): OpRecord`,
  `journalInfo(): JournalInfo | undefined`, `journalBlocks():
  JournalBlock[]`, all ext-only (`NotExt` on FAT). `armCrash` with an
  unknown phase is `BadArgument`; on ext2 it is `Unsupported`.
- DTOs (camelCase): `JournalInfo { inode, maxlen, firstBlock, sequence,
  start, head, mode, needsRecovery, maxTransaction }`; `JournalBlock {
  index, block, kind, tid?, home?, escaped?, stale }` with `kind` in
  `superblock | descriptor | copy | commit | revoke | unused`. Absent
  optionals serialise as `null`, like every `Option` in the existing DTOs
  (`tid` is null for the superblock and unused blocks; `home` and `escaped`
  are null except for copies).
- Error codes: `NeedsRecovery` joins the table. Journal events serialise
  like every `ExtEvent` through the existing `EventRecord`: `kind`, `text`
  (the Display string, which carries the tid, block, and index values),
  and `region` (the event's range; the clean-case `RecoveryScanned` has the
  empty region `0..0`). Event fields are not exposed as separate DTO
  members; slice 4 reads them from `text` or adds fields then.
- `crates/wasm/README.md`: the ext3 constructor, the crash and recovery
  methods, the journal DTOs, the `NeedsRecovery` code, and a "Loading an
  ext3 image" paragraph with the `mke2fs` recipe
  (`mke2fs -t ext3 -b 1024 -I 128 -O none,has_journal,filetype,sparse_super
  -J size=1`; the leading `none` matters, since mke2fs otherwise adds
  `ext_attr`, `resize_inode`, `dir_index`, and `large_file`, which the
  loader rejects) and the note that images from other tools must not carry journal
  checksums or 64-bit tags.

## 9. What the web layer sees

Two type additions and nothing more: the `RegionKind` union in
`web/ui/src/lib/wasm.ts` (or wherever the union lives) gains `"journal"`,
and the error-code union, if there is one, gains `"NeedsRecovery"`. The
FAT16 adapter's `colorForRegion` falls through to its default colour for
the new kind. `pnpm test` and `pnpm build` stay green with no other change;
the explorer still reports `no adapter for ext2` or `no adapter for ext3`
(the status names the family; its text is slice 4's to change).

## 10. Tests

### 10.1 Unit (`crates/ext/src/journal/`)

Journal superblock encode/decode round trip and the format-time bytes of
section 2; tag packing (uuid after the first tag, `SAME_UUID`, `LAST_TAG`,
122 tags per descriptor, 123 tagged blocks → 2 descriptors); the escape
rule on a block starting with the magic; the size table and both
`InvalidGeometry` messages; the ring `wrap`; commit-time bytes from a known
clock; classification on a synthetic draft (a change spanning a data block
and a directory block splits into one data portion and one tagged block).

### 10.2 Integration (`crates/ext/tests/ext3.rs`)

- Format: counters, inode 8's map (82..1105 data, 1106..1110 indirect on
  the default disk), the superblock fields in every copy, `s_jnl_blocks`
  equals `i_block`, `layout()` carries the journal region and the split
  data region, `block_owners` lists `<journal>`, `fs_type() == "ext3"`,
  `journal_info()`.
- Every slice-2 mutation test scenario re-run on ext3 in both modes with
  the record checked against 5.2: change order, whole-block copies and
  checkpoints, the flag bytes, the journal superblock bytes, the head
  advancing, and the sequence incrementing; the final image equals the ext2
  image of the same operations outside the journal's blocks and the
  section-2 superblock fields (a helper masks those and asserts equality).
- Ordered mode: data changes precede step 3 and are never tagged; data mode:
  data blocks are tagged and reach home only at step 6.
- The ring: enough small mutations to pass `maxlen`, checking the wrap and
  that `journal_blocks()` reports the stale transactions.
- The size limit: a data-mode create of 300 KiB fails with the section-5
  message, changes nothing, and leaves history untouched.
- Escaped copy: a data-mode file starting with `C0 3B 39 98` tags with
  `ESCAPE`, the copy's first four bytes are zero, and the home block keeps
  the magic after checkpoint and after recovery.
- Each crash phase in each mode for `create_file`, `write_file` (grow and
  shrink), `delete_file`, `create_dir`, `remove_dir`: the record stops where
  section 6 says, the name carries the suffix, `needs_recovery()` is true,
  a mutation returns `NeedsRecovery`, reads see the raw state (ordered
  before-commit: the file is absent from its directory and its blocks are
  free in the bitmap yet hold the data), and `recover()` leaves the image
  equal to the uncrashed image of the same operation (after-commit and
  during-checkpoint) or to the pre-operation image plus orphaned data
  (before-commit, ordered) or to the pre-operation image (before-commit,
  data), in each case ignoring the journal's blocks and the journal-related
  superblock fields, with the journal superblock's `s_sequence` following
  the section-6 rule.
- `recover()` on a clean volume: the empty record with one event.
- `from_image` of a crashed image: `needs_recovery()` true, no replay;
  after `recover()` equal to recovering in place.
- Validation: each `Unsupported` and `CorruptImage` reason of section 4
  from a planted image; a raw write that breaks the journal superblock
  trips the corruption gate and a repairing raw write clears it.
- ext2 volumes: `arm_crash` is `Unsupported`, `journal_info()` is `None`,
  `recover()` returns the empty record, and every slice-2 test passes
  unchanged.

### 10.3 e2fsprogs oracle (`crates/ext/tests/e2fsprogs.rs`)

Same skip-locally / hard-fail-under-`CI` rule and helpers as slice 2:

- `dumpe2fs` after `formatExt3` in each mode: `Filesystem features:
  has_journal filetype sparse_super`, `Default mount options:` exactly
  `journal_data_ordered` or `journal_data` (the volume sets no other
  default option; mke2fs would add `user_xattr acl`), `Journal inode: 8`,
  `Journal backup: inode blocks`, `Journal features: (none)`, `Total
  journal size: 1024k`, `Total journal blocks: 1024`, `Journal sequence:
  0x00000001`, `Journal start: 0`; dumpe2fs 1.47.4 also prints `Max
  transaction length: 1024` (it derives the value from `s_maxlen` when
  `s_max_transaction` is 0) and shows an 8192-block journal as `Total
  journal size: 8M`; `e2fsck -fn` clean.
- After a mixed sequence of mutations in each mode (the slice-2 scripted
  sequence): `e2fsck -fn` clean, `debugfs -R "ls -l"` and `stat` agree as in
  slice 2, and `debugfs -R "logdump -aO"` (`-a` prints the tag lines, `-O`
  walks stale transactions from journal block 1) lists a known
  transaction's tags (`FS block N logged at journal block M (flags 0x..)`)
  and commit in the order and at the indexes our record names; the test
  reloads the image first so the head restarts at block 1 and the walk
  reaches the transaction it then makes.
- For each crash phase and mode, on a `create_file` of 3 blocks: `debugfs
  -R logdump` on the crashed image shows the descriptor with our tags and,
  for the after-commit and during-checkpoint phases, the commit; then
  `e2fsck -fy` on a copy exits 0 or 1 and prints `recovering journal`, a
  following `e2fsck -fn` on that copy is clean, and the copy equals our
  `recover()` output byte for byte in every block except the superblock
  copies, which are compared field by field ignoring `s_wtime`, `s_mtime`,
  `s_lastcheck`, `s_state`, `s_mnt_count`, `s_kbytes_written`, and the
  reserved tail; the journal superblock must be byte-equal. A difference
  anywhere else is a finding, not a new exclusion.
- The escaped-block case through the same replay comparison.
- `crates/ext/tests/mount_linux.rs` gains an ext3 case behind `#[ignore]`:
  loop-mount a crashed after-commit image read-write, unmount, and compare
  to `recover()` with the same masking.

## 11. Documentation

`docs/ROADMAP.md`: slice 3 moves to "landed" with the surface above;
slice 4's bullet gains the journal panel's inputs (`journalInfo`,
`journalBlocks`, the `journal` region, the `<journal>` owner, the crash and
recover methods) and the deferred items this slice adds. `crates/ext`'s
module docs describe the journal module. The spec and plan for slice 3 are
referenced from the ROADMAP like slice 2's.

## 12. Amendments to the slice-2 spec

- `BlockRole` gains `Journal`; `block_owners` lists inode 8 (section 7).
- `Superblock` gains the fields of section 1; ext2 images are unchanged.
- `Superblock::validate` accepts the ext3 variants of section 4.
- `layout()` may split a data region around the journal (section 7).
- `run_mutation` snapshots `journal` beside `sb` and `gds`.
- The corruption gate covers the journal superblock (section 4).

## 13. Rulings

- The emulator has no background writeback, so every mutation runs its
  transaction to completion and leaves the on-disk state of a cleanly
  unmounted volume (`s_start == 0`, `needs_recovery` clear). Linux reaches
  that state only at unmount; the emulator reaches it after every
  operation. That is why `e2fsck -fn` is clean after every mutation and
  why the needs-recovery flag is set and cleared inside each operation.
- The primary superblock's journaled copy carries `needs_recovery` set
  (section 5.1 step 4) so the checkpoint and e2fsck's replay write
  identical bytes; the explicit clear at step 8 is then a real change.
- A crash is a successful operation with a truncated record, never an
  error: `finish_op` rolls back on `Err`, and the crash must leave its
  bytes on the disk.
- Head placement restarts at `s_first` on load and advances within a
  session; the emulator does not persist the head, matching a kernel
  without `journal_cycle_record`.
- Tagged blocks are ordered by ascending physical block number; jbd2's
  order depends on buffer dirtying order, which no oracle checks.
- One mutation is one transaction; a transaction that would exceed
  `maxlen / 4` tagged blocks fails rather than being split. On the default
  disk only data-mode writes above roughly 250 KiB reach the limit.
- `recover()` on a clean volume records an empty operation rather than
  failing, so a terminal `recover` can report a clean journal.
- Loading never replays, so a learner can load a crashed image, inspect it,
  and recover it deliberately.
- Format writes the journal outside any transaction, as mke2fs does.

## Verification

- `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets --
  -D warnings`, `cargo test --workspace` (the e2fsprogs tests run locally
  now that the tools are installed, and are a hard gate in CI),
  `wasm-pack test --node crates/wasm`, `wasm-pack build crates/wasm
  --target bundler`.
- `pnpm install --frozen-lockfile && pnpm test && pnpm build` in `web/ui`
  with no test expectation changed.
- By hand once, on a Linux host if available: the ignored ext3 mount test.
