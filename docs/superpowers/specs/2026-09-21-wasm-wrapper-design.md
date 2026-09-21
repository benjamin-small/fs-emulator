# WASM Wrapper Crate — Design

**Date:** 2026-09-21
**Status:** Approved
**Builds on:** `docs/superpowers/specs/2026-09-21-fat16-emulator-design.md`

## Purpose

Expose the emulator to a browser UI. A new crate `crates/wasm` wraps the
`FileSystem` trait and the FAT-specific inspection API behind one
wasm-bindgen class, `Volume`, with plain JS objects crossing the boundary and
TypeScript types shipped in the generated `.d.ts`. A minimal demo page under
`web/demo` proves the package end to end.

The UI consumer is a bundler-based TypeScript app (Vite or similar) importing
the `wasm-pack build --target bundler` output as an npm package.

## Non-goals for v1

- Zero-copy views into WASM linear memory (every byte export is a copy)
- Publishing to npm
- Any UI framework choice; the demo page is plain TypeScript
- Streaming or paging of large exports beyond `historyAt`

## Crate: `crates/wasm`

- Package name `fs-emulator-wasm`, library name `fs_emulator_wasm`,
  `crate-type = ["cdylib", "rlib"]`.
- Dependencies (this crate only; `fs-core` and `fat` stay zero-dependency):
  `wasm-bindgen 0.2`, `serde 1` (derive), `serde_bytes 0.11`,
  `serde-wasm-bindgen 0.6`, `js-sys 0.3`, `console_error_panic_hook 0.1`.
  Dev: `wasm-bindgen-test 0.3`.
- Workspace member, so `cargo test --workspace` compiles and runs its native
  unit tests, and `cargo build --workspace --target wasm32-unknown-unknown`
  builds it.

### Modules

| Module | Responsibility |
|---|---|
| `dto` | `Serialize`/`Deserialize` mirrors of every type the UI sees, and `From` conversions from the core types. No wasm-bindgen imports, so it is testable natively. |
| `error` | `Error` → JS `Error` with a `code` property. |
| `volume` | The `#[wasm_bindgen]` `Volume` class: constructors, generic ops, byte access, FAT inspection. |
| `types` | The hand-written TypeScript declarations, attached with `typescript_custom_section`. |
| `lib` | `pub use`, the `start` function installing `console_error_panic_hook`. |

### DTOs (`dto.rs`)

Field names are camelCase on the JS side (`#[serde(rename_all = "camelCase")]`).

```rust
DateTime      { year, month, day, hour, minute, second }      // Serialize + Deserialize
EntryInfo     { name, is_dir, size: u64, created: Option<DateTime>, modified, accessed }
ByteChange    { offset: usize, before: Vec<u8> (serde_bytes), after: Vec<u8> (serde_bytes) }
EventRecord   { kind: String, text: String, region: Option<Range> }   // text = Display of the Event
Range         { start: usize, end: usize }
OpRecord      { op: String, changes: Vec<ByteChange>, events: Vec<EventRecord> }
Region        { name: String, sectors: Range64 { start: u64, end: u64 }, kind: String }  // kind: "boot" | "metadata" | "allocationTable" | "directory" | "data" | "reserved" | "other"
Annotation    { range: Range, label: String, value: String }
FormatOptions { bytes_per_sector?, sectors_per_cluster?, total_sectors?, fat_count?, root_entries?, reserved_sectors?, volume_label?: String, volume_id?, enforce_fat16_range? }  // Deserialize; every field optional, defaults from fat::FormatOptions::default(); volume_label is padded/truncated to 11 bytes
BootSector    { oem_name: String, bytes_per_sector, sectors_per_cluster, reserved_sectors, fat_count, root_entries, total_sectors: u32, media, sectors_per_fat, sectors_per_track, heads, hidden_sectors, drive_number, boot_signature, volume_id, volume_label: String, fs_type: String }
Geometry      { variant: String ("fat16"), bytes_per_sector, sectors_per_cluster, reserved_sectors, fat_count, sectors_per_fat, root_entries, root_dir_sectors, first_root_dir_sector, first_data_sector, total_sectors, cluster_count }
FatEntry      tagged union on "kind": { kind: "free" } | { kind: "next", cluster: u32 } | { kind: "endOfChain" } | { kind: "bad" } | { kind: "reserved" }
RawEntry      tagged union on "kind": { kind: "free" } | { kind: "deleted", bytes } | { kind: "short", name: String, attr: u8, first_cluster: u32, size: u32, created?, modified?, accessed?, is_dir: bool } | { kind: "lfn", order: u8, is_last: bool, checksum: u8, text: String }
ClusterOwner  { cluster: u32, path: String, is_dir: bool, first_cluster: u32 }   // one per owned cluster
```

Serialization uses `serde_wasm_bindgen::Serializer::json_compatible()` so
maps become plain objects and `u64` becomes a JS number (all values here are
far below 2^53).

### Errors (`error.rs`)

`fn to_js(err: fs_core::Error) -> JsValue` builds `js_sys::Error::new(&err.to_string())`
and sets a `code` property via `js_sys::Reflect::set` to the variant name:
`NotFound`, `AlreadyExists`, `InvalidPath`, `InvalidName`, `DiskFull`,
`DirectoryFull`, `NotADirectory`, `IsADirectory`, `DirectoryNotEmpty`,
`FileTooLarge`, `InvalidGeometry`, `CorruptImage`, `Unsupported`. The wrapper
adds two codes of its own: `NotFat` (a FAT-only method called on another
filesystem) and `BadArgument` (an options object that fails to deserialize,
or a sector/index out of range where the core API would panic).

### `Volume` (`volume.rs`)

```rust
#[wasm_bindgen]
pub struct Volume { inner: Inner }
enum Inner { Fat(FatFs) }
impl Volume {
    fn fs(&self) -> &dyn FileSystem
    fn fs_mut(&mut self) -> &mut dyn FileSystem
    fn fat(&self) -> Result<&FatFs, JsValue>   // Err(NotFat) for other variants
}
```

Methods, all `#[wasm_bindgen(js_name = camelCase)]` with
`unchecked_return_type` naming the TypeScript type:

| JS name | Behaviour |
|---|---|
| `Volume.formatFat16(options?)` | `FormatOptions` DTO (or undefined) → `fat::FormatOptions` → `FatFs::format`. |
| `Volume.fromImage(bytes)` | `Uint8Array` → `Vec<u8>` → `FatFs::from_image`. |
| `fsType()` | `"FAT16"`. |
| `setNow(now)` | DateTime DTO → `set_now`. |
| `createFile(path, data)`, `writeFile(path, data)`, `deleteFile(path)`, `createDir(path)`, `removeDir(path)` | trait ops; return `OpRecord` DTO. |
| `readFile(path)` | `Uint8Array` copy. |
| `listDir(path)`, `stat(path)` | `EntryInfo[]` / `EntryInfo`. |
| `layout()` | `Region[]`. |
| `annotateSector(sector)` | `Annotation[]` (empty past the end). |
| `historyLength()`, `historyAt(index)` | `historyAt` throws `BadArgument` when out of range. |
| `sectorSize()`, `sectorCount()` | numbers. |
| `sector(n)` | `Uint8Array` copy; `BadArgument` past the end. |
| `image()` | `Uint8Array` copy of the whole disk. |
| `bootSector()`, `geometry()` | FAT only. |
| `fatEntries(fat)` | `FatEntry[]`; empty for an out-of-range copy (core behaviour). |
| `clusterChain(start)` | `number[]`; core errors map to `CorruptImage`. |
| `rawDirEntries(path)` | `RawEntry[]`. |
| `clusterOwners()` | `ClusterOwner[]`, sorted by cluster. |
| `annotateSectorWith(sector, owners)` | rebuilds the `BTreeMap` from the array and calls the core method. |

Data parameters are `&[u8]` (wasm-bindgen copies from `Uint8Array`).
Byte returns are `Vec<u8>` (wasm-bindgen produces a new `Uint8Array`).

### TypeScript (`types.rs`)

One `typescript_custom_section` string declaring the interfaces above plus
`type FatEntry = ...` and `type RawEntry = ...` unions, and
`interface FsError extends Error { code: string }`. The `Volume` class
signature is generated by wasm-bindgen with the `unchecked_return_type` /
`unchecked_param_type` names pointing at these interfaces.

### `lib.rs`

`#[wasm_bindgen(start)] fn init() { console_error_panic_hook::set_once(); }`
so any residual panic reaches the browser console with a message.

## Demo page: `web/demo`

Vite + TypeScript, no framework. `package.json` with `dev`, `build`,
`preview`; `pnpm` lockfile committed. Depends on the local package via
`"fs-emulator-wasm": "file:../../crates/wasm/pkg"`. `vite.config.ts` enables
top-level await targets so the bundler-target wasm loads.

`index.html` + `main.ts` renders four panes:

1. **Actions**: path input, textarea, buttons for Add file, Overwrite,
   Delete, Mkdir; a Load image file input and an Export image download link.
2. **Root listing**: `listDir("/")` rendered as a table.
3. **Last operation**: `op` and every `events[].text` of the most recent
   `historyAt(historyLength() - 1)`, plus the changed sector numbers derived
   from `changes`.
4. **Sector view**: a sector number input, the `layout()` region that
   contains it, a 16-bytes-per-row hex dump of `sector(n)`, and the
   `annotateSector(n)` list; annotation ranges highlight bytes on hover.

Errors thrown by the package are shown in a status line as `code: message`.

## Build and CI

- `crates/wasm/README.md` documents: `wasm-pack build crates/wasm --target bundler`,
  `wasm-pack test --node crates/wasm`, and running the demo
  (`cd web/demo && pnpm install && pnpm dev`).
- CI job `wasm`: install wasm-pack, `wasm-pack test --node crates/wasm`,
  `wasm-pack build crates/wasm --target bundler`, then `pnpm install --frozen-lockfile`
  and `pnpm build` in `web/demo`. The existing `check` job already covers
  `cargo test --workspace`, clippy (now including `crates/wasm`) and the
  wasm32 build.
- `crates/wasm/pkg/` and `web/demo/node_modules`, `web/demo/dist` are
  git-ignored.

## Testing

**Native (`cargo test -p fs-emulator-wasm`)**, in `dto.rs`: every `From`
conversion produces the expected fields; `FormatOptions` DTO with all fields
absent equals `fat::FormatOptions::default()`; `volume_label` padding and
truncation; `FatEntry`/`RawEntry` tag names; `RegionKind` string names;
`ClusterOwner` list is sorted.

**Boundary (`wasm-pack test --node crates/wasm`)**, in `tests/volume.rs`:
format then `fsType`, `sectorSize`, `sectorCount`; create/list/read/stat
round trip with an `Uint8Array`; `OpRecord` shape (op string, non-empty
changes with `before`/`after` as `Uint8Array`, events with kind/text/region);
`historyLength` grows per mutating op and `historyAt` out of range throws
with `code === "BadArgument"`; `NotFound` and `AlreadyExists` codes;
`fromImage(image())` round trip; `fatEntries(0)[0].kind === "reserved"`;
`clusterOwners` and `annotateSectorWith` agree with `annotateSector`;
`rawDirEntries("/")` shows an `lfn` then a `short` entry for a long name;
`formatFat16({ totalSectors: 2048 })` throws `InvalidGeometry`;
`formatFat16({ totalSectors: 2048, enforceFat16Range: false })` succeeds.

**Demo**: `pnpm build` in CI proves it compiles against the generated types.
Manual check in a browser is the acceptance test for the four panes.
