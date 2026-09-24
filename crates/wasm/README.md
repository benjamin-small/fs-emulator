# fs-emulator-wasm

Browser bindings for the emulator. One class, `Volume`, exposes the
filesystem-agnostic operations plus family-specific inspection; results are
plain JS objects with TypeScript types in the generated `.d.ts`.

```ts
import { Volume } from "fs-emulator-wasm";

const vol = Volume.formatFat16(undefined);
const record = vol.createFile("/Hello world.txt", new TextEncoder().encode("hi"));
for (const e of record.events) console.log(e.kind, e.text);
console.log(vol.listDir("/"));

const ext = Volume.formatExt2({ label: "teach" });
console.log(ext.fsType(), ext.listDir("/")); // "ext2", [lost+found]

const ext3 = Volume.formatExt3({ journalMode: "data" });
ext3.armCrash("after_commit");
ext3.createFile("/a.txt", new TextEncoder().encode("hi")); // op "create_file /a.txt (crashed after commit)"
ext3.recover(); // replays the committed transaction
```

Errors are `Error` objects with a `code` property; see Error codes.

## Constructors

| Constructor | Options | Volume |
|---|---|---|
| `formatFat16(options)` | `FormatOptions` | FAT16 |
| `formatExt2(options)` | `ExtFormatOptions` | minimal ext2 revision 1, 1 KiB blocks |
| `formatExt3(options)` | `Ext3FormatOptions` | the same ext2 layout plus a JBD2 journal on inode 8 (`fsType()` is `"ext3"`) |
| `fromImage(bytes)` | | whatever family the image is; see Detection |

Every `format*` constructor accepts `undefined` (or `null`) for the defaults.

```ts
interface ExtFormatOptions {
  totalBlocks?: number;    // default 16384 (16 MiB, two block groups); 64 to 262144
  inodesPerGroup?: number; // default one inode per 16 KiB; a multiple of 8, 16 to 8192
  label?: string;          // at most 16 bytes of UTF-8; default empty
  uuid?: string;           // 32 hex digits, bare or hyphenated 8-4-4-4-12;
                           // default e2f5ee00-2026-4923-8000-000000000001
}

interface Ext3FormatOptions extends ExtFormatOptions {
  journalBlocks?: number;         // default mke2fs's size for the volume: 1024 below
                                  // 32,768 blocks, 4096 below 262,144, else 8192
  journalMode?: "ordered" | "data"; // default "ordered"
}
```

On an ext volume the disk's sector is the block, so `sectorSize()` is 1024
and sector N is block N. An unknown key (including `journalBlocks` or
`journalMode` given to `formatExt2`), a value of the wrong type (a
`journalBlocks` that is not a non-negative integer, a `journalMode` other than
the two strings), a malformed `uuid`, a label longer than 16 bytes, or a
volume over the 256 MiB limit (`totalBlocks * 1024`, like FAT's
`totalSectors * bytesPerSector`) throws `BadArgument`; options the format
itself rejects (fewer than 64 blocks, an `inodesPerGroup` that is not a
multiple of 8; for ext3 a volume below 2,048 blocks, a journal below 1,024
blocks, or one above half the volume's free blocks) throw `InvalidGeometry`.
On the default disk the journal occupies blocks 82 to 1105.

## Generic and filesystem-specific methods

`Volume` wraps the `FileSystem` trait from `fs-core`, so `createFile`,
`writeFile`, `readFile`, `deleteFile`, `createDir`, `removeDir`, `listDir`,
`stat`, `setNow`, `layout`, `annotateSector`, `historyLength`, `historyAt`,
`sectorSize`, `sectorCount`, `sector`, `writeRaw`, `readRaw`, `corruption`,
and `image` work on every filesystem the crate holds. `fsType()` names the
one inside (`"FAT16"`, `"ext2"`, or `"ext3"`).

`writeRaw(offset, bytes)` writes anywhere on the disk and returns an
`OpRecord` like every other mutating call, so the timeline and diff cover it;
a range past the end throws `OutOfBounds` and writes nothing. A write that
touches the family's on-disk metadata re-parses it (FAT: the first 512
bytes, the boot sector; ext: the primary superblock in block 1 and the
primary group descriptor table, and on ext3 also the journal superblock,
inode 8, and the journal's indirect blocks): if it still parses and fits the
disk, the new geometry is adopted (the disk is the truth); if not, the path
methods (`createFile`, `writeFile`, `readFile`, `deleteFile`, `createDir`,
`removeDir`, `listDir`, `stat`, FAT's `rawDirEntries`, and ext's `recover`)
throw `CorruptImage` with a message naming what no longer parses (`boot
sector no longer parses after a raw write` on FAT, `superblock or group
descriptors no longer parse after a raw write` on ext) until a later
`writeRaw` repairs it. On ext3 a broken journal superblock, inode 8, or
journal indirect block gives that same ext prefix, followed by `: ` and the
cause, even though the superblock and descriptors still parse.
`corruption()` returns that `CorruptImage` message while the gate is up, or
`null` while the volume is mounted. `layout`, `annotateSector`, `sector`,
`readRaw`, `image`, and `writeRaw` keep working meanwhile, and FAT's
`bootSector()`/`geometry()` return the last good values. `readRaw(offset,
len)` returns a copy of the bytes and throws `BadArgument` past the end. Both
take byte offsets, not sector numbers, as non-negative integers (wasm-bindgen
does not validate them).

`bootSector`, `geometry`, `fatEntries`, `clusterChain`, `rawDirEntries`,
`clusterOwners`, and `annotateSectorWith` are FAT-only and throw an error
with code `NotFat` (message `not a FAT volume`) on any other volume; FAT32
will reuse them. The methods below are ext-only and throw `NotExt` (message
`not an ext volume`) on any other volume; the rest of the ext inspection
(superblock, inodes, block ownership) arrives with the explorer's ext panels.
A UI should branch on `fsType()` before calling the specific ones. The plan
is in `docs/ROADMAP.md`.

- `blockGroupCount()`: how many block groups the volume has (2 on the
  default disk).
- `armCrash(phase)`: make the next path mutation stop at `"before_commit"`
  (after the last journal copy), `"after_commit"` (after the commit block),
  or `"during_checkpoint"` (after the first block written home); any other
  string throws `BadArgument`, and on ext2 it throws `Unsupported`
  (`unsupported: this volume has no journal`).
- `disarmCrash()`: clear the armed phase (a no-op when none is armed, and on
  ext2).
- `crashPhase()`: the armed phase as `armCrash` takes it, or `undefined`.
  Firing clears it. Both methods type the phase as `string`; its values are
  exactly `"before_commit"`, `"after_commit"`, and `"during_checkpoint"`.
- `needsRecovery()`: whether a crash left the journal for `recover()`;
  always `false` on ext2.
- `recover()`: replay the committed transactions and discard an uncommitted
  one, as a mount does, recorded as the operation `recover` and returned as
  its `OpRecord`. On a clean journal, and on ext2, the record has no changes
  and one `recovery_scanned` event (`journal is clean; nothing to replay`)
  whose `region` is `{ start: 0, end: 0 }`.
- `journalInfo()`: a `JournalInfo`, or `undefined` on ext2.
- `journalBlocks()`: one `JournalBlock` per journal index, in order; `[]` on
  ext2.

```ts
interface JournalInfo {
  inode: number;          // always 8
  maxlen: number;         // journal blocks, the journal superblock included
  firstBlock: number;     // physical block of journal index 0
  sequence: number;       // the tid the next transaction takes (while
                          // needsRecovery, the crashed transaction's tid)
  start: number;          // s_start on disk: 0 when the journal is empty
  head: number;           // the journal index the next transaction starts at
  mode: "ordered" | "data";
  needsRecovery: boolean;
  maxTransaction: number; // maxlen / 4, the most blocks one transaction may tag
}

interface JournalBlock {
  index: number;          // journal index (0 is the journal superblock)
  block: number;          // physical block
  kind: "superblock" | "descriptor" | "copy" | "commit" | "revoke" | "unused";
  tid: number | null;     // the transaction; null for superblock and unused
  home: number | null;    // copy only: the block it is a copy of
  escaped: boolean | null; // copy only: its first four bytes were the magic
  stale: boolean;         // a leftover of a finished transaction
}
```

Like every `Option` these DTOs carry, an absent `tid`, `home`, or `escaped`
is `null`, never `undefined` or a missing key. On an ext3 volume every path
mutation runs as one journal transaction, so its record shows the whole
write order: first the body's ext2 events (`inode_allocated`,
`data_written`, and the rest, whose regions name bytes that reach the disk
only with the data home or the checkpoint), then the journal's:
`recovery_flag_set`, the data home (ordered mode),
`transaction_started`, `journal_block_written` for the descriptor, each
copy, and the commit, `checkpointed` per block, `journal_emptied`, and
`recovery_flag_cleared`; a crashed record stops at the armed phase, ends with
a `crashed` event, and its `op` gains ` (crashed before commit)`,
` (crashed after commit)`, or ` (crashed during checkpoint)`. `recover()`
records `recovery_scanned`, `replayed` per block or
`transaction_discarded`, `journal_emptied`, and `recovery_flag_cleared`.
While `needsRecovery()` is true the path mutations throw `NeedsRecovery`;
reads, `layout`, `annotateSector`, and `writeRaw` see the raw on-disk state.
`layout()` carries a region of kind `journal` named `journal` over the
journal's data blocks only: its indirect blocks stay in the data region,
which is split around the journal, and a foreign image whose journal is
fragmented gets one `journal (part k)` region per run, so match on the kind,
not the name. `annotateSector` of a journal block explains it (its fields,
its tags, or the block it copies). A transaction that would tag more than
`maxTransaction` blocks throws `Unsupported` and changes nothing: on the
default disk only data-mode writes above roughly 250 KiB reach it.

## Error codes

| Code | Raised when |
|---|---|
| `NotFound`, `AlreadyExists`, `InvalidPath`, `InvalidName` | a path or name does not resolve, already exists, or is malformed |
| `DiskFull`, `DirectoryFull`, `FileTooLarge` | no room for the data, the entry, or the file size |
| `NotADirectory`, `IsADirectory`, `DirectoryNotEmpty` | the path names the wrong kind of entry |
| `InvalidGeometry` | format options the filesystem cannot lay out |
| `CorruptImage` | on-disk metadata that does not parse (see `corruption()`) |
| `Unsupported` | a feature the emulator does not implement, `armCrash` on ext2, or an image with no recognisable signature |
| `OutOfBounds` | `writeRaw` past the end of the disk |
| `NeedsRecovery` | a path mutation on an ext3 volume whose journal needs `recover()` (message `needs recovery`) |
| `BadArgument` | a malformed argument from JS (the wrapper's own) |
| `NotFat`, `NotExt` | a family-specific method on another family (the wrapper's own) |

## Detection

`fromImage(bytes)` names the family from the image's signature before it
parses anything, checking in this order:

1. ext: the image is at least 1,082 bytes long and the `u16le` at offset
   1080 (the superblock's `s_magic`) is `0xEF53`;
2. FAT: the image is at least 512 bytes long and bytes 510..512 are `55 AA`.

The ext magic covers ext2 and ext3 alike; the ext parser tells them apart by
the `has_journal` feature, and `fsType()` then says `"ext2"` or `"ext3"`.
ext goes first because a bootable ext image can also carry `55 AA` at 510.
When an image carries the ext magic but does not parse as ext and also carries
`55 AA` at 510 (a FAT volume can hold `53 EF` at 1080 in its FAT), it is
handed to the FAT parser, and FAT's result, error included, is what
`fromImage` returns.
Whether the metadata behind a signature parses is then that family's parser's
job, so a recognised image can still throw `CorruptImage` (or `Unsupported`
for an ext feature the emulator does not implement). An image that matches
neither signature throws `Unsupported` with the message `unsupported: no
recognisable filesystem signature`; that includes images shorter than 512
bytes and FAT images whose `55 AA` is gone, which used to reach the FAT parser
and throw `CorruptImage`. An image over 256 MiB throws `BadArgument` before
detection.

### Loading an ext2 image

The ext2 loader takes revision 1 images with 1 KiB blocks, 128-byte inodes,
exactly the `filetype` and `sparse_super` features, and no compat features
other than ext3's `has_journal`; anything else there throws `Unsupported`
naming the reason. It also needs 8,192 blocks per group (otherwise
`CorruptImage`).
The intended `mke2fs` form for an image it can load (not yet run against this
crate by hand; CI's e2fsprogs checks go the other way, judging the
emulator's images) is:

```
mke2fs -t ext2 -b 1024 -I 128 -O none,filetype,sparse_super <image>
```

The last group must hold its own metadata (a backup superblock and
descriptor table when it has them, both bitmaps, and its inode table). With
512 inodes per group (the default 16 MiB volume's), group 1 needs 68 blocks,
so `formatExt2` with `totalBlocks` from 8,194 to 8,260 throws
`InvalidGeometry` (`group 1 has N blocks but its metadata needs 68`), and
`fromImage` throws `CorruptImage` with the same text for an image whose
`s_blocks_count` falls there; with the default inode ratio at that size (256
inodes per group, 36 metadata blocks) the range is 8,194 to 8,229. Every
group boundary has the same shape: a last group of 1 block up to one short of
its metadata. `mke2fs` never writes such an image, because it drops a last
group that small from the block count.

### Loading an ext3 image

An ext3 image loads when it meets the ext2 rules above plus the journal's:
an internal journal on inode 8 of at least 1,024 blocks, a version-2 journal
superblock with 1 KiB blocks, no journal features, and ordered or data
journaling (writeback throws `Unsupported`). The `mke2fs` form is:

```
mke2fs -t ext3 -b 1024 -I 128 -O none,has_journal,filetype,sparse_super -J size=1 <image>
```

The leading `none,` matters: without it `mke2fs -t ext3` adds its default
`ext_attr`, `resize_inode`, `dir_index`, and `large_file` features, which
throw `Unsupported`. (Checked by hand with e2fsprogs 1.47.4 on a 16,384-block
image: it loads as `"ext3"` with a 1,024-block ordered journal, takes a
`createFile`, and recovers.) Images from other tools must not carry journal
checksums (`journal_checksum_v2`, `journal_checksum_v3`) or 64-bit tags
(`journal_64bit`); those, like `journal_async_commit` and an external
journal, throw `Unsupported` at load, and `recover()` throws `Unsupported`
for a log that holds revoke records. Loading never replays
the journal: an image whose superblock has `needs_recovery` set (a crashed
volume, or a real one that was not unmounted) loads with `needsRecovery()`
true, its path mutations throw `NeedsRecovery`, and its reads show the
on-disk state until `recover()` replays it, as `e2fsck -fy` would. A loaded
journal starts its next transaction at journal block 1, as the kernel does.

## Build and test

```
wasm-pack build crates/wasm --target bundler   # writes crates/wasm/pkg
wasm-pack test --node crates/wasm              # boundary tests
cargo test -p fs-emulator-wasm                 # native DTO and detection tests
```

## Demo

Build the wasm package first: `wasm-pack build crates/wasm --target bundler`.
The demo is written against FAT16: it formats FAT16 only, and an ext image
loaded into it reports `not a FAT volume` (its file listing calls
`clusterOwners`).

```
cd web/demo && pnpm install && pnpm dev
```
