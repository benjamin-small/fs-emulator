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
`CorruptImage`, ..., plus `NotFat` and `BadArgument`).

## Generic and filesystem-specific methods

`Volume` wraps the `FileSystem` trait from `fs-core`, so `createFile`,
`writeFile`, `readFile`, `deleteFile`, `createDir`, `removeDir`, `listDir`,
`stat`, `setNow`, `layout`, `annotateSector`, `historyLength`, `historyAt`,
`sectorSize`, `sectorCount`, `sector`, and `image` work on every filesystem
the crate will ever hold. `fsType()` names the one inside (`"FAT16"` today).

`bootSector`, `geometry`, `fatEntries`, `clusterChain`, `rawDirEntries`,
`clusterOwners`, and `annotateSectorWith` are FAT-only and throw an error
with code `NotFat` on any other volume. When FAT32 lands it reuses them; when
ext2 lands it adds its own constructor (`formatExt2`) and its own inspection
methods that throw `NotExt`. A UI should branch on `fsType()` before calling
the specific ones. The plan is in `docs/ROADMAP.md`.

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
