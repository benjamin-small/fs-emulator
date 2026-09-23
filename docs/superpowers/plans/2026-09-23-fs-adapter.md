# Filesystem Adapter Seam Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename the explorer to "fs explorer" and move every FAT16 assumption in `web/ui` behind a per-filesystem-family `FsAdapter` (one implementation, `fat16`, today), so the ext crate planned next can plug in without forking the UI, while FAT behaviour, every shell output string, every lesson string, and every attribution colour stay identical.

**Architecture:** A new module tree `web/ui/src/fs/` owns every filesystem-specific fact the UI uses: `fs/adapter.ts` declares `UnitSpace` (pure allocation-unit arithmetic) and `FsAdapter` (bound to a `Volume`, caching owners and tables as plain fields refreshed per operation); `fs/fat16/` is the only implementation and the new home of the FAT helpers (`direntry`, `fatchain`, `remnants`, the cluster geometry, the metadata trigger, the Format model, `FatMap.svelte`, `FormatForm.svelte`); `fs/index.ts` is the family registry and `fs/panels.ts` maps a family to its Svelte panels. `VolumeStore` holds the adapter and stays the single reactive clock through `epoch`; components, stores, the shell, and the scenario runner reach FAT facts only through it. The wasm crate changes only to put `corruption()` on the `FileSystem` trait and to name the `fromImage` detection rule. A source-scan test keeps FAT-only wasm calls and types out of everything outside `src/fs/fat16/`.

**Tech Stack:** Rust 1.87 workspace (fs-core, fat, wasm via wasm-bindgen + serde-wasm-bindgen), Svelte 5 runes + TypeScript strict, Vite 6, Vitest 3 in the node environment loading the real wasm package, pnpm 10, `@benjamin-small/browser-terminal@0.3.0` (exact).

**Spec:** `docs/superpowers/specs/2026-09-23-fs-adapter-design.md` (binding). Design record: `~/.claude/plans/ok-let-s-start-working-quiet-wren.md`.

## Global Constraints

1. FAT16 behaviour, every shell output string, every lesson string, and every attribution colour stay identical, with exactly two sanctioned string changes: (a) the `touch` no-op message in `web/ui/src/shell/commands.ts` and its pin in `web/ui/tests/shell/mutations.test.ts` change "FAT explorer" to "fs explorer" (spec section 5); (b) `CORRUPT_HELP` in `web/ui/src/shell/errors.ts` is reworded family-neutral, keeping `dd --of=/dev/hda` (spec section 3; the tests assert it only through the constant, so no expectation changes). No other asserted string or shell output changes. Beyond those two, the only expectation edits allowed in existing tests are the field renames `cluster` to `unit` and `firstCluster` to `firstUnit` in generic types, plus import paths and how a FAT adapter is constructed.
2. No ext code. Nothing is added for ext except the interface shape the spec defines.
3. Names, signatures, and file paths are the spec's, verbatim (section 1 interface, section 2 file table, section 3 store split, section 4 Rust changes, section 6 tests).
4. Reactivity rule: adapter caches are plain fields; every Svelte `$derived` that calls an adapter method reads `volume.epoch` first.
5. Boundary rule (enforced by `tests/adapterBoundary.test.ts` from Task 7): outside `web/ui/src/fs/fat16/**` and `web/ui/src/lib/wasm.ts`, no source file calls `geometry(`, `fatEntries(`, `clusterOwners(`, `annotateSectorWith(`, `rawDirEntries(`, `bootSector(`, `clusterChain(`, or `formatFat16(`, and none imports `ClusterOwner`, `FatEntry`, `Geometry`, `RawEntry`, `BootSector`, or `FormatOptions` from `lib/wasm`; imports from `fs/fat16` are allowed only from `src/fs/index.ts`, `src/fs/panels.ts`, and `src/scenarios/**`.
6. `crates/fs-core` and `crates/fat` keep zero external dependencies and no I/O, clock, or threads; `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass at every commit; `FatFs` behaviour and every wasm DTO are untouched.
7. `web/ui`: Svelte 5 runes, TypeScript strict with `verbatimModuleSyntax`, Vitest in the node environment loading the real wasm package (vitest has no Svelte plugin: nothing a node test imports may import a `.svelte` file); no new dependencies.
8. Gates by task: Tasks 1 and 2 keep every gate green (Rust gates, `pnpm test`, `pnpm build`). Tasks 3 and 4 keep `pnpm test` green; `pnpm build` (svelte-check) is expected to fail until Task 5 because components migrate there, and each such task says so in its final run step. Tasks 5, 6, and 7 keep `pnpm test` and `pnpm build` green. Task 7 ends with the browser pass in the spec's Verification section.
9. One commit per task, conventional message (`feat`, `fix`, `refactor`, `test`, `docs`, `chore`), no attribution lines. Specs are binding: where task text and spec differ, the spec wins.

---

### Task 1: Rust: `corruption()` on the trait and image detection

Spec: `docs/superpowers/specs/2026-09-23-fs-adapter-design.md`, section "4. Wasm and Rust" in full (points 1 to 6), including the `crates/wasm/README.md` and `docs/ROADMAP.md` edits points 5 and 6 name and the `host.ts` comment point 3 names. Everything below was applied to a copy of the tree at `81b0b56` and run through every gate (all green), so copy the code verbatim.

**Files:**
- Modify: `crates/fs-core/src/fs.rs` (import at line 5; the trait method goes after `write_raw` at line 31; the test `trait_is_object_safe` at lines 105-114)
- Modify: `crates/fat/src/fs.rs` (the `impl FileSystem for FatFs` block at lines 1019-1065; the new forwarding goes after the `write_raw` forwarding at lines 1050-1052; the inherent `corruption` at lines 88-93 stays as it is)
- Modify: `crates/wasm/src/volume.rs` (`enum Inner` lines 13-15; `fat()` lines 80-85; `from_image` lines 120-133; `read_raw` lines 269-289; the FAT-only `corruption` lines 310-318; the file ends at line 372)
- Modify: `crates/wasm/README.md` (the generic list lines 21-26, the `writeRaw` paragraph lines 28-41, the FAT-only paragraph lines 43-50)
- Modify: `docs/ROADMAP.md` ("What stays fixed" bullets lines 29-32; the deferred `web/ui` terminal paragraph lines 113-119; checklist step 5 lines 175-177)
- Modify: `web/ui/src/shell/host.ts` (the `corruptionOf` doc comment lines 32-36)
- Test: `crates/fs-core/src/fs.rs` (`trait_is_object_safe`), `crates/fat/tests/fat16.rs` (`usable_as_a_trait_object` at lines 229-235), `crates/wasm/src/volume.rs` (a new native test module at the end of the file)

**Interfaces:**
- Consumes (existing): `fs_core::Error` with its `CorruptImage(String)` variant; `fat::FatFs::corruption(&self) -> Option<&Error>` (inherent, `crates/fat/src/fs.rs:91`); `fat::FatFs::from_image(bytes: Vec<u8>) -> fs_core::Result<FatFs>` and its two `CorruptImage` texts (`image is shorter than one sector` from `crates/fat/src/fs.rs:56`, `boot sector signature is not 55 AA` from `crates/fat/src/boot_sector.rs:218`); `fat::FatFs::disk(&self) -> &Disk` and `fs_core::Disk::as_bytes`; `crate::error::{js_error, to_js}` and `Volume::fs(&self) -> &dyn FileSystem` in `crates/wasm/src/volume.rs`; the wasm boundary test `corruption_is_null_until_sector_zero_breaks_and_clears_when_repaired` (`crates/wasm/tests/volume.rs:337`), which stays byte-for-byte unchanged; `corruptionOf(vol: Volume): string | null` in `web/ui/src/shell/host.ts:37` and its pin in `web/ui/tests/shell/errors.test.ts:70`.
- Produces:
  - `fs_core::FileSystem::corruption(&self) -> Option<&Error>` (provided trait method, default `None`)
  - `<fat::FatFs as fs_core::FileSystem>::corruption`, delegating to the inherent `FatFs::corruption`
  - `Volume.corruption(): string | null` in the generated `.d.ts`: the same JS signature as today, now served from the generic `impl Volume` block through `self.fs().corruption()`, so it never throws `NotFat`. Task 4's `VolumeStore.refreshMeta` drops its try/catch on the strength of this.
  - `enum Detected { Fat, Unknown }` and `fn detect(bytes: &[u8]) -> Detected` in `crates/wasm/src/volume.rs` (private; the ext slice adds `Detected::Ext` and the `Unsupported` fallthrough)
  - `Volume.fromImage` unchanged in behaviour: every current error text and code is identical
  - the README "Detection" paragraph, the ROADMAP adapter bullet and rewritten checklist step 5, the resolved `fatEntryOffset` note removed

Gate commands used throughout (run from the repository root):

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
wasm-pack test --node crates/wasm
wasm-pack build crates/wasm --target bundler
```

---

- [ ] **Step 1: Write the failing trait-level `corruption()` assertions**

In `crates/fs-core/src/fs.rs`, inside `trait_is_object_safe` (lines 105-114), replace:

```rust
        assert_eq!(fs.fs_type(), "null");
        assert_eq!(fs.layout()[0].kind, crate::layout::RegionKind::Other);
```

with:

```rust
        assert_eq!(fs.fs_type(), "null");
        assert!(fs.corruption().is_none());
        assert_eq!(fs.layout()[0].kind, crate::layout::RegionKind::Other);
```

(`NullFs` gains no method: the default answers `None`.)

In `crates/fat/tests/fat16.rs`, `usable_as_a_trait_object` (lines 229-235), replace:

```rust
    assert_eq!(fs.read_file("/T").unwrap(), b"trait");
    assert_eq!(fs.fs_type(), "FAT16");
}
```

with:

```rust
    assert_eq!(fs.read_file("/T").unwrap(), b"trait");
    assert_eq!(fs.fs_type(), "FAT16");
    assert!(fs.corruption().is_none());
    fs.write_raw(510, &[0, 0]).unwrap();
    assert!(matches!(fs.corruption(), Some(Error::CorruptImage(_))));
    fs.write_raw(510, &[0x55, 0xAA]).unwrap();
    assert!(fs.corruption().is_none());
}
```

(`Error` and `FileSystem` are already imported at line 2 of that file; `fs` is a `Box<dyn FileSystem>`, so these calls resolve to the trait method, not the inherent one.)

- [ ] **Step 2: Run them and confirm the failures**

Run: `cargo test -p fs-core trait_is_object_safe`

Expected: `error[E0599]: no method named `corruption` found for struct `Box<dyn fs::FileSystem>` in the current scope` pointing at `crates/fs-core/src/fs.rs:112`.

Run: `cargo test -p fat --test fat16 usable_as_a_trait_object`

Expected: the same `error[E0599]: no method named `corruption` found for struct `Box<dyn FileSystem>`` twice, at `crates/fat/tests/fat16.rs:235` and `:237`.

- [ ] **Step 3: Add `corruption()` to the trait and forward it from `FatFs`**

`crates/fs-core/src/fs.rs`, line 5. Replace:

```rust
use crate::{DateTime, Disk, EntryInfo, OpRecord, Result};
```

with:

```rust
use crate::{DateTime, Disk, EntryInfo, Error, OpRecord, Result};
```

(The test module's own `use crate::{DateTime, Disk, EntryInfo, Error, OpRecord, Result};` at line 42 stays: an explicit import shadows the `super::*` glob without a warning.)

Same file, the trait (lines 31-33). Replace:

```rust
    fn write_raw(&mut self, offset: u64, bytes: &[u8]) -> Result<OpRecord>;

    fn disk(&self) -> &Disk;
```

with:

```rust
    fn write_raw(&mut self, offset: u64, bytes: &[u8]) -> Result<OpRecord>;
    /// The `CorruptImage` a raw write left behind, or `None` while mounted;
    /// families without a gate keep the default.
    fn corruption(&self) -> Option<&Error> {
        None
    }

    fn disk(&self) -> &Disk;
```

`crates/fat/src/fs.rs`, in `impl FileSystem for FatFs` (lines 1050-1055). Replace:

```rust
    fn write_raw(&mut self, offset: u64, bytes: &[u8]) -> Result<OpRecord> {
        FatFs::write_raw(self, offset, bytes)
    }
    fn disk(&self) -> &Disk {
        FatFs::disk(self)
    }
```

with:

```rust
    fn write_raw(&mut self, offset: u64, bytes: &[u8]) -> Result<OpRecord> {
        FatFs::write_raw(self, offset, bytes)
    }
    fn corruption(&self) -> Option<&Error> {
        FatFs::corruption(self)
    }
    fn disk(&self) -> &Disk {
        FatFs::disk(self)
    }
```

(`Error` is already in that file's `use fs_core::{...}` at lines 12-14. The inherent `pub fn corruption` at lines 88-93 is untouched, so the crate API is unchanged.)

- [ ] **Step 4: Run the fs-core and fat tests**

Run: `cargo test -p fs-core && cargo test -p fat`

Expected: `fs::tests::trait_is_object_safe ... ok` among `25 passed` for fs-core; for fat, `82 passed` in the lib, `usable_as_a_trait_object ... ok` among `16 passed` in `fat16`, and `0 passed; 0 failed; 1 ignored` for `mount_macos`.

---

- [ ] **Step 5: Write the failing native `detect` test**

Append to the end of `crates/wasm/src/volume.rs` (after the closing `}` of the FAT-specific `impl Volume` block, line 372):

```rust

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{detect, Detected};

    #[test]
    fn detect_names_fat_by_the_55_aa_signature_in_a_full_first_sector() {
        let mut img = vec![0u8; 512];
        assert_eq!(detect(&img), Detected::Unknown);
        img[510] = 0x55;
        img[511] = 0xAA;
        assert_eq!(detect(&img), Detected::Fat);
        assert_eq!(detect(&img[..511]), Detected::Unknown);
        assert_eq!(detect(&[]), Detected::Unknown);
        img[510] = 0xAA;
        img[511] = 0x55;
        assert_eq!(detect(&img), Detected::Unknown);
        let mut long = vec![0xFFu8; 4096];
        long[510] = 0x55;
        long[511] = 0xAA;
        assert_eq!(detect(&long), Detected::Fat);
        let real = fat::FatFs::format(fat::FormatOptions::default()).unwrap();
        assert_eq!(detect(real.disk().as_bytes()), Detected::Fat);
    }
}
```

(The `cfg(all(test, not(target_arch = "wasm32")))` gate is the one `crates/wasm/src/error.rs:37` and `crates/wasm/src/dto.rs:463` use, so the module runs under `cargo test -p fs-emulator-wasm` and is absent from the wasm32 build.)

- [ ] **Step 6: Run it and confirm the failure**

Run: `cargo test -p fs-emulator-wasm detect`

Expected: `error[E0432]: unresolved imports `super::detect`, `super::Detected`` at `crates/wasm/src/volume.rs:376`.

- [ ] **Step 7: Add `Detected`, `detect`, wire `from_image`, and reword `fat()`**

`crates/wasm/src/volume.rs`, lines 13-15. Replace:

```rust
enum Inner {
    Fat(FatFs),
}
```

with:

```rust
enum Inner {
    Fat(FatFs),
}

/// The family an image's signature names; `from_image` picks the parser from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Detected {
    Fat,
    Unknown,
}

/// Detection rule: FAT when the image is at least one 512-byte sector long
/// and bytes 510..512 are `55 AA`. Whether the BPB inside parses is
/// `FatFs::from_image`'s job, not this function's.
///
/// Reserved for the ext slice: an ext superblock is recognised by the `u16le`
/// at offset 1080 equal to `0xEF53`, and that check must run before the FAT
/// check because a bootable ext image can also carry `55 AA` at 510. Once
/// `Inner::Ext` exists, `Unknown` becomes
/// `Unsupported("no recognisable filesystem signature")` in `from_image`.
fn detect(bytes: &[u8]) -> Detected {
    if bytes.len() >= 512 && bytes[510..512] == [0x55, 0xAA] {
        Detected::Fat
    } else {
        Detected::Unknown
    }
}
```

Same file, `fat()` (lines 80-81). Replace:

```rust
    /// The FAT volume, or a `NotFat` error once other filesystems exist.
    fn fat(&self) -> Result<&FatFs, JsValue> {
```

with:

```rust
    /// The FAT volume: `Ok` for the only variant today; the `NotFat` arm
    /// arrives with `Inner::Ext`.
    fn fat(&self) -> Result<&FatFs, JsValue> {
```

(The single-arm match stays: a second arm is an unreachable pattern under `-D warnings`.)

Same file, `from_image` (lines 129-135). Replace:

```rust
        let fs = FatFs::from_image(bytes).map_err(to_js)?;
        Ok(Volume {
            inner: Inner::Fat(fs),
        })
    }

    #[wasm_bindgen(js_name = fsType)]
```

with:

```rust
        let fs = match detect(&bytes) {
            // Both arms parse as FAT today, so an image without the signature
            // still fails inside `FatFs::from_image` with the same
            // `CorruptImage` text and code as before. The ext slice adds an
            // `Ext` arm and turns `Unknown` into `Unsupported`.
            Detected::Fat | Detected::Unknown => FatFs::from_image(bytes).map_err(to_js)?,
        };
        Ok(Volume {
            inner: Inner::Fat(fs),
        })
    }

    #[wasm_bindgen(js_name = fsType)]
```

- [ ] **Step 8: Run the wasm crate's native tests**

Run: `cargo test -p fs-emulator-wasm`

Expected: `volume::tests::detect_names_fat_by_the_55_aa_signature_in_a_full_first_sector ... ok` and `12 passed` (11 before this task).

---

- [ ] **Step 9: Move `corruption` from the FAT-only block to the generic one**

`crates/wasm/src/volume.rs`, in the FAT-specific `impl Volume` (lines 310-320). Replace:

```rust
    /// The `CorruptImage` message while the boot sector does not parse after
    /// a `writeRaw`, or `null` while the volume is mounted.
    #[wasm_bindgen(unchecked_return_type = "string | null")]
    pub fn corruption(&self) -> Result<JsValue, JsValue> {
        Ok(match self.fat()?.corruption() {
            Some(err) => JsValue::from_str(&err.to_string()),
            None => JsValue::NULL,
        })
    }

    /// Every entry of one FAT copy, indexed by cluster; empty for a copy that does not exist.
```

with:

```rust
    /// Every entry of one FAT copy, indexed by cluster; empty for a copy that does not exist.
```

Same file, in the generic `impl Volume`, after `read_raw` (lines 286-291). Replace:

```rust
        Ok(disk
            .read(offset as usize, (end - offset as u64) as usize)
            .to_vec())
    }

    /// A copy of the whole disk image.
```

with:

```rust
        Ok(disk
            .read(offset as usize, (end - offset as u64) as usize)
            .to_vec())
    }

    /// The `CorruptImage` message a raw write left behind (the on-disk
    /// metadata no longer parses), or `null` while the volume is mounted.
    /// Families without a gate answer `null` always.
    #[wasm_bindgen(unchecked_return_type = "string | null")]
    pub fn corruption(&self) -> Result<JsValue, JsValue> {
        Ok(match self.fs().corruption() {
            Some(err) => JsValue::from_str(&err.to_string()),
            None => JsValue::NULL,
        })
    }

    /// A copy of the whole disk image.
```

(The return type stays `Result<JsValue, JsValue>` so the boundary test's `v.corruption().unwrap()` and the generated `corruption(): string | null` are unchanged; the body can no longer fail.)

- [ ] **Step 10: Run the Rust gates**

Run: `cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace`

Expected: fmt prints nothing; clippy ends in `Finished` with no warnings; every `test result:` line reads `ok`, with fs-core `25 passed`, fat `82 passed` (lib) and `16 passed` (`fat16`), `1 ignored` (`mount_macos`), and fs-emulator-wasm `12 passed`.

- [ ] **Step 11: Run the wasm-pack gates and check the declaration**

Run: `wasm-pack test --node crates/wasm`

Expected: `running 9 tests`, `corruption_is_null_until_sector_zero_breaks_and_clears_when_repaired ... ok`, `test result: ok. 9 passed` (tests/volume.rs is unchanged).

Run: `wasm-pack build crates/wasm --target bundler && grep -n "corruption(): string | null" crates/wasm/pkg/fs_emulator_wasm.d.ts`

Expected: `[INFO]: 📦   Your wasm pkg is ready to publish at crates/wasm/pkg.` and one grep hit, `    corruption(): string | null;`.

---

- [ ] **Step 12: Update the wasm README**

`crates/wasm/README.md`, lines 24-26. Replace:

```
`sectorSize`, `sectorCount`, `sector`, `writeRaw`, `readRaw`, and `image`
work on every filesystem the crate will ever hold. `fsType()` names the one
inside (`"FAT16"` today).
```

with:

```
`sectorSize`, `sectorCount`, `sector`, `writeRaw`, `readRaw`, `corruption`,
and `image` work on every filesystem the crate will ever hold. `fsType()`
names the one inside (`"FAT16"` today).
```

Same file, lines 35-50 (the end of the `writeRaw` paragraph and the whole FAT-only paragraph). Replace:

```
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
```

with:

```
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
```

- [ ] **Step 13: Update the ROADMAP**

`docs/ROADMAP.md`, under "What stays fixed", after the region-driven bullet (lines 29-32). Replace:

```
- **The UI is region-driven.** The hex dump, disk ribbon, byte attribution,
  strings overlay, and timeline read `layout()` regions and the journal, not
  FAT structures. Filesystem-specific panels (the FAT map, the entry and chain
  trace, the scenarios) sit beside them.
```

with:

```
- **The UI is region-driven.** The hex dump, disk ribbon, byte attribution,
  strings overlay, and timeline read `layout()` regions and the journal, not
  FAT structures. Filesystem-specific panels (the FAT map, the entry and chain
  trace, the scenarios) sit beside them.
- **Per-family UI knowledge lives behind `FsAdapter`.** What the explorer
  knows about one family (its allocation-unit noun, byte ownership, chain
  tracing, `stat` facts, the Format form, the map panel) is one adapter under
  `web/ui/src/fs/<family>/`, chosen from `fsType()` and guarded by a
  source-scan test (`web/ui/tests/adapterBoundary.test.ts`) that keeps family-specific wasm calls out of the rest of
  `web/ui/src`.
```

Same file, the deferred `web/ui` terminal paragraph (lines 115-119). Replace:

```
`/dev/zero` is unreachable through the runner; `fatEntryOffset` is duplicated
between the shell's `stat` and `Inspector.svelte` until FAT32 work extracts
it; `select` warns before validating its target; `flagGiven` and the range
message are repeated between `commands.ts` and `dd.ts`; `commands.ts` should
get a second module before the next command group; the loading and error
```

with:

```
`/dev/zero` is unreachable through the runner; `select` warns before
validating its target; `flagGiven` and the range message are repeated
between `commands.ts` and `dd.ts`; `commands.ts` should get a second module
before the next command group; the loading and error
```

Same file, checklist step 5 (lines 175-177). Replace:

```
5. UI: extend `attribution` with the new region kinds and colors, add a
   family-specific map panel, extend the inspector and the Format panel, and
   write scenarios that teach what is different about this filesystem.
```

with:

```
5. UI: implement `FsAdapter` under `web/ui/src/fs/<family>/`, register it in
   `fs/index.ts` and `fs/panels.ts`, add its signature to `detect`, and write
   scenarios that teach what is different about this filesystem.
```

- [ ] **Step 14: Reword the `corruptionOf` comment in `host.ts`**

`web/ui/src/shell/host.ts`, lines 32-36. Replace:

```ts
/**
 * `vol.corruption()`, or `null` when the call itself throws. `corruption()` will throw
 * `NotFat` on a non-FAT volume in the future, and a read command must not die on that;
 * VolumeStore.refreshMeta guards it the same way.
 */
```

with:

```ts
/**
 * `vol.corruption()`, or `null` when the call itself throws. `corruption()` is on the
 * `FileSystem` trait, so no family makes it throw `NotFat`; the guard stays so a read
 * command never dies on a wasm boundary failure (errors.test.ts feeds it a throwing
 * double).
 */
```

(The try/catch body is unchanged; `tests/shell/errors.test.ts:70` still feeds it a throwing double and still passes. The store's own try/catch in `state/volume.svelte.ts:46-48` is Task 4's, which removes it.)

- [ ] **Step 15: Run the web gates against the rebuilt package**

Run (from the repository root; the package built in Step 11 is a `file:` dependency that pnpm copies into its store, so reinstall before testing):

```
cd web/ui && pnpm install --frozen-lockfile && pnpm test && pnpm build
```

Expected: `Test Files  34 passed (34)` and `Tests  268 passed (268)` (the same counts as before this task); svelte-check reports no errors; vite ends with `✓ built in` and the `dist/` listing.

- [ ] **Step 16: Commit**

```
git add crates/fs-core/src/fs.rs crates/fat/src/fs.rs crates/fat/tests/fat16.rs crates/wasm/src/volume.rs crates/wasm/README.md docs/ROADMAP.md web/ui/src/shell/host.ts
git commit -m "feat(wasm): corruption() on the FileSystem trait and a named fromImage detection rule"
```

(`crates/wasm/pkg/` is gitignored; the rebuilt package is not committed.)
### Task 2: web/ui: the fs module and the fat16 adapter

Spec: `docs/superpowers/specs/2026-09-23-fs-adapter-design.md`, section 1 (the adapter interface) and section 2 (the FAT implementation and the registry) in full, plus the section 6 tests `tests/fs/fat16.test.ts`, `tests/fs/registry.test.ts`, `tests/fs/fat16-format.test.ts` and the `space` fixture. Every file below was run under vitest 3.2 against the real wasm package and type-checked with the repository's tsconfig options before being written down, so copy the code verbatim. This task does not depend on Task 1's wasm rebuild: every `Volume` method it calls, including `corruption()`, exists in `crates/wasm/pkg` at HEAD.

The shape of the task: the adapter interface and the registry are new; `direntry.ts`, `fatchain.ts` and `remnants.ts` move under `src/fs/fat16/` with one-line re-export shims left at their old paths, so `layers.svelte.ts`, `commands.ts`, `FatMap.svelte`, the four scenario files and the tests that still import from `core/` compile unchanged; the cluster arithmetic (`clusterOfSector`, `clusterByteRange`) and the boot-sector helpers (`BOOT_SECTOR_LEN`, `touchesBootSector`) stay where they are and are re-exported from their new FAT homes, because Task 3 rewrites `core/attribution.ts` and `core/patch.ts`. Nothing outside `src/fs/` and the three moved files changes.

**Files:**
- Create: `web/ui/src/fs/adapter.ts`, `web/ui/src/fs/index.ts`, `web/ui/src/fs/fat16/index.ts`, `web/ui/src/fs/fat16/adapter.ts`, `web/ui/src/fs/fat16/geometry.ts`, `web/ui/src/fs/fat16/metadata.ts`, `web/ui/src/fs/fat16/format.ts`
- Move (`git mv`, then the whole file is rewritten: the body is verbatim, the import paths change): `web/ui/src/core/fatchain.ts` -> `web/ui/src/fs/fat16/fatchain.ts` (import at line 1; `describeFatEntry` appended after line 27), `web/ui/src/core/direntry.ts` -> `web/ui/src/fs/fat16/direntry.ts` (imports at lines 1-3), `web/ui/src/core/remnants.ts` -> `web/ui/src/fs/fat16/remnants.ts` (imports at lines 1-4, the comment at line 23)
- Create (one-line re-export shims at the old paths, deleted by Task 3): `web/ui/src/core/fatchain.ts`, `web/ui/src/core/direntry.ts`, `web/ui/src/core/remnants.ts`
- Modify: `web/ui/tests/direntry.test.ts` (lines 3-4), `web/ui/tests/fatchain.test.ts` (line 2), `web/ui/tests/remnants.test.ts` (lines 3-4 and 6), `web/ui/tests/fixtures/geometry.ts` (line 1; a line appended after line 12)
- Test: `web/ui/tests/fs/registry.test.ts`, `web/ui/tests/fs/fat16-format.test.ts`, `web/ui/tests/fs/fat16.test.ts`
- Read only (the code copied into `src/fs/fat16/` comes from here; these files are untouched in this task and are edited by Tasks 3 to 5): `web/ui/src/components/ActionsPanel.svelte` lines 6-15 and 29-46 (`SIZES`, `CLUSTER_SIZES`, the FAT16 constants, `clusterCountFor`, `clusterProblem`), `web/ui/src/components/Inspector.svelte` lines 30-38 (`describe`) and 63-80 (the trace rows), `web/ui/src/shell/commands.ts` lines 263-286 (`stat`), 292-320 (`df`), 592-631 (`mkfs`'s flags and strings) and 484 (the `write --append` clause), `web/ui/src/shell/dd.ts` line 163 (the `dd` clause), `web/ui/src/shell/vfs.ts` line 74 (`namesMatch`), `web/ui/src/components/DirTree.svelte` line 12 (the corrupt note), `web/ui/src/components/HexView.svelte` line 81 (the cluster-start rule), `web/ui/src/state/volume.svelte.ts` lines 43-49 (`refreshMeta`), `web/ui/src/core/attribution.ts` lines 16-26 and 46-55, `web/ui/src/core/patch.ts` lines 28-35, `web/ui/src/scenarios/fundamentals.ts` lines 26-29 (`fatOffset`)

**Interfaces:**
- Consumes (existing, at HEAD):
  - `web/ui/src/lib/wasm.ts`: `Volume` with `static formatFat16(options: FormatOptions | undefined): Volume`, `fsType(): string`, `geometry(): Geometry`, `clusterOwners(): ClusterOwner[]` (sorted by cluster: the wasm DTO is built from a `BTreeMap`), `fatEntries(fat: number): FatEntry[]`, `annotateSectorWith(sector: number, owners: ClusterOwner[]): Annotation[]`, `layout(): Region[]`, `rawDirEntries(path: string): RawEntry[]`, `readRaw`, `writeRaw`, `corruption(): string | null`; the types `Annotation, ClusterOwner, FatEntry, FormatOptions, Geometry, RawEntry, Region, RegionKind`.
  - `web/ui/src/core/attribution.ts`: `clusterOfSector(g: Geometry, sector: number): number | undefined`, `clusterByteRange(g: Geometry, cluster: number): { start: number; end: number }` (lines 16-26).
  - `web/ui/src/core/palette.ts`: `COLOR_FREE = 0, COLOR_BOOT = 1, COLOR_FAT = 2, COLOR_DIR = 3, COLOR_FAT_ALT = 10`, `type ColorIndex = number`.
  - `web/ui/src/core/patch.ts`: `interface ByteChangeLike { offset: number; before: Uint8Array; after: Uint8Array }`, `BOOT_SECTOR_LEN = 512`, `touchesBootSector(changes: ByteChangeLike[]): boolean` (lines 28-35).
  - `web/ui/src/core/intervals.ts`: `interface Interval { start: number; end: number }`, `normalize(list: Interval[]): Interval[]`.
  - Tests only: `buildTree(vol: Volume, owners: ClusterOwner[]): TreeNode` from `core/tree.ts`, `scanZeroSectors(image: Uint8Array, sectorSize: number): Uint8Array` from `core/zeros.ts`.
  - Spec section 1: the interface block, copied verbatim into `src/fs/adapter.ts`.
- Produces (what Tasks 3 to 7 rely on):
  - `web/ui/src/fs/adapter.ts`: `type FsFamilyId = "fat16"`; `interface UnitVocab { singular; plural; letter; first }`; `interface UnitOwner { unit: number; path: string; isDir: boolean; firstUnit: number }`; `interface UnitSpace { unit; unitCount; unitSize; sectorSize; totalSectors; unitOfSector(sector): number | undefined; unitByteRange(unit): Interval; unitOfOffset(offset): number | undefined; unitStartsAt(sector): boolean; colorForRegion(region: Region): ColorIndex }`; `interface TraceRow { label: string; offset: number | null }`; `type StatFacts = Record<string, string | number | number[]>`; `interface DfFacts { unitSize; units; used; free }`; `interface MkfsFlag { long; desc; kind: "int" | "str"; option: string }`; `interface MkfsSpec { summary; done; flags: MkfsFlag[] }`; `interface FsFamily<O = unknown> { id; name; format(options?: O): Volume; bind(vol: Volume): FsAdapter; mkfs: MkfsSpec }`; `interface FsAdapter extends UnitSpace { id; name; family: FsFamily; vol: Volume; refresh(): void; owners: readonly UnitOwner[]; ownerOf(path): UnitOwner | undefined; chain(path): number[]; entrySlots(path): Interval | null; remnants?(zeros: Uint8Array): Interval[]; dataStart(path): number | null; regionStart(kind: RegionKind): number | undefined; stat(path): StatFacts; df(): DfFacts; trace(path): TraceRow[]; describeUnit(unit): string | null; annotateSector(sector): Annotation[]; parseAddr?(v: string): number | undefined; namesMatch(a, b): boolean; touchesMetadata(changes: ByteChangeLike[]): boolean; corruptNote: string; notes: { rewrite: string; partialWrite: string } }`.
  - `web/ui/src/fs/index.ts`: `FAMILIES: Record<FsFamilyId, FsFamily>`, `DEFAULT_FAMILY: FsFamilyId = "fat16"`, `familyIdOf(fsType: string): FsFamilyId` (matches `FsFamily.name`; throws `Error("no adapter for " + fsType)`), `adapterFor(vol: Volume): FsAdapter`.
  - `web/ui/src/fs/fat16/index.ts`: `fat16: FsFamily<Fat16FormatOptions>` (`id "fat16"`, `name "FAT16"`, `format(o) = Volume.formatFat16(o)`, `bind(v) = new Fat16Adapter(v)`, `mkfs = MKFS`), `type Fat16FormatOptions = FormatOptions`, `asFat16(fs: FsAdapter): Fat16Adapter` (throws `Error("not a FAT16 adapter: " + fs.id)` unless `fs.id === "fat16"`), `class Fat16Adapter`, and the re-exports `fat16Space(g: Geometry): UnitSpace`, `FAT_UNIT: UnitVocab` (`{ singular: "cluster", plural: "clusters", letter: "c", first: 2 }`), `fatColorForRegion(region: Region): ColorIndex`, `describeFatEntry(e: FatEntry): string` (`"free"`, `` `next → ${e.cluster}` ``, `"end of chain"`, `"bad"`, `"reserved"`), `buildChain`, `clusterState`, `type ClusterState`, `findEntrySlots`, `slotOffset`, `walkEntries`, `findRemnants`, `clusterOfSector`, `clusterByteRange`, `BOOT_SECTOR_LEN`, `touchesBootSector`, `CORRUPT_NOTE`, `NOTES`, `SIZES`, `CLUSTER_SIZES`, `clusterCountFor(total: number, spc: number): number`, `checkFormat(o: Pick<FormatOptions, "totalSectors" | "sectorsPerCluster">): { clusters: number; problem: string | null }`, `DEFAULTS: { totalSectors: 32768; sectorsPerCluster: 4; volumeLabel: "" }`, `MKFS: MkfsSpec`.
  - `Fat16Adapter` (beyond the interface): public plain field `fat: FatEntry[]` (FAT 0, for FatMap), public `owners: UnitOwner[]`, `fatEntryOffset(cluster: number, copy = 0): number` (`(g.reservedSectors + copy * g.sectorsPerFat) * g.bytesPerSector + cluster * 2`), and a constructor `new Fat16Adapter(vol: Volume)` that calls `refresh()` once.
  - Strings later tasks compose with: `CORRUPT_NOTE = "Boot sector does not parse; the tree is unavailable until a raw write repairs it."`; `NOTES.rewrite = "FAT has no append; the old chain is freed and reallocated"` (Task 4's `write --append` logs `` `appended ${n} bytes by rewriting the whole file (${host.adapter.notes.rewrite})` ``); `NOTES.partialWrite = "FAT has no partial writes; the whole file was rewritten"` (Task 4's `dd` logs it as the whole line); `MKFS.summary = "Format /dev/hda as FAT16 (clears the timeline)"`, `MKFS.done = "formatted /dev/hda as FAT16; the timeline was cleared"`, `MKFS.flags[i].option` in order `totalSectors, sectorsPerCluster, volumeLabel, rootEntries, fatCount, reservedSectors`; `describeUnit(n)` returns `` `FAT: ${describeFatEntry(fat[n])}` `` or `null`, so Task 5's Inspector renders it where today's ` · FAT: {describe(...)}` sits.
  - `web/ui/tests/fixtures/geometry.ts`: `space: UnitSpace = fat16Space(geo)`.
  - Hand-offs to Task 3: delete the three `core/` shims and repoint their importers (`shell/commands.ts` lines 4-5, `state/layers.svelte.ts` lines 2-5, `components/FatMap.svelte` line 3, `scenarios/{directory,longName,fundamentals,smallFile}.ts` line 1, `tests/{corruption,integration}.test.ts`, `tests/shell/read-commands.test.ts`); flip the two re-exports (`geometry.ts` defines `clusterOfSector`/`clusterByteRange` and `core/attribution.ts` stops exporting them; `metadata.ts` defines `BOOT_SECTOR_LEN`/`touchesBootSector` and `core/patch.ts` stops); when `core/palette.ts` renames `COLOR_FAT`/`COLOR_FAT_ALT` to `COLOR_TABLE`/`COLOR_TABLE_ALT` and `core/attribution.ts` gains `defaultColorForRegion`, `fatColorForRegion` becomes the `"FAT 1"` rule plus `defaultColorForRegion(region)`.

Gate commands, run from `web/ui`:

```
pnpm test
pnpm build
```

---

- [ ] **Step 1: Write the failing registry test**

Create `web/ui/tests/fs/registry.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { DEFAULT_FAMILY, FAMILIES, adapterFor, familyIdOf } from "../../src/fs";
import { fat16 } from "../../src/fs/fat16";

describe("the family registry", () => {
  it("maps Volume.fsType() to a family id, and refuses a type no adapter handles", () => {
    expect(familyIdOf("FAT16")).toBe("fat16");
    expect(() => familyIdOf("EXT3")).toThrow("no adapter for EXT3");
    expect(() => familyIdOf("")).toThrow("no adapter for ");
  });

  it("registers FAT16 as the default family, whose format() yields a volume of its own name", () => {
    expect(DEFAULT_FAMILY).toBe("fat16");
    expect(FAMILIES[DEFAULT_FAMILY]).toBe(fat16);
    expect(FAMILIES.fat16.name).toBe("FAT16");
    const vol = FAMILIES.fat16.format({ rootEntries: 16 });
    expect(vol.fsType()).toBe("FAT16");
    expect(vol.geometry().rootEntries).toBe(16);
    expect(FAMILIES.fat16.format().geometry().rootEntries).toBe(512); // no options: the default disk
  });

  it("adapterFor binds the family whose name is the volume's fsType", () => {
    const vol = FAMILIES.fat16.format();
    const fs = adapterFor(vol);
    expect(fs.id).toBe("fat16");
    expect(fs.name).toBe("FAT16");
    expect(fs.family).toBe(fat16);
    expect(fs.vol).toBe(vol);
  });
});
```

- [ ] **Step 2: Write the failing format-model test**

Create `web/ui/tests/fs/fat16-format.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { Volume, type FormatOptions } from "../../src/lib/wasm";
import { CLUSTER_SIZES, DEFAULTS, MKFS, SIZES, checkFormat, clusterCountFor } from "../../src/fs/fat16";
import { FAT16_MAX_CLUSTERS, FAT16_MIN_CLUSTERS } from "../../src/fs/fat16/format";

describe("clusterCountFor", () => {
  it("reproduces the core's cluster count for the default disk and for every form choice", () => {
    expect(clusterCountFor(32768, 4)).toBe(8167);
    for (const { totalSectors } of SIZES) {
      for (const sectorsPerCluster of CLUSTER_SIZES) {
        const { clusters, problem } = checkFormat({ totalSectors, sectorsPerCluster });
        expect(clusters).toBe(clusterCountFor(totalSectors, sectorsPerCluster));
        // The form's rule and the core's agree: the button is enabled exactly when formatFat16 succeeds.
        let formatted: number | null = null;
        try { formatted = Volume.formatFat16({ totalSectors, sectorsPerCluster }).geometry().clusterCount; } catch { formatted = null; }
        expect(problem === null, `${totalSectors} sectors, ${sectorsPerCluster} per cluster`).toBe(formatted !== null);
        if (formatted !== null) expect(clusters).toBe(formatted);
      }
    }
  });
});

describe("checkFormat", () => {
  it("names the problem at both FAT16 bounds and none inside them", () => {
    const few = checkFormat({ totalSectors: 8192, sectorsPerCluster: 8 });
    expect(few.clusters).toBeLessThan(FAT16_MIN_CLUSTERS);
    expect(few.problem).toBe("too few for FAT16");
    const many = checkFormat({ totalSectors: 131072, sectorsPerCluster: 1 });
    expect(many.clusters).toBeGreaterThan(FAT16_MAX_CLUSTERS);
    expect(many.problem).toBe("too many for FAT16");
    expect(checkFormat({ totalSectors: 32768, sectorsPerCluster: 4 })).toEqual({ clusters: 8167, problem: null });
    expect(checkFormat({})).toEqual({ clusters: 8167, problem: null }); // absent options take the form's defaults
  });
});

describe("DEFAULTS and MKFS", () => {
  it("DEFAULTS reproduces the default disk's geometry", () => {
    expect(DEFAULTS).toEqual({ totalSectors: 32768, sectorsPerCluster: 4, volumeLabel: "" });
    expect(Volume.formatFat16(DEFAULTS).geometry()).toEqual(Volume.formatFat16(undefined).geometry());
  });

  it("keeps the shell's mkfs strings", () => {
    expect(MKFS.summary).toBe("Format /dev/hda as FAT16 (clears the timeline)");
    expect(MKFS.done).toBe("formatted /dev/hda as FAT16; the timeline was cleared");
    expect(MKFS.flags.map((f) => [f.long, f.kind])).toEqual([
      ["sectors", "int"], ["spc", "int"], ["label", "str"], ["root-entries", "int"], ["fats", "int"], ["reserved", "int"],
    ]);
  });

  it("every flag's option is a FormatOptions key the wasm formatter accepts", () => {
    // Listing every key of the type here is the compile-time half: a key added to or
    // dropped from FormatOptions fails this object literal.
    const keys: Record<keyof FormatOptions, true> = {
      bytesPerSector: true, sectorsPerCluster: true, totalSectors: true, fatCount: true, rootEntries: true,
      reservedSectors: true, volumeLabel: true, volumeId: true, enforceFat16Range: true,
    };
    const sample: Record<string, number | string> = { totalSectors: 32768, sectorsPerCluster: 4, volumeLabel: "X", rootEntries: 512, fatCount: 2, reservedSectors: 1 };
    expect(MKFS.flags.map((f) => f.option)).toEqual(["totalSectors", "sectorsPerCluster", "volumeLabel", "rootEntries", "fatCount", "reservedSectors"]);
    for (const f of MKFS.flags) {
      expect(keys[f.option as keyof FormatOptions], f.option).toBe(true);
      // formatFat16 rejects unknown keys, so a misspelt option would throw here.
      expect(() => Volume.formatFat16({ [f.option]: sample[f.option] }), f.option).not.toThrow();
    }
  });
});
```

- [ ] **Step 3: Write the failing adapter test**

Create `web/ui/tests/fs/fat16.test.ts`. Its fixture is the one `tests/shell/read-commands.test.ts` builds (a directory in cluster 2, a two-cluster file in it, a long-named root file), so the `stat` and `df` values below are the ones the shell prints today; the corruption block is `tests/corruption.test.ts` ported to the adapter.

```ts
import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { adapterFor } from "../../src/fs";
import { asFat16, fat16, findEntrySlots } from "../../src/fs/fat16";
import { buildTree } from "../../src/core/tree";
import { scanZeroSectors } from "../../src/core/zeros";

const enc = (s: string) => new TextEncoder().encode(s);
const ROOT = 65 * 512;      // the root directory's first byte on the default disk
const DATA = 97 * 512;      // cluster 2's first byte
const CLUSTER = 2048;

/** The read-commands fixture: a directory (cluster 2), a two-cluster file in it (3, 4), and a
 *  long-named root file (5) whose entry takes two LFN slots and a short one. */
function fixture() {
  const vol = Volume.formatFat16(undefined);
  vol.createDir("/DOCS");
  vol.createFile("/DOCS/N.TXT", new Uint8Array(3000));
  vol.createFile("/Hello world.txt", enc("hello from the shell\n"));
  return { vol, fs: asFat16(adapterFor(vol)) };
}

describe("Fat16Adapter: identity and unit space", () => {
  it("is the fat16 family bound to its volume", () => {
    const { vol, fs } = fixture();
    expect(fs.id).toBe("fat16");
    expect(fs.name).toBe("FAT16");
    expect(fs.family).toBe(fat16);
    expect(fs.vol).toBe(vol);
    const alien = new Proxy(fs, { get: (t, k, r) => (k === "id" ? "ext3" : Reflect.get(t, k, r)) });
    expect(() => asFat16(alien)).toThrow("not a FAT16 adapter: ext3");
  });

  it("names the unit and reports the default geometry", () => {
    const { fs } = fixture();
    expect(fs.unit).toEqual({ singular: "cluster", plural: "clusters", letter: "c", first: 2 });
    expect(fs.unitCount).toBe(8167);
    expect(fs.unitSize).toBe(CLUSTER);
    expect(fs.sectorSize).toBe(512);
    expect(fs.totalSectors).toBe(32768);
  });

  it("does cluster arithmetic like the core", () => {
    const { fs } = fixture();
    expect(fs.unitOfSector(96)).toBeUndefined();
    expect(fs.unitOfSector(97)).toBe(2);
    expect(fs.unitOfSector(101)).toBe(3);
    expect(fs.unitByteRange(2)).toEqual({ start: DATA, end: DATA + CLUSTER });
    expect(fs.unitOfOffset(DATA + 7)).toBe(2);
    expect(fs.unitOfOffset(0)).toBeUndefined();
    expect(fs.unitStartsAt(97)).toBe(true);
    expect(fs.unitStartsAt(98)).toBe(false);
    expect(fs.unitStartsAt(101)).toBe(true);
    expect(fs.unitStartsAt(65)).toBe(false); // not a data sector
  });

  it("colours FAT 1 apart from FAT 0", () => {
    const { fs } = fixture();
    const [boot, fat0, fat1, root, data] = fs.vol.layout();
    expect(fs.colorForRegion(fat0)).toBe(2);
    expect(fs.colorForRegion(fat1)).toBe(10);
    expect(fs.colorForRegion(root)).toBe(3);
    expect(fs.colorForRegion(data)).toBe(0);
    expect(fs.colorForRegion(boot)).toBe(1);
  });
});

describe("Fat16Adapter: owners, chains, and entries", () => {
  it("mirrors the wasm owners in the generic shape, sorted by unit", () => {
    const { fs } = fixture();
    expect(fs.owners).toEqual([
      { unit: 2, path: "/DOCS", isDir: true, firstUnit: 2 },
      { unit: 3, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3 },
      { unit: 4, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3 },
      { unit: 5, path: "/Hello world.txt", isDir: false, firstUnit: 5 },
    ]);
    expect(fs.ownerOf("/DOCS/N.TXT")).toEqual({ unit: 3, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3 });
    expect(fs.ownerOf("/nope")).toBeUndefined();
  });

  it("follows a file's chain and returns [] where there is none", () => {
    const { fs } = fixture();
    expect(fs.chain("/DOCS/N.TXT")).toEqual([3, 4]);
    expect(fs.chain("/DOCS")).toEqual([2]);
    expect(fs.chain("/")).toEqual([]);
    expect(fs.chain("/nope")).toEqual([]);
  });

  it("finds the entry slots of a root entry, a subdirectory entry, an LFN name, and none for /", () => {
    const { vol, fs } = fixture();
    expect(fs.entrySlots("/DOCS")).toEqual({ start: ROOT, end: ROOT + 32 });
    expect(fs.entrySlots("/Hello world.txt")).toEqual({ start: ROOT + 32, end: ROOT + 4 * 32 }); // 2 LFN + short
    expect(fs.entrySlots("/DOCS/N.TXT")).toEqual({ start: DATA + 2 * 32, end: DATA + 3 * 32 });   // after . and ..
    expect(fs.entrySlots("/")).toBeNull();
    expect(fs.entrySlots("/nope")).toBeNull();
    expect(fs.entrySlots("/DOCS/N.TXT")).toEqual(findEntrySlots(vol, vol.geometry(), vol.fatEntries(0), vol.clusterOwners(), "/DOCS/N.TXT"));
  });

  it("reports remnants after a delete: the freed slots and the dirty free cluster", () => {
    const { vol, fs } = fixture();
    expect(fs.remnants(scanZeroSectors(vol.image(), 512))).toEqual([]);
    vol.deleteFile("/Hello world.txt");
    fs.refresh();
    const r = fs.remnants(scanZeroSectors(vol.image(), 512));
    expect(r).toContainEqual({ start: ROOT + 32, end: ROOT + 4 * 32 });
    expect(r).toContainEqual(fs.unitByteRange(5));
    expect(fs.ownerOf("/Hello world.txt")).toBeUndefined();
  });

  it("locates the data of the root, a file, and nothing for an empty file", () => {
    const { vol, fs } = fixture();
    expect(fs.dataStart("/")).toBe(ROOT);
    expect(fs.dataStart("/DOCS/N.TXT")).toBe(DATA + CLUSTER);
    expect(fs.dataStart("/nope")).toBeNull();
    vol.createFile("/EMPTY", new Uint8Array(0));
    fs.refresh();
    expect(fs.dataStart("/EMPTY")).toBeNull();
  });

  it("reads region starts from the layout", () => {
    const { fs } = fixture();
    expect(fs.regionStart("boot")).toBe(0);
    expect(fs.regionStart("allocationTable")).toBe(1); // FAT 0, the first of that kind
    expect(fs.regionStart("directory")).toBe(65);
    expect(fs.regionStart("data")).toBe(97);
    expect(fs.regionStart("metadata")).toBeUndefined();
  });
});

describe("Fat16Adapter: what the shell and the Inspector print", () => {
  it("stat returns exactly the seven FAT keys, formatted as the shell prints them", () => {
    const { vol, fs } = fixture();
    const n = fs.stat("/DOCS/N.TXT");
    expect(Object.keys(n)).toEqual(["firstCluster", "chain", "clusters", "entryOffset", "entrySlots", "fatEntryOffset", "dataOffset"]);
    expect(n).toEqual({
      firstCluster: 3, chain: [3, 4], clusters: 2,
      entryOffset: "0xc240", entrySlots: "0xc240-0xc260", fatEntryOffset: "0x206", dataOffset: "0xca00",
    });
    expect(fs.stat("/")).toEqual({ firstCluster: 0, chain: [], clusters: 0, entryOffset: "", entrySlots: "", fatEntryOffset: "", dataOffset: "0x8200" });
    vol.createFile("/EMPTY", new Uint8Array(0));
    fs.refresh();
    const empty = fs.stat("/EMPTY");
    expect(empty).toMatchObject({ firstCluster: 0, chain: [], clusters: 0, fatEntryOffset: "", dataOffset: "" });
    expect(empty.entryOffset).toBe("0x8280"); // root slot 4, after DOCS and the three Hello world.txt slots
    expect(empty.entrySlots).toBe("0x8280-0x82a0");
  });

  it("df counts used clusters from FAT 0", () => {
    const { fs } = fixture();
    expect(fs.df()).toEqual({ unitSize: CLUSTER, units: 8167, used: 4, free: 8163 });
  });

  it("trace gives the Inspector its three rows, or the muted no-data row", () => {
    const { vol, fs } = fixture();
    expect(fs.trace("/DOCS/N.TXT")).toEqual([
      { label: "Directory entry · offset 0xc240", offset: 0xc240 },
      { label: "FAT chain · 2 clusters starting at 3 · FAT entry at 0x206", offset: 0x206 },
      { label: "Data · cluster 3 at 0xca00", offset: 0xca00 },
    ]);
    vol.createFile("/EMPTY", new Uint8Array(0));
    fs.refresh();
    expect(fs.trace("/EMPTY")).toEqual([
      { label: "Directory entry · offset 0x8280", offset: 0x8280 },
      { label: "Data · no data clusters", offset: null },
    ]);
    expect(fs.trace("/nope")).toEqual([{ label: "Data · no data clusters", offset: null }]);
  });

  it("describes a FAT entry the way the Inspector's card does", () => {
    const { fs } = fixture();
    expect(fs.describeUnit(2)).toBe("FAT: end of chain");
    expect(fs.describeUnit(3)).toBe("FAT: next → 4");
    expect(fs.describeUnit(6)).toBe("FAT: free");
    expect(fs.describeUnit(0)).toBe("FAT: reserved");
    expect(fs.describeUnit(100000)).toBeNull();
  });

  it("annotates a sector with the cached owners", () => {
    const { vol, fs } = fixture();
    expect(fs.annotateSector(0)).toEqual(vol.annotateSectorWith(0, vol.clusterOwners()));
    expect(fs.annotateSector(65)).toEqual(vol.annotateSectorWith(65, vol.clusterOwners()));
    expect(fs.annotateSector(65).length).toBeGreaterThan(0);
  });

  it("matches names case-insensitively and flags boot-sector writes as metadata changes", () => {
    const { fs } = fixture();
    expect(fs.namesMatch("hello world.txt", "HELLO WORLD.TXT")).toBe(true);
    expect(fs.namesMatch("a.txt", "b.txt")).toBe(false);
    const c = (offset: number) => ({ offset, before: new Uint8Array(1), after: new Uint8Array(1) });
    expect(fs.touchesMetadata([c(511)])).toBe(true);
    expect(fs.touchesMetadata([c(1000), c(17)])).toBe(true);
    expect(fs.touchesMetadata([c(512), c(4096)])).toBe(false);
    expect(fs.touchesMetadata([])).toBe(false);
  });

  it("locates a cluster's FAT entry in either copy", () => {
    const { fs } = fixture();
    expect(fs.fatEntryOffset(3)).toBe(1 * 512 + 3 * 2);
    expect(fs.fatEntryOffset(2, 1)).toBe((1 + 32) * 512 + 2 * 2);
    expect(fs.corruptNote).toBe("Boot sector does not parse; the tree is unavailable until a raw write repairs it.");
    expect(fs.notes).toEqual({
      rewrite: "FAT has no append; the old chain is freed and reallocated",
      partialWrite: "FAT has no partial writes; the whole file was rewritten",
    });
  });
});

describe("Fat16Adapter: caches", () => {
  it("answers from the last refresh() until the next one", () => {
    const { vol, fs } = fixture();
    vol.createFile("/NEW.TXT", new Uint8Array(1));
    expect(fs.ownerOf("/NEW.TXT")).toBeUndefined();
    expect(fs.chain("/NEW.TXT")).toEqual([]);
    expect(fs.fat[6]).toEqual({ kind: "free" });
    expect(fs.df().used).toBe(4);
    fs.refresh();
    expect(fs.ownerOf("/NEW.TXT")).toEqual({ unit: 6, path: "/NEW.TXT", isDir: false, firstUnit: 6 });
    expect(fs.chain("/NEW.TXT")).toEqual([6]);
    expect(fs.fat[6]).toEqual({ kind: "endOfChain" });
    expect(fs.df().used).toBe(5);
  });

  it("re-reads the geometry: a boot-sector write that grows the root directory moves the data area", () => {
    const vol = Volume.formatFat16({ rootEntries: 16 });
    const fs = asFat16(adapterFor(vol));
    expect(fs.dataStart("/")).toBe(65 * 512);
    expect(fs.unitByteRange(2).start).toBe(66 * 512);
    const rec = vol.writeRaw(17, new Uint8Array([32, 0])); // BPB_RootEntCnt, u16 little-endian
    expect(fs.touchesMetadata(rec.changes)).toBe(true);
    fs.refresh();
    expect(fs.unitByteRange(2).start).toBe(67 * 512);
    expect(fs.regionStart("data")).toBe(67);
  });

  it("survives a corrupt volume as the store did, and sees it again once repaired", () => {
    const { vol, fs } = fixture();
    const slots = fs.entrySlots("/DOCS/N.TXT");
    const saved = vol.readRaw(0, 512);
    expect(vol.corruption()).toBeNull();
    vol.writeRaw(0, new Uint8Array(512));
    expect(vol.corruption()).toContain("boot sector no longer parses");
    // The same three unguarded wasm calls refreshMeta made: the FAT crate answers from its
    // last good geometry, so the owners are still there; only the path-based walks are gated.
    expect(() => fs.refresh()).not.toThrow();
    expect(fs.owners).toHaveLength(4);
    expect(fs.chain("/DOCS/N.TXT")).toEqual([3, 4]);
    expect(fs.entrySlots("/DOCS/N.TXT")).toBeNull();
    expect(fs.remnants(scanZeroSectors(vol.image(), 512))).toEqual([]);
    // The owner list is irrelevant to an empty root: listDir throws CorruptImage and buildTree absorbs it.
    expect(buildTree(vol, [])).toMatchObject({ name: "/", path: "/", isDir: true, children: [] });
    vol.writeRaw(0, saved);
    expect(vol.corruption()).toBeNull();
    fs.refresh();
    expect(fs.entrySlots("/DOCS/N.TXT")).toEqual(slots);
    expect(fs.owners).toHaveLength(4);
    expect(buildTree(vol, []).children.map((c) => c.name)).toEqual(["DOCS", "Hello world.txt"]);
  });
});
```

- [ ] **Step 4: Run the three new files and confirm they cannot resolve the module**

Run: `pnpm vitest run tests/fs`

Expected: all three files fail to load, each with an error of the form `Error: Cannot find module '../../src/fs' imported from '/Users/bsmall/dev/fs-emulator/web/ui/tests/fs/registry.test.ts'` (the format test names `'../../src/fs/fat16'`), then `Test Files  3 failed (3)` and `Tests  no tests`.

- [ ] **Step 5: Create the adapter interface**

Create `web/ui/src/fs/adapter.ts`. The declarations are the spec's section 1 block verbatim; the doc comment carries the contracts and the reactivity rule.

```ts
/**
 * The filesystem adapter seam. Every filesystem-specific fact the explorer, the shell and
 * the lessons need lives behind these interfaces, with one implementation per
 * `Volume.fsType()` family (`fs/fat16/` today); `fs/index.ts` picks the implementation.
 *
 * Two layers. `UnitSpace` is pure arithmetic over one geometry and can be built from a
 * fixture without a `Volume`. `FsAdapter` is bound to one `Volume` and caches owners,
 * tables, and geometry as plain fields that `refresh()` re-reads.
 *
 * Contracts:
 *
 * - `unitCount + unit.first` is the size of the attribution tables (`ownerByUnit`,
 *   `colorByUnit`); for FAT that is `clusterCount + 2`, exactly the table size before the seam.
 * - `stat()` returns already-formatted values (hex strings, `""` for absent) so the shell
 *   prints them untouched. The generic keys (`path`, `name`, `type`, `size`, `created`,
 *   `modified`, `accessed`) stay in `shell/commands.ts`.
 * - `df()` returns generic keys; the shell derives the printed keys from the noun:
 *   `${unit.singular}Size` and `unit.plural`. FAT prints `clusterSize` and `clusters`, so
 *   `tests/shell/read-commands.test.ts` is unchanged.
 * - The address help is composed generically in `shell/addr.ts`:
 *   `addresses: 0x1f (hex), 512 (decimal), s:65 (sector), ${letter}:3 (${singular})`,
 *   which for FAT is byte-identical to the old `ADDR_HELP`.
 * - Reactivity rule. Adapter caches are plain fields, not runes, so every `$derived` that
 *   calls an adapter method reads `volume.epoch` first (see state/volume.svelte.ts).
 */
import type { Volume, Region, RegionKind, Annotation } from "../lib/wasm";
import type { Interval } from "../core/intervals";
import type { ColorIndex } from "../core/palette";
import type { ByteChangeLike } from "../core/patch";

export type FsFamilyId = "fat16";                       // ext adds "ext3"

/** The allocation-unit noun: how the UI, shell, and lessons name a data unit. */
export interface UnitVocab {
  singular: string;   // "cluster"
  plural: string;     // "clusters"
  letter: string;     // "c": the shell address prefix (c:N)
  first: number;      // first valid unit index (FAT: 2)
}

/** One unit -> the path that owns it. Generic mirror of wasm's ClusterOwner. */
export interface UnitOwner { unit: number; path: string; isDir: boolean; firstUnit: number }

/** Pure unit arithmetic over one geometry. Buildable without a Volume. */
export interface UnitSpace {
  readonly unit: UnitVocab;
  readonly unitCount: number;                           // FAT: clusterCount
  readonly unitSize: number;                            // bytes per unit
  readonly sectorSize: number;                          // vol.sectorSize()
  readonly totalSectors: number;                        // vol.sectorCount()
  unitOfSector(sector: number): number | undefined;     // undefined outside the data area
  unitByteRange(unit: number): Interval;                // [start, end) bytes; no bounds check, as today
  unitOfOffset(offset: number): number | undefined;
  unitStartsAt(sector: number): boolean;                // the dump's row-label rule
  colorForRegion(region: Region): ColorIndex;           // FAT: "FAT 1" gets COLOR_TABLE_ALT
}

/** One row of the Inspector's "Selected file" trace. */
export interface TraceRow { label: string; offset: number | null }   // null = muted text, no jump
/** stat facts the shell prints as key/value after the generic ones. Keys are the family's. */
export type StatFacts = Record<string, string | number | number[]>;
export interface DfFacts { unitSize: number; units: number; used: number; free: number }

export interface MkfsFlag { long: string; desc: string; kind: "int" | "str"; option: string }
export interface MkfsSpec { summary: string; done: string; flags: MkfsFlag[] }

/** Static, per family: what exists before a volume of that family does. */
export interface FsFamily<O = unknown> {
  readonly id: FsFamilyId;
  readonly name: string;                                // "FAT16"; equals Volume.fsType() of format()'s result
  format(options?: O): Volume;                          // the one place Volume.formatFat16 is called
  bind(vol: Volume): FsAdapter;                         // caller has matched fsType; reads the caches once
  readonly mkfs: MkfsSpec;
}

/**
 * Bound to one Volume. Caches (owners, tables, geometry) are PLAIN fields refreshed by
 * `refresh()`; they are not reactive. Any `$derived` that calls a method here must read
 * `volume.epoch` first (see state/volume.svelte.ts). Tests use it without runes.
 */
export interface FsAdapter extends UnitSpace {
  readonly id: FsFamilyId;
  readonly name: string;
  readonly family: FsFamily;
  readonly vol: Volume;

  /** Re-read owners, tables and geometry from `vol`. Called by the store after every op and on bind. */
  refresh(): void;
  readonly owners: readonly UnitOwner[];                // sorted by unit
  ownerOf(path: string): UnitOwner | undefined;         // first owner row for a path

  chain(path: string): number[];                        // units in chain order; [] when none
  entrySlots(path: string): Interval | null;            // FAT: LFN + short slots; ext later: the inode's bytes
  /** Present only for families with "deleted things still on disk". Absent hides the Show remnants toggle. */
  remnants?(zeros: Uint8Array): Interval[];
  dataStart(path: string): number | null;               // "/" -> root directory bytes; file -> first unit start
  regionStart(kind: RegionKind): number | undefined;    // first sector of the first region of that kind (from vol.layout())

  stat(path: string): StatFacts;                        // FAT: firstCluster, chain, clusters, entryOffset, entrySlots, fatEntryOffset, dataOffset
  df(): DfFacts;
  trace(path: string): TraceRow[];                      // Inspector rows, family wording
  describeUnit(unit: number): string | null;            // Inspector: "FAT: next -> 4"
  annotateSector(sector: number): Annotation[];         // FAT: annotateSectorWith(sector, cached owners)

  /** Family address forms beyond s:N / hex / decimal / <letter>:N (ext: i:N). undefined = not ours. */
  parseAddr?(v: string): number | undefined;
  namesMatch(a: string, b: string): boolean;            // FAT: case-insensitive (vfs.canonicalize)

  /** True when the op may have moved regions; the store re-reads layout. FAT: any change starting below 512. */
  touchesMetadata(changes: ByteChangeLike[]): boolean;
  readonly corruptNote: string;                         // DirTree's note while the volume is corrupt
  readonly notes: { rewrite: string; partialWrite: string };  // the write --append and dd log clauses
}
```

- [ ] **Step 6: Create the FAT geometry module**

Create `web/ui/src/fs/fat16/geometry.ts`. `fatColorForRegion` is `colorForRegion` from `core/attribution.ts` lines 46-55, unchanged; `unitStartsAt` is the rule from `HexView.svelte` line 81 behind the same "is a data sector" guard HexView applies.

```ts
import type { Geometry, Region } from "../../lib/wasm";
import { clusterByteRange, clusterOfSector } from "../../core/attribution";
import { COLOR_BOOT, COLOR_DIR, COLOR_FAT, COLOR_FAT_ALT, COLOR_FREE, type ColorIndex } from "../../core/palette";
import type { UnitSpace, UnitVocab } from "../adapter";

// The cluster arithmetic still lives in core/attribution.ts until Task 3 moves the two
// functions here for real; this is already their FAT home for every importer.
export { clusterByteRange, clusterOfSector };

/** How FAT names its allocation unit: the shell writes `c:N`, and data clusters start at 2. */
export const FAT_UNIT: UnitVocab = { singular: "cluster", plural: "clusters", letter: "c", first: 2 };

/** Region colours by kind, with FAT's one exception: the mirror copy ("FAT 1") gets its own
 *  lighter violet so "the FAT is written twice" is visible in the ribbon and the dump's
 *  owner stripe. */
export function fatColorForRegion(region: Region): ColorIndex {
  switch (region.kind) {
    case "allocationTable": return region.name === "FAT 1" ? COLOR_FAT_ALT : COLOR_FAT;
    case "directory": return COLOR_DIR;
    case "data": return COLOR_FREE;
    default: return COLOR_BOOT;
  }
}

/** Pure cluster arithmetic over one geometry; buildable from a fixture without a Volume. */
export function fat16Space(g: Geometry): UnitSpace {
  return {
    unit: FAT_UNIT,
    unitCount: g.clusterCount,
    unitSize: g.bytesPerSector * g.sectorsPerCluster,
    sectorSize: g.bytesPerSector,
    totalSectors: g.totalSectors,
    unitOfSector: (sector) => clusterOfSector(g, sector),
    unitByteRange: (unit) => clusterByteRange(g, unit),
    unitOfOffset: (offset) => clusterOfSector(g, Math.floor(offset / g.bytesPerSector)),
    // The dump labels a row "cluster N" only on the first sector of a data cluster.
    unitStartsAt: (sector) => clusterOfSector(g, sector) !== undefined && (sector - g.firstDataSector) % g.sectorsPerCluster === 0,
    colorForRegion: fatColorForRegion,
  };
}
```

- [ ] **Step 7: Move fatchain.ts and add describeFatEntry**

Run: `git mv web/ui/src/core/fatchain.ts web/ui/src/fs/fat16/fatchain.ts`

Then replace the whole file with the following (line 1's path changes; `describeFatEntry` is `Inspector.svelte`'s `describe`, lines 30-38, moved):

```ts
import type { FatEntry } from "../../lib/wasm";

export type ClusterState = "free" | "used" | "end" | "bad" | "reserved";

export function clusterState(e: FatEntry): ClusterState {
  switch (e.kind) {
    case "free": return "free";
    case "next": return "used";
    case "endOfChain": return "end";
    case "bad": return "bad";
    default: return "reserved";
  }
}

/** Clusters in chain order starting at `first`; [] if the start is not an allocated cluster. */
export function buildChain(fat: FatEntry[], first: number): number[] {
  const out: number[] = [];
  let c = first;
  while (c >= 2 && c < fat.length && out.length < fat.length) {
    const e = fat[c];
    if (e.kind !== "next" && e.kind !== "endOfChain") break;
    out.push(c);
    if (e.kind === "endOfChain") break;
    c = e.cluster;
  }
  return out;
}

/** One FAT entry in words, as the Inspector's "At this byte" card prints it after "FAT: ". */
export function describeFatEntry(e: FatEntry): string {
  switch (e.kind) {
    case "free": return "free";
    case "next": return `next → ${e.cluster}`;
    case "endOfChain": return "end of chain";
    case "bad": return "bad";
    case "reserved": return "reserved";
  }
}
```

- [ ] **Step 8: Move direntry.ts**

Run: `git mv web/ui/src/core/direntry.ts web/ui/src/fs/fat16/direntry.ts`

Then replace the whole file with the following (only lines 1-3, the imports, differ from today's file):

```ts
import type { ClusterOwner, FatEntry, Geometry, RawEntry, Volume } from "../../lib/wasm";
import { clusterByteRange } from "./geometry";
import { buildChain } from "./fatchain";

const ENTRY = 32;

/**
 * Walks raw directory entries, accumulating the preceding LFN text (if any) for
 * each non-`lfn` entry so callers don't have to reimplement VFAT's
 * last-entry-first-on-disk assembly. `cb` is called once per non-`lfn` entry with
 * its index, the entry itself, the accumulated long name ("" if the entry has no
 * LFN), and the index of the first slot of the group (the first LFN entry, or the
 * entry's own index when there is no LFN) — the same "first" used for
 * `slotOffset` ranges. Stops at a `free` entry (end of the directory), or early
 * when `cb` returns `true`.
 */
export function walkEntries(entries: RawEntry[], cb: (i: number, e: RawEntry, longName: string, firstIndex: number) => boolean | void): void {
  let lfnText = "", lfnStart = -1;
  for (let i = 0; i < entries.length; i++) {
    const e = entries[i];
    if (e.kind === "lfn") { if (e.isLast) { lfnText = ""; lfnStart = i; } lfnText = e.text + lfnText; continue; }
    const firstIndex = lfnStart >= 0 && lfnText ? lfnStart : i;
    const stop = cb(i, e, lfnText, firstIndex);
    if (e.kind === "free") break;
    lfnText = ""; lfnStart = -1;
    if (stop) break;
  }
}

export function findEntrySlots(vol: Volume, g: Geometry, fat: FatEntry[], owners: ClusterOwner[], path: string): { start: number; end: number } | null {
  if (path === "/") return null;
  const cut = path.lastIndexOf("/");
  const parent = cut === 0 ? "/" : path.slice(0, cut);
  const name = path.slice(cut + 1).toUpperCase();
  let entries: RawEntry[];
  // Deliberately broad: this runs inside a `$derived` behind a selection that can be stale
  // (NotFound right after a delete) or unreadable (CorruptImage after a raw write), and a
  // throw there would take the whole render down. "No slots" is the right answer for both.
  // See tests/corruption.test.ts.
  try { entries = vol.rawDirEntries(parent); } catch { return null; }
  let result: { start: number; end: number } | null = null;
  walkEntries(entries, (i, e, longName, firstIndex) => {
    if (e.kind !== "short") return false;
    const matches = e.name.toUpperCase() === name || longName.toUpperCase() === name;
    if (!matches) return false;
    const start = slotOffset(g, fat, owners, parent, firstIndex);
    const last = slotOffset(g, fat, owners, parent, i);
    // A slot past the end of the parent's cluster chain has no byte range (a
    // truncated or corrupt directory); report "no range" rather than a bogus one.
    if (start < 0 || last < 0) return true;
    result = { start, end: last + ENTRY };
    return true;
  });
  return result;
}

/** Byte offset of directory slot `slot` in `dir`, or -1 when the slot falls past
 *  the end of that directory's cluster chain. */
export function slotOffset(g: Geometry, fat: FatEntry[], owners: ClusterOwner[], dir: string, slot: number): number {
  if (dir === "/") return g.firstRootDirSector * g.bytesPerSector + slot * ENTRY;
  const owner = owners.find((o) => o.path === dir);
  const chain = owner ? buildChain(fat, owner.firstCluster) : [];
  const perCluster = (g.bytesPerSector * g.sectorsPerCluster) / ENTRY;
  const cluster = chain[Math.floor(slot / perCluster)];
  if (cluster === undefined) return -1;
  return clusterByteRange(g, cluster).start + (slot % perCluster) * ENTRY;
}
```

- [ ] **Step 9: Move remnants.ts**

Run: `git mv web/ui/src/core/remnants.ts web/ui/src/fs/fat16/remnants.ts`

Then replace the whole file with the following (lines 1-4, the imports, and the file name in the comment at line 23 differ from today's file):

```ts
import type { ClusterOwner, FatEntry, Geometry, Volume } from "../../lib/wasm";
import { clusterByteRange } from "./geometry";
import { slotOffset, walkEntries } from "./direntry";
import { normalize, type Interval } from "../../core/intervals";

const ENTRY = 32;
const MAX_DEPTH = 64;

/** Byte ranges of deleted directory slots (root + every reachable subdirectory) and
 *  free clusters that still hold non-zero bytes (data left behind by a delete). */
export function findRemnants(vol: Volume, g: Geometry, fat: FatEntry[], owners: ClusterOwner[], zeros: Uint8Array): Interval[] {
  const out: Interval[] = [];

  // Guards against a corrupt/crafted image where a subdirectory entry's first
  // cluster points back at an ancestor (or itself): track visited first-clusters
  // so such a cycle is skipped rather than recursed into forever, with a depth
  // cap as a belt-and-braces backstop in case cluster resolution is ever fooled.
  const visited = new Set<number>();

  const walk = (dir: string, depth: number) => {
    if (depth > MAX_DEPTH) return;
    let entries;
    // Deliberately broad, as in fs/fat16/direntry.ts: this walk runs inside a `$derived`, and a
    // directory that has gone away (NotFound after a delete) or become unreadable
    // (CorruptImage after a raw write) must be skipped, not thrown from.
    // See tests/corruption.test.ts.
    try { entries = vol.rawDirEntries(dir); } catch { return; }
    walkEntries(entries, (i, e, longName) => {
      if (e.kind === "deleted") {
        const start = slotOffset(g, fat, owners, dir, i);
        if (start >= 0) out.push({ start, end: start + ENTRY }); // -1: slot past the chain's end
        return false;
      }
      if (e.kind === "short" && e.isDir && e.name !== "." && e.name !== "..") {
        const childName = longName || e.name;
        const childPath = dir === "/" ? `/${childName}` : `${dir}/${childName}`;
        const owner = owners.find((o) => o.path === childPath);
        const firstCluster = owner ? owner.firstCluster : e.firstCluster;
        if (firstCluster >= 2 && !visited.has(firstCluster)) {
          visited.add(firstCluster);
          walk(childPath, depth + 1);
        }
      }
      return false;
    });
  };
  walk("/", 0);

  const sectorsPerCluster = g.sectorsPerCluster;
  for (let c = 2; c <= g.clusterCount + 1; c++) {
    if (fat[c]?.kind !== "free") continue;
    const firstSector = clusterByteRange(g, c).start / g.bytesPerSector;
    let dirty = false;
    for (let s = firstSector; s < firstSector + sectorsPerCluster; s++) {
      if (zeros[s] === 0) { dirty = true; break; }
    }
    if (dirty) out.push(clusterByteRange(g, c));
  }

  return normalize(out);
}
```

- [ ] **Step 10: Leave one-line shims at the old paths**

Create `web/ui/src/core/fatchain.ts`:

```ts
export * from "../fs/fat16/fatchain";
```

Create `web/ui/src/core/direntry.ts`:

```ts
export * from "../fs/fat16/direntry";
```

Create `web/ui/src/core/remnants.ts`:

```ts
export * from "../fs/fat16/remnants";
```

(`shell/commands.ts`, `state/layers.svelte.ts`, `components/FatMap.svelte`, the four scenario files and the `corruption`, `integration` and `read-commands` tests keep importing from `core/`; Task 3 repoints them and deletes these three files.)

- [ ] **Step 11: Point the three moved suites at the new paths**

In `web/ui/tests/direntry.test.ts` (lines 3-4) replace:

```ts
import { clusterByteRange } from "../src/core/attribution";
import { findEntrySlots, slotOffset } from "../src/core/direntry";
```

with:

```ts
import { clusterByteRange } from "../src/fs/fat16/geometry";
import { findEntrySlots, slotOffset } from "../src/fs/fat16/direntry";
```

In `web/ui/tests/fatchain.test.ts` (line 2) replace:

```ts
import { buildChain, clusterState } from "../src/core/fatchain";
```

with:

```ts
import { buildChain, clusterState } from "../src/fs/fat16/fatchain";
```

In `web/ui/tests/remnants.test.ts` (lines 3-4) replace:

```ts
import { findRemnants } from "../src/core/remnants";
import { findEntrySlots } from "../src/core/direntry";
```

with:

```ts
import { findRemnants } from "../src/fs/fat16/remnants";
import { findEntrySlots } from "../src/fs/fat16/direntry";
```

and in the same file (line 6) replace:

```ts
import { clusterByteRange } from "../src/core/attribution";
```

with:

```ts
import { clusterByteRange } from "../src/fs/fat16/geometry";
```

(`clusterByteRange` is imported from `fs/fat16/geometry` rather than left on `core/attribution` so these two lines survive Task 3, which moves the function there for real.)

- [ ] **Step 12: Run every existing suite over the moved modules**

Run: `pnpm vitest run --exclude "tests/fs/**"`

Expected: `Test Files  34 passed (34)` and `Tests  268 passed (268)`, the same numbers as before this task: the moves and shims change nothing observable. (`tests/fs/**` is excluded only because its three files still cannot resolve `src/fs`.)

- [ ] **Step 13: Create the FAT metadata module**

Create `web/ui/src/fs/fat16/metadata.ts`. `CORRUPT_NOTE` is `DirTree.svelte` line 12's text; the two `NOTES` clauses are `commands.ts` line 484's parenthetical and `dd.ts` line 163's log line, strings unchanged.

```ts
import { BOOT_SECTOR_LEN, touchesBootSector } from "../../core/patch";
import type { FsAdapter } from "../adapter";

// Still defined in core/patch.ts until Task 3 moves the two here for real; this is
// already their FAT home for every importer.
export { BOOT_SECTOR_LEN, touchesBootSector };

/** DirTree's note while a raw write has left the boot sector unparsable. */
export const CORRUPT_NOTE = "Boot sector does not parse; the tree is unavailable until a raw write repairs it.";

/**
 * The FAT clauses the shell logs when a write had to rewrite a whole file, strings unchanged:
 * `write --append` logs `appended ${n} bytes by rewriting the whole file (${NOTES.rewrite})`,
 * and dd's `--seek` overlay logs `NOTES.partialWrite` as a line of its own.
 */
export const NOTES: FsAdapter["notes"] = {
  rewrite: "FAT has no append; the old chain is freed and reallocated",
  partialWrite: "FAT has no partial writes; the whole file was rewritten",
};
```

- [ ] **Step 14: Create the FAT format model**

Create `web/ui/src/fs/fat16/format.ts`. `SIZES`, `CLUSTER_SIZES`, the constants and `clusterCountFor` are `ActionsPanel.svelte` lines 6-15 and 31-41 copied (Task 5 deletes the copy in the component); `checkFormat` is the component's `clusters`/`clusterProblem` pair (lines 43-46) with `null` for "no problem"; `DEFAULTS` is the form's initial state (lines 25-27); `MKFS` is `commands.ts` lines 594-603 (the flags) with `summary` from line 596 and `done` from line 629.

```ts
import type { FormatOptions } from "../../lib/wasm";
import type { MkfsFlag, MkfsSpec } from "../adapter";

/** The Format form's disk sizes. */
export const SIZES: { label: string; totalSectors: number }[] = [
  { label: "4 MB", totalSectors: 8192 },
  { label: "16 MB", totalSectors: 32768 },
  { label: "64 MB", totalSectors: 131072 },
];
export const CLUSTER_SIZES = [1, 2, 4, 8];

// FAT16 geometry constants, matching the core's formatter.
export const BYTES_PER_SECTOR = 512, ROOT_ENTRIES = 512, DIR_ENTRY = 32, RESERVED = 1, FAT_COPIES = 2;
export const FAT16_MIN_CLUSTERS = 4085, FAT16_MAX_CLUSTERS = 65524;

/** The cluster count these options would produce, by the core's rule: the smallest
 *  sectors-per-FAT that can index every cluster the leftover space yields. */
export function clusterCountFor(total: number, spc: number): number {
  const rootDirSectors = Math.ceil((ROOT_ENTRIES * DIR_ENTRY) / BYTES_PER_SECTOR);
  const entriesPerFatSector = BYTES_PER_SECTOR / 2; // FAT16 entries are 2 bytes
  for (let spf = 1; spf <= total; spf++) {
    const usable = total - RESERVED - FAT_COPIES * spf - rootDirSectors;
    if (usable <= 0) return 0;
    const clusters = Math.floor(usable / spc);
    if (spf * entriesPerFatSector >= clusters + 2) return clusters;
  }
  return 0;
}

/** The Format form's initial values: the mounted default disk (16 MB, 4 sectors per cluster,
 *  no label), so opening the form shows the geometry that is already on screen. */
export const DEFAULTS: Required<Pick<FormatOptions, "totalSectors" | "sectorsPerCluster" | "volumeLabel">> = {
  totalSectors: 32768,
  sectorsPerCluster: 4,
  volumeLabel: "",
};

/** The cluster count the options would give and, when it is outside the FAT16 range, the
 *  warning the Format form shows beside it (and disables the button on). Absent options
 *  take the form's defaults. */
export function checkFormat(o: Pick<FormatOptions, "totalSectors" | "sectorsPerCluster">): { clusters: number; problem: string | null } {
  const clusters = clusterCountFor(o.totalSectors ?? DEFAULTS.totalSectors, o.sectorsPerCluster ?? DEFAULTS.sectorsPerCluster);
  const problem = clusters < FAT16_MIN_CLUSTERS ? "too few for FAT16" : clusters > FAT16_MAX_CLUSTERS ? "too many for FAT16" : null;
  return { clusters, problem };
}

// Typed against the wasm options so a flag cannot name a key the core would reject.
const MKFS_FLAGS: (MkfsFlag & { option: keyof FormatOptions })[] = [
  { long: "sectors", desc: "total sectors (default 32768 = 16 MB)", kind: "int", option: "totalSectors" },
  { long: "spc", desc: "sectors per cluster (default 4)", kind: "int", option: "sectorsPerCluster" },
  { long: "label", desc: "volume label, up to 11 characters", kind: "str", option: "volumeLabel" },
  { long: "root-entries", desc: "root directory entries (default 512)", kind: "int", option: "rootEntries" },
  { long: "fats", desc: "FAT copies (default 2)", kind: "int", option: "fatCount" },
  { long: "reserved", desc: "reserved sectors (default 1)", kind: "int", option: "reservedSectors" },
];

/** The shell's `mkfs` for this family: its summary, its done line, and the six flags. */
export const MKFS: MkfsSpec = {
  summary: "Format /dev/hda as FAT16 (clears the timeline)",
  done: "formatted /dev/hda as FAT16; the timeline was cleared",
  flags: MKFS_FLAGS,
};
```

- [ ] **Step 15: Create the FAT adapter class and the family object**

Create `web/ui/src/fs/fat16/adapter.ts`. `refresh()` makes the three calls of `volume.svelte.ts` lines 44-45 and 34; `stat()` is `commands.ts` lines 267-286 with its `hexAddr` (line 64) and the geometry taken from the cache; `df()` is lines 298-304; `trace()` is `Inspector.svelte` lines 68-76's three rows with the `firstCluster`, `fatEntryOffset` and `dataOffset` deriveds of lines 24-28; `namesMatch` is `vfs.ts` line 74. The `fat16` family object sits in this file rather than `index.ts` because the class and the object refer to each other (`bind` news the class; the class's `family` is the object): a cycle between two modules is resolved by vite-node, the vitest runner, to an exports object that never receives the late binding, and `family` comes back `undefined`. `index.ts` re-exports it, so the module surface is the spec's.

```ts
import { Volume, type Annotation, type ClusterOwner, type FatEntry, type FormatOptions, type Geometry, type Region, type RegionKind } from "../../lib/wasm";
import type { Interval } from "../../core/intervals";
import type { ColorIndex } from "../../core/palette";
import type { ByteChangeLike } from "../../core/patch";
import type { DfFacts, FsAdapter, FsFamily, FsFamilyId, StatFacts, TraceRow, UnitOwner, UnitSpace, UnitVocab } from "../adapter";
import { findEntrySlots } from "./direntry";
import { buildChain, describeFatEntry } from "./fatchain";
import { MKFS } from "./format";
import { fat16Space } from "./geometry";
import { CORRUPT_NOTE, NOTES, touchesBootSector } from "./metadata";
import { findRemnants } from "./remnants";

const hexAddr = (n: number): string => `0x${n.toString(16)}`;

/** wasm's row in the generic shape. The raw rows are kept too, for the helpers that take `ClusterOwner[]`. */
function toUnitOwner(o: ClusterOwner): UnitOwner {
  return { unit: o.cluster, path: o.path, isDir: o.isDir, firstUnit: o.firstCluster };
}

/**
 * FAT16 behind the adapter. `refresh()` makes the same three wasm calls the store's
 * `refreshMeta` made before the seam (`geometry`, `clusterOwners`, `fatEntries(0)`), unguarded,
 * so a corrupt volume behaves exactly as it did: the FAT crate answers from its last good
 * geometry, and the directory walks inside `entrySlots` and `remnants` absorb `CorruptImage`
 * themselves. Every cache is a plain field (see the reactivity rule in fs/adapter.ts).
 */
export class Fat16Adapter implements FsAdapter {
  readonly id: FsFamilyId = "fat16";
  readonly name = "FAT16";
  readonly family: FsFamily<FormatOptions> = fat16;
  readonly vol: Volume;
  /** Every entry of FAT 0, indexed by cluster; FatMap paints from it. */
  fat: FatEntry[] = [];
  /** The owner rows in the generic shape, in cluster order (wasm returns them sorted). */
  owners: UnitOwner[] = [];
  readonly corruptNote = CORRUPT_NOTE;
  readonly notes = NOTES;
  private geo!: Geometry;
  private space!: UnitSpace;
  private rawOwners: ClusterOwner[] = [];

  constructor(vol: Volume) {
    this.vol = vol;
    this.refresh();
  }

  refresh(): void {
    this.geo = this.vol.geometry();
    this.space = fat16Space(this.geo);
    this.rawOwners = this.vol.clusterOwners();
    this.owners = this.rawOwners.map(toUnitOwner);
    this.fat = this.vol.fatEntries(0);
  }

  // UnitSpace, over the geometry the last refresh() read.
  get unit(): UnitVocab { return this.space.unit; }
  get unitCount(): number { return this.space.unitCount; }
  get unitSize(): number { return this.space.unitSize; }
  get sectorSize(): number { return this.space.sectorSize; }
  get totalSectors(): number { return this.space.totalSectors; }
  unitOfSector(sector: number): number | undefined { return this.space.unitOfSector(sector); }
  unitByteRange(unit: number): Interval { return this.space.unitByteRange(unit); }
  unitOfOffset(offset: number): number | undefined { return this.space.unitOfOffset(offset); }
  unitStartsAt(sector: number): boolean { return this.space.unitStartsAt(sector); }
  colorForRegion(region: Region): ColorIndex { return this.space.colorForRegion(region); }

  ownerOf(path: string): UnitOwner | undefined {
    return this.owners.find((o) => o.path === path);
  }

  chain(path: string): number[] {
    const owner = this.ownerOf(path);
    return owner ? buildChain(this.fat, owner.firstUnit) : [];
  }

  entrySlots(path: string): Interval | null {
    return findEntrySlots(this.vol, this.geo, this.fat, this.rawOwners, path);
  }

  remnants(zeros: Uint8Array): Interval[] {
    return findRemnants(this.vol, this.geo, this.fat, this.rawOwners, zeros);
  }

  dataStart(path: string): number | null {
    if (path === "/") return this.geo.firstRootDirSector * this.geo.bytesPerSector;
    const owner = this.ownerOf(path);
    return owner ? this.unitByteRange(owner.firstUnit).start : null;
  }

  regionStart(kind: RegionKind): number | undefined {
    return this.vol.layout().find((r) => r.kind === kind)?.sectors.start;
  }

  /** Byte offset of `cluster`'s 16-bit entry in FAT copy `copy`; copy 0 is the one the explorer reads. */
  fatEntryOffset(cluster: number, copy = 0): number {
    const g = this.geo;
    return (g.reservedSectors + copy * g.sectorsPerFat) * g.bytesPerSector + cluster * 2;
  }

  /** The seven FAT facts `stat` prints after the generic ones, formatted as the shell always printed them. */
  stat(path: string): StatFacts {
    const first = this.ownerOf(path)?.firstUnit ?? 0;
    const chain = buildChain(this.fat, first);
    const slots = this.entrySlots(path);
    return {
      firstCluster: first,
      chain,
      clusters: chain.length,
      entryOffset: slots ? hexAddr(slots.start) : "",
      entrySlots: slots ? `${hexAddr(slots.start)}-${hexAddr(slots.end)}` : "",
      fatEntryOffset: first >= 2 ? hexAddr(this.fatEntryOffset(first)) : "",
      dataOffset: path === "/" ? hexAddr(this.geo.firstRootDirSector * this.geo.bytesPerSector) : first >= 2 ? hexAddr(this.unitByteRange(first).start) : "",
    };
  }

  /** Used clusters counted from FAT 0, as `df` always counted them. */
  df(): DfFacts {
    const g = this.geo, fat = this.fat;
    let used = 0;
    for (let c = 2; c < g.clusterCount + 2 && c < fat.length; c++) if (fat[c].kind !== "free") used++;
    return { unitSize: g.bytesPerSector * g.sectorsPerCluster, units: g.clusterCount, used, free: g.clusterCount - used };
  }

  /** The Inspector's "Selected file" rows: the directory entry, the FAT chain, the data. Wording unchanged. */
  trace(path: string): TraceRow[] {
    const rows: TraceRow[] = [];
    const entry = this.entrySlots(path);
    if (entry) rows.push({ label: `Directory entry · offset ${hexAddr(entry.start)}`, offset: entry.start });
    const chain = this.chain(path);
    const first = chain[0];
    if (first === undefined) {
      rows.push({ label: "Data · no data clusters", offset: null });
    } else {
      const fatEntry = this.fatEntryOffset(first);
      const data = this.unitByteRange(first).start;
      rows.push({ label: `FAT chain · ${chain.length} clusters starting at ${first} · FAT entry at ${hexAddr(fatEntry)}`, offset: fatEntry });
      rows.push({ label: `Data · cluster ${first} at ${hexAddr(data)}`, offset: data });
    }
    return rows;
  }

  describeUnit(unit: number): string | null {
    const e = this.fat[unit];
    return e ? `FAT: ${describeFatEntry(e)}` : null;
  }

  annotateSector(sector: number): Annotation[] {
    return this.vol.annotateSectorWith(sector, this.rawOwners);
  }

  /** FAT lookups are case-insensitive, so `/docs/n.txt` is `/DOCS/N.TXT` on disk. */
  namesMatch(a: string, b: string): boolean {
    return a.toUpperCase() === b.toUpperCase();
  }

  touchesMetadata(changes: ByteChangeLike[]): boolean {
    return touchesBootSector(changes);
  }
}

/**
 * The FAT16 family: the one place `Volume.formatFat16` is called, and the adapter it binds.
 * Defined beside the class rather than in index.ts because the two refer to each other
 * (`bind` news the class, the class's `family` is this object): keeping both in one module
 * avoids an import cycle, which vite-node (the vitest runner) resolves to an exports object
 * that never receives the late binding. index.ts re-exports it.
 */
export const fat16: FsFamily<FormatOptions> = {
  id: "fat16",
  name: "FAT16",
  format: (options) => Volume.formatFat16(options),
  bind: (vol) => new Fat16Adapter(vol),
  mkfs: MKFS,
};
```

- [ ] **Step 16: Create the fat16 module surface**

Create `web/ui/src/fs/fat16/index.ts`:

```ts
import type { FormatOptions } from "../../lib/wasm";
import type { FsAdapter } from "../adapter";
import { Fat16Adapter } from "./adapter";

export type Fat16FormatOptions = FormatOptions;

/** The FAT16 family: `fat16.format` is the one place `Volume.formatFat16` is called, and
 *  `fat16.bind` makes the adapter. It is defined beside the class (see adapter.ts). */
export { fat16, Fat16Adapter } from "./adapter";

/** The FAT adapter behind a generic one, for the few callers that need a FAT-only fact
 *  (FatMap's `fat`, the fundamentals scenario's `fatEntryOffset`). */
export function asFat16(fs: FsAdapter): Fat16Adapter {
  if (fs.id !== "fat16") throw new Error(`not a FAT16 adapter: ${fs.id}`);
  return fs as Fat16Adapter;
}

export { FAT_UNIT, clusterByteRange, clusterOfSector, fat16Space, fatColorForRegion } from "./geometry";
export { buildChain, clusterState, describeFatEntry, type ClusterState } from "./fatchain";
export { findEntrySlots, slotOffset, walkEntries } from "./direntry";
export { findRemnants } from "./remnants";
export { BOOT_SECTOR_LEN, CORRUPT_NOTE, NOTES, touchesBootSector } from "./metadata";
export { CLUSTER_SIZES, DEFAULTS, MKFS, SIZES, checkFormat, clusterCountFor } from "./format";
```

- [ ] **Step 17: Create the registry**

Create `web/ui/src/fs/index.ts`:

```ts
import type { Volume } from "../lib/wasm";
import type { FsAdapter, FsFamily, FsFamilyId } from "./adapter";
import { fat16 } from "./fat16";

/** Every registered family, by id. A new family is added here and in `fs/panels.ts`. */
export const FAMILIES: Record<FsFamilyId, FsFamily> = { fat16 };
export const DEFAULT_FAMILY: FsFamilyId = "fat16";

/** The family whose `name` is this `Volume.fsType()` string ("FAT16" -> "fat16"). Throws for a
 *  type no adapter handles; the store's `load` turns that into a status line like any other
 *  load failure. When FAT32 shares the FAT adapter, this is the one place to remap. */
export function familyIdOf(fsType: string): FsFamilyId {
  for (const family of Object.values(FAMILIES)) if (family.name === fsType) return family.id;
  throw new Error(`no adapter for ${fsType}`);
}

export function adapterFor(vol: Volume): FsAdapter {
  return FAMILIES[familyIdOf(vol.fsType())].bind(vol);
}
```

- [ ] **Step 18: Run the three new suites**

Run: `pnpm vitest run tests/fs`

Expected: `✓ tests/fs/registry.test.ts (3 tests)`, `✓ tests/fs/fat16-format.test.ts (5 tests)`, `✓ tests/fs/fat16.test.ts (20 tests)`, then `Test Files  3 passed (3)` and `Tests  28 passed (28)`.

- [ ] **Step 19: Add the `space` fixture**

In `web/ui/tests/fixtures/geometry.ts` replace line 1:

```ts
import type { Geometry, Region } from "../../src/lib/wasm";
```

with:

```ts
import type { Geometry, Region } from "../../src/lib/wasm";
import { fat16Space } from "../../src/fs/fat16/geometry";
```

and append after the closing `];` of `layout` (line 12):

```ts
/** The same disk as a `UnitSpace`, for tests of the generic core that need no Volume. */
export const space = fat16Space(geo);
```

(Nothing reads `space` yet; Task 3's `attribution`, `lesson` and `shell/addr` tests do. The import is the geometry module rather than the `fs/fat16` index so the fixture does not load the wasm package into the tests that use only `geo` and `layout`.)

- [ ] **Step 20: Run both gates**

Run: `pnpm test`

Expected: `Test Files  37 passed (37)` and `Tests  296 passed (296)`: the 268 of before plus the 28 new ones, with every existing assertion untouched.

Run: `pnpm build`

Expected: the svelte-check line `COMPLETED 417 FILES 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS` (404 files before this task, plus the seven new sources, the three moved files at their new paths, and the three new tests; the shims reuse the old paths), then `vite v6.4.3 building for production...` and `✓ built in` with no error.

- [ ] **Step 21: Confirm the change set is exactly this task's**

Run, from the repository root: `git status --short`

Expected, and nothing else:

```
RM web/ui/src/core/direntry.ts -> web/ui/src/fs/fat16/direntry.ts
RM web/ui/src/core/fatchain.ts -> web/ui/src/fs/fat16/fatchain.ts
RM web/ui/src/core/remnants.ts -> web/ui/src/fs/fat16/remnants.ts
 M web/ui/tests/direntry.test.ts
 M web/ui/tests/fatchain.test.ts
 M web/ui/tests/fixtures/geometry.ts
 M web/ui/tests/remnants.test.ts
?? web/ui/src/core/direntry.ts
?? web/ui/src/core/fatchain.ts
?? web/ui/src/core/remnants.ts
?? web/ui/src/fs/adapter.ts
?? web/ui/src/fs/fat16/adapter.ts
?? web/ui/src/fs/fat16/format.ts
?? web/ui/src/fs/fat16/geometry.ts
?? web/ui/src/fs/fat16/index.ts
?? web/ui/src/fs/fat16/metadata.ts
?? web/ui/src/fs/index.ts
?? web/ui/tests/fs/
```

(The three `RM` lines are the staged `git mv` renames whose new files were then rewritten; the `??` lines under `src/core/` are the shims re-created at the old paths. Anything else in the list means a file outside the task's scope was touched: revert it.)

- [ ] **Step 22: Commit**

```
git add web/ui/src/fs web/ui/src/core/direntry.ts web/ui/src/core/fatchain.ts web/ui/src/core/remnants.ts web/ui/tests/fs web/ui/tests/direntry.test.ts web/ui/tests/fatchain.test.ts web/ui/tests/remnants.test.ts web/ui/tests/fixtures/geometry.ts
git commit -m "feat(ui): the fs adapter seam and the FAT16 adapter behind it

Add src/fs/adapter.ts (UnitSpace, FsFamily, FsAdapter and the contracts they
carry), the registry in src/fs/index.ts (FAMILIES, DEFAULT_FAMILY, familyIdOf,
adapterFor), and src/fs/fat16/: Fat16Adapter with plain caches refreshed by
refresh(), the fat16 family, fat16Space and FAT_UNIT, describeFatEntry, the
corrupt note and rewrite clauses, and the Format model (SIZES, CLUSTER_SIZES,
clusterCountFor, checkFormat, DEFAULTS, MKFS). direntry.ts, fatchain.ts and
remnants.ts move under src/fs/fat16/ with one-line shims at their old paths so
no other source changes yet; the cluster arithmetic and boot-sector helpers are
re-exported from their new homes until Task 3 moves them. stat, df, the
Inspector trace and every shell string are reproduced byte for byte and pinned
by tests/fs/fat16.test.ts, registry.test.ts and fat16-format.test.ts."
```
### Task 3: web/ui: generic core over UnitSpace

Spec: `docs/superpowers/specs/2026-09-23-fs-adapter-design.md`, section 3's paragraph on `core/attribution.ts`, `core/palette.ts`, `core/tree.ts`, `core/lesson.ts`, and `core/patch.ts`, and section 6's lines for the `attribution`, `lesson`, `patch`, `integration`, and `corruption` tests and `tests/fixtures/geometry.ts`. After this task the four generic core modules know nothing about FAT: they take a `UnitSpace` and `UnitOwner` rows from Task 2's `src/fs/adapter.ts`, and the two cluster helpers and the boot-sector trigger live only under `src/fs/fat16/`. Every asserted string and colour stays the same; the only expectation edits are `cluster` to `unit` and `firstCluster` to `firstUnit`.

All commands run from `/Users/bsmall/dev/fs-emulator/web/ui`. Prerequisite: Task 2 is committed (`src/fs/adapter.ts`, `src/fs/index.ts`, `src/fs/fat16/{geometry,metadata,direntry,remnants,fatchain,format,adapter,index}.ts`, `tests/fs/*.test.ts` exist and `pnpm test` is green). Line numbers below are HEAD `81b0b56` for files Task 2 did not touch. Task 2's output is fully specified and this task builds on exactly that shape: `src/fs/fat16/geometry.ts` imports `clusterByteRange` and `clusterOfSector` from `../../core/attribution` and re-exports them with an `export { ... }` statement (Task 2 Step 6); `src/fs/fat16/metadata.ts` imports and re-exports `BOOT_SECTOR_LEN` and `touchesBootSector` from `../../core/patch` the same way (Task 2 Step 13); the moved `direntry.ts` and `remnants.ts` already import `clusterByteRange` from `./geometry` (Task 2 Steps 8 and 9) and `adapter.ts` imports `touchesBootSector` from `./metadata` (Step 15); `tests/{direntry,fatchain,remnants}.test.ts` already import from `fs/fat16` (Step 11); and `tests/fixtures/geometry.ts` already exports `space` (Step 19). Each step that edits one of Task 2's files greps it first, so a drift from that shape is caught before the edit rather than papered over.

**Files:**

- Modify: `web/ui/src/core/attribution.ts` (whole file, lines 1-71: `Attr.cluster`, `AttributionTable.geometry/owners/ownerByCluster/colorByCluster`, `clusterOfSector` 16-20, `clusterByteRange` 22-26, `buildAttribution` 28-39, the private `colorForRegion` 46-55, `attrAtSector` 57-67, `attrAtOffset` 69-71)
- Modify: `web/ui/src/core/palette.ts` (lines 3-6: `COLOR_FAT`, `COLOR_FAT_ALT`)
- Modify: `web/ui/src/core/tree.ts` (whole file, lines 1-21)
- Modify: `web/ui/src/core/lesson.ts` (whole file, lines 1-34)
- Modify: `web/ui/src/core/patch.ts` (delete lines 27-35, `BOOT_SECTOR_LEN` and `touchesBootSector`; lines 1-26 stay)
- Modify: `web/ui/src/fs/fat16/geometry.ts` (Task 2's file: its `core/attribution` import, the `export { clusterByteRange, clusterOfSector };` re-export with its two comment lines, the palette import, and `fatColorForRegion`; `FAT_UNIT` and `fat16Space` are not touched)
- Modify: `web/ui/src/fs/fat16/metadata.ts` (Task 2's file: the `import { BOOT_SECTOR_LEN, touchesBootSector } from "../../core/patch";` line and the `export { BOOT_SECTOR_LEN, touchesBootSector };` line with the comment between them, replaced by the two definitions; `CORRUPT_NOTE` and `NOTES` are not touched)
- Check only, no edit expected: `web/ui/src/fs/fat16/direntry.ts`, `web/ui/src/fs/fat16/remnants.ts` (Task 2 already imports `clusterByteRange` from `./geometry` in both) and `web/ui/src/fs/fat16/adapter.ts` (imports `touchesBootSector` from `./metadata` and no cluster helper); Steps 7 and 12's greps confirm it
- Modify: `web/ui/src/shell/commands.ts` (line 3 only, `import { clusterByteRange } from "../core/attribution";`: an import path; `tests/shell/*.test.ts` load this file under vitest, so it must keep resolving)
- Test: `web/ui/tests/attribution.test.ts` (whole file, 43 lines)
- Test: `web/ui/tests/lesson.test.ts` (whole file, 56 lines)
- Test: `web/ui/tests/patch.test.ts` (line 2)
- Test: `web/ui/tests/integration.test.ts` (whole file, 130 lines)
- Test: `web/ui/tests/corruption.test.ts` (lines 4-5 import paths, from the `core/direntry` and `core/remnants` shims to `fs/fat16`; a new `adapterFor` import after line 3; `corruptFixture` lines 18 and 24; lines 29-30; line 48)
- Test: `web/ui/tests/shell/read-commands.test.ts` (line 2): import path only. `tests/direntry.test.ts` line 3 and `tests/remnants.test.ts` line 6 already import `clusterByteRange` from `fs/fat16/geometry` (Task 2 Step 11) and are not touched.
- Check only, no edit: `web/ui/tests/fixtures/geometry.ts` (Task 2 Step 19 already added `space`; the spec's section 6 assigns that change once) and `web/ui/tests/fs/fat16.test.ts` (Task 2 pins the region colours as the literals `2` and `10`, so the palette rename does not reach it)

**Interfaces:**

Consumes (Task 2, `web/ui/src/fs/adapter.ts`, the spec's section 1 verbatim):

```ts
export interface UnitVocab { singular: string; plural: string; letter: string; first: number }
export interface UnitOwner { unit: number; path: string; isDir: boolean; firstUnit: number }
export interface UnitSpace {
  readonly unit: UnitVocab; readonly unitCount: number; readonly unitSize: number;
  readonly sectorSize: number; readonly totalSectors: number;
  unitOfSector(sector: number): number | undefined;
  unitByteRange(unit: number): Interval;
  unitOfOffset(offset: number): number | undefined;
  unitStartsAt(sector: number): boolean;
  colorForRegion(region: Region): ColorIndex;
}
export interface FsAdapter extends UnitSpace { readonly owners: readonly UnitOwner[]; refresh(): void; /* ... */ }
```

Consumes (Task 2, `web/ui/src/fs/index.ts`): `adapterFor(vol: Volume): FsAdapter`, whose `bind` reads the owner cache once, so `adapterFor(vol).owners` reflects every op run on `vol` before the call.

Consumes (Task 2, `web/ui/src/fs/fat16/geometry.ts`): `FAT_UNIT: UnitVocab` (`{ singular: "cluster", plural: "clusters", letter: "c", first: 2 }`), `fat16Space(g: Geometry): UnitSpace`, `fatColorForRegion(region: Region): ColorIndex` (the `"FAT 1"` rule). Until this task the file imports `clusterOfSector` and `clusterByteRange` from `core/attribution.ts` and re-exports them (Task 2 Step 6); Step 5 makes them live there.

Consumes (Task 2, `web/ui/src/fs/fat16/metadata.ts`): `CORRUPT_NOTE`, `NOTES`, and `BOOT_SECTOR_LEN`/`touchesBootSector`, which the file imports from `core/patch.ts` and re-exports (Task 2 Step 13); Step 12 makes them live there. Consumes (Task 2, `web/ui/src/fs/fat16/direntry.ts` and `remnants.ts`): `findEntrySlots(vol, g, fat, owners, path)` and `findRemnants(...)` with the HEAD signatures (raw `ClusterOwner[]`). Consumes (Task 2, `web/ui/tests/fixtures/geometry.ts`): `space = fat16Space(geo)`, the fixture disk as a `UnitSpace`.

Consumes (existing): `core/palette.ts` values `COLOR_FREE = 0, COLOR_BOOT = 1, COLOR_FAT = 2, COLOR_DIR = 3, COLOR_FILE_BASE = 4, COLOR_FAT_ALT = 10`, `colorIndexForPath`; `core/patch.ts` `ByteChangeLike`, `applyChanges`, `changedSectors`; `core/corrupt.ts` `isCorrupt`; `lib/wasm` types `Region`, `RegionKind`, `Volume`, `Geometry` (the last only inside `src/fs/fat16/`); `state/scenarios.svelte.ts` `StepFocus` (still `cluster?: number` until Task 4 renames it to `unit?`).

Produces (exact names later tasks rely on):

```ts
// src/core/palette.ts (values and --own-N variables unchanged)
export const COLOR_FREE = 0, COLOR_BOOT = 1, COLOR_TABLE = 2, COLOR_DIR = 3, COLOR_FILE_BASE = 4;
export const COLOR_TABLE_ALT = 10;

// src/core/attribution.ts (no import from fs/fat16 or of any FAT wasm type)
export interface Attr { regionKind: RegionKind; regionName: string; sector: number; unit?: number; ownerPath?: string; isDir?: boolean; free: boolean; colorIndex: ColorIndex }
export interface AttributionTable { space: UnitSpace; regions: Region[]; owners: readonly UnitOwner[]; ownerByUnit: Int32Array; colorByUnit: Uint8Array }
export function buildAttribution(space: UnitSpace, layout: Region[], owners: readonly UnitOwner[]): AttributionTable   // tables have space.unit.first + space.unitCount rows
export function defaultColorForRegion(region: Region): ColorIndex   // by kind only
export function attrAtSector(t: AttributionTable, sector: number): Attr   // colour via t.space.colorForRegion, unit via t.space.unitOfSector
export function attrAtOffset(t: AttributionTable, offset: number): Attr   // sector = floor(offset / t.space.sectorSize)

// src/core/tree.ts
export interface TreeNode { name: string; path: string; isDir: boolean; size: number; firstUnit: number | null; children: TreeNode[] }   // null = no data
export function buildTree(vol: Volume, owners: readonly UnitOwner[]): TreeNode

// src/core/lesson.ts (reads focus.unit; output strings identical to today's)
export function describeFocus(focus: StepFocus | null | undefined, space: UnitSpace, regionNameAt: (sector: number) => string): string | null

// src/core/patch.ts keeps exactly: ByteChangeLike, applyChanges, changedSectors

// src/fs/fat16/geometry.ts (defined here, no longer in core)
export function clusterOfSector(g: Geometry, sector: number): number | undefined
export function clusterByteRange(g: Geometry, cluster: number): { start: number; end: number }

// src/fs/fat16/metadata.ts (defined here, no longer in core)
export const BOOT_SECTOR_LEN = 512
export function touchesBootSector(changes: ByteChangeLike[]): boolean
```

---

- [ ] **Step 1: Write the failing attribution test against the generic API**

Replace the whole of `tests/attribution.test.ts` with:

```ts
import { describe, expect, it } from "vitest";
import { attrAtOffset, attrAtSector, buildAttribution, defaultColorForRegion } from "../src/core/attribution";
import { COLOR_TABLE, COLOR_TABLE_ALT, colorIndexForPath } from "../src/core/palette";
import type { UnitOwner } from "../src/fs/adapter";
import { clusterByteRange, clusterOfSector } from "../src/fs/fat16/geometry";
import { geo, layout, space } from "./fixtures/geometry";

const owners: UnitOwner[] = [
  { unit: 2, path: "/DOCS", isDir: true, firstUnit: 2 },
  { unit: 3, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3 },
  { unit: 4, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3 },
];

describe("attribution", () => {
  const t = buildAttribution(space, layout, owners);
  it("maps sectors to clusters like the core", () => {
    expect(clusterOfSector(geo, 96)).toBeUndefined();
    expect(clusterOfSector(geo, 97)).toBe(2);
    expect(clusterOfSector(geo, 100)).toBe(2);
    expect(clusterOfSector(geo, 101)).toBe(3);
    expect(clusterOfSector(geo, 32764)).toBe(8168);
    expect(clusterOfSector(geo, 32767)).toBeUndefined(); // past the last full cluster
    expect(clusterByteRange(geo, 2)).toEqual({ start: 97 * 512, end: 97 * 512 + 2048 });
  });
  it("sizes its tables from the space: unit.first + unitCount rows, the FAT's clusterCount + 2", () => {
    expect(t.space).toBe(space);
    expect(t.ownerByUnit.length).toBe(geo.clusterCount + 2);
    expect(t.colorByUnit.length).toBe(geo.clusterCount + 2);
    expect(t.ownerByUnit[2]).toBe(0); // index into owners
    expect(t.ownerByUnit[5]).toBe(-1);
  });
  it("attributes metadata regions", () => {
    expect(attrAtSector(t, 0)).toMatchObject({ regionKind: "boot", colorIndex: 1, free: false });
    expect(attrAtSector(t, 1)).toMatchObject({ regionName: "FAT 0", colorIndex: 2 });
    expect(attrAtSector(t, 64)).toMatchObject({ regionName: "FAT 1", colorIndex: COLOR_TABLE_ALT }); // the mirror copy gets its own lighter hue
    expect(attrAtSector(t, 65)).toMatchObject({ regionKind: "directory", colorIndex: 3 });
  });
  it("colours a region by kind alone by default; the space adds what only the family knows", () => {
    const fat1 = layout[2];
    expect(fat1.name).toBe("FAT 1");
    expect(defaultColorForRegion(fat1)).toBe(COLOR_TABLE);
    expect(space.colorForRegion(fat1)).toBe(COLOR_TABLE_ALT);
    expect(layout.map((r) => defaultColorForRegion(r))).toEqual([1, 2, 2, 3, 0]); // boot, FAT 0, FAT 1, root directory, data
  });
  it("attributes data clusters to owners and free space", () => {
    expect(attrAtOffset(t, 97 * 512)).toMatchObject({ unit: 2, ownerPath: "/DOCS", isDir: true, colorIndex: 3, free: false });
    expect(attrAtOffset(t, 97 * 512)).not.toHaveProperty("cluster");
    expect(attrAtOffset(t, 101 * 512 + 7)).toMatchObject({ unit: 3, ownerPath: "/DOCS/N.TXT", isDir: false, colorIndex: colorIndexForPath("/DOCS/N.TXT") });
    expect(attrAtOffset(t, 105 * 512)).toMatchObject({ unit: 4, ownerPath: "/DOCS/N.TXT" });
    expect(attrAtOffset(t, 109 * 512)).toMatchObject({ unit: 5, free: true, colorIndex: 0 });
    expect(attrAtOffset(t, 109 * 512).ownerPath).toBeUndefined();
  });
  it("file hues are stable and in range", () => {
    const i = colorIndexForPath("/A.TXT");
    expect(i).toBeGreaterThanOrEqual(4);
    expect(i).toBeLessThanOrEqual(9);
    expect(colorIndexForPath("/A.TXT")).toBe(i);
  });
});
```

Every expected value is the one the file asserts at HEAD; the field `cluster` became `unit`, the owners are `UnitOwner` rows, `COLOR_FAT_ALT` became `COLOR_TABLE_ALT`, and two tests are new: the table size (`unit.first + unitCount`) and `defaultColorForRegion` against `space.colorForRegion`.

- [ ] **Step 2: Run it and confirm the failure**

Run: `pnpm exec vitest run tests/attribution.test.ts`

Expected: the file collects (`space` exists: Task 2 Step 19), and `Tests 4 failed | 2 passed (6)` against today's `Geometry`-based `buildAttribution`/`attrAtSector`. The `describe` body's `buildAttribution(space, layout, owners)` reads `space.clusterCount`, which a `UnitSpace` does not have, so `n` is `NaN`, both typed arrays are zero-length, and no owner row is stored; nothing throws. Then, per test:

- `maps sectors to clusters like the core` passes already: Task 2 re-exports the two helpers from `fs/fat16/geometry`.
- `sizes its tables from the space` fails at `expect(t.space).toBe(space)`: received `undefined` (today's table field is `geometry`).
- `attributes metadata regions` fails at sector 64: `colorIndex` is `10` (today's `COLOR_FAT_ALT`) but the expectation is `COLOR_TABLE_ALT`, which the palette does not export until Step 4, so `undefined` (vitest does not type-check, and a missing named export reads as `undefined`).
- `colours a region by kind alone by default` fails with `TypeError: defaultColorForRegion is not a function`: `core/attribution.ts` does not export it until Step 6.
- `attributes data clusters to owners and free space` fails at its first `toMatchObject`: today's `attrAtOffset` divides by `t.geometry.bytesPerSector`, `undefined` on a `UnitSpace`, so the sector is `NaN`, the region is `unknown`, and the result has no `unit` or `ownerPath`.
- `file hues are stable and in range` passes: it never reads the table.

- [ ] **Step 3: Confirm the fixture already has its `UnitSpace`**

Run: `grep -n "export const space = fat16Space(geo)" tests/fixtures/geometry.ts`

Expected: exactly one line. Task 2 Step 19 added the `fat16Space` import and this export, the spec's section 6 assigns the fixture change to that task, and this task does not edit the file. If the grep prints nothing, stop: Task 2 is not fully committed, and the prerequisite above is not met.

- [ ] **Step 4: Rename the palette's allocation-table constants**

In `src/core/palette.ts`, replace lines 3-6:

```ts
export const COLOR_FREE = 0, COLOR_BOOT = 1, COLOR_FAT = 2, COLOR_DIR = 3, COLOR_FILE_BASE = 4;
/** The second FAT copy: the same violet as `COLOR_FAT`, a shade lighter, so the two
 *  copies read as related but are told apart at a glance in the ribbon and dump. */
export const COLOR_FAT_ALT = 10;
```

with:

```ts
export const COLOR_FREE = 0, COLOR_BOOT = 1, COLOR_TABLE = 2, COLOR_DIR = 3, COLOR_FILE_BASE = 4;
/** A second copy of the allocation table (FAT's mirror): the same violet as `COLOR_TABLE`, a
 *  shade lighter, so the two copies read as related but are told apart at a glance in the
 *  ribbon and dump. */
export const COLOR_TABLE_ALT = 10;
```

Then find every remaining importer: `grep -rln "COLOR_FAT" src tests`

Expected: exactly `src/core/attribution.ts` (rewritten in Step 6) and `src/fs/fat16/geometry.ts` (Step 5); `tests/attribution.test.ts` no longer matches, and `tests/fs/fat16.test.ts` never did (Task 2 pins the mirror colour through `colorForRegion` as the literals `2` and `10`). No file under `src/components`, `src/state`, or `src/shell` names either constant (checked at HEAD: only `attribution.ts` did).

- [ ] **Step 5: `fs/fat16/geometry.ts` owns `clusterOfSector` and `clusterByteRange`**

Run: `grep -n "clusterOfSector\|clusterByteRange\|core/attribution\|core/palette\|COLOR_" src/fs/fat16/geometry.ts`

Expected, with the line numbers of Task 2 Step 6's file: line 2 `import { clusterByteRange, clusterOfSector } from "../../core/attribution";`, line 3 the palette import naming `COLOR_BOOT, COLOR_DIR, COLOR_FAT, COLOR_FAT_ALT, COLOR_FREE, type ColorIndex`, line 8 `export { clusterByteRange, clusterOfSector };` (the two helpers still live in `core/attribution.ts`; this line is their FAT-side alias, with a two-line comment above it that begins `// The cluster arithmetic still lives in core/attribution.ts until Task 3 moves the two`), the `COLOR_FAT_ALT : COLOR_FAT` return inside `fatColorForRegion`, and the `clusterOfSector(g, ...)`/`clusterByteRange(g, ...)` calls inside `fat16Space`. Do not change `FAT_UNIT` or `fat16Space`: they are Task 2's and `tests/fs/fat16.test.ts` pins them.

1. Replace the `"../../core/attribution"` import line with

   ```ts
   import { defaultColorForRegion } from "../../core/attribution";
   ```

   Then delete the `export { clusterByteRange, clusterOfSector };` statement and the two comment lines above it (`// The cluster arithmetic still lives in core/attribution.ts until Task 3 moves the two` / `// functions here for real; this is already their FAT home for every importer.`). This deletion is not optional: a module with both `export function clusterOfSector` and `export { clusterOfSector }` does not compile (`tsc`: `TS2323 Cannot redeclare exported variable` and `TS2484 Export declaration conflicts with exported declaration`; vite-node: `Multiple exports with the same name`), and every vitest suite that loads `fs/fat16` would break with it. In their place, directly after the import block, add the two functions verbatim from `core/attribution.ts` lines 16-26 at HEAD:

   ```ts
   export function clusterOfSector(g: Geometry, sector: number): number | undefined {
     if (sector < g.firstDataSector) return undefined;
     const c = 2 + Math.floor((sector - g.firstDataSector) / g.sectorsPerCluster);
     return c <= g.clusterCount + 1 ? c : undefined;
   }

   export function clusterByteRange(g: Geometry, cluster: number): { start: number; end: number } {
     const size = g.bytesPerSector * g.sectorsPerCluster;
     const start = g.firstDataSector * g.bytesPerSector + (cluster - 2) * size;
     return { start, end: start + size };
   }
   ```

2. Make the palette import exactly

   ```ts
   import { COLOR_TABLE_ALT, type ColorIndex } from "../../core/palette";
   ```

   (every other palette name the file imported served `fatColorForRegion`, which the next point replaces). Keep the file's existing `import type { Geometry, Region } from "../../lib/wasm"` and its `UnitSpace`/`UnitVocab` type imports from `"../adapter"`.

3. Replace the whole `fatColorForRegion` function (its doc comment included) with

   ```ts
   /** FAT's one colour rule of its own: the mirror copy "FAT 1" gets a lighter violet so "the FAT
    *  is written twice" is visible in the ribbon and the dump's owner stripe. Everything else is
    *  coloured by kind. */
   export function fatColorForRegion(region: Region): ColorIndex {
     if (region.kind === "allocationTable" && region.name === "FAT 1") return COLOR_TABLE_ALT;
     return defaultColorForRegion(region);
   }
   ```

Run the grep again. Expected, in some order and with the file's own line numbers: the `defaultColorForRegion` import line, the palette import line, the two `export function cluster...` lines, the `COLOR_TABLE_ALT` return inside `fatColorForRegion`, and the `clusterOfSector(g, ...)`/`clusterByteRange(g, ...)` calls inside `fat16Space`. No line names `COLOR_FAT`, and no `export {` line remains.

- [ ] **Step 6: Rewrite `core/attribution.ts` over `UnitSpace`**

Replace the whole of `src/core/attribution.ts` with:

```ts
import type { Region, RegionKind } from "../lib/wasm";
import type { UnitOwner, UnitSpace } from "../fs/adapter";
import { COLOR_BOOT, COLOR_DIR, COLOR_FREE, COLOR_TABLE, colorIndexForPath, type ColorIndex } from "./palette";

export type { ColorIndex };
export interface Attr {
  regionKind: RegionKind; regionName: string; sector: number;
  unit?: number; ownerPath?: string; isDir?: boolean; free: boolean; colorIndex: ColorIndex;
}
/** Byte ownership for one epoch. `ownerByUnit` and `colorByUnit` are indexed by unit number and
 *  have `space.unit.first + space.unitCount` rows (FAT: `clusterCount + 2`), so the rows below
 *  `unit.first` are never owned. */
export interface AttributionTable {
  space: UnitSpace; regions: Region[]; owners: readonly UnitOwner[];
  /** unit -> index into owners, or -1 */
  ownerByUnit: Int32Array;
  colorByUnit: Uint8Array;
}

export function buildAttribution(space: UnitSpace, layout: Region[], owners: readonly UnitOwner[]): AttributionTable {
  const n = space.unit.first + space.unitCount;
  const ownerByUnit = new Int32Array(n).fill(-1);
  const colorByUnit = new Uint8Array(n);
  owners.forEach((o, i) => {
    if (o.unit < n) {
      ownerByUnit[o.unit] = i;
      colorByUnit[o.unit] = o.isDir ? COLOR_DIR : colorIndexForPath(o.path);
    }
  });
  return { space, regions: layout, owners, ownerByUnit, colorByUnit };
}

function regionOf(regions: Region[], sector: number): Region {
  for (const r of regions) if (sector >= r.sectors.start && sector < r.sectors.end) return r;
  return { name: "unknown", sectors: { start: sector, end: sector + 1 }, kind: "other" };
}

/** A region's colour from its kind alone. A family's `UnitSpace.colorForRegion` starts here and
 *  adds what only it knows (FAT: the mirror copy "FAT 1" gets `COLOR_TABLE_ALT`). */
export function defaultColorForRegion(region: Region): ColorIndex {
  switch (region.kind) {
    case "allocationTable": return COLOR_TABLE;
    case "directory": return COLOR_DIR;
    case "data": return COLOR_FREE;
    default: return COLOR_BOOT;
  }
}

export function attrAtSector(t: AttributionTable, sector: number): Attr {
  const region = regionOf(t.regions, sector);
  const base: Attr = { regionKind: region.kind, regionName: region.name, sector, free: false, colorIndex: t.space.colorForRegion(region) };
  if (region.kind !== "data") return base;
  const unit = t.space.unitOfSector(sector);
  if (unit === undefined) return { ...base, free: true };
  const idx = t.ownerByUnit[unit];
  if (idx < 0) return { ...base, unit, free: true, colorIndex: COLOR_FREE };
  const o = t.owners[idx];
  return { ...base, unit, ownerPath: o.path, isDir: o.isDir, free: false, colorIndex: t.colorByUnit[unit] };
}

export function attrAtOffset(t: AttributionTable, offset: number): Attr {
  return attrAtSector(t, Math.floor(offset / t.space.sectorSize));
}
```

What changed against HEAD: the two cluster functions are gone (Step 5 owns them), the `"FAT 1"` branch of the old private `colorForRegion` moved to `fatColorForRegion` and the rest became the exported `defaultColorForRegion`, `n` is `space.unit.first + space.unitCount` (`2 + clusterCount`, today's `geometry.clusterCount + 2`), and every `cluster` field or name is `unit`. The file imports nothing from `fs/fat16` and no FAT wasm type; `Region`/`RegionKind` are generic and allowed.

- [ ] **Step 7: Re-point every importer of the two cluster functions**

`core/attribution.ts` no longer exports `clusterOfSector` or `clusterByteRange`, and the node tests load `src/shell/commands.ts` (through every `tests/shell/*.test.ts`), so its import must move now, before any of those suites runs. Task 2 already did the rest: `src/fs/fat16/direntry.ts` and `remnants.ts` import `clusterByteRange` from `./geometry` (Task 2 Steps 8 and 9), `src/fs/fat16/adapter.ts` imports no cluster helper at all (it goes through `fat16Space`), and `tests/direntry.test.ts` line 3 and `tests/remnants.test.ts` line 6 import from `../src/fs/fat16/geometry` (Task 2 Step 11).

Run: `grep -rn "clusterByteRange\|clusterOfSector" src/fs src/shell src/scenarios tests | grep "core/attribution"`

Expected: exactly two lines. For each, change only the module path, nothing else on the line:

| file (HEAD line) | old path | new path |
|---|---|---|
| `src/shell/commands.ts` (3) | `"../core/attribution"` | `"../fs/fat16/geometry"` |
| `tests/shell/read-commands.test.ts` (2) | `"../../src/core/attribution"` | `"../../src/fs/fat16/geometry"` |

If the grep prints any other line, Task 2 is not committed as specified: re-point that line to `fs/fat16/geometry` the same way (`"./geometry"` from inside `src/fs/fat16/`) and say so in this task's report. `src/shell/commands.ts` is the one `src/` file outside `fs/` this task touches, and the `fs/fat16` import it now carries is exactly what Task 7's boundary test forbids in the shell; Task 4 removes it when `stat` moves to `host.adapter.stat(canon)`. `src/state/layers.svelte.ts` (1), `src/state/scenarios.svelte.ts` (2), and `src/components/{FatMap,Inspector,TreeNodeView}.svelte` also import `clusterByteRange` from `core/attribution` and are deliberately outside the grep: no node test loads them, Task 4 (stores and scenario runner) and Task 5 (components) rewrite those imports, and until then they are svelte-check errors.

Run the grep again. Expected: it prints nothing.

- [ ] **Step 8: Run the attribution test and confirm the palette rename is complete**

Run: `pnpm exec vitest run tests/attribution.test.ts && grep -rn "COLOR_FAT" src tests; echo "grep exit=$?"`

Expected: `tests/attribution.test.ts (6 tests)` passes; the grep prints nothing and `grep exit=1`.

- [ ] **Step 9: Write the failing lesson test over `focus.unit` and a `UnitSpace`**

Replace the whole of `tests/lesson.test.ts` with:

```ts
import { describe, expect, it } from "vitest";
import { describeFocus } from "../src/core/lesson";
import { layout, space } from "./fixtures/geometry";

/** The panel's `regionNameAt`, which reads the attribution table; the layout fixture is enough here. */
const regionNameAt = (sector: number): string =>
  layout.find((r) => sector >= r.sectors.start && sector < r.sectors.end)?.name ?? "unknown";

const describe_ = (focus: Parameters<typeof describeFocus>[0]) => describeFocus(focus, space, regionNameAt);

describe("describeFocus", () => {
  it("names the selected file", () => {
    expect(describe_({ path: "/DOCS/N.TXT" })).toBe("Files: /DOCS/N.TXT, its entry, chain, and clusters");
  });

  it("names a cluster with the byte offset it starts at", () => {
    // Cluster 2 is the first data cluster: sector 97 x 512 bytes. The noun comes from the space.
    expect(describe_({ unit: 2 })).toBe("Cluster 2 in the data region (offset 0xc200)");
  });

  it("names a sector with its region", () => {
    expect(describe_({ sector: 65 })).toBe("Sector 65, root directory");
  });

  it("names an offset with the region of the sector it lands in", () => {
    expect(describe_({ offset: 0x8200 })).toBe("Offset 0x8200 in root directory");
  });

  it("names one place only, the one the dump actually goes to", () => {
    // `applyFocus` jumps to offset, else sector, else unit; the card must say the same.
    expect(describe_({ offset: 0x200, sector: 5, unit: 9 })).toBe("Offset 0x200 in FAT 0");
    expect(describe_({ sector: 5, unit: 9 })).toBe("Sector 5, FAT 0");
  });

  it("mentions the remnant hatching and the strings overlay", () => {
    expect(describe_({ showRemnants: true })).toBe("Deleted entries are shown hatched");
    expect(describe_({ strings: true })).toBe("Printable strings are highlighted");
  });

  it("joins the file, the one place, and the overlays, in a fixed order, capitalising once", () => {
    expect(describe_({ strings: true, sector: 1, path: "/A.TXT", showRemnants: true })).toBe(
      "Files: /A.TXT, its entry, chain, and clusters; sector 1, FAT 0; deleted entries are shown hatched; printable strings are highlighted",
    );
    expect(describe_({ path: "/A.TXT", unit: 2, strings: true })).toBe(
      "Files: /A.TXT, its entry, chain, and clusters; cluster 2 in the data region (offset 0xc200); printable strings are highlighted",
    );
  });

  it("is null when there is nothing to point at", () => {
    expect(describe_(null)).toBeNull();
    expect(describe_(undefined)).toBeNull();
    expect(describe_({})).toBeNull();
    expect(describe_({ path: null })).toBeNull(); // clears the selection; nothing to look at
    expect(describe_({ showRemnants: false, strings: false })).toBeNull();
  });
});
```

Every expected string is HEAD's; `{ cluster: N }` became `{ unit: N }` and `geo` became `space`.

- [ ] **Step 10: Run it and confirm the failure**

Run: `pnpm exec vitest run tests/lesson.test.ts`

Expected: `Tests 4 failed | 4 passed (8)` against today's `describeFocus(focus, geo, ...)`: `names a cluster with the byte offset it starts at` gets `null` instead of `"Cluster 2 in the data region (offset 0xc200)"` (it reads `focus.cluster`, which is absent), and `names an offset with the region of the sector it lands in` gets `"Offset 0x8200 in unknown"` (it divides by `space.bytesPerSector`, which is `undefined`, so the sector is `NaN`); `names one place only` (`"Offset 0x200 in unknown"`) and `joins the file...` (the unit clause missing) fail the same two ways.

- [ ] **Step 11: Rewrite `core/lesson.ts` over `UnitSpace`**

Replace the whole of `src/core/lesson.ts` with:

```ts
import type { UnitSpace } from "../fs/adapter";
import type { StepFocus } from "../state/scenarios.svelte";

const hex = (n: number): string => "0x" + n.toString(16);

/**
 * The Lesson card's "Look at:" line: what the current scenario step pointed the UI at,
 * in words, so someone who looked away knows where to look back. `regionNameAt` comes
 * from the attribution table (`attrAtSector(...).regionName`), which the pure core
 * cannot reach on its own; `space` supplies the family's unit noun and byte arithmetic.
 *
 * Null means the step moved nothing worth naming (no focus at all, or a focus that only
 * cleared the selection), and the card leaves the line out.
 */
export function describeFocus(
  focus: StepFocus | null | undefined,
  space: UnitSpace,
  regionNameAt: (sector: number) => string,
): string | null {
  if (!focus) return null;
  const parts: string[] = [];
  if (typeof focus.path === "string") parts.push(`Files: ${focus.path}, its entry, chain, and clusters`);
  // One place, not three: `ScenarioRunner.applyFocus` jumps to offset, else sector, else
  // unit, so the card names whichever of them the dump actually went to.
  if (focus.offset !== undefined) parts.push(`offset ${hex(focus.offset)} in ${regionNameAt(Math.floor(focus.offset / space.sectorSize))}`);
  else if (focus.sector !== undefined) parts.push(`sector ${focus.sector}, ${regionNameAt(focus.sector)}`);
  else if (focus.unit !== undefined) parts.push(`${space.unit.singular} ${focus.unit} in the data region (offset ${hex(space.unitByteRange(focus.unit).start)})`);
  if (focus.showRemnants) parts.push("deleted entries are shown hatched");
  if (focus.strings) parts.push("printable strings are highlighted");
  if (!parts.length) return null;
  const s = parts.join("; ");
  return s[0].toUpperCase() + s.slice(1);
}
```

For FAT `space.unit.singular` is `"cluster"` and `space.unitByteRange(2).start` is `0xc200`, so the output is byte-identical; the `"Files: ..., its entry, chain, and clusters"` literal and `"in the data region"` stay as the spec says. `focus.unit` does not exist on today's `StepFocus` (`cluster?: number`); vitest does not type-check, and Task 4 renames the field, so this is one of the svelte-check errors Step 18 expects (`TS2339` three times here, `TS2353` four times in `tests/lesson.test.ts`).

Run: `pnpm exec vitest run tests/lesson.test.ts`

Expected: `tests/lesson.test.ts (8 tests)` passes.

- [ ] **Step 12: Move `BOOT_SECTOR_LEN` and `touchesBootSector` into `fs/fat16/metadata.ts` for real**

Replace the whole of `src/core/patch.ts` with its first 26 lines at HEAD (nothing else changes in them):

```ts
export interface ByteChangeLike { offset: number; before: Uint8Array; after: Uint8Array }

/** Apply an operation's byte changes to a cached image. Forward writes `after` in
 *  order; reverse writes `before` in reverse order, so reverse∘forward is the identity
 *  even when writes overlap. */
export function applyChanges(buf: Uint8Array, changes: ByteChangeLike[], direction: "forward" | "reverse"): void {
  if (direction === "forward") {
    for (const c of changes) buf.set(c.after, c.offset);
  } else {
    for (let i = changes.length - 1; i >= 0; i--) buf.set(changes[i].before, changes[i].offset);
  }
}

/** Sorted, de-duplicated sector numbers touched by the changes. An empty change
 *  (the core allows a zero-length raw write, even at the very end of the disk)
 *  touches no sector. */
export function changedSectors(changes: ByteChangeLike[], sectorSize: number): number[] {
  const set = new Set<number>();
  for (const c of changes) {
    if (c.after.length === 0) continue;
    const first = Math.floor(c.offset / sectorSize);
    const last = Math.floor((c.offset + c.after.length - 1) / sectorSize);
    for (let s = first; s <= last; s++) set.add(s);
  }
  return [...set].sort((a, b) => a - b);
}
```

Then: `grep -n "core/patch\|BOOT_SECTOR_LEN\|touchesBootSector" src/fs/fat16/metadata.ts`

Expected, with the line numbers of Task 2 Step 13's file: line 1 `import { BOOT_SECTOR_LEN, touchesBootSector } from "../../core/patch";` and line 6 `export { BOOT_SECTOR_LEN, touchesBootSector };`, with the two-line comment that begins `// Still defined in core/patch.ts until Task 3 moves the two here for real` on lines 4-5 between them (line 2 is `import type { FsAdapter } from "../adapter";`, which stays). Delete both the import line and the `export { ... }` line, plus the comment between them; a module with both `export const BOOT_SECTOR_LEN` and `export { BOOT_SECTOR_LEN }` fails to compile the same way Step 5 describes. Put the `import type` line below at the top of the file, above the `FsAdapter` import, and the two definitions directly after the import block, above `CORRUPT_NOTE`; keep `CORRUPT_NOTE` and `NOTES` exactly as Task 2 wrote them:

```ts
import type { ByteChangeLike } from "../../core/patch";

/** Bytes the FAT core re-parses as the boot sector after a raw write. */
export const BOOT_SECTOR_LEN = 512;

/** True when any change starts inside the boot sector. The core may then have adopted
 *  a new geometry, so the store must re-read the layout (`Fat16Adapter.touchesMetadata`). */
export function touchesBootSector(changes: ByteChangeLike[]): boolean {
  return changes.some((c) => c.offset < BOOT_SECTOR_LEN);
}
```

The file's first lines then read `import type { ByteChangeLike } from "../../core/patch";`, `import type { FsAdapter } from "../adapter";`, a blank line, and the `BOOT_SECTOR_LEN` doc comment. Run the grep again: expected, the `ByteChangeLike` import, `export const BOOT_SECTOR_LEN = 512;`, `export function touchesBootSector(`, and the `c.offset < BOOT_SECTOR_LEN` line; no `export {` line and no value import from `core/patch`.

Then: `grep -rn "touchesBootSector\|BOOT_SECTOR_LEN" src/fs src/shell src/scenarios tests | grep "core/patch"`

Expected: only `tests/patch.test.ts` line 2, which changes next. `src/fs/fat16/adapter.ts` already imports `touchesBootSector` from `"./metadata"` (Task 2 Step 15), so it does not print. `src/state/volume.svelte.ts` (3) also imports it from `core/patch` and is outside the grep on purpose: Task 4 replaces that call with `this.adapter.touchesMetadata(rec.changes)`.

Then change line 2 of `tests/patch.test.ts` from

```ts
import { applyChanges, changedSectors, touchesBootSector } from "../src/core/patch";
```

to

```ts
import { applyChanges, changedSectors } from "../src/core/patch";
import { touchesBootSector } from "../src/fs/fat16/metadata";
```

The two `describe` blocks stay as they are: `touchesBootSector` is a pure move, so the existing tests are its regression net.

Run: `pnpm exec vitest run tests/patch.test.ts tests/fs`

Expected: `tests/patch.test.ts (4 tests)` passes, and every file under `tests/fs` still passes (Task 2's `Fat16Adapter.touchesMetadata` resolves `touchesBootSector` from `./metadata`).

- [ ] **Step 13: Write the failing integration test with `adapterFor`, `unit`, and `firstUnit`**

Replace the whole of `tests/integration.test.ts` with:

```ts
import { describe, expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";
import { applyChanges, changedSectors } from "../src/core/patch";
import { attrAtOffset, buildAttribution } from "../src/core/attribution";
import { buildTree } from "../src/core/tree";
import { adapterFor } from "../src/fs";
import { findEntrySlots } from "../src/fs/fat16/direntry";
import { clusterByteRange } from "../src/fs/fat16/geometry";
import { touchesBootSector } from "../src/fs/fat16/metadata";
import type { OpRecord, Region, Volume as VolumeType } from "../src/lib/wasm";

describe("package integration", () => {
  it("seek round trip: the same loops VolumeStore.seek runs reproduce every step's image", () => {
    const vol = Volume.formatFat16(undefined);
    const fresh = Buffer.from(vol.image()); // the disk before any operation (cursor -1)
    const ops: ((v: VolumeType) => OpRecord)[] = [
      (v) => v.createFile("/A.TXT", new TextEncoder().encode("alpha")),
      (v) => v.createDir("/D"),
      (v) => v.writeRaw(43, new TextEncoder().encode("SHELLDISK  ")), // the volume label, inside sector 0
      (v) => v.createFile("/D/B.BIN", new Uint8Array(3000)),
      (v) => v.deleteFile("/A.TXT"),
    ];
    const history: OpRecord[] = [];
    const snaps: Buffer[] = [];
    for (const op of ops) { history.push(op(vol)); snaps.push(Buffer.from(vol.image())); }
    // The raw write journals one change for exactly the written range and lands in sector 0.
    expect(history[2].changes).toHaveLength(1);
    expect(history[2].changes[0].offset).toBe(43);
    expect(Array.from(history[2].changes[0].after)).toEqual(Array.from(new TextEncoder().encode("SHELLDISK  ")));
    expect(changedSectors(history[2].changes, 512)).toEqual([0]);
    expect(touchesBootSector(history[2].changes)).toBe(true);
    expect(history.filter((_, i) => i !== 2).some((r) => touchesBootSector(r.changes))).toBe(false);

    // The cached image the store patches, starting where the store's cursor is: the last step.
    const image = vol.image();
    let cursor = history.length - 1;
    const imageAt = (step: number) => (step === -1 ? fresh : snaps[step]);

    for (let step = cursor - 1; step >= -1; step--) {
      for (let i = cursor; i > step; i--) applyChanges(image, history[i].changes, "reverse"); // seek(), step < cursor
      cursor = step;
      expect(Buffer.compare(Buffer.from(image), imageAt(step)), `reverse to step ${step}`).toBe(0);
    }
    for (let step = 0; step < history.length; step++) {
      for (let i = cursor + 1; i <= step; i++) applyChanges(image, history[i].changes, "forward"); // seek(), step > cursor
      cursor = step;
      expect(Buffer.compare(Buffer.from(image), imageAt(step)), `forward to step ${step}`).toBe(0);
    }
    expect(Buffer.compare(Buffer.from(image), Buffer.from(vol.image()))).toBe(0);
  });
  it("patching the cached image from OpRecord.changes matches vol.image()", () => {
    const vol = Volume.formatFat16(undefined);
    const image = vol.image();
    const rec = vol.createFile("/Hello world.txt", new TextEncoder().encode("hello from the ui"));
    applyChanges(image, rec.changes, "forward");
    expect(Buffer.compare(Buffer.from(image), Buffer.from(vol.image()))).toBe(0);
    expect(changedSectors(rec.changes, 512)).toContain(97);
    const rec2 = vol.deleteFile("/Hello world.txt");
    applyChanges(image, rec2.changes, "forward");
    expect(Buffer.compare(Buffer.from(image), Buffer.from(vol.image()))).toBe(0);
    applyChanges(image, rec2.changes, "reverse");
    applyChanges(image, rec.changes, "reverse");
    expect(Buffer.compare(Buffer.from(image), Buffer.from(Volume.formatFat16(undefined).image()))).toBe(0);
  });
  it("attribution names the file that owns a cluster and the tree lists it", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createDir("/DOCS");
    vol.createFile("/DOCS/N.TXT", new Uint8Array(3000));
    const fs = adapterFor(vol); // bound after the ops, so its cached owners are current
    const t = buildAttribution(fs, vol.layout(), fs.owners);
    const { start } = fs.unitByteRange(3);
    expect(attrAtOffset(t, start)).toMatchObject({ unit: 3, ownerPath: "/DOCS/N.TXT", isDir: false });
    const tree = buildTree(vol, fs.owners);
    expect(tree.children[0]).toMatchObject({ name: "DOCS", path: "/DOCS", isDir: true, firstUnit: 2 });
    expect(tree.children[0].children[0]).toMatchObject({ name: "N.TXT", path: "/DOCS/N.TXT", size: 3000, firstUnit: 3 });
  });
  it("the tree reports null, not 0, for the root and for an empty file: neither owns a unit", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createFile("/EMPTY.TXT", new Uint8Array(0));
    const tree = buildTree(vol, adapterFor(vol).owners);
    expect(tree.firstUnit).toBeNull();
    expect(tree.children[0]).toMatchObject({ name: "EMPTY.TXT", size: 0, firstUnit: null });
  });
  it("finds a file's directory entry slots, including LFN entries, in root and subdirectories", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createFile("/My long name.txt", new Uint8Array(1));
    vol.createDir("/D");
    vol.createFile("/D/inner.txt", new Uint8Array(1));
    const g = vol.geometry(), fat = vol.fatEntries(0), owners = vol.clusterOwners();
    const root = g.firstRootDirSector * g.bytesPerSector;
    expect(findEntrySlots(vol, g, fat, owners, "/My long name.txt")).toEqual({ start: root, end: root + 3 * 32 });   // 2 LFN + short
    expect(findEntrySlots(vol, g, fat, owners, "/D")).toEqual({ start: root + 3 * 32, end: root + 4 * 32 });
    const dStart = clusterByteRange(g, owners.find((o) => o.path === "/D")!.firstCluster).start; // /D's own cluster
    expect(findEntrySlots(vol, g, fat, owners, "/D/inner.txt")).toEqual({ start: dStart + 2 * 32, end: dStart + 3 * 32 });   // after . and ..
    expect(findEntrySlots(vol, g, fat, owners, "/nope")).toBeNull();
    expect(findEntrySlots(vol, g, fat, owners, "/")).toBeNull();
  });
  it("a raw write that changes the root-entry count is adopted: geometry and layout follow", () => {
    // 16 root entries fill exactly one 512-byte sector; 32 fill two. VolumeStore.run re-reads
    // the layout whenever the adapter's touchesMetadata(rec.changes) (FAT: touchesBootSector)
    // is true; this replays the same wasm calls and checks that the volume reports the grown
    // root region.
    const vol = Volume.formatFat16({ rootEntries: 16 });
    const before = vol.geometry();
    const layoutBefore = vol.layout();
    expect(before.rootEntries).toBe(16);
    expect(before.rootDirSectors).toBe(1);

    const rec = vol.writeRaw(17, new Uint8Array([32, 0])); // BPB_RootEntCnt, u16 little-endian
    expect(rec.changes).toHaveLength(1);
    expect(rec.changes[0].offset).toBe(17);
    expect(Array.from(rec.changes[0].before)).toEqual([16, 0]);
    expect(Array.from(rec.changes[0].after)).toEqual([32, 0]);
    expect(touchesBootSector(rec.changes)).toBe(true);

    const after = vol.geometry();
    const layoutAfter = vol.layout();
    expect(after.rootEntries).toBe(32);
    expect(after.rootDirSectors).toBe(2);
    expect(after.firstRootDirSector).toBe(before.firstRootDirSector);
    expect(after.firstDataSector).toBe(before.firstDataSector + 1);
    expect(after.bytesPerSector).toBe(before.bytesPerSector);
    expect(after.totalSectors).toBe(before.totalSectors);

    const root = (l: Region[]) => l.find((r) => r.kind === "directory")!;
    const data = (l: Region[]) => l.find((r) => r.kind === "data")!;
    expect(root(layoutBefore).sectors.start).toBe(before.firstRootDirSector);
    expect(root(layoutBefore).sectors.end).toBe(before.firstDataSector);
    expect(root(layoutAfter).sectors.start).toBe(before.firstRootDirSector);
    expect(root(layoutAfter).sectors.end).toBe(before.firstDataSector + 1); // grew by one sector
    expect(data(layoutAfter).sectors.start).toBe(data(layoutBefore).sectors.start + 1);
    expect(data(layoutAfter).sectors.end).toBe(data(layoutBefore).sectors.end);

    // Path operations keep working on the adopted geometry, and an ordinary op
    // (root entry, FAT, data cluster) never lands in sector 0.
    expect(vol.listDir("/")).toEqual([]);
    const rec2 = vol.createFile("/A.TXT", new TextEncoder().encode("a"));
    expect(touchesBootSector(rec2.changes)).toBe(false);
    expect(vol.listDir("/").map((e) => e.name)).toEqual(["A.TXT"]);
  });
});
```

Against HEAD: import paths (`findEntrySlots`, `clusterByteRange`, `touchesBootSector` from `fs/fat16`, `adapterFor` from `fs`), the third test builds its table from `adapterFor(vol)` and its owners and expects `unit`/`firstUnit`, one new test pins `firstUnit: null`; every other line is unchanged. `vol.geometry()`, `vol.fatEntries(0)`, and `vol.clusterOwners()` stay: tests may poke FAT internals.

- [ ] **Step 14: Run it and confirm the failure**

Run: `pnpm exec vitest run tests/integration.test.ts`

Expected: `Tests 2 failed | 4 passed (6)`, both on the tree: `attribution names the file that owns a cluster and the tree lists it` at `expect(tree.children[0]).toMatchObject({ ..., firstUnit: 2 })` (today's node has `firstCluster: 0`, since `UnitOwner` rows carry no `firstCluster`, and no `firstUnit` at all), and `the tree reports null, not 0...` at `expect(tree.firstUnit).toBeNull()` (received `undefined`). The attribution assertions in the third test and the `findEntrySlots` test pass already (Steps 6 and 7).

- [ ] **Step 15: Rewrite `core/tree.ts` over `UnitOwner`**

Replace the whole of `src/core/tree.ts` with:

```ts
import type { Volume } from "../lib/wasm";
import type { UnitOwner } from "../fs/adapter";
import { isCorrupt } from "./corrupt";

export interface TreeNode { name: string; path: string; isDir: boolean; size: number; firstUnit: number | null; children: TreeNode[] }

/** Recursive listing; first units come from the owner map (`null` for the root and for an
 *  empty file, which own no unit). `listDir` is gated: while the volume is corrupt (a raw
 *  write left the on-disk metadata unparsable) it throws `CorruptImage`, and this returns an
 *  empty root instead of throwing, so a `$derived` reading it doesn't freeze on the last good tree. */
export function buildTree(vol: Volume, owners: readonly UnitOwner[]): TreeNode {
  const firstByPath = new Map<string, number>();
  for (const o of owners) if (!firstByPath.has(o.path)) firstByPath.set(o.path, o.firstUnit);
  const walk = (path: string): TreeNode[] =>
    vol.listDir(path).map((e) => {
      const childPath = path === "/" ? `/${e.name}` : `${path}/${e.name}`;
      return { name: e.name, path: childPath, isDir: e.isDir, size: e.size, firstUnit: firstByPath.get(childPath) ?? null, children: e.isDir ? walk(childPath) : [] };
    });
  let children: TreeNode[];
  try { children = walk("/"); } catch (e) { if (isCorrupt(e)) children = []; else throw e; }
  return { name: "/", path: "/", isDir: true, size: 0, firstUnit: null, children };
}
```

`firstCluster: 0` ("no data", which `TreeNodeView` tested as `< 2`) is now `firstUnit: null`; a path in the owner map always has a real first unit, so no information is lost. Task 5 changes `TreeNodeView.svelte` to `node.firstUnit !== null`.

Run: `pnpm exec vitest run tests/integration.test.ts`

Expected: `tests/integration.test.ts (6 tests)` passes.

- [ ] **Step 16: Point the corruption test at `fs/fat16` and its `buildTree` calls at `UnitOwner` rows**

`tests/corruption.test.ts` passes raw `ClusterOwner[]` to `buildTree` (lines 30 and 48 at HEAD). It still passes at runtime (`buildTree` reads only `path` and the now-absent `firstUnit`, and the tests assert names and emptiness), but its calls no longer type-check. Task 2 did not touch this file (its Step 11 re-points only the `direntry`, `fatchain` and `remnants` suites; its Step 3 ports the corruption round trip into `tests/fs/fat16.test.ts` without deleting this one), so the text below is HEAD's, and its lines 4-5 still reach `findEntrySlots` and `findRemnants` through the `core/direntry.ts` and `core/remnants.ts` shims. The spec's section 6 lists `corruption` under "import paths only": this step re-points those two lines, which makes this file the last `core/direntry`/`core/remnants` importer under `tests/` to go, so that Task 4 Step 21 finds no test importing `core/remnants` and deletes that shim, and Task 7's boundary scan never sees a shim that re-exports `fs/fat16` from `src/core/`.

Replace lines 4-5

```ts
import { findEntrySlots } from "../src/core/direntry";
import { findRemnants } from "../src/core/remnants";
```

with

```ts
import { findEntrySlots } from "../src/fs/fat16/direntry";
import { findRemnants } from "../src/fs/fat16/remnants";
```

(the moved modules export both names unchanged, with the HEAD signatures over raw `ClusterOwner[]`). After line 3 (`import { buildTree } from "../src/core/tree";`) add:

```ts
import { adapterFor } from "../src/fs";
```

In `corruptFixture`, replace

```ts
  const saved = vol.readRaw(0, 512);

  expect(vol.corruption()).toBeNull();
```

with

```ts
  const saved = vol.readRaw(0, 512);
  const fs = adapterFor(vol); // bound before the corrupting write: its cached owners are the good ones

  expect(vol.corruption()).toBeNull();
```

and

```ts
  return { vol, owners, fat, geo, zeros, saved };
```

with

```ts
  return { vol, fs, owners, fat, geo, zeros, saved };
```

In the first test, replace

```ts
    const { vol, owners } = corruptFixture();
    const tree = buildTree(vol, owners);
```

with

```ts
    const { vol, fs } = corruptFixture();
    const tree = buildTree(vol, fs.owners);
```

In `writing the saved boot sector back clears corruption() ...`, replace

```ts
    const tree = buildTree(vol, vol.clusterOwners());
```

with

```ts
    const tree = buildTree(vol, adapterFor(vol).owners);
```

`owners` stays in the fixture: `findEntrySlots` and `findRemnants` take raw `ClusterOwner[]`.

Run: `pnpm exec vitest run tests/corruption.test.ts`

Expected: `tests/corruption.test.ts (6 tests)` passes.

- [ ] **Step 17: Confirm nothing under test still reaches the moved helpers through `core/`**

Run:

```sh
grep -rn "core/attribution\|core/patch" src/fs src/shell src/scenarios tests | grep -v "^tests/attribution.test.ts\|^tests/lesson.test.ts\|^tests/patch.test.ts\|^tests/integration.test.ts"
```

Expected: every printed line names only these, which are correct and stay: `defaultColorForRegion` from `core/attribution` (`src/fs/fat16/geometry.ts`), and `ByteChangeLike`, `applyChanges`, or `changedSectors` from `core/patch` (`src/fs/adapter.ts`, `src/fs/fat16/metadata.ts`, `src/fs/fat16/adapter.ts`, which types `touchesMetadata` with `ByteChangeLike`, and `tests/shell/dd.test.ts` line 6, `applyChanges`). A line naming `clusterByteRange`, `clusterOfSector`, `touchesBootSector`, or `BOOT_SECTOR_LEN` means Step 7 or Step 12 missed it: re-point it as those steps say and rerun the grep.

Then confirm no test still goes through Task 2's three `core/` shims:

```sh
grep -rn "core/direntry\|core/fatchain\|core/remnants" tests
```

Expected: exactly one line, `tests/shell/read-commands.test.ts:3:import { findEntrySlots } from "../../src/core/direntry";`, which Task 4 Step 19 deletes together with that file's line 2 when `stat` computes its expectations from the adapter; in particular no line names `core/remnants` or `core/fatchain`. Task 2 Step 11 re-pointed `tests/{direntry,fatchain,remnants}.test.ts`, Step 13 above re-pointed `tests/integration.test.ts`, and Step 16 `tests/corruption.test.ts`. Any other printed line means one of those steps was skipped: re-point it to the matching `../src/fs/fat16/` module. (`src/shell/commands.ts`, `src/state/layers.svelte.ts`, `src/components/FatMap.svelte` and `src/scenarios/*.ts` still import the shims and are Task 4's, 5's and 6's; the shims themselves go in Task 4 Step 21 and Task 5 Step 14.)

- [ ] **Step 18: Run the whole suite; note the expected svelte-check failure**

Run: `pnpm test`

Expected: every test file passes with 0 failures: `Test Files 37 passed (37)` and `Tests 299 passed (299)`, Task 2's 296 plus 3 (two new attribution tests and one new integration test), over the HEAD suites, Task 2's `tests/fs/*.test.ts`, and this task's `attribution` (6), `lesson` (8), `patch` (4), `integration` (6), `corruption` (6). `tests/shell/read-commands.test.ts` still passes with its `stat`, `df`, and `seek` output untouched, and `tests/scenarios.test.ts` and `tests/fundamentals.test.ts` are unaffected (the scenario files import only `findEntrySlots` and a `Scenario` type).

Run: `pnpm build; git status --short`

Expected: `pnpm build` fails in `svelte-check` (`vite build` does not run). This is the failure global constraint 8 allows for this task, and **Task 5 is where `pnpm build` turns green again**. The errors are confined to these twelve files, all owned by Task 4 (stores, scenario runner) or Task 5 (components), plus the one `StepFocus` rename Task 4 makes: `src/state/volume.svelte.ts` (`buildAttribution(this.geometry, this.layout, this.owners)`; `touchesBootSector` is not exported from `../core/patch`), `src/state/layers.svelte.ts` and `src/state/scenarios.svelte.ts` (`clusterByteRange` is not exported from `../core/attribution`), `src/core/lesson.ts` (`TS2339: Property 'unit' does not exist on type 'StepFocus'`, three times) and `tests/lesson.test.ts` (`TS2353 ... 'unit' does not exist in type 'StepFocus'`, four times), and `src/components/DirTree.svelte`, `FatMap.svelte`, `HexView.svelte`, `Inspector.svelte`, `LessonPanel.svelte`, `Ribbon.svelte`, `TreeNodeView.svelte` (`attr.cluster`, `ownerByCluster`, `colorByCluster`, `clusterByteRange`, `node.firstCluster`, `buildTree(volume.vol, volume.owners)` with `ClusterOwner[]`, `describeFocus(..., volume.geometry, ...)`). No error may come from `src/core/attribution.ts`, `src/core/tree.ts`, `src/core/patch.ts`, `src/core/palette.ts`, `src/shell/commands.ts`, `src/fs/**`, or any test other than `tests/lesson.test.ts`; if one does, fix it in this task before committing.

`git status --short` lists exactly these fourteen files, all `M`: `src/core/attribution.ts`, `src/core/palette.ts`, `src/core/tree.ts`, `src/core/lesson.ts`, `src/core/patch.ts`, `src/fs/fat16/geometry.ts`, `src/fs/fat16/metadata.ts`, `src/shell/commands.ts`, `tests/attribution.test.ts`, `tests/lesson.test.ts`, `tests/patch.test.ts`, `tests/integration.test.ts`, `tests/corruption.test.ts`, and `tests/shell/read-commands.test.ts`. Nothing else under `src/fs`, `tests/fs`, or `tests/fixtures` changes: Task 2 already left those files in the shape this task builds on.

- [ ] **Step 19: Commit**

From the repository root:

```sh
cd /Users/bsmall/dev/fs-emulator
git add web/ui/src/core/attribution.ts web/ui/src/core/palette.ts web/ui/src/core/tree.ts web/ui/src/core/lesson.ts web/ui/src/core/patch.ts \
  web/ui/src/fs/fat16/geometry.ts web/ui/src/fs/fat16/metadata.ts web/ui/src/shell/commands.ts \
  web/ui/tests/attribution.test.ts web/ui/tests/lesson.test.ts web/ui/tests/patch.test.ts web/ui/tests/integration.test.ts \
  web/ui/tests/corruption.test.ts web/ui/tests/shell/read-commands.test.ts
git commit -m "refactor(ui): make attribution, tree, and lesson generic over UnitSpace

core/attribution.ts builds its tables from a UnitSpace and UnitOwner rows
(Attr.unit, ownerByUnit, colorByUnit) and colours regions through
space.colorForRegion, with defaultColorForRegion as the by-kind fallback;
core/tree.ts takes UnitOwner rows and reports firstUnit (null = no data);
core/lesson.ts describes focus.unit through the space's noun and byte
ranges. clusterOfSector, clusterByteRange, BOOT_SECTOR_LEN, and
touchesBootSector now live only under fs/fat16 (geometry.ts, metadata.ts),
and the palette's COLOR_FAT/COLOR_FAT_ALT are COLOR_TABLE/COLOR_TABLE_ALT
with the same values. Output strings and colours are unchanged. The stores
and components migrate in Tasks 4 and 5, so svelte-check is red until then
while vitest stays green."
```

The list is exactly Step 18's fourteen files; `git status --short` from Step 18 is the list to check against before committing, and a file it shows that is not named here means a step above drifted from Task 2's shape and the report should say so.

---
### Task 4: web/ui: store, layers, and the shell through the adapter

Spec section 3 (the `VolumeStore` split and its bullets, the reactivity sites in `layers`, the `shell/addr.ts`, `shell/host.ts`, `storeHost`, `commands.ts`, `dd.ts`, `vfs.ts`, and `errors.ts` paragraphs, and the minimal `state/scenarios.svelte.ts` change: `StepFocus.unit`, `volume.format(DEFAULT_FAMILY, ...)`, `unitByteRange`) plus the shell entries of section 6 (`shell/addr`, `shell/read-commands`, `shell/helpers.ts`, `shell/errors`, `shell/vfs`). Everything in this task is a call-site migration: no output string of the shell changes except `CORRUPT_HELP`, which section 3 rewords on purpose.

**Files:**
- Modify: `web/ui/src/state/volume.svelte.ts` (whole file, lines 1-117: imports 1-5, fields 8-26, `adopt` 31-41, `refreshMeta` 43-49, `format` 51-53, `run` 62-79, the `seek` doc comment 81-83)
- Modify: `web/ui/src/state/layers.svelte.ts` (whole file, lines 1-46: imports 1-8, `chain`/`entry`/`remnant`/`sel` 21-36)
- Modify: `web/ui/src/state/scenarios.svelte.ts` (imports 1-5, `StepFocus` 7-14, `start` 45-52, `applyStep` 93-97, `applyFocus` 101-111)
- Modify: `web/ui/src/scenarios/deleteRemnants.ts` (line 26), `web/ui/src/scenarios/directory.ts` (line 23), `web/ui/src/scenarios/fundamentals.ts` (lines 73 and 89), `web/ui/src/scenarios/shell.ts` (line 33), `web/ui/src/scenarios/smallFile.ts` (line 31); `fillDisk.ts`, `format.ts`, `longName.ts`, `overwriteGrows.ts`, `index.ts` have no focus `cluster:` key and are untouched
- Modify: `web/ui/src/shell/addr.ts` (whole file, lines 1-41)
- Modify: `web/ui/src/shell/host.ts` (imports 1-2, `ShellHost` 9-28, `selectPath` 46-50; Task 1's Step 14 already reworded the `corruptionOf` comment above it, adding a line, and that comment and function stay as Task 1 left them)
- Modify: `web/ui/src/shell/storeHost.svelte.ts` (import 1, the returned object 17-49)
- Modify: `web/ui/src/shell/commands.ts` (imports 1-12, `hexAddr` 64, `cd` 182, `stat` 228-290, `df` 292-320, `seek` 343-352, the `writeVolumeFile` doc 412-417, `write` 448-486, `cp` 579, `mkfs` 592-630, `xxdSpec` 661-670, the `xxd` offset flag 676-677)
- Modify: `web/ui/src/shell/dd.ts` (the `overlayVolumeFile` doc 140-144 and its log line 163)
- Modify: `web/ui/src/shell/vfs.ts` (import 1, `namesMatch` and `canonicalize` 74-98)
- Modify: `web/ui/src/shell/errors.ts` (`CORRUPT_HELP` and its doc, lines 20-26)
- Delete (conditional, Step 21): `web/ui/src/core/remnants.ts`, the re-export shim Task 2 left, once nothing outside `src/fs` imports it; `core/direntry.ts` stays for the four scenario files (Task 6) and `core/fatchain.ts` for `FatMap.svelte` (Task 5)
- Test: `web/ui/tests/shell/addr.test.ts` (whole file, lines 1-59), `web/ui/tests/shell/helpers.ts` (whole file, lines 1-120; only `TestHost` and its imports change), `web/ui/tests/shell/read-commands.test.ts` (imports 2-4, the `stat` fixture 160-179, the `seek` tests 225-240), `web/ui/tests/shell/errors.test.ts` (imports 2-4, the `stub` 61-64), `web/ui/tests/shell/vfs.test.ts` (imports 2-4, `canonicalize` 78-96); `tests/shell/dd.test.ts`, `mutations.test.ts`, `xxd-command.test.ts`, `redirect.test.ts` build their host with `makeHost()` and change nothing; `tests/scenarios.test.ts` and `tests/fundamentals.test.ts` assert no focus `cluster` key and change nothing; `tests/lesson.test.ts` already passes `{ unit: N }` after Task 3

**Interfaces:**
- Consumes (Task 2, `web/ui/src/fs/adapter.ts`): `FsFamilyId`, `UnitVocab { singular; plural; letter; first }`, `UnitOwner { unit; path; isDir; firstUnit }`, `UnitSpace { unit; unitCount; unitSize; sectorSize; totalSectors; unitByteRange(unit): Interval; ... }`, `FsFamily<O> { id; name; format(options?: O): Volume; bind(vol): FsAdapter; mkfs: MkfsSpec }`, `MkfsSpec { summary; done; flags: MkfsFlag[] }`, `MkfsFlag { long; desc; kind: "int" | "str"; option }`, `FsAdapter extends UnitSpace { id; name; family; vol; refresh(); owners; ownerOf(path); chain(path): number[]; entrySlots(path): Interval | null; remnants?(zeros): Interval[]; stat(path): StatFacts; df(): DfFacts; parseAddr?(v); namesMatch(a, b); touchesMetadata(changes); notes: { rewrite; partialWrite } }`, `DfFacts { unitSize; units; used; free }`.
- Consumes (Task 2, `web/ui/src/fs/index.ts`): `FAMILIES: Record<FsFamilyId, FsFamily>`, `DEFAULT_FAMILY: FsFamilyId`, `adapterFor(vol: Volume): FsAdapter`.
- Consumes (Task 2, `web/ui/src/fs/fat16/`): `Fat16Adapter.stat(path)` returning exactly `firstCluster`, `chain`, `clusters`, `entryOffset`, `entrySlots`, `fatEntryOffset`, `dataOffset` with today's `0x…` hex strings and `""` for absent; `Fat16Adapter.df()` counting used clusters from FAT 0 like today's `df` loop; `NOTES.rewrite === "FAT has no append; the old chain is freed and reallocated"` and `NOTES.partialWrite === "FAT has no partial writes; the whole file was rewritten"`; `MKFS.summary === "Format /dev/hda as FAT16 (clears the timeline)"`, `MKFS.done === "formatted /dev/hda as FAT16; the timeline was cleared"`, and `MKFS.flags` with today's six `long`/`desc` pairs (`sectors`, `spc`, `label`, `root-entries`, `fats`, `reserved`) and `option` keys `totalSectors`, `sectorsPerCluster`, `volumeLabel`, `rootEntries`, `fatCount`, `reservedSectors`; `FAT_UNIT === { singular: "cluster", plural: "clusters", letter: "c", first: 2 }`.
- Consumes (Task 3): `buildAttribution(space: UnitSpace, layout: Region[], owners: readonly UnitOwner[]): AttributionTable` from `core/attribution.ts`; `core/patch.ts` exporting only `applyChanges` and `changedSectors` (so `touchesBootSector` is gone); `describeFocus(focus, space: UnitSpace, regionNameAt)` in `core/lesson.ts` reading `focus.unit`; `export const space = fat16Space(geo)` in `web/ui/tests/fixtures/geometry.ts`.
- Produces:
  - `VolumeStore.adapter: FsAdapter` (`$state.raw`, re-bound by `adopt`, refreshed by `refreshMeta` after every op), `VolumeStore.sectorSize: number` and `VolumeStore.totalSectors: number` (`$state`, set in `adopt`), `VolumeStore.format(family: FsFamilyId = this.adapter.id, options?: unknown): void`, `VolumeStore.attribution` rebuilt behind an `epoch` read; the fields `geometry`, `owners`, `fat` and the FAT-typed `format(options?: FormatOptions)` are gone
  - `ShellHost.adapter: FsAdapter` and `ShellHost.format(family: FsFamilyId, options?: unknown): void` (`web/ui/src/shell/host.ts`), implemented by `createStoreHost` and `TestHost`
  - `AddrSpace = Pick<UnitSpace, "sectorSize" | "unit" | "unitByteRange"> & { parseAddr?(v: string): number | undefined }`, `parseAddr(v: string | number, space: AddrSpace): number`, `addrHelp(space: AddrSpace): string` (`web/ui/src/shell/addr.ts`; `ADDR_HELP` and `AddrGeometry` are gone; `parseSize` and `SIZE_HELP` unchanged)
  - `canonicalize(fs: Pick<FsAdapter, "vol" | "namesMatch">, volumePath: string): string` (`web/ui/src/shell/vfs.ts`)
  - `CORRUPT_HELP === "the on-disk metadata no longer parses; rewind on the timeline, or write the saved bytes back with: <the saved bytes> | dd --of=/dev/hda"`
  - `StepFocus.unit?: number` (was `cluster?`) in `web/ui/src/state/scenarios.svelte.ts`; `Step.action`, `Step.focus`, and `Step.format?: FormatOptions` keep today's shapes for Task 6
  - `TestHost.adapter: FsAdapter`, `TestHost.format(family: FsFamilyId, options?: unknown)`, `TestHost.formats: { family: FsFamilyId; options: unknown }[]` (`web/ui/tests/shell/helpers.ts`)

Gate commands (run from `web/ui`):

```
pnpm test
pnpm vitest run tests/shell
pnpm build
```

---

- [ ] **Step 1: Move `VolumeStore` onto the adapter**

Replace the whole of `web/ui/src/state/volume.svelte.ts` with:

```ts
import { Volume, type FsError, type OpRecord, type Region } from "../lib/wasm";
import { buildAttribution, type AttributionTable } from "../core/attribution";
import { applyChanges, changedSectors } from "../core/patch";
import { rescanSectors, scanZeroSectors } from "../core/zeros";
import { DEFAULT_FAMILY, FAMILIES, adapterFor } from "../fs";
import type { FsAdapter, FsFamilyId } from "../fs/adapter";
import { selection } from "./selection.svelte";

export class VolumeStore {
  vol = $state.raw<Volume>(FAMILIES[DEFAULT_FAMILY].format());
  /** The per-family view of `vol`: owners, tables, unit arithmetic. Its caches are plain
   *  fields, not runes, so a `$derived` that calls a method on it must read `epoch` first or
   *  it keeps answering from the previous op (the rule stated in fs/adapter.ts). `adopt`
   *  re-binds it for a new volume; `refreshMeta` re-reads it after every op. */
  adapter = $state.raw<FsAdapter>(adapterFor(this.vol));
  image = $state.raw<Uint8Array>(new Uint8Array(0));
  epoch = $state(0);
  layout = $state.raw<Region[]>(this.vol.layout());
  sectorSize = $state(this.vol.sectorSize());
  totalSectors = $state(this.vol.sectorCount());
  zeros = $state.raw<Uint8Array>(new Uint8Array(0));
  history = $state.raw<OpRecord[]>([]);
  cursor = $state(-1);
  status = $state<{ text: string; code?: string } | null>(null);
  /** The `CorruptImage` message while a raw write has left the on-disk metadata unparsable,
   *  `null` while mounted. Refreshed alongside the adapter, so it tracks the volume
   *  through every op, format, and load. */
  corruption = $state<string | null>(null);

  // `epoch` is read first on purpose: `adapter.owners` is a plain field, so nothing else
  // in this expression would re-run it after an op.
  attribution: AttributionTable = $derived.by(() => { this.epoch; return buildAttribution(this.adapter, this.layout, this.adapter.owners); });
  atLatest = $derived(this.cursor === this.history.length - 1);

  constructor() { this.adopt(this.vol); }

  /** Take over a fresh Volume: bind its adapter, full image copy, full scans, empty history. */
  private adopt(vol: Volume) {
    this.vol = vol;
    this.adapter = adapterFor(vol);
    this.image = vol.image();
    this.layout = vol.layout();
    this.sectorSize = vol.sectorSize();
    this.totalSectors = vol.sectorCount();
    this.zeros = scanZeroSectors(this.image, this.sectorSize);
    this.history = [];
    this.cursor = -1;
    this.refreshMeta();
    this.epoch++;
  }

  private refreshMeta() {
    this.adapter.refresh();
    // `corruption()` is on the `FileSystem` trait, so every family answers it and it cannot
    // throw `NotFat`; no guard is needed.
    this.corruption = this.vol.corruption();
  }

  /** A fresh volume of `family` (the current one by default) with that family's options. */
  format(family: FsFamilyId = this.adapter.id, options?: unknown) {
    try { this.adopt(FAMILIES[family].format(options)); this.status = null; selection.reset(); } catch (e) { this.fail(e); }
  }

  /** Mount an image. An unregistered `fsType()` makes `adapterFor` throw, which lands in
   *  `status` like any other load failure. */
  load(bytes: Uint8Array) {
    try { this.adopt(Volume.fromImage(bytes)); this.status = null; selection.reset(); } catch (e) { this.fail(e); }
  }

  export(): Uint8Array { return this.vol.image(); }

  /** Run a mutation at the latest state. Returns the record, or null on failure (status set). */
  run(fn: (v: Volume) => OpRecord): OpRecord | null {
    if (!this.atLatest) this.backToNow();
    let rec: OpRecord;
    try { rec = fn(this.vol); } catch (e) { this.fail(e); return null; }
    applyChanges(this.image, rec.changes, "forward");
    rescanSectors(this.zeros, this.image, this.sectorSize, changedSectors(rec.changes, this.sectorSize));
    this.history = [...this.history, rec];
    this.cursor = this.history.length - 1;
    // A raw write into the family's metadata may have been adopted by the core (FAT
    // re-parses the boot sector), moving regions: only then is `layout` re-read. The
    // adapter's `refresh()` below re-reads the geometry on every op. Bytes per sector cannot
    // change (the core rejects that), so `sectorSize` and `zeros` stay valid.
    if (this.adapter.touchesMetadata(rec.changes)) this.layout = this.vol.layout();
    this.refreshMeta();
    this.status = null;
    this.epoch++;
    if (import.meta.env.DEV) this.checkInvariant();
    return rec;
  }

  /** View the disk as it was after `step` (0-based). No wasm calls; patches the cached image.
   *  The adapter and `layout` stay at the latest state, like the tree and the layers, so a
   *  rewound view of a boot-sector change shows the older bytes under the newest layout. */
  seek(step: number) {
    step = Math.max(-1, Math.min(step, this.history.length - 1));
    if (step === this.cursor) return;
    const touched = new Set<number>();
    if (step < this.cursor) {
      for (let i = this.cursor; i > step; i--) { applyChanges(this.image, this.history[i].changes, "reverse"); changedSectors(this.history[i].changes, this.sectorSize).forEach((s) => touched.add(s)); }
    } else {
      for (let i = this.cursor + 1; i <= step; i++) { applyChanges(this.image, this.history[i].changes, "forward"); changedSectors(this.history[i].changes, this.sectorSize).forEach((s) => touched.add(s)); }
    }
    rescanSectors(this.zeros, this.image, this.sectorSize, touched);
    this.cursor = step;
    // The error belonged to the state being left behind; keep it off the new one.
    this.status = null;
    this.epoch++;
  }

  backToNow() { this.seek(this.history.length - 1); }

  private fail(e: unknown) {
    const err = e as Partial<FsError>;
    this.status = { text: err.message ?? String(e), code: err.code };
  }

  private checkInvariant() {
    // Hoist both arrays into locals: `this.image` is a `$state.raw` field, so
    // reading it per byte costs a proxy/signal read on a multi-megabyte loop.
    const truth = this.vol.image(), mine = this.image;
    for (let i = 0; i < truth.length; i++) {
      if (truth[i] !== mine[i]) { console.error(`cached image drifted from the volume at offset ${i}`); return; }
    }
  }
}

export const volume = new VolumeStore();
```

What left: the `geometry`, `owners`, and `fat` fields, `Volume.formatFat16`, `touchesBootSector`, the `try/catch` around `corruption()`, and the `ClusterOwner`/`FatEntry`/`FormatOptions`/`Geometry` imports. `sectorSize` is plain state now instead of a derived over the geometry.

- [ ] **Step 2: Derive the layers from the adapter, behind `epoch`**

Replace the whole of `web/ui/src/state/layers.svelte.ts` with:

```ts
import { intervalsToSectors, normalize, type Interval } from "../core/intervals";
import { GAP_REVEAL } from "../core/segments";
import { selection } from "./selection.svelte";
import { volume } from "./volume.svelte";

export class LayersStore {
  /** Extra selection ranges other stores contribute (Task 6 adds the entry slot). */
  extraSel = $state.raw<Interval[]>([]);
  str = $state.raw<Interval[]>([]);
  visible = $state.raw<Interval>({ start: 0, end: 0 });

  diff: Interval[] = $derived.by(() => {
    const rec = volume.history[volume.cursor];
    return rec ? normalize(rec.changes.map((c) => ({ start: c.offset, end: c.offset + c.after.length }))) : [];
  });

  // Every derived below calls the adapter, whose caches are plain fields, so each one reads
  // `volume.epoch` first (the rule in fs/adapter.ts). Without it a delete would leave the old
  // chain highlighted until something else changed.
  chain: number[] = $derived.by(() => {
    const path = selection.path;
    if (!path) return [];
    volume.epoch;
    return volume.adapter.chain(path);
  });

  /** Byte range of the selected path's directory entry slots (FAT: LFN + short), if any. */
  entry: Interval | null = $derived(selection.path ? (volume.epoch, volume.adapter.entrySlots(selection.path)) : null);

  /** Deleted directory slots and dirty free units, when the "Show remnants" toggle is on. A
   *  family without `adapter.remnants` has none. Empty while the timeline is rewound: the
   *  directory walk asks the live volume, so its slots would be from the latest state while
   *  the bytes on screen are from an older one. */
  remnant: Interval[] = $derived(selection.showRemnants && volume.atLatest ? (volume.epoch, volume.adapter.remnants?.(volume.zeros) ?? []) : []);

  sel: Interval[] = $derived((volume.epoch, normalize([...this.chain.map((u) => volume.adapter.unitByteRange(u)), ...(this.entry ? [this.entry] : []), ...this.extraSel])));

  pinnedSectors: Set<number> = $derived.by(() => {
    const s = new Set<number>([...intervalsToSectors(this.diff, volume.sectorSize), ...intervalsToSectors(this.sel, volume.sectorSize), ...intervalsToSectors(this.remnant, volume.sectorSize)]);
    if (selection.cursorOffset !== null) s.add(Math.floor(selection.cursorOffset / volume.sectorSize));
    for (const g of selection.expandedGaps) for (let i = 0; i < GAP_REVEAL; i++) s.add(g + i);
    return s;
  });
}

export const layers = new LayersStore();
```

What left: the `clusterByteRange`, `findEntrySlots`, `buildChain`, and `findRemnants` imports.

- [ ] **Step 3: Rename `StepFocus.cluster` to `unit` and format through the registry**

In `web/ui/src/state/scenarios.svelte.ts`, replace the imports (lines 1-5):

```ts
import type { FormatOptions, OpRecord, Volume } from "../lib/wasm";
import { clusterByteRange } from "../core/attribution";
import { ScenarioCursor } from "../core/scenarioCursor";
import { selection } from "./selection.svelte";
import { volume } from "./volume.svelte";
```

with:

```ts
import type { FormatOptions, OpRecord, Volume } from "../lib/wasm";
import { ScenarioCursor } from "../core/scenarioCursor";
import { DEFAULT_FAMILY } from "../fs";
import { selection } from "./selection.svelte";
import { volume } from "./volume.svelte";
```

(`FormatOptions` stays on `Step.format` until Task 6 makes `Step<O>` generic. Vitest never loads this runes file, so Task 3 had no reason to touch line 2; if it did, the result is still the five-line block above.) In `StepFocus` (lines 7-14), replace:

```ts
  cluster?: number;
```

with:

```ts
  unit?: number;
```

In `start` (line 46), replace:

```ts
    volume.format(); // also resets the selection
```

with:

```ts
    volume.format(DEFAULT_FAMILY); // also resets the selection
```

In `applyStep` (line 94), replace:

```ts
    if (step.format) volume.format(step.format);
```

with:

```ts
    if (step.format) volume.format(DEFAULT_FAMILY, step.format);
```

In `applyFocus` (lines 107-108), replace:

```ts
    else if (focus.sector !== undefined) selection.jumpTo(focus.sector * volume.geometry.bytesPerSector);
    else if (focus.cluster !== undefined) selection.jumpTo(clusterByteRange(volume.geometry, focus.cluster).start);
```

with:

```ts
    else if (focus.sector !== undefined) selection.jumpTo(focus.sector * volume.sectorSize);
    else if (focus.unit !== undefined) selection.jumpTo(volume.adapter.unitByteRange(focus.unit).start);
```

`Step.action`, `Step.focus`, and `step.focus(volume.vol)` keep their shapes; Task 6 changes the callbacks.

- [ ] **Step 4: Rename the focus key in the five scenario files that use it**

Six one-line edits, the focus objects only (the `fatOffset` parameter named `cluster` in `fundamentals.ts` line 26 and every `cluster` in lesson text stay):

`web/ui/src/scenarios/deleteRemnants.ts` line 26, replace `      focus: { cluster: 2, showRemnants: true },` with `      focus: { unit: 2, showRemnants: true },`.

`web/ui/src/scenarios/directory.ts` line 23, replace `        return { cluster: owner?.firstCluster };` with `        return { unit: owner?.firstCluster };`.

`web/ui/src/scenarios/fundamentals.ts` line 73, replace `      focus: { path: HELLO, cluster: 2 },` with `      focus: { path: HELLO, unit: 2 },`; line 89, replace `      focus: { path: BIGGER, cluster: 5 },` with `      focus: { path: BIGGER, unit: 5 },`.

`web/ui/src/scenarios/shell.ts` line 33, replace `      focus: (v) => ({ cluster: v.clusterOwners().find((o) => o.path === FILE)?.firstCluster }),` with `      focus: (v) => ({ unit: v.clusterOwners().find((o) => o.path === FILE)?.firstCluster }),`.

`web/ui/src/scenarios/smallFile.ts` line 31, replace `      focus: { cluster: 2 },` with `      focus: { unit: 2 },`.

- [ ] **Step 5: Run the suite to confirm the pure layer still loads**

Run: `cd web/ui && pnpm test`

Expected: every file passes, the same set as after Task 3. The runes files edited so far are never loaded by vitest (the scenario files import `Scenario` as a type only), so this proves the scenario files and `tests/lesson.test.ts` agree on `unit` and nothing else moved; `grep -rn "cluster:" web/ui/src/scenarios` now lists exactly two lines, neither a focus key: `fundamentals.ts:26` (the `fatOffset` parameter) and `fundamentals.ts:72` (the lesson text, "the first cluster: 2"). Both stay until Task 6 rewrites that file.

- [ ] **Step 6: Write the failing address tests**

Replace the whole of `web/ui/tests/shell/addr.test.ts` with:

```ts
import { describe, expect, it } from "vitest";
import type { AddrSpace } from "../../src/shell/addr";
import { SIZE_HELP, addrHelp, parseAddr, parseSize } from "../../src/shell/addr";
import { ShellError } from "../../src/shell/errors";
import { space } from "../fixtures/geometry";

function thrown(fn: () => unknown): ShellError {
  try { fn(); } catch (e) { return e as ShellError; }
  throw new Error("expected a throw");
}

/** Yesterday's `ADDR_HELP`, byte for byte: what every FAT address error and flag description shows. */
const FAT_HELP = "addresses: 0x1f (hex), 512 (decimal), s:65 (sector), c:3 (cluster)";

/** A made-up family with 1 KiB units counted from 0 and the letter `b`, to prove the noun and
 *  letter come from the space rather than from the parser. */
const blocks: AddrSpace = {
  sectorSize: 1024,
  unit: { singular: "block", plural: "blocks", letter: "b", first: 0 },
  unitByteRange: (u) => ({ start: u * 1024, end: (u + 1) * 1024 }),
};

describe("addrHelp", () => {
  it("composes the FAT help exactly as before, and another family's from its noun and letter", () => {
    expect(addrHelp(space)).toBe(FAT_HELP);
    expect(addrHelp(blocks)).toBe("addresses: 0x1f (hex), 512 (decimal), s:65 (sector), b:3 (block)");
  });
});

describe("parseAddr", () => {
  it("accepts sectors, clusters, hex, decimal strings, and integers (HexView's g prompt forms)", () => {
    expect(parseAddr("s:65", space)).toBe(65 * 512);
    expect(parseAddr("S:0", space)).toBe(0);
    expect(parseAddr("c:2", space)).toBe(97 * 512);
    expect(parseAddr("C:2", space)).toBe(97 * 512);
    expect(parseAddr("c:3", space)).toBe(97 * 512 + 2048);
    expect(parseAddr("0x1F", space)).toBe(31);
    expect(parseAddr("0X1f", space)).toBe(31);
    expect(parseAddr("512", space)).toBe(512);
    expect(parseAddr(512, space)).toBe(512);
    expect(parseAddr(0, space)).toBe(0);
  });
  it("throws a ShellError naming the input, with the composed help", () => {
    for (const bad of ["-1", "s:x", "1k", "", "c:", "0x", "12 34", "i:1"]) {
      const e = thrown(() => parseAddr(bad, space));
      expect(e).toBeInstanceOf(ShellError);
      expect(e.message).toBe(`bad address '${bad}'`);
      expect(e.help).toBe(FAT_HELP);
    }
    expect(thrown(() => parseAddr(-3, space)).message).toBe("bad address '-3'");
    expect(thrown(() => parseAddr(1.5, space)).message).toBe("bad address '1.5'");
    // c:0 lands before the data region on this geometry: 97*512 - 2*2048 is still >= 0, but a
    // tiny geometry (data from sector 4, 8 sectors per cluster) goes negative, and negative
    // is never an address.
    const tiny: AddrSpace = {
      sectorSize: 512,
      unit: space.unit,
      unitByteRange: (u) => { const start = 4 * 512 + (u - 2) * 8 * 512; return { start, end: start + 8 * 512 }; },
    };
    expect(thrown(() => parseAddr("c:0", tiny)).message).toBe("bad address 'c:0'");
  });
  it("hands anything else to the family's own parser, which can add forms such as i:N", () => {
    const withInodes: AddrSpace = {
      sectorSize: space.sectorSize,
      unit: space.unit,
      unitByteRange: (u) => space.unitByteRange(u),
      parseAddr: (v) => (v === "i:1" ? 1234 : undefined),
    };
    expect(parseAddr("i:1", withInodes)).toBe(1234);
    expect(parseAddr("c:2", withInodes)).toBe(97 * 512); // the generic forms still come first
    const e = thrown(() => parseAddr("i:2", withInodes));
    expect(e.message).toBe("bad address 'i:2'");
    expect(e.help).toBe(FAT_HELP); // the hook adds no help of its own
  });
  it("reads the unit letter from the space, so another family's b:N is a unit address", () => {
    expect(parseAddr("b:3", blocks)).toBe(3 * 1024);
    expect(parseAddr("B:0", blocks)).toBe(0);
    expect(parseAddr("s:2", blocks)).toBe(2048);
    const e = thrown(() => parseAddr("c:3", blocks));
    expect(e.message).toBe("bad address 'c:3'");
    expect(e.help).toBe("addresses: 0x1f (hex), 512 (decimal), s:65 (sector), b:3 (block)");
  });
});

describe("parseSize", () => {
  it("accepts plain, k/M-suffixed, and hex sizes, and integers", () => {
    expect(parseSize("512")).toBe(512);
    expect(parseSize("1k")).toBe(1024);
    expect(parseSize("2K")).toBe(2048);
    expect(parseSize("4M")).toBe(4 * 1048576);
    expect(parseSize("1m")).toBe(1048576);
    expect(parseSize("0x200")).toBe(512);
    expect(parseSize(512)).toBe(512);
    expect(parseSize("0")).toBe(0);
  });
  it("throws a ShellError with SIZE_HELP", () => {
    for (const bad of ["abc", "1kb", "-1", "", "k"]) {
      const e = thrown(() => parseSize(bad));
      expect(e).toBeInstanceOf(ShellError);
      expect(e.message).toBe(`bad size '${bad}'`);
      expect(e.help).toBe(SIZE_HELP);
    }
    expect(thrown(() => parseSize(-1)).message).toBe("bad size '-1'");
    expect(thrown(() => parseSize(2.5)).message).toBe("bad size '2.5'");
  });
});
```

- [ ] **Step 7: Run the address tests and confirm the failure**

Run: `cd web/ui && pnpm vitest run tests/shell/addr.test.ts`

Expected: 4 of 7 tests fail: the `addrHelp` test (`addr.ts` has no such export, so `TypeError: addrHelp is not a function`) and three `parseAddr` tests, `accepts sectors, clusters, ...` (`parseAddr("s:65", space)` throws `bad address 's:65'` because the old code multiplies by `space.bytesPerSector`, which a `UnitSpace` does not have), `hands anything else to the family's own parser` (`i:1` throws), and `reads the unit letter from the space` (`b:3` throws). The `throws a ShellError` test and both `parseSize` tests pass already: the old `ADDR_HELP` is the same string as `FAT_HELP`, and every bad input still throws.

- [ ] **Step 8: Rewrite `shell/addr.ts` over an `AddrSpace`**

Replace the whole of `web/ui/src/shell/addr.ts` with:

```ts
import type { UnitSpace } from "../fs/adapter";
import { ShellError } from "./errors";

export const SIZE_HELP = "sizes: 512, 1k, 4M, 0x200";

/**
 * What an address parser needs from a family: the sector size, the unit noun and letter, the
 * unit's byte range, and optionally the family's own extra forms (`parseAddr`, which answers
 * `undefined` for anything that is not its own). A `UnitSpace` or an `FsAdapter` satisfies
 * it; tests build one from a fixture.
 */
export type AddrSpace = Pick<UnitSpace, "sectorSize" | "unit" | "unitByteRange"> & { parseAddr?(v: string): number | undefined };

/** The address forms in one line, for error help and flag descriptions. For FAT this reads
 *  `addresses: 0x1f (hex), 512 (decimal), s:65 (sector), c:3 (cluster)`. */
export function addrHelp(space: AddrSpace): string {
  return `addresses: 0x1f (hex), 512 (decimal), s:65 (sector), ${space.unit.letter}:3 (${space.unit.singular})`;
}

/** `<letter>:N` with the family's letter, case-insensitively like `s:N`; `undefined` otherwise. */
function unitIndex(v: string, letter: string): number | undefined {
  const prefix = `${letter}:`;
  if (v.length <= prefix.length || v.slice(0, prefix.length).toLowerCase() !== prefix.toLowerCase()) return undefined;
  const digits = v.slice(prefix.length);
  return /^\d+$/.test(digits) ? Number(digits) : undefined;
}

/**
 * The address forms HexView's `g` prompt accepts: `s:N` (sector), `0x…`, decimal, the
 * family's `<letter>:N` (a data unit: FAT's `c:N`, whose clusters start at 2), then whatever
 * else the family parses (`space.parseAddr`). A number passes through when it is a
 * non-negative integer. Bounds are the caller's business (HexView checks the image length,
 * `seek` the disk size).
 */
export function parseAddr(v: string | number, space: AddrSpace): number {
  const bad = () => new ShellError(`bad address '${v}'`, { help: addrHelp(space) });
  let off: number | undefined;
  if (typeof v === "number") off = v;
  else if (/^s:\d+$/i.test(v)) off = Number(v.slice(2)) * space.sectorSize;
  else if (/^0x[0-9a-f]+$/i.test(v)) off = parseInt(v, 16);
  else if (/^\d+$/.test(v)) off = Number(v);
  else {
    const unit = unitIndex(v, space.unit.letter);
    off = unit !== undefined ? space.unitByteRange(unit).start : space.parseAddr?.(v);
  }
  if (off === undefined || !Number.isInteger(off) || off < 0) throw bad();
  return off;
}

/** Byte counts: `512`, `1k`, `4M`, `0x200`, or a non-negative integer. Zero is allowed; callers decide. */
export function parseSize(v: string | number): number {
  const bad = () => new ShellError(`bad size '${v}'`, { help: SIZE_HELP });
  let n: number;
  if (typeof v === "number") n = v;
  else {
    const m = /^(\d+)([kKmM]?)$/.exec(v);
    if (m) n = Number(m[1]) * (m[2] === "" ? 1 : m[2].toLowerCase() === "k" ? 1024 : 1048576);
    else if (/^0x[0-9a-f]+$/i.test(v)) n = parseInt(v, 16);
    else throw bad();
  }
  if (!Number.isInteger(n) || n < 0) throw bad();
  return n;
}
```

What left: `ADDR_HELP`, `AddrGeometry`, the `Geometry` import, and the `c:` regex. The four forms keep their order and results; `<letter>:N` is matched by prefix rather than a regex built from the letter.

- [ ] **Step 9: Run the address tests**

Run: `cd web/ui && pnpm vitest run tests/shell/addr.test.ts`

Expected: `1 passed (1)` file, `7 passed (7)` tests. (`commands.ts` still imports `ADDR_HELP` and passes a `Geometry` to `parseAddr`, so `read-commands`, `mutations`, `dd`, `xxd-command`, and `redirect` fail from here until Step 18; that is the expected mid-task state.)

- [ ] **Step 10: `canonicalize` takes the adapter's `namesMatch`**

In `web/ui/src/shell/vfs.ts`, replace line 1:

```ts
import type { Volume } from "../lib/wasm";
```

with:

```ts
import type { FsAdapter } from "../fs/adapter";
```

Then replace lines 74-98:

```ts
const namesMatch = (a: string, b: string): boolean => a.toUpperCase() === b.toUpperCase();

/**
 * Re-spell each component of a volume path the way the directory stores it (FAT lookups are
 * case-insensitive, so `/docs/n.txt` is `/DOCS/N.TXT` on disk). A component that is missing,
 * or whose parent cannot be listed (a file in the middle, a corrupt volume), keeps the typed
 * spelling; callers validate existence with `stat` separately.
 */
export function canonicalize(vol: Volume, volumePath: string): string {
  const out: string[] = [];
  let dir = "/";
  for (const part of volumePath.split("/")) {
    if (part === "") continue;
    let name = part;
    try {
      const hit = vol.listDir(dir).find((e) => namesMatch(e.name, part));
      if (hit) name = hit.name;
    } catch {
      // unreadable parent: keep the typed spelling for this and the remaining components
    }
    out.push(name);
    dir = joinVolume(dir, name);
  }
  return "/" + out.join("/");
}
```

with:

```ts
/**
 * Re-spell each component of a volume path the way the directory stores it, matching names
 * the family's way (`fs.namesMatch`: FAT lookups are case-insensitive, so `/docs/n.txt` is
 * `/DOCS/N.TXT` on disk). A component that is missing, or whose parent cannot be listed (a
 * file in the middle, a corrupt volume), keeps the typed spelling; callers validate
 * existence with `stat` separately.
 */
export function canonicalize(fs: Pick<FsAdapter, "vol" | "namesMatch">, volumePath: string): string {
  const out: string[] = [];
  let dir = "/";
  for (const part of volumePath.split("/")) {
    if (part === "") continue;
    let name = part;
    try {
      const hit = fs.vol.listDir(dir).find((e) => fs.namesMatch(e.name, part));
      if (hit) name = hit.name;
    } catch {
      // unreadable parent: keep the typed spelling for this and the remaining components
    }
    out.push(name);
    dir = joinVolume(dir, name);
  }
  return "/" + out.join("/");
}
```

In `web/ui/tests/shell/vfs.test.ts`, replace lines 2-4:

```ts
import { Volume } from "../../src/lib/wasm";
import { ShellError } from "../../src/shell/errors";
import { DEVICE_HELP, MOUNT, PATH_HELP, Vfs, basename, canonicalize, joinVolume, promptFor } from "../../src/shell/vfs";
```

with:

```ts
import { Volume } from "../../src/lib/wasm";
import { adapterFor } from "../../src/fs";
import { ShellError } from "../../src/shell/errors";
import { DEVICE_HELP, MOUNT, PATH_HELP, Vfs, basename, canonicalize, joinVolume, promptFor } from "../../src/shell/vfs";
```

and the `canonicalize over a real volume` block (lines 78-96) with:

```ts
describe("canonicalize over a real volume", () => {
  it("fixes the case of every component to the on-disk name", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createFile("/Hello world.txt", new TextEncoder().encode("hi"));
    vol.createDir("/DOCS");
    vol.createFile("/DOCS/N.TXT", new Uint8Array(3));
    const fs = adapterFor(vol);
    expect(canonicalize(fs, "/hello world.txt")).toBe("/Hello world.txt");
    expect(canonicalize(fs, "/HELLO WORLD.TXT")).toBe("/Hello world.txt");
    expect(canonicalize(fs, "/docs/n.txt")).toBe("/DOCS/N.TXT");
    expect(canonicalize(fs, "/docs")).toBe("/DOCS");
    expect(canonicalize(fs, "/")).toBe("/");
  });
  it("keeps the typed spelling for components it cannot find or read", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createDir("/DOCS");
    const fs = adapterFor(vol);
    expect(canonicalize(fs, "/docs/missing.txt")).toBe("/DOCS/missing.txt");
    expect(canonicalize(fs, "/nope/deeper")).toBe("/nope/deeper");
  });
});
```

Run: `cd web/ui && pnpm vitest run tests/shell/vfs.test.ts`

Expected: `1 passed (1)` file, `11 passed (11)` tests (the same eleven as before; `canonicalize` reads the directory live through `fs.vol`, so the adapter's caches play no part here).

- [ ] **Step 11: Reword `CORRUPT_HELP` family-neutral and give the errors-test host an adapter**

In `web/ui/src/shell/errors.ts`, replace lines 20-26:

```ts
/**
 * The way back from an unparsable boot sector. `wrapFs` attaches it to every `CorruptImage`
 * error, whichever call produced it, and `assertMounted` uses the same text for the gate it
 * raises itself.
 */
export const CORRUPT_HELP =
  "the boot sector no longer parses; rewind on the timeline, or write the saved sector back with: <the saved bytes> | dd --of=/dev/hda";
```

with:

```ts
/**
 * The way back from on-disk metadata that no longer parses (for FAT, the boot sector).
 * `wrapFs` attaches it to every `CorruptImage` error, whichever call produced it, and
 * `assertMounted` uses the same text for the gate it raises itself. Family-neutral on
 * purpose: `wrapFs` has no adapter, so the family's own sentence lives in
 * `adapter.corruptNote` and in the Rust message the error carries.
 */
export const CORRUPT_HELP =
  "the on-disk metadata no longer parses; rewind on the timeline, or write the saved bytes back with: <the saved bytes> | dd --of=/dev/hda";
```

Every test that asserts on it compares to the constant (`errors.test.ts` lines 48-49, `redirect.test.ts` lines 202 and 206) or checks `toContain("dd --of=/dev/hda")` (`read-commands.test.ts` line 293), so no expectation changes.

In `web/ui/tests/shell/errors.test.ts`, replace lines 2-4:

```ts
import { Volume } from "../../src/lib/wasm";
import { CORRUPT_HELP, ShellError, fsCall, fsPhrase, wrapFs } from "../../src/shell/errors";
import { atLatest, corruptionOf, statusToError, type ShellHost } from "../../src/shell/host";
```

with:

```ts
import { Volume } from "../../src/lib/wasm";
import { adapterFor } from "../../src/fs";
import { CORRUPT_HELP, ShellError, fsCall, fsPhrase, wrapFs } from "../../src/shell/errors";
import { atLatest, corruptionOf, statusToError, type ShellHost } from "../../src/shell/host";
```

and the stub (lines 61-64):

```ts
  const stub = (cursor: number, historyLength: number): ShellHost => ({
    vol: Volume.formatFat16(undefined), cursor, historyLength,
    run: () => { throw new Error("unused"); }, format: () => {}, select: () => {}, jumpTo: () => {}, setPrompt: () => {}, closeTerminal: () => {},
  });
```

with:

```ts
  const stub = (cursor: number, historyLength: number): ShellHost => {
    const vol = Volume.formatFat16(undefined);
    return {
      vol, adapter: adapterFor(vol), cursor, historyLength,
      run: () => { throw new Error("unused"); }, format: () => {}, select: () => {}, jumpTo: () => {}, setPrompt: () => {}, closeTerminal: () => {},
    };
  };
```

Run: `cd web/ui && pnpm vitest run tests/shell/errors.test.ts`

Expected: `1 passed (1)` file, `10 passed (10)` tests; the `attaches the recovery hint` test passes against the new constant.

- [ ] **Step 12: `ShellHost` gains `adapter` and `format(family, options)`**

In `web/ui/src/shell/host.ts`, replace lines 1-2:

```ts
import type { FormatOptions, OpRecord, Volume } from "../lib/wasm";
import { canonicalize } from "./vfs";
```

with:

```ts
import type { OpRecord, Volume } from "../lib/wasm";
import type { FsAdapter, FsFamilyId } from "../fs/adapter";
import { canonicalize } from "./vfs";
```

Replace the interface (lines 9-28):

```ts
export interface ShellHost {
  /** The latest volume; every read goes here even while the timeline is rewound. */
  readonly vol: Volume;
  /** Timeline position: -1 before any op, `historyLength - 1` at the latest step. */
  readonly cursor: number;
  readonly historyLength: number;
  /** Run one journaled mutation at the latest state. Throws `{ message, code? }` on failure. */
  run(fn: (v: Volume) => OpRecord): OpRecord;
  /** Replace the volume; the store discards the history. Throws `{ message, code? }` on failure. */
  format(options: FormatOptions): void;
  select(path: string | null): void;
  jumpTo(offset: number): void;
  /**
   * Set the prefix the terminal renders before its `❯`, so the prompt shows the working
   * directory. The app hands this straight to browser-terminal's `setPrompt` (0.3.0+),
   * which applies it to every pane. Commands call it whenever `vfs.cwd` changes.
   */
  setPrompt(prefix: string): void;
  closeTerminal(): void;
}
```

with:

```ts
export interface ShellHost {
  /** The latest volume; every read goes here even while the timeline is rewound. */
  readonly vol: Volume;
  /** The family adapter bound to `vol`: refreshed by the host after every `run`, re-bound by
   *  `format`. Commands take unit arithmetic, `stat`/`df` facts, address help, `mkfs` flags,
   *  and the rewrite notes from it, never from a family module directly. */
  readonly adapter: FsAdapter;
  /** Timeline position: -1 before any op, `historyLength - 1` at the latest step. */
  readonly cursor: number;
  readonly historyLength: number;
  /** Run one journaled mutation at the latest state. Throws `{ message, code? }` on failure. */
  run(fn: (v: Volume) => OpRecord): OpRecord;
  /** Replace the volume with a fresh one of `family`, formatted with that family's options;
   *  the store discards the history. Throws `{ message, code? }` on failure. */
  format(family: FsFamilyId, options?: unknown): void;
  select(path: string | null): void;
  jumpTo(offset: number): void;
  /**
   * Set the prefix the terminal renders before its `❯`, so the prompt shows the working
   * directory. The app hands this straight to browser-terminal's `setPrompt` (0.3.0+),
   * which applies it to every pane. Commands call it whenever `vfs.cwd` changes.
   */
  setPrompt(prefix: string): void;
  closeTerminal(): void;
}
```

The `corruptionOf` comment and function below the interface are Task 1's (spec section 4 point 3; its Step 14 reworded the comment and kept the try/catch) and are left exactly as that task committed them. In `selectPath` (line 48, one lower than at HEAD because Task 1's comment is a line longer), replace:

```ts
  const canon = canonicalize(host.vol, path);
```

with:

```ts
  const canon = canonicalize(host.adapter, path);
```

- [ ] **Step 13: The store host exposes the adapter and forwards `format`**

In `web/ui/src/shell/storeHost.svelte.ts`, replace line 1:

```ts
import type { FormatOptions, OpRecord, Volume } from "../lib/wasm";
```

with:

```ts
import type { OpRecord, Volume } from "../lib/wasm";
import type { FsAdapter, FsFamilyId } from "../fs/adapter";
```

Replace lines 18-20:

```ts
    get vol(): Volume {
      return volume.vol;
    },
```

with:

```ts
    get vol(): Volume {
      return volume.vol;
    },
    get adapter(): FsAdapter {
      return volume.adapter;
    },
```

and lines 32-38:

```ts
    format(options: FormatOptions): void {
      volume.format(options);
      // On failure VolumeStore.format sets status and leaves the disk alone. Read it
      // before select(null), whose side effect is to clear status.
      if (volume.status) throw statusToError(volume.status);
      selection.select(null);
    },
```

with:

```ts
    format(family: FsFamilyId, options?: unknown): void {
      volume.format(family, options);
      // On failure VolumeStore.format sets status and leaves the disk alone. Read it
      // before select(null), whose side effect is to clear status.
      if (volume.status) throw statusToError(volume.status);
      selection.select(null);
    },
```

`TerminalPanel.svelte` calls `createStoreHost(close, (prefix) => term.setPrompt(prefix))`, whose signature is unchanged.

- [ ] **Step 14: `dd` takes its rewrite clause from the adapter**

In `web/ui/src/shell/dd.ts`, replace lines 140-144:

```ts
/**
 * Overlay `data` at `off` on the file at `path` (zero-padded; FAT has no partial writes).
 * Distinct from commands.ts's `writeVolumeFile`, which replaces or appends to a whole file:
 * this one is `dd`'s `--seek`, which places bytes inside an existing one.
 */
```

with:

```ts
/**
 * Overlay `data` at `off` on the file at `path` (zero-padded: the core has no partial writes,
 * and `adapter.notes.partialWrite` says so in the family's words). Distinct from commands.ts's
 * `writeVolumeFile`, which replaces or appends to a whole file: this one is `dd`'s `--seek`,
 * which places bytes inside an existing one.
 */
```

and line 163:

```ts
  if (had || off > 0) ctx.log("FAT has no partial writes; the whole file was rewritten");
```

with:

```ts
  if (had || off > 0) ctx.log(host.adapter.notes.partialWrite);
```

- [ ] **Step 15: `commands.ts`: imports, `cap`, `cd`, `stat`, `df`, `seek`**

In `web/ui/src/shell/commands.ts`, replace lines 2-6:

```ts
import type { DateTime, EntryInfo, FormatOptions, Volume } from "../lib/wasm";
import { clusterByteRange } from "../core/attribution";
import { findEntrySlots } from "../core/direntry";
import { buildChain } from "../core/fatchain";
import { ADDR_HELP, SIZE_HELP, parseAddr, parseSize } from "./addr";
```

with:

```ts
import type { DateTime, EntryInfo, Volume } from "../lib/wasm";
import { SIZE_HELP, addrHelp, parseAddr, parseSize } from "./addr";
```

(If Task 3 re-pointed the `clusterByteRange`, `findEntrySlots`, or `buildChain` imports at `../fs/fat16/...` to keep its own gate green, the three lines read differently; the result is the two-line block above, with no import from `../core/attribution`, `../core/direntry`, `../core/fatchain`, or `../fs/fat16`.) After line 64 (`export const hexAddr = ...`), add:

```ts

/** "cluster" -> "Cluster": a summary that opens with the family's unit noun. */
const cap = (s: string): string => s.charAt(0).toUpperCase() + s.slice(1);
```

In `cd` (line 182), replace:

```ts
          vfs.cwd = vfs.toVirtual(canonicalize(host.vol, r.path));
```

with:

```ts
          vfs.cwd = vfs.toVirtual(canonicalize(host.adapter, r.path));
```

In `stat`, replace the `volume` case (lines 255-287):

```ts
        case "volume": {
          assertMounted(host, display);
          let info: EntryInfo;
          try {
            info = vol.stat(r.path);
          } catch (e) {
            throw wrapFs(display, e);
          }
          const canon = canonicalize(vol, r.path);
          const g = vol.geometry();
          const fat = vol.fatEntries(0);
          const owners = vol.clusterOwners();
          const first = owners.find((o) => o.path === canon)?.firstCluster ?? 0;
          const chain = buildChain(fat, first);
          const slots = findEntrySlots(vol, g, fat, owners, canon);
          const bps = g.bytesPerSector;
          return {
            path: vfs.toVirtual(canon),
            name: info.name,
            type: info.isDir ? "dir" : "file",
            size: info.size,
            created: fmtDate(info.created),
            modified: fmtDate(info.modified),
            accessed: fmtDate(info.accessed),
            firstCluster: first,
            chain,
            clusters: chain.length,
            entryOffset: slots ? hexAddr(slots.start) : "",
            entrySlots: slots ? `${hexAddr(slots.start)}-${hexAddr(slots.end)}` : "",
            fatEntryOffset: first >= 2 ? hexAddr(g.reservedSectors * bps + first * 2) : "",
            dataOffset: canon === "/" ? hexAddr(g.firstRootDirSector * bps) : first >= 2 ? hexAddr(clusterByteRange(g, first).start) : "",
          };
        }
```

with:

```ts
        case "volume": {
          assertMounted(host, display);
          let info: EntryInfo;
          try {
            info = vol.stat(r.path);
          } catch (e) {
            throw wrapFs(display, e);
          }
          const canon = canonicalize(host.adapter, r.path);
          // The generic facts first, then the family's own (FAT: firstCluster, chain,
          // clusters, entryOffset, entrySlots, fatEntryOffset, dataOffset), already formatted.
          return {
            path: vfs.toVirtual(canon),
            name: info.name,
            type: info.isDir ? "dir" : "file",
            size: info.size,
            created: fmtDate(info.created),
            modified: fmtDate(info.modified),
            accessed: fmtDate(info.accessed),
            ...host.adapter.stat(canon),
          };
        }
```

Replace `df` (lines 292-320):

```ts
  const df: CommandDef = {
    spec: { name: "df", summary: "Cluster usage of the mounted volume" },
    fn: (_args, _input, ctx) => {
      warnIfRewound(host, ctx);
      assertMounted(host, "/mnt");
      const vol = host.vol;
      const g = vol.geometry();
      const fat = vol.fatEntries(0);
      let used = 0;
      for (let c = 2; c < g.clusterCount + 2 && c < fat.length; c++) if (fat[c].kind !== "free") used++;
      const free = g.clusterCount - used;
      const clusterSize = g.bytesPerSector * g.sectorsPerCluster;
      const pct = g.clusterCount > 0 ? Math.round((used / g.clusterCount) * 100) : 0;
      return [
        {
          filesystem: "/dev/hda",
          mounted: "/mnt",
          type: vol.fsType(),
          clusterSize,
          clusters: g.clusterCount,
          used,
          free,
          bytesUsed: used * clusterSize,
          bytesFree: free * clusterSize,
          use: `${pct}%`,
        },
      ];
    },
  };
```

with:

```ts
  const df: CommandDef = {
    spec: { name: "df", summary: `${cap(host.adapter.unit.singular)} usage of the mounted volume` },
    fn: (_args, _input, ctx) => {
      warnIfRewound(host, ctx);
      assertMounted(host, "/mnt");
      const { unitSize, units, used, free } = host.adapter.df();
      // The printed keys come from the noun, so FAT still prints `clusterSize` and `clusters`.
      const { singular, plural } = host.adapter.unit;
      const pct = units > 0 ? Math.round((used / units) * 100) : 0;
      return [
        {
          filesystem: "/dev/hda",
          mounted: "/mnt",
          type: host.vol.fsType(),
          [`${singular}Size`]: unitSize,
          [plural]: units,
          used,
          free,
          bytesUsed: used * unitSize,
          bytesFree: free * unitSize,
          use: `${pct}%`,
        },
      ];
    },
  };
```

Replace `seek` (lines 343-352):

```ts
  const seek: CommandDef = {
    spec: { name: "seek", summary: "Move the hex dump to an address", required: [P("addr", "0x1f, 512, s:65 (sector), or c:3 (cluster)")] },
    fn: (args) => {
      const vol = host.vol;
      const off = parseAddr(reqStr(args, 0, "addr"), vol.geometry());
      const limit = vol.sectorCount() * vol.sectorSize();
      if (off >= limit) throw new ShellError(`${hexAddr(off)} is past the end of the disk (${limit} bytes)`);
      host.jumpTo(off);
    },
  };
```

with:

```ts
  const seek: CommandDef = {
    spec: {
      name: "seek",
      summary: "Move the hex dump to an address",
      required: [P("addr", `0x1f, 512, s:65 (sector), or ${host.adapter.unit.letter}:3 (${host.adapter.unit.singular})`)],
    },
    fn: (args) => {
      const vol = host.vol;
      const off = parseAddr(reqStr(args, 0, "addr"), host.adapter);
      const limit = vol.sectorCount() * vol.sectorSize();
      if (off >= limit) throw new ShellError(`${hexAddr(off)} is past the end of the disk (${limit} bytes)`);
      host.jumpTo(off);
    },
  };
```

- [ ] **Step 16: `commands.ts`: `write`, `cp`, `mkfs`**

Replace the `writeVolumeFile` doc comment (lines 412-417):

```ts
/**
 * Put `bytes` in the volume file at `path`, creating it or overwriting it, and select it.
 * `append` reads the file and concatenates first — FAT has no append, so the whole file is
 * rewritten either way. The one place a file's contents change: the `write` command, which
 * adds the logging, and the `>`/`>>` redirect hook, which has no `ctx` to log to.
 */
```

with:

```ts
/**
 * Put `bytes` in the volume file at `path`, creating it or overwriting it, and select it.
 * `append` reads the file and concatenates first — the core has no append, so the whole file
 * is rewritten either way (`adapter.notes.rewrite` says so in the family's words). The one
 * place a file's contents change: the `write` command, which adds the logging, and the
 * `>`/`>>` redirect hook, which has no `ctx` to log to.
 */
```

In `write`'s spec (line 455), replace:

```ts
        { long: "at", shape: "str", desc: `disk address for /dev/hda (${ADDR_HELP})` },
```

with:

```ts
        { long: "at", shape: "str", desc: `disk address for /dev/hda (${addrHelp(host.adapter)})` },
```

In `write`'s body, replace lines 471-473:

```ts
        if (!flagGiven(at)) throw new ShellError("/dev/hda: give --at <addr>", { help: ADDR_HELP });
        if (data.length === 0) throw new ShellError("nothing to write", { help: "a raw write needs at least one byte" });
        const off = parseAddr(String(at), host.vol.geometry());
```

with:

```ts
        if (!flagGiven(at)) throw new ShellError("/dev/hda: give --at <addr>", { help: addrHelp(host.adapter) });
        if (data.length === 0) throw new ShellError("nothing to write", { help: "a raw write needs at least one byte" });
        const off = parseAddr(String(at), host.adapter);
```

and line 483:

```ts
      if (appended) ctx.log(`appended ${data.length} bytes by rewriting the whole file (FAT has no append; the old chain is freed and reallocated)`);
```

with:

```ts
      if (appended) ctx.log(`appended ${data.length} bytes by rewriting the whole file (${host.adapter.notes.rewrite})`);
```

In `cp` (line 579), replace:

```ts
        dstPath = joinPath(canonicalize(host.vol, dstPath), baseName(canonicalize(host.vol, srcPath)));
```

with:

```ts
        dstPath = joinPath(canonicalize(host.adapter, dstPath), baseName(canonicalize(host.adapter, srcPath)));
```

Replace `mkfs` (lines 592-630):

```ts
  const mkfs: CommandDef = {
    spec: {
      name: "mkfs",
      summary: "Format /dev/hda as FAT16 (clears the timeline)",
      flags: [
        { long: "sectors", shape: "int", desc: "total sectors (default 32768 = 16 MB)" },
        { long: "spc", shape: "int", desc: "sectors per cluster (default 4)" },
        { long: "label", shape: "str", desc: "volume label, up to 11 characters" },
        { long: "root-entries", shape: "int", desc: "root directory entries (default 512)" },
        { long: "fats", shape: "int", desc: "FAT copies (default 2)" },
        { long: "reserved", shape: "int", desc: "reserved sectors (default 1)" },
      ],
    },
    fn: (args, _input, ctx) => {
      const int = (name: string): number | undefined => {
        const v = args.flags[name];
        if (!flagGiven(v)) return undefined;
        if (typeof v !== "number" || !Number.isInteger(v) || v < 0) throw new ShellError(`--${name} must be a non-negative integer`);
        return v;
      };
      const options: FormatOptions = {};
      const sectors = int("sectors");
      if (sectors !== undefined) options.totalSectors = sectors;
      const spc = int("spc");
      if (spc !== undefined) options.sectorsPerCluster = spc;
      const rootEntries = int("root-entries");
      if (rootEntries !== undefined) options.rootEntries = rootEntries;
      const fats = int("fats");
      if (fats !== undefined) options.fatCount = fats;
      const reserved = int("reserved");
      if (reserved !== undefined) options.reservedSectors = reserved;
      const label = args.flags.label;
      if (flagGiven(label)) options.volumeLabel = String(label);
      fsCall("/dev/hda", () => host.format(options));
      vfs.cwd = "/mnt";
      host.setPrompt(promptFor(vfs.cwd));
      ctx.log("formatted /dev/hda as FAT16; the timeline was cleared");
    },
  };
```

with:

```ts
  // The flags, their option keys, and both messages are the family's (`FsFamily.mkfs`); the
  // spec is built once at registration, for the family mounted then.
  const mkfs: CommandDef = {
    spec: {
      name: "mkfs",
      summary: host.adapter.family.mkfs.summary,
      flags: host.adapter.family.mkfs.flags.map((f) => F(f.long, f.desc, { shape: f.kind })),
    },
    fn: (args, _input, ctx) => {
      const { flags, done } = host.adapter.family.mkfs;
      const options: Record<string, number | string> = {};
      for (const f of flags) {
        const v = args.flags[f.long];
        if (!flagGiven(v)) continue;
        if (f.kind === "int") {
          if (typeof v !== "number" || !Number.isInteger(v) || v < 0) throw new ShellError(`--${f.long} must be a non-negative integer`);
          options[f.option] = v;
        } else {
          options[f.option] = String(v);
        }
      }
      fsCall("/dev/hda", () => host.format(host.adapter.id, options));
      vfs.cwd = "/mnt";
      host.setPrompt(promptFor(vfs.cwd));
      ctx.log(done);
    },
  };
```

- [ ] **Step 17: `commands.ts`: the `xxd` offset flag**

In `xxdSpec` (line 666), replace:

```ts
      { long: "offset", shape: "str", desc: `start address (${ADDR_HELP})` },
```

with:

```ts
      { long: "offset", shape: "str", desc: `start address (${addrHelp(host.adapter)})` },
```

and in `xxd` (line 677), replace:

```ts
    const offset = offsetFlag === undefined || offsetFlag === null ? undefined : parseAddr(String(offsetFlag), host.vol.geometry());
```

with:

```ts
    const offset = offsetFlag === undefined || offsetFlag === null ? undefined : parseAddr(String(offsetFlag), host.adapter);
```

After this step `grep -n "geometry()\|fatEntries(\|clusterOwners(\|ADDR_HELP\|FormatOptions\|clusterByteRange\|findEntrySlots\|buildChain" web/ui/src/shell/*.ts` prints nothing. The `touch` message on line 552 keeps "FAT explorer" until Task 7.

- [ ] **Step 18: `TestHost` mirrors the store: bind, refresh, re-bind**

Replace lines 1-69 of `web/ui/tests/shell/helpers.ts` (everything up to and including `makeHost`; `CallArgs` onwards is unchanged) with:

```ts
import { Volume, type OpRecord } from "../../src/lib/wasm";
import { FAMILIES, adapterFor } from "../../src/fs";
import type { FsAdapter, FsFamilyId } from "../../src/fs/adapter";
import type { ShellHost } from "../../src/shell/host";
import type { CommandDef } from "../../src/shell/commands";
import type { CommandCtx, Value } from "@benjamin-small/browser-terminal";

/**
 * A ShellHost over a plain Volume. `run` mirrors VolumeStore.run: it snaps
 * back to the latest step, applies the op, appends the record, leaves the
 * cursor at the end, and refreshes the adapter. `format` re-binds the adapter
 * the way `VolumeStore.adopt` does. `rewind` only moves the cursor (the volume
 * itself always holds the latest state, exactly as in the app).
 */
export class TestHost implements ShellHost {
  vol: Volume;
  adapter: FsAdapter;
  history: OpRecord[] = [];
  cursor = -1;
  selected: (string | null)[] = [];
  jumps: number[] = [];
  prompts: string[] = [];
  formats: { family: FsFamilyId; options: unknown }[] = [];
  closed = false;

  constructor(vol: Volume) {
    this.vol = vol;
    this.adapter = adapterFor(vol);
  }

  get historyLength(): number {
    return this.history.length;
  }

  run(fn: (v: Volume) => OpRecord): OpRecord {
    this.cursor = this.history.length - 1;
    const rec = fn(this.vol); // a wasm FsError ({ message, code }) propagates as-is
    this.history.push(rec);
    this.cursor = this.history.length - 1;
    this.adapter.refresh();
    return rec;
  }

  format(family: FsFamilyId, options?: unknown): void {
    this.vol = FAMILIES[family].format(options);
    this.adapter = adapterFor(this.vol);
    this.history = [];
    this.cursor = -1;
    this.formats.push({ family, options });
  }

  select(path: string | null): void {
    this.selected.push(path);
  }

  jumpTo(offset: number): void {
    this.jumps.push(offset);
  }

  setPrompt(prefix: string): void {
    this.prompts.push(prefix);
  }

  closeTerminal(): void {
    this.closed = true;
  }

  /** View step `step` (-1 = before any op) without touching the volume. */
  rewind(step: number): void {
    this.cursor = Math.max(-1, Math.min(step, this.history.length - 1));
  }
}

export function makeHost(vol: Volume = Volume.formatFat16(undefined)): TestHost {
  return new TestHost(vol);
}
```

`refresh()` after a raw write that wipes sector 0 is safe: `Volume.geometry()`, `clusterOwners()`, and `fatEntries(0)` are not behind the FAT crate's `ensure_mounted` gate (they answer from the last good geometry), which is why the `corruption` tests in `read-commands`, `dd`, and `redirect` keep passing.

- [ ] **Step 19: `read-commands.test.ts` computes its expectations from the adapter**

In `web/ui/tests/shell/read-commands.test.ts`, replace lines 2-4:

```ts
import { clusterByteRange } from "../../src/core/attribution";
import { findEntrySlots } from "../../src/core/direntry";
import { ADDR_HELP } from "../../src/shell/addr";
```

with:

```ts
import { addrHelp } from "../../src/shell/addr";
```

(If Task 3 already re-pointed the first two imports at `src/fs/fat16/...`, remove those two lines instead; nothing below uses them.) In the `stat` describe, replace lines 160-165:

```ts
  it("reports where a file lives: entry slot, FAT entry, chain, data", async () => {
    const { host, defs } = setup();
    const vol = host.vol;
    const g = vol.geometry(), fat = vol.fatEntries(0), owners = vol.clusterOwners();
    const first = owners.find((o) => o.path === "/DOCS/N.TXT")!.firstCluster;
    const slots = findEntrySlots(vol, g, fat, owners, "/DOCS/N.TXT")!;
```

with:

```ts
  it("reports where a file lives: entry slot, FAT entry, chain, data", async () => {
    const { host, defs } = setup();
    const fs = host.adapter;
    const g = host.vol.geometry();
    const first = fs.ownerOf("/DOCS/N.TXT")!.firstUnit;
    const slots = fs.entrySlots("/DOCS/N.TXT")!;
```

and line 178:

```ts
      dataOffset: `0x${clusterByteRange(g, first).start.toString(16)}`,
```

with:

```ts
      dataOffset: `0x${fs.unitByteRange(first).start.toString(16)}`,
```

The `fatEntryOffset` expectation on line 177 keeps `g.reservedSectors * g.bytesPerSector + first * 2` (tests may compute FAT expectations from the geometry). In the `seek, select, exit` describe, replace lines 225-233:

```ts
  it("seek jumps the dump to sector, cluster, hex, and decimal addresses", async () => {
    const { host, defs } = setup();
    const g = host.vol.geometry();
    await call(defs, "seek", ["s:65"]);
    await call(defs, "seek", ["c:3"]);
    await call(defs, "seek", ["0x200"]);
    await call(defs, "seek", ["512"]);
    expect(host.jumps).toEqual([65 * 512, clusterByteRange(g, 3).start, 512, 512]);
  });
```

with:

```ts
  it("seek jumps the dump to sector, cluster, hex, and decimal addresses", async () => {
    const { host, defs } = setup();
    await call(defs, "seek", ["s:65"]);
    await call(defs, "seek", ["c:3"]);
    await call(defs, "seek", ["0x200"]);
    await call(defs, "seek", ["512"]);
    expect(host.jumps).toEqual([65 * 512, host.adapter.unitByteRange(3).start, 512, 512]);
  });
```

and line 237:

```ts
    expect((await callErr(defs, "seek", ["nope"])).help).toBe(ADDR_HELP);
```

with:

```ts
    expect((await callErr(defs, "seek", ["nope"])).help).toBe(addrHelp(host.adapter));
```

Every asserted output (`stat`'s keys and hex strings, `df`'s `clusterSize`/`clusters`/`used: 4`, `mount`, the corruption messages) is untouched.

- [ ] **Step 20: Run the shell suite**

Run: `cd web/ui && pnpm vitest run tests/shell`

Expected: all 10 files pass (`addr`, `bytes`, `dd`, `errors`, `mutations`, `read-commands`, `redirect`, `vfs`, `xxd`, `xxd-command`), 0 failed. In particular `mutations.test.ts` line 45 (`FAT has no append; ...`) and line 182 (`FAT explorer has no timestamp-only update`), `dd.test.ts` line 134 (`FAT has no partial writes; ...`), the `mkfs` tests (`formatted /dev/hda as FAT16; the timeline was cleared`, `--spc must be a non-negative integer`), and `read-commands` `df` (`clusterSize: 2048, clusters: g.clusterCount, used: 4`) pass unchanged, which pins the adapter's `notes`, `mkfs`, `stat`, and `df` to today's strings.

- [ ] **Step 21: Retire the core shims nothing imports any more**

Run: `cd web/ui && grep -rln "core/direntry\|core/fatchain\|core/remnants" src tests`

Expected: `src/scenarios/directory.ts`, `src/scenarios/fundamentals.ts`, `src/scenarios/longName.ts`, `src/scenarios/smallFile.ts` (all `core/direntry`, until Task 6), `src/components/FatMap.svelte` (`core/fatchain`, until Task 5), and no file for `core/remnants` (Task 2 pointed `tests/remnants.test.ts` and `tests/corruption.test.ts` at `src/fs/fat16/remnants`; `layers.svelte.ts` stopped importing it in Step 2). Then run `grep -rn "core/remnants" src tests` on its own: when it prints nothing, delete the shim:

```
git rm web/ui/src/core/remnants.ts
```

If it does print a file, leave `core/remnants.ts` in place and let Task 5 remove it. `core/direntry.ts` and `core/fatchain.ts` stay either way.

- [ ] **Step 22: Run the full suite, and confirm the expected red build**

Run: `cd web/ui && pnpm test`

Expected: every file passes (the 34 files that pass today plus the `tests/fs/` files Task 2 added), 0 failed.

Run: `cd web/ui && pnpm build`

Expected: **fails in svelte-check, as constraint 8 says it must until Task 5.** Every error is in `src/components/`: `ActionsPanel.svelte` (`volume.format(options)` now takes a family id first), `DirTree.svelte`, `Ribbon.svelte`, `Inspector.svelte`, and `FatMap.svelte` (`volume.owners`, `volume.fat`, `volume.geometry` no longer exist), `HexView.svelte` (`volume.geometry` and `parseAddr(v, volume.geometry)`), `TreeNodeView.svelte` (`clusterByteRange(volume.geometry, ...)`), and `LessonPanel.svelte` (`describeFocus(..., volume.geometry, ...)`). No error is reported under `src/state/`, `src/shell/`, `src/scenarios/`, or `tests/`. Task 5 migrates the components and turns the build green again.

- [ ] **Step 23: Commit**

```
git add web/ui/src/state/volume.svelte.ts web/ui/src/state/layers.svelte.ts web/ui/src/state/scenarios.svelte.ts web/ui/src/scenarios/deleteRemnants.ts web/ui/src/scenarios/directory.ts web/ui/src/scenarios/fundamentals.ts web/ui/src/scenarios/shell.ts web/ui/src/scenarios/smallFile.ts web/ui/src/shell/addr.ts web/ui/src/shell/host.ts web/ui/src/shell/storeHost.svelte.ts web/ui/src/shell/commands.ts web/ui/src/shell/dd.ts web/ui/src/shell/vfs.ts web/ui/src/shell/errors.ts web/ui/tests/shell/helpers.ts web/ui/tests/shell/addr.test.ts web/ui/tests/shell/read-commands.test.ts web/ui/tests/shell/errors.test.ts web/ui/tests/shell/vfs.test.ts
git commit -m "refactor(ui): route the store, layers, and the shell through the FsAdapter" -m "VolumeStore binds the family adapter on adopt and refreshes it after every op; sectorSize and totalSectors are plain state and format(family, options) goes through the registry. The layers derive the chain, entry slots, remnants, and selection ranges from the adapter behind an epoch read. The shell reaches unit arithmetic, stat and df facts, mkfs flags, address help, and the rewrite notes through host.adapter: parseAddr and addrHelp take an AddrSpace, canonicalize takes the adapter's namesMatch, and CORRUPT_HELP is family-neutral. StepFocus.cluster becomes unit. Every shell output string is unchanged; pnpm build stays red until the components migrate in Task 5."
```

(The `git rm` in Step 21, when it ran, is already staged.)
### Task 5: web/ui: components through the adapter

Spec `docs/superpowers/specs/2026-09-23-fs-adapter-design.md`: section 3's components table, section 2's `FatMap.svelte` and `FormatForm.svelte` rows and its `fs/panels.ts` paragraph, and the reactivity rule (sections 1 and 3) at the Inspector's new `trace` derived and the Lesson card's `lookAt`.

After Task 4 the store, the layers, the scenario runner, and the shell read the volume through `volume.adapter`, and `svelte-check` is red only in the components, which still read `volume.geometry`, `volume.owners`, `volume.fat`, and the cluster helpers. This task rewrites those components against the adapter, moves the two FAT-only panels into `src/fs/fat16/`, and adds the panel registry that `App` and `ActionsPanel` render from. Every path below is under `/Users/bsmall/dev/fs-emulator/`; every `pnpm` command runs from `web/ui`. Line numbers are today's (HEAD `81b0b56`). Vitest has no Svelte plugin, so a component edit is verified by `svelte-check` narrowed to that file; the task ends with the whole build green and a browser pass. The narrowed check, used by every run step below, is:

```
pnpm exec svelte-check --tsconfig ./tsconfig.json --output machine 2>&1 | grep -E "^[0-9]+ (ERROR|WARNING) " | grep '"src/<file>"'
```

Expected for a clean file: no output (`grep` exits 1). A problem line looks like `1790188321113 ERROR "src/components/HexView.svelte" 81:41 "Property 'geometry' does not exist on type 'VolumeStore'."`.

**Files:**
- Create: `web/ui/src/fs/panels.ts`
- Create: `web/ui/src/fs/fat16/FormatForm.svelte`
- Move: `web/ui/src/components/FatMap.svelte` to `web/ui/src/fs/fat16/FatMap.svelte` (`git mv`; edits at today's lines 2-3, 15, 51, 74, 85-86, 125, 146-148, 153, 156-157)
- Modify: `web/ui/src/App.svelte` (imports lines 2-20, left column lines 84-88; the h1 at line 55 stays "FAT explorer", Task 7 renames it)
- Modify: `web/ui/src/components/ActionsPanel.svelte` (lines 2-15, 21-46, 82-86, 136-158)
- Modify: `web/ui/src/components/HexView.svelte` (lines 80-83, 102, 108, 129)
- Modify: `web/ui/src/components/HexRow.svelte` (lines 4, 7)
- Modify: `web/ui/src/app.css` (line 80)
- Modify: `web/ui/src/components/Inspector.svelte` (lines 2-7, 16, 20-39, 48, 68-77)
- Modify: `web/ui/src/components/TreeNodeView.svelte` (lines 2-4, 15, 28)
- Modify: `web/ui/src/components/Ribbon.svelte` (lines 40, 106-108, 183-185, 225-228, 255, 259-267)
- Modify: `web/ui/src/components/StatusLine.svelte` (lines 4-22)
- Modify: `web/ui/src/components/DirTree.svelte` (lines 7, 12-18)
- Modify: `web/ui/src/components/LessonPanel.svelte` (lines 19-21)
- Delete: `web/ui/src/core/direntry.ts`, `web/ui/src/core/fatchain.ts`, `web/ui/src/core/remnants.ts` (the re-exporting shims Task 2 left, whichever Task 4's Step 21 has not already removed; step 14 re-points any importer still on them and then removes them, step 15 asserts all three are gone)
- Modify (step 14, only where the grep there finds the import): `web/ui/src/scenarios/directory.ts`, `web/ui/src/scenarios/fundamentals.ts`, `web/ui/src/scenarios/longName.ts`, `web/ui/src/scenarios/smallFile.ts` (line 1, the `findEntrySlots` import path only; Task 6 rewrites these files whole)
- Test: none added (a node test cannot import a `.svelte` file); the gates are `pnpm test`, `pnpm build`, and the browser pass in step 15

**Interfaces:**
- Consumes, from Task 2 (`web/ui/src/fs/adapter.ts`, section 1 of the spec): `type FsFamilyId = "fat16"`; `UnitVocab { singular: string; plural: string; letter: string; first: number }`; `UnitOwner { unit; path; isDir; firstUnit }`; `UnitSpace` with `readonly unit: UnitVocab`, `readonly unitCount: number`, `readonly unitSize: number`, `readonly sectorSize: number`, `readonly totalSectors: number`, `unitByteRange(unit: number): Interval`, `unitStartsAt(sector: number): boolean`; `FsAdapter extends UnitSpace` with `readonly id: FsFamilyId`, `readonly name: string`, `readonly owners: readonly UnitOwner[]`, `remnants?(zeros: Uint8Array): Interval[]`, `trace(path: string): TraceRow[]`, `describeUnit(unit: number): string | null`, `annotateSector(sector: number): Annotation[]`, `readonly corruptNote: string`, and `parseAddr?` so that an `FsAdapter` satisfies `shell/addr.ts`'s `AddrSpace`; `TraceRow { label: string; offset: number | null }` (null = muted text, no jump).
- Consumes, from Task 2 (`web/ui/src/fs/fat16/`): `index.ts` exports `asFat16(fs: FsAdapter): Fat16Adapter` (throws unless `fs.id === "fat16"`) and `type Fat16FormatOptions = FormatOptions`; `adapter.ts` exports `class Fat16Adapter implements FsAdapter` with the public field `fat: FatEntry[]`; `fatchain.ts` exports `clusterState(e: FatEntry): ClusterState` (`"free" | "used" | "end" | "bad" | "reserved"`); `format.ts` exports `SIZES: { label: string; totalSectors: number }[]` (`4 MB`/8192, `16 MB`/32768, `64 MB`/131072), `CLUSTER_SIZES: number[]` (`[1, 2, 4, 8]`), `checkFormat(o: { totalSectors: number; sectorsPerCluster: number }): { clusters: number; problem: string | null }` (problem `"too few for FAT16"`, `"too many for FAT16"`, or `null`), and `DEFAULTS` with non-optional `totalSectors: number` (32768), `sectorsPerCluster: number` (4), `volumeLabel: string` (`""`).
- Consumes, from Task 3: `core/attribution.ts` `Attr.unit?: number`, `AttributionTable { owners: readonly UnitOwner[]; ownerByUnit: Int32Array; colorByUnit: Uint8Array }`, `attrAtOffset(t, offset): Attr`, `attrAtSector(t, sector): Attr`; `core/tree.ts` `buildTree(vol: Volume, owners: readonly UnitOwner[]): TreeNode` and `TreeNode.firstUnit: number | null`; `core/lesson.ts` `describeFocus(focus: StepFocus | null | undefined, space: UnitSpace, regionNameAt: (sector: number) => string): string | null`; `shell/addr.ts` `parseAddr(v: string | number, space: AddrSpace): number`.
- Consumes, from Task 4: `state/volume.svelte.ts` `volume.adapter: FsAdapter` (`$state.raw`, re-bound by `adopt`), `volume.sectorSize: number`, `volume.totalSectors: number`, `volume.attribution: AttributionTable`, `volume.epoch: number`, `volume.format(family?: FsFamilyId, options?: unknown): void`, and the unchanged `vol`, `image`, `zeros`, `history`, `cursor`, `status`, `corruption`, `atLatest`, `run`, `load`, `export`, `seek`, `backToNow`; `state/layers.svelte.ts` `layers.chain: number[]`, `layers.diff`, `layers.sel`, `layers.str`, `layers.remnant`, `layers.visible`, `layers.pinnedSectors`; `state/scenarios.svelte.ts` `scenarios.focus: StepFocus | null` with `StepFocus.unit?: number`; `state/selection.svelte.ts` unchanged.
- Consumes, from `svelte@5.57.1`: `import type { Component } from "svelte"` (the default export of a runes component with no props is assignable to `Component`).
- Produces: `web/ui/src/fs/panels.ts` `export const PANELS: Record<FsFamilyId, { map: Component; format: Component }>` = `{ fat16: { map: FatMap, format: FormatForm } }`; `web/ui/src/fs/fat16/FatMap.svelte` and `web/ui/src/fs/fat16/FormatForm.svelte` (no props; the form calls `volume.format("fat16", options)`); `HexRow` prop `unitStart: boolean` and the CSS class `.row.unit-start`; components with no `volume.geometry`, `volume.owners`, `volume.fat`, `clusterByteRange`, `annotateSectorWith`, `ownerByCluster`, `colorByCluster`, `FatEntry`, or `FormatOptions` reference, and with `src/App.svelte` and `src/components/**` importing from `fs/panels` only, never from `fs/fat16` (what Task 7's `tests/adapterBoundary.test.ts` scans for).

---

- [ ] **Step 1: Create the FAT16 Format form**

Create `web/ui/src/fs/fat16/FormatForm.svelte`, the body of ActionsPanel's `<details class="format">` (today's lines 138-157) over `format.ts`:

```svelte
<script lang="ts">
  import { selection } from "../../state/selection.svelte";
  import { volume } from "../../state/volume.svelte";
  import { CLUSTER_SIZES, DEFAULTS, SIZES, checkFormat } from "./format";
  import type { Fat16FormatOptions } from "./index";

  // Mirror the mounted default disk (16 MB, 4 sectors per cluster) so opening the
  // form shows the geometry that is already on screen.
  let totalSectors = $state<number>(DEFAULTS.totalSectors);
  let sectorsPerCluster = $state<number>(DEFAULTS.sectorsPerCluster);
  let volumeLabel = $state<string>(DEFAULTS.volumeLabel);

  /** The cluster count these options would produce, by the core's rule, and the FAT16
   *  bound it breaks, if any. */
  const check = $derived(checkFormat({ totalSectors, sectorsPerCluster }));

  function formatDisk() {
    const options: Fat16FormatOptions = { totalSectors, sectorsPerCluster, volumeLabel };
    volume.format("fat16", options);
    selection.select(null);
  }
</script>

<label class="field">
  Size
  <select bind:value={totalSectors}>
    {#each SIZES as s}<option value={s.totalSectors}>{s.label}</option>{/each}
  </select>
</label>
<label class="field">
  Sectors per cluster
  <select bind:value={sectorsPerCluster}>
    {#each CLUSTER_SIZES as n}<option value={n}>{n}</option>{/each}
  </select>
</label>
<p class="cluster-count muted">
  {check.clusters.toLocaleString()} clusters{#if check.problem}{" "}· <span class="warn">{check.problem}</span>{/if}
</p>
<label class="field">
  Volume label
  <input class="mono" type="text" maxlength="11" bind:value={volumeLabel} />
</label>
<button onclick={formatDisk} disabled={check.problem !== null}>Format disk</button>
```

The markup is today's, byte for byte, except that `clusters`/`clusterProblem` are read off `check`; `.actions .field`, `.actions .cluster-count`, and `.actions .warn` in `app.css` still apply because the form renders inside `<section class="panel actions">`.

- [ ] **Step 2: Check the form**

Run: `pnpm exec svelte-check --tsconfig ./tsconfig.json --output machine 2>&1 | grep -E "^[0-9]+ (ERROR|WARNING) " | grep '"src/fs/fat16/FormatForm.svelte"'`

Expected: no output.

- [ ] **Step 3: Move the FAT map into the family and read the adapter**

Run: `git mv src/components/FatMap.svelte src/fs/fat16/FatMap.svelte`

Then replace the whole of `web/ui/src/fs/fat16/FatMap.svelte` with the following. Against today's file the changes are: the imports (lines 2-3, `attrAtOffset` alone from `core/attribution`; `clusterState` from `./fatchain`; `asFat16` from `./index`); `fs` and `clusterCount` (line 15); `fs.unitByteRange(c)` for `clusterByteRange(volume.geometry, c)` (lines 51, 148); `fs.fat` for `volume.fat` (lines 74, 153); `ownerByUnit`/`colorByUnit` for `ownerByCluster`/`colorByCluster` (lines 85-86, 146, 156); `.unit` for `.cluster` (line 125); and an `epoch` read at the top of `caption`.

```svelte
<script lang="ts">
  import { attrAtOffset } from "../../core/attribution";
  import { layers } from "../../state/layers.svelte";
  import { selection } from "../../state/selection.svelte";
  import { volume } from "../../state/volume.svelte";
  import { clusterState } from "./fatchain";
  import { asFat16 } from "./index";

  const CELL = 6, GAP = 1, MAX_HEIGHT = 260;

  let wrap = $state<HTMLDivElement>();
  let canvas = $state<HTMLCanvasElement>();
  let width = $state(0);
  let hoverCluster = $state<number | null>(null);

  // The FAT view of the volume's adapter: `fat` (the table), `unitCount`, `unitByteRange`.
  // Its caches are plain fields refreshed per op, not reactive, so every `$derived` and
  // `$effect` below that reads them reads `volume.epoch` first (the rule in fs/adapter.ts).
  const fs = $derived(asFat16(volume.adapter));
  const clusterCount = $derived((volume.epoch, fs.unitCount));
  const cols = $derived(Math.max(1, Math.floor(width / (CELL + GAP))));
  const rows = $derived(Math.max(1, Math.ceil(clusterCount / cols)));
  const canvasWidth = $derived(Math.max(1, Math.floor(width)));
  const canvasHeight = $derived(Math.max(1, rows * (CELL + GAP)));

  // Track the panel's content width so `cols` follows the sidebar's actual size
  // (a resizable layout, a narrower viewport, …) rather than a value baked in at mount.
  $effect(() => {
    if (!wrap) return;
    width = wrap.getBoundingClientRect().width;
    const ro = new ResizeObserver((entries) => { width = entries[0].contentRect.width; });
    ro.observe(wrap);
    return () => ro.disconnect();
  });

  $effect(() => {
    volume.epoch; layers.chain; layers.diff; selection.hoverOffset; canvasWidth; canvasHeight;
    paint();
  });

  function cellRect(c: number): { x: number; y: number } {
    const i = c - 2, col = i % cols, row = Math.floor(i / cols);
    return { x: col * (CELL + GAP), y: row * (CELL + GAP) };
  }

  function clusterAtPoint(px: number, py: number): number | null {
    const col = Math.floor(px / (CELL + GAP));
    const row = Math.floor(py / (CELL + GAP));
    if (col < 0 || col >= cols || row < 0) return null;
    const c = row * cols + col + 2;
    return c >= 2 && c <= clusterCount + 1 ? c : null;
  }

  function overlapsDiff(c: number): boolean {
    if (!layers.diff.length) return false;
    const { start, end } = fs.unitByteRange(c);
    for (const iv of layers.diff) if (iv.start < end && iv.end > start) return true;
    return false;
  }

  function paint() {
    if (!canvas) return;
    canvas.width = canvasWidth;
    canvas.height = canvasHeight;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);

    // Read once per paint (not cached across paints) so a light/dark switch is
    // picked up on the very next repaint without any extra wiring.
    const style = getComputedStyle(canvas);
    const hairline = style.getPropertyValue("--hairline").trim();
    const diffInk = style.getPropertyValue("--diff-ink").trim();
    const diffColor = style.getPropertyValue("--diff").trim();
    const focus = style.getPropertyValue("--focus").trim();
    const ink = style.getPropertyValue("--ink").trim();
    const own = (i: number) => style.getPropertyValue(`--own-${i}`).trim();

    const fat = fs.fat;
    const attribution = volume.attribution;

    for (let c = 2; c <= clusterCount + 1; c++) {
      const entry = fat[c];
      if (!entry) continue;
      const { x, y } = cellRect(c);
      const state = clusterState(entry);
      let fill = hairline;
      if (state === "bad") fill = diffInk;
      else {
        const idx = attribution.ownerByUnit[c];
        if (idx >= 0) fill = own(attribution.colorByUnit[c]);
      }
      ctx.fillStyle = fill;
      ctx.fillRect(x, y, CELL, CELL);

      if (state === "end") {
        ctx.fillStyle = ink;
        ctx.fillRect(x + 2, y + 2, 2, 2);
      }

      if (overlapsDiff(c)) {
        ctx.strokeStyle = diffColor;
        ctx.lineWidth = 1;
        ctx.strokeRect(x + 0.5, y + 0.5, CELL - 1, CELL - 1);
      }
    }

    const chain = layers.chain;
    if (chain.length) {
      ctx.strokeStyle = focus;
      ctx.lineWidth = 1;
      for (const c of chain) {
        const { x, y } = cellRect(c);
        ctx.strokeRect(x + 0.5, y + 0.5, CELL - 1, CELL - 1);
      }
      ctx.lineWidth = 2;
      ctx.beginPath();
      chain.forEach((c, i) => {
        const { x, y } = cellRect(c);
        const cx = x + CELL / 2, cy = y + CELL / 2;
        if (i === 0) ctx.moveTo(cx, cy); else ctx.lineTo(cx, cy);
      });
      ctx.stroke();
    }

    // Cross-link with the hex dump: whichever cluster the mouse is currently over
    // there gets a dashed outline here, so the map shows where in the whole disk
    // the hovered byte lives.
    if (selection.hoverOffset !== null) {
      const c = attrAtOffset(attribution, selection.hoverOffset).unit;
      if (c !== undefined) {
        const { x, y } = cellRect(c);
        ctx.save();
        ctx.strokeStyle = focus;
        ctx.setLineDash([1, 1]);
        ctx.lineWidth = 1;
        ctx.strokeRect(x + 0.5, y + 0.5, CELL - 1, CELL - 1);
        ctx.restore();
      }
    }
  }

  function onMove(e: MouseEvent) {
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    hoverCluster = clusterAtPoint(e.clientX - rect.left, e.clientY - rect.top);
  }
  function onLeave() { hoverCluster = null; }
  function onClick() {
    if (hoverCluster === null) return;
    const idx = volume.attribution.ownerByUnit[hoverCluster];
    if (idx >= 0) selection.select(volume.attribution.owners[idx].path);
    selection.jumpTo(fs.unitByteRange(hoverCluster).start);
  }

  const caption = $derived.by(() => {
    if (hoverCluster === null) return "";
    volume.epoch;
    const entry = fs.fat[hoverCluster];
    if (!entry) return "";
    const state = clusterState(entry);
    const idx = volume.attribution.ownerByUnit[hoverCluster];
    const owner = idx >= 0 ? volume.attribution.owners[idx].path : "—";
    return `cluster ${hoverCluster} · ${state} · ${owner}`;
  });
</script>

<section class="panel fatmap">
  <h2>FAT map &middot; {clusterCount.toLocaleString()} clusters</h2>
  {#if !volume.atLatest}<p class="muted stale-note">Shows the latest state, not the step you are viewing.</p>{/if}
  <div class="fatmap-wrap" bind:this={wrap} style:max-height="{MAX_HEIGHT}px">
    <canvas bind:this={canvas} onmousemove={onMove} onmouseleave={onLeave} onclick={onClick} aria-label="FAT cluster map"></canvas>
  </div>
  <p class="mono muted fatmap-caption">{caption || " "}</p>
</section>
```

The heading, the `aria-label`, the `.fatmap*` classes, and the caption text are unchanged on purpose (spec section 5). `clusterCount` re-reads `volume.epoch` itself rather than riding on `fs`: `asFat16(volume.adapter)` returns the same object after every op, so a derived that only depended on `fs` would keep the old count.

- [ ] **Step 4: Check the moved map**

Run: `ls src/components/FatMap.svelte; pnpm exec svelte-check --tsconfig ./tsconfig.json --output machine 2>&1 | grep -E "^[0-9]+ (ERROR|WARNING) " | grep '"src/fs/fat16/FatMap.svelte"'`

Expected: `ls: src/components/FatMap.svelte: No such file or directory`, then no problem lines.

- [ ] **Step 5: Create the panel registry**

Create `web/ui/src/fs/panels.ts`:

```ts
import type { Component } from "svelte";
import type { FsFamilyId } from "./adapter";
import FatMap from "./fat16/FatMap.svelte";
import FormatForm from "./fat16/FormatForm.svelte";

/**
 * The Svelte panels of each family, looked up by `volume.adapter.id`: `map` is the panel App
 * renders under the Files tree (the FAT map), `format` the body of the Actions panel's Format
 * details. They live here rather than on the adapter because `vitest.config.ts` has no Svelte
 * plugin: an adapter that imported a `.svelte` file could not be loaded by the node tests.
 */
export const PANELS: Record<FsFamilyId, { map: Component; format: Component }> = {
  fat16: { map: FatMap, format: FormatForm },
};
```

- [ ] **Step 6: Check the registry**

Run: `pnpm exec svelte-check --tsconfig ./tsconfig.json --output machine 2>&1 | grep -E "^[0-9]+ (ERROR|WARNING) " | grep '"src/fs/panels.ts"'`

Expected: no output (both `.svelte` defaults are accepted as `Component`).

- [ ] **Step 7: App renders the map panel from the registry**

`web/ui/src/App.svelte`. Replace (lines 3-5):

```svelte
  import DirTree from "./components/DirTree.svelte";
  import FatMap from "./components/FatMap.svelte";
  import HexView from "./components/HexView.svelte";
```

with:

```svelte
  import DirTree from "./components/DirTree.svelte";
  import HexView from "./components/HexView.svelte";
```

Replace (line 15):

```svelte
  import { inTextEntry } from "./core/keys";
```

with:

```svelte
  import { inTextEntry } from "./core/keys";
  import { PANELS } from "./fs/panels";
```

Replace (lines 20-22):

```svelte
  import { volume } from "./state/volume.svelte";

  /** Scrub to `n`
```

with:

```svelte
  import { volume } from "./state/volume.svelte";

  /** The map panel of the mounted volume's family (the FAT map today), from the panel
   *  registry; `volume.adapter` is re-bound on every format and load. */
  const MapPanel = $derived(PANELS[volume.adapter.id].map);

  /** Scrub to `n`
```

Replace (lines 85-87):

```svelte
      <DirTree />
      <FatMap />
      <ActionsPanel />
```

with:

```svelte
      <DirTree />
      <MapPanel />
      <ActionsPanel />
```

Line 55, `<h1>FAT explorer</h1>`, is not touched.

Run: `pnpm exec svelte-check --tsconfig ./tsconfig.json --output machine 2>&1 | grep -E "^[0-9]+ (ERROR|WARNING) " | grep '"src/App.svelte"'`

Expected: no output.

- [ ] **Step 8: ActionsPanel renders the Format form from the registry**

`web/ui/src/components/ActionsPanel.svelte`. Replace (lines 2-17):

```svelte
  import { volume } from "../state/volume.svelte";
  import { selection } from "../state/selection.svelte";
  import type { FormatOptions } from "../lib/wasm";

  const SIZES: { label: string; totalSectors: number }[] = [
    { label: "4 MB", totalSectors: 8192 },
    { label: "16 MB", totalSectors: 32768 },
    { label: "64 MB", totalSectors: 131072 },
  ];
  const CLUSTER_SIZES = [1, 2, 4, 8];

  // FAT16 geometry constants, matching the core's formatter.
  const BYTES_PER_SECTOR = 512, ROOT_ENTRIES = 512, DIR_ENTRY = 32, RESERVED = 1, FAT_COPIES = 2;
  const FAT16_MIN_CLUSTERS = 4085, FAT16_MAX_CLUSTERS = 65524;

  let path
```

with:

```svelte
  import { volume } from "../state/volume.svelte";
  import { selection } from "../state/selection.svelte";
  import { PANELS } from "../fs/panels";

  /** The Format form of the mounted volume's family, from the panel registry. */
  const FormatPanel = $derived(PANELS[volume.adapter.id].format);

  let path
```

Replace (lines 21-48):

```svelte
  let bytesInput = $state<HTMLInputElement>();

  // Mirror the mounted default disk (16 MB, 4 sectors per cluster) so opening the
  // form shows the geometry that is already on screen.
  let totalSectors = $state(32768);
  let sectorsPerCluster = $state(4);
  let volumeLabel = $state("");

  /** The cluster count these options would produce, by the core's rule: the smallest
   *  sectors-per-FAT that can index every cluster the leftover space yields. */
  function clusterCountFor(total: number, spc: number): number {
    const rootDirSectors = Math.ceil((ROOT_ENTRIES * DIR_ENTRY) / BYTES_PER_SECTOR);
    const entriesPerFatSector = BYTES_PER_SECTOR / 2; // FAT16 entries are 2 bytes
    for (let spf = 1; spf <= total; spf++) {
      const usable = total - RESERVED - FAT_COPIES * spf - rootDirSectors;
      if (usable <= 0) return 0;
      const clusters = Math.floor(usable / spc);
      if (spf * entriesPerFatSector >= clusters + 2) return clusters;
    }
    return 0;
  }

  const clusters = $derived(clusterCountFor(totalSectors, sectorsPerCluster));
  const clusterProblem = $derived(
    clusters < FAT16_MIN_CLUSTERS ? "too few for FAT16" : clusters > FAT16_MAX_CLUSTERS ? "too many for FAT16" : "",
  );

  function data()
```

with:

```svelte
  let bytesInput = $state<HTMLInputElement>();

  function data()
```

Replace (lines 82-88):

```svelte
  function formatDisk() {
    const options: FormatOptions = { totalSectors, sectorsPerCluster, volumeLabel };
    volume.format(options);
    selection.select(null);
  }

  async function onLoadImage
```

with:

```svelte
  async function onLoadImage
```

Replace (lines 136-158):

```svelte
    <details class="format">
      <summary>Format</summary>
      <label class="field">
        Size
        <select bind:value={totalSectors}>
          {#each SIZES as s}<option value={s.totalSectors}>{s.label}</option>{/each}
        </select>
      </label>
      <label class="field">
        Sectors per cluster
        <select bind:value={sectorsPerCluster}>
          {#each CLUSTER_SIZES as n}<option value={n}>{n}</option>{/each}
        </select>
      </label>
      <p class="cluster-count muted">
        {clusters.toLocaleString()} clusters{#if clusterProblem}{" "}· <span class="warn">{clusterProblem}</span>{/if}
      </p>
      <label class="field">
        Volume label
        <input class="mono" type="text" maxlength="11" bind:value={volumeLabel} />
      </label>
      <button onclick={formatDisk} disabled={!!clusterProblem}>Format disk</button>
    </details>
```

with:

```svelte
    <details class="format">
      <summary>Format</summary>
      <FormatPanel />
    </details>
```

Run: `pnpm exec svelte-check --tsconfig ./tsconfig.json --output machine 2>&1 | grep -E "^[0-9]+ (ERROR|WARNING) " | grep '"src/components/ActionsPanel.svelte"'`

Expected: no output.

- [ ] **Step 9: The dump names units through the adapter**

`web/ui/src/components/HexView.svelte`. Replace (lines 80-83):

```ts
    const sectorStart = offset % volume.sectorSize === 0;
    const clusterStart = attr.cluster !== undefined && sectorStart && (attr.sector - volume.geometry.firstDataSector) % volume.geometry.sectorsPerCluster === 0;
    const label = clusterStart ? `cluster ${attr.cluster}${attr.ownerPath ? ` · ${attr.ownerPath}` : ""}` : sectorStart ? `sector ${attr.sector}${attr.cluster === undefined ? ` · ${attr.regionName}` : ""}` : "";
    return { kind: "row" as const, key: r, offset, attr, cells, sectorStart, clusterStart, label };
```

with:

```ts
    const sectorStart = offset % volume.sectorSize === 0;
    // `unitStartsAt` is an adapter method (plain caches): `rowsInView`, the only caller of
    // `describeRow`, reads `volume.epoch` first.
    const { unit } = volume.adapter;
    const unitStart = attr.unit !== undefined && sectorStart && volume.adapter.unitStartsAt(attr.sector);
    const label = unitStart ? `${unit.singular} ${attr.unit}${attr.ownerPath ? ` · ${attr.ownerPath}` : ""}` : sectorStart ? `sector ${attr.sector}${attr.unit === undefined ? ` · ${attr.regionName}` : ""}` : "";
    return { kind: "row" as const, key: r, offset, attr, cells, sectorStart, unitStart, label };
```

Replace (line 102):

```ts
      case "g": { const v = globalThis.prompt("Jump to offset (0x…, decimal, s:sector, c:cluster)"); if (v) jump(v); return; }
```

with:

```ts
      case "g": { const { letter, singular } = volume.adapter.unit; const v = globalThis.prompt(`Jump to offset (0x…, decimal, s:sector, ${letter}:${singular})`); if (v) jump(v); return; }
```

Replace (line 108):

```ts
    try { off = parseAddr(v, volume.geometry); } catch { return; } // the prompt ignores bad input silently, as before
```

with:

```ts
    try { off = parseAddr(v, volume.adapter); } catch { return; } // the prompt ignores bad input silently, as before
```

Replace (line 129):

```svelte
            <HexRow offset={r.offset} attr={r.attr} cells={r.cells} sectorStart={r.sectorStart} clusterStart={r.clusterStart} label={r.label} />
```

with:

```svelte
            <HexRow offset={r.offset} attr={r.attr} cells={r.cells} sectorStart={r.sectorStart} unitStart={r.unitStart} label={r.label} />
```

For FAT the prompt reads exactly what it read before: `Jump to offset (0x…, decimal, s:sector, c:cluster)`.

`web/ui/src/components/HexRow.svelte`. Replace (line 4):

```ts
  let { offset, attr, cells, sectorStart, clusterStart, label = "" }: { offset: number; attr: Attr; cells: Cell[]; sectorStart: boolean; clusterStart: boolean; label?: string } = $props();
```

with:

```ts
  let { offset, attr, cells, sectorStart, unitStart, label = "" }: { offset: number; attr: Attr; cells: Cell[]; sectorStart: boolean; unitStart: boolean; label?: string } = $props();
```

Replace (line 7):

```svelte
<div class="row own-{attr.colorIndex}" class:sector-start={sectorStart} class:cluster-start={clusterStart} data-offset={offset}>
```

with:

```svelte
<div class="row own-{attr.colorIndex}" class:sector-start={sectorStart} class:unit-start={unitStart} data-offset={offset}>
```

`web/ui/src/app.css`. Replace (line 80):

```css
.row.cluster-start { border-top-color: var(--ink-muted); }
```

with:

```css
.row.unit-start { border-top-color: var(--ink-muted); }
```

Run: `grep -rn "cluster-start\|clusterStart" src; pnpm exec svelte-check --tsconfig ./tsconfig.json --output machine 2>&1 | grep -E "^[0-9]+ (ERROR|WARNING) " | grep '"src/components/HexView.svelte"\|"src/components/HexRow.svelte"'`

Expected: the `grep` prints nothing, and no problem lines. (`tests/layout.test.ts` reads `app.css` for `.app` rules only, so `pnpm test` is unaffected.)

- [ ] **Step 10: The Inspector annotates, names the unit, and traces through the adapter**

`web/ui/src/components/Inspector.svelte`. Replace (lines 2-7):

```ts
  import { volume } from "../state/volume.svelte";
  import { selection } from "../state/selection.svelte";
  import { layers } from "../state/layers.svelte";
  import { attrAtOffset, clusterByteRange } from "../core/attribution";
  import type { Annotation, FatEntry } from "../lib/wasm";
  import { describeRange, formatRange, formatValue } from "../core/annotationFormat";
```

with:

```ts
  import { volume } from "../state/volume.svelte";
  import { selection } from "../state/selection.svelte";
  import { attrAtOffset } from "../core/attribution";
  import type { Annotation } from "../lib/wasm";
  import { describeRange, formatRange, formatValue } from "../core/annotationFormat";
```

Replace (line 16):

```ts
    if (!a) { if (memo.size > 64) memo.clear(); a = volume.vol.annotateSectorWith(attr.sector, volume.owners); memo.set(key, a); }
```

with:

```ts
    if (!a) { if (memo.size > 64) memo.clear(); a = volume.adapter.annotateSector(attr.sector); memo.set(key, a); }
```

(The memo key already carries `volume.epoch`, line 14, so this derived obeys the reactivity rule as it stands.)

Replace (lines 20-39):

```ts
  const hex = (n: number) => "0x" + n.toString(16);

  // The three places a selected file lives on disk, each one a jump target: its
  // directory entry, its FAT chain, and the first cluster of its data.
  const firstCluster = $derived(layers.chain[0] ?? null);
  const fatEntryOffset = $derived(
    firstCluster === null ? null : volume.geometry.reservedSectors * volume.geometry.bytesPerSector + firstCluster * 2,
  );
  const dataOffset = $derived(firstCluster === null ? null : clusterByteRange(volume.geometry, firstCluster).start);

  function describe(e: FatEntry): string {
    switch (e.kind) {
      case "free": return "free";
      case "next": return `next → ${e.cluster}`;
      case "endOfChain": return "end of chain";
      case "bad": return "bad";
      case "reserved": return "reserved";
    }
  }
</script>
```

with:

```ts
  const hex = (n: number) => "0x" + n.toString(16);
  const cap = (s: string) => s[0].toUpperCase() + s.slice(1);

  // What the family's table says about the unit under the cursor ("FAT: end of chain"), or
  // null when it has nothing to say. `describeUnit` and `trace` are adapter methods over
  // plain caches, so both deriveds read `volume.epoch` first (the rule in fs/adapter.ts).
  const unitNote = $derived.by(() => {
    volume.epoch;
    return attr === null || attr.unit === undefined ? null : volume.adapter.describeUnit(attr.unit);
  });

  // The places a selected file lives on disk, in the family's words, each one a jump
  // target unless its offset is null (FAT: its directory entry, its FAT chain, and the
  // first cluster of its data).
  const trace = $derived.by(() => {
    volume.epoch;
    return selection.path ? volume.adapter.trace(selection.path) : [];
  });
</script>
```

Replace (line 48):

```svelte
      {#if attr.cluster !== undefined}<dt>Cluster</dt><dd class="mono">{attr.cluster}{#if volume.fat[attr.cluster]} · FAT: {describe(volume.fat[attr.cluster])}{/if}</dd>{/if}
```

with:

```svelte
      {#if attr.unit !== undefined}<dt>{cap(volume.adapter.unit.singular)}</dt><dd class="mono">{attr.unit}{#if unitNote} · {unitNote}{/if}</dd>{/if}
```

Replace (lines 68-77):

```svelte
      <ul class="trace">
        {#if layers.entry}
          <li><button class="link" onclick={() => selection.jumpTo(layers.entry!.start)}>Directory entry · offset {hex(layers.entry.start)}</button></li>
        {/if}
        {#if firstCluster !== null && fatEntryOffset !== null && dataOffset !== null}
          <li><button class="link" onclick={() => selection.jumpTo(fatEntryOffset!)}>FAT chain · {layers.chain.length} clusters starting at {firstCluster} · FAT entry at {hex(fatEntryOffset)}</button></li>
          <li><button class="link" onclick={() => selection.jumpTo(dataOffset!)}>Data · cluster {firstCluster} at {hex(dataOffset)}</button></li>
        {:else}
          <li class="muted">Data · no data clusters</li>
        {/if}
      </ul>
```

with:

```svelte
      <ul class="trace">
        {#each trace as row}
          {@const off = row.offset}
          {#if off !== null}
            <li><button class="link" onclick={() => selection.jumpTo(off)}>{row.label}</button></li>
          {:else}
            <li class="muted">{row.label}</li>
          {/if}
        {/each}
      </ul>
```

The `{@const off}` is what lets the click handler take `off` as a `number`: TypeScript does not carry the `row.offset !== null` narrowing into a closure over a property. The rendered facts are unchanged: `Cluster` for the `<dt>` (`cap("cluster")`), and `2 · FAT: end of chain` for the `<dd>`, because `Fat16Adapter.describeUnit(2)` is `"FAT: end of chain"` (spec section 6).

Run: `pnpm exec svelte-check --tsconfig ./tsconfig.json --output machine 2>&1 | grep -E "^[0-9]+ (ERROR|WARNING) " | grep '"src/components/Inspector.svelte"'`

Expected: no output.

- [ ] **Step 11: Tree rows name the first unit through the adapter**

`web/ui/src/components/TreeNodeView.svelte`. Replace (lines 2-4):

```ts
  import type { TreeNode } from "../core/tree";
  import { clusterByteRange } from "../core/attribution";
  import { selection } from "../state/selection.svelte";
```

with:

```ts
  import type { TreeNode } from "../core/tree";
  import { selection } from "../state/selection.svelte";
```

Replace (line 15):

```ts
    if (node.firstCluster >= 2) selection.jumpTo(clusterByteRange(volume.geometry, node.firstCluster).start);
```

with:

```ts
    if (node.firstUnit !== null) selection.jumpTo(volume.adapter.unitByteRange(node.firstUnit).start);
```

Replace (line 28):

```svelte
      <span class="meta mono muted">{node.size.toLocaleString()} B · {node.firstCluster >= 2 ? `first cluster ${node.firstCluster}` : "no data"}</span>
```

with:

```svelte
      <span class="meta mono muted">{node.size.toLocaleString()} B · {node.firstUnit !== null ? `first ${volume.adapter.unit.singular} ${node.firstUnit}` : "no data"}</span>
```

`pick` is an event handler, not a derived, so it needs no `epoch` read; the `unit.singular` read in the template is a plain field of the `$state.raw` adapter.

Run: `pnpm exec svelte-check --tsconfig ./tsconfig.json --output machine 2>&1 | grep -E "^[0-9]+ (ERROR|WARNING) " | grep '"src/components/TreeNodeView.svelte"'`

Expected: no output.

- [ ] **Step 12: The ribbon reads sector counts, owners, and free space through the store and adapter**

`web/ui/src/components/Ribbon.svelte`. Replace (line 40):

```ts
  const sectorsPerCol = $derived(Math.max(1, Math.ceil(volume.geometry.totalSectors / cols)));
```

with:

```ts
  const sectorsPerCol = $derived(Math.max(1, Math.ceil(volume.totalSectors / cols)));
```

Replace (lines 106-108):

```ts
    const key = `${volume.epoch}:${cols}:${sectorsPerCol}`;
    if (key === colorCacheKey) return colorCache;
    const total = volume.geometry.totalSectors;
```

with:

```ts
    const key = `${volume.epoch}:${cols}:${sectorsPerCol}`;
    if (key === colorCacheKey) return colorCache;
    const total = volume.totalSectors;
```

Replace (lines 183-184):

```ts
  function jumpToColumn(col: number) {
    const total = volume.geometry.totalSectors;
```

with:

```ts
  function jumpToColumn(col: number) {
    const total = volume.totalSectors;
```

Replace (lines 226-227):

```ts
    if (hoverCol === null) return "";
    const total = volume.geometry.totalSectors;
```

with:

```ts
    if (hoverCol === null) return "";
    const total = volume.totalSectors;
```

Replace (line 255):

```ts
    walk(buildTree(volume.vol, volume.owners));
```

with:

```ts
    walk(buildTree(volume.vol, volume.adapter.owners));
```

Replace (lines 259-267):

```ts
  const freeLabel = $derived.by(() => {
    volume.epoch;
    const { clusterCount, bytesPerSector, sectorsPerCluster } = volume.geometry;
    const clusterSize = bytesPerSector * sectorsPerCluster;
    const owner = volume.attribution.ownerByCluster;
    let free = 0;
    for (let c = 2; c <= clusterCount + 1; c++) if (owner[c] < 0) free++;
    return `${((free * clusterSize) / (1024 * 1024)).toFixed(1)} MB free`;
  });
```

with:

```ts
  const freeLabel = $derived.by(() => {
    volume.epoch;
    const { unit, unitCount, unitSize } = volume.adapter;
    const owner = volume.attribution.ownerByUnit;
    let free = 0;
    for (let u = unit.first; u < unit.first + unitCount; u++) if (owner[u] < 0) free++;
    return `${((free * unitSize) / (1024 * 1024)).toFixed(1)} MB free`;
  });
```

The loop visits `unitCount` units starting at `unit.first`, which for FAT is `2 .. clusterCount + 1` inclusive, the same cells as before. `files` and `freeLabel` already read `volume.epoch` (lines 247, 260), which is what the reactivity rule asks of them now that `owners`, `unitCount`, and `unitSize` are plain adapter fields.

Run: `pnpm exec svelte-check --tsconfig ./tsconfig.json --output machine 2>&1 | grep -E "^[0-9]+ (ERROR|WARNING) " | grep '"src/components/Ribbon.svelte"'`

Expected: no output.

- [ ] **Step 13: StatusLine, DirTree, and the Lesson card name the family through the adapter**

`web/ui/src/components/StatusLine.svelte`. Replace (lines 4-22):

```ts
  // Sentence-case, verb-first-adjacent copy per the design spec ("Disk full. Free
  // space or use a smaller file."); falls back to the raw wasm message for codes not
  // listed here (e.g. an unexpected InvalidGeometry from a hand-typed format option).
  const FRIENDLY: Record<string, string> = {
    DiskFull: "Disk full. Free space or use a smaller file.",
    DirectoryFull: "This folder is full. Remove an entry or use a different folder.",
    AlreadyExists: "Something already exists at that path.",
    NotFound: "Nothing exists at that path.",
    InvalidPath: "That path isn't valid.",
    InvalidName: "That name isn't valid for FAT16.",
    NotADirectory: "That path is a file, not a folder.",
    IsADirectory: "That path is a folder, not a file.",
    DirectoryNotEmpty: "That folder still has files in it.",
    FileTooLarge: "That file is too large for this disk.",
    InvalidGeometry: "Those format options don't add up to a valid disk.",
    CorruptImage: "That image doesn't look like a valid FAT16 volume.",
    OutOfBounds: "That range runs past the end of the disk.",
    Unsupported: "That isn't supported yet.",
  };
```

with:

```ts
  // Sentence-case, verb-first-adjacent copy per the design spec ("Disk full. Free
  // space or use a smaller file."); falls back to the raw wasm message for codes not
  // listed here (e.g. an unexpected InvalidGeometry from a hand-typed format option).
  // Two entries name the mounted family (`volume.adapter.name`, "FAT16" today), so the
  // table is derived and follows a format or load that binds another adapter.
  const FRIENDLY: Record<string, string> = $derived({
    DiskFull: "Disk full. Free space or use a smaller file.",
    DirectoryFull: "This folder is full. Remove an entry or use a different folder.",
    AlreadyExists: "Something already exists at that path.",
    NotFound: "Nothing exists at that path.",
    InvalidPath: "That path isn't valid.",
    InvalidName: `That name isn't valid for ${volume.adapter.name}.`,
    NotADirectory: "That path is a file, not a folder.",
    IsADirectory: "That path is a folder, not a file.",
    DirectoryNotEmpty: "That folder still has files in it.",
    FileTooLarge: "That file is too large for this disk.",
    InvalidGeometry: "Those format options don't add up to a valid disk.",
    CorruptImage: `That image doesn't look like a valid ${volume.adapter.name} volume.`,
    OutOfBounds: "That range runs past the end of the disk.",
    Unsupported: "That isn't supported yet.",
  });
```

With `volume.adapter.name === "FAT16"` both strings are the ones on the left.

`web/ui/src/components/DirTree.svelte`. Replace (line 7):

```ts
  const tree = $derived((volume.epoch, buildTree(volume.vol, volume.owners)));
```

with:

```ts
  const tree = $derived((volume.epoch, buildTree(volume.vol, volume.adapter.owners)));
```

Replace (lines 12-18):

```svelte
  {#if volume.corruption}<p class="muted stale-note">Boot sector does not parse; the tree is unavailable until a raw write repairs it.</p>{/if}
  {#if !volume.atLatest}<p class="muted stale-note">Shows the latest state, not the step you are viewing.</p>{/if}
  <label class="remnants">
    <input type="checkbox" bind:checked={selection.showRemnants} />
    Show remnants
    {#if !volume.atLatest}<span class="muted">(latest state only)</span>{/if}
  </label>
```

with:

```svelte
  {#if volume.corruption}<p class="muted stale-note">{volume.adapter.corruptNote}</p>{/if}
  {#if !volume.atLatest}<p class="muted stale-note">Shows the latest state, not the step you are viewing.</p>{/if}
  <!-- Only a family with "deleted things still on disk" has `remnants`; without it there is
       nothing for the toggle to show. -->
  {#if volume.adapter.remnants}
    <label class="remnants">
      <input type="checkbox" bind:checked={selection.showRemnants} />
      Show remnants
      {#if !volume.atLatest}<span class="muted">(latest state only)</span>{/if}
    </label>
  {/if}
```

`volume.adapter.remnants` is the optional method itself (present on `Fat16Adapter`), so the toggle renders for FAT exactly as before; `corruptNote` is `Fat16Adapter`'s `CORRUPT_NOTE`, the sentence that was inline.

`web/ui/src/components/LessonPanel.svelte`. Replace (lines 19-21):

```ts
  const lookAt = $derived(
    describeFocus(scenarios.focus, volume.geometry, (sector) => attrAtSector(volume.attribution, sector).regionName),
  );
```

with:

```ts
  // `describeFocus` reads the adapter's unit space (plain caches), so this derived reads
  // `volume.epoch` first (the rule in fs/adapter.ts).
  const lookAt = $derived.by(() => {
    volume.epoch;
    return describeFocus(scenarios.focus, volume.adapter, (sector) => attrAtSector(volume.attribution, sector).regionName);
  });
```

Run: `pnpm exec svelte-check --tsconfig ./tsconfig.json --output machine 2>&1 | grep -E "^[0-9]+ (ERROR|WARNING) " | grep '"src/components/StatusLine.svelte"\|"src/components/DirTree.svelte"\|"src/components/LessonPanel.svelte"'`

Expected: no output.

- [ ] **Step 14: Re-point whatever still imports the core shims, remove the shims, then run the whole suite**

Task 2 moved `direntry.ts`, `fatchain.ts`, and `remnants.ts` under `src/fs/fat16/` and left one-line shims (`export * from "../fs/fat16/<name>"`) at the old paths so its importers compiled unchanged. This task is where the shims end. Each is a relative import of `fs/fat16` from `src/core/`, which is exactly what Task 7's `tests/adapterBoundary.test.ts` rejects (spec section 6: only `src/fs/index.ts`, `src/fs/panels.ts`, and `src/scenarios/**` may import from `fs/fat16`), so a shim carried past this task's commit is a Task 7 gate failure, not something to note in a report and move on from.

Run: `grep -rn "core/direntry\|core/fatchain\|core/remnants" src tests`

Expected: at most these four lines, and nothing else:

```
src/scenarios/directory.ts:1:import { findEntrySlots } from "../core/direntry";
src/scenarios/fundamentals.ts:1:import { findEntrySlots } from "../core/direntry";
src/scenarios/longName.ts:1:import { findEntrySlots } from "../core/direntry";
src/scenarios/smallFile.ts:1:import { findEntrySlots } from "../core/direntry";
```

(Task 2 re-pointed `tests/{direntry,remnants,fatchain,corruption}.test.ts`, Task 3 `tests/integration.test.ts`, and Task 4 `src/state/layers.svelte.ts`, `src/shell/commands.ts`, and `tests/shell/read-commands.test.ts`; the FAT map's `clusterState` import moved in step 3. Task 4's Step 21 leaves the four scenario files on `core/direntry`; Task 6 rewrites them whole and already expects a re-pointed `direntry` import from this task, so nothing done here is undone later.)

Every line the grep prints is an importer to re-point now, before the shims go. The mapping is one line per file, keeping the relative prefix: `core/direntry` -> `fs/fat16/direntry`, `core/fatchain` -> `fs/fat16/fatchain`, `core/remnants` -> `fs/fat16/remnants`. For the four expected lines that is line 1 of each of `src/scenarios/{directory,fundamentals,longName,smallFile}.ts`:

```ts
import { findEntrySlots } from "../fs/fat16/direntry";
```

A `tests/**` line becomes `../src/fs/fat16/<name>` (or `../../src/fs/fat16/<name>` under `tests/shell/`); the boundary test scans `src/` only, and `src/scenarios/**` is on its allow list, so both re-points are final. A line under `src/state`, `src/shell`, or `src/components` is a miss from Task 4 or from steps 3-13 above: there a `fs/fat16` import is itself what the boundary test rejects, so the file gets the `volume.adapter`/`host.adapter` call its task names for that site instead of a re-point. Whichever case, do not leave the shim in place for it. Re-run the grep until it prints nothing.

Then remove the shims, unconditionally. Task 4's Step 21 may already have removed `core/remnants.ts`, and a plain `git rm` dies on a missing path without removing the others, hence the flag:

Run: `git rm --ignore-unmatch src/core/direntry.ts src/core/fatchain.ts src/core/remnants.ts`

Expected: an `rm 'src/core/<name>.ts'` line for each shim that was still there (three, or two if Task 4 removed `core/remnants.ts`), exit 0.

Run: `ls src/core/direntry.ts src/core/fatchain.ts src/core/remnants.ts`

Expected: `ls: src/core/<name>.ts: No such file or directory` for all three, and nothing else.

Run: `pnpm test`

Expected: every test file passes with the file and test counts Task 4's final run step reported; this task adds no test and removes none.

- [ ] **Step 15: Full gates: nothing FAT-only left in the components, no core shim left, svelte-check clean, vite builds**

Run:

```
grep -rn "volume\.geometry\|volume\.owners\|volume\.fat\b\|clusterByteRange\|annotateSectorWith\|ownerByCluster\|colorByCluster\|FatEntry\|FormatOptions\|fs/fat16" src/App.svelte src/components
ls src/core/direntry.ts src/core/fatchain.ts src/core/remnants.ts
pnpm test && pnpm build
```

Expected: the `grep` prints nothing (the components reach FAT facts only through `volume.adapter` and `fs/panels`); the `ls` prints `ls: src/core/<name>.ts: No such file or directory` for all three and nothing else (no shim reaches the commit in step 17; a path that lists is a step 14 miss, go back and finish it); vitest passes as in step 14; `svelte-check` reports 0 errors and 0 warnings (in a non-TTY run its summary line is `COMPLETED <n> FILES 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS`); `vite build` writes `dist/`.

- [ ] **Step 16: Browser pass: add, overwrite, delete, format twice, corrupt and repair**

Start the dev server with the `ui` launch configuration (`.claude/launch.json`: `pnpm --dir web/ui dev --port 5174 --strictPort`; `preview_start` with name `ui`) and open `http://localhost:5174/`. Read texts with `find`, `read_page`, or `get_page_text`; click with `computer`; set fields and selects with `form_input`; use `javascript_tool` only for the read-only queries named below. Every item is pass/fail:

1. Load. The h1 reads `FAT explorer` (Task 7 renames it). Files shows one row, `/`, with the meta `0 B · no data`. Under it the panel headed `FAT map · 8,167 clusters` (rendered by `PANELS.fat16.map`). The ribbon legend ends with `16.0 MB free`. The Timeline reads `No operations yet`; the Step strip reads `Run an action to see what it changes.`. Click the `Format` summary in Actions: it shows the `Size` and `Sectors per cluster` selects, the line `8,167 clusters`, the `Volume label` field, and the `Format disk` button (rendered by `PANELS.fat16.format`). `read_console_messages` with `onlyErrors`: nothing.
2. Add. Set `Path` to `/HELLO.TXT` (leave `Content` at `Hello from the browser`, 22 bytes) and click `Add file`. Step strip: `1 of 1` and `create_file /HELLO.TXT`. Files: a row `HELLO.TXT` with the meta `22 B · first cluster 2`. Ribbon legend: a swatch `HELLO.TXT` and `15.9 MB free`. Inspector, `Selected file`: the path `/HELLO.TXT` and three links, `Directory entry · offset 0x8200`, `FAT chain · 1 clusters starting at 2 · FAT entry at 0x204`, `Data · cluster 2 at 0xc200`. Click the Files row `HELLO.TXT`: the dump scrolls to the row whose offset column reads `0000c200`, labelled `cluster 2 · /HELLO.TXT`; `javascript_tool` with `document.querySelector(".row.unit-start .lbl")?.textContent` returns `cluster 2 · /HELLO.TXT` (the renamed class carries the row's darker top border). Click the first hex cell of that row (`48`): the Inspector facts read `Offset` `0xc200 · 49,664`, `Sector` `97 · data`, `Cluster` `2 · FAT: end of chain`, and `Owner` `/HELLO.TXT` as a link.
3. Overwrite from the form. Set `Content` to `Hello again from the browser` (28 bytes) and click `Overwrite`. Step strip: `2 of 2` and `write_file /HELLO.TXT`. Files meta: `28 B · first cluster 2`. The `0000c200` row shows amber cells (class `is-diff`) and its ASCII gutter begins `Hello again from`. The Inspector trace still says `1 clusters starting at 2`.
4. Overwrite from the shell so the chain grows. Click `Terminal`, type `dd if=/dev/zero of=/mnt/HELLO.TXT bs=512 count=5` and Enter: the terminal prints `FAT has no partial writes; the whole file was rewritten` (the `dd` clause, `host.adapter.notes.partialWrite` since Task 4, printed because the file exists), then `5+0 records in`, `5+0 records out`, `2560 bytes copied`. Step strip: `write_file /HELLO.TXT`. Files meta: `2,560 B · first cluster 2`. Inspector trace: `FAT chain · 2 clusters starting at 2 · FAT entry at 0x204`. Screenshot the FAT map: cells 2 and 3 (top left) are in the file's hue with a chain line joining them.
5. Delete. Click `Delete` (Path still `/HELLO.TXT`). Step strip: `delete_file /HELLO.TXT` with the buttons `Sector 1`, `Sector 33`, `Sector 65`. Files: the root row only. Ribbon legend: no `HELLO.TXT` swatch, `16.0 MB free`. The Inspector's `Selected file` section is gone. Screenshot the FAT map: cells 2 and 3 are back to the hairline colour with no chain line. Click `Sector 65` in the Step strip: the dump shows the `00008200` row whose first cell reads `e5`. Tick `Show remnants`: that row's cells are hatched (class `is-remnant`); untick it.
6. Format from the form. Open `Format`, set `Size` to `4 MB` and `Sectors per cluster` to `1`: the line reads `8,095 clusters`. Click `Format disk`: the FAT map heading reads `FAT map · 8,095 clusters`, the Timeline `No operations yet`, the Step strip `Run an action to see what it changes.`, the ribbon `4.0 MB free`, Files the root row only. Set `Size` to `64 MB` (spc still 1): the line reads `130,023 clusters · too many for FAT16` and `Format disk` is disabled. Set `Size` to `4 MB` and `Sectors per cluster` to `8`: `1,018 clusters · too few for FAT16`, disabled. Set `16 MB` and `4`: `8,167 clusters`, enabled.
7. Format from `mkfs`. In the terminal type `mkfs` and Enter: it prints `formatted /dev/hda as FAT16; the timeline was cleared`; the FAT map heading reads `FAT map · 8,167 clusters`; the ribbon `16.0 MB free`; the prompt reads `/mnt ❯`.
8. Corrupt and repair. Click `Add file` (Path `/HELLO.TXT`) so Files has a row. In the terminal: `dd if=/dev/hda bs=512 count=1 of=/dev/hda seek=32767` (stash sector 0 in the last sector): Step strip `write_raw 0xfffe00 +512`. Then `dd if=/dev/zero of=/dev/hda count=1`: Step strip `write_raw 0x0 +512`; Files shows the note `Boot sector does not parse; the tree is unavailable until a raw write repairs it.` above the root row alone; the top bar reads `Volume not mounted: boot sector no longer parses after a raw write: boot sector signature is not 55 AA`; the FAT map is still headed `FAT map · 8,167 clusters` (the adapter refreshed from the last good geometry); `ls /mnt` prints, in red, `/mnt: boot sector no longer parses after a raw write: boot sector signature is not 55 AA` with a help line containing `dd --of=/dev/hda`. Repair: `dd if=/dev/hda bs=512 skip=32767 count=1 of=/dev/hda`: Step strip `write_raw 0x0 +512`; the note and the `Volume not mounted` text are gone; Files lists `HELLO.TXT` with `22 B · first cluster 2` again; `ls /mnt` lists `HELLO.TXT`.
9. `read_console_messages` with `onlyErrors`: still nothing.

Record any failing item and fix it before this task is considered done. Items 2, 3, and 5 are the proof of the reactivity rule for the dump, the map, the tree, the ribbon, and the Inspector trace (spec section 7, ruling 1); item 8 is the corruption path through `refresh()` (ruling 6).

- [ ] **Step 17: Commit**

```
git add web/ui/src/fs/panels.ts web/ui/src/fs/fat16/FatMap.svelte web/ui/src/fs/fat16/FormatForm.svelte web/ui/src/App.svelte web/ui/src/app.css web/ui/src/components/ActionsPanel.svelte web/ui/src/components/DirTree.svelte web/ui/src/components/HexRow.svelte web/ui/src/components/HexView.svelte web/ui/src/components/Inspector.svelte web/ui/src/components/LessonPanel.svelte web/ui/src/components/Ribbon.svelte web/ui/src/components/StatusLine.svelte web/ui/src/components/TreeNodeView.svelte
git commit -m "refactor(ui): components read the volume through the adapter; the FAT map and Format form move to fs/fat16"
```

Add to the `git add` line every importer step 14 re-pointed: for the expected case, `web/ui/src/scenarios/directory.ts web/ui/src/scenarios/fundamentals.ts web/ui/src/scenarios/longName.ts web/ui/src/scenarios/smallFile.ts`. (`git mv` in step 3 and `git rm` in step 14 staged the removals of `web/ui/src/components/FatMap.svelte` and the core shims; `git status --short` before the commit shows nothing unstaged and no `src/core/{direntry,fatchain,remnants}.ts` other than as a staged `D`.)
### Task 6: web/ui: scenarios declare their family

Spec: `docs/superpowers/specs/2026-09-23-fs-adapter-design.md`, section 3's "Scenarios" paragraph and code block (`Scenario.family`, `Step.action(v, fs)`, `focus: (fs) => StepFocus`; `StepFocus.unit` was renamed in Task 4 Step 3), and the `scenarios` and `fundamentals` entries in section 6 ("Existing tests that change").

**Files:**
- Modify: `web/ui/src/state/scenarios.svelte.ts` (whole file; today lines 1-5 imports, 7-29 the three types, 45-52 `start`, 93-97 `applyStep`, 101-111 `applyFocus`)
- Modify: `web/ui/src/scenarios/fundamentals.ts` (whole file; today lines 1-3 imports, 26-32 the `fatOffset`/`entryOffset` helpers, 34-38 the scenario header, focuses at 53, 58, 63, 68, 73, 84, 89, 94)
- Modify: `web/ui/src/scenarios/shell.ts` (whole file; today lines 14-18 header, 33 and 62 the function focuses)
- Modify: `web/ui/src/scenarios/smallFile.ts` (whole file; today lines 1-2 imports, 4-8 header, 18-21 the `findEntrySlots` focus, 31 `cluster: 2`)
- Modify: `web/ui/src/scenarios/longName.ts` (whole file; today lines 1-2, 6-10, 20-23, 28-31)
- Modify: `web/ui/src/scenarios/directory.ts` (whole file; today lines 1-2, 7-11, 21-24 the owner focus, 30-33 the slots focus)
- Modify: `web/ui/src/scenarios/overwriteGrows.ts` (whole file; today lines 1-10 the `bytesPerCluster` helper, 12-16 header, actions at 20, 26, 43)
- Modify: `web/ui/src/scenarios/fillDisk.ts` (whole file; today lines 1, 3-7, 11 `format`, 17 and 23 the geometry-sized actions)
- Modify: `web/ui/src/scenarios/deleteRemnants.ts` (whole file; today lines 6-10, 21, 26)
- Modify: `web/ui/src/scenarios/format.ts` (whole file; today lines 3-7, 21, 26)
- Test: `web/ui/tests/scenarios.test.ts` (whole file; today lines 2 the `Volume` import, 67-85 the end-to-end loop, 99-105 the shell `run` helper)
- Test: `web/ui/tests/fundamentals.test.ts` (whole file; today lines 2-4 imports, 18-32 `runThrough`/`focusOf`, 40 the geometry read)
- Test (only if Step 10's grep prints it): `web/ui/tests/corruption.test.ts` (HEAD lines 4-5, the `../src/core/direntry` and `../src/core/remnants` import paths)
- Delete: `web/ui/src/core/direntry.ts`, `web/ui/src/core/fatchain.ts`, `web/ui/src/core/remnants.ts` (Task 2's one-line re-export shims, whichever of them still exist; Step 10, once Steps 5 and 7 have removed their last `core/direntry` importers)
- Read, unchanged: `web/ui/src/scenarios/index.ts` (`all: Scenario[]` keeps its nine entries and order), `web/ui/tests/lesson.test.ts` (Task 3 already switched its `{ cluster: N }` inputs to `{ unit: N }`; nothing is left for this task), `web/ui/src/components/ScenarioPanel.svelte` and `LessonPanel.svelte` (they call `scenarios.start/next/prev/stop` and read `scenarios.focus`, none of which change shape).

Every source file below is written whole. Task 4 Steps 3-4 already renamed `StepFocus.cluster` to `unit`, switched the runner to `volume.format(DEFAULT_FAMILY, ...)`, and renamed the focus key in `deleteRemnants`, `directory`, `fundamentals`, `shell`, and `smallFile`; the whole-file writes below supersede those edits. Nothing before this task re-pointed the four scenario files' `../core/direntry` imports (Task 4 Step 4 touched focus keys only), which is why Task 2's `core/` shims are still in place when this task starts and why Step 10 deletes them. Every step title, step text, summary, and scenario id is copied verbatim from today's files: the copy teaches FAT16 and stays.

**Interfaces:**
- Consumes (Task 2, `web/ui/src/fs/adapter.ts`): `type FsFamilyId = "fat16"`; `interface FsAdapter` with `readonly id: FsFamilyId`, `refresh(): void`, `ownerOf(path: string): UnitOwner | undefined` (`UnitOwner.firstUnit: number`), `entrySlots(path: string): Interval | null`, `dataStart(path: string): number | null`, `regionStart(kind: RegionKind): number | undefined`, `readonly unitSize: number`, `readonly unitCount: number`, `unitByteRange(unit: number): Interval`; `interface FsFamily<O = unknown> { format(options?: O): Volume; ... }`.
- Consumes (Task 2, `web/ui/src/fs/index.ts`): `FAMILIES: Record<FsFamilyId, FsFamily>`, `adapterFor(vol: Volume): FsAdapter`.
- Consumes (Task 2, `web/ui/src/fs/fat16/index.ts`): `asFat16(fs: FsAdapter): Fat16Adapter` (throws unless `fs.id === "fat16"`), `type Fat16FormatOptions = FormatOptions`; and on `Fat16Adapter` (`fs/fat16/adapter.ts`) `fatEntryOffset(cluster: number, copy = 0): number` = `(reservedSectors + copy * sectorsPerFat) * bytesPerSector + cluster * 2`, the formula `commands.ts` `stat` (line 284 today) and `Inspector.svelte` (line 26 today) use for copy 0.
- Consumes (Task 4, `web/ui/src/state/volume.svelte.ts`): `volume.adapter: FsAdapter` (`$state.raw`, re-bound by `adopt`, `refresh()`ed after every `run`), `volume.sectorSize: number`, `volume.format(family?: FsFamilyId, options?: unknown): void`, `volume.run(fn: (v: Volume) => OpRecord): OpRecord | null`, `volume.seek(step: number)`, `volume.cursor: number`.
- Consumes (Task 3, `web/ui/src/core/lesson.ts`): `describeFocus(focus: StepFocus | null | undefined, space: UnitSpace, regionNameAt)` reading `focus.unit`; (Task 5, `LessonPanel.svelte`) `describeFocus(scenarios.focus, volume.adapter, ...)`.
- Produces (`web/ui/src/state/scenarios.svelte.ts`):
  - `export interface StepFocus { offset?: number; sector?: number; unit?: number; path?: string | null; showRemnants?: boolean; strings?: boolean }`
  - `export interface Step<O = unknown> { title: string; text: string; action?: (v: Volume, fs: FsAdapter) => OpRecord; format?: O; focus?: StepFocus | ((fs: FsAdapter) => StepFocus) }`
  - `export interface Scenario<O = unknown> { id: string; title: string; summary: string; family: FsFamilyId; steps: Step<O>[] }`
  - `ScenarioRunner` keeps its public surface: `current`, `index`, `step`, `focus`, `start(s: Scenario)`, `next()`, `prev()`, `stop()`; `export const scenarios`.
- Produces (`web/ui/src/scenarios/*.ts`): nine `Scenario` values, each with `family: "fat16"`; `all` unchanged (fundamentals first, shell last); `fillDisk` is typed `Scenario<Fat16FormatOptions>`. No scenario file calls `geometry(`, `clusterOwners(`, `fatEntries(`, `rawDirEntries(`, `bootSector(`, `clusterChain(`, `annotateSectorWith(` or `formatFat16(`, and none imports a FAT type from `lib/wasm`; `fundamentals.ts` and `fillDisk.ts` import from `../fs/fat16`, which Task 7's boundary test allows for `src/scenarios/**`; after Step 10 no file under `src/core/` imports from `fs/fat16` (Task 2's three re-export shims were the only ones, and the boundary test does not allow `core/`).

Gate commands (run from `web/ui`):

```
pnpm vitest run tests/scenarios.test.ts tests/fundamentals.test.ts
pnpm test
pnpm build
```

---

- [ ] **Step 1: Rewrite `tests/scenarios.test.ts` for the family and the adapter**

Replace the whole of `web/ui/tests/scenarios.test.ts` with:

```ts
import { describe, expect, it } from "vitest";
import { adapterFor, FAMILIES } from "../src/fs";
import { ScenarioCursor } from "../src/core/scenarioCursor";
import { all } from "../src/scenarios";
import { scenario as shell } from "../src/scenarios/shell";

// Steps titled "Expect: ..." are the scenario's deliberate failure demonstrations; each
// one names the error code its action must throw.
const EXPECTED_ERROR_CODE: Record<string, string> = {
  "Expect: disk full": "DiskFull",
  "Expect: not empty": "DirectoryNotEmpty",
};

describe("scenario scripts", () => {
  it("every scenario has at least 3 steps, each with non-empty text", () => {
    for (const s of all) {
      expect(s.steps.length).toBeGreaterThanOrEqual(3);
      for (const step of s.steps) expect(step.text.trim().length).toBeGreaterThan(0);
    }
  });

  // browser-terminal 0.3.0 delivered redirection and `key=value` barewords, so step text
  // teaching the 0.2.0 workarounds is now wrong. A learner reads this copy and types what
  // it says; nothing else in the suite would catch a stale sentence.
  it("no step text still teaches a workaround 0.3.0 removed", () => {
    const stale = ["no `>`", "There is no `>`", "must be quoted"];
    for (const s of all) {
      for (const step of s.steps) {
        for (const phrase of stale) {
          expect(step.text, `${s.id} — "${step.title}"`).not.toContain(phrase);
        }
      }
    }
  });

  // `start()` formats the scenario's family before its first step, so an id the registry
  // does not know would only surface as a throw inside the runner. Pin it here instead.
  it("every scenario names a registered family", () => {
    for (const s of all) {
      expect(FAMILIES[s.family], `${s.id} names family "${s.family}"`).toBeDefined();
      expect(FAMILIES[s.family].id).toBe(s.family);
    }
  });

  // The runner itself needs runes, so the step bookkeeping it drives lives in a plain
  // class (src/core/scenarioCursor.ts) that can be exercised here.
  describe("step cursor", () => {
    it("runs each step once; going back and forward again only seeks", () => {
      const c = new ScenarioCursor(3);
      expect(c.canRunNext()).toBe(true);
      expect(c.next()).toEqual({ run: true });   // step 0
      c.advance(0);                              // volume.cursor after step 0
      expect(c.next()).toEqual({ run: true });   // step 1
      c.advance(1);                              // volume.cursor after step 1
      expect(c.index).toBe(1);

      expect(c.back()).toBe(0);                  // prev -> step 0's disk
      expect(c.index).toBe(0);
      expect(c.canRunNext()).toBe(false);
      expect(c.next()).toEqual({ seekTo: 1 });   // next -> step 1's disk, not a third run
      expect(c.index).toBe(1);
    });

    it("clamps at both ends", () => {
      const c = new ScenarioCursor(1);
      expect(c.back()).toBeNull();
      expect(c.next()).toEqual({ run: true });
      c.advance(7);
      expect(c.next()).toBeNull();               // past the last step
      expect(c.index).toBe(0);
      expect(c.back()).toBeNull();               // already at the first step
      expect(c.index).toBe(0);
    });
  });

  for (const s of all) {
    it(`runs "${s.title}" end to end against a fresh Volume`, () => {
      // The runner's shape: format the scenario's family, bind an adapter to the result, and
      // hand every action and function focus that adapter. A format is a new Volume (bind
      // again); an action is the same Volume with new contents (refresh).
      let vol = FAMILIES[s.family].format();
      let fs = adapterFor(vol);
      for (const step of s.steps) {
        const run = () => {
          if (step.format) {
            vol = FAMILIES[s.family].format(step.format);
            fs = adapterFor(vol);
          }
          if (step.action) {
            step.action(vol, fs);
            fs.refresh();
          }
          if (typeof step.focus === "function") step.focus(fs);
        };
        const expectedCode = EXPECTED_ERROR_CODE[step.title];
        if (step.title.startsWith("Expect:")) {
          expect(expectedCode, `unrecognized "Expect:" step title: ${step.title}`).toBeDefined();
          expect(run).toThrow(expect.objectContaining({ code: expectedCode }));
        } else {
          expect(run).not.toThrow();
        }
      }
    });
  }

  // The shell scenario's step text shows a terminal command and its action performs the
  // equivalent Volume call. Pin the outcomes the commands would leave behind.
  describe("work from the shell", () => {
    it("is registered last, after the eight explorer-driven scenarios", () => {
      expect(all.length).toBe(9);
      expect(all[all.length - 1]).toBe(shell);
      expect(shell.id).toBe("shell");
      expect(shell.steps.length).toBe(8);
      // Every step's text names the command it stands for.
      for (const step of shell.steps) expect(step.text).toMatch(/`[a-z]+[^`]*`/);
    });

    it("leaves the volume the way the equivalent commands would", () => {
      const vol = FAMILIES[shell.family].format();
      const fs = adapterFor(vol);
      const run = (i: number) => {
        const step = shell.steps[i];
        expect(step.action, `step ${i} "${step.title}" has no action`).toBeDefined();
        const rec = step.action!(vol, fs);
        fs.refresh();
        return rec;
      };
      const text = (b: Uint8Array) => new TextDecoder().decode(b);

      // echo 'Hello from the shell' | write /mnt/HELLO.TXT
      expect(run(1).op).toBe("create_file /HELLO.TXT");
      expect(text(vol.readFile("/HELLO.TXT"))).toBe("Hello from the shell");
      expect(vol.clusterOwners().find((o) => o.path === "/HELLO.TXT")?.firstCluster).toBe(2);

      // mkdir /mnt/DOCS
      expect(run(3).op).toBe("create_dir /DOCS");
      expect(vol.stat("/DOCS").isDir).toBe(true);

      // cp /mnt/HELLO.TXT /mnt/DOCS/COPY.TXT
      run(4);
      expect(vol.readFile("/DOCS/COPY.TXT")).toEqual(vol.readFile("/HELLO.TXT"));
      expect(vol.clusterOwners().find((o) => o.path === "/DOCS/COPY.TXT")?.firstCluster).toBe(4);

      // echo 'SHELLDISK  ' | dd --of=/dev/hda --bs=1 --seek=43
      // The BootSector DTO trims trailing spaces from the 11-byte label (dto.rs `text`).
      const patch = run(6);
      expect(patch.op).toBe("write_raw 0x2b +11");
      expect(patch.changes).toHaveLength(1);
      expect(patch.changes[0].offset).toBe(43);
      expect(patch.changes[0].after).toEqual(new TextEncoder().encode("SHELLDISK  "));
      expect(vol.bootSector().volumeLabel).toBe("SHELLDISK");
      expect(vol.corruption()).toBeNull();

      // rm /mnt/HELLO.TXT: the entry is marked deleted, DOCS (slot 1) is all that lists.
      expect(run(7).op).toBe("delete_file /HELLO.TXT");
      expect(vol.listDir("/").map((e) => e.name)).toEqual(["DOCS"]);
      expect(text(vol.readFile("/DOCS/COPY.TXT"))).toBe("Hello from the shell");
    });
  });
});
```

What changed and why: the `Volume` import is gone (construction goes through `FAMILIES[s.family].format(...)`, the registry Task 2 produced); the end-to-end loop and the shell `run` helper pass an adapter to every action and function focus, re-binding after a format and refreshing after an action, which is exactly what `VolumeStore.adopt` and `VolumeStore.run` do; the "every scenario names a registered family" case is new. The `vol.clusterOwners()`, `vol.bootSector()` and `vol.corruption()` reads stay: they are wasm calls in a test, which the boundary rule does not scope, and every asserted value is unchanged.

- [ ] **Step 2: Rewrite `tests/fundamentals.test.ts` in the same shape**

Replace the whole of `web/ui/tests/fundamentals.test.ts` with:

```ts
import { describe, expect, it } from "vitest";
import type { Volume } from "../src/lib/wasm";
import { adapterFor, FAMILIES } from "../src/fs";
import { all } from "../src/scenarios";
import { BIGGER, BIGGER_BYTES, biggerText, HELLO, HELLO_TEXT, scenario } from "../src/scenarios/fundamentals";

// "The fundamentals" quotes the default disk's numbers in its copy (sector 97, 8,167
// clusters, 4,796 bytes, ...). These tests pin every quoted number to the volume the
// scenario actually runs on, so a change to the default format cannot leave the lesson
// teaching stale arithmetic.

const n = (v: number) => v.toLocaleString("en-US");
const stepText = (title: string) => {
  const s = scenario.steps.find((st) => st.title === title);
  if (!s) throw new Error(`no step titled "${title}"`);
  return s.text;
};

/** Run the scenario the way the runner does: a format of its family, then each action in
 *  order with the adapter refreshed after it, stopping after the step with the given title.
 *  Returns the volume in that state. */
function runThrough(title: string): Volume {
  const vol = FAMILIES[scenario.family].format();
  const fs = adapterFor(vol);
  for (const step of scenario.steps) {
    if (step.action) {
      step.action(vol, fs);
      fs.refresh();
    }
    if (step.title === title) break;
  }
  return vol;
}

/** A step's focus as the runner resolves it: the function form gets an adapter bound to the
 *  volume in its current state. */
const focusOf = (title: string, vol: Volume) => {
  const s = scenario.steps.find((st) => st.title === title)!;
  return typeof s.focus === "function" ? s.focus(adapterFor(vol)) : s.focus;
};

describe("the fundamentals scenario", () => {
  it("is the first scenario, so the picker defaults to it", () => {
    expect(all[0].id).toBe("fundamentals");
  });

  it("quotes the default geometry correctly", () => {
    const g = FAMILIES[scenario.family].format().geometry();
    expect(g).toMatchObject({
      bytesPerSector: 512, sectorsPerCluster: 4, reservedSectors: 1, fatCount: 2, sectorsPerFat: 32,
      rootEntries: 512, rootDirSectors: 32, firstRootDirSector: 65, firstDataSector: 97, totalSectors: 32768, clusterCount: 8167,
    });
    const bpb = stepText("Sector 0 describes the rest");
    for (const s of [`${g.bytesPerSector} bytes per sector`, `${g.sectorsPerCluster} sectors per cluster`, `${g.reservedSectors} reserved sector`, `${g.fatCount} FATs`, `${g.rootEntries} root entries`, `${n(g.totalSectors)} sectors in total`, `${g.sectorsPerFat} sectors per FAT`]) {
      expect(bpb).toContain(s);
    }
    const regions = stepText("Where each region starts");
    const fat1 = g.reservedSectors + g.sectorsPerFat;
    for (const s of [`FAT 0 begins at sector ${g.reservedSectors}`, `FAT 1 begins at sector ${fat1}`, `root directory at ${g.firstRootDirSector}`, `${n(g.rootEntries * 32)} bytes, ${g.rootDirSectors} sectors`, `data area begins at sector ${g.firstDataSector}`]) {
      expect(regions).toContain(s);
    }
    expect(stepText("One disk, five regions")).toContain(`${n(g.totalSectors)} sectors of ${g.bytesPerSector} bytes`);
    expect(stepText("Free means zero in the table")).toContain(`${n(g.clusterCount)} clusters exist`);
  });

  it("puts HELLO.TXT in cluster 2 with an end-of-chain entry", () => {
    const vol = runThrough("One disk, five regions");
    expect(vol.stat(HELLO).size).toBe(HELLO_TEXT.length);
    expect(vol.clusterOwners().filter((o) => o.path === HELLO).map((o) => o.cluster)).toEqual([2]);
    expect(vol.fatEntries(0)[2]).toEqual({ kind: "endOfChain" });
    expect(vol.fatEntries(1)[2]).toEqual({ kind: "endOfChain" });
    const g = vol.geometry();
    expect(stepText("Following the links")).toContain(`sector ${g.firstDataSector} + (2 − 2) × ${g.sectorsPerCluster} = sector ${g.firstDataSector}`);
    expect(stepText("Following the links")).toContain(`Only ${HELLO_TEXT.length} of the cluster's ${n(g.bytesPerSector * g.sectorsPerCluster)} bytes`);
  });

  it("chains BIGGER.TXT through clusters 3, 4, 5 with the quoted slack", () => {
    const vol = runThrough("A larger file chains clusters");
    expect(biggerText(BIGGER_BYTES).length).toBe(BIGGER_BYTES);
    expect(vol.stat(BIGGER).size).toBe(BIGGER_BYTES);
    expect(vol.clusterOwners().filter((o) => o.path === BIGGER).map((o) => o.cluster).sort((a, b) => a - b)).toEqual([3, 4, 5]);
    const fat = vol.fatEntries(0);
    expect(fat[3]).toEqual({ kind: "next", cluster: 4 });
    expect(fat[4]).toEqual({ kind: "next", cluster: 5 });
    expect(fat[5]).toEqual({ kind: "endOfChain" });
    expect(fat[6]).toEqual({ kind: "free" });
    const g = vol.geometry();
    const perCluster = g.bytesPerSector * g.sectorsPerCluster;
    const clusters = Math.ceil(BIGGER_BYTES / perCluster);
    expect(clusters).toBe(3);
    expect(stepText("A larger file chains clusters")).toContain(`${n(BIGGER_BYTES)} bytes`);
    expect(stepText("Size versus space")).toContain(`${clusters} × ${n(perCluster)} = ${n(clusters * perCluster)}`);
    expect(stepText("Size versus space")).toContain(`The ${n(clusters * perCluster - BIGGER_BYTES)} bytes at the end of cluster 5`);
    const used = fat.filter((e) => e.kind !== "free").length - 2; // entries 0 and 1 are reserved
    expect(used).toBe(4);
    expect(stepText("Free means zero in the table")).toContain(`${used} are in use, so ${n(g.clusterCount - used)} entries read 0000`);
  });

  it("points each table step at the FAT entry it talks about", () => {
    const vol = runThrough("A larger file chains clusters");
    const g = vol.geometry();
    const fat0 = g.reservedSectors * g.bytesPerSector;
    const fat1 = (g.reservedSectors + g.sectorsPerFat) * g.bytesPerSector;
    expect(focusOf("The FAT: one entry per cluster", vol)).toEqual({ offset: fat0 + 2 * 2 });
    expect(focusOf("FAT 1 is a mirror", vol)).toEqual({ offset: fat1 + 2 * 2 });
    expect(focusOf("The chain in the table", vol)).toEqual({ path: BIGGER, offset: fat0 + 3 * 2 });
    expect(focusOf("Free means zero in the table", vol)).toEqual({ path: null, offset: fat0 + 6 * 2 });
    // HELLO.TXT took the first root slot, so its entry starts where the root directory does.
    expect(focusOf("The root directory names the file", vol)).toEqual({ path: HELLO, offset: g.firstRootDirSector * g.bytesPerSector });
    expect(focusOf("Sector 0 describes the rest", vol)).toEqual({ offset: 11 });
  });
});
```

Only construction and dispatch changed: `FAMILIES[scenario.family].format()` where `Volume.formatFat16(undefined)` was, `step.action(vol, fs)` plus `fs.refresh()`, `s.focus(adapterFor(vol))`, and `Volume` is now a type-only import. Every asserted number and string is the one in today's file. The FAT-copy expectation on line `focusOf("FAT 1 is a mirror", vol)` (`fat1 + 2 * 2`) is what pins `fatEntryOffset(2, 1)` to the second copy.

- [ ] **Step 3: Run both files and confirm they fail against today's scenarios**

Run: `pnpm vitest run tests/scenarios.test.ts tests/fundamentals.test.ts`

Expected: `tests/scenarios.test.ts` fails 11 of 16 cases and `tests/fundamentals.test.ts` fails 4 of 5, all for the same reason: no scenario has a `family` yet, so `FAMILIES[s.family]` is `undefined`, giving `TypeError: Cannot read properties of undefined (reading 'format')` in every "runs ... end to end", in "leaves the volume the way the equivalent commands would", and in the four fundamentals cases that build a volume; "every scenario names a registered family" fails with `AssertionError: fundamentals names family "undefined": expected undefined not to be undefined`. The cases that pass are the two text checks, the two cursor checks, "is registered last", and "is the first scenario".

- [ ] **Step 4: Rewrite the runner with the final generic types**

Replace the whole of `web/ui/src/state/scenarios.svelte.ts` with:

```ts
import type { OpRecord, Volume } from "../lib/wasm";
import type { FsAdapter, FsFamilyId } from "../fs/adapter";
import { ScenarioCursor } from "../core/scenarioCursor";
import { selection } from "./selection.svelte";
import { volume } from "./volume.svelte";

export interface StepFocus {
  offset?: number;
  sector?: number;
  unit?: number;
  path?: string | null;
  showRemnants?: boolean;
  strings?: boolean;
}

/** One step of a lesson. `O` is the family's format options: a step that carries `format`
 *  re-formats the disk with them before its action runs. Actions and function focuses get
 *  the adapter bound to the volume they run on, so a scenario reads geometry and ownership
 *  through the seam (`fs.unitSize`, `fs.entrySlots(path)`, ...) and never through a
 *  family-specific wasm call. */
export interface Step<O = unknown> {
  title: string;
  text: string;
  action?: (v: Volume, fs: FsAdapter) => OpRecord;
  format?: O;
  focus?: StepFocus | ((fs: FsAdapter) => StepFocus);
}

export interface Scenario<O = unknown> {
  id: string;
  title: string;
  summary: string;
  /** The family `start()` formats before the first step; every step runs on that family. */
  family: FsFamilyId;
  steps: Step<O>[];
}

/** Drives `volume`/`selection` through a scenario's steps, one at a time. */
export class ScenarioRunner {
  current = $state<Scenario | null>(null);
  index = $state(-1);
  readonly step: Step | null = $derived(this.current ? (this.current.steps[this.index] ?? null) : null);
  /** The current step's focus with any `(fs: FsAdapter) => StepFocus` already resolved, so
   *  the Lesson card can describe where it pointed the UI without resolving it a second time
   *  (the function form reads the live adapter, and would answer differently later). */
  focus = $state<StepFocus | null>(null);

  /** Step bookkeeping (which steps have run, and the volume cursor each one left). */
  private cursor: ScenarioCursor | null = null;

  /** Begin `s` from a clean default disk of its family, then apply its first step. */
  start(s: Scenario) {
    volume.format(s.family); // also resets the selection
    selection.reset();
    this.current = s;
    this.index = -1;
    this.cursor = new ScenarioCursor(s.steps.length);
    this.next();
  }

  /** Advance to the next step. A step that has already run is replayed by seeking the
   *  timeline back to where its run left the disk, never by running it again. No-op
   *  past the last step. */
  next() {
    if (!this.current || !this.cursor) return;
    const plan = this.cursor.next();
    if (!plan) return;
    this.index = this.cursor.index;
    const step = this.current.steps[this.index];
    if ("seekTo" in plan) {
      volume.seek(plan.seekTo);
      this.applyFocus(step);
      return;
    }
    this.applyStep(step);
    this.cursor.advance(volume.cursor);
  }

  /** Step back: show the disk as it was after the previous step and re-apply its focus.
   *  A step that formatted the disk cannot be rewound through (the format threw the old
   *  history away), so Prev clamps there. */
  prev() {
    if (!this.current || !this.cursor || this.index <= 0) return;
    if (this.current.steps[this.index].format) return;
    const target = this.cursor.back();
    this.index = this.cursor.index;
    if (target !== null) volume.seek(target);
    this.applyFocus(this.current.steps[this.index]);
  }

  stop() {
    this.current = null;
    this.index = -1;
    this.cursor = null;
    this.focus = null;
    selection.showRemnants = false;
    selection.stringsOn = false;
  }

  private applyStep(step: Step) {
    // `next()` is the only caller and returns early without a current scenario.
    if (step.format) volume.format(this.current!.family, step.format);
    if (step.action) volume.run((v) => step.action!(v, volume.adapter));
    this.applyFocus(step);
  }

  /** Every path that lands on a step ends here — a fresh run, a Next that seeks to a step
   *  that already ran, and Prev — so this is where the resolved focus is published. */
  private applyFocus(step: Step) {
    const focus = typeof step.focus === "function" ? step.focus(volume.adapter) : step.focus;
    this.focus = focus ?? null;
    if (!focus) return;
    if (focus.path !== undefined) selection.select(focus.path);
    if (focus.offset !== undefined) selection.jumpTo(focus.offset);
    else if (focus.sector !== undefined) selection.jumpTo(focus.sector * volume.sectorSize);
    else if (focus.unit !== undefined) selection.jumpTo(volume.adapter.unitByteRange(focus.unit).start);
    if (focus.showRemnants !== undefined) selection.showRemnants = focus.showRemnants;
    if (focus.strings !== undefined) selection.stringsOn = focus.strings;
  }
}

export const scenarios = new ScenarioRunner();
```

Against today's file: `FormatOptions` and `clusterByteRange` leave the imports, `FsAdapter`/`FsFamilyId` arrive; `Step` and `Scenario` take the spec's `O` parameter and `Scenario.family`; `start` formats `s.family`; `applyStep` formats `this.current.family` with `step.format` and runs `step.action!(v, volume.adapter)` inside `volume.run`; `applyFocus` resolves the function form with `volume.adapter`, maps `sector` through `volume.sectorSize` and `unit` through `volume.adapter.unitByteRange`. `applyFocus` runs imperatively from `next`/`prev` (not inside a `$derived`), so the reactivity rule does not apply here; the adapter it reads is the one `VolumeStore.run`/`adopt` just refreshed or re-bound. `next`, `prev`, `stop`, and the `step`/`focus` fields are unchanged.

- [ ] **Step 5: Rewrite `fundamentals.ts`: the one scenario with a FAT-only fact**

Replace the whole of `web/ui/src/scenarios/fundamentals.ts` with:

```ts
import { asFat16 } from "../fs/fat16";
import type { Scenario } from "../state/scenarios.svelte";

/**
 * The first lesson: a tour of the on-disk structure of a FAT16 volume and how its pieces
 * refer to one another. Two files are created so there is something to point at, but the
 * creation itself is not the subject (that is "Add a small file"); every other step only
 * moves the dump. The numbers quoted in the copy are the default disk's, and
 * tests/fundamentals.test.ts checks them against a freshly formatted Volume.
 *
 * The FAT entry offsets are the one fact here that no generic adapter method answers, so
 * this file reaches past the seam with `asFat16(fs).fatEntryOffset(cluster, copy)`; the
 * boundary test allows `src/scenarios/` to import from `fs/fat16` for exactly this.
 */

export const HELLO = "/HELLO.TXT";
export const BIGGER = "/BIGGER.TXT";
export const HELLO_TEXT = "Hello, FAT16!";
/** More than two clusters, less than three, so the last cluster is partly slack. */
export const BIGGER_BYTES = 4796;

/** Numbered lines, so the dump's ASCII gutter shows readable text in every cluster. */
export function biggerText(bytes: number): Uint8Array {
  let s = "";
  for (let n = 1; s.length < bytes; n++) s += `line ${String(n).padStart(4, "0")}: the quick brown fox jumps over the lazy dog\n`;
  return new TextEncoder().encode(s.slice(0, bytes));
}

export const scenario: Scenario = {
  id: "fundamentals",
  title: "The fundamentals",
  summary: "Tour the regions of a FAT16 disk and follow how a file's slot, chain, and clusters link together.",
  family: "fat16",
  steps: [
    {
      title: "One disk, five regions",
      text: "This volume is 32,768 sectors of 512 bytes: 16 MB of numbered bytes and nothing else. FAT16 divides them into five regions in a fixed order: the boot sector, two copies of the file allocation table, the root directory, and the data area. The ribbon above the dump draws them in that order. HELLO.TXT is already on the disk so there is something to point at.",
      action: (v) => v.createFile(HELLO, new TextEncoder().encode(HELLO_TEXT)),
      focus: { sector: 0 },
    },
    {
      title: "Sector 0 describes the rest",
      text: "The BIOS parameter block at the start of sector 0 is a handful of small numbers: 512 bytes per sector (offset 11), 4 sectors per cluster (13), 1 reserved sector (14), 2 FATs (16), 512 root entries (17), 32,768 sectors in total (19), and 32 sectors per FAT (22). Every other address on the disk is arithmetic on these seven numbers. Point at any of them and the Inspector decodes it.",
      focus: { offset: 11 },
    },
    {
      title: "Where each region starts",
      text: "One reserved sector, so FAT 0 begins at sector 1. Each FAT is 32 sectors, so FAT 1 begins at sector 33 and the root directory at 65. 512 entries × 32 bytes is 16,384 bytes, 32 sectors, so the data area begins at sector 97. Nothing on the disk stores those four numbers; the driver computes them from sector 0, and so does the explorer.",
      focus: (fs) => ({ sector: fs.regionStart("directory") }),
    },
    {
      title: "The FAT: one entry per cluster",
      text: "The allocation table is an array of 16-bit entries, one per data cluster, indexed by cluster number. Entries 0 and 1 are reserved (F8 FF FF FF: the media descriptor and an end marker), so entry 2 is the first real one. A value of 0000 means the cluster is free, FFF8 through FFFF means end of chain, and anything else is the number of the next cluster in the same file. Cluster 2 is HELLO.TXT, and its entry reads end of chain: nothing follows.",
      focus: (fs) => ({ offset: asFat16(fs).fatEntryOffset(2, 0) }),
    },
    {
      title: "FAT 1 is a mirror",
      text: "The second copy starts at sector 33 and is byte for byte the same as the first. Every allocation is written to both, so a damaged first table can be recovered from the second. The entry for cluster 2 here matches the one you just saw.",
      focus: (fs) => ({ offset: asFat16(fs).fatEntryOffset(2, 1) }),
    },
    {
      title: "The root directory names the file",
      text: "The root directory is 512 fixed 32-byte slots starting at sector 65. HELLO.TXT's slot holds its 8.3 name padded with spaces, attribute flags, timestamps, its first cluster at byte 26 of the slot, and its size at byte 28. That first-cluster field is the link from the name to the data.",
      focus: (fs) => ({ path: HELLO, offset: fs.entrySlots(HELLO)?.start }),
    },
    {
      title: "Following the links",
      text: "Reading a file is three hops. The directory slot gives the first cluster: 2. The FAT entry for cluster 2 says whether more follow: it does not. And cluster 2's bytes live at sector 97 + (2 − 2) × 4 = sector 97, because cluster numbering starts at 2 and each cluster is 4 sectors. Only 13 of the cluster's 2,048 bytes are the file; the rest is untouched zeros.",
      focus: { path: HELLO, unit: 2 },
    },
    {
      title: "A larger file chains clusters",
      text: "BIGGER.TXT is 4,796 bytes, more than two clusters' worth, so it takes three: 3, 4, and 5. The FAT map on the left draws the chain as a line and the ribbon shows the three clusters side by side.",
      action: (v) => v.createFile(BIGGER, biggerText(BIGGER_BYTES)),
      focus: { path: BIGGER },
    },
    {
      title: "The chain in the table",
      text: "Now the FAT reads like a linked list: entry 3 says 4, entry 4 says 5, entry 5 says end of chain. The links are sequential here because the disk was empty; on a busy disk they can point anywhere, and a chain that jumps around is what fragmentation means.",
      focus: (fs) => ({ path: BIGGER, offset: asFat16(fs).fatEntryOffset(3, 0) }),
    },
    {
      title: "Size versus space",
      text: "The directory slot records 4,796 bytes; the chain reserves 3 × 2,048 = 6,144. The 1,348 bytes at the end of cluster 5 belong to the file's allocation but not to its contents: slack. The Files panel shows the size; the ribbon shows the space.",
      focus: { path: BIGGER, unit: 5 },
    },
    {
      title: "Free means zero in the table",
      text: "8,167 clusters exist and 4 are in use, so 8,163 entries read 0000. Nothing marks the data area itself as free: when a file is deleted its bytes stay until another file overwrites them, which the \"Delete and see what remains\" scenario shows.",
      focus: (fs) => ({ path: null, offset: asFat16(fs).fatEntryOffset(6, 0) }),
    },
    {
      title: "Putting it together",
      text: "Every path through this disk starts at sector 0: its numbers locate the tables and the root directory, a directory slot names a first cluster, the table chains the rest, and cluster numbers turn into sector addresses by arithmetic. The other scenarios change the disk one operation at a time; watch the ribbon and the Step strip to see the same three places, slot, table, and data, move each time.",
      focus: { path: null, sector: 0 },
    },
  ],
};
```

Mapping from today's helpers: `fatOffset(v, fat, cluster)` was `(g.reservedSectors + fat * g.sectorsPerFat) * g.bytesPerSector + cluster * 2`, which is `asFat16(fs).fatEntryOffset(cluster, fat)` (note the argument order: cluster first, copy second); `entryOffset(v, path)` was `findEntrySlots(...)?.start`, which is `fs.entrySlots(path)?.start`; `v.geometry().firstRootDirSector` is `fs.regionStart("directory")`; `cluster: N` is `unit: N`. The `findEntrySlots` and `Volume` imports are gone; nothing here calls a wasm method other than `createFile`.

- [ ] **Step 6: Rewrite `shell.ts`**

Replace the whole of `web/ui/src/scenarios/shell.ts` with:

```ts
import type { Scenario } from "../state/scenarios.svelte";

const FILE = "/HELLO.TXT";
const DIR = "/DOCS";
const COPY = "/DOCS/COPY.TXT";
const GREETING = "Hello from the shell";

// The volume label lives at boot-sector offset 43, 11 bytes, space padded.
const LABEL_OFFSET = 43;
const LABEL = "SHELLDISK  ";

const enc = (s: string) => new TextEncoder().encode(s);

export const scenario: Scenario = {
  id: "shell",
  title: "Work from the shell",
  summary: "Do the same operations from the terminal drawer: /mnt is the volume, /dev/hda the raw disk.",
  family: "fat16",
  steps: [
    {
      title: "Open the terminal",
      text: "Press ` (backtick) or click Terminal to open the drawer. Type `ls /mnt`: a fresh disk has an empty root. `pwd` prints /mnt, and `mount` and `df` describe the volume.",
      focus: { path: null },
    },
    {
      title: "Write a file from a pipe",
      text: "Type `echo 'Hello from the shell' | write /mnt/HELLO.TXT`. Redirection does the same thing: `echo 'Hello from the shell' > /mnt/HELLO.TXT` is the same journaled write. The same three places change as with Add file: a directory entry, a FAT entry, and a data cluster.",
      action: (v) => v.createFile(FILE, enc(GREETING)),
      focus: { path: FILE },
    },
    {
      title: "Read it back",
      text: "`cat /mnt/HELLO.TXT` prints the text. `stat /mnt/HELLO.TXT` shows the entry offset, first cluster, chain, and data offset, and `seek c:2` moves the dump to that cluster.",
      focus: (fs) => ({ unit: fs.ownerOf(FILE)?.firstUnit }),
    },
    {
      title: "Make a directory",
      text: "`mkdir /mnt/DOCS` allocates a cluster for the directory and writes its `.` and `..` entries there. `cd /mnt/DOCS` then `ls` shows an empty listing (the dot entries are hidden, as on a real shell); `xxd /dev/hda --offset c:3 --len 64` shows the two entries on disk.",
      action: (v) => v.createDir(DIR),
      focus: { path: DIR },
    },
    {
      title: "Copy a file",
      text: "`cp /mnt/HELLO.TXT /mnt/DOCS/COPY.TXT` reads the bytes and creates the copy inside DOCS. A new cluster is allocated for it, right after the directory's own.",
      action: (v) => v.createFile(COPY, v.readFile(FILE)),
      focus: { path: COPY },
    },
    {
      title: "Read the raw boot sector",
      text: "`dd if=/dev/hda bs=512 count=1 | xxd` dumps sector 0 straight from the disk, the way a real tool would. The flag spelling, `dd --if=/dev/hda --bs=512 --count=1`, does the same.",
      focus: { sector: 0 },
    },
    {
      title: "Patch the disk directly",
      text: "`echo 'SHELLDISK  ' | dd --of=/dev/hda --bs=1 --seek=43` overwrites the 11-byte volume label at boot offset 43. The raw write is journaled like any other step, so it rewinds, and the inspector's boot annotation shows the new label.",
      action: (v) => v.writeRaw(LABEL_OFFSET, enc(LABEL)),
      focus: { offset: LABEL_OFFSET },
    },
    {
      title: "Delete and look at what remains",
      text: "`rm /mnt/HELLO.TXT` marks the entry deleted and frees its chain; the bytes stay on disk. `ls -l /mnt` no longer lists it, and the copy in DOCS is untouched.",
      action: (v) => v.deleteFile(FILE),
      // `dataStart("/")` is the root directory's first byte; it answers null only for a path
      // with no bytes, and `?? undefined` keeps the focus's optional `offset` shape.
      focus: (fs) => ({ offset: fs.dataStart("/") ?? undefined, path: null, showRemnants: true }),
    },
  ],
};
```

Mapping: `v.clusterOwners().find((o) => o.path === FILE)?.firstCluster` is `fs.ownerOf(FILE)?.firstUnit`; `v.geometry().firstRootDirSector * v.geometry().bytesPerSector` is `fs.dataStart("/")` (the spec's "`/` -> root directory bytes"), which on the default disk is `65 * 512 = 0x8200` as before.

- [ ] **Step 7: Rewrite the three entry-slot scenarios: `smallFile.ts`, `longName.ts`, `directory.ts`**

Replace the whole of `web/ui/src/scenarios/smallFile.ts` with:

```ts
import type { Scenario } from "../state/scenarios.svelte";

export const scenario: Scenario = {
  id: "small-file",
  title: "Add a small file",
  summary: "Create one small file and see exactly which bytes on disk moved.",
  family: "fat16",
  steps: [
    {
      title: "Three things changed",
      text: "Creating HELLO.TXT touches three places: a directory entry that names it, a FAT entry that allocates its cluster, and the data cluster that holds its bytes.",
      action: (v) => v.createFile("/HELLO.TXT", new TextEncoder().encode("Hello, FAT16!")),
      focus: { path: "/HELLO.TXT" },
    },
    {
      title: "The directory entry",
      text: "This 32-byte slot in the root directory stores the file's name, its size, and the cluster its data starts at.",
      focus: (fs) => ({ offset: fs.entrySlots("/HELLO.TXT")?.start }),
    },
    {
      title: "The FAT entry",
      text: "Back in the allocation table, the entry for cluster 2 now reads end-of-chain: the file uses exactly one cluster and nothing follows it.",
      focus: { sector: 1 },
    },
    {
      title: "The data cluster",
      text: "Cluster 2 holds the file's actual bytes. Only the first 13 bytes were written; the rest of the cluster is still zero.",
      focus: { unit: 2 },
    },
  ],
};
```

Replace the whole of `web/ui/src/scenarios/longName.ts` with:

```ts
import type { Scenario } from "../state/scenarios.svelte";

const PATH = "/Quarterly report (draft).txt";

export const scenario: Scenario = {
  id: "long-name",
  title: "A long file name",
  summary: "See how a name that does not fit 8.3 gets spread across several directory entries.",
  family: "fat16",
  steps: [
    {
      title: "A name too long for one slot",
      text: "This file's name is too long for a plain 8.3 directory entry, so the driver needs extra entries just to store it.",
      action: (v) => v.createFile(PATH, new TextEncoder().encode("Numbers for the quarter.")),
      focus: { path: PATH },
    },
    {
      title: "Long-name entries come first",
      text: "The long-name entries are written backwards, last fragment first, each holding up to 13 characters. They are followed by a short entry, QUARTE~1.TXT, which is what an old DOS driver would see.",
      focus: (fs) => ({ offset: fs.entrySlots(PATH)?.start }),
    },
    {
      title: "The checksum that ties them together",
      text: "Each long-name entry carries a checksum of the short name. The inspector's annotation for this slot shows that byte, which is how a reader confirms the fragments and the short entry belong together.",
      focus: (fs) => ({ offset: fs.entrySlots(PATH)?.start }),
    },
  ],
};
```

Replace the whole of `web/ui/src/scenarios/directory.ts` with:

```ts
import type { Scenario } from "../state/scenarios.svelte";

const DIR = "/DOCS";
const FILE = "/DOCS/NOTE.TXT";

export const scenario: Scenario = {
  id: "directory",
  title: "Directories are files too",
  summary: "Create a directory, put a file inside it, then try to remove it before and after emptying it.",
  family: "fat16",
  steps: [
    {
      title: "A directory is a cluster",
      text: "Creating DOCS allocates a cluster like any file would. That cluster is pre-filled with two entries: a dot entry pointing at itself, and a dot-dot entry pointing at its parent.",
      action: (v) => v.createDir(DIR),
      focus: { path: DIR },
    },
    {
      title: "The dot entries",
      text: "Jump into the directory's own cluster and the two dot entries are the first thing you find, before any file it holds.",
      focus: (fs) => ({ unit: fs.ownerOf(DIR)?.firstUnit }),
    },
    {
      title: "A file inside the directory",
      text: "NOTE.TXT's directory entry is written inside DOCS's own cluster, right after the dot entries, not in the root directory.",
      action: (v) => v.createFile(FILE, new TextEncoder().encode("Draft notes.")),
      focus: (fs) => ({ offset: fs.entrySlots(FILE)?.start }),
    },
    {
      title: "Expect: not empty",
      text: "Removing DOCS while NOTE.TXT still lives inside it fails with DirectoryNotEmpty. FAT16 refuses to free a directory's cluster while it still holds entries.",
      action: (v) => v.removeDir(DIR),
    },
    {
      title: "Empty it first",
      text: "Deleting the one file inside leaves DOCS holding nothing but its dot entries again.",
      action: (v) => v.deleteFile(FILE),
      focus: { path: DIR },
    },
    {
      title: "Now it can go",
      text: "With no files left inside, removeDir succeeds: the directory's cluster is freed and its own entry in the root directory is marked deleted.",
      action: (v) => v.removeDir(DIR),
      focus: { path: null },
    },
  ],
};
```

In all three, `findEntrySlots(v, v.geometry(), v.fatEntries(0), v.clusterOwners(), p)` becomes `fs.entrySlots(p)` (the adapter passes its cached raw owners and table to the moved `findEntrySlots`), and the `../core/direntry` import goes.

- [ ] **Step 8: Rewrite the two unit-arithmetic scenarios: `overwriteGrows.ts`, `fillDisk.ts`**

Replace the whole of `web/ui/src/scenarios/overwriteGrows.ts` with:

```ts
import type { Scenario } from "../state/scenarios.svelte";

const GROWING = "/GROWING.TXT";
const OTHER = "/OTHER.TXT";

export const scenario: Scenario = {
  id: "overwrite-grows",
  title: "Overwrite with a bigger file",
  summary: "Grow a file past its first cluster and watch its chain fragment around a neighbour.",
  family: "fat16",
  steps: [
    {
      title: "One cluster to start",
      text: "GROWING.TXT starts small enough to fit in a single cluster, so its chain is just one entry long.",
      action: (v, fs) => v.createFile(GROWING, new Uint8Array(fs.unitSize).fill(0x41)),
      focus: { path: GROWING },
    },
    {
      title: "Three clusters now",
      text: "Overwriting it with more data grows the chain to three clusters. The FAT map now draws that chain as a short polyline instead of a single dot.",
      action: (v, fs) => v.writeFile(GROWING, new Uint8Array(3 * fs.unitSize).fill(0x42)),
      focus: { path: GROWING },
    },
    {
      title: "Following the chain in the FAT",
      text: "Each cluster's FAT entry points to the next one in the chain, ending in an end-of-chain marker at the last cluster.",
      focus: { sector: 1 },
    },
    {
      title: "A second file arrives",
      text: "OTHER.TXT is created next and claims the first free cluster right after GROWING.TXT's chain.",
      action: (v) => v.createFile(OTHER, new TextEncoder().encode("A neighbour.")),
      focus: { path: OTHER },
    },
    {
      title: "Growing around a neighbour",
      text: "Overwriting GROWING.TXT again with five clusters' worth of data can't use the cluster OTHER.TXT now owns, so the chain skips over it. That skip is fragmentation.",
      action: (v, fs) => v.writeFile(GROWING, new Uint8Array(5 * fs.unitSize).fill(0x43)),
      focus: { path: GROWING },
    },
  ],
};
```

Replace the whole of `web/ui/src/scenarios/fillDisk.ts` with:

```ts
import type { Fat16FormatOptions } from "../fs/fat16";
import type { Scenario } from "../state/scenarios.svelte";

// Typed with the family's options so the `format` step is checked against FormatOptions;
// `Scenario<Fat16FormatOptions>` still fits `all: Scenario[]` because `O` only appears in
// `Step.format`.
export const scenario: Scenario<Fat16FormatOptions> = {
  id: "fill-disk",
  title: "Fill the disk",
  summary: "Run a tiny volume all the way to full and watch the next write fail cleanly.",
  family: "fat16",
  steps: [
    {
      title: "A deliberately tiny disk",
      text: "This volume is formatted much smaller than usual, with one sector per cluster, so it only takes one file to run it out of room.",
      format: { totalSectors: 8192, sectorsPerCluster: 1, enforceFat16Range: true },
      focus: { sector: 0 },
    },
    {
      title: "Nearly every cluster claimed",
      text: "BIG.BIN is sized to use every free cluster but one, leaving exactly one cluster of room on the whole disk.",
      action: (v, fs) => v.createFile("/BIG.BIN", new Uint8Array((fs.unitCount - 1) * fs.unitSize)),
      focus: { path: "/BIG.BIN" },
    },
    {
      title: "Expect: disk full",
      text: "Creating a file that needs two clusters fails with the DiskFull error, since only one cluster is left. The operation is rolled back, so the timeline gains no new step and the disk is unchanged.",
      action: (v, fs) => v.createFile("/TOOBIG.BIN", new Uint8Array(fs.unitSize + 1)),
    },
  ],
};
```

`bytesPerCluster(v)` (`bytesPerSector * sectorsPerCluster`) is `fs.unitSize`. In `fillDisk` today's sizes are `(clusterCount - 1) * bytesPerSector` and `bytesPerSector + 1`, written against a one-sector cluster; `(fs.unitCount - 1) * fs.unitSize` and `fs.unitSize + 1` are the same 512-byte multiples on that format (unit size 512) and say what the copy means. The adapter handed to these actions is bound to the tiny volume: the runner's `volume.format(..., step.format)` re-binds through `adopt` before `action` runs, and the test re-binds with `adapterFor` after a `format` step.

- [ ] **Step 9: Rewrite the last two scenarios: `deleteRemnants.ts`, `format.ts`**

Replace the whole of `web/ui/src/scenarios/deleteRemnants.ts` with:

```ts
import type { Scenario } from "../state/scenarios.svelte";

const PATH = "/NOTE.TXT";
const SENTENCE = "Meet at noon on Friday.";

export const scenario: Scenario = {
  id: "delete-remnants",
  title: "Delete and see what remains",
  summary: "Delete a file and find out how little actually disappears from the disk.",
  family: "fat16",
  steps: [
    {
      title: "A file with a memorable sentence",
      text: "NOTE.TXT holds one sentence, written into its single data cluster.",
      action: (v) => v.createFile(PATH, new TextEncoder().encode(SENTENCE)),
      focus: { path: PATH },
    },
    {
      title: "Deleting only marks the entry",
      text: "Deleting the file only rewrites the first byte of its directory entry to 0xE5. The rest of the name is still sitting right there, readable.",
      action: (v) => v.deleteFile(PATH),
      focus: (fs) => ({ offset: fs.dataStart("/") ?? undefined, showRemnants: true }),
    },
    {
      title: "The data never moved",
      text: "Cluster 2's bytes are untouched, and its FAT entry now reads free. That gap between 'marked deleted' and 'actually erased' is exactly what undelete tools rely on.",
      focus: { unit: 2, showRemnants: true },
    },
    {
      title: "A new file overwrites the remnant",
      text: "Creating another file reuses the first free directory slot and the first free cluster, so it lands right on top of NOTE.TXT's remnant and erases it for good.",
      action: (v) => v.createFile("/AGAIN.TXT", new TextEncoder().encode("A fresh file.")),
      focus: { path: "/AGAIN.TXT", showRemnants: true },
    },
  ],
};
```

Replace the whole of `web/ui/src/scenarios/format.ts` with:

```ts
import type { Scenario } from "../state/scenarios.svelte";

export const scenario: Scenario = {
  id: "format",
  title: "Format an empty disk",
  summary: "Walk the regions of a freshly formatted volume before any file exists.",
  family: "fat16",
  steps: [
    {
      title: "The boot sector",
      text: "Sector 0 holds the BIOS parameter block: sector size, cluster size, and where every other region starts. The inspector decodes each field when you point at this sector.",
      focus: { sector: 0 },
    },
    {
      title: "Two copies of the FAT",
      text: "The file allocation table starts at sector 1. Entries 0 and 1 are reserved housekeeping slots; every other entry is free until a file claims it.",
      focus: { sector: 1 },
    },
    {
      title: "The root directory",
      text: "After both FAT copies comes the root directory: 512 fixed 32-byte slots, laid out before any data cluster exists.",
      focus: (fs) => ({ sector: fs.regionStart("directory") }),
    },
    {
      title: "Free space collapses",
      text: "The data region starts here and it is all free. The dump folds thousands of identical empty sectors into a single collapsed row so you can skip past them.",
      focus: (fs) => ({ sector: fs.regionStart("data") }),
    },
  ],
};
```

`v.geometry().firstRootDirSector` is `fs.regionStart("directory")` and `v.geometry().firstDataSector` is `fs.regionStart("data")`: the first sector of the first region of that kind in `vol.layout()`, 65 and 97 on the default disk. `web/ui/src/scenarios/index.ts` needs no edit: `all: Scenario[]` still lists the same nine values in the same order.

- [ ] **Step 10: Retire Task 2's three `core/` re-export shims**

Task 2 moved `direntry.ts`, `fatchain.ts`, and `remnants.ts` under `src/fs/fat16/` and left one-line shims at their old paths (`web/ui/src/core/direntry.ts` is `export * from "../fs/fat16/direntry";`, and likewise `core/fatchain.ts` and `core/remnants.ts`) so the files still importing from `core/` compiled unchanged. Their last `src/` importers are now gone: Task 4 Step 2 (`layers.svelte.ts`) and Step 15 (`commands.ts`) took `core/direntry`, `core/fatchain`, and `core/remnants` out of the stores and the shell, Task 5 Step 3 moved the FAT map's `clusterState` import to `./fatchain`, and Steps 5 and 7 above dropped `../core/direntry` from `fundamentals.ts`, `smallFile.ts`, `longName.ts`, and `directory.ts`. Each shim is an import from `fs/fat16` in a file outside `src/fs/fat16/`, `src/fs/index.ts`, `src/fs/panels.ts`, and `src/scenarios/**`, which is exactly what the third test in Task 7's `tests/adapterBoundary.test.ts` fails on (`core/direntry.ts: imports from fs/fat16 (from "../fs/fat16/direntry"); only fs/index.ts, fs/panels.ts and scenarios/ may`, and the same line for the other two), so they leave in the task that removed their last importer. Task 4 Step 21 deletes `core/remnants.ts` only when nothing imports it, and Task 5 Step 14 leaves all three in place while any `core/direntry` importer remains (the four scenario files, until this task), so expect all three, or at least `direntry` and `fatchain`, to still exist here.

Run: `cd /Users/bsmall/dev/fs-emulator/web/ui && grep -rn "core/direntry\|core/fatchain\|core/remnants" src tests`

Expected: no output (the exit status is 1). Task 2 re-pointed `tests/{direntry,fatchain,remnants}.test.ts`, Task 3 `tests/integration.test.ts`, Task 4 `src/state/layers.svelte.ts`, `src/shell/commands.ts`, and `tests/shell/read-commands.test.ts`, Task 5 the FAT map, and this task the four scenario files. `tests/corruption.test.ts` imports `findEntrySlots` from `../src/core/direntry` and `findRemnants` from `../src/core/remnants` at HEAD (lines 4-5), and Task 3 Step 16 is where those two lines should have been re-pointed; if the grep prints them, re-point them now, to `../src/fs/fat16/direntry` and `../src/fs/fat16/remnants` (the exported names are unchanged), run `pnpm exec vitest run tests/corruption.test.ts` (expected: `tests/corruption.test.ts (6 tests)` passes), and add the file to the `git add` line in Step 14. Any other line the grep prints is a file whose task did not finish the re-point: fix that one import the same way (`core/<name>` to `fs/fat16/<name>`, with the relative path adjusted for the file's depth) rather than leaving the shim behind for it.

Run: `cd /Users/bsmall/dev/fs-emulator && ls web/ui/src/core/direntry.ts web/ui/src/core/fatchain.ts web/ui/src/core/remnants.ts 2>/dev/null`, then `git rm` every path it lists; when all three are still there:

```
git rm web/ui/src/core/direntry.ts web/ui/src/core/fatchain.ts web/ui/src/core/remnants.ts
```

Expected: `rm 'web/ui/src/core/direntry.ts'` and one such line per path removed. Name only the paths `ls` printed: `git rm` of a path that is already gone (Task 4 Step 21 may have removed `core/remnants.ts`) fails with `fatal: pathspec ... did not match any files` and removes nothing. Nothing else in `src/core/` is a shim; `attribution.ts`, `lesson.ts`, `tree.ts`, `patch.ts`, and the rest are the generic modules Task 3 rewrote, and they stay.

Run: `cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm exec vitest run tests/direntry.test.ts tests/fatchain.test.ts tests/remnants.test.ts tests/corruption.test.ts tests/integration.test.ts tests/shell`

Expected: every file passes; they import the moved helpers from `src/fs/fat16/` directly, so nothing resolved through the shims. A `Failed to resolve import "../src/core/..."` error names an import the grep above should have caught: fix that import, not the deletion.

- [ ] **Step 11: Run the two test files**

Run: `pnpm vitest run tests/scenarios.test.ts tests/fundamentals.test.ts`

Expected:

```
 ✓ tests/fundamentals.test.ts (5 tests)
 ✓ tests/scenarios.test.ts (16 tests)

 Test Files  2 passed (2)
      Tests  21 passed (21)
```

(`scenarios` was 15 cases at HEAD; the sixteenth is "every scenario names a registered family".)

- [ ] **Step 12: Run the full suite, the build, and a boundary pre-check**

Run: `pnpm test`

Expected: every test file passes, no failures (the file count is whatever Tasks 2 to 5 left plus none; `tests/lesson.test.ts` still reports 8 passing, `tests/scenarios.test.ts` 16, `tests/fundamentals.test.ts` 5).

Run: `pnpm build`

Expected: the svelte-check line ends `0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS`, then vite prints `✓ built in ...`. svelte-check type-checks `tests/` too (`tsconfig.json` includes it), so this also proves the two rewritten test files compile against the spec's signatures.

Run, from `web/ui` (a preview of the Task 7 scan over the files this task owns):

```
grep -rnE '\.(geometry|fatEntries|clusterOwners|annotateSectorWith|rawDirEntries|bootSector|clusterChain|formatFat16)\s*\(|core/direntry|[{,] cluster:' src/scenarios src/state/scenarios.svelte.ts
```

Expected: no output (the exit status is 1). At HEAD 81b0b56 the same command matches 26 lines, so an empty result is the migration, not a silent pattern. (`[{,] cluster:` is the focus-key form, `{ cluster: 2 }` and `{ path: HELLO, cluster: 2 }`; the plain word survives in the copy, as in "the first cluster: 2", and must.) The only `fs/fat16` imports under `src/scenarios/` are `fundamentals.ts` (`asFat16`) and `fillDisk.ts` (`Fat16FormatOptions`), both allowed by the boundary rule.

Run: `grep -rln "fs/fat16" src/core`

Expected: no output (the exit status is 1). Step 10 removed the three shims, and they were the only `src/core/` files importing from `fs/fat16` (Task 3's rewritten `attribution.ts`, `lesson.ts`, and `tree.ts` import only types from `../fs/adapter`, never from `fs/fat16`). This is the `core/` case of Task 7's third boundary test, checked here so Task 7 does not discover it.

- [ ] **Step 13: Browser pass: the picker, "The fundamentals", and "Work from the shell"**

Start the dev server: the `.claude/launch.json` configuration `ui` (`pnpm --dir web/ui dev --port 5174 --strictPort`), then open `http://localhost:5174`.

Picker: the "Learning scenarios" select lists nine options in this order: The fundamentals, Format an empty disk, Add a small file, A long file name, Overwrite with a bigger file, Delete and see what remains, Fill the disk, Directories are files too, Work from the shell. The fundamentals is preselected.

"The fundamentals" (12 steps; each Next must move the dump, and the card's "Look at:" line must read exactly as listed, because `describeFocus` output is a lesson string):
1. Start. The card reads "Lesson · step 1 of 12"; the Files tree lists HELLO.TXT; the timeline has one step (`create_file /HELLO.TXT`); the dump is at sector 0; Look at: `Sector 0, reserved (boot sector)`.
2. Next: cursor at offset 0xb; Look at: `Offset 0xb in reserved (boot sector)`; the Inspector decodes bytes per sector = 512.
3. Next: dump at sector 65; Look at: `Sector 65, root directory`.
4. Next: cursor at 0x204 (FAT 0, entry 2); Look at: `Offset 0x204 in FAT 0`; the Inspector shows sector 1's FAT annotations (the cursor is in the table, so it has no Cluster row).
5. Next: cursor at 0x4204; Look at: `Offset 0x4204 in FAT 1`.
6. Next: HELLO.TXT selected in the tree, cursor at 0x8200 with its 32-byte slot highlighted; Look at: `Files: /HELLO.TXT, its entry, chain, and clusters; offset 0x8200 in root directory`.
7. Next: dump at 0xc200 showing "Hello, FAT16!"; Look at: `Files: /HELLO.TXT, its entry, chain, and clusters; cluster 2 in the data region (offset 0xc200)`; the Inspector's Cluster row reads `2 · FAT: end of chain` and its Owner row `/HELLO.TXT`.
8. Next: BIGGER.TXT appears in the tree and the Step strip shows `create_file /BIGGER.TXT`; the cursor does not move (the focus only selects), but the selection highlight now covers clusters 3 to 5 and BIGGER.TXT's slot; the FAT map draws the 3-4-5 chain as a line; the ribbon shows three clusters of one colour; Look at: `Files: /BIGGER.TXT, its entry, chain, and clusters`.
9. Next: cursor at 0x206; Look at: `Files: /BIGGER.TXT, its entry, chain, and clusters; offset 0x206 in FAT 0`.
10. Next: dump at 0xda00 (cluster 5); Look at: `Files: /BIGGER.TXT, its entry, chain, and clusters; cluster 5 in the data region (offset 0xda00)`; the Inspector's Cluster row reads `5 · FAT: end of chain`; the tail of the cluster after the text is zeros (the 1,348 bytes of slack).
11. Next: the selection clears (no selected node in the tree, no selection highlight in the dump or ribbon), cursor at 0x20c; Look at: `Offset 0x20c in FAT 0`.
12. Next: dump at sector 0; Look at: `Sector 0, reserved (boot sector)`; the button reads Finish. Press Prev twice and Next twice: the timeline gains no new step (already-run steps are seeked, not re-run) and each Look at line comes back as above. Finish closes the card and focus lands on the Start button.

"Work from the shell" (8 steps), started from the picker after Finish:
1. Start: a fresh disk (the tree is empty, the timeline has no steps); no Look at line (the focus only clears the selection). Press ` to open the terminal; `ls /mnt` prints nothing; `df` shows `clusterSize` 2048 and `clusters` 8167.
2. Next: HELLO.TXT in the tree, the Step strip shows `create_file /HELLO.TXT`; Look at: `Files: /HELLO.TXT, its entry, chain, and clusters`. In the terminal `cat /mnt/HELLO.TXT` prints `Hello from the shell`.
3. Next: dump at 0xc200; Look at: `Cluster 2 in the data region (offset 0xc200)`; the Inspector's Cluster row reads `2 · FAT: end of chain`. In the terminal `stat /mnt/HELLO.TXT` shows `firstCluster` 2 and `dataOffset` 0xc200; `seek c:2` keeps the dump where it is.
4. Next: DOCS in the tree, the Step strip shows `create_dir /DOCS`; Look at: `Files: /DOCS, its entry, chain, and clusters`; the cursor stays put (a path-only focus) while the selection highlight moves to DOCS's cluster (3) and its root slot; scrolling the dump to 0xca00 (cluster 3) shows the `.` and `..` entries.
5. Next: DOCS/COPY.TXT in the tree; Look at: `Files: /DOCS/COPY.TXT, its entry, chain, and clusters`; the ribbon shows its cluster (4) right after DOCS's, and the highlighted slot is inside DOCS's cluster, not the root directory.
6. Next: dump at sector 0; Look at: `Sector 0, reserved (boot sector)`.
7. Next: cursor at 0x2b; Look at: `Offset 0x2b in reserved (boot sector)`; the Step strip shows `write_raw 0x2b +11`; the Inspector's boot annotation for the volume label reads SHELLDISK; the diff highlight covers 11 bytes.
8. Next: HELLO.TXT gone from the tree; the Show remnants checkbox is on and its root slot is drawn hatched at 0x8200; Look at: `Offset 0x8200 in root directory; deleted entries are shown hatched`. `ls -l /mnt` in the terminal lists DOCS only. Finish closes the card and turns Show remnants off.

If any Look at line, offset, or tree state differs, the fault is in this task's focus mapping (Steps 5 to 9) or in Task 4's `volume.format`/`run` re-binding; fix before committing.

- [ ] **Step 14: Commit**

```
git add web/ui/src/state/scenarios.svelte.ts web/ui/src/scenarios/deleteRemnants.ts web/ui/src/scenarios/directory.ts web/ui/src/scenarios/fillDisk.ts web/ui/src/scenarios/format.ts web/ui/src/scenarios/fundamentals.ts web/ui/src/scenarios/longName.ts web/ui/src/scenarios/overwriteGrows.ts web/ui/src/scenarios/shell.ts web/ui/src/scenarios/smallFile.ts web/ui/tests/scenarios.test.ts web/ui/tests/fundamentals.test.ts
git commit -m "refactor(ui): scenarios declare their family and reach the disk through FsAdapter

Scenario gains family, Step.action takes (v, fs) and function focuses take fs,
so the nine FAT16 lessons read entry slots, owners, region starts, and unit
sizes from the adapter instead of the FAT geometry. The runner formats the
scenario's family and resolves focuses with volume.adapter. Only the
fundamentals tour needs a FAT-only fact and uses asFat16(fs).fatEntryOffset.
The scenario and fundamentals tests build volumes through FAMILIES and hand
every action an adapter; every asserted number and string is unchanged.
The scenario files were the last importers of the core/{direntry,fatchain,
remnants}.ts re-export shims left when those helpers moved under fs/fat16,
so the shims go too: a core/ import from fs/fat16 is what the adapter
boundary test forbids."
```

(The `git rm` in Step 10 is already staged, and `git add` of a removed path fails with `pathspec ... did not match any files`, so the three shims are not on the `git add` line; `git status --short` before the commit shows them as `D ` and nothing unstaged. If Step 10 re-pointed `tests/corruption.test.ts`, add `web/ui/tests/corruption.test.ts` to the `git add` line.)
### Task 7: Naming pass, boundary test, docs

Spec `docs/superpowers/specs/2026-09-23-fs-adapter-design.md`, section 5 (the "fs explorer" rename), the `tests/adapterBoundary.test.ts` bullet of section 6, and the Verification section: this task renames the app, adds the source scan that keeps FAT-only wasm calls and types inside `src/fs/fat16/`, brings the READMEs and the roadmap in line with the adapter seam, runs every gate, and ends with the browser pass. Nothing here changes behaviour; every step is a string, a doc, a test, or a check.

**Files:**

- Create: `web/ui/tests/adapterBoundary.test.ts`
- Modify: `web/ui/index.html` (line 6, `<title>`)
- Modify: `web/ui/src/App.svelte` (line 55 today, `<h1>FAT explorer</h1>`; Task 5 may already have renamed it, Step 3 says how to tell)
- Modify: `.github/workflows/pages.yml` (line 19, the build job's `name`)
- Modify: `web/ui/src/shell/commands.ts` (line 552 today, the `touch` no-op message; Task 4 may already have renamed it, Step 6 says how to tell)
- Modify: `web/ui/tests/shell/mutations.test.ts` (line 182, the pin of that message)
- Modify: `web/ui/README.md` (line 1 title; lines 3–4 description; line 27 test comment; lines 224–235, the "What is FAT-specific" section)
- Modify: `docs/ROADMAP.md` (lines 54–56 FAT32 UI bullet; lines 93–96 the `direntry.ts`/`remnants.ts` note; the adapter bullet Task 1 added under "What stays fixed" is verified by grep, not edited)
- Modify: `README.md` (root; lines 31–36 the "Each layer only depends" paragraph; lines 115–119 the specs list)
- Test: `web/ui/tests/adapterBoundary.test.ts` (new), `web/ui/tests/shell/mutations.test.ts` (one expectation string)

**Interfaces:**

Consumes (all exist by the time this task runs):

- Task 2, `web/ui/src/fs/index.ts`: `export const FAMILIES: Record<FsFamilyId, FsFamily>`, `export const DEFAULT_FAMILY: FsFamilyId`, `export function familyIdOf(fsType: string): FsFamilyId`, `export function adapterFor(vol: Volume): FsAdapter`. The README describes them; nothing here imports them.
- Task 2, `web/ui/src/fs/adapter.ts`: `UnitVocab`, `UnitOwner`, `UnitSpace`, `TraceRow`, `StatFacts`, `DfFacts`, `MkfsFlag`, `MkfsSpec`, `FsFamily<O>`, `FsAdapter` (spec section 1, verbatim), and its documented reactivity rule. Described, not imported.
- Task 2, `web/ui/src/fs/fat16/`: `index.ts` (`export const fat16: FsFamily<FormatOptions>`, `export function asFat16(fs: FsAdapter): Fat16Adapter`), `adapter.ts` (`class Fat16Adapter implements FsAdapter`, `fatEntryOffset(cluster: number, copy = 0): number`), `geometry.ts`, `fatchain.ts`, `direntry.ts`, `remnants.ts`, `metadata.ts`, `format.ts` (`clusterCountFor`, `checkFormat`, `DEFAULTS`, `MKFS`). The scan's allowed directory is `src/fs/fat16/`; the sanity check in the test asserts `fs/fat16/adapter.ts` exists.
- Task 4, `web/ui/src/shell/host.ts`: `ShellHost.adapter: FsAdapter` and `format(family: FsFamilyId, options?: unknown): void`; `web/ui/src/shell/commands.ts` still logs the `touch` no-op through `` ctx.log(`${display} exists; ... has no timestamp-only update, nothing written`) ``; `web/ui/tests/shell/helpers.ts` `TestHost` holds an adapter. The pin and the message are the only shell strings this task touches.
- Task 5, `web/ui/src/fs/panels.ts`: `export const PANELS: Record<FsFamilyId, { map: Component; format: Component }>`; `App.svelte` renders `<MapPanel />` from it. Described in the README.
- Task 6, `web/ui/src/scenarios/*.ts`: `family: "fat16"` on every scenario; `fundamentals.ts` imports `asFat16` from `"../fs/fat16"`. The scan allows `src/scenarios/**` to import from `fs/fat16`.
- Task 1, `docs/ROADMAP.md`: the adapter bullet under "What stays fixed" (Task 1 Step 13 owns that bullet, spec section 4 item 6; its final wording names `web/ui/tests/adapterBoundary.test.ts`), checklist step 5 rewritten, the deferred `fatEntryOffset` note deleted; `crates/wasm/README.md`: `corruption` in the generic list and a "Detection" paragraph. Step 12 verifies them by grep.
- Existing: `web/ui/tests/layout.test.ts` (the source-scan shape: `readFileSync(new URL("../src/app.css", import.meta.url), "utf8")`), `web/ui/vitest.config.ts` (`include: ["tests/**/*.test.ts"]`, node environment), `web/ui/tsconfig.json` (`include: ["src", "tests", ...]`, so svelte-check type-checks the new test too), `.claude/launch.json` (`ui`: `pnpm --dir web/ui dev --port 5174 --strictPort`).

Produces:

- `web/ui/tests/adapterBoundary.test.ts`: `export function violations(rel: string, text: string): string[]` (a pure rule check the test's own positive controls call; no `src/` module may import a test) and the three tests `scans the tree it is meant to guard`, `recognises each kind of leak (positive controls)`, `keeps FAT-only wasm calls, FAT-only wasm types and fs/fat16 imports out of the generic code`.
- The renamed strings: `<title>fs explorer</title>` (`web/ui/index.html`), `<h1>fs explorer</h1>` (`web/ui/src/App.svelte`), `` `${display} exists; fs explorer has no timestamp-only update, nothing written` `` (`web/ui/src/shell/commands.ts`), `name: build fs explorer` (`.github/workflows/pages.yml`).
- No later task consumes anything from here; this is the last task of the plan.

Gate commands used in this task (run from the repository root unless a step says otherwise):

```
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
cargo build --workspace --target wasm32-unknown-unknown
wasm-pack test --node crates/wasm
wasm-pack build crates/wasm --target bundler
cd web/ui && pnpm install --frozen-lockfile && pnpm test && pnpm build
```

---

- [ ] **Step 1: Grep the tree for the old name and record a decision for every hit**

Run:

```
cd /Users/bsmall/dev/fs-emulator && grep -rn "FAT explorer" --exclude-dir=node_modules --exclude-dir=target --exclude-dir=pkg --exclude-dir=.git --exclude-dir=dist --exclude-dir=.superpowers .
```

Expected (the hits at HEAD `81b0b56`; Tasks 4 and 5 may already have removed the `commands.ts`, `mutations.test.ts`, and `App.svelte` lines, in which case those three do not print):

| hit | decision |
|---|---|
| `web/ui/README.md:1` `# FAT explorer UI` | rename (Step 11) |
| `web/ui/README.md:226` `The app is the FAT explorer today, but most of it does not know what FAT is.` | the whole section is rewritten (Step 11) |
| `web/ui/index.html:6` `<title>FAT explorer</title>` | rename (Step 2) |
| `web/ui/src/App.svelte:55` `<h1>FAT explorer</h1>` | rename (Step 3) |
| `web/ui/tests/shell/mutations.test.ts:182` | rename the pin (Step 5) |
| `web/ui/src/shell/commands.ts:552` | rename the message (Step 7) |
| `.github/workflows/pages.yml:19` `name: build FAT explorer` | rename (Step 4) |
| `docs/superpowers/plans/2026-09-21-fat-explorer-ui.md` (4 hits), `docs/superpowers/plans/2026-09-22-terminal-access.md` (5 hits) | keep: plans and specs are historical records (spec section 5) |

`.superpowers/*.diff` is untracked scratch (`git ls-files .superpowers` prints nothing) and is excluded above. Everything the spec lists as deliberately not renamed stays as it is: `FatMap.svelte`, the `FAT map` heading and `aria-label="FAT cluster map"`, the `.fatmap*` classes, all scenario copy, `formatFat16`/`NotFat`/`FatFs`, `tests/fixtures/geometry.ts`, the `fs-explorer.*` storage keys, the package names, the crates README's "FAT-specific" wording, and the `write --append` and `dd` clause texts.

- [ ] **Step 2: Rename the page title**

In `web/ui/index.html` (line 6), replace:

```html
    <title>FAT explorer</title>
```

with:

```html
    <title>fs explorer</title>
```

- [ ] **Step 3: Rename the h1**

In `web/ui/src/App.svelte` (line 55 at HEAD; after Task 5 the `<header class="topbar">` block is unchanged apart from the map panel), replace:

```svelte
    <h1>FAT explorer</h1>
```

with:

```svelte
    <h1>fs explorer</h1>
```

If Step 1 printed no `App.svelte` hit, Task 5 already made this edit; confirm with `grep -n "<h1>" web/ui/src/App.svelte` (expected: `<h1>fs explorer</h1>`) and move on.

- [ ] **Step 4: Rename the Pages build job**

In `.github/workflows/pages.yml` (line 19), replace:

```yaml
    name: build FAT explorer
```

with:

```yaml
    name: build fs explorer
```

- [ ] **Step 5: Pin the renamed `touch` message (the failing test)**

In `web/ui/tests/shell/mutations.test.ts` (line 182, inside `it("touch creates an empty file once and is a logged no-op afterwards")`), replace:

```ts
    expect(again.log).toEqual(["/mnt/e.txt exists; FAT explorer has no timestamp-only update, nothing written"]);
```

with:

```ts
    expect(again.log).toEqual(["/mnt/e.txt exists; fs explorer has no timestamp-only update, nothing written"]);
```

If Step 1 printed no `mutations.test.ts` hit, Task 4 already made this edit; leave the line as it is.

- [ ] **Step 6: Run the mutation tests and watch the pin fail**

Run: `cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm exec vitest run tests/shell/mutations.test.ts`

Expected: `touch creates an empty file once and is a logged no-op afterwards` fails with

```
AssertionError: expected [ '/mnt/e.txt exists; FAT explorer has no timestamp-only update, nothing written' ] to deeply equal [ '/mnt/e.txt exists; fs explorer has no timestamp-only update, nothing written' ]
```

and every other test in the file passes. If the whole file passes, Task 4 already renamed the message as well: skip Steps 7 and 8.

- [ ] **Step 7: Rename the `touch` message**

In `web/ui/src/shell/commands.ts` (line 552 at HEAD, inside `const touch: CommandDef`), replace:

```ts
        ctx.log(`${display} exists; FAT explorer has no timestamp-only update, nothing written`);
```

with:

```ts
        ctx.log(`${display} exists; fs explorer has no timestamp-only update, nothing written`);
```

- [ ] **Step 8: Run the mutation tests and watch them pass**

Run: `cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm exec vitest run tests/shell/mutations.test.ts`

Expected: every test in `tests/shell/mutations.test.ts` passes (the file's test count is unchanged).

- [ ] **Step 9: Write the adapter boundary test**

Create `web/ui/tests/adapterBoundary.test.ts` with exactly this content. It is a source scan like `tests/layout.test.ts`: it walks `src/**/*.{ts,svelte}` with `node:fs`, applies the two regexes from spec section 6 outside `src/fs/fat16/**` and `src/lib/wasm.ts`, and allows imports from `fs/fat16` only in `src/fs/index.ts`, `src/fs/panels.ts`, and `src/scenarios/**`. Its second test is a set of positive controls, one per rule, so the scanner is proven to catch each kind of leak without anyone having to plant one in `src/`.

```ts
import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

// The adapter seam, enforced as a source scan (spec 2026-09-23-fs-adapter-design.md, section 6).
// Outside src/fs/fat16/ and src/lib/wasm.ts nothing may call a FAT-only wasm method or import a
// FAT-only wasm type, and only the registry, the panel table and the scenarios may import from
// fs/fat16. The scan is textual, like layout.test.ts: a comment that spells `vol.geometry()`
// trips it too, so reword the comment rather than the rule.
const SRC = fileURLToPath(new URL("../src/", import.meta.url));

/** Files the FAT-only wasm surface is allowed in. */
const FAT_ONLY_ALLOWED = (rel: string) => rel.startsWith("fs/fat16/") || rel === "lib/wasm.ts";
/** Files that may import from fs/fat16 (besides fs/fat16 itself). */
const FAT16_IMPORTERS = (rel: string) => rel === "fs/index.ts" || rel === "fs/panels.ts" || rel.startsWith("scenarios/");

const FAT_ONLY_CALL = /\.(geometry|fatEntries|clusterOwners|annotateSectorWith|rawDirEntries|bootSector|clusterChain|formatFat16)\s*\(/g;
const FAT_ONLY_TYPE = /\b(ClusterOwner|FatEntry|Geometry|RawEntry|BootSector|FormatOptions)\b/;
const WASM_IMPORT = /import\s+(?:type\s+)?\{([^}]*)\}\s+from\s+["'][^"']*lib\/wasm["']/g;
const FAT16_IMPORT = /from\s+["'](?:\.{1,2}\/)+(?:fs\/)?fat16(?:\/[^"']*)?["']/g;

/** Every .ts and .svelte file under `dir`, as posix paths relative to src/, sorted. */
function walk(dir: string, rel = ""): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const child = rel ? `${rel}/${entry.name}` : entry.name;
    if (entry.isDirectory()) out.push(...walk(join(dir, entry.name), child));
    else if (/\.(ts|svelte)$/.test(entry.name)) out.push(child);
  }
  return out.sort();
}

/** Every rule `text` breaks for a file at `rel` (posix path under src/), as "rel: what". */
export function violations(rel: string, text: string): string[] {
  const found: string[] = [];
  if (!FAT_ONLY_ALLOWED(rel)) {
    for (const m of text.matchAll(FAT_ONLY_CALL)) found.push(`${rel}: calls .${m[1]}( (FAT-only wasm method; go through the adapter)`);
    for (const m of text.matchAll(WASM_IMPORT)) {
      const t = FAT_ONLY_TYPE.exec(m[1]);
      if (t) found.push(`${rel}: imports ${t[1]} from lib/wasm (FAT-only wasm type; use the fs/adapter types)`);
    }
  }
  if (!rel.startsWith("fs/fat16/") && !FAT16_IMPORTERS(rel)) {
    for (const m of text.matchAll(FAT16_IMPORT)) found.push(`${rel}: imports from fs/fat16 (${m[0].trim()}); only fs/index.ts, fs/panels.ts and scenarios/ may`);
  }
  return found;
}

describe("the adapter boundary", () => {
  const files = walk(SRC);

  it("scans the tree it is meant to guard", () => {
    expect(files).toContain("App.svelte");
    expect(files).toContain("shell/commands.ts");
    expect(files).toContain("state/volume.svelte.ts");
    expect(files).toContain("fs/fat16/adapter.ts");
    expect(files.length).toBeGreaterThan(50);
  });

  it("recognises each kind of leak (positive controls)", () => {
    expect(violations("state/volume.svelte.ts", "const g = vol.geometry();")).toEqual([
      "state/volume.svelte.ts: calls .geometry( (FAT-only wasm method; go through the adapter)",
    ]);
    expect(violations("shell/host.ts", 'import { Volume, type FormatOptions } from "../lib/wasm";')).toEqual([
      "shell/host.ts: imports FormatOptions from lib/wasm (FAT-only wasm type; use the fs/adapter types)",
    ]);
    expect(violations("core/tree.ts", 'import type {\n  ClusterOwner,\n  Volume,\n} from "../lib/wasm";')).toHaveLength(1);
    expect(violations("components/Inspector.svelte", 'import { asFat16 } from "../fs/fat16";')).toEqual([
      'components/Inspector.svelte: imports from fs/fat16 (from "../fs/fat16"); only fs/index.ts, fs/panels.ts and scenarios/ may',
    ]);
    expect(violations("state/layers.svelte.ts", 'import { buildChain } from "../fs/fat16/fatchain";')).toHaveLength(1);
    // The allowed places, and the generic surface, pass.
    expect(violations("fs/fat16/adapter.ts", 'this.geo = vol.geometry(); import type { Geometry } from "../../lib/wasm";')).toEqual([]);
    expect(violations("lib/wasm.ts", 'export type { Geometry } from "fs-emulator-wasm";')).toEqual([]);
    expect(violations("fs/index.ts", 'import { fat16 } from "./fat16";')).toEqual([]);
    expect(violations("fs/panels.ts", 'import FatMap from "./fat16/FatMap.svelte";')).toEqual([]);
    expect(violations("scenarios/fundamentals.ts", 'import { asFat16 } from "../fs/fat16";')).toEqual([]);
    expect(violations("shell/commands.ts", "vol.annotateSector(3); vol.layout(); vol.corruption(); vol.fsType();")).toEqual([]);
    expect(violations("state/volume.svelte.ts", 'import { Volume, type FsError, type OpRecord, type Region } from "../lib/wasm";')).toEqual([]);
  });

  it("keeps FAT-only wasm calls, FAT-only wasm types and fs/fat16 imports out of the generic code", () => {
    const found = files.flatMap((rel) => violations(rel, readFileSync(join(SRC, ...rel.split("/")), "utf8")));
    expect(found).toEqual([]);
  });
});
```

Notes on the rules, so nobody loosens them by accident:

- `FAT_ONLY_CALL` is the spec's regex verbatim. It matches a member call, so the generic `vol.annotateSector(`, `vol.layout(`, `vol.corruption(` and `vol.fsType(` pass, and a doc comment that says "the one place `Volume.formatFat16` is called" passes (no parenthesis), but `vol.geometry()` in a comment does not: reword the comment.
- `WASM_IMPORT` captures the brace list of any `import { ... } from "<anything>lib/wasm"` (with or without `type`, on one line or several) and `FAT_ONLY_TYPE` looks for a banned name in it as a whole word, so `AddrGeometry` or `Fat16FormatOptions` would not match but `type FormatOptions` does.
- `FAT16_IMPORT` matches `./fat16`, `../fs/fat16`, `../../fs/fat16/format` and `./fat16/FatMap.svelte`: any relative import whose path ends in a `fat16` segment or goes through one.
- Scope is `src/` only. Tests may still call `vol.geometry()` or import `FormatOptions` to compute expectations.

- [ ] **Step 10: Run the boundary test**

Run: `cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm exec vitest run tests/adapterBoundary.test.ts`

Expected:

```
 ✓ tests/adapterBoundary.test.ts (3 tests)
 Test Files  1 passed (1)
      Tests  3 passed (3)
```

There is no red-first run for the third test: it guards work Tasks 2 to 6 have already finished, and the positive controls are the proof it can fail (on the tree at HEAD `81b0b56`, before the migration, the same scan lists 61 leaks: every `.geometry(` in `state/volume.svelte.ts`, `shell/commands.ts` and the nine scenarios, every `FormatOptions` import in the shell, and so on).

If the third test fails, its assertion prints every offender as `<file>: <what>`. Each one is a miss from Tasks 2 to 6, not something to allow-list: a `.geometry(`/`.fatEntries(`/`.clusterOwners(`/`.annotateSectorWith(` call becomes a `volume.adapter.*` or `host.adapter.*` call (spec section 3 names the method for every site); a `FormatOptions` import outside the shell's `mkfs` becomes `unknown` options passed through `host.format(host.adapter.id, options)` or `volume.format(family, options)`; a `Geometry`/`ClusterOwner`/`FatEntry` type import becomes `UnitSpace`/`UnitOwner` from `../fs/adapter`; an import from `fs/fat16` outside the three allowed places means the file wanted a FAT fact and must ask the adapter for it instead. Fix at the site, rerun, and only then continue.

- [ ] **Step 11: Rewrite the UI README's title, description, test comment, and the filesystem-specific section**

Four edits in `web/ui/README.md`.

(a) Line 1. Replace:

```md
# FAT explorer UI
```

with:

```md
# fs explorer UI
```

(b) Lines 3–4 (the first two lines of the opening paragraph; line 5 onward, `cluster map for seeing where files land, ...`, is unchanged). Replace:

```md
A Svelte 5 (runes) app for exploring a FAT16 volume byte by byte: a
whole-disk hex dump with ASCII and a strings overlay, a disk ribbon and FAT
```

with:

```md
A Svelte 5 (runes) app for exploring a filesystem byte by byte, FAT16 today:
a whole-disk hex dump with ASCII and a strings overlay, a disk ribbon and FAT
```

(c) Line 27. Replace:

```md
pnpm test      # vitest over src/core and src/shell (loads the real wasm package)
```

with:

```md
pnpm test      # vitest over src/core, src/fs and src/shell (loads the real wasm package)
```

(d) Lines 224–235, the whole "What is FAT-specific" section, from its heading up to (not including) the blank line before `## Keyboard shortcuts`. Replace:

```md
## What is FAT-specific

The app is the FAT explorer today, but most of it does not know what FAT is.
The hex dump, ribbon, byte attribution, zero-run collapsing, strings overlay,
timeline, and diff replay read `layout()` regions and the operation journal
from `fs-emulator-wasm`, so a FAT32 or ext2 volume gets all of them
unchanged. The FAT-specific pieces are the FAT map, the entry → chain → data
trace in the inspector, the cluster labels in the dump, the Format panel's
geometry fields, and the scenarios. Adding a filesystem means new region
kinds and colors in `src/core/attribution.ts`, a map panel and inspector
section for its structures, and scenarios that teach what is different about
it. `docs/ROADMAP.md` has the checklist.
```

with:

```md
## What is filesystem-specific

The app is the fs explorer: one explorer for every filesystem family the wasm
package can hold, FAT16 today. The hex dump, ribbon, byte attribution,
zero-run collapsing, strings overlay, timeline, diff replay, and terminal read
`layout()` regions and the operation journal from `fs-emulator-wasm`, so a
second family gets all of them unchanged. Everything the UI knows about one
family lives behind the adapter in `src/fs/`:

- `src/fs/adapter.ts` declares the seam. `UnitSpace` is pure allocation-unit
  arithmetic over one geometry: the unit noun, plural, and shell letter
  (`cluster`, `clusters`, `c` for FAT), the unit count and size, sector to
  unit and unit to byte range, the dump's row-label rule, and the region
  colours. `FsAdapter` is bound to one `Volume` and adds what needs the disk:
  owners, chains, entry slots, remnants, `stat` and `df` facts, the
  Inspector's trace, sector annotations, extra address forms, name matching,
  the "this write may have moved regions" trigger, and the notes the tree and
  the shell print. Its caches are plain fields that the store refreshes after
  every operation, so a `$derived` that calls an adapter method reads
  `volume.epoch` first.
- `src/fs/fat16/` is the FAT16 implementation: `Fat16Adapter`, the cluster
  geometry, the FAT chain walk, the directory-entry and remnant scans, the
  boot-sector trigger, the Format model (`clusterCountFor`, `checkFormat`,
  the `mkfs` flags), `FatMap.svelte`, and `FormatForm.svelte`. It is the only
  place, besides the type facade `src/lib/wasm.ts`, that calls the FAT-only
  wasm methods (`geometry`, `fatEntries`, `clusterOwners`,
  `annotateSectorWith`, `rawDirEntries`, `bootSector`, `clusterChain`,
  `formatFat16`) or names their types.
- `src/fs/index.ts` is the registry: `FAMILIES`, `DEFAULT_FAMILY`,
  `familyIdOf(fsType)`, and `adapterFor(vol)`, which picks the adapter from
  `Volume.fsType()`. `src/fs/panels.ts` maps a family id to its map and
  Format panels, and `App.svelte` and `ActionsPanel.svelte` render whichever
  the mounted family names. The panels sit in that table rather than on the
  adapter because the node tests have no Svelte plugin and the adapter must
  stay importable from them.
- The scenarios keep their FAT16 copy (they teach FAT16) and declare
  `family: "fat16"`; a step reaches generic facts through the adapter it is
  handed and FAT-only ones through `asFat16(fs)`.

`tests/adapterBoundary.test.ts` keeps it that way: it scans
`src/**/*.{ts,svelte}` and fails on a FAT-only wasm call or type outside
`src/fs/fat16/` and `src/lib/wasm.ts`, and on an import from `fs/fat16`
anywhere but `src/fs/index.ts`, `src/fs/panels.ts`, and `src/scenarios/`.
Adding a family means an adapter under `src/fs/<family>/`, an entry in
`fs/index.ts` and `fs/panels.ts`, a map panel in its own words ("FAT map"
stays; ext will get "Block groups"), and scenarios that teach what is
different about it. `docs/ROADMAP.md` has the checklist.
```

Every pane description in the "Panes" section stays as it is: the FAT map heading is still "FAT map", the Inspector still lists offset, sector, cluster, and owner, and `stat`, `df`, and `seek c:2` in the Terminal table print exactly what they printed before.

- [ ] **Step 12: Check the roadmap for Task 1's edits, then fix the paths it does not cover**

First verify Task 1's section-4 edits are present. Run:

```
cd /Users/bsmall/dev/fs-emulator && grep -n "FsAdapter\|source-scan\|fatEntryOffset\|adapterBoundary" docs/ROADMAP.md; grep -n "Detection" crates/wasm/README.md
```

Expected: `docs/ROADMAP.md` prints an `FsAdapter` hit inside the "What stays fixed" list and another inside checklist step 5 ("implement `FsAdapter` under `web/ui/src/fs/<family>/`, register it in `fs/index.ts` and `fs/panels.ts`, add its signature to `detect`"); it prints no `fatEntryOffset` hit (Task 1 deleted the deferred note). `crates/wasm/README.md` prints a `Detection` hit. If any of these is missing, it is a Task 1 miss: apply the spec's section 4, items 5 and 6, before continuing.

Then one more check and two edits in `docs/ROADMAP.md`.

(a) Confirm the adapter bullet under "What stays fixed" names the guard test. That bullet is Task 1's (spec section 4, item 6; Task 1 Step 13 writes its final wording, which names `web/ui/tests/adapterBoundary.test.ts`), so this task does not edit it. Run:

```
cd /Users/bsmall/dev/fs-emulator && grep -n "adapterBoundary.test.ts" docs/ROADMAP.md
```

Expected: exactly one hit, inside the "What stays fixed" list, on the bullet that begins **Per-family UI knowledge lives behind `FsAdapter`.** (it follows the "**The UI is region-driven.**" bullet). No output means Task 1 Step 13 was skipped or run with an older wording: that is a Task 1 miss, fixed in that bullet (not by adding a second one here), and the grep must print its hit before you continue.

(b) Lines 54–56 at HEAD, the last bullet of "Next: FAT32 in `crates/fat`". Replace:

```md
- UI: the FAT map and chain tracing work unchanged; the Format panel needs a
  FAT32 option, and its JavaScript copy of the cluster-count formula must
  follow the Rust one (or be replaced by a wasm call).
```

with:

```md
- UI: the FAT map and chain tracing work unchanged; the Format panel needs a
  FAT32 option, and `clusterCountFor` in `web/ui/src/fs/fat16/format.ts`, the
  JavaScript copy of the cluster-count formula, must follow the Rust one (or
  be replaced by a wasm call).
```

(c) Lines 93–96 at HEAD, the end of the deferred `web/ui` paragraph. Replace:

```md
minimum region width, so tiny regions can vanish at narrow widths;
`src/core/direntry.ts` and `src/core/remnants.ts` keep their pre-existing
blanket `catch` around `rawDirEntries` on purpose, so a corrupt volume falls
back to "no range" or "skip this entry" instead of throwing.
```

with:

```md
minimum region width, so tiny regions can vanish at narrow widths;
`src/fs/fat16/direntry.ts` and `src/fs/fat16/remnants.ts` keep their
pre-existing blanket `catch` around `rawDirEntries` on purpose, so a corrupt
volume falls back to "no range" or "skip this entry" instead of throwing.
```

- [ ] **Step 13: Point the root README at the adapter and the new spec**

Two edits in `README.md` (repository root).

(a) Lines 31–36, the paragraph after the layer diagram. Replace:

```md
Each layer only depends on the one below it. The UI programs against the
`FileSystem` trait and the region and annotation types in `fs-core`, plus a
small set of filesystem-specific inspection calls it uses only when
`fsType()` says they apply. Adding a filesystem means a new crate, a new
variant in the wasm `Volume`, and new panels and scenarios in the UI; nothing
above `fs-core` needs to change shape.
```

with:

```md
Each layer only depends on the one below it. The UI programs against the
`FileSystem` trait and the region and annotation types in `fs-core`, plus a
small set of filesystem-specific inspection calls that only the family's
adapter under `web/ui/src/fs/<family>/` makes, chosen from `fsType()`. Adding
a filesystem means a new crate, a new variant in the wasm `Volume`, and an
adapter, panels, and scenarios in the UI; nothing above `fs-core` needs to
change shape.
```

(b) Lines 115–119, the specs list under "Documentation". Replace:

```md
  `2026-09-21-fat16-emulator-design.md` (core and FAT16, including the FAT32
  seams), `2026-09-21-wasm-wrapper-design.md`,
  `2026-09-21-fat-explorer-ui-design.md`, and
  `2026-09-22-terminal-access-design.md` (raw writes and the terminal drawer).
```

with:

```md
  `2026-09-21-fat16-emulator-design.md` (core and FAT16, including the FAT32
  seams), `2026-09-21-wasm-wrapper-design.md`,
  `2026-09-21-fat-explorer-ui-design.md`,
  `2026-09-22-terminal-access-design.md` (raw writes and the terminal drawer),
  and `2026-09-23-fs-adapter-design.md` (the "fs explorer" rename and the
  per-family `FsAdapter` seam in the UI).
```

- [ ] **Step 14: Run the Rust and wasm gates**

Run, from the repository root:

```
cargo fmt --all -- --check && cargo clippy --workspace --all-targets -- -D warnings && cargo test --workspace && cargo build --workspace --target wasm32-unknown-unknown && wasm-pack test --node crates/wasm && wasm-pack build crates/wasm --target bundler
```

Expected: `cargo fmt` prints nothing; clippy finishes with no warnings; `cargo test --workspace` passes every test, including Task 1's native `detect` test in `fs-emulator-wasm` and the one `--ignored` mount test left ignored; the wasm32 build finishes; `wasm-pack test --node` reports `test result: ok` for the boundary tests, `corruption_is_null_until_sector_zero_breaks_and_clears_when_repaired` among them; `wasm-pack build` rewrites `crates/wasm/pkg`. This task changed no Rust, so any failure here is a regression from an earlier task: stop and fix it there.

- [ ] **Step 15: Run the web gates**

Run:

```
cd /Users/bsmall/dev/fs-emulator/web/ui && pnpm install --frozen-lockfile && pnpm test && pnpm build
```

Expected: `pnpm install` reports the lockfile up to date (no new dependencies anywhere in this plan); `pnpm test` passes every file: the 34 files at HEAD `81b0b56` (268 tests), plus `tests/fs/fat16.test.ts`, `tests/fs/registry.test.ts`, and `tests/fs/fat16-format.test.ts` from Task 2, plus `tests/adapterBoundary.test.ts`; `pnpm build` runs `svelte-check` with `0 errors` (it type-checks `tests/` as well, so the new test file is covered) and `vite build` emits `dist/`.

- [ ] **Step 16: Confirm the old name is gone from everything that ships**

Run:

```
cd /Users/bsmall/dev/fs-emulator && grep -rn "FAT explorer" web/ui/src web/ui/tests web/ui/index.html web/ui/README.md .github README.md docs/ROADMAP.md crates; echo "exit=$?"
```

Expected: no output and `exit=1`. (The hits under `docs/superpowers/plans/` stay by design and are not in the search set.)

- [ ] **Step 17: Browser pass**

Start the dev server with the `ui` launch configuration (`.claude/launch.json`: `pnpm --dir web/ui dev --port 5174 --strictPort`) and open `http://localhost:5174/`. The UI never calls `setNow`, so every timestamp below is the FAT epoch, `1980-01-01 00:00:00`. Work through every item; each is pass/fail, and a failing item is fixed at its site before the commit.

1. **Naming.** The browser tab reads "fs explorer" (`document.title` in the console is `"fs explorer"`); the top-bar h1 reads "fs explorer"; the left column's map panel heading still reads `FAT map · 8,167 clusters`; the ribbon's free label reads `16.0 MB free`; the Files panel shows the empty root.

2. **Add.** In Actions, keep the defaults (Path `/Hello world.txt`, Content `Hello from the browser`, 22 bytes) and click **Add file**. The timeline gains `create_file /Hello world.txt`; the Files tree lists `Hello world.txt` with `22 B · first cluster 2`; the ribbon legend gains the file in its hue and the free label reads `15.9 MB free`; the dump row at `0000c200` is labelled `cluster 2 · /Hello world.txt` and its ASCII gutter shows the text; the FAT map's first cell is filled in the file's hue, and hovering it gives a caption of the form `cluster 2 · … · /Hello world.txt`. Click the file in the tree: the Inspector's "Selected file" trace shows three rows, as today: `Directory entry · offset 0x8200`, `FAT chain · 1 clusters starting at 2 · FAT entry at 0x204`, `Data · cluster 2 at 0xc200`. Hover a byte at `0xc200`: the Inspector reads `Cluster` `2 · FAT: end of chain` and `Owner` `/Hello world.txt`.

3. **Overwrite.** Change Content to `Hello again from the browser` (28 bytes) and click **Overwrite**. The timeline gains `write_file /Hello world.txt`; the tree reads `28 B · first cluster 2`; the dump row at `0000c200` shows the new text; the trace still says `1 clusters starting at 2`. (The chain is rewritten in place, as the "Overwrite with a bigger file" scenario's copy relies on.)

4. **Delete.** Click **Delete**. The timeline gains `delete_file /Hello world.txt`; the tree no longer lists the file; the ribbon legend drops it and the free label is back to `16.0 MB free`; the FAT map's first cell is free again; the Inspector's "Selected file" section is gone. Tick **Show remnants**: the three deleted slots at `0x8200` are hatched in the dump. This item and items 2 and 3 are the reactivity rule at work: owners, chain, and attribution all follow `volume.epoch` through the plain adapter caches.

5. **Format from the form.** Expand **Format**; choose Size `16 MB`, Sectors per cluster `8`: the line under the selects reads `4,087 clusters` with no warning. Type Volume label `FORMTEST` and click **Format disk**. The timeline is empty; the map heading reads `FAT map · 4,087 clusters`; hover byte `0x2b` (offset 43): the boot-sector annotation shows the label `FORMTEST`. Open the terminal (backtick) and type `df`: one row with `filesystem /dev/hda`, `mounted /mnt`, `type FAT16`, `clusterSize 4096`, `clusters 4087`, `used 0`, `free 4087`, `bytesUsed 0`, `bytesFree 16740352`, `use 0%`.

6. **Format from `mkfs`.** Type `mkfs --spc 4 --label SHELLDISK`. It prints `formatted /dev/hda as FAT16; the timeline was cleared`, the prompt reads `/mnt ❯`, the map heading is back to `FAT map · 8,167 clusters`, and `df` prints `clusterSize 2048`, `clusters 8167`, `used 0`, `free 8167`, `bytesUsed 0`, `bytesFree 16726016`, `use 0%`. `mkfs --help` lists the six flags `--sectors`, `--spc`, `--label`, `--root-entries`, `--fats`, `--reserved` with their descriptions as today (`total sectors (default 32768 = 16 MB)`, `sectors per cluster (default 4)`, `volume label, up to 11 characters`, `root directory entries (default 512)`, `FAT copies (default 2)`, `reserved sectors (default 1)`).

7. **Wipe sector 0 with `dd`, see the note, repair.** On the fresh disk from item 6: `echo hi | write /mnt/A.TXT` prints `2 bytes -> /mnt/A.TXT`. Stash the boot sector in the last sector: `dd if=/dev/hda bs=512 count=1 of=/dev/hda seek=32767` prints `1+0 records in`, `1+0 records out`, `512 bytes copied`, and the timeline gains `write_raw 0xfffe00 +512`. Wipe: `dd if=/dev/zero of=/dev/hda count=1` prints the same three lines and the timeline gains `write_raw 0x0 +512`. Now the Files panel shows `Boot sector does not parse; the tree is unavailable until a raw write repairs it.` and the status line shows `Volume not mounted: corrupt image: boot sector no longer parses after a raw write: boot sector signature is not 55 AA`; `mount` reports state `corrupt`; `ls /mnt` fails with an error line ending in `boot sector signature is not 55 AA` and a help line ending in `dd --of=/dev/hda`; the dump still renders (sector 0 as zeros) and the ribbon still paints. Repair: `dd if=/dev/hda bs=512 skip=32767 count=1 of=/dev/hda` prints the three `dd` lines and the timeline gains a second `write_raw 0x0 +512`; the note and the status message disappear; `mount` reports `ok`; `ls /mnt` lists `A.TXT`; the tree shows `A.TXT` with `2 B · first cluster 2`.

8. **The fundamentals, end to end.** Pick "The fundamentals" in the top bar and press **Start** (the disk is formatted afresh, the timeline is empty). Press **Next** through all twelve steps and check the Lesson card's "Look at:" line at each; the strings are `describeFocus`'s, pinned by `tests/lesson.test.ts` and `tests/fundamentals.test.ts`:

   | step | timeline | Look at: |
   |---|---|---|
   | 1 One disk, five regions | `create_file /HELLO.TXT` | `Sector 0, reserved (boot sector)` |
   | 2 Sector 0 describes the rest | | `Offset 0xb in reserved (boot sector)` |
   | 3 Where each region starts | | `Sector 65, root directory` |
   | 4 The FAT: one entry per cluster | | `Offset 0x204 in FAT 0` |
   | 5 FAT 1 is a mirror | | `Offset 0x4204 in FAT 1` |
   | 6 The root directory names the file | | `Files: /HELLO.TXT, its entry, chain, and clusters; offset 0x8200 in root directory` |
   | 7 Following the links | | `Files: /HELLO.TXT, its entry, chain, and clusters; cluster 2 in the data region (offset 0xc200)` |
   | 8 A larger file chains clusters | `create_file /BIGGER.TXT` | `Files: /BIGGER.TXT, its entry, chain, and clusters` (the FAT map draws the chain 3 → 4 → 5) |
   | 9 The chain in the table | | `Files: /BIGGER.TXT, its entry, chain, and clusters; offset 0x206 in FAT 0` |
   | 10 Size versus space | | `Files: /BIGGER.TXT, its entry, chain, and clusters; cluster 5 in the data region (offset 0xda00)` |
   | 11 Free means zero in the table | | `Offset 0x20c in FAT 0` |
   | 12 Putting it together | | `Sector 0, reserved (boot sector)`; the button reads **Finish** |

   On step 8 press **Prev** then **Next**: the timeline stays at two entries (a step never runs twice). Press **Finish**.

9. **Work from the shell, end to end.** Pick "Work from the shell", press **Start**, open the terminal. Step 1: `ls /mnt` prints an empty listing, `pwd` prints `/mnt`. Press **Next** (step 2 runs `create_file /HELLO.TXT`; Look at: `Files: /HELLO.TXT, its entry, chain, and clusters`), then type, and compare with exactly today's output:
   - `cat /mnt/HELLO.TXT` → `Hello from the shell`
   - `stat /mnt/HELLO.TXT` → `path /mnt/HELLO.TXT`, `name HELLO.TXT`, `type file`, `size 20`, `created 1980-01-01 00:00:00`, `modified 1980-01-01 00:00:00`, `accessed 1980-01-01 00:00:00`, `firstCluster 2`, `chain [2]`, `clusters 1`, `entryOffset 0x8200`, `entrySlots 0x8200-0x8220`, `fatEntryOffset 0x204`, `dataOffset 0xc200`
   - `df` → `filesystem /dev/hda`, `mounted /mnt`, `type FAT16`, `clusterSize 2048`, `clusters 8167`, `used 1`, `free 8166`, `bytesUsed 2048`, `bytesFree 16723968`, `use 0%`
   - `seek c:2` → prints nothing; the dump scrolls to the row `0000c200`, labelled `cluster 2 · /HELLO.TXT`
   - `seek nope` → error `bad address 'nope'` with the help `addresses: 0x1f (hex), 512 (decimal), s:65 (sector), c:3 (cluster)`
   - `xxd /dev/hda --offset s:65 --len 32` → exactly these two lines:
     ```
     00008200: 4845 4c4c 4f20 2020 5458 5420 0000 0000  HELLO   TXT ....
     00008210: 2100 2100 0000 0000 2100 0200 1400 0000  !.!.....!.......
     ```
   Press **Next** (step 3; Look at: `Cluster 2 in the data region (offset 0xc200)`). **Next** (step 4 runs `create_dir /DOCS`; Look at: `Files: /DOCS, its entry, chain, and clusters`); `xxd /dev/hda --offset c:3 --len 64` prints exactly:
     ```
     0000ca00: 2e20 2020 2020 2020 2020 2010 0000 0000  .          .....
     0000ca10: 2100 2100 0000 0000 2100 0300 0000 0000  !.!.....!.......
     0000ca20: 2e2e 2020 2020 2020 2020 2010 0000 0000  ..         .....
     0000ca30: 2100 2100 0000 0000 2100 0000 0000 0000  !.!.....!.......
     ```
   **Next** (step 5 runs `create_file /DOCS/COPY.TXT`; the tree shows `DOCS` › `COPY.TXT` with `20 B · first cluster 4`). **Next** (step 6; Look at: `Sector 0, reserved (boot sector)`); `dd if=/dev/hda bs=512 count=1 | xxd` prints 32 lines, the first beginning `00000000: eb3c 90` and the last, `000001f0: …`, ending in `55aa` / `U.`. **Next** (step 7 runs `write_raw 0x2b +11`; Look at: `Offset 0x2b in reserved (boot sector)`; hovering `0x2b` shows the boot-sector label annotation reading `SHELLDISK`). **Next** (step 8 runs `delete_file /HELLO.TXT`; Look at: `Offset 0x8200 in root directory; deleted entries are shown hatched`; the tree no longer lists `HELLO.TXT`, `DOCS/COPY.TXT` remains); `ls -l /mnt` prints one row, `DOCS`, `dir`, `0`, `1980-01-01 00:00:00`. Press **Finish**.

Record any failing item and fix it before committing; items 2 to 4 and 7 are where a missed `volume.epoch` read would show (a stale chain, a stale owner colour, or a tree that does not come back after the repair).

- [ ] **Step 18: Commit**

```
cd /Users/bsmall/dev/fs-emulator && git add web/ui/index.html web/ui/src/App.svelte .github/workflows/pages.yml web/ui/src/shell/commands.ts web/ui/tests/shell/mutations.test.ts web/ui/tests/adapterBoundary.test.ts web/ui/README.md docs/ROADMAP.md README.md && git commit -m "chore(ui): rename the app to fs explorer, guard the adapter boundary, update the docs"
```

(`git add web/ui/src/App.svelte` and `git add web/ui/src/shell/commands.ts` are no-ops when Tasks 5 and 4 already renamed those strings.)
