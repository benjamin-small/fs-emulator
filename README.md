# fs-emulator

[![CI](https://github.com/benjamin-small/fs-emulator/actions/workflows/ci.yml/badge.svg)](https://github.com/benjamin-small/fs-emulator/actions/workflows/ci.yml)
[![Explorer](https://img.shields.io/badge/explorer-live-378add)](https://benjamin-small.github.io/fs-emulator/)

Filesystem emulators on an in-memory virtual disk, written in Rust as a
learning tool. Every byte of the disk is a genuine on-disk layout, so exported
images mount on macOS and Linux, and every operation records exactly which
bytes changed and why. A browser UI built on those records lets you watch a
filesystem lay itself out one operation at a time.

FAT16 is implemented today. FAT32 comes next in the same crate, then ext2 and
ext3 in a new one, all on the same filesystem-agnostic core and the same UI.
See [docs/ROADMAP.md](docs/ROADMAP.md).

Try it: https://benjamin-small.github.io/fs-emulator/

## How it fits together

```
web/ui  (Svelte)         the explorer: hex dump, ribbon, timeline, scenarios, terminal
web/demo (TypeScript)    smoke test for the wasm package
        │
crates/wasm              one `Volume` class over the FileSystem trait
        │
crates/fat   crates/ext (planned)      one crate per filesystem family
        │
crates/fs-core           Disk, byte journal (incl. raw writes), regions, annotations, FileSystem trait
```

Each layer only depends on the one below it. The UI programs against the
`FileSystem` trait and the region and annotation types in `fs-core`, plus a
small set of filesystem-specific inspection calls that only the family's
adapter under `web/ui/src/fs/<family>/` makes, chosen from `fsType()`. Adding
a filesystem means a new crate, a new variant in the wasm `Volume`, and an
adapter, panels, and scenarios in the UI; nothing above `fs-core` needs to
change shape.

## Status

| Area | State |
|---|---|
| FAT16 (`crates/fat`) | Complete: format, create, overwrite, delete, directories, LFN, mountable images, per-sector annotations |
| FAT32 (`crates/fat`) | Planned. Cluster width, FAT entry codec, and `FatVariant` are already dispatched; see the roadmap for what is not |
| ext2, ext3 (`crates/ext`) | Planned |
| `crates/wasm` | Complete for FAT16; FAT-only methods throw `NotFat` on other volumes |
| `web/ui` | Complete for FAT16; the dump, ribbon, timeline, strings, and the terminal drawer (`/mnt`, `/dev/hda`) are region-driven and carry over |

## Crates

- `fs-core`: filesystem-agnostic core. The `Disk`, the change journal
  (`OpRecord`, `ByteChange`, `Event`), `Region` and `Annotation`, shared types,
  and the `FileSystem` trait, including `write_raw` for journaled writes to
  arbitrary disk offsets. Zero external dependencies, no I/O or clock, so
  it compiles unchanged for `wasm32-unknown-unknown`.
- `fat`: the FAT family. FAT16 today (`FatFs`) with FAT32's seams designed in,
  plus FAT-specific inspection: `fat_entries`, `cluster_chain`,
  `raw_dir_entries`, `cluster_owners`, `annotate_sector`.
- `wasm` (`fs-emulator-wasm`): the wasm-bindgen `Volume` class and TypeScript
  types for browsers. See `crates/wasm/README.md`.

## Example

```rust
use fat::{FatFs, FormatOptions};
use fs_core::FileSystem;

let mut fs = FatFs::format(FormatOptions::default()).unwrap();
fs.create_dir("/DOCS").unwrap();
let record = fs.create_file("/DOCS/Hello world.txt", b"hi").unwrap();
for event in &record.events {
    println!("{event}");
}
let image: &[u8] = fs.disk().as_bytes();
```

## Web

`web/ui` is the explorer: a whole-disk hex dump with an ASCII gutter and
strings overlay, a disk ribbon and FAT cluster map showing where files land, an
operation timeline that rewinds the disk byte for byte, guided scenarios, and a
terminal drawer (the user's `@benjamin-small/browser-terminal`) with the volume
at `/mnt` and the raw disk at `/dev/hda`, so `ls`, `cat`, `write`, `dd`, and
`xxd` drive the same journal as the forms. The build from `main` is published
to GitHub Pages by `.github/workflows/pages.yml`. See `web/ui/README.md` for
panes, the command table, shortcuts, and what in it is FAT-specific.

`web/demo` is a plain TypeScript page that exercises the wasm package; it
exists as a smoke test for the package, not as a UI.

Both need the wasm package built first:

```
wasm-pack build crates/wasm --target bundler
cd web/ui && pnpm install && pnpm dev       # or web/demo
```

## Development

```
cargo test --workspace
cargo test -p fat --test mount_macos -- --ignored   # mounts the image with hdiutil
cargo build --workspace --target wasm32-unknown-unknown
wasm-pack test --node crates/wasm
cd web/ui && pnpm test && pnpm build
```

CI (`.github/workflows/ci.yml`) runs fmt, clippy with warnings denied, the
Rust tests, the wasm32 build, the wasm-pack tests, and the demo and UI builds
on every pull request.

## Documentation

- [docs/ROADMAP.md](docs/ROADMAP.md): what comes next, what was deliberately
  deferred, and the conventions for adding a filesystem.
- `docs/superpowers/specs/`: the design documents, one per feature. They are
  the binding description of how each piece works:
  `2026-09-21-fat16-emulator-design.md` (core and FAT16, including the FAT32
  seams), `2026-09-21-wasm-wrapper-design.md`,
  `2026-09-21-fat-explorer-ui-design.md`,
  `2026-09-22-terminal-access-design.md` (raw writes and the terminal drawer),
  and `2026-09-23-fs-adapter-design.md` (the "fs explorer" rename and the
  per-family `FsAdapter` seam in the UI).
- `docs/superpowers/plans/`: the task-by-task implementation plans each spec
  was built from. They record how the code came to be, not how it must stay;
  the specs and the code win where they differ.

## License

MIT. See [LICENSE](LICENSE). The Cargo workspace and both web packages declare
the same license.
