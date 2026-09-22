# fs-emulator

Filesystem emulators on an in-memory virtual disk, written in Rust as a
learning tool. FAT16 is implemented today, byte-accurately; FAT32 and ext2/ext3
are planned on the same filesystem-agnostic core. Every operation records exactly which bytes changed and
why, so a UI can show how the disk evolves. Exported images mount on macOS and
Linux.

## Crates

- `fs-core`: filesystem-agnostic core. The `Disk`, the change journal
  (`OpRecord`, `ByteChange`, `Event`), shared types, and the `FileSystem`
  trait. FAT32 and ext2/ext3 will plug in here later.
- `fat`: the FAT16 implementation (`FatFs`) plus FAT-specific inspection:
  `fat_entries`, `cluster_chain`, `raw_dir_entries`, `annotate_sector`.
- `wasm` (`fs-emulator-wasm`): the wasm-bindgen `Volume` class and
  TypeScript types for browsers.

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

## Development

```
cargo test --workspace
cargo test -p fat --test mount_macos -- --ignored   # mounts the image with hdiutil
cargo build --workspace --target wasm32-unknown-unknown
```

Design: `docs/superpowers/specs/2026-09-21-fat16-emulator-design.md`.

## Demo

`web/demo` is a Vite + TypeScript page that exercises the `wasm` crate in a
browser. Build the wasm package first with
`wasm-pack build crates/wasm --target bundler`, then `cd web/demo && pnpm install && pnpm dev`.

## Explorer UI

`web/ui` is the learning UI (Svelte 5) for exploring FAT volumes. Build the
wasm package first with `wasm-pack build crates/wasm --target bundler`, then
`cd web/ui && pnpm install && pnpm dev`.
