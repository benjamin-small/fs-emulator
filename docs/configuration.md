# Configuration

fs-emulator is a static browser application backed by WebAssembly. It has no
server component, runtime secrets, accounts, or required environment
variables. A normal local build uses the defaults.

## Build-time base path

`VITE_BASE` sets the URL prefix written into the UI build. It defaults to `/`
for local development. The GitHub Pages workflow sets it to `/fs-emulator/`:

```sh
VITE_BASE=/fs-emulator/ pnpm --dir web/ui build
```

## Optional test artifact

`FS_EMULATOR_EXT2_IMAGE` is used only by one native Rust test. When set to a
file path, the test writes a formatted ext2 image containing `/hello.txt` so it
can be loaded into the explorer for manual inspection:

```sh
FS_EMULATOR_EXT2_IMAGE=/tmp/fs-emulator-ext2.img \
  cargo test -p fs-emulator-wasm writes_an_ext2_image_for_the_explorer_when_asked
```

The generated image is disposable and must not be committed.

## External test tools

The ext2 compatibility tests use `e2fsck`, `dumpe2fs`, and `debugfs` from
`e2fsprogs`. They skip with an explicit message when the tools are unavailable
locally, and fail when `CI` is set and the tools are missing. `CI` is owned by
the automation environment and is not application configuration.
