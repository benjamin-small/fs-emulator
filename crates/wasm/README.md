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
```

Errors are `Error` objects with a `code` property: one per core error
(`NotFound`, `DiskFull`, `CorruptImage`, `Unsupported`, `OutOfBounds`, ...),
plus the wrapper's own `BadArgument`, `NotFat`, and `NotExt`.

## Constructors

`formatFat16(options)` formats a FAT16 volume from `FormatOptions`, and
`formatExt2(options)` a minimal ext2 revision 1 volume with 1 KiB blocks
from `ExtFormatOptions`; both accept `undefined` for the defaults.

```ts
interface ExtFormatOptions {
  totalBlocks?: number;    // default 16384 (16 MiB, two block groups); 64 to 262144
  inodesPerGroup?: number; // default one inode per 16 KiB; a multiple of 8, 16 to 8192
  label?: string;          // at most 16 bytes of UTF-8; default empty
  uuid?: string;           // 32 hex digits, bare or hyphenated 8-4-4-4-12;
                           // default e2f5ee00-2026-4923-8000-000000000001
}
```

On an ext2 volume the disk's sector is the block, so `sectorSize()` is 1024
and sector N is block N. An unknown key, a malformed `uuid`, a label longer
than 16 bytes, or a volume over the 256 MiB limit (`totalBlocks * 1024`, like
FAT's `totalSectors * bytesPerSector`) throws `BadArgument`; options the
format itself rejects (fewer than 64 blocks, an `inodesPerGroup` that is not
a multiple of 8) throw `InvalidGeometry`. `fromImage(bytes)` loads an image
of either family; see Detection.

## Generic and filesystem-specific methods

`Volume` wraps the `FileSystem` trait from `fs-core`, so `createFile`,
`writeFile`, `readFile`, `deleteFile`, `createDir`, `removeDir`, `listDir`,
`stat`, `setNow`, `layout`, `annotateSector`, `historyLength`, `historyAt`,
`sectorSize`, `sectorCount`, `sector`, `writeRaw`, `readRaw`, `corruption`,
and `image` work on every filesystem the crate holds. `fsType()` names the
one inside (`"FAT16"` or `"ext2"`).

`writeRaw(offset, bytes)` writes anywhere on the disk and returns an
`OpRecord` like every other mutating call, so the timeline and diff cover it;
a range past the end throws `OutOfBounds` and writes nothing. A write that
touches the family's on-disk metadata re-parses it (FAT: the first 512
bytes, the boot sector; ext2: the primary superblock in block 1 and the
primary group descriptor table): if it still parses and fits the disk, the
new geometry is adopted (the disk is the truth); if not, the path methods
(`createFile`, `writeFile`, `readFile`, `deleteFile`, `createDir`,
`removeDir`, `listDir`, `stat`, and FAT's `rawDirEntries`) throw
`CorruptImage` with a message naming what no longer parses (`boot sector no
longer parses after a raw write` on FAT, `superblock or group descriptors no
longer parse after a raw write` on ext2) until a later `writeRaw` repairs it.
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
will reuse them. `blockGroupCount()` is ext-only and throws `NotExt`
(message `not an ext volume`) on any other volume; the spec names it as the
one ext-only method for this slice (decision 3, section 8). The rest of the
ext inspection (superblock, inodes, block ownership) arrives with the
explorer's ext panels. A UI should branch on `fsType()` before calling the specific
ones. The plan is in `docs/ROADMAP.md`.

## Detection

`fromImage(bytes)` names the family from the image's signature before it
parses anything, checking in this order:

1. ext: the image is at least 1,082 bytes long and the `u16le` at offset
   1080 (the superblock's `s_magic`) is `0xEF53`;
2. FAT: the image is at least 512 bytes long and bytes 510..512 are `55 AA`.

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
exactly the `filetype` and `sparse_super` features, and no compat features;
anything else there throws `Unsupported` naming the reason. It also needs
8,192 blocks per group (otherwise `CorruptImage`).
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
