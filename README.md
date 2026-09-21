# fat16-emulator

A byte-accurate FAT16 filesystem on an in-memory virtual disk, written in Rust
as a learning tool. Every operation records exactly which bytes changed and
why, so a UI can show how the disk evolves. Exported images mount on macOS and
Linux.

## Crates

- `fs-core`: filesystem-agnostic core. The `Disk`, the change journal
  (`OpRecord`, `ByteChange`, `Event`), shared types, and the `FileSystem`
  trait. FAT32 and ext2/ext3 will plug in here later.
- `fat`: the FAT16 implementation (`FatFs`) plus FAT-specific inspection:
  `fat_entries`, `cluster_chain`, `raw_dir_entries`, `annotate_sector`.

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
