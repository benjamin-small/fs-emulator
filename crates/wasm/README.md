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
`sectorSize`, `sectorCount`, `sector`, `writeRaw`, `readRaw`, and `image`
work on every filesystem the crate will ever hold. `fsType()` names the one
inside (`"FAT16"` today).

`writeRaw(offset, bytes)` writes anywhere on the disk and returns an
`OpRecord` like every other mutating call, so the timeline and diff cover it;
a range past the end throws `OutOfBounds` and writes nothing. A write that
touches the first 512 bytes re-parses the boot sector: if it still parses and
fits the disk, the new geometry is adopted (the disk is the truth); if not,
the path methods (`createFile`, `writeFile`, `readFile`, `deleteFile`,
`createDir`, `removeDir`, `listDir`, `stat`, `rawDirEntries`) throw
`CorruptImage` with a message beginning `boot sector no longer parses after a
raw write` until a later `writeRaw` repairs it. `layout`, `annotateSector`,
`sector`, `readRaw`, `image`, and `writeRaw` keep working meanwhile, and
`bootSector()`/`geometry()` return the last good values. `readRaw(offset,
len)` returns a copy of the bytes and throws `BadArgument` past the end. Both
take byte offsets, not sector numbers, as non-negative integers (wasm-bindgen
does not validate them).

`bootSector`, `geometry`, `corruption`, `fatEntries`, `clusterChain`,
`rawDirEntries`, `clusterOwners`, and `annotateSectorWith` are FAT-only and
throw an error with code `NotFat` on any other volume. `corruption()` returns
the `CorruptImage` message while the boot sector does not parse after a raw
write, or `null` while the volume is mounted. When FAT32 lands it reuses
them; when ext2 lands it adds its own constructor (`formatExt2`) and its own
inspection methods that throw `NotExt`. A UI should branch on `fsType()`
before calling the specific ones. The plan is in `docs/ROADMAP.md`.

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
