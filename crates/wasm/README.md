# fs-emulator-wasm

Browser bindings for the emulator. One class, `Volume`, exposes the
filesystem-agnostic operations plus FAT-specific inspection; results are plain
JS objects with TypeScript types in the generated `.d.ts`.

```ts
import { Volume } from "fs-emulator-wasm";

const vol = Volume.formatFat16(undefined);
const record = vol.createFile("/Hello world.txt", new TextEncoder().encode("hi"));
for (const e of record.events) console.log(e.kind, e.text);
console.log(vol.listDir("/"));
```

Errors are `Error` objects with a `code` property (`NotFound`, `DiskFull`,
`CorruptImage`, `OutOfBounds`, ..., plus `NotFat` and `BadArgument`).

## Generic and filesystem-specific methods

`Volume` wraps the `FileSystem` trait from `fs-core`, so `createFile`,
`writeFile`, `readFile`, `deleteFile`, `createDir`, `removeDir`, `listDir`,
`stat`, `setNow`, `layout`, `annotateSector`, `historyLength`, `historyAt`,
`sectorSize`, `sectorCount`, `sector`, `writeRaw`, `readRaw`, `corruption`,
and `image` work on every filesystem the crate will ever hold. `fsType()`
names the one inside (`"FAT16"` today).

`writeRaw(offset, bytes)` writes anywhere on the disk and returns an
`OpRecord` like every other mutating call, so the timeline and diff cover it;
a range past the end throws `OutOfBounds` and writes nothing. A write that
touches the first 512 bytes re-parses the boot sector: if it still parses and
fits the disk, the new geometry is adopted (the disk is the truth); if not,
the path methods (`createFile`, `writeFile`, `readFile`, `deleteFile`,
`createDir`, `removeDir`, `listDir`, `stat`, `rawDirEntries`) throw
`CorruptImage` with a message beginning `boot sector no longer parses after a
raw write` until a later `writeRaw` repairs it. `corruption()` returns that
`CorruptImage` message while the gate is up, or `null` while the volume is
mounted; a family without such a gate answers `null` always. `layout`,
`annotateSector`, `sector`, `readRaw`, `image`, and `writeRaw` keep working
meanwhile, and `bootSector()`/`geometry()` return the last good values.
`readRaw(offset, len)` returns a copy of the bytes and throws `BadArgument`
past the end. Both take byte offsets, not sector numbers, as non-negative
integers (wasm-bindgen does not validate them).

`bootSector`, `geometry`, `fatEntries`, `clusterChain`, `rawDirEntries`,
`clusterOwners`, and `annotateSectorWith` are FAT-only and throw an error
with code `NotFat` on any other volume. When FAT32 lands it reuses them; when
ext2 lands it adds its own constructor (`formatExt2`) and its own inspection
methods that throw `NotExt`. A UI should branch on `fsType()` before calling
the specific ones. The plan is in `docs/ROADMAP.md`.

## Detection

`fromImage(bytes)` names the family from the image's signature before it
parses anything: FAT when the image is at least 512 bytes long and bytes
510..512 are `55 AA`; whether the BPB inside parses is then the FAT parser's
job. Today every image, recognised or not, goes to the FAT parser, so an
image without the signature still fails there with `CorruptImage` (`image is
shorter than one sector`, or `boot sector signature is not 55 AA`). When ext
lands, its check (the `u16le` at offset 1080 equal to `0xEF53`) runs before
the FAT check, because a bootable ext image can also carry `55 AA` at 510,
and only an image that matches neither throws `Unsupported` (`no
recognisable filesystem signature`).

## Build and test

```
wasm-pack build crates/wasm --target bundler   # writes crates/wasm/pkg
wasm-pack test --node crates/wasm              # boundary tests
cargo test -p fs-emulator-wasm                 # native DTO tests
```

## Demo

Build the wasm package first: `wasm-pack build crates/wasm --target bundler`.

```
cd web/demo && pnpm install && pnpm dev
```
