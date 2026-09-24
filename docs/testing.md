# Testing

## Current coverage

Coverage was measured on September 24, 2026 from the pull request #13 branch.

- The Rust workspace reports **93.65% line coverage**, **94.11% region
  coverage**, and **89.89% function coverage** across `fs-core`, `fat`, `ext`,
  and the native portion of `fs-emulator-wasm`.
- The browser UI reports **48.49% line and statement coverage**, **93.88%
  branch coverage**, and **90.25% function coverage** across all UI source
  files. The aggregate line number includes Svelte components and stores that
  are validated by type-checking and production builds but are not instrumented
  by the Node-based Vitest suite. The instrumented pure modules report 97.74%
  line coverage in `src/core`, 100% in `src/scenarios`, and 95.35% in
  `src/shell`.

These are measurements, not minimum thresholds. Update this section when tests
or measurement boundaries change.

Reproduce the reports from the repository root:

```sh
cargo llvm-cov --workspace --all-features --summary-only
wasm-pack build crates/wasm --target bundler
pnpm --dir web/ui install --frozen-lockfile
pnpm --dir web/ui coverage
```

`cargo llvm-cov` is supplied by the `cargo-llvm-cov` tool. Coverage output is
local-only and ignored by Git.

## What is tested

The Rust suite exercises the in-memory disk and journal, path and timestamp
types, FAT16 boot sectors, allocation tables, directories and long names,
format/create/write/delete operations, corruption handling, ext2 superblocks,
block groups, bitmaps, inodes, direct and indirect block mapping, directory
growth, image import/export, annotations, and filesystem-family detection.
External ext2 tests validate generated images with `e2fsck`, `dumpe2fs`, and
`debugfs`. The ignored macOS and Linux mount tests provide explicit manual
checks against the host kernels.

`wasm-pack test --node crates/wasm` exercises the JavaScript-facing `Volume`
boundary. The TypeScript suite exercises the real generated WebAssembly package
through adapters and integration scenarios, plus byte attribution, formatting,
corruption, timeline patches, scenario playback, terminal parsing and commands,
virtual filesystem behavior, redirects, `dd`, `xxd`, theming, and layout
helpers. `pnpm --dir web/ui build` adds Svelte type-checking and a production
bundle gate for the component layer. The demo build checks the package from a
second consumer.
