# ext Explorer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Teach the fs explorer the ext family: one adapter under `web/ui/src/fs/ext/` bound to ext2 and ext3 volumes, a block-group map in place of the FAT map, an inode-aware Inspector, a Journal panel with the crash and recovery controls, the terminal's ext vocabulary with `crash`, `recover`, and `mkfs --type`, three ext3 lessons, and the ext-only wasm DTOs they need, with FAT16 unchanged and still the default volume.

**Architecture:** The wasm crate gains seven ext-only inspection methods (`extGeometry`, `extSuperblock`, `blockOwners`, `inodeNumber`, `extInode`, `dirEntries`, `fileBlocks`). The adapter seam gains what a second family needs and FAT does not: a sector noun beside the unit noun, a `fileParts` clause, owner roles, a family's `fsTypes`, an optional journal capability, and `extras` panels. `ExtAdapter` implements the seam over the new DTOs, with journal blocks never treated as units and indirect blocks as owner rows; the Journal panel and the `crash`/`recover` commands exist whenever the mounted adapter has the capability. The terminal re-registers its commands when the mounted family changes. Lessons run on the default ext3 disk and pin their numbers by test.

**Tech Stack:** Rust 1.87 workspace (wasm via wasm-bindgen + serde-wasm-bindgen), Svelte 5 runes + TypeScript strict, Vite 6, Vitest 3 in node loading the real wasm package, pnpm 12 (any pnpm 10.26 or newer works), `@benjamin-small/browser-terminal@0.3.0` (exact; `registerCommand` and `unregisterCommand`).

**Spec:** `docs/superpowers/specs/2026-09-24-ext-explorer-design.md` (binding), with `docs/superpowers/specs/2026-09-23-fs-adapter-design.md` binding for the seam except where the new spec's section 2 amends it. The shared interface contract the tasks were written against is reproduced in each task's Interfaces block.

## Global Constraints

1. FAT16 behaviour, every shell output string, every lesson string, and every attribution colour on FAT stay identical; every existing FAT test passes with no FAT string, number, or colour changed in any expectation. The only sanctioned edits are shape-only ones a new required field forces (an object literal in a `toEqual`, a stub that must carry `sector` or `fileParts`, the removed `MkfsSpec.done` and `MkfsSpec.summary` pins, a test stub gaining a no-op `setArmedPhase`, the scenario-count pin rephrased around the nine FAT lessons), Task 6's `mkfs --label` description in `tests/shell/help-text.test.ts` becoming `volume label (fat16: up to 11 characters; ext: up to 16 bytes)`, and the spec's sanctioned FAT-visible changes (a FAT `Unsupported` load shows the wasm text; `mkfs` has one summary and a `--type` flag; the `describeRange` tooltip takes a noun that defaults to "sector"). FAT16 stays `DEFAULT_FAMILY`.
2. Names, signatures, file paths, strings, and DTO shapes are the spec's and the contract's, verbatim. The ext-only wasm surface is called only under `src/fs/ext/**` and `src/lib/wasm.ts`; `fs/ext` is imported only by `fs/index.ts`, `fs/panels.ts`, and `scenarios/**`; `tests/adapterBoundary.test.ts` enforces both.
3. Reactivity rule: adapter caches are plain fields; every `$derived` or `$effect` that calls an adapter method reads `volume.epoch` first. The Journal panel memoises `journal.blocks()` per epoch.
4. Journal blocks are never units; indirect blocks are owner rows with `role: "indirect"`; on ext the sector noun is `block` and the unit row collapses into the address row.
5. `crates/fs-core`, `crates/fat`, and `crates/ext` are not changed by this plan; `crates/wasm` changes only in Task 1. `cargo fmt --all -- --check` and `cargo clippy --workspace --all-targets -- -D warnings` pass at every commit; no `#[allow]`, no `unwrap`/`expect` outside tests.
6. `web/ui`: Svelte 5 runes, TypeScript strict with `verbatimModuleSyntax`, Vitest in the node environment loading the real wasm package (nothing a node test imports may import a `.svelte` file); no new dependencies; `pnpm test` and `pnpm build` green at every task's commit, `web/demo` builds at Task 1's and Task 7's.
7. After any wasm change: `wasm-pack build crates/wasm --target bundler` then `CI=true pnpm install --frozen-lockfile` in `web/ui` before `pnpm test`. Any pnpm 10.26 or newer works (nvm's pnpm 12 and Homebrew's 10.33 are both on this machine): each web package keeps its pnpm settings in a tracked `pnpm-workspace.yaml`, and a frozen install must leave `pnpm-lock.yaml` and `pnpm-workspace.yaml` unmodified. `CI=true` lets pnpm purge a modules directory another pnpm wrote without asking.
8. Gates by task: Task 1 the Rust gates plus `wasm-pack test --node crates/wasm`, `wasm-pack build`, and the `web/ui` and `web/demo` builds; Tasks 2 to 7 the web gates; Task 7 ends with the browser pass of spec section 8.
9. One conventional commit per task, no attribution lines. Specs are binding: where task text and spec differ, the spec wins.
10. Reading the tasks: line ranges in a task's **Files** block refer to that task's own commit on its writer's branch; later tasks shift them, so use the anchor text. Test counts in **Expected** lines were observed on the task's base branch; when Tasks 5 and 6 run in sequence the second sees the first's tests too. The fix rounds inserted steps into Tasks 1, 2, 3, 4, and 6 (Task 2 Rounds 5 and 6, Task 3 Round 6, Task 1 Step 26); every Expected count was recomputed in sequential order after them, so the counts printed are the sequential ones.

---

---

### Task 1: the ext-only wasm DTOs

Spec: `docs/superpowers/specs/2026-09-24-ext-explorer-design.md` section 3 (the seven methods and their DTOs), section 4 (how the adapter consumes them: `refresh()` reads `extGeometry`, `extSuperblock`, `blockOwners`; `chain` reads `fileBlocks(path).data`; `entrySlots` and `parseAddr("i:N")` read `extInode(N).slot`; `trace` reads `dirEntries` and `fileBlocks(path).indirect`), section 8 (one wasm-pack test per method, ext3 and FAT `NotExt`), section 9 (`crates/wasm/README.md`: the new methods and DTOs).

Only `crates/wasm` changes; `crates/ext` already exposes every accessor this task needs (`ExtFs::{geometry, group_descriptors, superblock, block_owners, lookup, inode, dir_entries, disk}`, `ext::blockmap::{file_blocks, indirect_blocks}`, `Geometry::inode_location`), so no other crate is touched. Nothing under `web/` changes (Step 26 loads an `mke2fs` image through `web/ui` with a throwaway test file it deletes again).

**Files**

- Modify: `crates/wasm/src/dto.rs` (DTOs at lines 461-767, between `impl From<ext::JournalBlock> for JournalBlock` and `fn text`; native tests at lines 1567-1696, the end of `mod tests`)
- Modify: `crates/wasm/src/volume.rs` (the ext `impl Volume` doc comment at lines 487-492; the seven methods at lines 501-562, right after `block_group_count`)
- Modify: `crates/wasm/src/types.rs` (the ten TS declarations at lines 24-33, right after `JournalBlock`)
- Modify: `crates/wasm/README.md` (lines 101-225: the ext-only intro sentence, one bullet per method, the default-disk facts, and the TS block of the new DTOs, all between the `blockGroupCount()` bullet and the `armCrash(phase)` bullet; lines 347-350: the "Loading an ext2 image" note on the `mke2fs` recipe, which Step 26 checks)
- Test: `crates/wasm/tests/volume.rs` (lines 950-1378, appended after `journal_methods_throw_not_ext_on_fat`; the existing tests are unchanged)

**Interfaces**

Consumes (all existing):

```rust
// crates/ext
impl ExtFs {
    pub fn superblock(&self) -> &Superblock;
    pub fn geometry(&self) -> &Geometry;
    pub fn group_descriptors(&self) -> &[GroupDescriptor];
    pub fn inode(&self, ino: u32) -> fs_core::Result<Inode>;            // NotFound for 0 or past inodes_count
    pub fn lookup(&self, path: &str) -> fs_core::Result<u32>;           // through the corruption gate
    pub fn dir_entries(&self, path: &str) -> fs_core::Result<Vec<(u32, usize, DirEntry)>>; // (block, offset in block, entry)
    pub fn block_owners(&self) -> BTreeMap<u32, BlockOwner>;
    pub fn disk(&self) -> &fs_core::Disk;
}
impl Geometry { pub fn inode_location(&self, ino: u32) -> (u32, usize); } // (block, byte offset in the block)
pub fn ext::blockmap::file_blocks(disk: &Disk, inode: &Inode) -> Vec<u32>;
pub fn ext::blockmap::indirect_blocks(disk: &Disk, inode: &Inode) -> Vec<(u32, u8)>;
pub enum ext::BlockRole { Data, Directory, Indirect, Journal }
pub const ext::INODE_SIZE: u16; pub const ext::BLOCK_SIZE: u32; pub const ext::DEFAULT_UUID: [u8; 16];

// crates/wasm (private helpers in volume.rs and tests/volume.rs)
fn Volume::ext(&self) -> Result<&ExtFs, JsValue>;   // NotExt, "not an ext volume"
fn to_value<T: Serialize>(value: &T) -> Result<JsValue, JsValue>;   // serialize_missing_as_null
fn to_js(err: fs_core::Error) -> JsValue;
// tests/volume.rs: get, code, obj, message, fresh, fresh_ext, fresh_ext3, num, text_of
```

Produces (the generated `fs_emulator_wasm.d.ts`, `Volume` class):

```ts
extGeometry(): ExtGeometry;
extSuperblock(): ExtSuperblock;
blockOwners(): ExtBlockOwner[];
inodeNumber(path: string): number;
extInode(ino: number): ExtInode;
dirEntries(path: string): ExtDirEntry[];
fileBlocks(path: string): ExtFileBlocks;
```

and the declarations the `types.ts` string (`crates/wasm/src/types.rs`) appends to it:

```ts
export interface ExtGroup { index: number; firstBlock: number; blockCount: number; superblockBlock: number | null; descriptorsBlock: number | null; blockBitmap: number; inodeBitmap: number; inodeTable: number; firstData: number; freeBlocks: number; freeInodes: number; usedDirs: number; }
export interface ExtGeometry { blockSize: number; totalBlocks: number; firstDataBlock: number; blocksPerGroup: number; inodesPerGroup: number; inodesCount: number; inodeSize: number; inodeTableBlocks: number; descriptorBlocks: number; groups: ExtGroup[]; }
export interface ExtSuperblock { inodesCount: number; blocksCount: number; reservedBlocks: number; freeBlocks: number; freeInodes: number; firstDataBlock: number; logBlockSize: number; blocksPerGroup: number; inodesPerGroup: number; magic: number; state: number; revLevel: number; firstIno: number; inodeSize: number; featureCompat: number; featureIncompat: number; featureRoCompat: number; uuid: string; label: string; journalInum: number; defaultMountOpts: number; mtime: number; wtime: number; mntCount: number; }
export type ExtBlockOwnerRole = "data" | "directory" | "indirect" | "journal";
export interface ExtBlockOwner { block: number; inode: number; path: string; role: ExtBlockOwnerRole; }
export interface ExtInodeSlot { block: number; offset: number; }
export interface ExtInode { ino: number; mode: number; uid: number; gid: number; size: number; links: number; blocks: number; flags: number; atime: number; ctime: number; mtime: number; dtime: number; block: number[]; slot: ExtInodeSlot; }
export interface ExtDirEntry { block: number; offset: number; inode: number; recLen: number; nameLen: number; fileType: number; name: string; }
export interface ExtIndirectBlock { block: number; level: number; }
export interface ExtFileBlocks { data: number[]; indirect: ExtIndirectBlock[]; }
```

Rust DTOs in `fs_emulator_wasm::dto` (all `Serialize`, `rename_all = "camelCase"`): `ExtGroup`, `ExtGeometry` (`ExtGeometry::new(&ext::Geometry, &[ext::GroupDescriptor])`), `ExtSuperblock` (`From<&ext::Superblock>`), `ExtBlockOwner` (`ext_owners_to_list(&BTreeMap<u32, ext::BlockOwner>)`, `block_role_name(ext::BlockRole)`), `ExtInodeSlot`, `ExtInode` (`ExtInode::new(ino, &ext::Inode, &ext::Geometry)`), `ExtDirEntry` (`ExtDirEntry::new(block, offset_in_block, &ext::DirEntry)`), `ExtIndirectBlock`, `ExtFileBlocks` (`ExtFileBlocks::new(&fs_core::Disk, &ext::Inode)`), and the helpers `uuid_text(&[u8; 16]) -> String` and `nul_trimmed(&[u8]) -> String`.

Facts later tasks rely on (pinned by the tests below, default `Volume.formatExt3()` disk): `dirEntries` returns records in scan order (the directory's blocks in logical order, then by offset within each block), which is ascending by block for every directory this emulator writes; `blockOwners` rows for the journal's pointer blocks 1106..1110 are `{ inode: 8, path: "<journal>", role: "indirect" }` and for its data blocks 82..1105 `role: "journal"`; `ExtIndirectBlock.level` is `1` for the single-indirect block and for each second-level block under the double, `2` for the double-indirect block, in reference order (single, double, then its second-level blocks); `extInode(n).slot.offset` is `slot.block * 1024 + in-block offset`; `ExtInode.block` always has 15 entries.

- [ ] **Step 1: Confirm the starting point**

```sh
git status --short && git log --oneline -1
```

Expected: a clean tree on the feature branch (`feat/ext-explorer`, or the branch the executor was given), at the commit that holds the spec; nothing under `crates/wasm` modified yet.

#### Round 1: geometry and superblock

- [ ] **Step 2: Write the failing wasm-pack tests for `extGeometry` and `extSuperblock`**

Append to `crates/wasm/tests/volume.rs`, after `journal_methods_throw_not_ext_on_fat`. The fixture and helpers here are shared by the later rounds.

```rust
/// The default ext3 disk holding `/docs` (inode 12, block 1111),
/// `/hello.txt` (inode 13, 12 bytes in block 1112), and `/bigger.txt`
/// (inode 14, 13 data blocks 1113..=1125 and its single-indirect block
/// 1126): 16 blocks, 3 inodes, and one directory, all from group 0.
fn ext3_with_files() -> Volume {
    let mut v = fresh_ext3();
    v.create_dir("/docs").unwrap();
    v.create_file("/hello.txt", b"Hello, ext3!").unwrap();
    v.create_file("/bigger.txt", &[b'x'; 13312]).unwrap();
    v
}

/// Every element of a JS array of numbers.
fn nums(list: &JsValue) -> Vec<f64> {
    Array::from(list)
        .iter()
        .map(|n| n.as_f64().expect("a number"))
        .collect()
}

fn assert_nums(v: &JsValue, want: &[(&str, f64)]) {
    for (key, value) in want {
        assert_eq!(num(v, key), *value, "{key}");
    }
}

fn assert_not_ext(err: JsValue) {
    assert_eq!(code(err.clone()), "NotExt");
    assert_eq!(message(&err), "not an ext volume");
}

#[wasm_bindgen_test]
fn ext_geometry_merges_the_group_layout_with_the_descriptor_counts() {
    let geo = fresh_ext3().ext_geometry().unwrap();
    assert_nums(
        &geo,
        &[
            ("blockSize", 1024.0),
            ("totalBlocks", 16384.0),
            ("firstDataBlock", 1.0),
            ("blocksPerGroup", 8192.0),
            ("inodesPerGroup", 512.0),
            ("inodesCount", 1024.0),
            ("inodeSize", 128.0),
            ("inodeTableBlocks", 64.0),
            ("descriptorBlocks", 1.0),
        ],
    );
    let groups = Array::from(&get(&geo, "groups"));
    assert_eq!(groups.length(), 2);
    assert_nums(
        &groups.get(0),
        &[
            ("index", 0.0),
            ("firstBlock", 1.0),
            ("blockCount", 8192.0),
            ("superblockBlock", 1.0),
            ("descriptorsBlock", 2.0),
            ("blockBitmap", 3.0),
            ("inodeBitmap", 4.0),
            ("inodeTable", 5.0),
            ("firstData", 69.0),
            ("freeBlocks", 7082.0),
            ("freeInodes", 501.0),
            ("usedDirs", 2.0),
        ],
    );
    assert_nums(
        &groups.get(1),
        &[
            ("index", 1.0),
            ("firstBlock", 8193.0),
            ("blockCount", 8191.0),
            ("superblockBlock", 8193.0),
            ("descriptorsBlock", 8194.0),
            ("blockBitmap", 8195.0),
            ("inodeBitmap", 8196.0),
            ("inodeTable", 8197.0),
            ("firstData", 8261.0),
            ("freeBlocks", 8123.0),
            ("freeInodes", 512.0),
            ("usedDirs", 0.0),
        ],
    );

    // The counts come from the live descriptors.
    let busy = Array::from(&get(&ext3_with_files().ext_geometry().unwrap(), "groups")).get(0);
    assert_nums(
        &busy,
        &[
            ("freeBlocks", 7066.0),
            ("freeInodes", 498.0),
            ("usedDirs", 3.0),
        ],
    );

    // sparse_super: group 2 of a three-group disk has no superblock copy.
    let three = Volume::format_ext2(obj(&[("totalBlocks", JsValue::from(24576u32))])).unwrap();
    let g2 = Array::from(&get(&three.ext_geometry().unwrap(), "groups")).get(2);
    assert!(get(&g2, "superblockBlock").is_null());
    assert!(get(&g2, "descriptorsBlock").is_null());
    assert_nums(
        &g2,
        &[
            ("firstBlock", 16385.0),
            ("blockCount", 8191.0),
            ("blockBitmap", 16385.0),
            ("inodeBitmap", 16386.0),
            ("inodeTable", 16387.0),
        ],
    );

    assert_not_ext(fresh().ext_geometry().unwrap_err());
}

#[wasm_bindgen_test]
fn ext_superblock_reports_the_counts_uuid_and_label() {
    let sb = fresh_ext3().ext_superblock().unwrap();
    assert_nums(
        &sb,
        &[
            ("inodesCount", 1024.0),
            ("blocksCount", 16384.0),
            ("reservedBlocks", 0.0),
            ("freeBlocks", 15205.0),
            ("freeInodes", 1013.0),
            ("firstDataBlock", 1.0),
            ("logBlockSize", 0.0),
            ("blocksPerGroup", 8192.0),
            ("inodesPerGroup", 512.0),
            ("magic", 61267.0), // 0xEF53
            ("state", 1.0),
            ("revLevel", 1.0),
            ("firstIno", 11.0),
            ("inodeSize", 128.0),
            ("featureCompat", 4.0),   // has_journal
            ("featureIncompat", 2.0), // filetype
            ("featureRoCompat", 1.0), // sparse_super
            ("journalInum", 8.0),
            ("defaultMountOpts", 64.0), // ordered
            ("mtime", 0.0),
            ("wtime", 315532800.0), // 1980-01-01, the default clock
            ("mntCount", 0.0),
        ],
    );
    assert_eq!(text_of(&sb, "uuid"), "e2f5ee00-2026-4923-8000-000000000001");
    assert_eq!(text_of(&sb, "label"), "");

    // The counters follow the volume.
    let busy = ext3_with_files().ext_superblock().unwrap();
    assert_nums(&busy, &[("freeBlocks", 15189.0), ("freeInodes", 1010.0)]);

    // Options reach it; ext2 has no journal inode.
    let mut e = Volume::format_ext2(obj(&[
        ("label", JsValue::from_str("teach")),
        (
            "uuid",
            JsValue::from_str("0123456789ABCDEF0123456789abcdef"),
        ),
    ]))
    .unwrap();
    let sb = e.ext_superblock().unwrap();
    assert_eq!(text_of(&sb, "label"), "teach");
    assert_eq!(text_of(&sb, "uuid"), "01234567-89ab-cdef-0123-456789abcdef");
    assert_nums(&sb, &[("journalInum", 0.0), ("featureCompat", 0.0)]);

    // The label is lossy UTF-8: a raw 0xFF byte reads as U+FFFD.
    e.write_raw(1024 + 120, &[0xFF]).unwrap();
    assert_eq!(
        text_of(&e.ext_superblock().unwrap(), "label"),
        "\u{FFFD}each"
    );

    assert_not_ext(fresh().ext_superblock().unwrap_err());
}
```

- [ ] **Step 3: Write the failing native DTO tests**

Append inside `mod tests` at the end of `crates/wasm/src/dto.rs` (after `cluster_owner_lists_round_trip_sorted`):

```rust
    #[test]
    fn ext_uuids_print_hyphenated_and_labels_stop_at_the_first_nul() {
        assert_eq!(
            uuid_text(&ext::DEFAULT_UUID),
            "e2f5ee00-2026-4923-8000-000000000001"
        );
        assert_eq!(
            uuid_text(&[0xAB; 16]),
            "abababab-abab-abab-abab-abababababab"
        );
        assert_eq!(nul_trimmed(b"teach\0\0\0"), "teach");
        assert_eq!(nul_trimmed(b"a\0b"), "a");
        assert_eq!(nul_trimmed(b""), "");
        assert_eq!(nul_trimmed(b"sixteen-bytes!!!"), "sixteen-bytes!!!");
        assert_eq!(nul_trimmed(&[0xFF, b'x', 0]), "\u{FFFD}x");
    }

    #[test]
    fn ext_geometry_and_superblock_map_the_default_disk() {
        let fs = ext::ExtFs::format(ext::ExtFormatOptions::ext3()).unwrap();
        let geo = ExtGeometry::new(fs.geometry(), fs.group_descriptors());
        assert_eq!(
            (geo.total_blocks, geo.inodes_count, geo.inode_size),
            (16384, 1024, 128)
        );
        assert_eq!(
            geo.groups[0],
            ExtGroup {
                index: 0,
                first_block: 1,
                block_count: 8192,
                superblock_block: Some(1),
                descriptors_block: Some(2),
                block_bitmap: 3,
                inode_bitmap: 4,
                inode_table: 5,
                first_data: 69,
                free_blocks: 7082,
                free_inodes: 501,
                used_dirs: 2,
            }
        );
        let sb = ExtSuperblock::from(fs.superblock());
        assert_eq!((sb.free_blocks, sb.free_inodes), (15205, 1013));
        assert_eq!(sb.uuid, "e2f5ee00-2026-4923-8000-000000000001");
        assert_eq!((sb.label.as_str(), sb.journal_inum), ("", 8));
    }
```

- [ ] **Step 4: Run both and see them fail**

```sh
wasm-pack test --node crates/wasm 2>&1 | grep -E "^error" | sort | uniq -c
cargo test -p fs-emulator-wasm --lib 2>&1 | grep -E "^error" | sort | uniq -c
```

Expected: the wasm-pack run fails to compile with ``error[E0599]: no method named `ext_geometry` found for struct `Volume` in the current scope`` (4 times) and the same for `ext_superblock` (5 times); the native run fails to compile with `cannot find function` errors (E0425) for `uuid_text` and `nul_trimmed` and `cannot find ... type` errors (E0422/E0433) for `ExtGroup`, `ExtGeometry`, and `ExtSuperblock`.

- [ ] **Step 5: Add the geometry and superblock DTOs**

In `crates/wasm/src/dto.rs`, insert right after the `impl From<ext::JournalBlock> for JournalBlock { ... }` block and before `fn text(bytes: &[u8]) -> String`:

```rust
/// One block group of `extGeometry()`: where its structures live (from
/// `ext::GroupLayout`) and its counters (from the primary group
/// descriptor). `superblockBlock` and `descriptorsBlock` are `null` in a
/// group `sparse_super` gives no backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtGroup {
    pub index: u32,
    pub first_block: u32,
    pub block_count: u32,
    pub superblock_block: Option<u32>,
    pub descriptors_block: Option<u32>,
    pub block_bitmap: u32,
    pub inode_bitmap: u32,
    pub inode_table: u32,
    pub first_data: u32,
    pub free_blocks: u16,
    pub free_inodes: u16,
    pub used_dirs: u16,
}

/// `extGeometry()`: the numbers every ext panel derives from the superblock,
/// and each group's layout and counters.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtGeometry {
    pub block_size: u32,
    pub total_blocks: u32,
    pub first_data_block: u32,
    pub blocks_per_group: u32,
    pub inodes_per_group: u32,
    pub inodes_count: u32,
    pub inode_size: u16,
    pub inode_table_blocks: u32,
    pub descriptor_blocks: u32,
    pub groups: Vec<ExtGroup>,
}

impl ExtGeometry {
    /// Merge the layout with the primary descriptors, group by group.
    pub fn new(geo: &ext::Geometry, gds: &[ext::GroupDescriptor]) -> Self {
        let groups = geo
            .groups_layout
            .iter()
            .map(|g| {
                let gd = gds.get(g.index as usize).copied().unwrap_or_default();
                ExtGroup {
                    index: g.index,
                    first_block: g.first_block,
                    block_count: g.block_count,
                    superblock_block: g.superblock_block,
                    descriptors_block: g.descriptors_block,
                    block_bitmap: g.block_bitmap,
                    inode_bitmap: g.inode_bitmap,
                    inode_table: g.inode_table,
                    first_data: g.first_data,
                    free_blocks: gd.free_blocks_count,
                    free_inodes: gd.free_inodes_count,
                    used_dirs: gd.used_dirs_count,
                }
            })
            .collect();
        ExtGeometry {
            block_size: geo.block_size,
            total_blocks: geo.total_blocks,
            first_data_block: geo.first_data_block,
            blocks_per_group: geo.blocks_per_group,
            inodes_per_group: geo.inodes_per_group,
            inodes_count: geo.inodes_count,
            inode_size: ext::INODE_SIZE,
            inode_table_blocks: geo.inode_table_blocks,
            descriptor_blocks: geo.descriptor_blocks,
            groups,
        }
    }
}

/// The 16 UUID bytes as lower-case hex, hyphenated 8-4-4-4-12.
pub fn uuid_text(bytes: &[u8; 16]) -> String {
    let hex: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    format!(
        "{}-{}-{}-{}-{}",
        &hex[0..8],
        &hex[8..12],
        &hex[12..16],
        &hex[16..20],
        &hex[20..32]
    )
}

/// The bytes up to the first NUL, as lossy UTF-8.
pub fn nul_trimmed(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    String::from_utf8_lossy(&bytes[..end]).into_owned()
}

/// `extSuperblock()`: the primary superblock's fields the explorer shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtSuperblock {
    pub inodes_count: u32,
    pub blocks_count: u32,
    pub reserved_blocks: u32,
    pub free_blocks: u32,
    pub free_inodes: u32,
    pub first_data_block: u32,
    pub log_block_size: u32,
    pub blocks_per_group: u32,
    pub inodes_per_group: u32,
    pub magic: u16,
    pub state: u16,
    pub rev_level: u32,
    pub first_ino: u32,
    pub inode_size: u16,
    pub feature_compat: u32,
    pub feature_incompat: u32,
    pub feature_ro_compat: u32,
    pub uuid: String,
    pub label: String,
    pub journal_inum: u32,
    pub default_mount_opts: u32,
    pub mtime: u32,
    pub wtime: u32,
    pub mnt_count: u16,
}

impl From<&ext::Superblock> for ExtSuperblock {
    fn from(sb: &ext::Superblock) -> Self {
        ExtSuperblock {
            inodes_count: sb.inodes_count,
            blocks_count: sb.blocks_count,
            reserved_blocks: sb.r_blocks_count,
            free_blocks: sb.free_blocks_count,
            free_inodes: sb.free_inodes_count,
            first_data_block: sb.first_data_block,
            log_block_size: sb.log_block_size,
            blocks_per_group: sb.blocks_per_group,
            inodes_per_group: sb.inodes_per_group,
            magic: sb.magic,
            state: sb.state,
            rev_level: sb.rev_level,
            first_ino: sb.first_ino,
            inode_size: sb.inode_size,
            feature_compat: sb.feature_compat,
            feature_incompat: sb.feature_incompat,
            feature_ro_compat: sb.feature_ro_compat,
            uuid: uuid_text(&sb.uuid),
            label: nul_trimmed(&sb.volume_name),
            journal_inum: sb.journal_inum,
            default_mount_opts: sb.default_mount_opts,
            mtime: sb.mtime,
            wtime: sb.wtime,
            mnt_count: sb.mnt_count,
        }
    }
}
```

- [ ] **Step 6: Add `extGeometry` and `extSuperblock` to `Volume`**

In `crates/wasm/src/volume.rs`, replace the doc comment of the ext `impl Volume` block:

```rust
/// ext-specific methods. Each throws `code === "NotExt"` on a non-ext volume.
/// The superblock, inode, and block-ownership DTOs arrive with the explorer's
/// ext panels; for now there are the group count (slice 2) and the ext3
/// journal's crash, recovery, and inspection methods (slice 3, section 8).
/// The journal methods answer on ext2 too: no journal, no armed crash,
/// nothing to recover.
```

with:

```rust
/// ext-specific methods. Each throws `code === "NotExt"` on a non-ext volume.
/// The group count (slice 2), the ext3 journal's crash, recovery, and
/// inspection methods (slice 3, section 8), and the explorer's inspection
/// DTOs (slice 4, section 3): geometry, superblock, block owners, inodes,
/// directory entries, and a file's block map. The journal methods answer on
/// ext2 too: no journal, no armed crash, nothing to recover.
```

and insert, right after `block_group_count` in that block:

```rust
    /// The geometry and every group's layout, with the primary descriptors'
    /// free-block, free-inode, and directory counts.
    #[wasm_bindgen(js_name = extGeometry, unchecked_return_type = "ExtGeometry")]
    pub fn ext_geometry(&self) -> Result<JsValue, JsValue> {
        let fs = self.ext()?;
        to_value(&dto::ExtGeometry::new(
            fs.geometry(),
            fs.group_descriptors(),
        ))
    }

    /// The primary superblock as the volume holds it now.
    #[wasm_bindgen(js_name = extSuperblock, unchecked_return_type = "ExtSuperblock")]
    pub fn ext_superblock(&self) -> Result<JsValue, JsValue> {
        to_value(&dto::ExtSuperblock::from(self.ext()?.superblock()))
    }
```

- [ ] **Step 7: Run the tests and see them pass**

```sh
cargo fmt --all
wasm-pack test --node crates/wasm 2>&1 | grep -E "test result"
cargo test -p fs-emulator-wasm --lib 2>&1 | grep -E "test result"
```

Expected: `test result: ok. 23 passed; 0 failed` from wasm-pack (21 before) and `test result: ok. 26 passed; 0 failed` from the native lib tests (24 before).

#### Round 2: block owners and inodes

- [ ] **Step 8: Write the failing wasm-pack tests for `blockOwners`, `inodeNumber`, and `extInode`**

Append to `crates/wasm/tests/volume.rs`, after `ext_superblock_reports_the_counts_uuid_and_label`:

```rust
/// `(inode, path, role)` of one `ExtBlockOwner` row.
fn owner_of(row: &JsValue) -> (f64, String, String) {
    (
        num(row, "inode"),
        text_of(row, "path"),
        text_of(row, "role"),
    )
}

#[wasm_bindgen_test]
fn block_owners_ascend_by_block_and_name_their_role() {
    let rows: Vec<JsValue> = Array::from(&ext3_with_files().block_owners().unwrap())
        .iter()
        .collect();
    let blocks: Vec<f64> = rows.iter().map(|r| num(r, "block")).collect();
    assert!(blocks.windows(2).all(|w| w[0] < w[1]));
    // Root 1, lost+found 12, the journal 1024 + 5 pointer blocks, /docs 1,
    // /hello.txt 1, /bigger.txt 13 + 1.
    assert_eq!(rows.len(), 1058);
    assert_eq!((blocks[0], blocks[rows.len() - 1]), (69.0, 1126.0));
    let at = |block: f64| owner_of(&rows[blocks.iter().position(|&b| b == block).unwrap()]);
    let row = |inode: f64, path: &str, role: &str| (inode, path.to_string(), role.to_string());
    assert_eq!(at(69.0), row(2.0, "/", "directory"));
    assert_eq!(at(70.0), row(11.0, "/lost+found", "directory"));
    assert_eq!(at(81.0), row(11.0, "/lost+found", "directory"));
    assert_eq!(at(82.0), row(8.0, "<journal>", "journal"));
    assert_eq!(at(1105.0), row(8.0, "<journal>", "journal"));
    for pointer in 1106..=1110 {
        assert_eq!(at(pointer as f64), row(8.0, "<journal>", "indirect"));
    }
    assert_eq!(at(1111.0), row(12.0, "/docs", "directory"));
    assert_eq!(at(1112.0), row(13.0, "/hello.txt", "data"));
    assert_eq!(at(1113.0), row(14.0, "/bigger.txt", "data"));
    assert_eq!(at(1125.0), row(14.0, "/bigger.txt", "data"));
    assert_eq!(at(1126.0), row(14.0, "/bigger.txt", "indirect"));

    // ext2 has no journal: the root and lost+found only.
    let ext2 = Array::from(&fresh_ext().block_owners().unwrap());
    assert_eq!(ext2.length(), 13);
    assert!(ext2.iter().all(|r| text_of(&r, "role") == "directory"));

    assert_not_ext(fresh().block_owners().unwrap_err());
}

#[wasm_bindgen_test]
fn inode_number_resolves_a_path_like_lookup() {
    let v = ext3_with_files();
    for (path, ino) in [
        ("/", 2),
        ("/lost+found", 11),
        ("/docs", 12),
        ("/hello.txt", 13),
        ("/bigger.txt", 14),
    ] {
        assert_eq!(v.inode_number(path).unwrap(), ino, "{path}");
    }
    assert_eq!(code(v.inode_number("/nope").unwrap_err()), "NotFound");
    assert_eq!(code(v.inode_number("/docs/nope").unwrap_err()), "NotFound");
    assert_eq!(
        code(v.inode_number("/hello.txt/x").unwrap_err()),
        "NotADirectory"
    );
    assert_eq!(code(v.inode_number("relative").unwrap_err()), "InvalidPath");
    assert_not_ext(fresh().inode_number("/").unwrap_err());
}

#[wasm_bindgen_test]
fn inode_number_is_corrupt_image_while_a_raw_write_breaks_the_superblock() {
    let mut v = ext3_with_files();
    // Zero the superblock's magic (block 1 + 0x38): the lookup goes through the corruption gate.
    v.write_raw(1024 + 0x38, &[0, 0]).unwrap();
    let err = v.inode_number("/hello.txt").unwrap_err();
    assert_eq!(code(err.clone()), "CorruptImage");
    assert!(
        message(&err).contains("superblock or group descriptors no longer parse after a raw write")
    );
    // Writing the magic back repairs it.
    v.write_raw(1024 + 0x38, &[0x53, 0xEF]).unwrap();
    assert_eq!(v.inode_number("/hello.txt").unwrap(), 13);
}

#[wasm_bindgen_test]
fn ext_inode_decodes_the_slot_and_names_its_offset() {
    let v = ext3_with_files();
    let slot = |inode: &JsValue| {
        let s = get(inode, "slot");
        (num(&s, "block"), num(&s, "offset"))
    };

    let root = v.ext_inode(2).unwrap();
    assert_nums(
        &root,
        &[
            ("ino", 2.0),
            ("mode", 0o40755 as f64),
            ("uid", 0.0),
            ("gid", 0.0),
            ("size", 1024.0),
            ("links", 4.0), // ".", "..", lost+found's "..", /docs's ".."
            ("blocks", 2.0),
            ("flags", 0.0),
            ("atime", 315532800.0),
            ("ctime", 315532800.0),
            ("mtime", 315532800.0),
            ("dtime", 0.0),
        ],
    );
    let mut want = vec![0.0; 15];
    want[0] = 69.0;
    assert_eq!(nums(&get(&root, "block")), want);
    assert_eq!(slot(&root), (5.0, 5.0 * 1024.0 + 128.0));

    let hello = v.ext_inode(13).unwrap();
    assert_nums(
        &hello,
        &[
            ("mode", 0o100644 as f64),
            ("size", 12.0),
            ("links", 1.0),
            ("blocks", 2.0),
        ],
    );
    assert_eq!(nums(&get(&hello, "block"))[0], 1112.0);
    assert_eq!(slot(&hello), (6.0, 6.0 * 1024.0 + 0x200 as f64));

    // Twelve direct pointers, then the single-indirect block.
    let bigger = v.ext_inode(14).unwrap();
    assert_nums(&bigger, &[("size", 13312.0), ("blocks", 28.0)]);
    let mut want: Vec<f64> = (1113..=1124).map(f64::from).collect();
    want.extend([1126.0, 0.0, 0.0]);
    assert_eq!(nums(&get(&bigger, "block")), want);
    assert_eq!(slot(&bigger), (6.0, 6.0 * 1024.0 + 0x280 as f64));

    // The journal inode, and the last slot of group 1's table.
    let journal = v.ext_inode(8).unwrap();
    assert_eq!(num(&journal, "size"), 1048576.0);
    assert_eq!(slot(&journal), (5.0, 5.0 * 1024.0 + 0x380 as f64));
    let last = v.ext_inode(1024).unwrap();
    assert_eq!(num(&last, "mode"), 0.0);
    assert_eq!(slot(&last), (8260.0, 8260.0 * 1024.0 + 0x380 as f64));

    assert_eq!(code(v.ext_inode(0).unwrap_err()), "NotFound");
    assert_eq!(code(v.ext_inode(1025).unwrap_err()), "NotFound");
    assert_not_ext(fresh().ext_inode(2).unwrap_err());
}
```

- [ ] **Step 9: Write the failing native DTO tests**

Append inside `mod tests` in `crates/wasm/src/dto.rs`, after `ext_geometry_and_superblock_map_the_default_disk`:

```rust
    #[test]
    fn ext_owner_rows_carry_role_strings_in_block_order() {
        assert_eq!(block_role_name(ext::BlockRole::Data), "data");
        assert_eq!(block_role_name(ext::BlockRole::Directory), "directory");
        assert_eq!(block_role_name(ext::BlockRole::Indirect), "indirect");
        assert_eq!(block_role_name(ext::BlockRole::Journal), "journal");
        let fs = ext::ExtFs::format(ext::ExtFormatOptions::ext3()).unwrap();
        let list = ext_owners_to_list(&fs.block_owners());
        assert_eq!(list.len(), 13 + 1024 + 5);
        assert!(list.windows(2).all(|w| w[0].block < w[1].block));
        assert_eq!(
            list[0],
            ExtBlockOwner {
                block: 69,
                inode: 2,
                path: "/".into(),
                role: "directory".into()
            }
        );
        assert_eq!(
            list.last().unwrap(),
            &ExtBlockOwner {
                block: 1110,
                inode: 8,
                path: "<journal>".into(),
                role: "indirect".into()
            }
        );
    }

    #[test]
    fn ext_inode_slot_offsets_are_absolute() {
        let fs = ext::ExtFs::format(ext::ExtFormatOptions::ext3()).unwrap();
        let root = ExtInode::new(2, &fs.inode(2).unwrap(), fs.geometry());
        assert_eq!(
            root.slot,
            ExtInodeSlot {
                block: 5,
                offset: 5 * 1024 + 0x80
            }
        );
        assert_eq!((root.mode, root.links, root.block.len()), (0o40755, 3, 15));
        assert_eq!(root.block[0], 69);
        let lf = ExtInode::new(11, &fs.inode(11).unwrap(), fs.geometry());
        assert_eq!(lf.slot.offset, 6 * 1024 + 0x100);
        assert_eq!((lf.size, lf.blocks), (12288, 24));
    }
```

- [ ] **Step 10: Run both and see them fail**

```sh
wasm-pack test --node crates/wasm 2>&1 | grep -E "^error" | sort | uniq -c
cargo test -p fs-emulator-wasm --lib 2>&1 | grep -E "^error" | sort | uniq -c
```

Expected: wasm-pack fails to compile with ``error[E0599]: no method named `...` found for struct `Volume` in the current scope`` for `block_owners` (3 times), `ext_inode` (8), and `inode_number` (8); the native run fails to compile with `cannot find` errors (E0422/E0425/E0433) for `block_role_name`, `ext_owners_to_list`, `ExtBlockOwner`, `ExtInodeSlot`, and `ExtInode`.

- [ ] **Step 11: Add the owner and inode DTOs**

In `crates/wasm/src/dto.rs`, insert right after `impl From<&ext::Superblock> for ExtSuperblock { ... }` and before `fn text`:

```rust
/// `ext::BlockRole` as the string `ExtBlockOwner.role` carries.
pub fn block_role_name(role: ext::BlockRole) -> &'static str {
    match role {
        ext::BlockRole::Data => "data",
        ext::BlockRole::Directory => "directory",
        ext::BlockRole::Indirect => "indirect",
        ext::BlockRole::Journal => "journal",
    }
}

/// One row of `blockOwners()`: the inode and path a block belongs to and
/// what it holds (`data`, `directory`, `indirect`, or `journal`). The
/// journal's rows, its pointer blocks included, carry the path `<journal>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtBlockOwner {
    pub block: u32,
    pub inode: u32,
    pub path: String,
    pub role: String,
}

/// Sorted by block (BTreeMap order).
pub fn ext_owners_to_list(map: &BTreeMap<u32, ext::BlockOwner>) -> Vec<ExtBlockOwner> {
    map.iter()
        .map(|(&block, o)| ExtBlockOwner {
            block,
            inode: o.inode,
            path: o.path.clone(),
            role: block_role_name(o.role).to_string(),
        })
        .collect()
}

/// Where an inode's 128 bytes live: the inode-table block and the absolute
/// byte offset of the slot on the disk.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtInodeSlot {
    pub block: u32,
    pub offset: u64,
}

/// `extInode(ino)`: the decoded inode and its slot. `blocks` is `i_blocks`,
/// in 512-byte units; `block` is all 15 pointers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtInode {
    pub ino: u32,
    pub mode: u16,
    pub uid: u16,
    pub gid: u16,
    pub size: u32,
    pub links: u16,
    pub blocks: u32,
    pub flags: u32,
    pub atime: u32,
    pub ctime: u32,
    pub mtime: u32,
    pub dtime: u32,
    pub block: Vec<u32>,
    pub slot: ExtInodeSlot,
}

impl ExtInode {
    /// `ino` must be in `1..=geo.inodes_count` (`ExtFs::inode` checks it).
    pub fn new(ino: u32, inode: &ext::Inode, geo: &ext::Geometry) -> Self {
        let (block, in_block) = geo.inode_location(ino);
        ExtInode {
            ino,
            mode: inode.mode,
            uid: inode.uid,
            gid: inode.gid,
            size: inode.size,
            links: inode.links_count,
            blocks: inode.blocks,
            flags: inode.flags,
            atime: inode.atime,
            ctime: inode.ctime,
            mtime: inode.mtime,
            dtime: inode.dtime,
            block: inode.block.to_vec(),
            slot: ExtInodeSlot {
                block,
                offset: u64::from(block) * u64::from(geo.block_size) + in_block as u64,
            },
        }
    }
}
```

- [ ] **Step 12: Add `blockOwners`, `inodeNumber`, and `extInode` to `Volume`**

In `crates/wasm/src/volume.rs`, insert right after `ext_superblock`:

```rust
    /// Every owned block, ascending: directory, data, and indirect blocks
    /// by path, and on ext3 the journal's blocks under `<journal>`.
    #[wasm_bindgen(js_name = blockOwners, unchecked_return_type = "ExtBlockOwner[]")]
    pub fn block_owners(&self) -> Result<JsValue, JsValue> {
        to_value(&dto::ext_owners_to_list(&self.ext()?.block_owners()))
    }

    /// The inode number `path` names (2 for `/`).
    #[wasm_bindgen(js_name = inodeNumber)]
    pub fn inode_number(&self, path: &str) -> Result<u32, JsValue> {
        self.ext()?.lookup(path).map_err(to_js)
    }

    /// Inode `ino` as the disk holds it now, with its slot; `NotFound` for 0
    /// or past `inodesCount`.
    #[wasm_bindgen(js_name = extInode, unchecked_return_type = "ExtInode")]
    pub fn ext_inode(&self, ino: u32) -> Result<JsValue, JsValue> {
        let fs = self.ext()?;
        let inode = fs.inode(ino).map_err(to_js)?;
        to_value(&dto::ExtInode::new(ino, &inode, fs.geometry()))
    }
```

- [ ] **Step 13: Run the tests and see them pass**

```sh
cargo fmt --all
wasm-pack test --node crates/wasm 2>&1 | grep -E "test result"
cargo test -p fs-emulator-wasm --lib 2>&1 | grep -E "test result"
```

Expected: `test result: ok. 27 passed; 0 failed` (wasm-pack) and `test result: ok. 28 passed; 0 failed` (native lib).

#### Round 3: directory entries and file blocks

- [ ] **Step 14: Write the failing wasm-pack tests for `dirEntries` and `fileBlocks`**

Append to `crates/wasm/tests/volume.rs`, after `ext_inode_decodes_the_slot_and_names_its_offset`:

```rust
/// `(block, offset, inode, recLen, nameLen, fileType, name)` of every
/// `ExtDirEntry` in a `dirEntries` result.
type DirRow = (f64, f64, f64, f64, f64, f64, String);

fn dir_rows(list: &JsValue) -> Vec<DirRow> {
    Array::from(list)
        .iter()
        .map(|e| {
            (
                num(&e, "block"),
                num(&e, "offset"),
                num(&e, "inode"),
                num(&e, "recLen"),
                num(&e, "nameLen"),
                num(&e, "fileType"),
                text_of(&e, "name"),
            )
        })
        .collect()
}

#[wasm_bindgen_test]
fn dir_entries_list_every_record_with_absolute_offsets() {
    let mut v = ext3_with_files();
    let root = 69.0 * 1024.0;
    let entry = |offset: f64, inode: f64, rec: f64, len: f64, ft: f64, name: &str| {
        (69.0, root + offset, inode, rec, len, ft, name.to_string())
    };
    assert_eq!(
        dir_rows(&v.dir_entries("/").unwrap()),
        vec![
            entry(0.0, 2.0, 12.0, 1.0, 2.0, "."),
            entry(12.0, 2.0, 12.0, 2.0, 2.0, ".."),
            entry(24.0, 11.0, 20.0, 10.0, 2.0, "lost+found"),
            entry(44.0, 12.0, 12.0, 4.0, 2.0, "docs"),
            entry(56.0, 13.0, 20.0, 9.0, 1.0, "hello.txt"),
            entry(76.0, 14.0, 948.0, 10.0, 1.0, "bigger.txt"),
        ]
    );
    let docs = 1111.0 * 1024.0;
    assert_eq!(
        dir_rows(&v.dir_entries("/docs").unwrap()),
        vec![
            (1111.0, docs, 12.0, 12.0, 1.0, 2.0, ".".to_string()),
            (1111.0, docs + 12.0, 2.0, 1012.0, 2.0, 2.0, "..".to_string()),
        ]
    );

    // A record whose inode is zeroed stays in the list, and a name is
    // lossy UTF-8.
    v.write_raw(69 * 1024 + 76, &[0, 0, 0, 0]).unwrap();
    v.write_raw(69 * 1024 + 56 + 8, &[0xFF]).unwrap();
    let rows = dir_rows(&v.dir_entries("/").unwrap());
    assert_eq!(rows.len(), 6);
    assert_eq!(
        rows[4],
        entry(56.0, 13.0, 20.0, 9.0, 1.0, "\u{FFFD}ello.txt")
    );
    assert_eq!(rows[5], entry(76.0, 0.0, 948.0, 10.0, 1.0, "bigger.txt"));

    // The zeroed record no longer names a file.
    assert_eq!(code(v.inode_number("/bigger.txt").unwrap_err()), "NotFound");

    assert_eq!(code(v.dir_entries("/nope").unwrap_err()), "NotFound");
    assert_eq!(code(v.dir_entries("/docs/..x").unwrap_err()), "NotFound");
    assert_eq!(
        code(ext3_with_files().dir_entries("/hello.txt").unwrap_err()),
        "NotADirectory"
    );
    assert_not_ext(fresh().dir_entries("/").unwrap_err());
}

#[wasm_bindgen_test]
fn file_blocks_list_data_in_logical_order_and_every_pointer_block() {
    let mut v = ext3_with_files();
    let blocks = |v: &Volume, path: &str| {
        let fb = v.file_blocks(path).unwrap();
        let indirect: Vec<(f64, f64)> = Array::from(&get(&fb, "indirect"))
            .iter()
            .map(|i| (num(&i, "block"), num(&i, "level")))
            .collect();
        (nums(&get(&fb, "data")), indirect)
    };

    assert_eq!(blocks(&v, "/hello.txt"), (vec![1112.0], vec![]));
    assert_eq!(
        blocks(&v, "/bigger.txt"),
        ((1113..=1125).map(f64::from).collect(), vec![(1126.0, 1.0)])
    );
    assert_eq!(blocks(&v, "/"), (vec![69.0], vec![]));
    assert_eq!(blocks(&v, "/docs"), (vec![1111.0], vec![]));
    assert_eq!(
        blocks(&v, "/lost+found"),
        ((70..=81).map(f64::from).collect(), vec![])
    );

    // 269 blocks reach the double-indirect block: the single-indirect
    // block (level 1), the double (level 2), and its one second-level
    // block (level 1), after the data.
    v.create_file("/huge.bin", &[7u8; 269 * 1024]).unwrap();
    let (data, indirect) = blocks(&v, "/huge.bin");
    assert_eq!(data, (1127..=1395).map(f64::from).collect::<Vec<_>>());
    assert_eq!(indirect, vec![(1396.0, 1.0), (1397.0, 2.0), (1398.0, 1.0)]);

    assert_eq!(code(v.file_blocks("/nope").unwrap_err()), "NotFound");
    assert_not_ext(fresh().file_blocks("/").unwrap_err());
}
```

- [ ] **Step 15: Write the failing native DTO test**

Append inside `mod tests` in `crates/wasm/src/dto.rs`, after `ext_inode_slot_offsets_are_absolute`:

```rust
    #[test]
    fn ext_dir_entries_and_file_blocks_map_the_default_disk() {
        let mut fs = ext::ExtFs::format(ext::ExtFormatOptions::ext3()).unwrap();
        fs.create_file("/bigger.txt", &[b'x'; 13312]).unwrap();
        let rows: Vec<ExtDirEntry> = fs
            .dir_entries("/")
            .unwrap()
            .iter()
            .map(|(block, offset, e)| ExtDirEntry::new(*block, *offset, e))
            .collect();
        assert_eq!(rows.len(), 4);
        assert_eq!(
            rows[3],
            ExtDirEntry {
                block: 69,
                offset: 69 * 1024 + 44,
                inode: 12,
                rec_len: 980,
                name_len: 10,
                file_type: 1,
                name: "bigger.txt".into(),
            }
        );
        let inode = fs.inode(fs.lookup("/bigger.txt").unwrap()).unwrap();
        let blocks = ExtFileBlocks::new(fs.disk(), &inode);
        assert_eq!(blocks.data, (1111..=1123).collect::<Vec<u32>>());
        assert_eq!(
            blocks.indirect,
            vec![ExtIndirectBlock {
                block: 1124,
                level: 1
            }]
        );
    }
```

- [ ] **Step 16: Run both and see them fail**

```sh
wasm-pack test --node crates/wasm 2>&1 | grep -E "^error" | sort | uniq -c
cargo test -p fs-emulator-wasm --lib 2>&1 | grep -E "^error" | sort | uniq -c
```

Expected: wasm-pack fails to compile with ``error[E0599]: no method named `dir_entries` found for struct `Volume` in the current scope`` (7 times) and the same for `file_blocks` (3, one of them ``found for reference `&Volume` ``); the native run fails to compile with `cannot find` errors (E0422/E0425/E0433) for `ExtDirEntry`, `ExtIndirectBlock`, and `ExtFileBlocks`.

- [ ] **Step 17: Add the directory-entry and block-map DTOs**

In `crates/wasm/src/dto.rs`, insert right after `impl ExtInode { ... }` and before `fn text`:

```rust
/// One record of `dirEntries(path)`: the block holding it, its absolute
/// byte offset on the disk, and its fields; `name` is lossy UTF-8.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtDirEntry {
    pub block: u32,
    pub offset: u64,
    pub inode: u32,
    pub rec_len: u16,
    pub name_len: u8,
    pub file_type: u8,
    pub name: String,
}

impl ExtDirEntry {
    /// The entry `ExtFs::dir_entries` found at `offset` bytes into `block`.
    pub fn new(block: u32, offset: usize, e: &ext::DirEntry) -> Self {
        ExtDirEntry {
            block,
            offset: u64::from(block) * u64::from(ext::BLOCK_SIZE) + offset as u64,
            inode: e.inode,
            rec_len: e.rec_len,
            name_len: e.name_len,
            file_type: e.file_type,
            name: String::from_utf8_lossy(&e.name).into_owned(),
        }
    }
}

/// One pointer block of `fileBlocks(path)`: level 1 is a single-indirect
/// block or a second-level block under the double, level 2 the
/// double-indirect block.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtIndirectBlock {
    pub block: u32,
    pub level: u8,
}

/// `fileBlocks(path)`: the data blocks in logical order and the pointer
/// blocks in the order they are referenced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExtFileBlocks {
    pub data: Vec<u32>,
    pub indirect: Vec<ExtIndirectBlock>,
}

impl ExtFileBlocks {
    /// `blockmap::file_blocks` and `blockmap::indirect_blocks` of `inode`.
    pub fn new(disk: &fs_core::Disk, inode: &ext::Inode) -> Self {
        ExtFileBlocks {
            data: ext::blockmap::file_blocks(disk, inode),
            indirect: ext::blockmap::indirect_blocks(disk, inode)
                .into_iter()
                .map(|(block, level)| ExtIndirectBlock { block, level })
                .collect(),
        }
    }
}
```

- [ ] **Step 18: Add `dirEntries` and `fileBlocks` to `Volume`**

In `crates/wasm/src/volume.rs`, insert right after `ext_inode`:

```rust
    /// Every record of the directory `path` names, in the order a scan reads
    /// them (its blocks in logical order, then by offset), `.`, `..`, and
    /// zero-inode records included.
    #[wasm_bindgen(js_name = dirEntries, unchecked_return_type = "ExtDirEntry[]")]
    pub fn dir_entries(&self, path: &str) -> Result<JsValue, JsValue> {
        let list: Vec<dto::ExtDirEntry> = self
            .ext()?
            .dir_entries(path)
            .map_err(to_js)?
            .iter()
            .map(|(block, offset, entry)| dto::ExtDirEntry::new(*block, *offset, entry))
            .collect();
        to_value(&list)
    }

    /// The data blocks `path`'s inode maps, in logical order, and its
    /// pointer blocks with their levels.
    #[wasm_bindgen(js_name = fileBlocks, unchecked_return_type = "ExtFileBlocks")]
    pub fn file_blocks(&self, path: &str) -> Result<JsValue, JsValue> {
        let fs = self.ext()?;
        let inode = fs.inode(fs.lookup(path).map_err(to_js)?).map_err(to_js)?;
        to_value(&dto::ExtFileBlocks::new(fs.disk(), &inode))
    }
```

- [ ] **Step 19: Run the tests and see them pass**

```sh
cargo fmt --all
wasm-pack test --node crates/wasm 2>&1 | grep -E "test result"
cargo test -p fs-emulator-wasm --lib 2>&1 | grep -E "test result"
```

Expected: `test result: ok. 29 passed; 0 failed` (wasm-pack) and `test result: ok. 29 passed; 0 failed` (native lib).

#### Round 4: `types.ts` and README

- [ ] **Step 20: Check that the generated declarations lack the DTO types**

```sh
wasm-pack build crates/wasm --target bundler 2>&1 | tail -1
grep -c "^export interface Ext\|^export type ExtBlockOwnerRole" crates/wasm/pkg/fs_emulator_wasm.d.ts
grep -E "^\s+(extGeometry|extSuperblock|blockOwners|inodeNumber|extInode|dirEntries|fileBlocks)\(" crates/wasm/pkg/fs_emulator_wasm.d.ts
```

Expected: the build succeeds; the count is `2` (only `ExtFormatOptions` and `Ext3FormatOptions`); the seven signatures are already listed (from `unchecked_return_type`) but name types the file does not declare, so a TypeScript consumer calling them would not type-check.

- [ ] **Step 21: Declare the DTOs in the `types.ts` string**

In `crates/wasm/src/types.rs`, insert these lines right after the `export interface JournalBlock { ... }` line (inside the `TYPES` string, before `export interface BootSector`):

```ts
export interface ExtGroup { index: number; firstBlock: number; blockCount: number; superblockBlock: number | null; descriptorsBlock: number | null; blockBitmap: number; inodeBitmap: number; inodeTable: number; firstData: number; freeBlocks: number; freeInodes: number; usedDirs: number; }
export interface ExtGeometry { blockSize: number; totalBlocks: number; firstDataBlock: number; blocksPerGroup: number; inodesPerGroup: number; inodesCount: number; inodeSize: number; inodeTableBlocks: number; descriptorBlocks: number; groups: ExtGroup[]; }
export interface ExtSuperblock { inodesCount: number; blocksCount: number; reservedBlocks: number; freeBlocks: number; freeInodes: number; firstDataBlock: number; logBlockSize: number; blocksPerGroup: number; inodesPerGroup: number; magic: number; state: number; revLevel: number; firstIno: number; inodeSize: number; featureCompat: number; featureIncompat: number; featureRoCompat: number; uuid: string; label: string; journalInum: number; defaultMountOpts: number; mtime: number; wtime: number; mntCount: number; }
export type ExtBlockOwnerRole = "data" | "directory" | "indirect" | "journal";
export interface ExtBlockOwner { block: number; inode: number; path: string; role: ExtBlockOwnerRole; }
export interface ExtInodeSlot { block: number; offset: number; }
export interface ExtInode { ino: number; mode: number; uid: number; gid: number; size: number; links: number; blocks: number; flags: number; atime: number; ctime: number; mtime: number; dtime: number; block: number[]; slot: ExtInodeSlot; }
export interface ExtDirEntry { block: number; offset: number; inode: number; recLen: number; nameLen: number; fileType: number; name: string; }
export interface ExtIndirectBlock { block: number; level: number; }
export interface ExtFileBlocks { data: number[]; indirect: ExtIndirectBlock[]; }
```

- [ ] **Step 22: Rebuild and check the declarations**

```sh
wasm-pack build crates/wasm --target bundler 2>&1 | tail -1
grep -c "^export interface Ext\|^export type ExtBlockOwnerRole" crates/wasm/pkg/fs_emulator_wasm.d.ts
grep -E "^\s+(extGeometry|extSuperblock|blockOwners|inodeNumber|extInode|dirEntries|fileBlocks)\(" crates/wasm/pkg/fs_emulator_wasm.d.ts
```

Expected: `[INFO]: 📦   Your wasm pkg is ready to publish at crates/wasm/pkg.`, then `12`, then:

```text
    blockOwners(): ExtBlockOwner[];
    dirEntries(path: string): ExtDirEntry[];
    extGeometry(): ExtGeometry;
    extInode(ino: number): ExtInode;
    extSuperblock(): ExtSuperblock;
    fileBlocks(path: string): ExtFileBlocks;
    inodeNumber(path: string): number;
```

- [ ] **Step 23: Document the methods and DTOs in the README**

In `crates/wasm/README.md`, replace:

```markdown
will reuse them. The methods below are ext-only and throw `NotExt` (message
`not an ext volume`) on any other volume; the rest of the ext inspection
(superblock, inodes, block ownership) arrives with the explorer's ext panels.
A UI should branch on `fsType()` before calling the specific ones. The plan
is in `docs/ROADMAP.md`.
```

and the `blockGroupCount()` bullet that follows it, so that the section from `will reuse them.` down to (not including) the `armCrash(phase)` bullet reads:

````markdown
will reuse them. The methods below are ext-only and throw `NotExt` (message
`not an ext volume`) on any other volume. A UI should branch on `fsType()`
before calling the specific ones. The plan is in `docs/ROADMAP.md`.

- `blockGroupCount()`: how many block groups the volume has (2 on the
  default disk).
- `extGeometry()`: an `ExtGeometry`: the block and inode numbers every
  layer derives from the superblock, and per group an `ExtGroup` whose
  block numbers come from the layout and whose `freeBlocks`, `freeInodes`,
  and `usedDirs` come from the primary group descriptor.
- `extSuperblock()`: an `ExtSuperblock`, the primary superblock as the
  volume holds it now; `uuid` is lower-case hex hyphenated 8-4-4-4-12 and
  `label` stops at the first NUL (lossy UTF-8).
- `blockOwners()`: one `ExtBlockOwner` per owned block, ascending by block,
  from a walk of the tree: `role` is `"directory"`, `"data"`, or
  `"indirect"` for a path's blocks, and on ext3 the journal's data blocks
  are `"journal"` rows and its pointer blocks `"indirect"` rows, both with
  inode 8 and the path `"<journal>"`. Free blocks and metadata have no row.
- `inodeNumber(path)`: the inode number a path names (2 for `/`); it
  throws `NotFound`, `NotADirectory`, `InvalidPath`, or `CorruptImage` like
  the path methods.
- `extInode(ino)`: an `ExtInode`, inode `ino` as the disk holds it, with
  `slot`, the inode-table block and absolute byte offset of its 128 bytes.
  `NotFound` for 0 or past `inodesCount`.
- `dirEntries(path)`: one `ExtDirEntry` per record of a directory, in the
  order a scan reads them (the directory's blocks in logical order, then by
  offset), `.`, `..`, and records whose inode is 0 included; `offset` is
  absolute and `name` is lossy UTF-8. `NotADirectory` for a file.
- `fileBlocks(path)`: an `ExtFileBlocks`: `data`, the blocks the inode maps
  in logical order, and `indirect`, its pointer blocks in the order they
  are referenced (the single-indirect block at level 1, then the
  double-indirect block at level 2 followed by each second-level block at
  level 1). A directory's blocks are its `data`.

On the default ext3 disk the root directory is block 69, `lost+found` is
blocks 70 to 81, the journal's data is 82 to 1105 with its pointer blocks
1106 to 1110, and a new file starts at block 1111 with its pointer blocks
after its data. Inode `n` of group 0 sits at byte `5 * 1024 + (n - 1) *
128`.

```ts
interface ExtGeometry {
  blockSize: number;        // 1024
  totalBlocks: number;
  firstDataBlock: number;   // 1
  blocksPerGroup: number;   // 8192
  inodesPerGroup: number;
  inodesCount: number;
  inodeSize: number;        // 128
  inodeTableBlocks: number; // per group
  descriptorBlocks: number; // per copy of the descriptor table
  groups: ExtGroup[];
}

interface ExtGroup {
  index: number;
  firstBlock: number;
  blockCount: number;               // 8192, or the remainder for the last group
  superblockBlock: number | null;   // null where sparse_super keeps no backup
  descriptorsBlock: number | null;
  blockBitmap: number;
  inodeBitmap: number;
  inodeTable: number;               // first of inodeTableBlocks
  firstData: number;                // first block after the group's metadata
  freeBlocks: number;               // from the primary group descriptor
  freeInodes: number;
  usedDirs: number;
}

interface ExtSuperblock {
  inodesCount: number; blocksCount: number; reservedBlocks: number;
  freeBlocks: number; freeInodes: number; firstDataBlock: number;
  logBlockSize: number; blocksPerGroup: number; inodesPerGroup: number;
  magic: number;            // 0xEF53
  state: number; revLevel: number; firstIno: number; inodeSize: number;
  featureCompat: number; featureIncompat: number; featureRoCompat: number;
  uuid: string;             // "e2f5ee00-2026-4923-8000-000000000001" by default
  label: string;            // "" by default
  journalInum: number;      // 8 on ext3, 0 on ext2
  defaultMountOpts: number; mtime: number; wtime: number; mntCount: number;
}

type ExtBlockOwnerRole = "data" | "directory" | "indirect" | "journal";

interface ExtBlockOwner {
  block: number;
  inode: number;
  path: string;             // "<journal>" for the journal's blocks
  role: ExtBlockOwnerRole;
}

interface ExtInode {
  ino: number; mode: number; uid: number; gid: number;
  size: number;             // i_size in bytes
  links: number;
  blocks: number;           // i_blocks, in 512-byte units
  flags: number;
  atime: number; ctime: number; mtime: number; dtime: number; // Unix seconds
  block: number[];          // all 15 pointers: 12 direct, single, double, triple
  slot: ExtInodeSlot;
}

interface ExtInodeSlot {
  block: number;            // the inode-table block
  offset: number;           // absolute byte offset of the 128-byte slot
}

interface ExtDirEntry {
  block: number;
  offset: number;           // absolute byte offset of the record
  inode: number;            // 0 for an unused record
  recLen: number; nameLen: number; fileType: number;
  name: string;
}

interface ExtFileBlocks {
  data: number[];
  indirect: ExtIndirectBlock[];
}

interface ExtIndirectBlock {
  block: number;
  level: number;            // 1: single-indirect or second-level; 2: double-indirect
}
```
````

- [ ] **Step 24: Run the Rust gates**

```sh
cargo fmt --all -- --check
cargo clippy --workspace --all-targets -- -D warnings
cargo clippy -p fs-emulator-wasm --target wasm32-unknown-unknown --tests -- -D warnings
cargo test --workspace 2>&1 | grep -E "test result|FAILED"
wasm-pack test --node crates/wasm 2>&1 | grep -E "test result"
wasm-pack build crates/wasm --target bundler 2>&1 | tail -1
```

Expected: fmt prints nothing and exits 0; both clippy runs end `Finished` with no warning (the second one lints `tests/volume.rs`, which host clippy skips because it is `cfg(target_arch = "wasm32")`); every `cargo test` line is `test result: ok.` with no `FAILED` (the fs-emulator-wasm lib line reads `29 passed`); wasm-pack test reads `test result: ok. 29 passed; 0 failed`; the build ends `Your wasm pkg is ready to publish at crates/wasm/pkg.`

- [ ] **Step 25: Run the web gates (nothing under `web/` changes)**

The explorer and the demo consume the rebuilt package; they must still install, test, and build. Any pnpm 10.26 or newer works (`pnpm --version` prints `12.x` for nvm's or `10.33.0` for Homebrew's); both web packages keep their pnpm settings in a tracked `pnpm-workspace.yaml`, so a frozen install must leave `pnpm-lock.yaml` and `pnpm-workspace.yaml` unmodified. If either shows as modified in `git status`, restore it with `git checkout -- web/ui/pnpm-lock.yaml web/ui/pnpm-workspace.yaml` and run the install again. `CI=true` skips pnpm's interactive modules-purge prompt when `node_modules` was written by another pnpm.

```sh
pnpm --version
(cd web/ui && CI=true pnpm install --frozen-lockfile && pnpm test && pnpm build)
(cd web/demo && CI=true pnpm install --frozen-lockfile && pnpm build)
git status --short
```

Expected: `pnpm --version` prints `12.x` (nvm's) or `10.33.0` (Homebrew's); web/ui `Test Files  39 passed (39)`, `Tests  315 passed (315)`, svelte-check `0 ERRORS 0 WARNINGS`, `✓ built in`; web/demo `✓ built in`; `git status --short` lists only the five `crates/wasm` files (`README.md`, `src/dto.rs`, `src/types.rs`, `src/volume.rs`, `tests/volume.rs`).

- [ ] **Step 26: Check the README's ext2 recipe against the loader and say so**

The README's "Loading an ext2 image" section gives an `mke2fs` recipe and says it has not been run against this crate. Run it: the check needs e2fsprogs 1.47.4 (Homebrew's, which is keg-only, so call it by its full path; a `mke2fs` elsewhere on the `PATH` may be another version), and it loads the image through the package Step 25 installed into `web/ui`, with a throwaway test file that the step deletes again. From the repo root:

```sh
/opt/homebrew/opt/e2fsprogs/sbin/mke2fs -V 2>&1 | head -1
dd if=/dev/zero of=/tmp/ext2-recipe.img bs=1024 count=16384 2>/dev/null
/opt/homebrew/opt/e2fsprogs/sbin/mke2fs -q -F -t ext2 -b 1024 -I 128 -O none,filetype,sparse_super /tmp/ext2-recipe.img
cat > web/ui/tests/ext2-recipe.test.ts <<'EOF'
import { readFileSync } from "node:fs";
import { expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";

it("loads the README's mke2fs ext2 image", () => {
  const vol = Volume.fromImage(new Uint8Array(readFileSync("/tmp/ext2-recipe.img")));
  expect(vol.fsType()).toBe("ext2");
  expect(vol.listDir("/").map((e) => e.name)).toEqual(["lost+found"]);
  vol.createFile("/x.txt", new TextEncoder().encode("hi"));
  expect(vol.listDir("/").map((e) => e.name)).toEqual(["lost+found", "x.txt"]);
});
EOF
(cd web/ui && pnpm exec vitest run tests/ext2-recipe.test.ts)
rm web/ui/tests/ext2-recipe.test.ts /tmp/ext2-recipe.img
```

Expected: `mke2fs 1.47.4 (6-Mar-2025)`; mke2fs warns `128-byte inodes cannot handle dates beyond 2038 and are deprecated` and nothing else; vitest prints `Test Files  1 passed (1)`, `Tests  1 passed (1)`.

Then in `crates/wasm/README.md`, under `### Loading an ext2 image`, replace:

```markdown
The intended `mke2fs` form for an image it can load (not yet run against this
crate by hand; CI's e2fsprogs checks go the other way, judging the
emulator's images) is:
```

with:

```markdown
The `mke2fs` form for an image it can load (checked by hand against the
loader with e2fsprogs 1.47.4 on a 16,384-block image: it loads as `"ext2"`
and takes a `createFile`; CI's e2fsprogs checks go the other way, judging the
emulator's images) is:
```

- [ ] **Step 27: Commit**

```sh
git add crates/wasm/README.md crates/wasm/src/dto.rs crates/wasm/src/types.rs crates/wasm/src/volume.rs crates/wasm/tests/volume.rs
git commit -m "feat(wasm): ext inspection DTOs for the explorer"
```

Expected: one commit, `5 files changed, 1072 insertions(+), 11 deletions(-)`. Do not push.

---

### Task 2: the family-neutral seam amendments

Spec: `docs/superpowers/specs/2026-09-24-ext-explorer-design.md` sections 1 (`fsTypes`, `name`), 2 (all), 5 (Inspector, HexView, StatusLine, LessonPanel bullets), 6 (the composed `mkfs` done line), 10; slice-1 seam spec `docs/superpowers/specs/2026-09-23-fs-adapter-design.md` sections 1, 3, 6.

FAT output stays byte-identical with one exception, which spec section 5 sanctions: a FAT image the loader refuses with `Unsupported` (a FAT12 or FAT32 geometry) used to show the fixed status line `That isn't supported yet.` and now shows the wasm text (`unsupported: …`), because `Unsupported` leaves the status line's `FRIENDLY` table (Step 34); no test pinned either string. Every other FAT string, number, and colour is unchanged: the unit arithmetic, `ownerOf`, and `regionStart`, which every adapter writes the same way, move from `Fat16Adapter` into a shared base class `SpaceAdapter` in the new `fs/base.ts` (with the `hexAddr` helper), which `Fat16Adapter` extends and Task 3's `ExtAdapter` will too; and the chrome's literal sector words (the Step panel's buttons, the ribbon's caption, the annotation tooltip) now come from the family's sector noun, which on FAT is still `sector`. Two existing test lines have to change because the spec changes the shape they pin, and neither changes a FAT string: `tests/fs/fat16.test.ts` line 37 (the `fs.unit` object gains `fileParts`) and `tests/fs/fat16-format.test.ts` line 44 (`MKFS.done` no longer exists; the done line is pinned by `tests/shell/mutations.test.ts` line 241, unchanged). The `AddrSpace` stubs in `tests/shell/addr.test.ts` gain a `sector` field (and `blocks.unit` a `fileParts`) because the type requires them; their expectations are unchanged. `tests/fixtures/geometry.ts` needs no edit: its `space` is `fat16Space(geo)` and picks up `sector` from it.

Three more pieces land here because later tasks share them, and none changes what FAT shows:

- **The canvas grid** (Round 5). `core/grid.ts` holds the canvas prologue every canvas map repeats (`prepareCanvas`: the size at the device's pixel ratio, the scaled and cleared context), the cell arithmetic (`gridCols`, `gridRows`, `cellRect`, `cellAt`), and the marks they draw on a cell (`outlineCell`, `dotCell`, `dashCell`, `drawChain`); `core/observeWidth.ts` is the ResizeObserver width tracking as a Svelte action (`use:observeWidth`, which every Svelte 5 supports, so `package.json` keeps `"svelte": "^5.0.0"`). `FatMap.svelte` adopts both now, drawing the same pixels (same cells, same dot inset, same 2 px chain join; its canvas keeps its CSS size, and on a high-density screen `prepareCanvas` backs it at the device's ratio, so each CSS pixel is drawn exactly as before at that ratio); Task 4's block-group map and Task 5's Journal ring use them instead of their own copies.
- **The armed crash phase** (Step 30). `VolumeStore` gains `armedPhase`, re-read in `refreshMeta` from the journal capability, and `setArmedPhase(phase | null)`, which arms or disarms through the capability and updates it (the name is the store's own: the boundary scan keeps `.armCrash(` and `.disarmCrash(` for the wasm methods under `src/fs/ext/`). Arming is not an op and does not move `epoch`, so this reactive field is what lets Task 5's Journal panel and Task 6's `crash` command both arm and both see the result at once. On FAT it stays `null`.
- **One `hexAddr`** (Step 6). `shell/commands.ts` imports it from `fs/base` instead of keeping its own copy, and its two inline `` `0x${….toString(16)}` `` addresses (the `write --at` log line and `xxd`'s past-the-end error) call it too; the strings are unchanged.

Round 6 fixes three faults an ext volume would show in the shared chrome (Task 7's browser pass found them), each invisible on FAT: the ribbon's free label reads the family's free count, a new `FsAdapter.freeUnits()` (`core/freeSpace.ts`), where FAT keeps the old count of units without an owner row and Task 3's ext adapter answers `df().free`; the Step strip stops at 210 px and scrolls; and the dump's scroll requests are numbered by one counter (`core/scrollNonce.ts`) that `selection.reset()` no longer restarts.

**Files**

- Create: `web/ui/src/fs/base.ts` (1–52; Round 6 adds `freeUnits`, 39–46), `web/ui/src/core/grid.ts` (1–84), `web/ui/src/core/observeWidth.ts` (1–18), `web/ui/src/core/freeSpace.ts` (1–10), `web/ui/src/core/scrollNonce.ts` (1–13)
- Modify: `web/ui/src/fs/adapter.ts` (whole file, 1–209; Round 6 adds `freeUnits` after `df`)
- Modify: `web/ui/src/fs/index.ts` (9–14), `web/ui/src/fs/panels.ts` (6–15)
- Modify: `web/ui/src/fs/fat16/geometry.ts` (4, 18–22, 36), `web/ui/src/fs/fat16/format.ts` (57–62), `web/ui/src/fs/fat16/adapter.ts` (1–5, 23–26, 37–44, 153; removed: the local `hexAddr`, the `UnitSpace` getters and methods with `ownerOf`, and `regionStart`), `web/ui/src/fs/fat16/FatMap.svelte` (the grid helpers, `prepareCanvas`, and `use:observeWidth`, Step 39; pixels unchanged)
- Modify: `web/ui/src/core/palette.ts` (8–15), `web/ui/src/styles/tokens.css` (14, 25), `web/ui/src/app.css` (91; 140–143 the Step strip's comment and rule), `web/ui/src/core/attribution.ts` (3, 9–10, 47, 61), `web/ui/src/core/lesson.ts` (10–11, 23, 27), `web/ui/src/core/annotationFormat.ts` (24–27)
- Modify: `web/ui/src/shell/addr.ts` (1–57), `web/ui/src/shell/commands.ts` (3 the `hexAddr` import, the local `hexAddr` removed; 466 and 676 the two inline hex addresses; 581–583, 591, 606)
- Modify: `web/ui/src/state/volume.svelte.ts` (6 the `fs/adapter` import; the `needsRecovery` and `armedPhase` fields, `refreshMeta`'s last two lines, `setArmedPhase`), `web/ui/src/state/selection.svelte.ts` (1, 12–18), `web/ui/src/state/navigate.svelte.ts` (10–11)
- Modify: `web/ui/src/components/Inspector.svelte` (5, 23–26, 50–51, 60), `web/ui/src/components/HexView.svelte` (10, 63, 84–86, 106, 110–116), `web/ui/src/components/StatusLine.svelte` (7–8, 25, 45), `web/ui/src/components/StepPanel.svelte` (8–9, 24), `web/ui/src/components/Ribbon.svelte` (4, 228, 232, 261–263)
- Test: `web/ui/tests/fs/fat16.test.ts` (5, 37, 44–54), `web/ui/tests/fs/registry.test.ts` (4, 23–37, 48–61), `web/ui/tests/fs/fat16-format.test.ts` (44), `web/ui/tests/attribution.test.ts` (3, 53–73), `web/ui/tests/lesson.test.ts` (4, 50–61), `web/ui/tests/layout.test.ts` (3, 26–32 the Step strip guard, 34–42 the dump-row guard), `web/ui/tests/theme.test.ts` (3, 52–77), `web/ui/tests/annotationFormat.test.ts` (34–37), `web/ui/tests/shell/addr.test.ts` (16–35, 41–43, 74, 83, 103–126), `web/ui/tests/shell/mutations.test.ts` (252–263), `web/ui/tests/grid.test.ts` (1–40, new), `web/ui/tests/freeSpace.test.ts` (1–42, new), `web/ui/tests/scrollNonce.test.ts` (1–23, new)
- Not touched: `web/ui/src/components/App.svelte` (Task 4 renders `extras`), `web/ui/src/fs/fat16/index.ts` (its geometry re-export stays as it is; `FAT_SECTOR` is used only in `geometry.ts`), `web/ui/package.json` (a Svelte action needs no newer Svelte), every other shell command (Task 6).

**Interfaces**

Consumes: nothing new (the wasm `OpRecord` and `RegionKind`, which already includes `"journal"`).

Produces (exact):

```ts
// fs/adapter.ts
export type FsFamilyId = "fat16";   // Task 4 adds "ext"
export interface AddrVocab { singular: string; plural: string; letter: string }
export interface UnitVocab { singular: string; plural: string; letter: string; first: number; fileParts: string }
export interface UnitOwner { unit: number; path: string; isDir: boolean; firstUnit: number; role?: "data" | "directory" | "indirect" }
export interface UnitSpace { readonly unit: UnitVocab; readonly sector: AddrVocab; /* ...unchanged */ }
export function unitIsSector(space: Pick<UnitSpace, "unit" | "sector">): boolean;   // unit.singular === sector.singular
export interface MkfsSpec { summary: string; flags: MkfsFlag[] }   // `done` goes in Step 24; `summary` stays until Task 6, which gives `mkfs` one summary for every family and removes it, leaving the spec's end state { flags }
export type CrashPhase = "before_commit" | "after_commit" | "during_checkpoint";
export const CRASH_PHASES: readonly CrashPhase[];              // ["before_commit", "after_commit", "during_checkpoint"]
export const CRASH_PHASE_LABELS: Record<CrashPhase, string>;  // "before commit" | "after commit" | "during checkpoint"
export interface JournalState { mode: "ordered" | "data"; sequence: number; head: number; start: number; maxlen: number; firstBlock: number; maxTransaction: number; needsRecovery: boolean }
export interface JournalRingBlock { index: number; block: number; kind: "superblock" | "descriptor" | "copy" | "commit" | "revoke" | "unused"; tid: number | null; home: number | null; escaped: boolean | null; stale: boolean }
export interface JournalCapability { state(): JournalState; blocks(): JournalRingBlock[]; arm(phase: CrashPhase): void; disarm(): void; phase(): CrashPhase | null; recover(): OpRecord }
export interface FsFamily<O = unknown> { readonly id: FsFamilyId; readonly name: string; readonly fsTypes: readonly string[]; format(options?: O): Volume; bind(vol: Volume): FsAdapter; readonly mkfs: MkfsSpec }
export interface FsAdapter extends UnitSpace { /* ...unchanged... */ freeUnits(): number; readonly extraAddrHelp?: string; readonly needsRecovery: boolean; readonly journal?: JournalCapability }   // freeUnits: the ribbon's free count (Round 6)
// fs/base.ts (new): the members every adapter writes the same way, written once
export const hexAddr: (n: number) => string;   // `0x${n.toString(16)}`; shell/commands.ts imports it (no second copy, no inline `0x${….toString(16)}`)
export abstract class SpaceAdapter implements UnitSpace {
  abstract readonly vol: Volume; abstract owners: UnitOwner[]; protected abstract space: UnitSpace;
  // every UnitSpace getter and method delegates to `space`; plus:
  ownerOf(path: string): UnitOwner | undefined;         // the first owner row for path
  freeUnits(): number;                                  // units unit.first .. unit.first + unitCount with no owner row (Round 6); ext overrides it in Task 3
  regionStart(kind: RegionKind): number | undefined;    // vol.layout()'s first region of that kind
}
// fs/fat16/adapter.ts: class Fat16Adapter extends SpaceAdapter implements FsAdapter (same behaviour)
// fs/index.ts: familyIdOf(fsType) returns the family whose fsTypes includes fsType; throws `no adapter for ${fsType}`
// fs/panels.ts: PANELS: Record<FsFamilyId, { map: Component; format: Component; extras: Component[] }>   (fat16 extras: [])
// fs/fat16: FAT_UNIT.fileParts = "its entry, chain, and clusters"; FAT_SECTOR = { "sector", "sectors", "s" } (geometry.ts, beside FAT_UNIT); fat16Space(g).sector = FAT_SECTOR; Fat16Adapter.sector (the SpaceAdapter getter); Fat16Adapter.needsRecovery = false; fat16.fsTypes = ["FAT16"]; MKFS = { summary, flags }
// core/palette.ts: export const COLOR_JOURNAL = 11;  tokens.css: --own-11 dark #d4a017, light #b7791f; app.css: .row.own-11 (the dump's stripe and tint)
// core/annotationFormat.ts: describeRange(r: ByteRange, sector = "sector"): string   // `bytes ${start}..${end} of the ${sector} (decimal)`
// core/attribution.ts: Attr.role?: UnitOwner["role"]; attrAtSector copies the owner's role; defaultColorForRegion(kind "journal") = COLOR_JOURNAL
// core/lesson.ts: `Files: ${path}, ${space.unit.fileParts}`; the sector clause is `${space.sector.singular} ${n}, ${region}`
// shell/addr.ts: export type AddrSpace = Pick<UnitSpace, "sectorSize" | "unit" | "sector" | "unitByteRange"> & { parseAddr?(v: string): number | undefined; extraAddrHelp?: string }
// shell/commands.ts: mkfs logs `formatted /dev/hda as ${host.vol.fsType()}; the timeline was cleared`
// state/volume.svelte.ts: VolumeStore.needsRecovery = $state(false), set in refreshMeta from adapter.needsRecovery;
//   VolumeStore.armedPhase = $state<CrashPhase | null>(null), set in refreshMeta from adapter.journal?.phase() ?? null;
//   VolumeStore.setArmedPhase(phase: CrashPhase | null): boolean arms (a phase) or disarms (null) through the capability and re-reads armedPhase
//   (true with nothing to do when there is no journal; false, with status set, when the volume refuses)
// core/grid.ts (new; the canvas prologue, pure maths, and the marks, over a CanvasRenderingContext2D)
export function prepareCanvas(canvas: HTMLCanvasElement, width: number, height: number): CanvasRenderingContext2D | null;   // width × height CSS px backed at devicePixelRatio; the context scaled to CSS px and cleared
export function gridCols(width: number, cell: number, gap: number): number;            // max(1, floor(width / (cell + gap)))
export function gridRows(count: number, cols: number): number;                         // max(1, ceil(count / cols))
export function cellRect(index: number, cols: number, cell: number, gap: number): { x: number; y: number };
export function cellAt(px: number, py: number, cols: number, count: number, cell: number, gap: number): number | null;   // the gap after a cell is the cell's
export function outlineCell(ctx: CanvasRenderingContext2D, x: number, y: number, cell: number, color: string): void;   // 1 px, inside the cell
export function dotCell(ctx: CanvasRenderingContext2D, x: number, y: number, cell: number, color: string): void;       // 2 px, centred
export function dashCell(ctx: CanvasRenderingContext2D, x: number, y: number, cell: number, color: string): void;      // the dump-hover outline
export function drawChain(ctx: CanvasRenderingContext2D, cells: readonly { x: number; y: number }[], cell: number, color: string, joinWidth: number): void;
// core/observeWidth.ts (new)
export function observeWidth(el: HTMLElement, onWidth: (width: number) => void): ActionReturn<(width: number) => void>;   // a Svelte action: <div use:observeWidth={(w) => (width = w)}>
// fs/fat16/FatMap.svelte: uses both (FAT: cell 6, gap 1, dot inset 2, chain join 2 px); the same pixels as before
// Inspector address row: <dt>{cap(sector.singular)}</dt><dd>{sector} · {regionName}[ · {describeUnit note} when unitIsSector]</dd>; unit row only when !unitIsSector
// HexView g prompt: `Jump to offset (0x…, decimal, ${sector.letter}:${sector.singular}[, ${unit.letter}:${unit.singular} when the nouns differ])`
// StatusLine: FRIENDLY.NeedsRecovery; no FRIENDLY.Unsupported; <span>Volume needs recovery</span> while volume.needsRecovery
// StepPanel: `${Sector noun} ${n}` buttons ("Sector 1" on FAT); Ribbon caption `${sector.singular} ${n} · ${owner or region}`; Inspector annotation tooltip describeRange(range, sector.singular)
// core/freeSpace.ts (new)
export function freeSpaceLabel(fs: Pick<FsAdapter, "freeUnits" | "unitSize">): string;   // `${(freeUnits() * unitSize / 2**20).toFixed(1)} MB free`; the Ribbon's label, from volume.adapter
// core/scrollNonce.ts (new)
export class ScrollNonces { next(): number }   // 1, 2, 3, ...; SelectionStore keeps one for its life, so jumpTo's nonce never repeats across reset()
// app.css: .step gains max-height: 210px; overflow: auto
```

Environment for every command below, from `web/ui`: any pnpm 10.26 or newer (nvm's pnpm 12 and Homebrew's 10.33 both work). Both web packages keep their pnpm settings in a tracked `pnpm-workspace.yaml`, so a frozen install must leave `web/ui/pnpm-lock.yaml` and `web/ui/pnpm-workspace.yaml` unmodified; if either shows as modified in `git status` at any point, restore it with `git checkout web/ui/pnpm-lock.yaml web/ui/pnpm-workspace.yaml` and run `CI=true pnpm install --frozen-lockfile` again (`CI=true` lets pnpm purge a stale modules directory without asking).

The wasm package must exist before the install (`web/ui` depends on `file:../../crates/wasm/pkg`): from the repo root, `wasm-pack build crates/wasm --target bundler`; then, from `web/ui`, `CI=true pnpm install --frozen-lockfile` (`CI=true` lets pnpm purge a stale modules directory without asking). Baseline: `pnpm test` prints `Test Files  39 passed (39)` and `Tests  315 passed (315)`.

#### Round 1: the adapter types and the FAT vocabulary

- [ ] **Step 1: Write the failing FAT vocabulary tests.** In `web/ui/tests/fs/fat16.test.ts`, add the import after the `../../src/fs/fat16` import (line 4):

```ts
import { unitIsSector, type FsAdapter } from "../../src/fs/adapter";
```

Replace the `fs.unit` expectation in `it("names the unit and reports the default geometry", ...)` with:

```ts
    expect(fs.unit).toEqual({ singular: "cluster", plural: "clusters", letter: "c", first: 2, fileParts: "its entry, chain, and clusters" });
```

and add this test right after that `it` (before `it("does cluster arithmetic like the core", ...)`):

```ts
  it("names its sector apart from its cluster, and has no journal to recover", () => {
    const { fs } = fixture();
    expect(fs.sector).toEqual({ singular: "sector", plural: "sectors", letter: "s" });
    expect(unitIsSector(fs)).toBe(false);
    expect(fs.needsRecovery).toBe(false);
    const generic: FsAdapter = fs; // the optional members, as the chrome and the shell see them
    expect(generic.journal).toBeUndefined();
    expect(generic.extraAddrHelp).toBeUndefined();
    expect(fat16.fsTypes).toEqual(["FAT16"]);
  });
```

- [ ] **Step 2: Write the failing seam-helper tests.** In `web/ui/tests/fs/registry.test.ts`, add after the `../../src/fs/fat16` import:

```ts
import { CRASH_PHASES, CRASH_PHASE_LABELS, unitIsSector } from "../../src/fs/adapter";
```

and append at the end of the file:

```ts

describe("the seam's family-neutral helpers", () => {
  it("unitIsSector compares the two nouns, so a family whose unit is its sector shows one name", () => {
    const sector = { singular: "sector", plural: "sectors", letter: "s" };
    const block = { singular: "block", plural: "blocks", letter: "b" };
    expect(unitIsSector({ unit: { ...block, first: 1, fileParts: "its inode, block map, and blocks" }, sector: block })).toBe(true);
    expect(unitIsSector({ unit: { singular: "cluster", plural: "clusters", letter: "c", first: 2, fileParts: "its entry, chain, and clusters" }, sector })).toBe(false);
  });

  it("lists the crash phases in order, each with its label", () => {
    expect(CRASH_PHASES).toEqual(["before_commit", "after_commit", "during_checkpoint"]);
    expect(CRASH_PHASES.map((p) => CRASH_PHASE_LABELS[p])).toEqual(["before commit", "after commit", "during checkpoint"]);
  });
});
```

- [ ] **Step 3: Run them and watch them fail.**

```sh
pnpm test tests/fs
```

Expected: `Tests  4 failed`: `the seam's family-neutral helpers > unitIsSector compares the two nouns...` and `> lists the crash phases in order...` (the exports do not exist), `Fat16Adapter: identity and unit space > names the unit and reports the default geometry` (no `fileParts`), and `> names its sector apart from its cluster...` (no `sector`).

- [ ] **Step 4: Rewrite `web/ui/src/fs/adapter.ts`.** Replace the whole file with the following. (`MkfsSpec` keeps its `done` field in this round; Round 3 removes it together with the shell's use of it. The journal bullet of the contract comment already names the fixed owner `color` that spec section 2 adds to `UnitOwner`; the field itself arrives with Task 4, the first task that has a journal to colour.)

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
 * - The sector noun rule. `sector` is what the chrome calls one disk sector (FAT "sector",
 *   ext "block"); `unit` is the allocation unit (FAT "cluster", ext "block"). Where the two
 *   coincide (`unitIsSector`) the chrome shows one name: the Inspector folds its unit row
 *   into the address row, and the address help and HexView's `g` prompt list one form.
 * - The address help is composed generically in `shell/addr.ts` from the two nouns:
 *   `addresses: 0x1f (hex), 512 (decimal), ${sector.letter}:65 (${sector.singular})`, then
 *   `, ${unit.letter}:3 (${unit.singular})` when the unit is not the sector, then the
 *   adapter's `extraAddrHelp`. For FAT that is byte-identical to the old `ADDR_HELP`.
 * - Journal blocks are never units. Attribution keys units off `data` regions only; a
 *   family's journal lies in regions of kind `journal` (coloured `COLOR_JOURNAL`), and those
 *   blocks never appear in `owners`. A journal block outside those regions (ext's pointer
 *   blocks, in a data region) is an owner row with a fixed `color`, so it reads as the
 *   journal's rather than free.
 * - Reactivity rule. Adapter caches are plain fields, not runes, so every `$derived` that
 *   calls an adapter method reads `volume.epoch` first (see state/volume.svelte.ts).
 */
import type { Volume, Region, RegionKind, Annotation, OpRecord } from "../lib/wasm";
import type { Interval } from "../core/intervals";
import type { ColorIndex } from "../core/palette";
import type { ByteChangeLike } from "../core/patch";

export type FsFamilyId = "fat16";                       // ext adds "ext"

/** A noun and its shell address letter: how the chrome names one disk sector. */
export interface AddrVocab {
  singular: string;   // "sector"
  plural: string;     // "sectors"
  letter: string;     // "s": the shell address prefix (s:N, which every family also accepts)
}

/** The allocation-unit noun: how the UI, shell, and lessons name a data unit. */
export interface UnitVocab {
  singular: string;   // "cluster"
  plural: string;     // "clusters"
  letter: string;     // "c": the shell address prefix (c:N)
  first: number;      // first valid unit index (FAT: 2)
  fileParts: string;  // the Lesson card's clause after a path: "its entry, chain, and clusters"
}

/** One unit -> the path that owns it. Generic mirror of wasm's ClusterOwner. `role` names what
 *  the unit holds for its path where the family tells them apart (ext: a data block, a
 *  directory block, or an indirect pointer block); FAT leaves it undefined. An indirect row
 *  takes its path's hue like a data row. */
export interface UnitOwner { unit: number; path: string; isDir: boolean; firstUnit: number; role?: "data" | "directory" | "indirect" }

/** Pure unit arithmetic over one geometry. Buildable without a Volume. */
export interface UnitSpace {
  readonly unit: UnitVocab;
  readonly sector: AddrVocab;                           // FAT: sector / s; ext: block / b
  readonly unitCount: number;                           // FAT: clusterCount
  readonly unitSize: number;                            // bytes per unit
  readonly sectorSize: number;                          // vol.sectorSize()
  readonly totalSectors: number;                        // sectors the filesystem describes (FAT: the BPB total); the store's `totalSectors` is the disk's
  unitOfSector(sector: number): number | undefined;     // undefined outside the data area
  unitByteRange(unit: number): Interval;                // [start, end) bytes; no bounds check, as today
  unitOfOffset(offset: number): number | undefined;
  unitStartsAt(sector: number): boolean;                // the dump's row-label rule
  colorForRegion(region: Region): ColorIndex;           // FAT: "FAT 1" gets COLOR_TABLE_ALT
}

/** True when the family's allocation unit is its sector (ext: both are the block), so the
 *  chrome names it once. Compares the singular nouns. */
export function unitIsSector(space: Pick<UnitSpace, "unit" | "sector">): boolean {
  return space.unit.singular === space.sector.singular;
}

/** One row of the Inspector's "Selected file" trace. */
export interface TraceRow { label: string; offset: number | null }   // null = muted text, no jump
/** stat facts the shell prints as key/value after the generic ones. Keys are the family's. */
export type StatFacts = Record<string, string | number | number[]>;
export interface DfFacts { unitSize: number; units: number; used: number; free: number }

export interface MkfsFlag { long: string; desc: string; kind: "int" | "str"; option: string }
export interface MkfsSpec { summary: string; done: string; flags: MkfsFlag[] }

/** Where an armed crash stops the next journaled change: before its commit block is written,
 *  after the commit but before the checkpoint, or part way through the checkpoint. The strings
 *  are wasm's `armCrash` phases. */
export type CrashPhase = "before_commit" | "after_commit" | "during_checkpoint";
export const CRASH_PHASES: readonly CrashPhase[] = ["before_commit", "after_commit", "during_checkpoint"];
export const CRASH_PHASE_LABELS: Record<CrashPhase, string> = {
  before_commit: "before commit",
  after_commit: "after commit",
  during_checkpoint: "during checkpoint",
};

/** The journal's header facts: a structural twin of wasm's `JournalInfo`, declared here so
 *  nothing outside a family's folder imports an ext-only wasm type. */
export interface JournalState {
  mode: "ordered" | "data";
  sequence: number;
  head: number;
  start: number;
  maxlen: number;
  firstBlock: number;
  maxTransaction: number;
  needsRecovery: boolean;
}

/** One block of the journal ring, in reading order: a structural twin of wasm's
 *  `JournalBlock`. `home` is the block a copy is written back to; `stale` marks a transaction
 *  already checkpointed. */
export interface JournalRingBlock {
  index: number;
  block: number;
  kind: "superblock" | "descriptor" | "copy" | "commit" | "revoke" | "unused";
  tid: number | null;
  home: number | null;
  escaped: boolean | null;
  stale: boolean;
}

/**
 * The optional journal part of an adapter (ext3). `state()` answers from the adapter's cache,
 * refreshed with it; `blocks()` reads the volume, so callers memoise it per `volume.epoch`.
 * Arming and disarming are not recorded operations. `recover()` runs nothing through the
 * timeline itself: it returns the volume's op for the caller to run, as
 * `volume.run(() => journal.recover())` or `host.run(() => journal.recover())`.
 */
export interface JournalCapability {
  state(): JournalState;
  blocks(): JournalRingBlock[];
  arm(phase: CrashPhase): void;
  disarm(): void;
  phase(): CrashPhase | null;
  recover(): OpRecord;
}

/** Static, per family: what exists before a volume of that family does. */
export interface FsFamily<O = unknown> {
  readonly id: FsFamilyId;
  readonly name: string;                                // the family's display name: "FAT16"
  readonly fsTypes: readonly string[];                  // the Volume.fsType() strings it binds: ["FAT16"]
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
  readonly name: string;                                // the bound volume's type: "FAT16"
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
  /** What `parseAddr` adds to the address help, appended as is (ext: ", i:11 (inode)"). */
  readonly extraAddrHelp?: string;
  namesMatch(a: string, b: string): boolean;            // FAT: case-insensitive (vfs.canonicalize)

  /** True when the op may have moved regions; the store re-reads layout. FAT: any change starting below 512. */
  touchesMetadata(changes: ByteChangeLike[]): boolean;
  readonly corruptNote: string;                         // DirTree's note while the volume is corrupt
  readonly notes: { rewrite: string; partialWrite: string };  // the write --append and dd log clauses

  /** True while the volume holds an unfinished journal transaction; FAT and ext2: always false. */
  readonly needsRecovery: boolean;
  /** Present only for a family with a journal (ext3); drives the Journal panel, `crash`, and `recover`. */
  readonly journal?: JournalCapability;
}
```

- [ ] **Step 5: Give FAT its sector noun and file clause.** In `web/ui/src/fs/fat16/geometry.ts` replace the adapter import with:

```ts
import type { AddrVocab, UnitSpace, UnitVocab } from "../adapter";
```

replace the `FAT_UNIT` line with:

```ts
export const FAT_UNIT: UnitVocab = { singular: "cluster", plural: "clusters", letter: "c", first: 2, fileParts: "its entry, chain, and clusters" };

/** How FAT names one disk sector: the shell writes `s:N`. */
export const FAT_SECTOR: AddrVocab = { singular: "sector", plural: "sectors", letter: "s" };
```

and in `fat16Space` add the `sector` line after `unit: FAT_UNIT,`:

```ts
    unit: FAT_UNIT,
    sector: FAT_SECTOR,
```

`web/ui/src/fs/fat16/index.ts` keeps its geometry re-export as it is: nothing outside `geometry.ts` uses `FAT_SECTOR`, which reaches the chrome through `fat16Space(g).sector`.

- [ ] **Step 6: Write the shared adapter base.** The `UnitSpace` delegation (which gains `sector` in this task), `ownerOf`, and `regionStart` read the same in every family, so they live once in a base class that FAT extends now and ext extends in Task 3. Create `web/ui/src/fs/base.ts`:

```ts
import type { Region, RegionKind, Volume } from "../lib/wasm";
import type { Interval } from "../core/intervals";
import type { ColorIndex } from "../core/palette";
import type { AddrVocab, UnitOwner, UnitSpace, UnitVocab } from "./adapter";

/** An offset as the shell and the Inspector print it: `0x` and lower-case hex. */
export const hexAddr = (n: number): string => `0x${n.toString(16)}`;

/**
 * The part of an adapter every family writes the same way, so each family writes it once:
 * the `UnitSpace` members delegate to the space its last `refresh()` built, `ownerOf` is the
 * first owner row for a path, and `regionStart` reads the volume's layout. A family extends it,
 * sets `space` and `owners` in `refresh()`, and supplies the rest of `FsAdapter`. The fields
 * stay plain (the reactivity rule in fs/adapter.ts).
 */
export abstract class SpaceAdapter implements UnitSpace {
  abstract readonly vol: Volume;
  abstract owners: UnitOwner[];
  /** The unit arithmetic over the geometry the last `refresh()` read. */
  protected abstract space: UnitSpace;

  get unit(): UnitVocab { return this.space.unit; }
  get sector(): AddrVocab { return this.space.sector; }
  get unitCount(): number { return this.space.unitCount; }
  get unitSize(): number { return this.space.unitSize; }
  get sectorSize(): number { return this.space.sectorSize; }
  get totalSectors(): number { return this.space.totalSectors; }
  unitOfSector(sector: number): number | undefined { return this.space.unitOfSector(sector); }
  unitByteRange(unit: number): Interval { return this.space.unitByteRange(unit); }
  unitOfOffset(offset: number): number | undefined { return this.space.unitOfOffset(offset); }
  unitStartsAt(sector: number): boolean { return this.space.unitStartsAt(sector); }
  colorForRegion(region: Region): ColorIndex { return this.space.colorForRegion(region); }

  /** The first owner row for `path` (its first unit's row on FAT). */
  ownerOf(path: string): UnitOwner | undefined {
    return this.owners.find((o) => o.path === path);
  }

  /** The first sector of the first region of that kind in the volume's layout. */
  regionStart(kind: RegionKind): number | undefined {
    return this.vol.layout().find((r) => r.kind === kind)?.sectors.start;
  }
}
```

`fs/base.ts` calls only the family-neutral `layout()`, so the boundary scan in `tests/adapterBoundary.test.ts` passes it, and `fs/fat16` (and later `fs/ext`) may import it: the scan only restricts imports *from* the family folders. `shell/commands.ts` has its own copy of the same helper; it takes this one instead. In `web/ui/src/shell/commands.ts` delete the line ``export const hexAddr = (n: number): string => `0x${n.toString(16)}`;`` and the blank line after it (no other file imports it from there), and add after `import type { DateTime, EntryInfo, Volume } from "../lib/wasm";`:

```ts
import { hexAddr } from "../fs/base";
```

Two addresses in `commands.ts` are formatted inline rather than through the helper; they call it too, printing the same text. In `write`'s raw branch replace ``ctx.log(`${data.length} bytes -> /dev/hda at 0x${off.toString(16)}`);`` with

```ts
        ctx.log(`${data.length} bytes -> /dev/hda at ${hexAddr(off)}`);
```

and in `xxd`'s `raw` case replace ``if (base >= disk) throw new ShellError(`0x${base.toString(16)} is past the end of the disk (${disk} bytes)`);`` with

```ts
          if (base >= disk) throw new ShellError(`${hexAddr(base)} is past the end of the disk (${disk} bytes)`);
```

Beyond that and the `mkfs` done line (Step 24), this task does not touch `commands.ts`.

- [ ] **Step 7: Put the FAT adapter on the base and give it and its family the new members.** In `web/ui/src/fs/fat16/adapter.ts` replace the imports and the local `hexAddr` (everything from the first line through `const hexAddr = ...;`) with:

```ts
import { Volume, type Annotation, type ClusterOwner, type FatEntry, type FormatOptions, type Geometry } from "../../lib/wasm";
import type { Interval } from "../../core/intervals";
import type { ByteChangeLike } from "../../core/patch";
import type { DfFacts, FsAdapter, FsFamily, FsFamilyId, StatFacts, TraceRow, UnitOwner, UnitSpace } from "../adapter";
import { SpaceAdapter, hexAddr } from "../base";
import { findEntrySlots } from "./direntry";
import { buildChain, describeFatEntry } from "./fatchain";
import { MKFS } from "./format";
import { fat16Space } from "./geometry";
import { CORRUPT_NOTE, NOTES, touchesBootSector } from "./metadata";
import { findRemnants } from "./remnants";
```

replace the last line of the class's doc comment and the class line

```ts
 * themselves. Every cache is a plain field (see the reactivity rule in fs/adapter.ts).
 */
export class Fat16Adapter implements FsAdapter {
```

with

```ts
 * themselves. Every cache is a plain field (see the reactivity rule in fs/adapter.ts). The unit
 * arithmetic, `ownerOf`, and `regionStart` are the shared `SpaceAdapter`'s (fs/base.ts).
 */
export class Fat16Adapter extends SpaceAdapter implements FsAdapter {
```

after `readonly notes = NOTES;` add:

```ts
  /** FAT has no journal, so there is never a transaction to recover (and no `journal`). */
  readonly needsRecovery = false;
```

replace the private fields and the constructor

```ts
  private geo!: Geometry;
  private space!: UnitSpace;
  private rawOwners: ClusterOwner[] = [];

  constructor(vol: Volume) {
    this.vol = vol;
    this.refresh();
  }
```

with

```ts
  protected space!: UnitSpace;
  private geo!: Geometry;
  private rawOwners: ClusterOwner[] = [];

  constructor(vol: Volume) {
    super();
    this.vol = vol;
    this.refresh();
  }
```

delete the `// UnitSpace, over the geometry the last refresh() read.` block (the comment, the five getters, and the five methods under it) and the `ownerOf` method after it, so `refresh()` is followed directly by `chain`; delete the `regionStart` method (between `dataStart` and `fatEntryOffset`); and in the `fat16` family object add `fsTypes` after `name`:

```ts
  id: "fat16",
  name: "FAT16",
  fsTypes: ["FAT16"],
```

The class keeps every other member as it is; `ownerOf`, `regionStart`, and the `UnitSpace` members now come from `SpaceAdapter` with the same bodies, and the new `sector` getter with them.

- [ ] **Step 8: Run the tests.**

```sh
pnpm test
```

Expected: `Test Files  39 passed (39)`, `Tests  318 passed (318)`.

#### Round 2: registry, panels, palette, attribution, lesson

- [ ] **Step 9: Write the failing registry test.** In `web/ui/tests/fs/registry.test.ts` change the adapter import to:

```ts
import { CRASH_PHASES, CRASH_PHASE_LABELS, unitIsSector, type FsFamily } from "../../src/fs/adapter";
```

and insert before `it("adapterFor binds the family whose name is the volume's fsType", ...)`:

```ts
  it("matches a type against each family's fsTypes, not its display name", () => {
    expect(fat16.fsTypes).toEqual(["FAT16"]);
    // One family may bind several types (ext: ext2 and ext3) under a display name that is
    // none of them; swap in such a family for the length of the check.
    const table = FAMILIES as Record<string, FsFamily>;
    table.fat16 = { ...fat16, name: "FAT", fsTypes: ["FAT16", "FAT16B"] };
    try {
      expect(familyIdOf("FAT16")).toBe("fat16");
      expect(familyIdOf("FAT16B")).toBe("fat16");
      expect(() => familyIdOf("FAT")).toThrow("no adapter for FAT");
    } finally {
      table.fat16 = fat16;
    }
    expect(FAMILIES.fat16).toBe(fat16);
  });

```

The existing `expect(() => familyIdOf("EXT3")).toThrow("no adapter for EXT3")` stays true: nothing registers ext yet.

- [ ] **Step 10: Write the failing attribution tests.** In `web/ui/tests/attribution.test.ts` change the palette import to:

```ts
import { COLOR_JOURNAL, COLOR_TABLE, COLOR_TABLE_ALT, colorIndexForPath } from "../src/core/palette";
```

and insert before `it("file hues are stable and in range", ...)`:

```ts
  it("colours a journal region COLOR_JOURNAL and never treats its sectors as units", () => {
    expect(COLOR_JOURNAL).toBe(11);
    const journal = { name: "journal", sectors: { start: 200, end: 300 }, kind: "journal" as const };
    expect(defaultColorForRegion(journal)).toBe(COLOR_JOURNAL);
    // The same disk with a journal carved out of the data region: its sectors map to clusters
    // in the unit arithmetic, but attribution keys units off `data` regions only.
    const split = [...layout.slice(0, 4), { name: "data", sectors: { start: 97, end: 200 }, kind: "data" as const }, journal, { name: "data", sectors: { start: 300, end: 32768 }, kind: "data" as const }];
    const tj = buildAttribution(space, split, owners);
    expect(space.unitOfSector(250)).toBeDefined();
    expect(attrAtSector(tj, 250)).toEqual({ regionKind: "journal", regionName: "journal", sector: 250, free: false, colorIndex: COLOR_JOURNAL });
  });
  it("gives an indirect owner row its path's hue and reports the role", () => {
    const withRole: UnitOwner[] = [
      { unit: 5, path: "/BIG", isDir: false, firstUnit: 5, role: "data" },
      { unit: 6, path: "/BIG", isDir: false, firstUnit: 5, role: "indirect" },
    ];
    const tr = buildAttribution(space, layout, withRole);
    expect(attrAtSector(tr, 113)).toMatchObject({ unit: 6, ownerPath: "/BIG", role: "indirect", colorIndex: colorIndexForPath("/BIG") });
    expect(attrAtSector(tr, 109)).toMatchObject({ unit: 5, role: "data", colorIndex: colorIndexForPath("/BIG") });
    expect(attrAtSector(t, 97).role).toBeUndefined(); // FAT rows carry no role
  });
```

- [ ] **Step 11: Write the failing lesson test.** In `web/ui/tests/lesson.test.ts` add after the fixture import:

```ts
import type { UnitSpace } from "../src/fs/adapter";
```

and insert before `it("is null when there is nothing to point at", ...)` (every existing FAT string in the file stays as it is):

```ts
  it("takes the file clause and the sector noun from the space (a family whose unit is its block)", () => {
    const blocks: UnitSpace = {
      ...space,
      unit: { singular: "block", plural: "blocks", letter: "b", first: 1, fileParts: "its inode, block map, and blocks" },
      sector: { singular: "block", plural: "blocks", letter: "b" },
    };
    const ext = (focus: Parameters<typeof describeFocus>[0]) => describeFocus(focus, blocks, regionNameAt);
    expect(ext({ path: "/a" })).toBe("Files: /a, its inode, block map, and blocks");
    expect(ext({ sector: 65 })).toBe("Block 65, root directory");
    expect(ext({ path: "/a", sector: 1 })).toBe("Files: /a, its inode, block map, and blocks; block 1, FAT 0");
  });

```

- [ ] **Step 12: Write the failing guards for the journal colour's dump rule and its values.** The dump colours a row through `HexRow.svelte`'s `class="row own-{attr.colorIndex}"` and the `.row.own-N` rules in `app.css`, which stop at `own-10`; a journal block without a rule would get neither its stripe nor its tint. In `web/ui/tests/layout.test.ts` add after the `vitest` import:

```ts
import { COLOR_JOURNAL, COLOR_TABLE_ALT } from "../src/core/palette";
```

and add inside `describe("app.css layout guards", ...)`, after the `.app.term-side` test:

```ts

  it("gives every palette colour, the journal's included, a dump row stripe and tint", () => {
    // HexRow classes a row `own-${colorIndex}`; an index with no rule here draws a row with
    // neither its left stripe nor its tint, so the rules run up to the last index, COLOR_JOURNAL.
    expect(COLOR_JOURNAL).toBeGreaterThan(COLOR_TABLE_ALT);
    for (let i = 1; i <= COLOR_JOURNAL; i++) {
      expect(rule(`.row.own-${i}`), `.row.own-${i}`).toContain(`border-left-color: var(--own-${i});`);
      expect(css).toContain(`.row.own-${i} .b { background: color-mix(in srgb, var(--own-${i}) 14%, transparent); }`);
    }
  });
```

The journal amber's values follow a stated rule rather than an eye check: in each theme, at least 25° of hue or 20 % of HSL lightness from every file hue (`--own-4`..`--own-9`) and from the table violets (`--own-2`, `--own-10`). In `web/ui/tests/theme.test.ts` add after the `vitest` import:

```ts
import { COLOR_FILE_BASE, COLOR_JOURNAL, COLOR_TABLE, COLOR_TABLE_ALT, FILE_HUE_COUNT } from "../src/core/palette";
```

and add inside `describe("theme wiring outside the module graph", ...)`, after its last test (`the tokens key on data-theme ...`):

```ts

  it("the journal amber keeps 25° of hue or 20 % of lightness from every file hue and table violet, in both themes", () => {
    const css = readFileSync(new URL("../src/styles/tokens.css", here), "utf8");
    const own = (block: string | undefined) =>
      Object.fromEntries([...(block ?? "").matchAll(/--own-(\d+): (#[0-9a-f]{6})/g)].map((m) => [Number(m[1]), m[2]]));
    const dark: Record<number, string> = own(css.match(/:root \{([^}]*)\}/)?.[1]);
    const light: Record<number, string> = { ...dark, ...own(css.match(/:root\[data-theme="light"\] \{([^}]*)\}/)?.[1]) };
    /** Hue in degrees and HSL lightness in percent of a `#rrggbb` colour. */
    const hsl = (hex: string) => {
      const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
      const max = Math.max(r, g, b), min = Math.min(r, g, b), d = max - min;
      const h = d === 0 ? 0 : max === r ? ((g - b) / d + 6) % 6 : max === g ? (b - r) / d + 2 : (r - g) / d + 4;
      return { hue: h * 60, lightness: ((max + min) / 2) * 100 };
    };
    const others = [COLOR_TABLE, COLOR_TABLE_ALT, ...Array.from({ length: FILE_HUE_COUNT }, (_, i) => COLOR_FILE_BASE + i)];
    expect([dark[COLOR_JOURNAL], light[COLOR_JOURNAL]]).toEqual(["#d4a017", "#b7791f"]);
    for (const [theme, colours] of [["dark", dark], ["light", light]] as const) {
      const amber = hsl(colours[COLOR_JOURNAL]);
      for (const i of others) {
        const other = hsl(colours[i]);
        const apart = Math.abs(amber.hue - other.hue);
        const hue = Math.min(apart, 360 - apart), lightness = Math.abs(amber.lightness - other.lightness);
        expect(hue >= 25 || lightness >= 20, `${theme}: --own-${COLOR_JOURNAL} against --own-${i} (${hue.toFixed(1)}°, ${lightness.toFixed(1)} %)`).toBe(true);
      }
    }
  });
```

- [ ] **Step 13: Run them and watch them fail.**

```sh
pnpm test tests/fs/registry.test.ts tests/attribution.test.ts tests/lesson.test.ts tests/layout.test.ts tests/theme.test.ts
```

Expected: `Tests  6 failed | 27 passed (33)`: the fsTypes test (`familyIdOf("FAT16B")` throws), both attribution tests (`COLOR_JOURNAL` is undefined; no `role` on the attr), the lesson test (`Files: /a, its entry, chain, and blocks`), the dump-row guard (`COLOR_JOURNAL` is undefined), and the amber test (`expected [ undefined, undefined ] to deeply equal [ '#d4a017', '#b7791f' ]`).

- [ ] **Step 14: Match types in the registry.** In `web/ui/src/fs/index.ts` replace `familyIdOf`'s comment and loop with:

```ts
/** The family whose `fsTypes` lists this `Volume.fsType()` string ("FAT16" -> "fat16"; one
 *  family may bind several types, as ext binds ext2 and ext3). Throws for a type no adapter
 *  handles; the store's `load` turns that into a status line like any other load failure. */
export function familyIdOf(fsType: string): FsFamilyId {
  for (const family of Object.values(FAMILIES)) if (family.fsTypes.includes(fsType)) return family.id;
  throw new Error(`no adapter for ${fsType}`);
}
```

- [ ] **Step 15: Give each family's panels an `extras` list.** In `web/ui/src/fs/panels.ts` replace the doc comment and table with:

```ts
/**
 * The Svelte panels of each family, looked up by `volume.adapter.id`: `map` is the panel App
 * renders under the Files tree (the FAT map), `format` the body of the Actions panel's Format
 * details, and `extras` the family's further panels, rendered under the map in order (FAT has
 * none; ext's Journal panel goes here). They live here rather than on the adapter because `vitest.config.ts` has no Svelte
 * plugin: an adapter that imported a `.svelte` file could not be loaded by the node tests.
 */
export const PANELS: Record<FsFamilyId, { map: Component; format: Component; extras: Component[] }> = {
  fat16: { map: FatMap, format: FormatForm, extras: [] },
};
```

`App.svelte` does not render `extras` yet (Task 4 does).

- [ ] **Step 16: Add the journal colour and its dump rule.** In `web/ui/src/core/palette.ts` add after `COLOR_TABLE_ALT`:

```ts
/** A journal's blocks: a warm amber apart from every file hue and from the table violet, so
 *  "written twice, once here first" reads as its own thing in the ribbon, the dump, and the map.
 *  The rule its `--own-11` values keep (tests/theme.test.ts checks it): in each theme, at least
 *  25° of hue or 20 % of HSL lightness from every file hue (`--own-4`..`--own-9`) and from the
 *  table violets (`--own-2`, `--own-10`). Dark `#d4a017` (hue 44°, lightness 46 %: 28° from
 *  the salmon `--own-5`, 26 % darker than the pale amber `--own-9`); light `#b7791f` (hue 36°,
 *  lightness 42 %: 29 % darker than `--own-5`, 30 % darker than `--own-9`). */
export const COLOR_JOURNAL = 11;
```

In `web/ui/src/styles/tokens.css` add after the `--own-10` line of the `:root` (dark) block:

```css
  --own-11: #d4a017; /* journal amber: 25° of hue or 20% of lightness from --own-2, --own-4..--own-10 (core/palette.ts) */
```

and replace the light block's `--own-*` line with:

```css
  --own-1: #8b93a1; --own-2: #7f77dd; --own-3: #1d9e75; --own-10: #afa9ec; --own-11: #b7791f;
```

The values, measured (hue difference, lightness difference) against the nearest colours: dark `#d4a017` against `--own-5` #f0997b (28.1°, 25.1 %), `--own-9` #fac775 (6.5°, 25.9 %), `--own-8` #97c459 (41.7°, 9.8 %); light `#b7791f` against `--own-5` (20.1°, 29.2 %), `--own-9` (1.5°, 30.0 %), `--own-8` (49.7°, 13.9 %); every violet and the teal, pink, and blue hues are more than 50° away in both themes. (The light theme overrides only `--own-1`, `--own-2`, `--own-3`, and `--own-10`, so its file hues are the dark ones.)

In `web/ui/src/app.css` add after the `.row.own-10 { ... }` line, so a journal block in the dump gets its stripe and tint like every other colour:

```css
.row.own-11 { border-left-color: var(--own-11); } .row.own-11 .b { background: color-mix(in srgb, var(--own-11) 14%, transparent); }
```

- [ ] **Step 17: Attribute journals and roles.** In `web/ui/src/core/attribution.ts` replace the palette import with:

```ts
import { COLOR_BOOT, COLOR_DIR, COLOR_FREE, COLOR_JOURNAL, COLOR_TABLE, colorIndexForPath, type ColorIndex } from "./palette";
```

add to `Attr` after the `unit?: ...` line:

```ts
  /** The owner row's role, where the family sets one (ext: "indirect" for a pointer block). */
  role?: UnitOwner["role"];
```

add the `journal` case to `defaultColorForRegion` after `case "data": return COLOR_FREE;`:

```ts
    case "journal": return COLOR_JOURNAL;
```

and make the owned-unit return of `attrAtSector`:

```ts
  return { ...base, unit, ownerPath: o.path, isDir: o.isDir, role: o.role, free: false, colorIndex: t.colorByUnit[unit] };
```

`buildAttribution` is unchanged: it already colours any non-directory row, indirect or not, with `colorIndexForPath(o.path)`.

- [ ] **Step 18: Take the lesson's nouns from the space.** In `web/ui/src/core/lesson.ts` replace the last sentence of the doc comment's first paragraph with:

```ts
 * cannot reach on its own; `space` supplies the family's nouns (the file clause
 * `unit.fileParts`, the sector and unit names) and byte arithmetic.
```

replace the path clause with:

```ts
  if (typeof focus.path === "string") parts.push(`Files: ${focus.path}, ${space.unit.fileParts}`);
```

and the sector clause with:

```ts
  else if (focus.sector !== undefined) parts.push(`${space.sector.singular} ${focus.sector}, ${regionNameAt(focus.sector)}`);
```

- [ ] **Step 19: Run the tests.**

```sh
pnpm test
```

Expected: `Test Files  39 passed (39)`, `Tests  324 passed (324)`.

#### Round 3: addresses and the `mkfs` done line

- [ ] **Step 20: Write the failing address tests.** In `web/ui/tests/shell/addr.test.ts` replace the `blocks` stub with the following, and add the ext-shaped stub and its help after it:

```ts
/** A made-up family with 1 KiB units counted from 0 and the letter `b`, to prove the noun and
 *  letter come from the space rather than from the parser. Its sector keeps FAT's noun. */
const blocks: AddrSpace = {
  sectorSize: 1024,
  unit: { singular: "block", plural: "blocks", letter: "b", first: 0, fileParts: "its blocks" },
  sector: { singular: "sector", plural: "sectors", letter: "s" },
  unitByteRange: (u) => ({ start: u * 1024, end: (u + 1) * 1024 }),
};

/** An ext-shaped family: 1 KiB blocks that are both its sector and its unit (counted from 1,
 *  block N at N KiB), plus an inode form of its own with the help text for it. */
const extLike: AddrSpace = {
  sectorSize: 1024,
  unit: { singular: "block", plural: "blocks", letter: "b", first: 1, fileParts: "its inode, block map, and blocks" },
  sector: { singular: "block", plural: "blocks", letter: "b" },
  unitByteRange: (u) => ({ start: u * 1024, end: (u + 1) * 1024 }),
  parseAddr: (v) => (/^i:\d+$/.test(v) ? 5 * 1024 + (Number(v.slice(2)) - 1) * 128 : undefined),
  extraAddrHelp: ", i:11 (inode)",
};
const EXT_HELP = "addresses: 0x1f (hex), 512 (decimal), b:65 (block), i:11 (inode)";
```

Add a second test inside `describe("addrHelp", ...)`, after the existing one:

```ts
  it("names one form when the unit is the sector, then the family's extra forms", () => {
    expect(addrHelp(extLike)).toBe(EXT_HELP);
  });
```

In the two inline stubs `tiny` and `withInodes`, add `sector: space.sector,` after `unit: space.unit,` (their expectations do not change):

```ts
      unit: space.unit,
      sector: space.sector,
```

Insert before `describe("parseSize", ...)`:

```ts
describe("parseAddr on a family whose sector letter is not s", () => {
  it("accepts s:N and the family's sector letter as sectors, hex, decimal, and the family's form", () => {
    expect(parseAddr("s:3", extLike)).toBe(3 * 1024);
    expect(parseAddr("b:3", extLike)).toBe(3 * 1024);
    expect(parseAddr("B:69", extLike)).toBe(69 * 1024);
    expect(parseAddr("0x400", extLike)).toBe(1024);
    expect(parseAddr("2048", extLike)).toBe(2048);
    expect(parseAddr("i:11", extLike)).toBe(5 * 1024 + 10 * 128);
  });
  it("reads b:N as a sector before a unit, which is what makes it safe where the two differ", () => {
    // A family whose sector letter is b but whose unit starts elsewhere: b:N is the sector.
    const shifted: AddrSpace = { ...extLike, unit: { ...extLike.unit, letter: "u" }, unitByteRange: (u) => ({ start: 7 + u, end: 8 + u }) };
    expect(parseAddr("b:2", shifted)).toBe(2048);
    expect(parseAddr("u:2", shifted)).toBe(9);
  });
  it("throws with the family's help, which lists every form it accepts", () => {
    for (const bad of ["c:3", "i:x", "b:", "x:1"]) {
      const e = thrown(() => parseAddr(bad, extLike));
      expect(e.message).toBe(`bad address '${bad}'`);
      expect(e.help).toBe(EXT_HELP);
    }
  });
});

```

- [ ] **Step 21: Write the failing `mkfs` tests.** In `web/ui/tests/shell/mutations.test.ts`, inside `describe("mkfs", ...)`, add after the first test (`formats with the given geometry...`, whose pinned `formatted /dev/hda as FAT16; the timeline was cleared` stays as it is):

```ts

  it("names the type the new volume reports in its done line", async () => {
    const { host, defs } = setup();
    // A host whose freshly formatted volume reports another type: the line follows
    // `fsType()`, not a string of the family's.
    const format = host.format.bind(host);
    host.format = (family, options) => {
      format(family, options);
      host.vol = Object.assign(Object.create(host.vol), { fsType: () => "ext3" });
    };
    expect((await call(defs, "mkfs")).log).toEqual(["formatted /dev/hda as ext3; the timeline was cleared"]);
  });
```

In `web/ui/tests/fs/fat16-format.test.ts`, `it("keeps the shell's mkfs strings", ...)`, replace the `MKFS.done` line with:

```ts
    expect(MKFS).not.toHaveProperty("done"); // the shell composes the done line from fsType() (tests/shell/mutations.test.ts)
```

- [ ] **Step 22: Run them and watch them fail.**

```sh
pnpm test tests/shell/addr.test.ts tests/shell/mutations.test.ts tests/fs/fat16-format.test.ts
```

Expected: `Tests  5 failed`: `addrHelp > names one form when the unit is the sector...` (today's help says `s:65 (sector), b:3 (block)`), `parseAddr on a family whose sector letter is not s > reads b:N as a sector before a unit...` and `> throws with the family's help...` (help mismatch), `mkfs > names the type the new volume reports in its done line` (logs `... as FAT16; ...`), and `DEFAULTS and MKFS > keeps the shell's mkfs strings` (`MKFS` still has `done`).

- [ ] **Step 23: Compose the address help and parse the sector letter.** In `web/ui/src/shell/addr.ts` replace everything from the first line through the end of `parseAddr` (lines 1–48 before the change; `SIZE_HELP` and `parseSize` are unchanged) with:

```ts
import { unitIsSector, type UnitSpace } from "../fs/adapter";
import { ShellError } from "./errors";

export const SIZE_HELP = "sizes: 512, 1k, 4M, 0x200";

/**
 * What an address parser needs from a family: the sector size, the sector and unit nouns and
 * letters, the unit's byte range, and optionally the family's own extra forms (`parseAddr`,
 * which answers `undefined` for anything that is not its own, and `extraAddrHelp`, the help
 * text for them). A `UnitSpace` or an `FsAdapter` satisfies it; tests build one from a fixture.
 */
export type AddrSpace = Pick<UnitSpace, "sectorSize" | "unit" | "sector" | "unitByteRange"> & {
  parseAddr?(v: string): number | undefined;
  extraAddrHelp?: string;
};

/** The address forms in one line, for error help and flag descriptions: the sector form, the
 *  unit form when the unit is not the sector, then the family's extras. For FAT this reads
 *  `addresses: 0x1f (hex), 512 (decimal), s:65 (sector), c:3 (cluster)`; for ext
 *  `addresses: 0x1f (hex), 512 (decimal), b:65 (block), i:11 (inode)`. */
export function addrHelp(space: AddrSpace): string {
  const { sector, unit } = space;
  const unitForm = unitIsSector(space) ? "" : `, ${unit.letter}:3 (${unit.singular})`;
  return `addresses: 0x1f (hex), 512 (decimal), ${sector.letter}:65 (${sector.singular})${unitForm}${space.extraAddrHelp ?? ""}`;
}

/** `<letter>:N` with the family's letter, case-insensitively like `s:N`; `undefined` otherwise. */
function letterIndex(v: string, letter: string): number | undefined {
  const prefix = `${letter}:`;
  if (v.length <= prefix.length || v.slice(0, prefix.length).toLowerCase() !== prefix.toLowerCase()) return undefined;
  const digits = v.slice(prefix.length);
  return /^\d+$/.test(digits) ? Number(digits) : undefined;
}

/**
 * The address forms HexView's `g` prompt accepts, in order: `s:N` (a sector, on every family),
 * `0x…`, decimal, the family's sector letter when it is not `s` (ext's `b:N`), the family's
 * unit letter (a data unit: FAT's `c:N`, whose clusters start at 2), then whatever else the
 * family parses (`space.parseAddr`, ext's `i:N`). A number passes through when it is a
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
    const sector = space.sector.letter.toLowerCase() === "s" ? undefined : letterIndex(v, space.sector.letter);
    const unit = sector === undefined ? letterIndex(v, space.unit.letter) : undefined;
    off = sector !== undefined ? sector * space.sectorSize : unit !== undefined ? space.unitByteRange(unit).start : space.parseAddr?.(v);
  }
  if (off === undefined || !Number.isInteger(off) || off < 0) throw bad();
  return off;
}
```

The shell's callers (`seek`, `write --at`, `xxd --offset`) already pass `host.adapter`, which now carries `sector` and may carry `extraAddrHelp`; they need no change.

- [ ] **Step 24: Drop `done` from `MkfsSpec` and compose the line in the shell.** `summary` stays for now, although the spec's section 2 ends with `MkfsSpec` as `{ flags }`: the shell's `mkfs` reads a family's summary until Task 6 gives it one summary for every family and removes `summary` from the seam and both families (the spec lets it survive until then). Task 3's `MKFS` carries one meanwhile. In `web/ui/src/fs/adapter.ts` replace the `MkfsSpec` line with:

```ts
/** The shell's `mkfs` for one family. It has no done line: the shell composes
 *  `formatted /dev/hda as ${vol.fsType()}; the timeline was cleared` from the new volume, so a
 *  family with two types (ext2, ext3) names the one it made. */
export interface MkfsSpec { summary: string; flags: MkfsFlag[] }
```

In `web/ui/src/fs/fat16/format.ts` replace `MKFS` and its comment with:

```ts
/** The shell's `mkfs` for this family: its summary and the six flags. The done line is the
 *  shell's, composed from the new volume's `fsType()`. */
export const MKFS: MkfsSpec = {
  summary: "Format /dev/hda as FAT16 (clears the timeline)",
  flags: MKFS_FLAGS,
};
```

In `web/ui/src/shell/commands.ts` (`mutationCommands`), replace the comment above `const mkfs` with:

```ts
  // The flags, their option keys, and the summary are the family's (`FsFamily.mkfs`); the
  // spec is built once at registration, for the family mounted then. The done line names the
  // type the new volume reports, so one family with two types says which it made.
```

replace `const { flags, done } = host.adapter.family.mkfs;` with:

```ts
      const { flags } = host.adapter.family.mkfs;
```

and replace `ctx.log(done);` with:

```ts
      ctx.log(`formatted /dev/hda as ${host.vol.fsType()}; the timeline was cleared`);
```

- [ ] **Step 25: Check nothing else read `done`.**

```sh
grep -rnE "mkfs\.done|MKFS\.done|\.done\b|done:" src tests
```

Expected: exactly two hits, both an iterator result (`r.done`), neither a `mkfs` done field: `src/components/StringsPanel.svelte:68` and `tests/strings.test.ts:9`.

- [ ] **Step 26: Run the tests.**

```sh
pnpm test
```

Expected: `Test Files  39 passed (39)`, `Tests  329 passed (329)`.

#### Round 4: the store and the components

The chrome's own sector words move to the family's sector noun: the Inspector's rows (below), the dump's labels and `g` prompt, the Step panel's `Sector N` buttons, the ribbon's hover caption, and the annotation tooltip, whose text comes from `core/annotationFormat.ts`. (`grep -rniE "sector" src/components` lists the rest: variable names, comments, and `volume.sectorSize`, none of them shown to the learner.) Only the tooltip's helper is node-testable; the Svelte runes files are not, so `svelte-check` (in `pnpm build`) is their gate, then the browser.

- [ ] **Step 27: Write the failing tooltip test.** In `web/ui/tests/annotationFormat.test.ts`, inside `describe("describeRange", ...)`, add after its test:

```ts

  it("names the family's sector when given one", () => {
    expect(describeRange({ start: 0x38, end: 0x3a }, "block")).toBe("bytes 56..58 of the block (decimal)");
  });
```

- [ ] **Step 28: Run it and watch it fail.**

```sh
pnpm test tests/annotationFormat.test.ts
```

Expected: `Tests  1 failed | 8 passed (9)`: `expected 'bytes 56..58 of the sector (decimal)' to be 'bytes 56..58 of the block (decimal)'`.

- [ ] **Step 29: Let the tooltip take the noun.** In `web/ui/src/core/annotationFormat.ts` replace `describeRange` and its comment with:

```ts
/** The tooltip behind a range: the same bounds in decimal, in the family's sector noun (the
 *  Inspector passes `sector.singular`: "sector" on FAT, "block" on ext). */
export function describeRange(r: ByteRange, sector = "sector"): string {
  return `bytes ${r.start}..${r.end} of the ${sector} (decimal)`;
}
```

The default keeps the existing call and its pinned `bytes 11..13 of the sector (decimal)` unchanged.

- [ ] **Step 30: Mirror `needsRecovery` and the armed crash phase in the store.** Arming a crash is not an op: it moves neither the timeline nor `epoch`, so nothing reactive would tell the Journal panel (Task 5) that the terminal's `crash` (Task 6) armed one, or the reverse. The store keeps the phase as state and is the one place both arm through. In `web/ui/src/state/volume.svelte.ts` replace the import `import type { FsAdapter, FsFamilyId } from "../fs/adapter";` with:

```ts
import type { CrashPhase, FsAdapter, FsFamilyId } from "../fs/adapter";
```

add after the `corruption` field:

```ts
  /** True while the journal holds an unfinished transaction (the adapter's `needsRecovery`;
   *  always false on FAT and ext2). Refreshed with the adapter, like `corruption`. */
  needsRecovery = $state(false);
  /** The crash phase armed in the mounted journal, or null (always null without a journal).
   *  Arming is not an op, so it moves neither the timeline nor `epoch`: this field is how the
   *  Journal panel shows a phase the terminal's `crash` armed, and the reverse. `refreshMeta`
   *  re-reads it, because the next op uses the phase up and a format or load binds a new journal. */
  armedPhase = $state<CrashPhase | null>(null);
```

and replace the closing of `refreshMeta()` (from `this.corruption = this.vol.corruption();` through its closing brace) with:

```ts
    this.corruption = this.vol.corruption();
    this.needsRecovery = this.adapter.needsRecovery;
    this.armedPhase = this.adapter.journal?.phase() ?? null;
  }

  /** Arm a crash in the mounted journal, so its next op stops at `phase`, or disarm it (`null`).
   *  Without a journal there is nothing to arm. Returns false, with `status` set, when the volume
   *  refuses. */
  setArmedPhase(phase: CrashPhase | null): boolean {
    const journal = this.adapter.journal;
    if (!journal) return true;
    try {
      if (phase === null) journal.disarm();
      else journal.arm(phase);
    } catch (e) { this.fail(e); return false; }
    this.armedPhase = journal.phase();
    return true;
  }
```

`setArmedPhase` follows the store's rule that it never throws (failure lands in `status`, like `run`); it returns whether it worked so Task 6's store host can turn a refusal back into a thrown error. Nothing calls it yet, and on FAT `armedPhase` stays `null`. The name is the store's own rather than the wasm's `armCrash`/`disarmCrash`, so the boundary scan (`tests/adapterBoundary.test.ts`, Task 3) can keep flagging every `.armCrash(` and `.disarmCrash(` outside `src/fs/ext/`.

- [ ] **Step 31: Name the Inspector's address row by the sector noun.** In `web/ui/src/components/Inspector.svelte` add after the `attrAtOffset` import:

```ts
  import { unitIsSector } from "../fs/adapter";
```

add after the `cap` helper:

```ts

  // Where the family's unit is its sector (ext: both are the block) the unit row would repeat
  // the address row, so it folds into it: `Block 1111 · data (group 0) · data block 0 of /a`.
  const oneNoun = $derived.by(() => { volume.epoch; return unitIsSector(volume.adapter); });
```

and replace the `Sector` row and the unit row with:

```svelte
      <dt>{cap(volume.adapter.sector.singular)}</dt><dd class="mono">{attr.sector} · {attr.regionName}{#if oneNoun && unitNote}{" "}· {unitNote}{/if}</dd>
      {#if attr.unit !== undefined && !oneNoun}<dt>{cap(volume.adapter.unit.singular)}</dt><dd class="mono">{attr.unit}{#if unitNote}{" "}· {unitNote}{/if}</dd>{/if}
```

and give the annotation range's tooltip the noun, replacing its `<span class="muted" ...>` line with:

```svelte
          <span class="muted" title={describeRange(a.range, volume.adapter.sector.singular)}>{formatRange(a.range, volume.sectorSize)}</span>
```

On FAT this renders `Sector` / `97 · data` and `Cluster` / `2 · FAT: free`, and the tooltip `bytes 11..13 of the sector (decimal)`, exactly as before.

- [ ] **Step 32: Label the dump's rows and the `g` prompt by the nouns.** In `web/ui/src/components/HexView.svelte` add after the `parseAddr` import:

```ts
  import { unitIsSector } from "../fs/adapter";
```

replace the gap row's return with:

```ts
      return { kind: "gap" as const, key: r, segment, text: `· · · ${segment.sectorCount.toLocaleString()} empty ${volume.adapter.sector.plural} (${fmtBytes(bytes)}) · · ·` };
```

replace the two lines that read the unit noun and build `label` (keeping the `unitStart` line between them) with:

```ts
    const { unit, sector } = volume.adapter;
    const unitStart = attr.unit !== undefined && sectorStart && volume.adapter.unitStartsAt(attr.sector);
    const label = unitStart ? `${unit.singular} ${attr.unit}${attr.ownerPath ? ` · ${attr.ownerPath}` : ""}` : sectorStart ? `${sector.singular} ${attr.sector}${attr.unit === undefined ? ` · ${attr.regionName}` : ""}` : "";
```

replace the `case "g":` line with:

```ts
      case "g": { const v = globalThis.prompt(`Jump to offset (${jumpForms()})`); if (v) jump(v); return; }
```

and add before `function jump(v: string)`:

```ts
  /** The `g` prompt's forms: FAT `0x…, decimal, s:sector, c:cluster`; where the unit is the
   *  sector (ext) the one noun: `0x…, decimal, b:block`. */
  function jumpForms(): string {
    const { sector, unit } = volume.adapter;
    const unitForm = unitIsSector(volume.adapter) ? "" : `, ${unit.letter}:${unit.singular}`;
    return `0x…, decimal, ${sector.letter}:${sector.singular}${unitForm}`;
  }
```

- [ ] **Step 33: Name the Step panel's buttons and the ribbon's caption by the sector noun.** In `web/ui/src/components/StepPanel.svelte` add after the `sectors` derived:

```ts
  // The buttons name a sector in the family's noun: "Sector 1" on FAT, "Block 1" on ext.
  const noun = $derived.by(() => { volume.epoch; const s = volume.adapter.sector.singular; return s[0].toUpperCase() + s.slice(1); });
```

and replace the button's label `Sector {sector}` so the button reads:

```svelte
          <button onclick={() => selection.jumpTo(sector * volume.sectorSize)}>{noun} {sector}</button>
```

In `web/ui/src/components/Ribbon.svelte` replace the body of the `caption` derived with:

```ts
    if (hoverCol === null) return "";
    volume.epoch;
    const total = volume.totalSectors;
    const sector = Math.min(hoverCol * sectorsPerCol, Math.max(0, total - 1));
    const attr: Attr = attrAtSector(volume.attribution, sector);
    return `${volume.adapter.sector.singular} ${sector} · ${attr.ownerPath ?? attr.regionName}`;
```

Both read `volume.epoch` before the adapter (the reactivity rule). On FAT the buttons still read `Sector 1` and the caption `sector 0 · reserved (boot sector)`.

- [ ] **Step 34: Teach the status line `NeedsRecovery`.** In `web/ui/src/components/StatusLine.svelte` add to the comment above `FRIENDLY`, after its third line:

```ts
  // `Unsupported` is left out on purpose: the loader's messages already name what was
  // refused ("journal feature journal_64bit"), and a fixed phrase would hide it.
```

replace the `Unsupported: "That isn't supported yet.",` entry with:

```ts
    NeedsRecovery: "This volume needs recovery: the journal holds an unfinished transaction. Recover it from the Journal panel or with recover.",
```

and add after the `volume.corruption` span:

```svelte
  {#if volume.needsRecovery}<span>Volume needs recovery</span>{/if}
```

#### Round 5: the shared canvas grid

The FAT map is the first of three canvas maps (Task 4 adds the block-group map, Task 5 the Journal ring), and each would otherwise repeat the same canvas prologue, the same cell arithmetic, the same marks, and the same `ResizeObserver` effect. They live once in `core/`: the maths is node-testable, and the drawing helpers take the context and a colour, so each map keeps its own cell size and colours. The FAT map adopts them here with no visible change: its 6 px cells, 1 px gap, 2 px end-of-chain dot at inset 2, 1 px outlines, 2 px chain join, and dashed hover cell are exactly the calls it made before, in the same order. `prepareCanvas` sizes a canvas in CSS pixels and backs it at `devicePixelRatio`, as the ext maps need for their text; the FAT map keeps its CSS size, and at a ratio of 1 its backing store is the one it had, while at 2 each CSS pixel it drew is drawn as the same colour at the device's resolution. The width tracking is a Svelte action (`use:observeWidth`), which every Svelte 5 has, so `package.json`'s `"svelte": "^5.0.0"` stays true.

- [ ] **Step 35: Write the failing grid tests.** Create `web/ui/tests/grid.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { cellAt, cellRect, gridCols, gridRows } from "../src/core/grid";

describe("the canvas cell grid", () => {
  it("fits whole cells and their gaps to the width, never fewer than one column or row", () => {
    expect(gridCols(198, 6, 1)).toBe(28); // the FAT map's 6 px cells in the sidebar's 198 px
    expect(gridCols(198, 4, 1)).toBe(39); // 4 px cells
    expect(gridCols(3, 4, 1)).toBe(1);
    expect(gridCols(0, 6, 1)).toBe(1); // before the first measurement
    expect(gridRows(8167, 28)).toBe(292); // the default FAT16 disk's clusters
    expect(gridRows(1024, 39)).toBe(27);
    expect(gridRows(0, 39)).toBe(1);
  });

  it("places cell i left to right, wrapping at the column count", () => {
    expect(cellRect(0, 39, 4, 1)).toEqual({ x: 0, y: 0 });
    expect(cellRect(38, 39, 4, 1)).toEqual({ x: 190, y: 0 });
    expect(cellRect(39, 39, 4, 1)).toEqual({ x: 0, y: 5 });
    expect(cellRect(1023, 39, 4, 1)).toEqual({ x: 45, y: 130 }); // 1023 = 26 * 39 + 9
    expect(cellRect(29, 28, 6, 1)).toEqual({ x: 7, y: 7 });
  });

  it("finds the cell under a point, the gap after a cell counting as the cell", () => {
    const at = (px: number, py: number) => cellAt(px, py, 39, 1024, 4, 1);
    expect(at(0, 0)).toBe(0);
    expect(at(194, 4)).toBe(38);
    expect(at(2, 7)).toBe(39);
    expect(at(47, 132)).toBe(1023);
    expect(at(50, 130)).toBeNull(); // past the last cell on the last row
    expect(at(196, 0)).toBeNull(); // right of the last column
    expect(at(0, 135)).toBeNull(); // below the last row
    expect(at(-1, 0)).toBeNull();
    expect(at(0, -1)).toBeNull();
    for (const i of [0, 1, 38, 39, 500, 1023]) {
      const { x, y } = cellRect(i, 39, 4, 1);
      expect(at(x, y), `cell ${i}`).toBe(i);
      expect(at(x + 4, y + 4), `cell ${i}'s gap`).toBe(i);
    }
  });
});
```

- [ ] **Step 36: Run them and watch them fail.**

```sh
pnpm exec vitest run tests/grid.test.ts
```

Expected: `Test Files  1 failed (1)`, `Tests  no tests`, with `Error: Cannot find module '../src/core/grid' imported from '.../web/ui/tests/grid.test.ts'`.

- [ ] **Step 37: Write the grid and the width action.** Create `web/ui/src/core/grid.ts`:

```ts
/**
 * The cell grids the maps draw on a canvas (the FAT map's clusters, each band of the ext
 * block-group map, the Journal panel's ring): `count` square cells of side `cell`, each
 * followed by a `gap`, filled left to right and wrapped to a width. The maths is pure so the
 * node tests can check it; the drawing helpers are the marks every map puts on a cell.
 */

/** Size `canvas` to `width` × `height` CSS pixels at the device's pixel ratio, so what a map
 *  draws stays sharp, and hand back its context scaled to CSS pixels and cleared (null when the
 *  canvas has no 2D context). Every canvas map starts its paint with it. */
export function prepareCanvas(canvas: HTMLCanvasElement, width: number, height: number): CanvasRenderingContext2D | null {
  const dpr = window.devicePixelRatio || 1;
  canvas.width = Math.round(width * dpr);
  canvas.height = Math.round(height * dpr);
  canvas.style.width = `${width}px`;
  canvas.style.height = `${height}px`;
  const ctx = canvas.getContext("2d");
  if (!ctx) return null;
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
  ctx.clearRect(0, 0, width, height);
  return ctx;
}

/** How many cells fit across `width`: never fewer than one, so a map lays out before its
 *  first width measurement. */
export function gridCols(width: number, cell: number, gap: number): number {
  return Math.max(1, Math.floor(width / (cell + gap)));
}

/** How many rows `count` cells take in `cols` columns: never fewer than one. */
export function gridRows(count: number, cols: number): number {
  return Math.max(1, Math.ceil(count / cols));
}

/** The top-left corner of cell `index`. */
export function cellRect(index: number, cols: number, cell: number, gap: number): { x: number; y: number } {
  const pitch = cell + gap;
  return { x: (index % cols) * pitch, y: Math.floor(index / cols) * pitch };
}

/** The index of the cell under `(px, py)`, the gap after a cell counting as the cell; null
 *  left of or above the grid, right of its last column, or past its last cell. */
export function cellAt(px: number, py: number, cols: number, count: number, cell: number, gap: number): number | null {
  const pitch = cell + gap;
  const col = Math.floor(px / pitch), row = Math.floor(py / pitch);
  if (col < 0 || col >= cols || row < 0) return null;
  const i = row * cols + col;
  return i < count ? i : null;
}

/** A 1 px outline just inside a cell (the diff, a chain's cells). */
export function outlineCell(ctx: CanvasRenderingContext2D, x: number, y: number, cell: number, color: string): void {
  ctx.strokeStyle = color;
  ctx.lineWidth = 1;
  ctx.strokeRect(x + 0.5, y + 0.5, cell - 1, cell - 1);
}

/** A 2 px dot in the middle of a cell (FAT's end-of-chain mark, ext's pointer blocks). */
export function dotCell(ctx: CanvasRenderingContext2D, x: number, y: number, cell: number, color: string): void {
  const inset = (cell - 2) / 2;
  ctx.fillStyle = color;
  ctx.fillRect(x + inset, y + inset, 2, 2);
}

/** A dashed outline: the cell holding the byte the mouse is over in the hex dump. */
export function dashCell(ctx: CanvasRenderingContext2D, x: number, y: number, cell: number, color: string): void {
  ctx.save();
  ctx.setLineDash([1, 1]);
  outlineCell(ctx, x, y, cell, color);
  ctx.restore();
}

/** The selected file's chain: each cell outlined, then the cells' centres joined in order by a
 *  line `joinWidth` px wide. */
export function drawChain(ctx: CanvasRenderingContext2D, cells: readonly { x: number; y: number }[], cell: number, color: string, joinWidth: number): void {
  for (const c of cells) outlineCell(ctx, c.x, c.y, cell, color);
  ctx.lineWidth = joinWidth;
  ctx.beginPath();
  cells.forEach((c, i) => {
    const cx = c.x + cell / 2, cy = c.y + cell / 2;
    if (i === 0) ctx.moveTo(cx, cy); else ctx.lineTo(cx, cy);
  });
  ctx.stroke();
}
```

and `web/ui/src/core/observeWidth.ts` (a Svelte action, which every Svelte 5 supports; `update` takes a new callback if the parameter changes):

```ts
import type { ActionReturn } from "svelte/action";

/**
 * A Svelte action that reports an element's width now and its content width on every resize,
 * so a canvas map wraps to the column it sits in (a resizable layout, a narrower viewport)
 * rather than to a width read once at mount: `<div use:observeWidth={(w) => (width = w)}>`.
 * The observer goes when the element does.
 */
export function observeWidth(el: HTMLElement, onWidth: (width: number) => void): ActionReturn<(width: number) => void> {
  let report = onWidth;
  report(el.getBoundingClientRect().width);
  const ro = new ResizeObserver((entries) => report(entries[0].contentRect.width));
  ro.observe(el);
  return {
    update(next) { report = next; },
    destroy() { ro.disconnect(); },
  };
}
```

- [ ] **Step 38: Run them and watch them pass.**

```sh
pnpm exec vitest run tests/grid.test.ts
```

Expected: `Test Files  1 passed (1)`, `Tests  3 passed (3)`.

- [ ] **Step 39: Put the FAT map on the grid.** In `web/ui/src/fs/fat16/FatMap.svelte` add after `import { attrAtOffset } from "../../core/attribution";`:

```ts
  import { cellAt, cellRect, dashCell, dotCell, drawChain, gridCols, gridRows, outlineCell, prepareCanvas } from "../../core/grid";
  import { observeWidth } from "../../core/observeWidth";
```

replace

```ts
  let wrap = $state<HTMLDivElement>();
  let canvas = $state<HTMLCanvasElement>();
  let width = $state(0);
```

with

```ts
  let canvas = $state<HTMLCanvasElement>();
  // The panel's content width, kept current by `observeWidth` on the wrapper so `cols` follows
  // the sidebar's actual size (a resizable layout, a narrower viewport, …) rather than a value
  // baked in at mount.
  let width = $state(0);
```

replace

```ts
  const cols = $derived(Math.max(1, Math.floor(width / (CELL + GAP))));
  const rows = $derived(Math.max(1, Math.ceil(clusterCount / cols)));
```

with

```ts
  const cols = $derived(gridCols(width, CELL, GAP));
  const rows = $derived(gridRows(clusterCount, cols));
```

delete the width-tracking effect and the blank line after it (from ``// Track the panel's content width so `cols` follows the sidebar's actual size`` through the effect's closing `});`), replace `cellRect` and `clusterAtPoint` with

```ts
  /** Cluster `c`'s cell: the grid starts at cluster 2, the first data cluster. */
  function clusterCell(c: number): { x: number; y: number } {
    return cellRect(c - 2, cols, CELL, GAP);
  }

  function clusterAtPoint(px: number, py: number): number | null {
    const i = cellAt(px, py, cols, clusterCount, CELL, GAP);
    return i === null ? null : i + 2;
  }
```

in `paint()` replace its prologue

```ts
    canvas.width = canvasWidth;
    canvas.height = canvasHeight;
    const ctx = canvas.getContext("2d");
    if (!ctx) return;
    ctx.clearRect(0, 0, canvas.width, canvas.height);
```

with

```ts
    const ctx = prepareCanvas(canvas, canvasWidth, canvasHeight);
    if (!ctx) return;
```

replace the cluster loop's `const { x, y } = cellRect(c);` with `const { x, y } = clusterCell(c);`, and replace everything from `if (state === "end") {` down to the closing brace of the `if (chain.length)` block with

```ts
      if (state === "end") dotCell(ctx, x, y, CELL, ink);
      if (overlapsDiff(c)) outlineCell(ctx, x, y, CELL, diffColor);
    }

    if (layers.chain.length) drawChain(ctx, layers.chain.map(clusterCell), CELL, focus, 2);
```

replace the hover outline's body

```ts
        const { x, y } = cellRect(c);
        ctx.save();
        ctx.strokeStyle = focus;
        ctx.setLineDash([1, 1]);
        ctx.lineWidth = 1;
        ctx.strokeRect(x + 0.5, y + 0.5, CELL - 1, CELL - 1);
        ctx.restore();
```

with

```ts
        const { x, y } = clusterCell(c);
        dashCell(ctx, x, y, CELL, focus);
```

and in the markup replace `<div class="fatmap-wrap" bind:this={wrap} style:max-height="{MAX_HEIGHT}px">` with

```svelte
  <div class="fatmap-wrap" use:observeWidth={(w) => (width = w)} style:max-height="{MAX_HEIGHT}px">
```

The node tests cannot load the component; `svelte-check` (Step 47) type-checks it and Step 48 looks at it.

#### Round 6: three chrome faults an ext volume shows

Three pieces of the shared chrome misbehave on an ext volume, and none of them changes what FAT shows. The ribbon's free label counts every unit no file owns, which on ext includes the inode tables, the bitmaps, and the journal (a fresh ext3 disk would read `16.0 MB free` where `df` counts 15,205 free blocks); it asks the family instead, through a new `FsAdapter.freeUnits()`. The shared `SpaceAdapter` answers it with the old count (the units from `unit.first` with no owner row), so FAT's label stays exactly what it was on every FAT volume, including one with a bad cluster or a lost chain, where FAT's own `df()` would count them used; Task 3's ext adapter overrides it with `df().free`. The Step strip has no height limit, and an ext3 Add file lists 18 block buttons and 30 events; it stops at 210 px and scrolls, a cap the FAT actions never reach. And the dump's scroll nonce restarts at 1 when `selection.reset()` runs, while `HexView` keeps the last nonce it scrolled to, so the first jump after a reset is ignored whenever the jump before it also carried nonce 1 (starting a lesson right after closing another at its first step leaves the dump where it was); the store now numbers its requests with one counter that `reset()` leaves running. The store runs on runes, which the node tests cannot load, so the counter is a plain class in `core/` and a source scan pins that the store keeps one.

- [ ] **Step 40: Write the failing tests.** Create `web/ui/tests/freeSpace.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { Volume } from "../src/lib/wasm";
import { buildAttribution } from "../src/core/attribution";
import { freeSpaceLabel } from "../src/core/freeSpace";
import { adapterFor } from "../src/fs";

/** The label the ribbon printed before it asked the family: every unit no file owns, counted free. */
function ownerlessLabel(vol: Volume): string {
  const fs = adapterFor(vol);
  const { ownerByUnit } = buildAttribution(fs, vol.layout(), fs.owners);
  let free = 0;
  for (let u = fs.unit.first; u < fs.unit.first + fs.unitCount; u++) if (ownerByUnit[u] < 0) free++;
  return `${((free * fs.unitSize) / (1024 * 1024)).toFixed(1)} MB free`;
}

describe("freeSpaceLabel", () => {
  it("gives the FAT default disk the label the owner count gave it", () => {
    const vol = Volume.formatFat16(undefined);
    expect(freeSpaceLabel(adapterFor(vol))).toBe(ownerlessLabel(vol));
    expect(freeSpaceLabel(adapterFor(vol))).toBe("16.0 MB free");
  });

  it("still agrees with the owner count on FAT once a directory and a file hold clusters", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createDir("/DOCS");
    vol.createFile("/DOCS/N.TXT", new Uint8Array(300_000));
    expect(freeSpaceLabel(adapterFor(vol))).toBe(ownerlessLabel(vol));
  });

  it("counts FAT's free clusters as the ones no owner row claims", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createDir("/DOCS");
    vol.createFile("/DOCS/N.TXT", new Uint8Array(300_000));
    const fs = adapterFor(vol);
    expect(fs.owners.length).toBeGreaterThan(1);
    expect(fs.freeUnits()).toBe(fs.unitCount - fs.owners.length);
  });

  it("prints the free units in MiB to one decimal", () => {
    expect(freeSpaceLabel({ freeUnits: () => 15205, unitSize: 1024 })).toBe("14.8 MB free");
  });
});
```

Create `web/ui/tests/scrollNonce.test.ts`:

```ts
import { readFileSync } from "node:fs";
import { describe, expect, it } from "vitest";
import { ScrollNonces } from "../src/core/scrollNonce";

describe("ScrollNonces", () => {
  it("numbers every request past the one before it", () => {
    const n = new ScrollNonces();
    expect([n.next(), n.next(), n.next()]).toEqual([1, 2, 3]);
  });

  it("is the selection store's one counter, and reset() leaves it counting", () => {
    // The store runs on runes, which the node tests cannot load, so this reads its source, as
    // layout.test.ts reads app.css. A counter made anew in reset() would restart at 1, the nonce
    // HexView last scrolled to, and the first jump after a lesson start would not scroll.
    const src = readFileSync(new URL("../src/state/selection.svelte.ts", import.meta.url), "utf8");
    expect(src.match(/new ScrollNonces\(\)/g)).toHaveLength(1);
    expect(src).toContain("nonce: this.nonces.next()");
    const at = src.indexOf("  reset() {");
    expect(at, "selection.svelte.ts has no `  reset() {`: update this scan").toBeGreaterThanOrEqual(0);
    const reset = src.slice(at);
    expect(reset.slice(0, reset.indexOf("\n  }\n"))).not.toMatch(/nonces|ScrollNonces/);
  });
});
```

In `web/ui/tests/layout.test.ts` add inside `describe("app.css layout guards", ...)`, after the `.app.term-side` test (so it sits before Step 12's dump-row guard, which stays the last test):

```ts
  it("caps the Step strip's height and scrolls it, so a long step cannot push the grid down", () => {
    // An ext3 Add file lists 18 block buttons and 30 events; uncapped, the strip grew to 571 px in a
    // 760 px window. 210 px is about six rows of buttons, more than the FAT actions fill.
    const step = rule(".step");
    expect(step).toContain("max-height: 210px");
    expect(step).toContain("overflow: auto");
  });
```

- [ ] **Step 41: Run them and watch them fail.**

```sh
pnpm exec vitest run tests/freeSpace.test.ts tests/scrollNonce.test.ts tests/layout.test.ts
```

Expected: `Test Files  3 failed (3)`, `Tests  1 failed | 3 passed (4)`: `tests/freeSpace.test.ts` and `tests/scrollNonce.test.ts` fail to load (`Error: Cannot find module '../src/core/freeSpace'` and `'../src/core/scrollNonce'`), and the Step strip guard fails with `AssertionError: expected ' display: flex; flex-wrap: wrap; alig…' to contain 'max-height: 210px'`.

- [ ] **Step 42: Write the family's free count and the two helpers.** In `web/ui/src/fs/adapter.ts` add after `df(): DfFacts;` in `FsAdapter`:

```ts
  /** The units the ribbon counts free. FAT: the units no owner row claims, the count the ribbon
   *  always made (a bad cluster or a lost chain stays free there). ext: `df().free`, because the
   *  inode tables, the bitmaps, and the journal have no owner rows and are not free. */
  freeUnits(): number;
```

In `web/ui/src/fs/base.ts` add after `ownerOf` (its closing brace, then a blank line, then this, so `regionStart`'s comment follows after another blank line):

```ts
  /** The units from `unit.first` through the last with no owner row: the ribbon's free count.
   *  A family whose unowned units are not all free (ext) overrides it. */
  freeUnits(): number {
    const owned = new Set(this.owners.map((o) => o.unit));
    let free = 0;
    for (let u = this.unit.first; u < this.unit.first + this.unitCount; u++) if (!owned.has(u)) free++;
    return free;
  }
```

`Fat16Adapter` inherits it: its owner rows are the clusters `clusterOwners` reports, so this is the count the ribbon made from the attribution table's `ownerByUnit`. Create `web/ui/src/core/freeSpace.ts`:

```ts
import type { FsAdapter } from "../fs/adapter";

/** The ribbon's free-space label, from the family's free count (`freeUnits`): on FAT the units
 *  no owner row claims, the count the ribbon always made, so FAT's label is the one it always
 *  was; on ext the superblock's count, because counting the blocks without an owner row would
 *  call the inode tables, the bitmaps, and the journal free (`16.0 MB free` on a fresh ext3 disk
 *  where 15,205 blocks are free). */
export function freeSpaceLabel(fs: Pick<FsAdapter, "freeUnits" | "unitSize">): string {
  return `${((fs.freeUnits() * fs.unitSize) / (1024 * 1024)).toFixed(1)} MB free`;
}
```

Create `web/ui/src/core/scrollNonce.ts`:

```ts
/**
 * Numbers the dump's scroll requests. HexView scrolls when a request's nonce differs from the
 * last one it acted on, and it remembers that nonce for as long as it is mounted: across a
 * format, a load, and a lesson start. So the numbering never restarts. When the selection's
 * reset began again at 1, the first jump after it carried the nonce of the first jump before
 * it, and the dump stayed where it was (close a lesson at its first step, start another).
 */
export class ScrollNonces {
  private last = 0;

  /** One more than every nonce issued so far. */
  next(): number { return ++this.last; }
}
```

- [ ] **Step 43: Read the ribbon's free label from the family.** In `web/ui/src/components/Ribbon.svelte` add after `import { attrAtSector, type Attr } from "../core/attribution";`:

```ts
  import { freeSpaceLabel } from "../core/freeSpace";
```

and replace the `freeLabel` derived

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

with

```ts
  // The family's free count: on FAT the units no file owns, as always; on ext the superblock's,
  // because the inode tables, the bitmaps, and the journal have no owner and are not free.
  const freeLabel = $derived.by(() => { volume.epoch; return freeSpaceLabel(volume.adapter); });
```

It reads `volume.epoch` before the adapter (the reactivity rule); `freeUnits()` answers from the adapter's caches, which `refresh()` renews after every op.

- [ ] **Step 44: Number the scroll requests with one counter.** In `web/ui/src/state/selection.svelte.ts` add as the first line:

```ts
import { ScrollNonces } from "../core/scrollNonce";
```

and replace everything from the `scrollTarget` field through `jumpTo`

```ts
  scrollTarget = $state<{ offset: number; nonce: number } | null>(null);

  /** Move the byte cursor and ask the dump to scroll there.
   *  Reads `scrollTarget` before writing it; callers inside an `$effect` must wrap
   *  the call in `untrack()` so the effect does not depend on its own write. */
  jumpTo(offset: number) { this.cursorOffset = offset; this.scrollTarget = { offset, nonce: (this.scrollTarget?.nonce ?? 0) + 1 }; }
```

with

```ts
  scrollTarget = $state<{ offset: number; nonce: number } | null>(null);
  // One counter for the store's life: `reset()` leaves it running, because the dump remembers
  // the last nonce it scrolled to across a reset (see `ScrollNonces`).
  private readonly nonces = new ScrollNonces();

  /** Move the byte cursor and ask the dump to scroll there, even to the offset it is at: every
   *  request carries a fresh nonce. Reads no state, so an `$effect` may call it. */
  jumpTo(offset: number) { this.cursorOffset = offset; this.scrollTarget = { offset, nonce: this.nonces.next() }; }
```

`reset()` is unchanged: it still sets `scrollTarget` to `null`, and the next `jumpTo` numbers past every nonce before it. `jumpTo` no longer reads `scrollTarget`, so the note in `web/ui/src/state/navigate.svelte.ts` about its read-then-write goes: replace

```ts
 * alone. These are event handlers, not effects, so `jumpTo`'s read-then-write of
 * `scrollTarget` needs no `untrack`.
```

with

```ts
 * alone.
```

- [ ] **Step 45: Cap the Step strip.** In `web/ui/src/app.css` replace the Step strip's comment and rule

```css
/* Step strip (under the top bar): one wrapping row of heading, counter, op, sector buttons,
   and events. */
.step { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 14px; padding: 5px 10px; }
```

with

```css
/* Step strip (under the top bar): one wrapping row of heading, counter, op, sector buttons,
   and events. It scrolls past about six rows of buttons (210 px), which the FAT actions never
   reach; an ext3 Add file's 18 block buttons and 30 events would push the grid down. */
.step { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 14px; padding: 5px 10px; max-height: 210px; overflow: auto; }
```

The FAT default disk's Add file fills 83 px of the strip at 1440 px wide and 145 px at 760 px, under the cap, so it looks as before.

- [ ] **Step 46: Run them and watch them pass.**

```sh
pnpm exec vitest run tests/freeSpace.test.ts tests/scrollNonce.test.ts tests/layout.test.ts
```

Expected: `Test Files  3 passed (3)`, `Tests  10 passed (10)`.

#### Gates, browser, and commit

- [ ] **Step 47: Run the web gates.**

```sh
CI=true pnpm install --frozen-lockfile && pnpm test && pnpm build
git status --short
```

Expected: `pnpm test` prints `Test Files  42 passed (42)` and `Tests  340 passed (340)`; `pnpm build` prints `svelte-check ... 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS` and vite's `✓ built in ...`; `git status --short` lists only the files in **Files** (no `pnpm-lock.yaml`, no `pnpm-workspace.yaml`).

- [ ] **Step 48: Check the FAT chrome in the browser.** `pnpm dev`, open the printed URL, and confirm on the default FAT16 disk: the Inspector on byte 0 shows `Sector` / `0 · reserved (boot sector)`, and hovering an annotation's range shows `bytes … of the sector (decimal)`; on a data byte (press `g`, enter `c:2`) `Sector` / `97 · data`, `Cluster` / `2 · FAT: free`, `Owner` / `free`; the `g` prompt reads `Jump to offset (0x…, decimal, s:sector, c:cluster)`; the dump labels `cluster 2` and the gap `63 empty sectors (31.5 KB)`; after Add file the Step panel's buttons read `Sector N` as before and the strip is its old height (83 px at 1440 px wide, no scrollbar); the ribbon reads `16.0 MB free` on the fresh disk and `15.9 MB free` after Add file, as before; hovering the ribbon captions `sector N · …`; the status line is empty; the console has no errors. `--own-11` computes to `#d4a017` (dark) and `#b7791f` (light); nothing on a FAT disk uses it yet. The FAT map looks exactly as before: its canvas is 198 × 2044 CSS px in the default layout (28 columns of 6 px cells, 292 rows; its backing store is that times `devicePixelRatio`), the end-of-chain dots, the selected chain's outlines and 2 px join, the diff outlines, and the dashed cell under a dump hover draw as they did, and narrowing the window re-wraps it. Start a lesson, scroll the dump far down, Close the lesson at its first step, and Start another: the dump scrolls back to the new lesson's first focus (before Step 44 it stayed where it was).

- [ ] **Step 49: Commit.** From the repo root:

```sh
cd ../..
git add web/ui/src web/ui/tests
git commit -m "refactor(ui): family-neutral seam additions for a second filesystem family"
```

---

### Task 3: the ext adapter behind the seam (not yet registered)

Spec: `docs/superpowers/specs/2026-09-24-ext-explorer-design.md` sections 1 (`ExtFamilyOptions`, `ext.format`, the ext2-with-journal-keys error, `FsAdapter.name` is the volume's type), 2 (the seam this adapter implements: `sector`, `fileParts`, `role`, `needsRecovery`, `journal`, `extraAddrHelp`), 4 (all of it), 5 (the Format form's sizes and the five `checkFormat` problems), 6 (ext's six `mkfs` flags and their descriptions), 8 (`tests/fixtures/extGeometry.ts`, `tests/fs/ext.test.ts`, `tests/fs/ext-format.test.ts`, `tests/attribution.test.ts`, `tests/lesson.test.ts`, `tests/adapterBoundary.test.ts`), 10 (journal blocks are never units; indirect blocks are owner rows).

The ext family lives under `web/ui/src/fs/ext/` and is complete here, but nothing registers it: `FsFamilyId` stays `"fat16"`, `FAMILIES` and `PANELS` are untouched (Task 4 adds `"ext"` to the union, registers `ext`, and adds its panels, because a `Record<FsFamilyId, ...>` entry for ext needs the Svelte panels). Until then the family's id is the cast `"ext" as string as FsFamilyId` (one constant, `EXT_ID` in `adapter.ts`, commented), `asExt` compares `(fs.id as string) !== "ext"`, and the tests bind with `ext.bind(vol)`. `ExtAdapter` extends Task 2's `SpaceAdapter` (`fs/base.ts`) for the `UnitSpace` members, `ownerOf`, and `regionStart`, as `Fat16Adapter` does, and takes `hexAddr` from there. No FAT file changes and no FAT expectation is edited; the three shared test files only gain ext imports and new `it`s. Round 6 changes one generic file, `core/tree.ts`, so the tree's root takes its first unit from an owner row when the family has one (ext's root directory, block 69; FAT has none and keeps `null`), adds one assertion, no changed expectation, to the FAT tree test in `tests/integration.test.ts`, and gives `ExtAdapter` its own `freeUnits()` (the superblock's free count, `df().free`) in place of `SpaceAdapter`'s count of unowned blocks, so the ribbon reads `14.8 MB free` on the default ext3 disk. Nothing renders ext yet, so the browser is not part of this task.

**Files**

- Create: `web/ui/src/fs/ext/geometry.ts` (1–34), `web/ui/src/fs/ext/format.ts` (1–105), `web/ui/src/fs/ext/metadata.ts` (1–39), `web/ui/src/fs/ext/journal.ts` (1–45), `web/ui/src/fs/ext/adapter.ts` (1–272; Round 6's `freeUnits` 188–192), `web/ui/src/fs/ext/index.ts` (1–19)
- Modify: `web/ui/src/lib/wasm.ts` (6–11: the type-only ext re-exports), `web/ui/src/core/tree.ts` (7–9 the doc comment, 22 the root's `firstUnit`)
- Create (test data): `web/ui/tests/fixtures/extGeometry.ts` (1–36)
- Test: `web/ui/tests/fs/ext.test.ts` (1–514, new; Round 6's import, root test, and free-label test included), `web/ui/tests/fs/ext-format.test.ts` (1–127, new), `web/ui/tests/attribution.test.ts` (3, 7, 15–21, 83–107), `web/ui/tests/lesson.test.ts` (4, 63–73), `web/ui/tests/adapterBoundary.test.ts` (6–11, 24–34, 61–71, 114–150), `web/ui/tests/integration.test.ts` (80–82)
- Not touched: `web/ui/src/fs/{adapter,base,index,panels}.ts`, everything under `web/ui/src/fs/fat16/`, every `core/` file but `tree.ts`, `crates/`.

**Interfaces**

Consumes (Task 1, on `Volume` and in `fs-emulator-wasm`'s types): `extGeometry(): ExtGeometry`, `extSuperblock(): ExtSuperblock`, `blockOwners(): ExtBlockOwner[]` (ascending; the journal's rows have `path: "<journal>"`: data blocks 82..1105 `role: "journal"`, pointer blocks 1106..1110 `role: "indirect"`), `inodeNumber(path): number`, `extInode(ino): ExtInode` (`slot.offset` absolute; `NotFound` for 0 or past `inodesCount`), `dirEntries(path): ExtDirEntry[]` (scan order, `offset` absolute), `fileBlocks(path): ExtFileBlocks` (`data` in logical order; `indirect` as `{ block, level }`, level 2 only for the double-indirect block); and the slice-3 methods `formatExt2(ExtFormatOptions | undefined)`, `formatExt3(Ext3FormatOptions | undefined)`, `journalInfo(): JournalInfo | undefined`, `journalBlocks(): JournalBlock[]`, `armCrash(phase: string)`, `disarmCrash()`, `crashPhase(): string | undefined` (cleared once a crash fires), `needsRecovery(): boolean`, `recover(): OpRecord`, `annotateSector(n)`, `layout()`, `fsType()`. Task 2, from `fs/adapter.ts`: `AddrVocab`, `UnitVocab` (with `fileParts`), `UnitSpace` (with `sector`), `unitIsSector`, `UnitOwner` (with `role?`), `FsAdapter` (with `needsRecovery`, `journal?`, `extraAddrHelp?`), `FsFamily` (with `fsTypes`), `MkfsSpec { summary; flags }` (Task 6 removes `summary` when `mkfs` gets one summary for every family), `MkfsFlag`, `CrashPhase`, `CRASH_PHASES`, `JournalCapability`, `JournalState`, `JournalRingBlock`, `DfFacts`, `StatFacts`, `TraceRow`, `FsFamilyId` (`"fat16"` only); from `fs/base.ts`: `SpaceAdapter` (the abstract base whose `UnitSpace` getters and methods delegate to a protected `space`, with `ownerOf`, `freeUnits` (the units with no owner row), and `regionStart`; a subclass supplies `vol`, `owners`, and `space`) and `hexAddr`; from `core/`: `defaultColorForRegion`, `COLOR_JOURNAL` (11), `ByteChangeLike`, `Interval`, `ColorIndex`, and `freeSpaceLabel(fs: Pick<FsAdapter, "freeUnits" | "unitSize">)` (`core/freeSpace.ts`, Round 6's test only).

Produces (exact; `fs/ext/index.ts` re-exports all of it):

```ts
// src/lib/wasm.ts (type-only; allowed only under src/fs/ext/ by the boundary test)
export type { Ext3FormatOptions, ExtBlockOwner, ExtBlockOwnerRole, ExtDirEntry, ExtFileBlocks, ExtFormatOptions, ExtGeometry, ExtGroup, ExtIndirectBlock, ExtInode, ExtInodeSlot, ExtSuperblock, JournalBlock, JournalBlockKind, JournalInfo, JournalMode } from "fs-emulator-wasm";

// fs/ext/geometry.ts
export const EXT_UNIT: UnitVocab;     // { singular: "block", plural: "blocks", letter: "b", first: 1, fileParts: "its inode, block map, and blocks" }
export const EXT_SECTOR: AddrVocab;   // { singular: "block", plural: "blocks", letter: "b" }
export function extSpace(g: ExtGeometry): UnitSpace;   // unitCount totalBlocks-1, unitSize = sectorSize = blockSize, totalSectors totalBlocks; unitOfSector(s) = s for firstDataBlock <= s < totalBlocks; colorForRegion = defaultColorForRegion (journal -> COLOR_JOURNAL)

// fs/ext/format.ts
export interface ExtFamilyOptions { variant?: "ext2" | "ext3"; totalBlocks?: number; inodesPerGroup?: number; label?: string; uuid?: string; journalBlocks?: number; journalMode?: "ordered" | "data" }
export const SIZES: { label: string; totalBlocks: number }[];   // 4 MB 4096, 16 MB 16384, 64 MB 65536, 256 MB 262144
export const DEFAULTS: Required<Pick<ExtFamilyOptions, "variant" | "totalBlocks" | "label">>;   // { variant: "ext3", totalBlocks: 16384, label: "" }
// (the core's bounds MIN_BLOCKS 64, MAX_BLOCKS 262144, BLOCKS_PER_GROUP 8192, BLOCK_SIZE 1024,
//  MIN_VOLUME_BLOCKS_FOR_JOURNAL 2048, MIN_JOURNAL_BLOCKS 1024, LABEL_BYTES 16 are module constants, not exported)
export function defaultInodesPerGroup(totalBlocks: number): number;        // 512 on every form size; 16 at 64 blocks
export function defaultJournalBlocks(totalBlocks: number): number | null;  // null < 2048, 1024 < 32768, 4096 < 262144, else 8192
export function checkFormat(o: ExtFamilyOptions): { problem: string | null };
export const MKFS: MkfsSpec;   // summary "Format /dev/hda as ext2 or ext3 (clears the timeline)" (Task 6 removes it with MkfsSpec.summary); flags blocks, inodes-per-group, label, uuid, journal-blocks, journal-mode
export function toWasmOptions(o: ExtFamilyOptions): { variant: "ext2"; options: ExtFormatOptions } | { variant: "ext3"; options: Ext3FormatOptions };   // drops undefined keys; throws Error("journalBlocks and journalMode need variant ext3")

// fs/ext/metadata.ts
export function touchesMetadata(geo: ExtGeometry, journalBlocks: readonly number[], indirectBlocks: readonly number[], changes: ByteChangeLike[]): boolean;
export const CORRUPT_NOTE: string;
export const NOTES: FsAdapter["notes"];

// fs/ext/journal.ts
export class ExtJournal implements JournalCapability { constructor(vol: Volume, info: () => JournalInfo) }

// fs/ext/adapter.ts
export class ExtAdapter extends SpaceAdapter implements FsAdapter {   // SpaceAdapter (Task 2, fs/base.ts): the UnitSpace members, ownerOf, regionStart
  readonly id: FsFamilyId;               // "ext" (cast until Task 4)
  name: "ext2" | "ext3";                 // vol.fsType(), re-read by refresh()
  readonly family: FsFamily<ExtFamilyOptions>;
  readonly vol: Volume;
  geo: ExtGeometry; sb: ExtSuperblock;   // plain caches, re-read by refresh()
  owners: UnitOwner[];                   // non-journal rows: { unit, path, isDir: role === "directory", firstUnit, role }; firstUnit = fileBlocks(path).data[0]
  journalBlocks: number[];               // the <journal> rows' blocks, ascending (82..1110 on the default disk); [] on ext2
  journal?: ExtJournal;                  // ext3 only; the same instance across refreshes
  needsRecovery: boolean;
  readonly corruptNote: string; readonly notes: FsAdapter["notes"]; readonly extraAddrHelp: ", i:11 (inode)";
  refresh(): void;
  // every other FsAdapter member (chain, entrySlots, dataStart, stat, df, trace, describeUnit,
  // annotateSector, parseAddr, namesMatch, touchesMetadata; the rest from SpaceAdapter), plus:
  freeUnits(): number;                   // overrides SpaceAdapter's: df().free (Round 6)
  inodeOffset(ino: number): number;      // extInode(ino).slot.offset; throws for a bad inode number
  dirEntryOffset(path: string): number | null;   // the entry naming path in its parent; null for "/" or absent
}
export const ext: FsFamily<ExtFamilyOptions>;   // id "ext", name "ext", fsTypes ["ext2", "ext3"], format via toWasmOptions, bind, mkfs: MKFS

// fs/ext/index.ts
export function asExt(fs: FsAdapter): ExtAdapter;   // throws `not an ext adapter: ${fs.id}`

// tests/fixtures/extGeometry.ts
export const geo: ExtGeometry; export const layout: Region[]; export const space: UnitSpace;   // the default ext3 disk, fresh

// core/tree.ts (changed): buildTree's root has firstUnit = the owners' "/" row's firstUnit, else null (FAT)
```

Strings and facts later tasks rely on (all pinned by this task's tests):

- `trace(path)` rows, in order: `directory entry in ${parent} (block ${B} + 0x${nn})` (offset from `dirEntries`; none for `/`), `inode ${ino} (block ${B} + 0x${nn})`, then per pointer block `single-indirect block ${n}`, `double-indirect block ${n}`, or `indirect block ${n} (level 1, under the double)`, then `data block ${first} (first of ${k})` or `no data blocks` (offset `null`). A missing path gives only `no data blocks`. Each offset is the row's absolute byte (a pointer or data block's first byte).
- `describeUnit(b)`: `data block ${i} of ${path}`, `directory block ${i} of ${path}` (`i` the logical index), `single-indirect block of ${path}`, `double-indirect block of ${path}`, `indirect block of ${path} (level 1)`, or `free`; `null` for block 0, a group's metadata, any journal block (data or pointer), and past the end.
- `stat(path)` keys in order: `inode`, `inodeOffset` (hex), `mode` (`"0100644"`, `"040755"`), `links`, `blocks512`, `dataBlocks`, `indirectBlocks`, `dirEntryOffset` (hex or `""`). `df()`: `{ unitSize: 1024, units: blocksCount, used: blocksCount - freeBlocks, free: freeBlocks }` (1179 used, 15205 free on a fresh default disk).
- `chain("/")` is `[69]`, `entrySlots("/")` is inode 2's slot (5·1024 + 0x80 .. + 0x100), `dataStart("/")` is 69·1024, and `buildTree(vol, owners).firstUnit` is 69 (the Files tree's root row reads `0 B · first block 69`); on FAT the tree's root stays `null`.
- `freeUnits()` is `df().free` (15,205 on a fresh default disk), so `freeSpaceLabel(fs)` (Task 2's ribbon label) is `14.8 MB free`; `SpaceAdapter`'s count of blocks without an owner row would have said `16.0 MB free`.
- `CORRUPT_NOTE`: `Superblock or group descriptors do not parse; the tree is unavailable until a raw write repairs them.`
- `NOTES`: `rewrite` `ext keeps the file's blocks and maps new ones after them`; `partialWrite` `the ext volume has no partial writes; the whole file was rewritten over its blocks`.
- `checkFormat` problems, checked in this order: `too few blocks (minimum 64)`, `too many blocks (maximum 262,144)`, `a journal needs at least 2048 blocks` (ext3, the default variant), `the journal must be at least 1024 blocks` (ext3 with `journalBlocks` set), `label is longer than 16 bytes` (UTF-8 bytes). It does not check `inodesPerGroup` or `uuid`; the formatter refuses those with its own message.
- `MKFS.flags` (`long`, `desc`, `kind`, `option`): `blocks` "total 1 KiB blocks (default 16384 = 16 MB)" int `totalBlocks`; `inodes-per-group` "inodes per block group (default one per 16 KiB)" int `inodesPerGroup`; `label` "volume label, up to 16 bytes" str `label`; `uuid` "32 hex digits, hyphens optional" str `uuid`; `journal-blocks` "journal size in blocks, ext3 only" int `journalBlocks`; `journal-mode` "ordered or data, ext3 only" str `journalMode`.
- `firstUnit` on every path row is the path's first data (or directory) block in logical order, `fileBlocks(path).data[0]` (`chain(path)[0]`), not its lowest block: a file that grows after a lower block was freed maps that block later (`/b` growing into a deleted `/a`'s block 1111 has chain `[1112, 1111]` and `firstUnit` 1112). The tree's "first block" (`buildTree`) and the lessons' focuses read it.
- The journal's five pointer blocks (1106..1110 on the default disk) lie in the `data (group 0)` region after the `journal` region (82..1105), not in it. The binding rule (spec sections 2 and 4, as amended) is that blocks inside `journal` regions never become units, while the pointer blocks stay in `owners` as `<journal>` rows `{ path: "<journal>", isDir: false, role: "indirect", color: COLOR_JOURNAL }` (colour 11), so the map, the dump, and the Inspector show them owned by the journal rather than free. Every journal block, data and pointer, is in `journalBlocks`. This task stops one step short of that rule: `UnitOwner.color` does not exist until Task 4, so here the pointer blocks are in `journalBlocks` only, attribution calls them free, and `describeUnit` returns `null` for them as for the journal's data blocks. Task 4 applies the rule and changes this task's code and tests in these places:
  - `ExtAdapter`'s owners filter keeps the `<journal>` rows outside `journal` regions (the pointer blocks) as `{ isDir: false, role: "indirect", color: COLOR_JOURNAL }` and drops only the rows inside them;
  - in the first test of `describe("ExtAdapter: owners, chains, and slots", ...)` in `tests/fs/ext.test.ts` (which it retitles `... and none of the journal's data blocks`), the `fs.owners.filter((o) => o.path !== "/lost+found")` of its `toEqual` gains `&& o.path !== "<journal>"`, and `expect(fs.owners.some((o) => o.path === "<journal>")).toBe(false);` (the deleted `<journal>` check) goes;
  - in `it("describes what a block holds for its owner, free, or nothing outside the data area", ...)` the `null` list `[0, 1, 5, 68, 90, 1106, 1110, 8193, 8260, 16384, -1]` becomes `[0, 1, 5, 68, 90, 1105, 8193, 8260, 16384, -1]` (1105 is the journal's last data block), because `describeUnit` gains a first branch that names the pointer blocks `pointer block of the journal`, which Task 4's new test `keeps the journal's pointer blocks as <journal> rows in the journal colour, and never its data blocks` pins for 1106..1110;
  - `tests/attribution.test.ts`: `attrAtSector(te, 1107)` is still `free: true` (that table is built from an owner list without the journal's rows); Task 4 only rewords the comment above it and adds the owned case as a new test.
- `ExtJournal.phase()` is `null` once an armed crash has fired (wasm clears it). `state()` reads the adapter's cache, so it is current only after `refresh()` (the store refreshes after every op); `blocks()` reads the volume on every call.
- The boundary scan allows `.recover(` outside `src/fs/ext/` only on a receiver named `journal` (`journal.recover()`, `journal!.recover()`, `journal?.recover()`); write `volume.adapter.journal!.recover()` or bind a local named `journal`, never `const j = ...; j.recover()`. `.armCrash(` and `.disarmCrash(` count on every receiver: generic code arms through the capability's `arm`/`disarm`, and the store and the shell host name their method `setArmedPhase` (Tasks 2 and 6), so the scan needs no exemption for them.
- When Task 4 adds `"ext"` to `FsFamilyId`: replace `EXT_ID` in `adapter.ts` with the literal `"ext"` and drop the `as string` in `asExt`; nothing else changes.

Environment for every command below, from `web/ui`: any pnpm 10.26 or newer (nvm's pnpm 12 and Homebrew's 10.33 both work). Both web packages keep their pnpm settings in a tracked `pnpm-workspace.yaml`, so a frozen install must leave `web/ui/pnpm-lock.yaml` and `web/ui/pnpm-workspace.yaml` unmodified; if either shows as modified in `git status` at any point, restore it with `git checkout web/ui/pnpm-lock.yaml web/ui/pnpm-workspace.yaml` and run `CI=true pnpm install --frozen-lockfile` again (`CI=true` lets pnpm purge a stale modules directory without asking).

#### Round 1: the unit space, the fixture, and the ext DTO types

- [ ] **Step 1: Build the wasm package Task 1 changed and install it.** The build runs from the repo root; then change to `web/ui`, where every later command runs:

```sh
wasm-pack build crates/wasm --target bundler
cd web/ui
CI=true pnpm install --frozen-lockfile
grep -c extGeometry node_modules/fs-emulator-wasm/fs_emulator_wasm.d.ts
pnpm test
```

Expected: wasm-pack ends with `Your wasm pkg is ready to publish at crates/wasm/pkg.`; the grep prints a non-zero count (the installed package declares Task 1's methods); `pnpm test` prints `Test Files  42 passed (42)` and `Tests  340 passed (340)` (Tasks 1 and 2 applied, Task 2's `tests/grid.test.ts`, `tests/freeSpace.test.ts`, and `tests/scrollNonce.test.ts` included). (`CI=true` lets pnpm purge a stale modules directory without asking.)

- [ ] **Step 2: Write the fixture.** Create `web/ui/tests/fixtures/extGeometry.ts`: the default ext3 disk's geometry and layout as plain data, copied from `Volume.formatExt3(undefined)` (the first test of Step 3 compares them with a live volume so they cannot drift).

```ts
import type { ExtGeometry, Region } from "../../src/lib/wasm";
import { extSpace } from "../../src/fs/ext/geometry";

// The default `Volume.formatExt3(undefined)` disk: 16,384 blocks of 1 KiB in two groups of
// 512 inodes; group 0's metadata is blocks 1..68, the root directory is block 69, lost+found
// 70..81, the journal's data 82..1105 (its five pointer blocks 1106..1110 lie in the data
// region after it); group 1 starts with its backup superblock at 8193. The free counts are
// the freshly formatted ones. tests/fs/ext.test.ts checks both against a live volume.
export const geo: ExtGeometry = {
  blockSize: 1024, totalBlocks: 16384, firstDataBlock: 1, blocksPerGroup: 8192, inodesPerGroup: 512,
  inodesCount: 1024, inodeSize: 128, inodeTableBlocks: 64, descriptorBlocks: 1,
  groups: [
    { index: 0, firstBlock: 1, blockCount: 8192, superblockBlock: 1, descriptorsBlock: 2, blockBitmap: 3, inodeBitmap: 4, inodeTable: 5, firstData: 69, freeBlocks: 7082, freeInodes: 501, usedDirs: 2 },
    { index: 1, firstBlock: 8193, blockCount: 8191, superblockBlock: 8193, descriptorsBlock: 8194, blockBitmap: 8195, inodeBitmap: 8196, inodeTable: 8197, firstData: 8261, freeBlocks: 8123, freeInodes: 512, usedDirs: 0 },
  ],
};
export const layout: Region[] = [
  { name: "boot block", sectors: { start: 0, end: 1 }, kind: "boot" },
  { name: "superblock", sectors: { start: 1, end: 2 }, kind: "boot" },
  { name: "group descriptors (group 0)", sectors: { start: 2, end: 3 }, kind: "metadata" },
  { name: "block bitmap (group 0)", sectors: { start: 3, end: 4 }, kind: "allocationTable" },
  { name: "inode bitmap (group 0)", sectors: { start: 4, end: 5 }, kind: "allocationTable" },
  { name: "inode table (group 0)", sectors: { start: 5, end: 69 }, kind: "metadata" },
  { name: "data (group 0)", sectors: { start: 69, end: 82 }, kind: "data" },
  { name: "journal", sectors: { start: 82, end: 1106 }, kind: "journal" },
  { name: "data (group 0)", sectors: { start: 1106, end: 8193 }, kind: "data" },
  { name: "backup superblock (group 1)", sectors: { start: 8193, end: 8194 }, kind: "boot" },
  { name: "group descriptors (group 1)", sectors: { start: 8194, end: 8195 }, kind: "metadata" },
  { name: "block bitmap (group 1)", sectors: { start: 8195, end: 8196 }, kind: "allocationTable" },
  { name: "inode bitmap (group 1)", sectors: { start: 8196, end: 8197 }, kind: "allocationTable" },
  { name: "inode table (group 1)", sectors: { start: 8197, end: 8261 }, kind: "metadata" },
  { name: "data (group 1)", sectors: { start: 8261, end: 16384 }, kind: "data" },
];

/** The same disk as a `UnitSpace`, for tests of the generic core that need no Volume. */
export const space = extSpace(geo);
```

- [ ] **Step 3: Write the failing unit-space tests.** Create `web/ui/tests/fs/ext.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { unitIsSector } from "../../src/fs/adapter";
import { defaultColorForRegion } from "../../src/core/attribution";
import { COLOR_BOOT, COLOR_FREE, COLOR_JOURNAL, COLOR_TABLE } from "../../src/core/palette";
import { EXT_SECTOR, EXT_UNIT } from "../../src/fs/ext/geometry";
import { geo, layout, space } from "../fixtures/extGeometry";

const BLOCK = 1024;

describe("the ext unit space", () => {
  it("the fixture is the default ext3 disk", () => {
    const vol = Volume.formatExt3(undefined);
    expect(vol.extGeometry()).toEqual(geo);
    expect(vol.layout()).toEqual(layout);
  });

  it("names the block once: the unit is the sector", () => {
    expect(EXT_UNIT).toEqual({ singular: "block", plural: "blocks", letter: "b", first: 1, fileParts: "its inode, block map, and blocks" });
    expect(EXT_SECTOR).toEqual({ singular: "block", plural: "blocks", letter: "b" });
    expect(space.unit).toBe(EXT_UNIT);
    expect(space.sector).toBe(EXT_SECTOR);
    expect(unitIsSector(space)).toBe(true);
  });

  it("does block arithmetic: every block from firstDataBlock is a unit, block 0 is not", () => {
    expect(space.unitCount).toBe(16383);
    expect(space.unit.first + space.unitCount).toBe(16384); // the attribution tables' size
    expect(space.unitSize).toBe(BLOCK);
    expect(space.sectorSize).toBe(BLOCK);
    expect(space.totalSectors).toBe(16384);
    expect(space.unitOfSector(0)).toBeUndefined();
    expect(space.unitOfSector(1)).toBe(1);
    expect(space.unitOfSector(1111)).toBe(1111);
    expect(space.unitOfSector(16383)).toBe(16383);
    expect(space.unitOfSector(16384)).toBeUndefined();
    expect(space.unitByteRange(1111)).toEqual({ start: 1111 * BLOCK, end: 1112 * BLOCK });
    expect(space.unitOfOffset(1111 * BLOCK + 7)).toBe(1111);
    expect(space.unitOfOffset(100)).toBeUndefined();
    expect(space.unitStartsAt(69)).toBe(true);
    expect(space.unitStartsAt(0)).toBe(false);
  });

  it("colours journal regions COLOR_JOURNAL and every other region by kind", () => {
    expect(layout.map((r) => space.colorForRegion(r))).toEqual([
      COLOR_BOOT, COLOR_BOOT, COLOR_BOOT, COLOR_TABLE, COLOR_TABLE, COLOR_BOOT, COLOR_FREE, COLOR_JOURNAL, COLOR_FREE,
      COLOR_BOOT, COLOR_BOOT, COLOR_TABLE, COLOR_TABLE, COLOR_BOOT, COLOR_FREE,
    ]);
    expect(space.colorForRegion).toBe(defaultColorForRegion); // it already gives journal regions COLOR_JOURNAL
  });
});
```

- [ ] **Step 4: Write the failing ext attribution tests.** In `web/ui/tests/attribution.test.ts`, replace the palette import (line 3) with:

```ts
import { COLOR_BOOT, COLOR_DIR, COLOR_FREE, COLOR_JOURNAL, COLOR_TABLE, COLOR_TABLE_ALT, colorIndexForPath } from "../src/core/palette";
```

add after `import { geo, layout, space } from "./fixtures/geometry";`:

```ts
import { geo as extGeo, layout as extLayout, space as extSpace } from "./fixtures/extGeometry";
```

add after the `owners` constant (after its closing `];` and a blank line):

```ts
/** Rows as the ext adapter makes them: the root directory, and a file with one data block and
 *  its single-indirect block (the real /bigger.txt has 13 data blocks, 1113..1125). */
const extOwners: UnitOwner[] = [
  { unit: 69, path: "/", isDir: true, firstUnit: 69, role: "directory" },
  { unit: 1113, path: "/bigger.txt", isDir: false, firstUnit: 1113, role: "data" },
  { unit: 1126, path: "/bigger.txt", isDir: false, firstUnit: 1113, role: "indirect" },
];
```

and add these two tests right before `it("file hues are stable and in range", ...)`:

```ts
  it("on ext: journal regions are never units, and metadata blocks are coloured by kind", () => {
    const te = buildAttribution(extSpace, extLayout, extOwners);
    expect(te.ownerByUnit.length).toBe(extGeo.totalBlocks); // unit.first (1) + unitCount (totalBlocks - 1)
    expect(attrAtSector(te, 0)).toEqual({ regionKind: "boot", regionName: "boot block", sector: 0, free: false, colorIndex: COLOR_BOOT });
    expect(attrAtSector(te, 3)).toMatchObject({ regionName: "block bitmap (group 0)", colorIndex: COLOR_TABLE });
    expect(attrAtSector(te, 6)).toMatchObject({ regionName: "inode table (group 0)", colorIndex: COLOR_BOOT });
    expect(attrAtSector(te, 8193)).toMatchObject({ regionName: "backup superblock (group 1)", colorIndex: COLOR_BOOT });
    // Block 90 is a unit by arithmetic, and even an owner row claiming it would not make it one.
    expect(extSpace.unitOfSector(90)).toBe(90);
    const claimed = buildAttribution(extSpace, extLayout, [...extOwners, { unit: 90, path: "/x", isDir: false, firstUnit: 90, role: "data" }]);
    for (const table of [te, claimed]) {
      expect(attrAtSector(table, 90)).toEqual({ regionKind: "journal", regionName: "journal", sector: 90, free: false, colorIndex: COLOR_JOURNAL });
    }
    // The journal's pointer blocks 1106..1110 lie in the data region after it; the adapter keeps
    // them out of `owners` (they are in `journalBlocks`), so attribution calls them free.
    expect(attrAtSector(te, 1107)).toMatchObject({ regionName: "data (group 0)", unit: 1107, free: true, colorIndex: COLOR_FREE });
  });
  it("on ext: an indirect block takes its file's hue and reports its role", () => {
    const te = buildAttribution(extSpace, extLayout, extOwners);
    expect(attrAtSector(te, 69)).toMatchObject({ unit: 69, ownerPath: "/", isDir: true, role: "directory", colorIndex: COLOR_DIR, free: false });
    expect(attrAtSector(te, 1113)).toMatchObject({ unit: 1113, ownerPath: "/bigger.txt", role: "data", colorIndex: colorIndexForPath("/bigger.txt") });
    expect(attrAtSector(te, 1126)).toMatchObject({ unit: 1126, ownerPath: "/bigger.txt", isDir: false, role: "indirect", colorIndex: colorIndexForPath("/bigger.txt"), free: false });
    expect(attrAtOffset(te, 1127 * 1024 + 5)).toMatchObject({ unit: 1127, free: true, colorIndex: COLOR_FREE });
    expect(attrAtOffset(te, 1127 * 1024 + 5).role).toBeUndefined();
  });
```

- [ ] **Step 5: Write the failing ext vocabulary test for the Lesson card.** In `web/ui/tests/lesson.test.ts`, add after `import { layout, space } from "./fixtures/geometry";`:

```ts
import { layout as extLayout, space as extSpace } from "./fixtures/extGeometry";
```

and add this test right before `it("is null when there is nothing to point at", ...)`:

```ts
  it("speaks ext's vocabulary on the default ext3 disk: blocks, the inode clause, ext's region names", () => {
    const extRegionAt = (sector: number): string =>
      extLayout.find((r) => sector >= r.sectors.start && sector < r.sectors.end)?.name ?? "unknown";
    const ext = (focus: Parameters<typeof describeFocus>[0]) => describeFocus(focus, extSpace, extRegionAt);
    expect(ext({ path: "/hello.txt" })).toBe("Files: /hello.txt, its inode, block map, and blocks");
    expect(ext({ sector: 69 })).toBe("Block 69, data (group 0)");
    expect(ext({ sector: 82 })).toBe("Block 82, journal");
    expect(ext({ unit: 1112 })).toBe("Block 1112 in the data region (offset 0x116000)");
    expect(ext({ offset: 0x1a00 })).toBe("Offset 0x1a00 in inode table (group 0)");
    expect(ext({ path: "/hello.txt", offset: 0x1a00 })).toBe("Files: /hello.txt, its inode, block map, and blocks; offset 0x1a00 in inode table (group 0)");
  });

```

- [ ] **Step 6: Run them and watch them fail.**

```sh
pnpm exec vitest run tests/fs/ext.test.ts tests/attribution.test.ts tests/lesson.test.ts
```

Expected: all three files fail to load with `Error: Cannot find module '../../src/fs/ext/geometry'` (imported from `tests/fs/ext.test.ts` and from `tests/fixtures/extGeometry.ts`); `Test Files  3 failed (3)`.

- [ ] **Step 7: Re-export the ext DTO types.** In `web/ui/src/lib/wasm.ts`, append after the existing `export type { ... } from "fs-emulator-wasm";` block:

```ts
// The ext family's DTOs: only src/fs/ext/ may import them (tests/adapterBoundary.test.ts).
export type {
  Ext3FormatOptions, ExtBlockOwner, ExtBlockOwnerRole, ExtDirEntry, ExtFileBlocks, ExtFormatOptions,
  ExtGeometry, ExtGroup, ExtIndirectBlock, ExtInode, ExtInodeSlot, ExtSuperblock, JournalBlock,
  JournalBlockKind, JournalInfo, JournalMode,
} from "fs-emulator-wasm";
```

- [ ] **Step 8: Write the unit space.** Create `web/ui/src/fs/ext/geometry.ts`:

```ts
import type { ExtGeometry } from "../../lib/wasm";
import { defaultColorForRegion } from "../../core/attribution";
import type { AddrVocab, UnitSpace, UnitVocab } from "../adapter";

/** How ext names its allocation unit: the block, `b:N` in the shell, from block 1 (block 0 is
 *  the boot block, outside every group). */
export const EXT_UNIT: UnitVocab = { singular: "block", plural: "blocks", letter: "b", first: 1, fileParts: "its inode, block map, and blocks" };

/** How ext names one disk sector: the disk's sector is the 1 KiB block, so the same noun. */
export const EXT_SECTOR: AddrVocab = { singular: "block", plural: "blocks", letter: "b" };

/**
 * Pure block arithmetic over one geometry; buildable from a fixture without a Volume. Every
 * block from `firstDataBlock` up is a unit (the attribution tables have `totalBlocks` rows);
 * attribution only treats the ones in `data` regions as units, so metadata and journal blocks
 * never are. Regions are coloured by kind through `defaultColorForRegion`, which already gives
 * `journal` regions `COLOR_JOURNAL`.
 */
export function extSpace(g: ExtGeometry): UnitSpace {
  const unitOfSector = (s: number): number | undefined => (s >= g.firstDataBlock && s < g.totalBlocks ? s : undefined);
  return {
    unit: EXT_UNIT,
    sector: EXT_SECTOR,
    unitCount: g.totalBlocks - 1,
    unitSize: g.blockSize,
    sectorSize: g.blockSize,
    totalSectors: g.totalBlocks,
    unitOfSector,
    unitByteRange: (b) => ({ start: b * g.blockSize, end: (b + 1) * g.blockSize }),
    unitOfOffset: (offset) => unitOfSector(Math.floor(offset / g.blockSize)),
    unitStartsAt: (s) => unitOfSector(s) !== undefined,
    colorForRegion: defaultColorForRegion,
  };
}
```

- [ ] **Step 9: Run them and watch them pass.**

```sh
pnpm exec vitest run tests/fs/ext.test.ts tests/attribution.test.ts tests/lesson.test.ts
```

Expected: `Test Files  3 passed (3)`, `Tests  24 passed (24)`.

#### Round 2: the format model (options, sizes, defaults, checks, mkfs flags)

- [ ] **Step 10: Write the failing format tests.** Create `web/ui/tests/fs/ext-format.test.ts`. Its `MKFS.summary` pin lives only until Task 6, which gives `mkfs` one summary for every family, removes `summary` from `MkfsSpec` (the spec's end state is `{ flags }`), and replaces the pin with `expect(MKFS).not.toHaveProperty("summary")`:

```ts
import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { DEFAULTS, MKFS, SIZES, checkFormat, defaultInodesPerGroup, defaultJournalBlocks, toWasmOptions, type ExtFamilyOptions } from "../../src/fs/ext/format";

/** What `ext.format` does with the options, without the family object. */
function formatWith(o: ExtFamilyOptions): Volume {
  const w = toWasmOptions(o);
  return w.variant === "ext2" ? Volume.formatExt2(w.options) : Volume.formatExt3(w.options);
}

describe("defaultInodesPerGroup", () => {
  it("is the core's rule: one inode per 16 KiB of each group, a multiple of 8 in 16..8192", () => {
    expect(defaultInodesPerGroup(16384)).toBe(512);
    expect(defaultInodesPerGroup(262144)).toBe(512);
    expect(defaultInodesPerGroup(64)).toBe(16);
    expect(defaultInodesPerGroup(8200)).toBe(256); // two groups, the second a runt: the volume's rate split evenly
    expect(defaultInodesPerGroup(1100)).toBe(72);
  });

  it("agrees with the formatter for every size the form offers and a few odd ones", () => {
    for (const totalBlocks of [...SIZES.map((s) => s.totalBlocks), 64, 1100, 2047, 2048, 32767]) {
      expect(Volume.formatExt2({ totalBlocks }).extGeometry().inodesPerGroup, `${totalBlocks} blocks`).toBe(defaultInodesPerGroup(totalBlocks));
    }
  });
});

describe("defaultJournalBlocks", () => {
  it("is mke2fs's table: none below 2048 blocks, then 1024, 4096, 8192", () => {
    expect(defaultJournalBlocks(2047)).toBeNull();
    expect(defaultJournalBlocks(2048)).toBe(1024);
    expect(defaultJournalBlocks(32767)).toBe(1024);
    expect(defaultJournalBlocks(32768)).toBe(4096);
    expect(defaultJournalBlocks(262143)).toBe(4096);
    expect(defaultJournalBlocks(262144)).toBe(8192);
  });

  it("agrees with the journal formatExt3 lays down", () => {
    for (const totalBlocks of [2048, ...SIZES.map((s) => s.totalBlocks)]) {
      expect(Volume.formatExt3({ totalBlocks }).journalInfo()?.maxlen, `${totalBlocks} blocks`).toBe(defaultJournalBlocks(totalBlocks));
    }
  });
});

describe("checkFormat", () => {
  it("names each problem the Format form disables its button on", () => {
    expect(checkFormat({ totalBlocks: 63 }).problem).toBe("too few blocks (minimum 64)");
    expect(checkFormat({ totalBlocks: 262145 }).problem).toBe("too many blocks (maximum 262,144)");
    expect(checkFormat({ totalBlocks: 2047 }).problem).toBe("a journal needs at least 2048 blocks"); // ext3 is the default variant
    expect(checkFormat({ variant: "ext2", totalBlocks: 2047 }).problem).toBeNull();
    expect(checkFormat({ journalBlocks: 1023 }).problem).toBe("the journal must be at least 1024 blocks");
    expect(checkFormat({ label: "a".repeat(17) }).problem).toBe("label is longer than 16 bytes");
    expect(checkFormat({ label: "é".repeat(8) }).problem).toBeNull(); // 16 bytes of UTF-8
    expect(checkFormat({ label: "é".repeat(9) }).problem).toBe("label is longer than 16 bytes"); // counted in bytes, not characters
  });

  it("finds nothing wrong with the defaults, and every problem it names the formatter refuses too", () => {
    expect(checkFormat({})).toEqual({ problem: null });
    expect(checkFormat(DEFAULTS)).toEqual({ problem: null });
    const bad: ExtFamilyOptions[] = [{ totalBlocks: 63 }, { totalBlocks: 262145 }, { totalBlocks: 2047 }, { journalBlocks: 1023 }, { label: "a".repeat(17) }];
    for (const o of bad) {
      expect(checkFormat(o).problem, JSON.stringify(o)).not.toBeNull();
      expect(() => formatWith(o), JSON.stringify(o)).toThrow();
    }
  });

  it("passes every size and variant the form offers, and the formatter agrees", () => {
    for (const { totalBlocks } of SIZES) {
      for (const variant of ["ext2", "ext3"] as const) {
        expect(checkFormat({ variant, totalBlocks }).problem).toBeNull();
        expect(formatWith({ variant, totalBlocks }).fsType()).toBe(variant);
      }
    }
  });
});

describe("the form's choices and the shell's mkfs", () => {
  it("offers four sizes and defaults to the lessons' disk", () => {
    expect(SIZES).toEqual([
      { label: "4 MB", totalBlocks: 4096 },
      { label: "16 MB", totalBlocks: 16384 },
      { label: "64 MB", totalBlocks: 65536 },
      { label: "256 MB", totalBlocks: 262144 },
    ]);
    expect(DEFAULTS).toEqual({ variant: "ext3", totalBlocks: 16384, label: "" });
    const vol = formatWith(DEFAULTS);
    expect(vol.fsType()).toBe("ext3");
    expect(vol.extGeometry()).toEqual(Volume.formatExt3(undefined).extGeometry());
  });

  it("keeps the six mkfs flags with their descriptions", () => {
    expect(MKFS.summary).toBe("Format /dev/hda as ext2 or ext3 (clears the timeline)");
    expect(MKFS.flags).toEqual([
      { long: "blocks", desc: "total 1 KiB blocks (default 16384 = 16 MB)", kind: "int", option: "totalBlocks" },
      { long: "inodes-per-group", desc: "inodes per block group (default one per 16 KiB)", kind: "int", option: "inodesPerGroup" },
      { long: "label", desc: "volume label, up to 16 bytes", kind: "str", option: "label" },
      { long: "uuid", desc: "32 hex digits, hyphens optional", kind: "str", option: "uuid" },
      { long: "journal-blocks", desc: "journal size in blocks, ext3 only", kind: "int", option: "journalBlocks" },
      { long: "journal-mode", desc: "ordered or data, ext3 only", kind: "str", option: "journalMode" },
    ]);
  });

  it("every flag's option reaches the formatter", () => {
    const sample: Record<string, number | string> = {
      totalBlocks: 4096, inodesPerGroup: 256, label: "disk", uuid: "0123456789abcdef0123456789abcdef", journalBlocks: 1024, journalMode: "data",
    };
    for (const f of MKFS.flags) {
      // formatExt3 rejects unknown keys, so a misspelt option would throw here.
      expect(() => formatWith({ [f.option]: sample[f.option] }), f.option).not.toThrow();
    }
    const vol = formatWith({ label: "disk", uuid: "0123456789abcdef0123456789abcdef", journalMode: "data" });
    expect(vol.extSuperblock()).toMatchObject({ label: "disk", uuid: "01234567-89ab-cdef-0123-456789abcdef" });
    expect(vol.journalInfo()?.mode).toBe("data");
  });
});

describe("toWasmOptions", () => {
  it("splits the variant from the formatter's options and drops absent keys", () => {
    expect(toWasmOptions({})).toEqual({ variant: "ext3", options: {} });
    expect(toWasmOptions({ variant: "ext2", totalBlocks: 4096, label: undefined })).toEqual({ variant: "ext2", options: { totalBlocks: 4096 } });
    expect(toWasmOptions({ journalBlocks: 2048, journalMode: "data", uuid: "x" })).toEqual({ variant: "ext3", options: { journalBlocks: 2048, journalMode: "data", uuid: "x" } });
  });

  it("refuses journal options on ext2", () => {
    expect(() => toWasmOptions({ variant: "ext2", journalMode: "data" })).toThrow("journalBlocks and journalMode need variant ext3");
    expect(() => toWasmOptions({ variant: "ext2", journalBlocks: 1024 })).toThrow("journalBlocks and journalMode need variant ext3");
  });
});
```

- [ ] **Step 11: Run it and watch it fail.**

```sh
pnpm exec vitest run tests/fs/ext-format.test.ts
```

Expected: `Error: Cannot find module '../../src/fs/ext/format'`; `Test Files  1 failed (1)`.

- [ ] **Step 12: Write the format model.** Create `web/ui/src/fs/ext/format.ts`. `MKFS.summary` is here because Task 2's `MkfsSpec` still requires it and the shell's `mkfs` still prints a family's summary; Task 6 removes the field, this summary, and its test in Step 10 together:

```ts
import type { Ext3FormatOptions, ExtFormatOptions } from "../../lib/wasm";
import type { MkfsFlag, MkfsSpec } from "../adapter";

/** What `ext.format` takes: the variant (ext3 by default) and the core's options. The two
 *  journal keys exist on ext3 only; `toWasmOptions` refuses them on ext2. */
export interface ExtFamilyOptions {
  variant?: "ext2" | "ext3";
  totalBlocks?: number;
  inodesPerGroup?: number;
  label?: string;
  uuid?: string;
  journalBlocks?: number;
  journalMode?: "ordered" | "data";
}

/** The Format form's disk sizes, in 1 KiB blocks. */
export const SIZES: { label: string; totalBlocks: number }[] = [
  { label: "4 MB", totalBlocks: 4096 },
  { label: "16 MB", totalBlocks: 16384 },
  { label: "64 MB", totalBlocks: 65536 },
  { label: "256 MB", totalBlocks: 262144 },
];

/** The Format form's initial values: the disk every ext lesson runs on (ext3, 16 MB, no label). */
export const DEFAULTS: Required<Pick<ExtFamilyOptions, "variant" | "totalBlocks" | "label">> = {
  variant: "ext3",
  totalBlocks: 16384,
  label: "",
};

// The core's bounds (crates/ext: MIN_BLOCKS, MAX_BLOCKS, BLOCKS_PER_GROUP, and the journal's
// MIN_VOLUME_BLOCKS_FOR_JOURNAL and MIN_JOURNAL_BLOCKS), for the rules below.
const MIN_BLOCKS = 64, MAX_BLOCKS = 262144, BLOCKS_PER_GROUP = 8192, BLOCK_SIZE = 1024;
const MIN_VOLUME_BLOCKS_FOR_JOURNAL = 2048, MIN_JOURNAL_BLOCKS = 1024, LABEL_BYTES = 16;

/** The core's `default_inodes_per_group`: mke2fs's one inode per 16 KiB of the volume, split
 *  evenly over the groups (block 0 is outside every group, so a volume has
 *  `ceil((totalBlocks - 1) / 8192)` of them; a runt last group gets the same count), rounded up
 *  to a multiple of 8 and clamped to 16..8192. 512 on the default disk. */
export function defaultInodesPerGroup(totalBlocks: number): number {
  const groups = Math.max(1, Math.ceil((totalBlocks - 1) / BLOCKS_PER_GROUP));
  const perGroup = Math.floor((totalBlocks * BLOCK_SIZE) / 16384 / groups);
  return Math.min(8192, Math.max(16, Math.ceil(perGroup / 8) * 8));
}

/** mke2fs's journal size table, as the core applies it: no journal fits below 2048 blocks;
 *  1024 blocks below 32,768; 4096 below 262,144; else 8192. */
export function defaultJournalBlocks(totalBlocks: number): number | null {
  if (totalBlocks < MIN_VOLUME_BLOCKS_FOR_JOURNAL) return null;
  if (totalBlocks < 32768) return 1024;
  if (totalBlocks < 262144) return 4096;
  return 8192;
}

/** The problem the Format form names beside its button (and disables it on), or null. Absent
 *  options take the form's defaults. The core refuses a few more (an inode count that is not a
 *  multiple of 8, a journal too big for the volume, a malformed UUID) and says so in its own
 *  words through the status line. */
export function checkFormat(o: ExtFamilyOptions): { problem: string | null } {
  const variant = o.variant ?? DEFAULTS.variant;
  const total = o.totalBlocks ?? DEFAULTS.totalBlocks;
  let problem: string | null = null;
  if (total < MIN_BLOCKS) problem = "too few blocks (minimum 64)";
  else if (total > MAX_BLOCKS) problem = "too many blocks (maximum 262,144)";
  else if (variant === "ext3" && total < MIN_VOLUME_BLOCKS_FOR_JOURNAL) problem = "a journal needs at least 2048 blocks";
  else if (variant === "ext3" && o.journalBlocks !== undefined && o.journalBlocks < MIN_JOURNAL_BLOCKS) problem = "the journal must be at least 1024 blocks";
  else if (new TextEncoder().encode(o.label ?? "").length > LABEL_BYTES) problem = "label is longer than 16 bytes";
  return { problem };
}

// Typed against the family options so a flag cannot name a key the formatter would reject.
const MKFS_FLAGS: (MkfsFlag & { option: keyof ExtFamilyOptions })[] = [
  { long: "blocks", desc: "total 1 KiB blocks (default 16384 = 16 MB)", kind: "int", option: "totalBlocks" },
  { long: "inodes-per-group", desc: "inodes per block group (default one per 16 KiB)", kind: "int", option: "inodesPerGroup" },
  { long: "label", desc: "volume label, up to 16 bytes", kind: "str", option: "label" },
  { long: "uuid", desc: "32 hex digits, hyphens optional", kind: "str", option: "uuid" },
  { long: "journal-blocks", desc: "journal size in blocks, ext3 only", kind: "int", option: "journalBlocks" },
  { long: "journal-mode", desc: "ordered or data, ext3 only", kind: "str", option: "journalMode" },
];

/** The shell's `mkfs` for this family: its summary and the six flags. The done line is the
 *  shell's, composed from the new volume's `fsType()` (`ext2` or `ext3`). */
export const MKFS: MkfsSpec = {
  summary: "Format /dev/hda as ext2 or ext3 (clears the timeline)",
  flags: MKFS_FLAGS,
};

/** The keys of `o` whose value is not `undefined` (the formatter rejects unknown keys, and a
 *  form or a shell flag left blank must mean "the default"). */
function defined<T extends object>(o: T): Partial<T> {
  return Object.fromEntries(Object.entries(o).filter(([, v]) => v !== undefined)) as Partial<T>;
}

/** The family options as the wasm formatter takes them: `formatExt2` without the journal keys,
 *  `formatExt3` with them. Journal keys on ext2 are an error rather than silently dropped. */
export function toWasmOptions(o: ExtFamilyOptions):
  | { variant: "ext2"; options: ExtFormatOptions }
  | { variant: "ext3"; options: Ext3FormatOptions } {
  const { variant = "ext3", journalBlocks, journalMode, ...common } = o;
  if (variant === "ext2") {
    if (journalBlocks !== undefined || journalMode !== undefined) throw new Error("journalBlocks and journalMode need variant ext3");
    return { variant, options: defined(common) };
  }
  return { variant: "ext3", options: defined({ ...common, journalBlocks, journalMode }) };
}
```

- [ ] **Step 13: Run it and watch it pass.**

```sh
pnpm exec vitest run tests/fs/ext-format.test.ts
```

Expected: `Test Files  1 passed (1)`, `Tests  12 passed (12)`. (The 256 MB formats take a few milliseconds each.)

#### Round 3: what counts as metadata, and the journal capability

- [ ] **Step 14: Write the failing metadata and journal tests.** In `web/ui/tests/fs/ext.test.ts`, add after the `../../src/fs/ext/geometry` import:

```ts
import { ExtJournal } from "../../src/fs/ext/journal";
import { CORRUPT_NOTE, NOTES, touchesMetadata } from "../../src/fs/ext/metadata";
```

add after `const BLOCK = 1024;`:

```ts
const enc = (s: string) => new TextEncoder().encode(s);
/** A one-byte change at `offset`, as a raw write records it. */
const at = (offset: number) => ({ offset, before: new Uint8Array(1), after: new Uint8Array(1) });
```

and append at the end of the file (after a blank line):

```ts
describe("ext metadata", () => {
  const journal = Array.from({ length: 1110 - 82 + 1 }, (_, i) => 82 + i); // 82..1105 and the pointer blocks 1106..1110

  it("counts a change as metadata below a group's first data block, in the journal, or in an indirect block", () => {
    expect(touchesMetadata(geo, journal, [1126], [at(0)])).toBe(true);                   // the boot block
    expect(touchesMetadata(geo, journal, [1126], [at(1 * BLOCK + 0x38)])).toBe(true);    // the superblock's magic
    expect(touchesMetadata(geo, journal, [1126], [at(6 * BLOCK + 0x200)])).toBe(true);   // an inode in the table
    expect(touchesMetadata(geo, journal, [1126], [at(68 * BLOCK + 1023)])).toBe(true);   // the last inode-table byte
    expect(touchesMetadata(geo, journal, [1126], [at(90 * BLOCK)])).toBe(true);          // a journal data block
    expect(touchesMetadata(geo, journal, [1126], [at(1107 * BLOCK)])).toBe(true);        // a journal pointer block
    expect(touchesMetadata(geo, journal, [1126], [at(1126 * BLOCK + 4)])).toBe(true);    // a file's single-indirect block
    expect(touchesMetadata(geo, journal, [1126], [at(8195 * BLOCK)])).toBe(true);        // group 1's block bitmap
  });

  it("does not count a change inside directory or file data", () => {
    expect(touchesMetadata(geo, journal, [1126], [at(69 * BLOCK)])).toBe(false);         // the root directory
    expect(touchesMetadata(geo, journal, [1126], [at(1112 * BLOCK), at(1125 * BLOCK)])).toBe(false);
    expect(touchesMetadata(geo, journal, [1126], [at(8261 * BLOCK)])).toBe(false);       // group 1's first data block
    expect(touchesMetadata(geo, [], [], [at(90 * BLOCK)])).toBe(false);                   // no journal (ext2): block 90 is data
    expect(touchesMetadata(geo, journal, [1126], [])).toBe(false);
  });

  it("words the corrupt note and the rewrite clauses for ext", () => {
    expect(CORRUPT_NOTE).toBe("Superblock or group descriptors do not parse; the tree is unavailable until a raw write repairs them.");
    expect(NOTES).toEqual({
      rewrite: "ext keeps the file's blocks and maps new ones after them",
      partialWrite: "the ext volume has no partial writes; the whole file was rewritten over its blocks",
    });
  });
});

describe("ExtJournal", () => {
  function fresh() {
    const vol = Volume.formatExt3(undefined);
    return { vol, journal: new ExtJournal(vol, () => vol.journalInfo()!) };
  }

  it("state() is the journal header without the inode number", () => {
    const { journal } = fresh();
    expect(journal.state()).toEqual({ mode: "ordered", sequence: 1, head: 1, start: 0, maxlen: 1024, firstBlock: 82, maxTransaction: 256, needsRecovery: false });
  });

  it("blocks() reads the ring: a create writes a descriptor, seven copies, and a commit, stale once checkpointed", () => {
    const { vol, journal } = fresh();
    expect(journal.blocks().filter((b) => b.kind !== "unused").map((b) => b.kind)).toEqual(["superblock"]);
    vol.createFile("/hello.txt", enc("Hello, ext3!"));
    const ring = journal.blocks();
    expect(ring).toHaveLength(1024);
    expect(ring[0]).toEqual({ index: 0, block: 82, kind: "superblock", tid: null, home: null, escaped: null, stale: false });
    const used = ring.filter((b) => b.kind !== "unused");
    expect(used.map((b) => b.kind)).toEqual(["superblock", "descriptor", "copy", "copy", "copy", "copy", "copy", "copy", "copy", "commit"]);
    expect(used.filter((b) => b.kind === "copy").map((b) => b.home)).toEqual([1, 2, 3, 4, 5, 6, 69]);
    expect(used.slice(1).every((b) => b.tid === 1 && b.stale)).toBe(true);
    expect(journal.state()).toMatchObject({ sequence: 2, head: 10, start: 0 });
  });

  it("arms, reports, and disarms a crash phase", () => {
    const { journal } = fresh();
    expect(journal.phase()).toBeNull();
    journal.arm("during_checkpoint");
    expect(journal.phase()).toBe("during_checkpoint");
    journal.disarm();
    expect(journal.phase()).toBeNull();
    journal.disarm(); // a no-op when nothing is armed
    expect(journal.phase()).toBeNull();
  });

  it("a crash leaves a live transaction; recover() replays it and clears the flag", () => {
    const { vol, journal } = fresh();
    journal.arm("after_commit");
    const rec = vol.createFile("/crash.txt", new Uint8Array(2 * BLOCK));
    expect(rec.events.at(-1)?.text).toBe("crashed after commit");
    expect(journal.phase()).toBeNull(); // the crash used the armed phase up
    expect(journal.state()).toMatchObject({ needsRecovery: true, start: 1 });
    expect(journal.blocks().filter((b) => b.kind !== "unused" && b.kind !== "superblock").every((b) => !b.stale)).toBe(true);
    const r = journal.recover();
    expect(r.op).toBe("recover");
    expect(r.events[0].text).toBe("scanned the journal from block 1, sequence 1: 1 committed transaction, 7 tagged blocks");
    expect(journal.state()).toMatchObject({ needsRecovery: false, start: 0 });
    expect(vol.listDir("/").map((e) => e.name)).toContain("crash.txt");
  });

  it("recover() on a clean journal is a record with no changes and one event", () => {
    const { journal } = fresh();
    const r = journal.recover();
    expect(r).toEqual({ op: "recover", changes: [], events: [{ kind: "recovery_scanned", text: "journal is clean; nothing to replay", region: { start: 0, end: 0 } }] });
  });
});
```

- [ ] **Step 15: Run it and watch it fail.**

```sh
pnpm exec vitest run tests/fs/ext.test.ts
```

Expected: `Error: Cannot find module '../../src/fs/ext/journal'`; `Test Files  1 failed (1)`.

- [ ] **Step 16: Write the metadata rule and the ext strings.** Create `web/ui/src/fs/ext/metadata.ts`:

```ts
import type { ExtGeometry } from "../../lib/wasm";
import type { ByteChangeLike } from "../../core/patch";
import type { FsAdapter } from "../adapter";

/**
 * True when any change starts in a block the core may re-parse or that moves what the layout
 * shows: a block below its group's first data block (the boot block, a superblock, the
 * descriptors, the bitmaps, an inode table), one of the journal's blocks (its data and its
 * pointer blocks), or an indirect block of any path. The store then re-reads the layout
 * (`ExtAdapter.touchesMetadata`). A change inside a file's data blocks is not metadata.
 */
export function touchesMetadata(
  geo: ExtGeometry,
  journalBlocks: readonly number[],
  indirectBlocks: readonly number[],
  changes: ByteChangeLike[],
): boolean {
  const journal = new Set(journalBlocks), indirect = new Set(indirectBlocks);
  return changes.some((c) => {
    const block = Math.floor(c.offset / geo.blockSize);
    const group = geo.groups.find((g) => block >= g.firstBlock && block < g.firstBlock + g.blockCount);
    const belowData = group ? block < group.firstData : block < geo.firstDataBlock;
    return belowData || journal.has(block) || indirect.has(block);
  });
}

/** DirTree's note while a raw write has left the superblock or descriptors unparsable. */
export const CORRUPT_NOTE = "Superblock or group descriptors do not parse; the tree is unavailable until a raw write repairs them.";

/**
 * The ext clauses the shell logs when a write had to rewrite a whole file: `write --append`
 * logs `appended ${n} bytes by rewriting the whole file (${NOTES.rewrite})`, and dd's
 * `--seek` overlay logs `NOTES.partialWrite` as a line of its own. The ext core overwrites a
 * file in place: it keeps the blocks the file has and maps new ones after them.
 */
export const NOTES: FsAdapter["notes"] = {
  rewrite: "ext keeps the file's blocks and maps new ones after them",
  partialWrite: "the ext volume has no partial writes; the whole file was rewritten over its blocks",
};
```

- [ ] **Step 17: Write the journal capability.** Create `web/ui/src/fs/ext/journal.ts`:

```ts
import type { JournalInfo, OpRecord, Volume } from "../../lib/wasm";
import { CRASH_PHASES, type CrashPhase, type JournalCapability, type JournalRingBlock, type JournalState } from "../adapter";

/**
 * The ext3 journal behind the seam's `JournalCapability`. `state()` answers from `info`, which
 * the adapter points at its cached `JournalInfo` (refreshed with it); `blocks()` reads the
 * volume every call, so callers memoise it per `volume.epoch`. The phase strings are wasm's
 * `armCrash` phases, which are already the seam's `CrashPhase`, so they pass through unchanged.
 * `recover()` returns the volume's op for the caller to run through the timeline.
 */
export class ExtJournal implements JournalCapability {
  private readonly vol: Volume;
  private readonly info: () => JournalInfo;

  constructor(vol: Volume, info: () => JournalInfo) {
    this.vol = vol;
    this.info = info;
  }

  state(): JournalState {
    const { mode, sequence, head, start, maxlen, firstBlock, maxTransaction, needsRecovery } = this.info();
    return { mode, sequence, head, start, maxlen, firstBlock, maxTransaction, needsRecovery };
  }

  blocks(): JournalRingBlock[] {
    return this.vol.journalBlocks();
  }

  arm(phase: CrashPhase): void {
    this.vol.armCrash(phase);
  }

  disarm(): void {
    this.vol.disarmCrash();
  }

  phase(): CrashPhase | null {
    const p = this.vol.crashPhase();
    return CRASH_PHASES.find((c) => c === p) ?? null;
  }

  recover(): OpRecord {
    return this.vol.recover();
  }
}
```

- [ ] **Step 18: Run it and watch it pass.**

```sh
pnpm exec vitest run tests/fs/ext.test.ts
```

Expected: `Test Files  1 passed (1)`, `Tests  12 passed (12)`.

#### Round 4: the adapter and the family

- [ ] **Step 19: Write the failing adapter tests.** In `web/ui/tests/fs/ext.test.ts`, add right after the `../../src/fs/ext/geometry` import (before the `journal` and `metadata` imports):

```ts
import { ExtAdapter, MKFS, asExt, ext } from "../../src/fs/ext";
import { fat16 } from "../../src/fs/fat16";
import { buildTree } from "../../src/core/tree";
```

and append at the end of the file (after a blank line):

```ts
/** The default ext3 disk with a directory (inode 12, block 1111), a one-block file (inode 13,
 *  block 1112), and a 13-block file (inode 14, blocks 1113..1125, its single-indirect block
 *  1126). The root's entries sit at block 69 + 0, 12, 24 (lost+found), 44, 56, 76. */
function fixture() {
  const vol = ext.format();
  vol.createDir("/docs");
  vol.createFile("/hello.txt", enc("Hello, ext3!"));
  vol.createFile("/bigger.txt", new Uint8Array(13 * BLOCK));
  return { vol, fs: asExt(ext.bind(vol)) };
}
const range = (from: number, to: number) => Array.from({ length: to - from + 1 }, (_, i) => from + i);

describe("ExtAdapter: identity and unit space", () => {
  it("is the ext family bound to its volume, named by the volume's type", () => {
    const { vol, fs } = fixture();
    expect(fs).toBeInstanceOf(ExtAdapter);
    expect(fs.id).toBe("ext");
    expect(fs.name).toBe("ext3");
    expect(fs.family).toBe(ext);
    expect(fs.vol).toBe(vol);
    expect(ext.bind(ext.format({ variant: "ext2" })).name).toBe("ext2");
    expect(() => asExt(fat16.bind(fat16.format()))).toThrow("not an ext adapter: fat16");
  });

  it("the family binds ext2 and ext3 and formats ext3 by default", () => {
    expect(ext.id).toBe("ext");
    expect(ext.name).toBe("ext");
    expect(ext.fsTypes).toEqual(["ext2", "ext3"]);
    expect(ext.mkfs).toBe(MKFS);
    const vol = ext.format();
    expect(vol.fsType()).toBe("ext3");
    expect(vol.extGeometry()).toEqual(geo);
    expect(vol.journalInfo()?.mode).toBe("ordered");
    expect(ext.format({ variant: "ext2", totalBlocks: 4096 }).fsType()).toBe("ext2");
    expect(ext.format({ totalBlocks: 4096, journalMode: "data" }).journalInfo()?.mode).toBe("data");
    expect(() => ext.format({ variant: "ext2", journalMode: "data" })).toThrow("journalBlocks and journalMode need variant ext3");
  });

  it("does block arithmetic over the volume's geometry", () => {
    const { fs } = fixture();
    expect(fs.unit).toBe(EXT_UNIT);
    expect(fs.sector).toBe(EXT_SECTOR);
    expect(unitIsSector(fs)).toBe(true);
    expect([fs.unitCount, fs.unitSize, fs.sectorSize, fs.totalSectors]).toEqual([16383, BLOCK, BLOCK, 16384]);
    expect(fs.unitOfSector(0)).toBeUndefined();
    expect(fs.unitOfSector(1112)).toBe(1112);
    expect(fs.unitByteRange(1112)).toEqual({ start: 1112 * BLOCK, end: 1113 * BLOCK });
    expect(fs.unitOfOffset(1112 * BLOCK + 3)).toBe(1112);
    expect(fs.unitStartsAt(1112)).toBe(true);
    expect(fs.colorForRegion(layout[7])).toBe(COLOR_JOURNAL);
    expect(fs.geo.groups.map((g) => g.firstData)).toEqual([69, 8261]);
  });
});

describe("ExtAdapter: owners, chains, and slots", () => {
  it("lists every path's blocks with their roles and first block, and no journal rows", () => {
    const { fs } = fixture();
    const bigger = (unit: number, role: "data" | "indirect") => ({ unit, path: "/bigger.txt", isDir: false, firstUnit: 1113, role });
    expect(fs.owners.filter((o) => o.path !== "/lost+found")).toEqual([
      { unit: 69, path: "/", isDir: true, firstUnit: 69, role: "directory" },
      { unit: 1111, path: "/docs", isDir: true, firstUnit: 1111, role: "directory" },
      { unit: 1112, path: "/hello.txt", isDir: false, firstUnit: 1112, role: "data" },
      ...range(1113, 1125).map((b) => bigger(b, "data")),
      bigger(1126, "indirect"),
    ]);
    expect(fs.owners.filter((o) => o.path === "/lost+found")).toEqual(range(70, 81).map((unit) => ({ unit, path: "/lost+found", isDir: true, firstUnit: 70, role: "directory" })));
    expect(fs.owners.some((o) => o.path === "<journal>")).toBe(false);
    expect(fs.owners.map((o) => o.unit)).toEqual([...fs.owners.map((o) => o.unit)].sort((a, b) => a - b));
    expect(fs.journalBlocks).toEqual(range(82, 1110)); // the journal's data 82..1105 and its pointer blocks 1106..1110
    expect(fs.ownerOf("/hello.txt")).toEqual({ unit: 1112, path: "/hello.txt", isDir: false, firstUnit: 1112, role: "data" });
    expect(fs.ownerOf("/nope")).toBeUndefined();
  });

  it("an ext2 volume has no journal blocks", () => {
    const fs = asExt(ext.bind(ext.format({ variant: "ext2" })));
    expect(fs.journalBlocks).toEqual([]);
    expect(fs.owners.map((o) => o.unit)).toEqual(range(69, 81)); // the root and lost+found
  });

  it("chains a file's data blocks in logical order, a directory's blocks, and [] where there are none", () => {
    const { vol, fs } = fixture();
    expect(fs.chain("/bigger.txt")).toEqual(range(1113, 1125)); // the pointer block 1126 is not data
    expect(fs.chain("/hello.txt")).toEqual([1112]);
    expect(fs.chain("/docs")).toEqual([1111]);
    expect(fs.chain("/")).toEqual([69]);
    expect(fs.chain("/nope")).toEqual([]);
    vol.createFile("/empty", new Uint8Array(0));
    fs.refresh();
    expect(fs.chain("/empty")).toEqual([]);
  });

  it("takes a path's first block from its chain, not from its lowest block", () => {
    // /a takes block 1111 and /b 1112; with /a deleted, /b grows into the freed 1111, which
    // becomes its second logical block.
    const vol = ext.format();
    vol.createFile("/a", enc("a"));
    vol.createFile("/b", enc("b"));
    vol.deleteFile("/a");
    vol.writeFile("/b", new Uint8Array(2 * BLOCK));
    const fs = asExt(ext.bind(vol));
    expect(fs.chain("/b")).toEqual([1112, 1111]);
    expect(fs.ownerOf("/b")?.firstUnit).toBe(1112);
    expect(fs.owners.filter((o) => o.path === "/b").map((o) => [o.unit, o.firstUnit])).toEqual([[1111, 1112], [1112, 1112]]);
    expect(fs.dataStart("/b")).toBe(1112 * BLOCK);
    expect(buildTree(vol, fs.owners).children.find((c) => c.path === "/b")?.firstUnit).toBe(1112); // the tree's "first block"
  });

  it("gives a path's inode slot as its entry slots, and the inode and entry offsets", () => {
    const { fs } = fixture();
    expect(fs.entrySlots("/hello.txt")).toEqual({ start: 6 * BLOCK + 0x200, end: 6 * BLOCK + 0x280 });
    expect(fs.entrySlots("/docs")).toEqual({ start: 6 * BLOCK + 0x180, end: 6 * BLOCK + 0x200 });
    expect(fs.entrySlots("/")).toEqual({ start: 5 * BLOCK + 0x80, end: 5 * BLOCK + 0x100 });
    expect(fs.entrySlots("/nope")).toBeNull();
    expect(fs.inodeOffset(2)).toBe(5 * BLOCK + 0x80);
    expect(fs.inodeOffset(8)).toBe(5 * BLOCK + 0x380);
    expect(fs.inodeOffset(11)).toBe(6 * BLOCK + 0x100);
    expect(() => fs.inodeOffset(0)).toThrow();
    expect(fs.dirEntryOffset("/docs")).toBe(69 * BLOCK + 44);
    expect(fs.dirEntryOffset("/hello.txt")).toBe(69 * BLOCK + 56);
    expect(fs.dirEntryOffset("/bigger.txt")).toBe(69 * BLOCK + 76);
    expect(fs.dirEntryOffset("/")).toBeNull();
    expect(fs.dirEntryOffset("/nope")).toBeNull();
  });

  it("locates the data of the root, a directory, a file, and nothing for an empty file", () => {
    const { vol, fs } = fixture();
    expect(fs.dataStart("/")).toBe(69 * BLOCK);
    expect(fs.dataStart("/docs")).toBe(1111 * BLOCK);
    expect(fs.dataStart("/bigger.txt")).toBe(1113 * BLOCK);
    expect(fs.dataStart("/nope")).toBeNull();
    vol.createFile("/empty", new Uint8Array(0));
    fs.refresh();
    expect(fs.dataStart("/empty")).toBeNull();
  });

  it("reads region starts from the layout", () => {
    const { fs } = fixture();
    expect(fs.regionStart("boot")).toBe(0);
    expect(fs.regionStart("metadata")).toBe(2);
    expect(fs.regionStart("allocationTable")).toBe(3);
    expect(fs.regionStart("data")).toBe(69);
    expect(fs.regionStart("journal")).toBe(82);
    expect(fs.regionStart("directory")).toBeUndefined();
  });
});

describe("ExtAdapter: what the shell and the Inspector print", () => {
  it("stat returns the ext facts, formatted as the shell prints them", () => {
    const { fs } = fixture();
    const hello = fs.stat("/hello.txt");
    expect(Object.keys(hello)).toEqual(["inode", "inodeOffset", "mode", "links", "blocks512", "dataBlocks", "indirectBlocks", "dirEntryOffset"]);
    expect(hello).toEqual({ inode: 13, inodeOffset: "0x1a00", mode: "0100644", links: 1, blocks512: 2, dataBlocks: [1112], indirectBlocks: [], dirEntryOffset: "0x11438" });
    expect(fs.stat("/bigger.txt")).toEqual({ inode: 14, inodeOffset: "0x1a80", mode: "0100644", links: 1, blocks512: 28, dataBlocks: range(1113, 1125), indirectBlocks: [1126], dirEntryOffset: "0x1144c" });
    expect(fs.stat("/")).toEqual({ inode: 2, inodeOffset: "0x1480", mode: "040755", links: 4, blocks512: 2, dataBlocks: [69], indirectBlocks: [], dirEntryOffset: "" });
  });

  it("df counts blocks from the superblock", () => {
    const { fs } = fixture();
    expect(fs.df()).toEqual({ unitSize: BLOCK, units: 16384, used: 1195, free: 15189 });
    expect(asExt(ext.bind(ext.format())).df()).toEqual({ unitSize: BLOCK, units: 16384, used: 1179, free: 15205 });
  });

  it("trace gives the entry, the inode, the pointer blocks, and the first data block", () => {
    const { vol, fs } = fixture();
    expect(fs.trace("/hello.txt")).toEqual([
      { label: "directory entry in / (block 69 + 0x38)", offset: 69 * BLOCK + 0x38 },
      { label: "inode 13 (block 6 + 0x200)", offset: 6 * BLOCK + 0x200 },
      { label: "data block 1112 (first of 1)", offset: 1112 * BLOCK },
    ]);
    expect(fs.trace("/bigger.txt")).toEqual([
      { label: "directory entry in / (block 69 + 0x4c)", offset: 69 * BLOCK + 0x4c },
      { label: "inode 14 (block 6 + 0x280)", offset: 6 * BLOCK + 0x280 },
      { label: "single-indirect block 1126", offset: 1126 * BLOCK },
      { label: "data block 1113 (first of 13)", offset: 1113 * BLOCK },
    ]);
    expect(fs.trace("/")).toEqual([
      { label: "inode 2 (block 5 + 0x80)", offset: 5 * BLOCK + 0x80 },
      { label: "data block 69 (first of 1)", offset: 69 * BLOCK },
    ]);
    vol.createFile("/docs/n.txt", new Uint8Array(0));
    fs.refresh();
    expect(fs.trace("/docs/n.txt")).toEqual([
      { label: "directory entry in /docs (block 1111 + 0x18)", offset: 1111 * BLOCK + 0x18 },
      { label: "inode 15 (block 6 + 0x300)", offset: 6 * BLOCK + 0x300 },
      { label: "no data blocks", offset: null },
    ]);
    expect(fs.trace("/nope")).toEqual([{ label: "no data blocks", offset: null }]);
  });

  it("names a double-indirect block and the level-1 blocks under it", () => {
    const vol = ext.format();
    vol.createFile("/huge.bin", new Uint8Array(269 * BLOCK)); // 12 direct + 256 single + 1 through the double
    const fs = asExt(ext.bind(vol));
    expect(fs.trace("/huge.bin").map((r) => r.label)).toEqual([
      "directory entry in / (block 69 + 0x2c)",
      "inode 12 (block 6 + 0x180)",
      "single-indirect block 1380",
      "double-indirect block 1381",
      "indirect block 1382 (level 1, under the double)",
      "data block 1111 (first of 269)",
    ]);
    expect(fs.describeUnit(1380)).toBe("single-indirect block of /huge.bin");
    expect(fs.describeUnit(1381)).toBe("double-indirect block of /huge.bin");
    expect(fs.describeUnit(1382)).toBe("indirect block of /huge.bin (level 1)");
    expect(fs.describeUnit(1379)).toBe("data block 268 of /huge.bin");
  });

  it("describes what a block holds for its owner, free, or nothing outside the data area", () => {
    const { fs } = fixture();
    expect(fs.describeUnit(1112)).toBe("data block 0 of /hello.txt");
    expect(fs.describeUnit(1125)).toBe("data block 12 of /bigger.txt");
    expect(fs.describeUnit(1126)).toBe("single-indirect block of /bigger.txt");
    expect(fs.describeUnit(69)).toBe("directory block 0 of /");
    expect(fs.describeUnit(81)).toBe("directory block 11 of /lost+found");
    expect(fs.describeUnit(1111)).toBe("directory block 0 of /docs");
    expect(fs.describeUnit(1127)).toBe("free");
    expect(fs.describeUnit(16383)).toBe("free");
    for (const b of [0, 1, 5, 68, 90, 1106, 1110, 8193, 8260, 16384, -1]) expect(fs.describeUnit(b), `block ${b}`).toBeNull();
  });

  it("annotates a block through the volume: block 1 is the superblock's fields", () => {
    const { fs } = fixture();
    const fields = fs.annotateSector(1);
    expect(fields[0]).toEqual({ range: { start: 0, end: 4 }, label: "inodes count", value: "1024" });
    expect(fields.find((a) => a.label === "magic")).toEqual({ range: { start: 0x38, end: 0x3a }, label: "magic", value: "0xEF53" });
  });

  it("parses i:N as inode N's slot and leaves every other form to the shell", () => {
    const { fs } = fixture();
    expect(fs.parseAddr("i:2")).toBe(5 * BLOCK + 0x80);
    expect(fs.parseAddr("I:13")).toBe(6 * BLOCK + 0x200);
    for (const v of ["i:0", "i:1025", "i:", "i:x", "b:3", "12"]) expect(fs.parseAddr(v), v).toBeUndefined();
    expect(fs.extraAddrHelp).toBe(", i:11 (inode)");
  });

  it("traces, stats, counts, describes, and parses ext2 the way it does ext3", () => {
    // The fixture's three paths on each variant. ext2 has no journal, so its first free block is
    // 82, right after lost+found; block 90 is bigger's seventh data block there and a journal
    // block on ext3.
    const cases = [
      { variant: "ext2", docs: 82, hello: 83, bigger: 84, used: 166, free: 16218, at90: "data block 6 of /bigger.txt" },
      { variant: "ext3", docs: 1111, hello: 1112, bigger: 1113, used: 1195, free: 15189, at90: null },
    ] as const;
    for (const c of cases) {
      const vol = ext.format({ variant: c.variant });
      vol.createDir("/docs");
      vol.createFile("/hello.txt", enc("Hello, ext3!"));
      vol.createFile("/bigger.txt", new Uint8Array(13 * BLOCK));
      const fs = asExt(ext.bind(vol));
      const pointer = c.bigger + 13;
      expect(fs.trace("/bigger.txt"), c.variant).toEqual([
        { label: "directory entry in / (block 69 + 0x4c)", offset: 69 * BLOCK + 0x4c },
        { label: "inode 14 (block 6 + 0x280)", offset: 6 * BLOCK + 0x280 },
        { label: `single-indirect block ${pointer}`, offset: pointer * BLOCK },
        { label: `data block ${c.bigger} (first of 13)`, offset: c.bigger * BLOCK },
      ]);
      expect(fs.stat("/hello.txt"), c.variant).toEqual({ inode: 13, inodeOffset: "0x1a00", mode: "0100644", links: 1, blocks512: 2, dataBlocks: [c.hello], indirectBlocks: [], dirEntryOffset: "0x11438" });
      expect(fs.stat("/bigger.txt").indirectBlocks, c.variant).toEqual([pointer]);
      expect(fs.df(), c.variant).toEqual({ unitSize: BLOCK, units: 16384, used: c.used, free: c.free });
      expect(fs.describeUnit(c.docs), c.variant).toBe("directory block 0 of /docs");
      expect(fs.describeUnit(c.hello), c.variant).toBe("data block 0 of /hello.txt");
      expect(fs.describeUnit(pointer), c.variant).toBe("single-indirect block of /bigger.txt");
      expect(fs.describeUnit(pointer + 1), c.variant).toBe("free");
      expect(fs.describeUnit(90), c.variant).toBe(c.at90);
      expect(fs.describeUnit(68), c.variant).toBeNull();
      expect(fs.parseAddr("i:13"), c.variant).toBe(6 * BLOCK + 0x200);
      expect(fs.parseAddr("i:1025"), c.variant).toBeUndefined();
    }
  });

  it("matches names exactly and flags inode writes as metadata, not data writes", () => {
    const { vol, fs } = fixture();
    expect(fs.namesMatch("hello.txt", "hello.txt")).toBe(true);
    expect(fs.namesMatch("hello.txt", "HELLO.TXT")).toBe(false);
    expect(fs.touchesMetadata(vol.writeRaw(1112 * BLOCK, enc("J")).changes)).toBe(false);                    // hello's data
    expect(fs.touchesMetadata(vol.writeRaw(6 * BLOCK + 0x200 + 0x1a, new Uint8Array([1])).changes)).toBe(true); // hello's inode (i_links_count)
    expect(fs.touchesMetadata([at(1126 * BLOCK)])).toBe(true);                                                // bigger's indirect block
    expect(fs.touchesMetadata([at(1107 * BLOCK)])).toBe(true);                                                // a journal pointer block
    expect(fs.touchesMetadata(vol.createFile("/new.txt", enc("x")).changes)).toBe(true);
    expect(fs.touchesMetadata([])).toBe(false);
  });

  it("words the corrupt note and the rewrite clauses for ext", () => {
    const { fs } = fixture();
    expect(fs.corruptNote).toBe(CORRUPT_NOTE);
    expect(fs.notes).toBe(NOTES);
  });
});

describe("ExtAdapter: caches and the journal capability", () => {
  it("answers from the last refresh() until the next one", () => {
    const { vol, fs } = fixture();
    vol.createFile("/new.txt", enc("x"));
    expect(fs.ownerOf("/new.txt")).toBeUndefined();
    expect(fs.df().free).toBe(15189);
    expect(fs.journal!.state().sequence).toBe(4);
    fs.refresh();
    expect(fs.ownerOf("/new.txt")).toEqual({ unit: 1127, path: "/new.txt", isDir: false, firstUnit: 1127, role: "data" });
    expect(fs.df().free).toBe(15188);
    expect(fs.journal!.state().sequence).toBe(5);
  });

  it("has a journal on ext3 only, and nothing to recover after a clean format", () => {
    const { fs } = fixture();
    expect(fs.journal).toBeInstanceOf(ExtJournal);
    expect(fs.needsRecovery).toBe(false);
    expect(fs.journal!.state()).toEqual({ mode: "ordered", sequence: 4, head: 30, start: 0, maxlen: 1024, firstBlock: 82, maxTransaction: 256, needsRecovery: false });
    const journal = fs.journal;
    fs.refresh();
    expect(fs.journal).toBe(journal); // the same capability across refreshes
    const ext2 = asExt(ext.bind(ext.format({ variant: "ext2" })));
    expect(ext2.journal).toBeUndefined();
    expect(ext2.needsRecovery).toBe(false);
  });

  it("the journal's blocks after a create: the ring the Journal panel draws", () => {
    const vol = ext.format();
    const fs = asExt(ext.bind(vol));
    vol.createFile("/hello.txt", enc("Hello, ext3!"));
    fs.refresh();
    const used = fs.journal!.blocks().filter((b) => b.kind !== "unused");
    expect(used.map((b) => [b.index, b.block, b.kind])).toEqual([
      [0, 82, "superblock"], [1, 83, "descriptor"],
      [2, 84, "copy"], [3, 85, "copy"], [4, 86, "copy"], [5, 87, "copy"], [6, 88, "copy"], [7, 89, "copy"], [8, 90, "copy"],
      [9, 91, "commit"],
    ]);
  });

  it("a crash makes the volume need recovery; recover() returns the record that clears it", () => {
    const { vol, fs } = fixture();
    const journal = fs.journal!;
    journal.arm("after_commit");
    expect(journal.phase()).toBe("after_commit");
    vol.createFile("/crash.txt", new Uint8Array(2 * BLOCK));
    expect(fs.needsRecovery).toBe(false); // the cache, until refresh()
    fs.refresh();
    expect(fs.needsRecovery).toBe(true);
    expect(journal.state().needsRecovery).toBe(true);
    expect(() => vol.createFile("/more.txt", enc("x"))).toThrow();
    const rec = journal.recover();
    expect(rec.op).toBe("recover");
    expect(rec.changes.length).toBeGreaterThan(0);
    fs.refresh();
    expect(fs.needsRecovery).toBe(false);
    expect(journal.state().needsRecovery).toBe(false);
    expect(fs.ownerOf("/crash.txt")?.firstUnit).toBe(1127);
  });
});
```

- [ ] **Step 20: Run it and watch it fail.**

```sh
pnpm exec vitest run tests/fs/ext.test.ts
```

Expected: `Error: Cannot find module '../../src/fs/ext'`; `Test Files  1 failed (1)`.

- [ ] **Step 21: Write the adapter and the family.** Create `web/ui/src/fs/ext/adapter.ts`:

```ts
import { Volume, type Annotation, type ExtBlockOwner, type ExtFileBlocks, type ExtGeometry, type ExtIndirectBlock, type ExtInode, type ExtSuperblock, type JournalInfo } from "../../lib/wasm";
import type { Interval } from "../../core/intervals";
import type { ByteChangeLike } from "../../core/patch";
import type { DfFacts, FsAdapter, FsFamily, FsFamilyId, StatFacts, TraceRow, UnitOwner, UnitSpace } from "../adapter";
import { SpaceAdapter, hexAddr } from "../base";
import { MKFS, toWasmOptions, type ExtFamilyOptions } from "./format";
import { extSpace } from "./geometry";
import { ExtJournal } from "./journal";
import { CORRUPT_NOTE, NOTES, touchesMetadata as changesTouchMetadata } from "./metadata";

/** The family id. The cast goes when Task 4 adds "ext" to `FsFamilyId` and registers the family. */
const EXT_ID = "ext" as string as FsFamilyId;

/** The path `blockOwners()` gives the journal's blocks (inode 8's data and pointer blocks). */
const JOURNAL_PATH = "<journal>";

/** An absolute byte offset as `block B + 0xNN`: its block and the offset inside it. */
function blockPlus(offset: number, blockSize: number): string {
  return `block ${Math.floor(offset / blockSize)} + ${hexAddr(offset % blockSize)}`;
}

/** `/docs/n.txt` -> `/docs`; a root entry -> `/`. */
function parentOf(path: string): string {
  const i = path.lastIndexOf("/");
  return i <= 0 ? "/" : path.slice(0, i);
}

type PathRow = ExtBlockOwner & { role: "data" | "directory" | "indirect" };
const isPathRow = (r: ExtBlockOwner): r is PathRow => r.path !== JOURNAL_PATH && r.role !== "journal";

/** The non-journal rows in the generic shape. `firstUnit` is the path's first data (or
 *  directory) block in logical order, `firstBlockOf(path)`: not always its lowest, because a file
 *  that grows after a lower block was freed maps that block later. Where the path cannot be
 *  walked (a corrupt volume) it falls back to the path's lowest data or directory block. */
function toUnitOwners(rows: ExtBlockOwner[], firstBlockOf: (path: string) => number | undefined): UnitOwner[] {
  const pathRows = rows.filter(isPathRow);
  const first = new Map<string, number>();
  for (const r of pathRows) if (r.role !== "indirect" && !first.has(r.path)) first.set(r.path, r.block);
  for (const [path, lowest] of first) first.set(path, firstBlockOf(path) ?? lowest);
  return pathRows.map((r) => ({ unit: r.block, path: r.path, isDir: r.role === "directory", firstUnit: first.get(r.path) ?? r.block, role: r.role }));
}

/** Which pointer block `ind` is for its inode: `i_block[12]` is the single-indirect block, a
 *  level-2 block the double-indirect one, and any other level-1 block sits under the double. */
function indirectKind(inode: ExtInode | null, ind: ExtIndirectBlock): "single" | "double" | "under" {
  if (ind.level === 2) return "double";
  return inode?.block[12] === ind.block ? "single" : "under";
}

/**
 * ext2 and ext3 behind the adapter. `refresh()` reads the geometry, the superblock, the block
 * owners, and the journal's header and recovery flag into plain fields (the reactivity rule
 * in fs/adapter.ts); none of those calls fails on a corrupt volume. Path walks (`fileBlocks`,
 * `inodeNumber`, `dirEntries`) go through the core's corruption gate, so the methods that
 * need them answer "nothing" (`[]`, `null`, the muted trace row) while the volume is corrupt.
 * Journal blocks never become units: their owner rows go to `journalBlocks`, not `owners`.
 * The unit arithmetic, `ownerOf`, and `regionStart` are the shared `SpaceAdapter`'s (fs/base.ts).
 */
export class ExtAdapter extends SpaceAdapter implements FsAdapter {
  readonly id: FsFamilyId = EXT_ID;
  /** The bound volume's type: "ext3" with a journal, else "ext2". */
  name: "ext2" | "ext3" = "ext3";
  readonly family: FsFamily<ExtFamilyOptions> = ext;
  readonly vol: Volume;
  geo!: ExtGeometry;
  sb!: ExtSuperblock;
  /** The directory, data, and indirect blocks of every path, in block order; no journal rows. */
  owners: UnitOwner[] = [];
  /** The journal's blocks (its data blocks and its pointer blocks), ascending; [] on ext2. */
  journalBlocks: number[] = [];
  journal?: ExtJournal;
  needsRecovery = false;
  readonly corruptNote = CORRUPT_NOTE;
  readonly notes = NOTES;
  readonly extraAddrHelp = ", i:11 (inode)";
  protected space!: UnitSpace;
  private info: JournalInfo | undefined;
  private byUnit = new Map<number, UnitOwner>();
  private journalSet = new Set<number>();
  private indirectBlocks: number[] = [];
  // Path walks, memoised until the next refresh(); null when the walk failed.
  private files = new Map<string, ExtFileBlocks | null>();
  private inodes = new Map<string, ExtInode | null>();

  constructor(vol: Volume) {
    super();
    this.vol = vol;
    this.refresh();
  }

  refresh(): void {
    this.files.clear();
    this.inodes.clear();
    this.name = this.vol.fsType() === "ext2" ? "ext2" : "ext3";
    this.geo = this.vol.extGeometry();
    this.space = extSpace(this.geo);
    this.sb = this.vol.extSuperblock();
    const rows = this.vol.blockOwners();
    this.owners = toUnitOwners(rows, (path) => this.blocksOf(path)?.data[0]);
    this.byUnit = new Map(this.owners.map((o) => [o.unit, o]));
    this.indirectBlocks = this.owners.filter((o) => o.role === "indirect").map((o) => o.unit);
    this.journalBlocks = rows.filter((r) => r.path === JOURNAL_PATH).map((r) => r.block);
    this.journalSet = new Set(this.journalBlocks);
    this.info = this.vol.journalInfo();
    this.needsRecovery = this.vol.needsRecovery();
    this.journal = this.info ? (this.journal ?? new ExtJournal(this.vol, () => this.journalInfo())) : undefined;
  }

  /** The cached journal header; the capability's `state()` reads it. */
  private journalInfo(): JournalInfo {
    if (!this.info) throw new Error("this volume has no journal");
    return this.info;
  }

  private blocksOf(path: string): ExtFileBlocks | null {
    if (!this.files.has(path)) {
      let blocks: ExtFileBlocks | null = null;
      try { blocks = this.vol.fileBlocks(path); } catch { blocks = null; }
      this.files.set(path, blocks);
    }
    return this.files.get(path) ?? null;
  }

  private inodeOf(path: string): ExtInode | null {
    if (!this.inodes.has(path)) {
      let inode: ExtInode | null = null;
      try { inode = this.vol.extInode(this.vol.inodeNumber(path)); } catch { inode = null; }
      this.inodes.set(path, inode);
    }
    return this.inodes.get(path) ?? null;
  }

  /** The path's data blocks in logical order (a directory's blocks for a directory); [] when none. */
  chain(path: string): number[] {
    return [...(this.blocksOf(path)?.data ?? [])];
  }

  /** The path's inode: its 128-byte slot in the inode table (`/` is inode 2). */
  entrySlots(path: string): Interval | null {
    const inode = this.inodeOf(path);
    return inode ? { start: inode.slot.offset, end: inode.slot.offset + this.geo.inodeSize } : null;
  }

  /** The first byte of the path's first block: the root directory's block for `/`. */
  dataStart(path: string): number | null {
    const first = this.blocksOf(path)?.data[0];
    return first === undefined ? null : this.unitByteRange(first).start;
  }

  /** The absolute byte offset of inode `ino`'s slot; throws `NotFound` for 0 or past the last inode. */
  inodeOffset(ino: number): number {
    return this.vol.extInode(ino).slot.offset;
  }

  /** The absolute byte offset of the directory entry naming `path` in its parent; null for `/`
   *  and for a path that is not there. */
  dirEntryOffset(path: string): number | null {
    if (path === "/") return null;
    const name = path.slice(path.lastIndexOf("/") + 1);
    let entries;
    try { entries = this.vol.dirEntries(parentOf(path)); } catch { return null; }
    return entries.find((e) => e.inode !== 0 && e.name === name)?.offset ?? null;
  }

  /** The ext facts `stat` prints after the generic ones, formatted as the shell prints them. */
  stat(path: string): StatFacts {
    const inode = this.inodeOf(path);
    const blocks = this.blocksOf(path);
    const entry = this.dirEntryOffset(path);
    return {
      inode: inode?.ino ?? 0,
      inodeOffset: inode ? hexAddr(inode.slot.offset) : "",
      mode: inode ? `0${inode.mode.toString(8)}` : "",
      links: inode?.links ?? 0,
      blocks512: inode?.blocks ?? 0,
      dataBlocks: blocks ? [...blocks.data] : [],
      indirectBlocks: blocks ? blocks.indirect.map((b) => b.block) : [],
      dirEntryOffset: entry === null ? "" : hexAddr(entry),
    };
  }

  /** Block usage from the cached superblock's counters. */
  df(): DfFacts {
    const { blocksCount, freeBlocks } = this.sb;
    return { unitSize: this.geo.blockSize, units: blocksCount, used: blocksCount - freeBlocks, free: freeBlocks };
  }

  /** The Inspector's "Selected file" rows: the entry in the parent, the inode, the pointer
   *  blocks, then the first data block (or the muted `no data blocks`). */
  trace(path: string): TraceRow[] {
    const bs = this.geo.blockSize;
    const rows: TraceRow[] = [];
    const entry = this.dirEntryOffset(path);
    if (entry !== null) rows.push({ label: `directory entry in ${parentOf(path)} (${blockPlus(entry, bs)})`, offset: entry });
    const inode = this.inodeOf(path);
    if (inode) rows.push({ label: `inode ${inode.ino} (${blockPlus(inode.slot.offset, bs)})`, offset: inode.slot.offset });
    const blocks = this.blocksOf(path);
    for (const ind of blocks?.indirect ?? []) {
      const kind = indirectKind(inode, ind);
      const label = kind === "single" ? `single-indirect block ${ind.block}` : kind === "double" ? `double-indirect block ${ind.block}` : `indirect block ${ind.block} (level 1, under the double)`;
      rows.push({ label, offset: ind.block * bs });
    }
    const data = blocks?.data ?? [];
    if (data.length > 0) rows.push({ label: `data block ${data[0]} (first of ${data.length})`, offset: data[0] * bs });
    else rows.push({ label: "no data blocks", offset: null });
    return rows;
  }

  /** What block `b` holds for its owner, or `free`; null outside the data area (block 0, a
   *  group's metadata, the journal's blocks, past the end). */
  describeUnit(b: number): string | null {
    const g = this.geo;
    if (!Number.isInteger(b) || b < g.firstDataBlock || b >= g.totalBlocks || this.journalSet.has(b)) return null;
    const group = g.groups.find((x) => b >= x.firstBlock && b < x.firstBlock + x.blockCount);
    if (!group || b < group.firstData) return null;
    const owner = this.byUnit.get(b);
    if (!owner) return "free";
    const { path } = owner;
    if (owner.role === "indirect") {
      const ind = this.blocksOf(path)?.indirect.find((x) => x.block === b) ?? { block: b, level: 1 };
      const kind = indirectKind(this.inodeOf(path), ind);
      return kind === "single" ? `single-indirect block of ${path}` : kind === "double" ? `double-indirect block of ${path}` : `indirect block of ${path} (level 1)`;
    }
    let i = this.chain(path).indexOf(b);
    // A path the core can no longer walk (a corrupt volume): count its blocks in block order.
    if (i < 0) i = this.owners.filter((o) => o.path === path && o.role === owner.role).findIndex((o) => o.unit === b);
    return `${owner.role === "directory" ? "directory" : "data"} block ${i} of ${path}`;
  }

  annotateSector(sector: number): Annotation[] {
    return this.vol.annotateSector(sector);
  }

  /** `i:N`, inode N's slot; undefined for anything else, and for an inode that does not exist. */
  parseAddr(v: string): number | undefined {
    const m = /^i:(\d+)$/i.exec(v);
    if (!m) return undefined;
    try { return this.inodeOffset(Number(m[1])); } catch { return undefined; }
  }

  /** ext names are case-sensitive bytes. */
  namesMatch(a: string, b: string): boolean {
    return a === b;
  }

  touchesMetadata(changes: ByteChangeLike[]): boolean {
    return changesTouchMetadata(this.geo, this.journalBlocks, this.indirectBlocks, changes);
  }
}

/**
 * The ext family: ext2 and ext3 under one adapter, `ext.format()` the default ext3 disk every
 * ext lesson runs on. Defined beside the class for the reason fat16's is (the two refer to each
 * other; one module avoids an import cycle vite-node cannot resolve). index.ts re-exports it.
 */
export const ext: FsFamily<ExtFamilyOptions> = {
  id: EXT_ID,
  name: "ext",
  fsTypes: ["ext2", "ext3"],
  format: (options) => {
    const w = toWasmOptions(options ?? {});
    return w.variant === "ext2" ? Volume.formatExt2(w.options) : Volume.formatExt3(w.options);
  },
  bind: (vol) => new ExtAdapter(vol),
  mkfs: MKFS,
};
```

- [ ] **Step 22: Write the family's index.** Create `web/ui/src/fs/ext/index.ts`:

```ts
import type { FsAdapter } from "../adapter";
import type { ExtAdapter } from "./adapter";

/** The ext family: `ext.format` is the one place `Volume.formatExt2`/`formatExt3` are called,
 *  and `ext.bind` makes the adapter. It is defined beside the class (see adapter.ts). */
export { ext, ExtAdapter } from "./adapter";

/** The ext adapter behind a generic one, for the few callers that need an ext-only fact (the
 *  block-group map's `geo`, the journal panel, the lessons' `inodeOffset`). The `as string`
 *  goes when "ext" joins `FsFamilyId`. */
export function asExt(fs: FsAdapter): ExtAdapter {
  if ((fs.id as string) !== "ext") throw new Error(`not an ext adapter: ${fs.id}`);
  return fs as ExtAdapter;
}

export * from "./geometry";
export * from "./format";
export { ExtJournal } from "./journal";
export { CORRUPT_NOTE, NOTES, touchesMetadata } from "./metadata";
```

- [ ] **Step 23: Run it and watch it pass.**

```sh
pnpm exec vitest run tests/fs/ext.test.ts
```

Expected: `Test Files  1 passed (1)`, `Tests  36 passed (36)`.

#### Round 5: the boundary scan learns the ext-only surface

- [ ] **Step 24: Write the failing positive controls.** In `web/ui/tests/adapterBoundary.test.ts`, add this test right before `it("keeps FAT-only wasm calls, FAT-only wasm types and fs/fat16 imports out of the generic code", ...)`:

```ts
  it("recognises each kind of ext leak (positive controls)", () => {
    expect(files).toContain("fs/ext/adapter.ts");
    expect(violations("state/volume.svelte.ts", "const g = vol.extGeometry(); vol.needsRecovery();")).toEqual([
      "state/volume.svelte.ts: calls .extGeometry( (ext-only wasm method; go through the adapter)",
      "state/volume.svelte.ts: calls .needsRecovery( (ext-only wasm method; go through the adapter)",
    ]);
    for (const call of ["blockOwners()", "inodeNumber(p)", "extInode(2)", "dirEntries(p)", "fileBlocks(p)", "blockGroupCount()", "journalInfo()", "journalBlocks()", "armCrash(x)", "disarmCrash()", "crashPhase()", "extSuperblock()"]) {
      expect(violations("shell/commands.ts", `vol.${call};`), call).toHaveLength(1);
    }
    expect(violations("scenarios/crashRecover.ts", "Volume.formatExt3(undefined); v.recover();")).toEqual([
      "scenarios/crashRecover.ts: calls .formatExt3( (ext-only wasm method; go through the adapter)",
      "scenarios/crashRecover.ts: calls .recover( (ext-only wasm method; go through the adapter)",
    ]);
    expect(violations("shell/host.ts", 'import { Volume, type ExtGeometry } from "../lib/wasm";')).toEqual([
      "shell/host.ts: imports ExtGeometry from lib/wasm (ext-only wasm type; use the fs/adapter types)",
    ]);
    expect(violations("components/StatusLine.svelte", 'import type { JournalInfo } from "fs-emulator-wasm";')).toEqual([
      "components/StatusLine.svelte: imports JournalInfo from fs-emulator-wasm (ext-only wasm type; use the fs/adapter types)",
    ]);
    for (const t of ["ExtGroup", "ExtSuperblock", "ExtBlockOwner", "ExtInode", "ExtDirEntry", "ExtFileBlocks", "ExtFormatOptions", "Ext3FormatOptions", "JournalBlock"]) {
      expect(violations("core/tree.ts", `import type { ${t} } from "../lib/wasm";`), t).toHaveLength(1);
    }
    expect(violations("components/Inspector.svelte", 'import { asExt } from "../fs/ext";')).toEqual([
      'components/Inspector.svelte: imports from fs/ext (from "../fs/ext"); only fs/index.ts, fs/panels.ts and scenarios/ may',
    ]);
    expect(violations("state/layers.svelte.ts", 'import { ExtJournal } from "../fs/ext/journal";')).toHaveLength(1);
    // The allowed places, and the generic surface, pass: the journal capability's recover(),
    // the adapter's needsRecovery field, and the family-neutral wasm calls.
    expect(violations("fs/ext/adapter.ts", 'this.geo = this.vol.extGeometry(); import type { ExtGeometry } from "../../lib/wasm";')).toEqual([]);
    expect(violations("lib/wasm.ts", 'export type { ExtGeometry, JournalInfo } from "fs-emulator-wasm";')).toEqual([]);
    expect(violations("fs/index.ts", 'import { ext } from "./ext";')).toEqual([]);
    expect(violations("fs/panels.ts", 'import BlockGroupMap from "./ext/BlockGroupMap.svelte";')).toEqual([]);
    expect(violations("scenarios/crashRecover.ts", 'import { asExt } from "../fs/ext"; asExt(fs).journal!.recover();')).toEqual([]);
    expect(violations("state/scenarios.svelte.ts", "volume.run(() => journal.recover());")).toEqual([]);
    expect(violations("shell/commands.ts", "host.run(() => host.adapter.journal!.recover()); host.adapter.journal?.recover(); volume.needsRecovery = fs.needsRecovery;")).toEqual([]);
    expect(violations("state/volume.svelte.ts", 'import { Volume, type FsError, type OpRecord, type Region } from "../lib/wasm";')).toEqual([]);
  });

```

- [ ] **Step 25: Run it and watch it fail.**

```sh
pnpm exec vitest run tests/adapterBoundary.test.ts
```

Expected: `× the adapter boundary > recognises each kind of ext leak (positive controls)` with `AssertionError: expected [] to deeply equal [ …(2) ]`; `Tests  1 failed | 3 passed (4)`.

- [ ] **Step 26: Teach the scan the ext rules.** In `web/ui/tests/adapterBoundary.test.ts`, replace the comment block above `const SRC` (the five lines starting `// The adapter seam, enforced as a source scan`) with:

```ts
// The adapter seam, enforced as a source scan (spec 2026-09-23-fs-adapter-design.md, section 6;
// spec 2026-09-24-ext-explorer-design.md, section 8). Outside src/fs/fat16/ and src/lib/wasm.ts
// nothing may call a FAT-only wasm method or import a FAT-only wasm type, and only the registry,
// the panel table and the scenarios may import from fs/fat16; the same holds for the ext-only
// surface and src/fs/ext/. The scan is textual, like layout.test.ts: a comment that spells
// `vol.geometry()` trips it too, so reword the comment rather than the rule.
```

add after the `FAT16_IMPORT` constant (and a blank line):

```ts
/** Files the ext-only wasm surface is allowed in. */
const EXT_ONLY_ALLOWED = (rel: string) => rel.startsWith("fs/ext/") || rel === "lib/wasm.ts";
/** Files that may import from fs/ext (besides fs/ext itself): the same three as for fs/fat16. */
const EXT_IMPORTERS = FAT16_IMPORTERS;

// `recover` is also the journal capability's method, which generic code calls through a
// receiver named `journal` (`journal.recover()`, `journal!.recover()`, `journal?.recover()`),
// so only a `.recover(` on any other receiver counts.
const EXT_ONLY_CALL = /\.(extGeometry|extSuperblock|blockOwners|inodeNumber|extInode|dirEntries|fileBlocks|blockGroupCount|journalInfo|journalBlocks|armCrash|disarmCrash|crashPhase|needsRecovery|formatExt2|formatExt3)\s*\(|(?<!\bjournal[!?]?)\.(recover)\s*\(/g;
const EXT_ONLY_TYPE = /\b(ExtGeometry|ExtGroup|ExtSuperblock|ExtBlockOwner|ExtBlockOwnerRole|ExtInode|ExtInodeSlot|ExtDirEntry|ExtFileBlocks|ExtIndirectBlock|ExtFormatOptions|Ext3FormatOptions|JournalInfo|JournalBlock|JournalBlockKind|JournalMode)\b/;
const EXT_IMPORT = /from\s+["'](?:\.{1,2}\/)+(?:fs\/)?ext(?:\/[^"']*)?["']/g;
```

and in `violations`, add after the `fs/fat16` import check's closing `}` and before `return found;`:

```ts
  if (!EXT_ONLY_ALLOWED(rel)) {
    for (const m of text.matchAll(EXT_ONLY_CALL)) found.push(`${rel}: calls .${m[1] ?? m[2]}( (ext-only wasm method; go through the adapter)`);
    for (const m of text.matchAll(WASM_IMPORT)) {
      const t = EXT_ONLY_TYPE.exec(m[1]);
      const source = m[2].endsWith("lib/wasm") ? "lib/wasm" : m[2];
      if (t) found.push(`${rel}: imports ${t[1]} from ${source} (ext-only wasm type; use the fs/adapter types)`);
    }
  }
  if (!rel.startsWith("fs/ext/") && !EXT_IMPORTERS(rel)) {
    for (const m of text.matchAll(EXT_IMPORT)) found.push(`${rel}: imports from fs/ext (${m[0].trim()}); only fs/index.ts, fs/panels.ts and scenarios/ may`);
  }
```

- [ ] **Step 27: Run it and watch it pass.** The full-tree scan (`keeps FAT-only wasm calls ...`) now applies the ext rules to every file under `src/` too; the only generic-code mention of `recover()` is the `journal.recover()` in `fs/adapter.ts`'s comment, which the receiver rule allows.

```sh
pnpm exec vitest run tests/adapterBoundary.test.ts
```

Expected: `Test Files  1 passed (1)`, `Tests  4 passed (4)`.

#### Round 6: the tree's root and the free label on ext

Two pieces of generic code meet ext here. `buildTree` gave the root `firstUnit: null` because FAT's root is a fixed region that owns no cluster; ext's root directory owns block 69 and has an owner row (`{ unit: 69, path: "/", ... }`), so the Files tree's root row read `0 B · no data` on ext. The root now takes its first unit from the owners' `/` row when there is one, and FAT, which has none, keeps `null`. The ribbon's free label (Task 2's `freeSpaceLabel`, over the family's `freeUnits()`) would read `16.0 MB free` on the default ext3 disk with `SpaceAdapter`'s count of blocks without an owner row, because the inode tables, the bitmaps, and the journal have none; `ExtAdapter` overrides `freeUnits()` with the superblock's free count, and the label is pinned at `14.8 MB free`.

- [ ] **Step 28: Write the failing tests.** In `web/ui/tests/fs/ext.test.ts` add after `import { buildTree } from "../../src/core/tree";`:

```ts
import { freeSpaceLabel } from "../../src/core/freeSpace";
```

inside `describe("ExtAdapter: owners, chains, and slots", ...)`, add after the test `takes a path's first block from its chain, not from its lowest block`:

```ts

  it("gives the tree's root the root directory's block as its first block", () => {
    // ext's root is a directory with a block and an owner row of its own; FAT's root is a fixed
    // region with no row, and its tree root stays null (tests/integration.test.ts).
    const { vol, fs } = fixture();
    expect(fs.ownerOf("/")?.firstUnit).toBe(69);
    expect(buildTree(vol, fs.owners).firstUnit).toBe(69);
  });
```

and inside `describe("ExtAdapter: what the shell and the Inspector print", ...)`, add after the test `df counts blocks from the superblock`:

```ts

  it("labels the ribbon's free space from df, not from the blocks no file owns", () => {
    // 15,205 free blocks of 1 KiB; the blocks no file owns (16.0 MB) include the inode tables,
    // the bitmaps, and the journal.
    const fs = asExt(ext.bind(ext.format()));
    expect(fs.freeUnits()).toBe(15205);
    expect(freeSpaceLabel(fs)).toBe("14.8 MB free");
  });
```

In `web/ui/tests/integration.test.ts`, in the test `the tree reports null, not 0, for the root and for an empty file: neither owns a unit`, replace

```ts
    const tree = buildTree(vol, adapterFor(vol).owners);
    expect(tree.firstUnit).toBeNull();
```

with

```ts
    const fs = adapterFor(vol);
    const tree = buildTree(vol, fs.owners);
    expect(fs.ownerOf("/")).toBeUndefined(); // FAT's root is a fixed region, not a cluster: no owner row
    expect(tree.firstUnit).toBeNull();
```

(one assertion added; no FAT expectation changes).

- [ ] **Step 29: Run them and watch the root and free-label tests fail.**

```sh
pnpm exec vitest run tests/fs/ext.test.ts tests/integration.test.ts
```

Expected: `Test Files  1 failed | 1 passed (2)`, `Tests  2 failed | 42 passed (44)`: `ExtAdapter: owners, chains, and slots > gives the tree's root the root directory's block as its first block` fails with `expected null to be 69`, and `ExtAdapter: what the shell and the Inspector print > labels the ribbon's free space from df, not from the blocks no file owns` with `expected 16370 to be 15205` (`SpaceAdapter` counts the 16,383 blocks from 1 less the 13 with owner rows: the root directory and lost+found).

- [ ] **Step 30: Give the root its first unit from the owners, and ext its free count.** In `web/ui/src/core/tree.ts` replace the first two lines of `buildTree`'s doc comment

```ts
/** Recursive listing; first units come from the owner map (`null` for the root and for an
 *  empty file, which own no unit). `listDir` is gated: while the volume is corrupt (a raw
```

with

```ts
/** Recursive listing; first units come from the owner map, `null` where a path owns no unit:
 *  an empty file, and FAT's root (a fixed region with no owner row); ext's root directory owns
 *  a block, so the root has one there. `listDir` is gated: while the volume is corrupt (a raw
```

and the root's return line

```ts
  return { name: "/", path: "/", isDir: true, size: 0, firstUnit: null, children };
```

with

```ts
  return { name: "/", path: "/", isDir: true, size: 0, firstUnit: firstByPath.get("/") ?? null, children };
```

In `web/ui/src/fs/ext/adapter.ts` add after `df()`'s closing brace (and a blank line):

```ts
  /** The superblock's free count, not the blocks without an owner row: those include the inode
   *  tables, the bitmaps, and the journal. */
  freeUnits(): number {
    return this.df().free;
  }
```

- [ ] **Step 31: Run them and watch them pass.**

```sh
pnpm exec vitest run tests/fs/ext.test.ts tests/integration.test.ts
```

Expected: `Test Files  2 passed (2)`, `Tests  44 passed (44)`.

#### Gates and commit

- [ ] **Step 32: Run the web gates.**

```sh
pnpm test && pnpm build
```

Expected: `pnpm test` prints `Test Files  44 passed (44)` and `Tests  394 passed (394)`; `pnpm build` prints svelte-check's `COMPLETED ... 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS` and vite's `✓ built in ...`. Then `git status --short` lists only the files in **Files** (no `pnpm-lock.yaml`, no `pnpm-workspace.yaml`).

- [ ] **Step 33: Commit.** From the repo root:

```sh
cd ../..
git add web/ui/src/lib/wasm.ts web/ui/src/core/tree.ts web/ui/src/fs/ext web/ui/tests/fixtures/extGeometry.ts web/ui/tests/fs/ext.test.ts web/ui/tests/fs/ext-format.test.ts web/ui/tests/attribution.test.ts web/ui/tests/lesson.test.ts web/ui/tests/adapterBoundary.test.ts web/ui/tests/integration.test.ts
git commit -m "feat(ui): the ext adapter behind the seam"
```

---

### Task 4: ext registered and visible (block-group map, Format form, Filesystem select, extras slot)

Spec: `docs/superpowers/specs/2026-09-24-ext-explorer-design.md` sections 1 (`FsFamilyId = "fat16" | "ext"`, `fsTypes`, `PANELS.extras` rendered by `App.svelte`), 2 (the `UnitOwner.color` amendment: `buildAttribution` uses `color` verbatim when a row carries it), 4 (the journal pointer-block paragraph: `<journal>` rows inside `journal` regions never become units; the pointer blocks, in a data region, stay in `owners` as `{ path: "<journal>", isDir: false, role: "indirect", color: COLOR_JOURNAL }`), 5 (the `BlockGroupMap.svelte`, `ExtFormatForm.svelte`, `ActionsPanel.svelte`, and `App.svelte` bullets), 8 (`tests/fs/registry.test.ts` both families and both ext types; `tests/attribution.test.ts`; `tests/layout.test.ts` covers the new panels' styles; the browser pass), 10 (journal blocks are never units; indirect blocks are owner rows).

The ext family becomes a registered family: `"ext"` joins `FsFamilyId`, `FAMILIES` and `PANELS` gain it, and Task 3's two casts go. The journal's five pointer blocks (1106..1110 on the default disk) stop looking free: they become `<journal>` owner rows with a fixed `COLOR_JOURNAL` colour (the new `UnitOwner.color`). Two Svelte panels arrive under `web/ui/src/fs/ext/`: the block-group map, whose drawing model (text, cell geometry, fills, marks) lives in a plain module `blockMap.ts` so the node tests can pin it, and the Format form, whose field-to-options mapping is `formOptions` in `format.ts`. The Actions panel's Format details gain a `Filesystem` select, and `App.svelte` renders each family's `extras` after its map. FAT16 stays `DEFAULT_FAMILY`; no FAT string, number, or colour in any expectation changes, and no existing FAT test line is edited (the registry's first test keeps `familyIdOf("EXT3")`, which still throws because `fsTypes` match exactly; the new ext test adds `ntfs`).

The `<journal>` owner is the first owner that names no file, so the seam learns the idea: `isPseudoOwner(path)` in `fs/adapter.ts` (a path in angle brackets), and `fs/ext` exports the string as `JOURNAL_OWNER`; no code outside the ext adapter spells `"<journal>"`. The map's click and the Inspector's Owner row both ask `isPseudoOwner`: on a pseudo-owner the click only jumps and the Owner row is plain text, not the select button. `describeUnit` names the pointer blocks `pointer block of the journal`, so the Inspector's collapsed address row reads `Block 1107 · data (group 0) · pointer block of the journal` like every other owned data block. The map draws on Task 2's shared grid (`core/grid.ts`, `core/observeWidth.ts`): the cell arithmetic, the dot, outline, chain, and dashed-hover marks, and the width tracking are the FAT map's own helpers. `fs/adapter.ts`'s header doc now names both families.

Choices made here that the spec's text did not settle:

- **Group count wording** (pinned): the heading says `1 group` (not `1 groups`) on a one-group disk.
- **Header wrapping** (pinned): a band header that does not fit the panel width breaks at its ` · ` separators onto more lines (the text is unchanged; the sidebar's 198 px content box is narrower than the one-line header in 11 px mono). The component measures the headers while it paints, not in a `$derived` (measuring sets the canvas font, a side effect), and stores the line count in state; every band's header row is as tall as the tallest needs.
- **Number format** (pinned): the heading's group and block counts and a band's free count are `toLocaleString()` numbers (`16,384 blocks`, `7,082 free` on an `en` locale); a band's block range is raw, written with an en dash (`blocks 1–8192`, `blocks 8193–16383`), as a range of addresses like the dump's.
- **Pseudo-owner clicks**: a click on a `<journal>` pointer block jumps to the block but selects nothing, because `<journal>` names no file in the tree.
- **Chain join**: the selected chain's centres are joined by a 1 px line (FAT's is 2 px over 6 px cells; a 2 px line would cover most of a 4 px cell).

**Files**

- Create: `web/ui/src/fs/ext/blockMap.ts` (1–128), `web/ui/src/fs/ext/BlockGroupMap.svelte` (1–140), `web/ui/src/fs/ext/ExtFormatForm.svelte` (1–60)
- Modify: `web/ui/src/fs/adapter.ts` (2–5 header doc, 42 `FsFamilyId`, 64–74 `UnitOwner`, 76–81 `isPseudoOwner`), `web/ui/src/core/attribution.ts` (29), `web/ui/src/fs/ext/adapter.ts` (1 `lib/wasm` import, 3 palette import, 12–14 `JOURNAL_OWNER`, old 11–13 `EXT_ID` removed, 28 and 112 the renamed constant, 30–48 `toUnitOwners`, 63–64 class comment, 68 `id`, 75–76 `owners` comment, 108–109 `refresh`, 219–230 `describeUnit`, 270 `ext.id`), `web/ui/src/fs/ext/index.ts` (6 `JOURNAL_OWNER` export, 9–11 `asExt`), `web/ui/src/components/Inspector.svelte` (5, 52–53 the Owner row), `web/ui/src/fs/ext/format.ts` (106–129 `ExtFormFields`, `formOptions`), `web/ui/src/fs/index.ts` (3, 6–8), `web/ui/src/fs/panels.ts` (5–6, 10–13, 18), `web/ui/src/components/ActionsPanel.svelte` (4–5, 8–15, 107–112), `web/ui/src/App.svelte` (22–24, 92), `web/ui/src/app.css` (140–144, 210)
- Test: `web/ui/tests/fs/blockMap.test.ts` (1–145, new), `web/ui/tests/attribution.test.ts` (96–97, 100–105, 114–126), `web/ui/tests/fs/ext.test.ts` (3, 7 imports; 206, 209, 223–234, 394), `web/ui/tests/fs/registry.test.ts` (4, 14–26, 62–73), `web/ui/tests/fs/ext-format.test.ts` (3, 128–153), `web/ui/tests/layout.test.ts` (43–61)
- Not touched: `crates/`, `web/ui/src/fs/fat16/**`, `web/ui/src/core/grid.ts` and `core/observeWidth.ts` (Task 2's; used as they are), `web/ui/tests/adapterBoundary.test.ts` (the new files are under `src/fs/ext/`, which may use the ext-only surface; `ActionsPanel.svelte` imports `fs/index.ts` and `fs/panels.ts`, and `Inspector.svelte` only `fs/adapter.ts`, not `fs/ext`).

**Interfaces**

Consumes: Task 2's `fs/adapter.ts` (`FsFamilyId`, `UnitOwner` with `role?`, `FsFamily`, `FsAdapter`), `fs/panels.ts` (`PANELS` with `extras`), `core/attribution.ts` (`buildAttribution`, `attrAtSector`, `AttributionTable`), `core/palette.ts` (`COLOR_JOURNAL = 11`, `COLOR_DIR`, `colorIndexForPath`, `ColorIndex`), `state/volume.svelte.ts` (`volume.adapter`, `volume.epoch`, `volume.attribution`, `volume.atLatest`, `volume.format(family, options)`), `state/layers.svelte.ts` (`layers.chain`, `layers.diff`), `state/selection.svelte.ts` (`selection.hoverOffset`, `select`, `jumpTo`), `core/grid.ts` (`prepareCanvas`, `gridCols`, `gridRows`, `cellRect`, `cellAt`, `outlineCell`, `dotCell`, `dashCell`, `drawChain`) and `core/observeWidth.ts` (`observeWidth`, a Svelte action: `use:observeWidth={(w) => (width = w)}`). Task 3's `fs/ext`: `ext`, `ExtAdapter` (`geo: ExtGeometry`, `journalBlocks: number[]`, `owners`), `asExt`, and from `format.ts` `SIZES`, `DEFAULTS`, `checkFormat`, `defaultInodesPerGroup`, `defaultJournalBlocks`, `toWasmOptions`, `ExtFamilyOptions`. Wasm types `ExtGeometry`, `ExtGroup` (type-only, under `src/fs/ext/`).

Produces (exact):

```ts
// fs/adapter.ts
export type FsFamilyId = "fat16" | "ext";
export interface UnitOwner { unit: number; path: string; isDir: boolean; firstUnit: number; role?: "data" | "directory" | "indirect"; color?: ColorIndex }
export function isPseudoOwner(path: string): boolean;   // path.startsWith("<"): an owner that names no file (ext's "<journal>")
// the header doc names both families (fs/fat16/ for FAT16, fs/ext/ for ext2 and ext3)
// core/attribution.ts: colorByUnit[o.unit] = o.color ?? (o.isDir ? COLOR_DIR : colorIndexForPath(o.path))
// fs/ext/adapter.ts
export const JOURNAL_OWNER = "<journal>";   // was the module-private JOURNAL_PATH
//   ExtAdapter.owners also holds { unit: b, path: JOURNAL_OWNER, isDir: false, role: "indirect", color: COLOR_JOURNAL, firstUnit: b }
//   for every journal row outside the layout's `journal` regions (1106..1110 on the default disk); journalBlocks unchanged (82..1110);
//   describeUnit(b) is "pointer block of the journal" for those rows and still null for the journal's data blocks (82..1105);
//   ExtAdapter.id is the literal "ext"; ext.id is "ext"
// fs/ext/index.ts: exports JOURNAL_OWNER; asExt(fs) compares fs.id !== "ext" (no cast)
// components/Inspector.svelte: the Owner row is plain text (no select button) for a pseudo-owner
// fs/index.ts
export const FAMILIES: Record<FsFamilyId, FsFamily>;   // { fat16, ext }, in that order
// fs/panels.ts
export const PANELS: Record<FsFamilyId, { map: Component; format: Component; extras: Component[] }>;   // ext: { map: BlockGroupMap, format: ExtFormatForm, extras: [] }

// fs/ext/format.ts (added)
export interface ExtFormFields { variant: "ext2" | "ext3"; totalBlocks: number; inodesPerGroup: number | null | undefined; label: string; journalMode: "ordered" | "data"; journalBlocks: number | null | undefined }
export function formOptions(f: ExtFormFields): ExtFamilyOptions;   // blank/NaN numbers -> undefined; journalMode/journalBlocks only on ext3

// fs/ext/blockMap.ts (new; imported by BlockGroupMap.svelte and the tests only)
export const CELL = 4, GAP = 1, PITCH = 5, HEADER = 16, BAND_GAP = 8, MAX_HEIGHT = 260;
export const FILL_FREE = 255;
export interface Band { group: number; first: number; count: number; top: number; cellsTop: number; rows: number }
export interface MapLayout { cols: number; bands: Band[]; height: number }
export function mapHeading(geo: ExtGeometry): string;        // `Block groups · 2 groups · 16,384 blocks` ("1 group" when one)
export function bandHeader(g: ExtGroup): string;             // `group 0 · blocks 1–8192 · 7,082 free`
export function wrapHeader(text: string, width: number, measure: (s: string) => number): string[];
export function layoutBands(geo: ExtGeometry, width: number, headerLines?: number): MapLayout;   // cols = gridCols(width, CELL, GAP), rows per band gridRows
export function cellOf(m: MapLayout, b: number): { x: number; y: number } | null;                   // cellRect within the block's band
export function blockAt(m: MapLayout, x: number, y: number): number | null;                         // cellAt within the band under y
export function blockFills(t: AttributionTable, geo: ExtGeometry, journalBlocks: readonly number[]): Uint8Array;
export function indirectBlocks(t: AttributionTable): number[];
export function blocksTouched(ranges: readonly Interval[], blockSize: number, totalBlocks: number): number[];
export function blockCaption(t: AttributionTable, b: number): string;   // `block 1107 · data (group 0) · <journal>`, `block 1126 · data (group 0) · free`
export function clickPath(t: AttributionTable, b: number): string | null;   // owner path; null for free, metadata, and a pseudo-owner (isPseudoOwner)

// Svelte (not importable by node tests)
// fs/ext/BlockGroupMap.svelte: <section class="panel bgmap">, <h2>{mapHeading}</h2>, the FAT map's stale note, <div class="bgmap-wrap"> (max-height 260px,
//   use:observeWidth), <canvas aria-label="Block group map"> sized by core/grid's prepareCanvas, <p class="mono muted bgmap-caption">; marks through core/grid (dot, outline,
//   drawChain with a 1 px join, dashCell); the header line count is state that paint() measures
// fs/ext/ExtFormatForm.svelte: Variant (ext2|ext3), Size (SIZES), Inodes per group, Label (maxlength 16), [ext3] Journal mode (ordered|data), Journal blocks;
//   <p class="format-check warn">{problem}</p> when checkFormat finds one; "Format disk" -> volume.format("ext", options); selection.select(null)
// components/ActionsPanel.svelte: <select id="format-family"> under a "Filesystem" label, options value = family id ("fat16", "ext"), label = family name ("FAT16", "ext")
// App.svelte: {#each PANELS[volume.adapter.id].extras as Extra}<Extra />{/each} right after <MapPanel />
// app.css: .bgmap-wrap, .bgmap-wrap canvas, .bgmap-caption, .actions .format-check
```

Environment for every command below, from `web/ui`: any pnpm 10.26 or newer (nvm's pnpm 12 and Homebrew's 10.33 both work). Both web packages keep their pnpm settings in a tracked `pnpm-workspace.yaml`, so a frozen install must leave `web/ui/pnpm-lock.yaml` and `web/ui/pnpm-workspace.yaml` unmodified; if either shows as modified in `git status` at any point, restore it with `git checkout web/ui/pnpm-lock.yaml web/ui/pnpm-workspace.yaml` and run `CI=true pnpm install --frozen-lockfile` again (`CI=true` lets pnpm purge a stale modules directory without asking).

#### Round 1: the journal's pointer blocks are owned, in the journal colour

- [ ] **Step 1: Build the wasm package and install it.** From the repo root:

```sh
wasm-pack build crates/wasm --target bundler
cd web/ui && CI=true pnpm install --frozen-lockfile && pnpm test
```

Expected: wasm-pack ends `Your wasm pkg is ready to publish at crates/wasm/pkg.`; `pnpm test` prints `Test Files  44 passed (44)` and `Tests  394 passed (394)` (Tasks 1 to 3 applied in order, Task 2's `tests/grid.test.ts`, `tests/freeSpace.test.ts`, and `tests/scrollNonce.test.ts` included).

- [ ] **Step 2: Write the failing attribution tests.** In `web/ui/tests/attribution.test.ts`, inside the test `on ext: journal regions are never units, and metadata blocks are coloured by kind`, replace the two comment lines

```ts
    // The journal's pointer blocks 1106..1110 lie in the data region after it; the adapter keeps
    // them out of `owners` (they are in `journalBlocks`), so attribution calls them free.
```

with

```ts
    // The journal's pointer blocks 1106..1110 lie in the data region after it. A table without
    // their owner rows would call them free; the adapter keeps them in `owners` (next test).
```

(the `expect(attrAtSector(te, 1107))...free: true...` line under them stays), and add right after that test's closing `});`:

```ts
  it("on ext: the journal's pointer blocks are owned by <journal> in the journal colour", () => {
    const pointers: UnitOwner[] = [1106, 1107, 1108, 1109, 1110].map((unit) => ({ unit, path: "<journal>", isDir: false, firstUnit: unit, role: "indirect", color: COLOR_JOURNAL }));
    const te = buildAttribution(extSpace, extLayout, [...extOwners.slice(0, 1), ...pointers, ...extOwners.slice(1)]);
    expect(attrAtSector(te, 1107)).toEqual({ regionKind: "data", regionName: "data (group 0)", sector: 1107, unit: 1107, ownerPath: "<journal>", isDir: false, role: "indirect", free: false, colorIndex: COLOR_JOURNAL });
    expect(attrAtSector(te, 1113)).toMatchObject({ ownerPath: "/bigger.txt", colorIndex: colorIndexForPath("/bigger.txt") }); // rows without `color` are unchanged
  });
```

Then add right before `it("file hues are stable and in range", () => {`:

```ts
  it("uses an owner row's fixed colour verbatim, over the directory colour and the path's hue", () => {
    const fixed: UnitOwner[] = [
      { unit: 2, path: "/DOCS", isDir: true, firstUnit: 2, color: COLOR_BOOT },
      { unit: 3, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3, color: COLOR_JOURNAL },
      { unit: 4, path: "/DOCS/N.TXT", isDir: false, firstUnit: 3 },
    ];
    const tf = buildAttribution(space, layout, fixed);
    expect(tf.colorByUnit[2]).toBe(COLOR_BOOT);
    expect(tf.colorByUnit[3]).toBe(COLOR_JOURNAL);
    expect(tf.colorByUnit[4]).toBe(colorIndexForPath("/DOCS/N.TXT"));
    expect(attrAtSector(tf, 101)).toMatchObject({ unit: 3, ownerPath: "/DOCS/N.TXT", colorIndex: COLOR_JOURNAL });
    expect(attrAtSector(tf, 97)).toMatchObject({ unit: 2, isDir: true, colorIndex: COLOR_BOOT });
  });
```

(`COLOR_BOOT`, `COLOR_JOURNAL`, `colorIndexForPath`, and the `UnitOwner` type are already imported.)

- [ ] **Step 3: Write the failing adapter test.** In `web/ui/tests/fs/ext.test.ts` replace the imports `import { unitIsSector } from "../../src/fs/adapter";` and `import { ExtAdapter, MKFS, asExt, ext } from "../../src/fs/ext";` with:

```ts
import { isPseudoOwner, unitIsSector } from "../../src/fs/adapter";
```

```ts
import { ExtAdapter, JOURNAL_OWNER, MKFS, asExt, ext } from "../../src/fs/ext";
```

In `describe("ExtAdapter: owners, chains, and slots", ...)`, change the first test's title and filter:

```ts
  it("lists every path's blocks with their roles and first block, and none of the journal's data blocks", () => {
    const { fs } = fixture();
    const bigger = (unit: number, role: "data" | "indirect") => ({ unit, path: "/bigger.txt", isDir: false, firstUnit: 1113, role });
    expect(fs.owners.filter((o) => o.path !== "/lost+found" && o.path !== "<journal>")).toEqual([
```

delete its line

```ts
    expect(fs.owners.some((o) => o.path === "<journal>")).toBe(false);
```

and add a test right before `it("an ext2 volume has no journal blocks", () => {`:

```ts
  it("keeps the journal's pointer blocks as <journal> rows in the journal colour, and never its data blocks", () => {
    const { fs } = fixture();
    // The pointer blocks 1106..1110 lie in the data region after the journal region (82..1105):
    // units, so they are owned rather than free. The journal's data blocks are not units at all.
    expect(fs.owners.filter((o) => o.path === "<journal>")).toEqual(range(1106, 1110).map((unit) => ({ unit, path: "<journal>", isDir: false, role: "indirect", color: COLOR_JOURNAL, firstUnit: unit })));
    const owned = new Set(fs.owners.map((o) => o.unit));
    expect(range(82, 1105).filter((b) => owned.has(b))).toEqual([]);
    expect(fs.owners.filter((o) => o.color !== undefined).map((o) => o.path)).toEqual(Array(5).fill("<journal>")); // no other row carries a colour
    expect(JOURNAL_OWNER).toBe("<journal>");
    expect(fs.owners.filter((o) => isPseudoOwner(o.path)).map((o) => o.unit)).toEqual(range(1106, 1110)); // no volume path starts with "<"
    for (const b of range(1106, 1110)) expect(fs.describeUnit(b), `block ${b}`).toBe("pointer block of the journal");
  });

```

(`COLOR_JOURNAL` and `range` already exist in this file.) The pointer blocks are now described, so in `it("describes what a block holds for its owner, free, or nothing outside the data area", ...)` replace the loop

```ts
    for (const b of [0, 1, 5, 68, 90, 1106, 1110, 8193, 8260, 16384, -1]) expect(fs.describeUnit(b), `block ${b}`).toBeNull();
```

with (1105 is the journal's last data block, still outside the units)

```ts
    for (const b of [0, 1, 5, 68, 90, 1105, 8193, 8260, 16384, -1]) expect(fs.describeUnit(b), `block ${b}`).toBeNull();
```

- [ ] **Step 4: Run them and watch them fail.**

```sh
pnpm exec vitest run tests/attribution.test.ts tests/fs/ext.test.ts
```

Expected: `Test Files  2 failed (2)`, `Tests  3 failed | 48 passed (51)`; the three new tests fail (the owner colour is the path's hue, and `owners` has no `<journal>` rows).

- [ ] **Step 5: Add the colour override to `UnitOwner`.** The contract comment at the top of `web/ui/src/fs/adapter.ts` already describes it (Task 2 wrote the journal bullet: a journal block outside the `journal` regions "is an owner row with a fixed `color`"). In that file replace the one-line `export interface UnitOwner { ... }` (its doc comment stays) with:

```ts
export interface UnitOwner {
  unit: number;
  path: string;
  isDir: boolean;
  firstUnit: number;
  role?: "data" | "directory" | "indirect";
  /** A fixed colour override for special owners such as the journal's pointer blocks (ext:
   *  `COLOR_JOURNAL`); attribution uses it verbatim instead of the directory colour or the
   *  path's hue. FAT rows leave it undefined. */
  color?: ColorIndex;
}
```

(`ColorIndex` is already imported type-only from `../core/palette`.) Add right after it the test every surface uses for an owner that names no file:

```ts

/** An owner that names no file in the tree, such as ext's `<journal>` (its pointer blocks): the
 *  path is in angle brackets, which no volume path starts with. The map and the Inspector name
 *  it but never select it. */
export function isPseudoOwner(path: string): boolean {
  return path.startsWith("<");
}
```

and, because this task registers the second family, replace the header doc's first sentence

```ts
 * The filesystem adapter seam. Every filesystem-specific fact the explorer, the shell and
 * the lessons need lives behind these interfaces, with one implementation per
 * `Volume.fsType()` family (`fs/fat16/` today); `fs/index.ts` picks the implementation.
```

with

```ts
 * The filesystem adapter seam. Every filesystem-specific fact the explorer, the shell and
 * the lessons need lives behind these interfaces, with one implementation per family:
 * `fs/fat16/` for `FAT16` and `fs/ext/` for `ext2` and `ext3` (a family lists the
 * `Volume.fsType()` strings it binds in `fsTypes`); `fs/index.ts` picks the implementation.
```

- [ ] **Step 6: Use it in attribution.** In `web/ui/src/core/attribution.ts`, inside `buildAttribution`, replace

```ts
      colorByUnit[o.unit] = o.isDir ? COLOR_DIR : colorIndexForPath(o.path);
```

with

```ts
      colorByUnit[o.unit] = o.color ?? (o.isDir ? COLOR_DIR : colorIndexForPath(o.path));
```

- [ ] **Step 7: Keep the pointer blocks in the ext adapter's owners, and show them as the journal's.** In `web/ui/src/fs/ext/adapter.ts`, replace the `../../lib/wasm` import (it gains `Region`) with:

```ts
import { Volume, type Annotation, type ExtBlockOwner, type ExtFileBlocks, type ExtGeometry, type ExtIndirectBlock, type ExtInode, type ExtSuperblock, type JournalInfo, type Region } from "../../lib/wasm";
```

add the palette import after the `../../core/intervals` import:

```ts
import { COLOR_JOURNAL } from "../../core/palette";
```

The journal's path becomes the exported pseudo-owner name. Replace

```ts
/** The path `blockOwners()` gives the journal's blocks (inode 8's data and pointer blocks). */
const JOURNAL_PATH = "<journal>";
```

with

```ts
/** The path `blockOwners()` gives the journal's blocks (inode 8's data and pointer blocks), and
 *  the owner of the pointer-block rows in `owners`: a pseudo-owner (`isPseudoOwner`). */
export const JOURNAL_OWNER = "<journal>";
```

and rename its two other uses in the file: `isPathRow` becomes `r.path !== JOURNAL_OWNER && r.role !== "journal"`, and in `refresh()` the journal rows are `rows.filter((r) => r.path === JOURNAL_OWNER)`. In `web/ui/src/fs/ext/index.ts` replace `export { ext, ExtAdapter } from "./adapter";` with:

```ts
export { ext, ExtAdapter, JOURNAL_OWNER } from "./adapter";
```

Replace `toUnitOwners` and its doc comment with:

```ts
/** The rows in the generic shape. `firstUnit` is the path's first data (or directory) block in
 *  logical order, `firstBlockOf(path)`: not always its lowest, because a file that grows after a
 *  lower block was freed maps that block later. Where the path cannot be walked (a corrupt
 *  volume) it falls back to the path's lowest data or directory block. The journal's rows inside
 *  `journal` regions are dropped (those blocks are never units); the rest of them (its pointer
 *  blocks, which lie in a data region) stay as `<journal>` indirect rows in `COLOR_JOURNAL`, so
 *  they read as the journal's, not free. */
function toUnitOwners(rows: ExtBlockOwner[], firstBlockOf: (path: string) => number | undefined, journalRegions: readonly Region[]): UnitOwner[] {
  const inJournalRegion = (b: number) => journalRegions.some((r) => b >= r.sectors.start && b < r.sectors.end);
  const first = new Map<string, number>();
  for (const r of rows) if (isPathRow(r) && r.role !== "indirect" && !first.has(r.path)) first.set(r.path, r.block);
  for (const [path, lowest] of first) first.set(path, firstBlockOf(path) ?? lowest);
  const out: UnitOwner[] = [];
  for (const r of rows) {
    if (isPathRow(r)) out.push({ unit: r.block, path: r.path, isDir: r.role === "directory", firstUnit: first.get(r.path) ?? r.block, role: r.role });
    else if (r.path === JOURNAL_OWNER && !inJournalRegion(r.block)) out.push({ unit: r.block, path: JOURNAL_OWNER, isDir: false, role: "indirect", color: COLOR_JOURNAL, firstUnit: r.block });
  }
  return out;
}
```

In the class's doc comment replace the line

```ts
 * Journal blocks never become units: their owner rows go to `journalBlocks`, not `owners`.
```

(the `SpaceAdapter` line after it stays) with

```ts
 * Every journal block is in `journalBlocks`; the ones inside `journal` regions never become
 * units, and the pointer blocks (in a data region) stay in `owners` as `<journal>` rows.
```

replace the `owners` field's comment

```ts
  /** The directory, data, and indirect blocks of every path, in block order; no journal rows. */
```

with

```ts
  /** The directory, data, and indirect blocks of every path, in block order, and the journal's
   *  pointer blocks as `<journal>` rows in `COLOR_JOURNAL`; never the journal's data blocks. */
```

and in `refresh()` replace `this.owners = toUnitOwners(rows, (path) => this.blocksOf(path)?.data[0]);` with:

```ts
    // On ext a region's sectors are blocks (the disk's sector is the block; see extSpace).
    this.owners = toUnitOwners(rows, (path) => this.blocksOf(path)?.data[0], this.vol.layout().filter((r) => r.kind === "journal"));
```

Finally, name the pointer blocks in `describeUnit` (Step 3 pins the string), so the Inspector's collapsed address row reads `Block 1107 · data (group 0) · pointer block of the journal` like any other owned block. Replace its doc comment and its first lines, through `if (!owner) return "free";`, with:

```ts
  /** What block `b` holds for its owner (`pointer block of the journal` for the journal's own
   *  pointer blocks), or `free`; null outside the data area (block 0, a group's metadata, the
   *  journal's data blocks, past the end). */
  describeUnit(b: number): string | null {
    const g = this.geo;
    if (!Number.isInteger(b) || b < g.firstDataBlock || b >= g.totalBlocks) return null;
    const group = g.groups.find((x) => b >= x.firstBlock && b < x.firstBlock + x.blockCount);
    if (!group || b < group.firstData) return null;
    const owner = this.byUnit.get(b);
    if (owner?.path === JOURNAL_OWNER) return "pointer block of the journal";
    if (this.journalSet.has(b)) return null;
    if (!owner) return "free";
```

(the rest of the method, from `const { path } = owner;`, is unchanged). `indirectBlocks` (derived from `owners` by role) now also lists 1106..1110; `touchesMetadata` already counted them through `journalBlocks`, so its answers do not change. The journal's data blocks 82..1105 are still `null` to `describeUnit`: they are not units, so the Inspector never asks.

The Inspector's Owner row is a button that selects the owner's path; `<journal>` is not a path in the tree, so on a pseudo-owner the row is plain text. In `web/ui/src/components/Inspector.svelte` replace `import { unitIsSector } from "../fs/adapter";` with:

```ts
  import { isPseudoOwner, unitIsSector } from "../fs/adapter";
```

and replace the Owner row (the line that starts `{#if attr.ownerPath}<dt>Owner</dt>`) with:

```svelte
      <!-- A pseudo-owner (ext's journal pointer blocks) names no file in the tree: plain text, not a button. -->
      {#if attr.ownerPath && isPseudoOwner(attr.ownerPath)}<dt>Owner</dt><dd>{attr.ownerPath}</dd>{:else if attr.ownerPath}<dt>Owner</dt><dd><button class="link" onclick={() => selection.select(attr.ownerPath!)}>{attr.ownerPath}</button></dd>{:else if attr.regionKind === "data"}<dt>Owner</dt><dd class="muted">free</dd>{/if}
```

On FAT no owner path starts with `<`, so the row is the button it was.

- [ ] **Step 8: Run them and watch them pass.**

```sh
pnpm exec vitest run tests/attribution.test.ts tests/fs/ext.test.ts
```

Expected: `Test Files  2 passed (2)`, `Tests  51 passed (51)`.

#### Round 2: the registry knows ext

- [ ] **Step 9: Write the failing registry tests.** In `web/ui/tests/fs/registry.test.ts`, add after `import { fat16 } from "../../src/fs/fat16";`:

```ts
import { ext } from "../../src/fs/ext";
```

The first test stays as it is: its `familyIdOf("EXT3")` still throws, because a family's `fsTypes` match the type string exactly and ext's are lower-case. Add these two tests right after its closing `});`:

```ts

  it("maps both ext types to the one ext family, matching the type string exactly", () => {
    expect(familyIdOf("ext2")).toBe("ext");
    expect(familyIdOf("ext3")).toBe("ext");
    expect(() => familyIdOf("EXT3")).toThrow("no adapter for EXT3"); // fsType() strings are lower-case
    expect(() => familyIdOf("ext4")).toThrow("no adapter for ext4");
    expect(() => familyIdOf("ntfs")).toThrow("no adapter for ntfs");
  });

  it("registers fat16 then ext, by id", () => {
    expect(Object.keys(FAMILIES)).toEqual(["fat16", "ext"]);
    expect(FAMILIES.ext).toBe(ext);
    expect(Object.values(FAMILIES).map((f) => [f.id, f.name])).toEqual([["fat16", "FAT16"], ["ext", "ext"]]);
  });
```

Then add after the test `adapterFor binds the family whose name is the volume's fsType` (inside the same `describe`):

```ts

  it("adapterFor binds the ext family to an ext3 volume and to an ext2 one", () => {
    const vol = FAMILIES.ext.format();
    const fs = adapterFor(vol);
    expect(fs.id).toBe("ext");
    expect(fs.name).toBe("ext3");
    expect(fs.family).toBe(ext);
    expect(fs.vol).toBe(vol);
    expect(fs.journal).toBeDefined();
    const two = adapterFor(FAMILIES.ext.format({ variant: "ext2" }));
    expect([two.id, two.name, two.journal]).toEqual(["ext", "ext2", undefined]);
  });
```

Every FAT assertion in the file stays as it is.

- [ ] **Step 10: Run it and watch it fail.**

```sh
pnpm exec vitest run tests/fs/registry.test.ts
```

Expected: `Tests  3 failed | 6 passed (9)`: `Error: no adapter for ext2`, `expected [ 'fat16' ] to deeply equal [ 'fat16', 'ext' ]`, and `Cannot read properties of undefined (reading 'format')`.

- [ ] **Step 11: Register the family and drop the casts.** In `web/ui/src/fs/adapter.ts` replace `export type FsFamilyId = "fat16";                       // ext adds "ext"` with:

```ts
export type FsFamilyId = "fat16" | "ext";
```

In `web/ui/src/fs/index.ts` replace

```ts
import { fat16 } from "./fat16";

/** Every registered family, by id. A new family is added here and in `fs/panels.ts`. */
export const FAMILIES: Record<FsFamilyId, FsFamily> = { fat16 };
```

with

```ts
import { ext } from "./ext";
import { fat16 } from "./fat16";

/** Every registered family, by id, in the order the Format details' Filesystem select lists
 *  them. A new family is added here and in `fs/panels.ts`. */
export const FAMILIES: Record<FsFamilyId, FsFamily> = { fat16, ext };
```

In `web/ui/src/fs/ext/adapter.ts` delete

```ts
/** The family id. The cast goes when Task 4 adds "ext" to `FsFamilyId` and registers the family. */
const EXT_ID = "ext" as string as FsFamilyId;

```

replace `readonly id: FsFamilyId = EXT_ID;` with `readonly id: FsFamilyId = "ext";`, and in `export const ext` replace `id: EXT_ID,` with `id: "ext",`. In `web/ui/src/fs/ext/index.ts` replace `asExt` and its comment with:

```ts
/** The ext adapter behind a generic one, for the few callers that need an ext-only fact (the
 *  block-group map's `geo` and `journalBlocks`, the journal panel, the lessons' `inodeOffset`). */
export function asExt(fs: FsAdapter): ExtAdapter {
  if (fs.id !== "ext") throw new Error(`not an ext adapter: ${fs.id}`);
  return fs as ExtAdapter;
}
```

- [ ] **Step 12: Run it and watch it pass.**

```sh
pnpm exec vitest run tests/fs/registry.test.ts
```

Expected: `Tests  9 passed (9)`. `pnpm test` passes too (`Tests  400 passed (400)`). Do not run `pnpm build` yet: `PANELS` is a `Record<FsFamilyId, ...>` and gains its `ext` entry in Step 27, so svelte-check reports it missing until then.

#### Round 3: the block-group map

- [ ] **Step 13: Write the failing map-model tests.** Create `web/ui/tests/fs/blockMap.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { buildAttribution } from "../../src/core/attribution";
import { COLOR_BOOT, COLOR_DIR, COLOR_JOURNAL, COLOR_TABLE, colorIndexForPath } from "../../src/core/palette";
import { asExt, ext } from "../../src/fs/ext";
import {
  BAND_GAP, CELL, FILL_FREE, HEADER, MAX_HEIGHT, PITCH,
  bandHeader, blockAt, blockCaption, blockFills, blocksTouched, cellOf, clickPath, indirectBlocks, layoutBands, mapHeading, wrapHeader,
} from "../../src/fs/ext/blockMap";

const BLOCK = 1024;
const n = (x: number) => x.toLocaleString();

/** The default ext3 disk with /hello.txt (block 1111) and /bigger.txt (13 data blocks
 *  1112..1124 and its single-indirect block 1125), and the attribution table the store builds. */
function fixture() {
  const vol = ext.format();
  vol.createFile("/hello.txt", new TextEncoder().encode("Hello, ext3!"));
  vol.createFile("/bigger.txt", new Uint8Array(13 * BLOCK));
  const fs = asExt(ext.bind(vol));
  return { vol, fs, t: buildAttribution(fs, vol.layout(), fs.owners) };
}

describe("the block-group map's text", () => {
  it("heads the panel with the group and block counts", () => {
    const { fs } = fixture();
    expect(mapHeading(fs.geo)).toBe(`Block groups · 2 groups · ${n(16384)} blocks`);
    expect(mapHeading(asExt(ext.bind(ext.format({ totalBlocks: 4096 }))).geo)).toBe(`Block groups · 1 group · ${n(4096)} blocks`);
  });

  it("heads each band with its group, its block range, and its cached free count", () => {
    const fresh = asExt(ext.bind(ext.format()));
    expect(fresh.geo.groups.map(bandHeader)).toEqual([`group 0 · blocks 1–8192 · ${n(7082)} free`, `group 1 · blocks 8193–16383 · ${n(8123)} free`]);
    const { fs } = fixture();
    expect(bandHeader(fs.geo.groups[0])).toBe(`group 0 · blocks 1–8192 · ${n(7082 - 15)} free`); // 1 + 13 data blocks and 1 pointer block
  });

  it("wraps a band header at its separators only when it does not fit the width", () => {
    const header = `group 0 · blocks 1–8192 · 7,082 free`;
    const measure = (s: string) => s.length * 6; // a 6 px monospace advance
    expect(wrapHeader(header, 400, measure)).toEqual([header]);
    expect(wrapHeader(header, 150, measure)).toEqual(["group 0 · blocks 1–8192", "7,082 free"]);
    expect(wrapHeader(header, 50, measure)).toEqual(["group 0", "blocks 1–8192", "7,082 free"]); // a part wider than the width still gets a line
  });

  it("captions a block with its region, then its owner or free", () => {
    const { t } = fixture();
    expect(blockCaption(t, 0)).toBe("block 0 · boot block");
    expect(blockCaption(t, 3)).toBe("block 3 · block bitmap (group 0)");
    expect(blockCaption(t, 90)).toBe("block 90 · journal");
    expect(blockCaption(t, 69)).toBe("block 69 · data (group 0) · /");
    expect(blockCaption(t, 1107)).toBe("block 1107 · data (group 0) · <journal>");
    expect(blockCaption(t, 1111)).toBe("block 1111 · data (group 0) · /hello.txt");
    expect(blockCaption(t, 1125)).toBe("block 1125 · data (group 0) · /bigger.txt");
    expect(blockCaption(t, 1126)).toBe("block 1126 · data (group 0) · free");
    expect(blockCaption(t, 8195)).toBe("block 8195 · block bitmap (group 1)");
  });

  it("selects a clicked block's owner, but never the journal, which is not a path", () => {
    const { t } = fixture();
    expect(clickPath(t, 1111)).toBe("/hello.txt");
    expect(clickPath(t, 1125)).toBe("/bigger.txt");
    expect(clickPath(t, 69)).toBe("/");
    expect(clickPath(t, 1107)).toBeNull();
    expect(clickPath(t, 1126)).toBeNull();
    expect(clickPath(t, 3)).toBeNull();
    expect(clickPath(t, 90)).toBeNull();
  });
});

describe("the block-group map's geometry", () => {
  it("uses 4 px cells with a 1 px gap and the FAT map's 260 px scroll box", () => {
    expect([CELL, PITCH, MAX_HEIGHT]).toEqual([4, 5, 260]);
  });

  it("stacks one band per group: a header row, then the group's blocks wrapped to the width", () => {
    const { fs } = fixture();
    const m = layoutBands(fs.geo, 300);
    expect(m.cols).toBe(60);
    const rows = Math.ceil(8192 / 60); // 137, and the same for group 1's 8191 blocks
    expect(m.bands).toEqual([
      { group: 0, first: 1, count: 8192, top: 0, cellsTop: HEADER, rows },
      { group: 1, first: 8193, count: 8191, top: HEADER + rows * PITCH + BAND_GAP, cellsTop: 2 * HEADER + rows * PITCH + BAND_GAP, rows },
    ]);
    expect(m.height).toBe(2 * (HEADER + rows * PITCH) + BAND_GAP);
    expect(layoutBands(fs.geo, 0).cols).toBe(1); // before the first measurement
    // Headers wrapped onto two lines make every band's header row twice as tall.
    const two = layoutBands(fs.geo, 300, 2);
    expect(two.bands.map((b) => [b.top, b.cellsTop])).toEqual([[0, 2 * HEADER], [2 * HEADER + rows * PITCH + BAND_GAP, 4 * HEADER + rows * PITCH + BAND_GAP]]);
    expect(two.height).toBe(2 * (2 * HEADER + rows * PITCH) + BAND_GAP);
  });

  it("places a block's cell and finds the block under a point, and nothing between bands", () => {
    const { fs } = fixture();
    const m = layoutBands(fs.geo, 300);
    const band1 = m.bands[1].cellsTop;
    expect(cellOf(m, 1)).toEqual({ x: 0, y: HEADER });
    expect(cellOf(m, 2)).toEqual({ x: PITCH, y: HEADER });
    expect(cellOf(m, 61)).toEqual({ x: 0, y: HEADER + PITCH });
    expect(cellOf(m, 8193)).toEqual({ x: 0, y: band1 });
    expect(cellOf(m, 0)).toBeNull(); // the boot block is outside every group
    expect(cellOf(m, 16384)).toBeNull();
    for (const b of [1, 2, 60, 61, 1111, 8192, 8193, 16383]) {
      const c = cellOf(m, b)!;
      expect(blockAt(m, c.x, c.y), `block ${b}`).toBe(b);
      expect(blockAt(m, c.x + CELL, c.y + CELL), `block ${b}'s gap`).toBe(b); // the gap after a cell is the cell's
    }
    expect(blockAt(m, 0, HEADER - 1)).toBeNull();                 // group 0's header row
    expect(blockAt(m, 0, band1 - 1)).toBeNull();                  // group 1's header row
    const last0 = cellOf(m, 8192)!;
    expect(blockAt(m, last0.x + PITCH, last0.y)).toBeNull();      // past group 0's last block
    expect(blockAt(m, 300, HEADER)).toBeNull();                   // past the last column
    expect(blockAt(m, -1, HEADER)).toBeNull();
    expect(blockAt(m, 0, m.height)).toBeNull();
  });
});

describe("the block-group map's colours and marks", () => {
  it("colours metadata by region kind, every journal block amber, owned blocks by owner, free blocks as the hairline", () => {
    const { fs, t } = fixture();
    const fills = blockFills(t, fs.geo, fs.journalBlocks);
    expect(fills.length).toBe(16384);
    expect([fills[0], fills[1], fills[3], fills[5], fills[68]]).toEqual([COLOR_BOOT, COLOR_BOOT, COLOR_TABLE, COLOR_BOOT, COLOR_BOOT]);
    expect([fills[69], fills[70]]).toEqual([COLOR_DIR, COLOR_DIR]);        // the root and lost+found
    expect([fills[82], fills[1105], fills[1106], fills[1110]]).toEqual([COLOR_JOURNAL, COLOR_JOURNAL, COLOR_JOURNAL, COLOR_JOURNAL]);
    expect(fills[1111]).toBe(colorIndexForPath("/hello.txt"));
    expect([fills[1112], fills[1125]]).toEqual([colorIndexForPath("/bigger.txt"), colorIndexForPath("/bigger.txt")]);
    expect([fills[1126], fills[16383]]).toEqual([FILL_FREE, FILL_FREE]);
    expect([fills[8193], fills[8195]]).toEqual([COLOR_BOOT, COLOR_TABLE]);
    // Whatever attribution says, a block the adapter lists as the journal's is amber.
    expect(blockFills(t, fs.geo, [1126])[1126]).toBe(COLOR_JOURNAL);
  });

  it("marks every indirect row: the journal's pointer blocks and a file's", () => {
    const { t } = fixture();
    expect(indirectBlocks(t)).toEqual([1106, 1107, 1108, 1109, 1110, 1125]);
  });

  it("lists the blocks a set of byte ranges touches, once each, within the disk", () => {
    expect(blocksTouched([{ start: 5 * BLOCK + 10, end: 7 * BLOCK }], BLOCK, 16384)).toEqual([5, 6]);
    expect(blocksTouched([{ start: 0, end: 1 }, { start: 100, end: 2 * BLOCK + 1 }], BLOCK, 16384)).toEqual([0, 1, 2]);
    expect(blocksTouched([{ start: 16383 * BLOCK, end: 16385 * BLOCK }], BLOCK, 16384)).toEqual([16383]);
    expect(blocksTouched([{ start: 8, end: 8 }], BLOCK, 16384)).toEqual([]);
    expect(blocksTouched([], BLOCK, 16384)).toEqual([]);
  });
});
```

The numbers are written through `toLocaleString()` because the component formats them that way; on an `en` locale they read `16,384`, `7,082`, `7,067`.

- [ ] **Step 14: Run it and watch it fail.**

```sh
pnpm exec vitest run tests/fs/blockMap.test.ts
```

Expected: `Test Files  1 failed (1)`, `Tests  no tests`, with `Error: Cannot find module '../../src/fs/ext/blockMap'`.

- [ ] **Step 15: Write the map model.** Create `web/ui/src/fs/ext/blockMap.ts`:

```ts
import type { ExtGeometry, ExtGroup } from "../../lib/wasm";
import { attrAtSector, type AttributionTable } from "../../core/attribution";
import { cellAt, cellRect, gridCols, gridRows } from "../../core/grid";
import type { Interval } from "../../core/intervals";
import { COLOR_JOURNAL } from "../../core/palette";
import { isPseudoOwner } from "../adapter";

/**
 * The block-group map's model, kept out of the component so the node tests can check it: the
 * text it draws, where each block's cell sits, and what colour and marks each cell gets. On ext
 * the disk's sector is the block (see extSpace), so a block number is also the attribution
 * table's sector number.
 */

/** A cell's side, the pitch from one cell to the next (a 1 px gap), one line of a band's header,
 *  the space between bands, and the scroll box's maximum height (the FAT map's). */
export const CELL = 4, GAP = 1, PITCH = CELL + GAP, HEADER = 16, BAND_GAP = 8, MAX_HEIGHT = 260;

/** The fill of a free block: the hairline colour rather than a palette index. */
export const FILL_FREE = 255;

/** One block group's band: its header row at `top`, then `rows` rows of cells from `cellsTop`. */
export interface Band { group: number; first: number; count: number; top: number; cellsTop: number; rows: number }
export interface MapLayout { cols: number; bands: Band[]; height: number }

/** `Block groups · 2 groups · 16,384 blocks` ("1 group" on a one-group disk). */
export function mapHeading(geo: ExtGeometry): string {
  const groups = geo.groups.length;
  return `Block groups · ${groups.toLocaleString()} ${groups === 1 ? "group" : "groups"} · ${geo.totalBlocks.toLocaleString()} blocks`;
}

/** A band's header: `group 0 · blocks 1–8192 · 7,082 free`, from the adapter's cached geometry. */
export function bandHeader(g: ExtGroup): string {
  return `group ${g.index} · blocks ${g.firstBlock}–${g.firstBlock + g.blockCount - 1} · ${g.freeBlocks.toLocaleString()} free`;
}

/** A band header broken at its ` · ` separators into as few lines as fit `width` (a part wider
 *  than the width still gets a line of its own); `measure` is the canvas's text width. The
 *  sidebar is narrower than a one-line header in the map's font. */
export function wrapHeader(text: string, width: number, measure: (s: string) => number): string[] {
  const lines: string[] = [];
  let line = "";
  for (const part of text.split(" · ")) {
    const joined = line ? `${line} · ${part}` : part;
    if (line && measure(joined) > width) { lines.push(line); line = part; } else line = joined;
  }
  if (line) lines.push(line);
  return lines;
}

/** One band per group, stacked, each group's blocks wrapped to `width` under a header of
 *  `headerLines` lines. Block 0 (the boot block on a 1 KiB-block disk) lies outside every group
 *  and has no cell. */
export function layoutBands(geo: ExtGeometry, width: number, headerLines = 1): MapLayout {
  const cols = gridCols(width, CELL, GAP);
  const header = headerLines * HEADER;
  const bands: Band[] = [];
  let top = 0;
  for (const g of geo.groups) {
    const rows = gridRows(g.blockCount, cols);
    bands.push({ group: g.index, first: g.firstBlock, count: g.blockCount, top, cellsTop: top + header, rows });
    top += header + rows * PITCH + BAND_GAP;
  }
  return { cols, bands, height: Math.max(1, top - BAND_GAP) };
}

/** The top-left corner of block `b`'s cell, or null when no band holds it. */
export function cellOf(m: MapLayout, b: number): { x: number; y: number } | null {
  const band = m.bands.find((x) => b >= x.first && b < x.first + x.count);
  if (!band) return null;
  const c = cellRect(b - band.first, m.cols, CELL, GAP);
  return { x: c.x, y: band.cellsTop + c.y };
}

/** The block whose cell (or the gap after it) is under `(x, y)`; null over a header row, past a
 *  band's last block, or outside the map. */
export function blockAt(m: MapLayout, x: number, y: number): number | null {
  for (const band of m.bands) {
    if (y < band.cellsTop || y >= band.cellsTop + band.rows * PITCH) continue;
    const i = cellAt(x, y - band.cellsTop, m.cols, band.count, CELL, GAP);
    return i === null ? null : band.first + i;
  }
  return null;
}

/** Each block's fill, indexed by block: metadata by region kind (the table's `colorForRegion`),
 *  every block in `journalBlocks` `COLOR_JOURNAL`, an owned data or directory block its owner's
 *  colour, and a free block `FILL_FREE`. */
export function blockFills(t: AttributionTable, geo: ExtGeometry, journalBlocks: readonly number[]): Uint8Array {
  const fills = new Uint8Array(geo.totalBlocks).fill(FILL_FREE);
  for (let b = 0; b < geo.totalBlocks; b++) {
    const a = attrAtSector(t, b);
    if (!a.free) fills[b] = a.colorIndex;
  }
  for (const b of journalBlocks) if (b >= 0 && b < geo.totalBlocks) fills[b] = COLOR_JOURNAL;
  return fills;
}

/** The blocks of every owner row with `role: "indirect"` (the map's 2 px dot), in owner order. */
export function indirectBlocks(t: AttributionTable): number[] {
  return t.owners.filter((o) => o.role === "indirect").map((o) => o.unit);
}

/** Every block the byte ranges touch, ascending, each once, clipped to the disk. */
export function blocksTouched(ranges: readonly Interval[], blockSize: number, totalBlocks: number): number[] {
  const out = new Set<number>();
  for (const r of ranges) {
    if (r.end <= r.start) continue;
    const last = Math.min(totalBlocks - 1, Math.floor((r.end - 1) / blockSize));
    for (let b = Math.floor(r.start / blockSize); b <= last; b++) out.add(b);
  }
  return [...out].sort((a, b) => a - b);
}

/** The hover caption: `block ${b} · ${regionName}`, then ` · ${ownerPath}` when owned or
 *  ` · free` for a free data block. */
export function blockCaption(t: AttributionTable, b: number): string {
  const a = attrAtSector(t, b);
  const tail = a.ownerPath !== undefined ? ` · ${a.ownerPath}` : a.free ? " · free" : "";
  return `block ${b} · ${a.regionName}${tail}`;
}

/** The path a click on block `b` selects: its owner, except a pseudo-owner such as the
 *  journal's `<journal>` rows, which names no file in the tree (a click there only jumps). */
export function clickPath(t: AttributionTable, b: number): string | null {
  const path = attrAtSector(t, b).ownerPath;
  return path === undefined || isPseudoOwner(path) ? null : path;
}
```

- [ ] **Step 16: Run it and watch it pass.**

```sh
pnpm exec vitest run tests/fs/blockMap.test.ts
```

Expected: `Test Files  1 passed (1)`, `Tests  11 passed (11)`.

- [ ] **Step 17: Write the map panel.** Create `web/ui/src/fs/ext/BlockGroupMap.svelte`. It follows `FatMap.svelte`'s pattern and uses the same helpers (Task 2's `core/grid.ts` and `core/observeWidth.ts`): `volume.epoch` is read first in every `$derived` and `$effect` that touches the adapter's plain-field caches (`fs.geo`, `fs.journalBlocks`, `volume.attribution`), `use:observeWidth` tracks the width, `prepareCanvas` sizes the canvas at `devicePixelRatio` (which keeps the band headers, text, sharp) and hands back the scaled, cleared context, and one `$effect` repaints on `volume.epoch`, `layers.chain`, `layers.diff`, `selection.hoverOffset`, the headers, the layout, and the fills. Unlike FatMap it wraps the headers to the width, measuring them with the canvas. Measuring sets the canvas's font, so it happens in `paint()`, not in a `$derived`: `paint()` stores the tallest header's line count in `headerLines` (state that `layoutBands` reads) and, when that count changes, returns so the effect paints again with the taller or shorter header rows; it settles on the second pass. Measuring on every paint also picks up the web font once it has loaded. The node tests cannot load `.svelte` files (no Svelte plugin in `vitest.config.ts`); svelte-check at the gates and the browser step verify it.

```svelte
<script lang="ts">
  import { cellRect, dashCell, dotCell, drawChain, outlineCell, prepareCanvas } from "../../core/grid";
  import { observeWidth } from "../../core/observeWidth";
  import { layers } from "../../state/layers.svelte";
  import { selection } from "../../state/selection.svelte";
  import { volume } from "../../state/volume.svelte";
  import { CELL, FILL_FREE, GAP, HEADER, MAX_HEIGHT, bandHeader, blockAt, blockCaption, blockFills, blocksTouched, cellOf, clickPath, indirectBlocks, layoutBands, mapHeading, wrapHeader } from "./blockMap";
  import { asExt } from "./index";

  let canvas = $state<HTMLCanvasElement>();
  // The panel's content width, kept current by `observeWidth` on the wrapper so the bands wrap
  // to the sidebar's actual size.
  let width = $state(0);
  let hoverBlock = $state<number | null>(null);
  /** The lines the tallest band header needs at this width in the map's font; every band's
   *  header row is that tall. `paint` measures it, because measuring sets the canvas's font, a
   *  side effect no `$derived` may have. */
  let headerLines = $state(1);

  // The ext view of the volume's adapter: `geo` (the groups and their free counts) and
  // `journalBlocks`. Its caches are plain fields refreshed per op, not reactive, so every
  // `$derived` and `$effect` below that reads them reads `volume.epoch` first (the rule in
  // fs/adapter.ts).
  const fs = $derived(asExt(volume.adapter));
  const geo = $derived((volume.epoch, fs.geo));
  const headers = $derived(geo.groups.map(bandHeader));
  const map = $derived(layoutBands(geo, width, headerLines));
  const fills = $derived((volume.epoch, blockFills(volume.attribution, geo, fs.journalBlocks)));
  const heading = $derived(mapHeading(geo));

  $effect(() => {
    volume.epoch; layers.chain; layers.diff; selection.hoverOffset; headers; map; fills;
    paint();
  });

  function headerFont(el: HTMLCanvasElement): string {
    return `11px ${getComputedStyle(el).getPropertyValue("--font-mono").trim()}`;
  }

  function paint() {
    if (!canvas) return;
    // Drawn at the device's ratio, so the band headers stay sharp.
    const ctx = prepareCanvas(canvas, Math.max(1, Math.floor(width)), map.height);
    if (!ctx) return;

    // Each band's header in lines that fit the width in the map's font (a one-line header is
    // wider than the sidebar), measured on every paint so a late web font is picked up. When
    // the tallest needs a different number of lines the layout changes: record it and let the
    // effect paint again with the new layout.
    ctx.font = headerFont(canvas);
    const wrapped = width > 0 ? headers.map((t) => wrapHeader(t, Math.floor(width), (s) => ctx.measureText(s).width)) : headers.map((t) => [t]);
    const lines = Math.max(1, ...wrapped.map((l) => l.length));
    if (lines !== headerLines) {
      headerLines = lines;
      return;
    }

    // Read once per paint (not cached across paints) so a light/dark switch is
    // picked up on the very next repaint without any extra wiring.
    const style = getComputedStyle(canvas);
    const hairline = style.getPropertyValue("--hairline").trim();
    const diffColor = style.getPropertyValue("--diff").trim();
    const focus = style.getPropertyValue("--focus").trim();
    const ink = style.getPropertyValue("--ink").trim();
    const muted = style.getPropertyValue("--ink-muted").trim();
    const own = new Map<number, string>();
    const fillFor = (f: number) => {
      if (f === FILL_FREE) return hairline;
      let c = own.get(f);
      if (c === undefined) own.set(f, (c = style.getPropertyValue(`--own-${f}`).trim()));
      return c;
    };

    ctx.textBaseline = "middle";
    ctx.fillStyle = muted;
    map.bands.forEach((band, i) => wrapped[i].forEach((line, k) => ctx.fillText(line, 0, band.top + k * HEADER + HEADER / 2 - 1)));

    // Runs of one colour are the rule (a file's blocks, the journal, free space), so the fill
    // style changes only at a run's edge.
    let last = -1;
    for (const band of map.bands) {
      for (let i = 0; i < band.count; i++) {
        const f = fills[band.first + i];
        if (f !== last) { ctx.fillStyle = fillFor(f); last = f; }
        const c = cellRect(i, map.cols, CELL, GAP);
        ctx.fillRect(c.x, band.cellsTop + c.y, CELL, CELL);
      }
    }

    // A pointer block (a file's indirect block, or the journal's) gets a dot, like the FAT
    // map's end-of-chain mark.
    for (const b of indirectBlocks(volume.attribution)) {
      const c = cellOf(map, b);
      if (c) dotCell(ctx, c.x, c.y, CELL, ink);
    }

    for (const b of blocksTouched(layers.diff, geo.blockSize, geo.totalBlocks)) {
      const c = cellOf(map, b);
      if (c) outlineCell(ctx, c.x, c.y, CELL, diffColor);
    }

    const chain = layers.chain.map((b) => cellOf(map, b)).filter((c): c is { x: number; y: number } => c !== null);
    if (chain.length) drawChain(ctx, chain, CELL, focus, 1);

    // Cross-link with the hex dump: the block under the mouse there gets a dashed outline
    // here, so the map shows where in the whole disk the hovered byte lives.
    if (selection.hoverOffset !== null) {
      const c = cellOf(map, Math.floor(selection.hoverOffset / geo.blockSize));
      if (c) dashCell(ctx, c.x, c.y, CELL, focus);
    }
  }

  function onMove(e: MouseEvent) {
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    hoverBlock = blockAt(map, e.clientX - rect.left, e.clientY - rect.top);
  }
  function onLeave() { hoverBlock = null; }
  function onClick() {
    if (hoverBlock === null) return;
    const path = clickPath(volume.attribution, hoverBlock);
    if (path !== null) selection.select(path);
    selection.jumpTo(hoverBlock * geo.blockSize);
  }

  const caption = $derived.by(() => {
    if (hoverBlock === null) return "";
    volume.epoch;
    return blockCaption(volume.attribution, hoverBlock);
  });
</script>

<section class="panel bgmap">
  <h2>{heading}</h2>
  {#if !volume.atLatest}<p class="muted stale-note">Shows the latest state, not the step you are viewing.</p>{/if}
  <div class="bgmap-wrap" use:observeWidth={(w) => (width = w)} style:max-height="{MAX_HEIGHT}px">
    <canvas bind:this={canvas} onmousemove={onMove} onmouseleave={onLeave} onclick={onClick} aria-label="Block group map"></canvas>
  </div>
  <p class="mono muted bgmap-caption">{caption || " "}</p>
</section>
```

#### Round 4: the Format form and the Filesystem select

- [ ] **Step 18: Write the failing form-mapping tests.** In `web/ui/tests/fs/ext-format.test.ts` replace the `../../src/fs/ext/format` import with:

```ts
import { DEFAULTS, MKFS, SIZES, checkFormat, defaultInodesPerGroup, defaultJournalBlocks, formOptions, toWasmOptions, type ExtFamilyOptions, type ExtFormFields } from "../../src/fs/ext/format";
```

and append at the end of the file:

```ts

describe("formOptions (the Format form's fields as family options)", () => {
  const fields: ExtFormFields = { variant: "ext3", totalBlocks: 16384, inodesPerGroup: undefined, label: "", journalMode: "ordered", journalBlocks: undefined };

  it("reads a blank number input as the default and keeps the rest as typed", () => {
    expect(formOptions(fields)).toEqual({ variant: "ext3", totalBlocks: 16384, inodesPerGroup: undefined, label: "", journalMode: "ordered", journalBlocks: undefined });
    expect(toWasmOptions(formOptions(fields))).toEqual({ variant: "ext3", options: { totalBlocks: 16384, label: "", journalMode: "ordered" } });
    expect(formOptions({ ...fields, inodesPerGroup: null, journalBlocks: null })).toEqual(formOptions(fields));
    expect(formOptions({ ...fields, inodesPerGroup: Number.NaN }).inodesPerGroup).toBeUndefined();
    expect(formOptions({ ...fields, inodesPerGroup: 256, journalBlocks: 2048, journalMode: "data", label: "disk" })).toEqual({ variant: "ext3", totalBlocks: 16384, inodesPerGroup: 256, label: "disk", journalMode: "data", journalBlocks: 2048 });
  });

  it("formats the default disk from the untouched form", () => {
    const vol = formatWith(formOptions(fields));
    expect(vol.fsType()).toBe("ext3");
    expect(vol.extGeometry()).toEqual(Volume.formatExt3(undefined).extGeometry());
  });

  it("drops the journal fields on ext2, so switching the variant never trips the journal-on-ext2 error", () => {
    const two = formOptions({ ...fields, variant: "ext2", journalMode: "data", journalBlocks: 512 });
    expect(two).toEqual({ variant: "ext2", totalBlocks: 16384, inodesPerGroup: undefined, label: "" });
    expect(checkFormat(two).problem).toBeNull();
    expect(formatWith(two).fsType()).toBe("ext2");
    expect(checkFormat(formOptions({ ...fields, journalBlocks: 512 })).problem).toBe("the journal must be at least 1024 blocks");
  });
});
```

- [ ] **Step 19: Run it and watch it fail.**

```sh
pnpm exec vitest run tests/fs/ext-format.test.ts
```

Expected: `Tests  3 failed | 12 passed (15)`, each with `TypeError: (0 , formOptions) is not a function`.

- [ ] **Step 20: Write the mapping.** Append to `web/ui/src/fs/ext/format.ts` (after `toWasmOptions`):

```ts

/** The Format form's fields as its inputs bind them: a number input left blank reads as `null`
 *  or `undefined`. */
export interface ExtFormFields {
  variant: "ext2" | "ext3";
  totalBlocks: number;
  inodesPerGroup: number | null | undefined;
  label: string;
  journalMode: "ordered" | "data";
  journalBlocks: number | null | undefined;
}

/** The family options the Format form's fields mean: a blank (or unreadable) number is the
 *  default, and the journal fields count only on ext3, so a form switched to ext2 with journal
 *  fields filled in formats ext2 rather than tripping `toWasmOptions`'s error. */
export function formOptions(f: ExtFormFields): ExtFamilyOptions {
  const num = (v: number | null | undefined) => (typeof v === "number" && Number.isFinite(v) ? v : undefined);
  const o: ExtFamilyOptions = { variant: f.variant, totalBlocks: f.totalBlocks, inodesPerGroup: num(f.inodesPerGroup), label: f.label };
  if (f.variant === "ext3") {
    o.journalMode = f.journalMode;
    o.journalBlocks = num(f.journalBlocks);
  }
  return o;
}
```

(`fs/ext/index.ts` re-exports `./format` with `export *`, so both names are exported from `fs/ext` too.)

- [ ] **Step 21: Run it and watch it pass.**

```sh
pnpm exec vitest run tests/fs/ext-format.test.ts
```

Expected: `Tests  15 passed (15)`.

- [ ] **Step 22: Write the Format form.** Create `web/ui/src/fs/ext/ExtFormatForm.svelte`, reusing the FAT form's classes (`field`, `mono`, `warn`) so the two forms look alike:

```svelte
<script lang="ts">
  import { selection } from "../../state/selection.svelte";
  import { volume } from "../../state/volume.svelte";
  import { DEFAULTS, SIZES, checkFormat, defaultInodesPerGroup, defaultJournalBlocks, formOptions } from "./format";

  // Start from the disk every ext lesson runs on (ext3, 16 MB, ordered, default inode and
  // journal sizes); a blank number field means "the default", shown as its placeholder.
  let variant = $state<"ext2" | "ext3">(DEFAULTS.variant);
  let totalBlocks = $state<number>(DEFAULTS.totalBlocks);
  let inodesPerGroup = $state<number | null | undefined>(undefined);
  let label = $state<string>(DEFAULTS.label);
  let journalMode = $state<"ordered" | "data">("ordered");
  let journalBlocks = $state<number | null | undefined>(undefined);

  const options = $derived(formOptions({ variant, totalBlocks, inodesPerGroup, label, journalMode, journalBlocks }));
  /** The problem the options have, if any; it disables the button. */
  const check = $derived(checkFormat(options));

  function formatDisk() {
    volume.format("ext", options);
    selection.select(null);
  }
</script>

<label class="field">
  Variant
  <select bind:value={variant}>
    <option value="ext2">ext2</option>
    <option value="ext3">ext3</option>
  </select>
</label>
<label class="field">
  Size
  <select bind:value={totalBlocks}>
    {#each SIZES as s}<option value={s.totalBlocks}>{s.label}</option>{/each}
  </select>
</label>
<label class="field">
  Inodes per group
  <input class="mono" type="number" min="16" step="8" placeholder={String(defaultInodesPerGroup(totalBlocks))} bind:value={inodesPerGroup} />
</label>
<label class="field">
  Label
  <input class="mono" type="text" maxlength="16" bind:value={label} />
</label>
{#if variant === "ext3"}
  <label class="field">
    Journal mode
    <select bind:value={journalMode}>
      <option value="ordered">ordered</option>
      <option value="data">data</option>
    </select>
  </label>
  <label class="field">
    Journal blocks
    <input class="mono" type="number" min="1024" placeholder={String(defaultJournalBlocks(totalBlocks) ?? "")} bind:value={journalBlocks} />
  </label>
{/if}
{#if check.problem}<p class="format-check warn">{check.problem}</p>{/if}
<button onclick={formatDisk} disabled={check.problem !== null}>Format disk</button>
```

- [ ] **Step 23: Add the Filesystem select.** In `web/ui/src/components/ActionsPanel.svelte` replace

```ts
  import { PANELS } from "../fs/panels";

  /** The Format form of the mounted volume's family, from the panel registry. */
  const FormatPanel = $derived(PANELS[volume.adapter.id].format);
```

with

```ts
  import { FAMILIES } from "../fs";
  import type { FsFamilyId } from "../fs/adapter";
  import { PANELS } from "../fs/panels";

  /** The family the Format details will format: the mounted one until the Filesystem select
   *  picks another, and back to the mounted one whenever that changes (a format, a load). */
  const mounted = $derived(volume.adapter.id);
  let family = $state<FsFamilyId>(volume.adapter.id);
  $effect(() => { family = mounted; });

  /** The chosen family's Format form, from the panel registry. */
  const FormatPanel = $derived(PANELS[family].format);
```

and in the markup replace

```svelte
      <summary>Format</summary>
      <FormatPanel />
```

with

```svelte
      <summary>Format</summary>
      <label class="field">
        Filesystem
        <select id="format-family" bind:value={family}>
          {#each Object.values(FAMILIES) as f}<option value={f.id}>{f.name}</option>{/each}
        </select>
      </label>
      <FormatPanel />
```

`mounted` is a `$derived` of the id, so the effect re-runs only when the mounted family actually changes (a format of the same family leaves a user's pick alone), and the select sits inside the fieldset that is disabled while the timeline is rewound.

#### Round 5: the panel table, the extras slot, and the styles

- [ ] **Step 24: Write the failing layout guards.** In `web/ui/tests/layout.test.ts` add inside `describe("app.css layout guards", ...)`, after its last test (Task 2's `gives every palette colour, the journal's included, a dump row stripe and tint`):

```ts

  it("scrolls the block-group map inside its own box, never sideways", () => {
    // The canvas is sized from the last measured width, so between a resize and the observer's
    // next paint it can be wider than the column; hidden overflow keeps that from showing a
    // horizontal scrollbar, and the vertical scroll is what keeps a 256 MB disk's bands to 260 px.
    const wrap = rule(".bgmap-wrap");
    expect(wrap).toContain("overflow-y: auto");
    expect(wrap).toContain("overflow-x: hidden");
    // An inline canvas sits on the text baseline, leaving a strip under the last band that makes
    // the box scroll a few pixels even when the bands fit.
    expect(rule(".bgmap-wrap canvas")).toContain("display: block");
  });

  it("lets the Format details' inputs and selects fill the Actions column", () => {
    // The ext form's number inputs and the Filesystem select would otherwise keep their intrinsic
    // widths: ragged against the other fields, and able to overflow the 220 px left column, which
    // then scrolls sideways.
    expect(rule(".actions .field input, .actions .field textarea, .actions .field select")).toContain("width: 100%");
  });
```

- [ ] **Step 25: Run it and watch it fail.**

```sh
pnpm exec vitest run tests/layout.test.ts
```

Expected: `Tests  1 failed | 5 passed (6)`: the block-group map test fails (`the given combination of arguments (null and string) is invalid`, because there is no `.bgmap-wrap` rule yet); the Actions rule already exists and its guard passes.

- [ ] **Step 26: Add the styles.** In `web/ui/src/app.css`, after the `/* FAT map */` block's last line (`.fatmap-caption { ... }`), add:

```css

/* Block-group map (ext) */
.bgmap-wrap { overflow-y: auto; overflow-x: hidden; }
.bgmap-wrap canvas { display: block; cursor: pointer; }
.bgmap-caption { margin: 6px 0 0; font-size: 11px; min-height: 14px; }
```

and after `.actions .cluster-count { margin: 0 0 6px; font-size: 12px; }` add:

```css
.actions .format-check { margin: 0 0 6px; font-size: 12px; }
```

- [ ] **Step 27: Register the ext panels.** In `web/ui/src/fs/panels.ts` add after `import FormatForm from "./fat16/FormatForm.svelte";`:

```ts
import BlockGroupMap from "./ext/BlockGroupMap.svelte";
import ExtFormatForm from "./ext/ExtFormatForm.svelte";
```

replace the first four lines of the doc comment's body

```ts
 * The Svelte panels of each family, looked up by `volume.adapter.id`: `map` is the panel App
 * renders under the Files tree (the FAT map), `format` the body of the Actions panel's Format
 * details, and `extras` the family's further panels, rendered under the map in order (FAT has
 * none; ext's Journal panel goes here).
```

with

```ts
 * The Svelte panels of each family, looked up by `volume.adapter.id`: `map` is the panel App
 * renders under the Files tree (the FAT map, the block-group map), `format` the body of the
 * Actions panel's Format details for the family its Filesystem select names, and `extras` the
 * family's further panels, rendered under the map in order (FAT has none; ext's Journal panel
 * goes here).
```

and add the ext entry after the fat16 one:

```ts
  fat16: { map: FatMap, format: FormatForm, extras: [] },
  ext: { map: BlockGroupMap, format: ExtFormatForm, extras: [] },
```

- [ ] **Step 28: Render the extras.** In `web/ui/src/App.svelte` replace the `MapPanel` doc comment

```ts
  /** The map panel of the mounted volume's family (the FAT map today), from the panel
   *  registry; `volume.adapter` is re-bound on every format and load. */
```

with

```ts
  /** The map panel of the mounted volume's family (the FAT map, the block-group map), from the
   *  panel registry; `volume.adapter` is re-bound on every format and load. The family's extra
   *  panels follow it in order. */
```

and in the left column add the extras line right after `<MapPanel />`:

```svelte
      <MapPanel />
      {#each PANELS[volume.adapter.id].extras as Extra}<Extra />{/each}
      <ActionsPanel />
```

- [ ] **Step 29: Run the layout guards and watch them pass.**

```sh
pnpm exec vitest run tests/layout.test.ts
```

Expected: `Tests  6 passed (6)`.

#### Gates, browser, and commit

- [ ] **Step 30: Run the web gates.**

```sh
pnpm test && pnpm build
```

Expected: `pnpm test` prints `Test Files  45 passed (45)` and `Tests  416 passed (416)`; `pnpm build` prints svelte-check's `COMPLETED ... 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS` and vite's `✓ built in ...`. Then `git status --short` lists only the files in **Files** (no `pnpm-lock.yaml`, no `pnpm-workspace.yaml`).

- [ ] **Step 31: Check it in the browser.** In `web/ui`, `pnpm dev` (or `pnpm exec vite --port <free port> --strictPort`), open the page, and check:
  1. The app opens on FAT16 as before; the Actions panel's Format details now start with a `Filesystem` select showing `FAT16` (options `FAT16`, `ext`), followed by the unchanged FAT form (Size, Sectors per cluster, `8,167 clusters`, Volume label, Format disk).
  2. Pick `ext`: the form becomes Variant `ext3`, Size `16 MB`, Inodes per group (placeholder `512`), Label, Journal mode `ordered`, Journal blocks (placeholder `1024`). Typing `500` in Journal blocks shows `the journal must be at least 1024 blocks` and disables Format disk; clearing it re-enables the button. Variant `ext2` hides the two journal fields.
  3. Format disk (ext3, 16 MB): the map panel is `Block groups · 2 groups · 16,384 blocks`, the tree shows `/` and `lost+found` (first block 70), and the map draws two bands, each headed on two lines in the 220 px column (`group 0 · blocks 1–8192` / `7,082 free`), with the metadata cells grey and violet, the root and lost+found teal, the journal band amber from block 82 through the pointer blocks 1106..1110 (each with an ink dot), and free cells in the hairline colour; the map scrolls inside its 260 px box.
  4. Hover: block 1107 captions `block 1107 · data (group 0) · <journal>`, a cell in the inode table `block 30 · inode table (group 0)`, a header row blank. Click a metadata cell: the dump's cursor jumps to that block (Inspector `Block 30 · inode table (group 0)`). Click block 1107: the dump jumps there, nothing is selected, and the Inspector reads `Block` / `1107 · data (group 0) · pointer block of the journal` and `Owner` / `<journal>` as plain text (no link). Hovering a byte in the dump draws a dashed focus outline on its block.
  5. Add file (`/Hello world.txt`): the tree shows `Hello world.txt · 22 B · first block 1111`, the band header drops to `7,081 free`, block 1111 takes the file's hue with the focus outline of the selected chain, and the blocks the op wrote (69, the journal's 83..91, 1111, the metadata) carry the diff outline. After a second op, scrubbing back one step shows `Shows the latest state, not the step you are viewing.` under the heading.
  6. Pick `FAT16` in the Filesystem select and Format disk: the map panel is `FAT map · 8,167 clusters` again, the FAT form is unchanged, the select reads `FAT16`, and the console has no errors. An ext2 4 MB format heads the map `Block groups · 1 group · 4,096 blocks`.

- [ ] **Step 32: Commit.** From the repo root:

```sh
git add web/ui/src/App.svelte web/ui/src/app.css web/ui/src/components/ActionsPanel.svelte web/ui/src/components/Inspector.svelte web/ui/src/core/attribution.ts web/ui/src/fs/adapter.ts web/ui/src/fs/ext web/ui/src/fs/index.ts web/ui/src/fs/panels.ts web/ui/tests/attribution.test.ts web/ui/tests/fs/ext-format.test.ts web/ui/tests/fs/ext.test.ts web/ui/tests/fs/registry.test.ts web/ui/tests/fs/blockMap.test.ts web/ui/tests/layout.test.ts
git commit -m "feat(ui): register the ext family with a block-group map and Format form"
```

Facts later tasks rely on (pinned by this task's tests):

- `ExtAdapter.owners` contains `<journal>` rows (`JOURNAL_OWNER`, exported from `fs/ext`) for the pointer blocks (1106..1110 on the default disk) with `role: "indirect"`, `color: COLOR_JOURNAL`, `firstUnit` the row's own block; anything iterating `owners` for files (the tree, the lessons) must skip them with `isPseudoOwner(o.path)` (from `fs/adapter`) or compare with `JOURNAL_OWNER`, never a bare `"<journal>"`. `buildTree` is unaffected (it walks `listDir`, and `<journal>` is never listed), and `ownerOf(JOURNAL_OWNER)` now returns the first pointer row. `describeUnit` returns `pointer block of the journal` for the pointer blocks and `null` for the journal's data blocks (82..1105), and `journalBlocks` still lists 82..1110.
- `volume.attribution` now calls blocks 1106..1110 owned: every `attrAtSector` on them reports `ownerPath: "<journal>"`, `role: "indirect"`, `colorIndex: 11`. The Inspector shows that owner as plain text; the map's click on it only jumps.
- Map strings: heading `Block groups · ${groups.toLocaleString()} group(s) · ${totalBlocks.toLocaleString()} blocks`; band header `group ${i} · blocks ${first}–${last} · ${freeBlocks.toLocaleString()} free` (the range raw with an en dash, the free count localized; wrapped at ` · ` when narrow); caption `block ${b} · ${regionName}` + ` · ${ownerPath}` | ` · free`; `aria-label="Block group map"`; classes `panel bgmap`, `bgmap-wrap`, `bgmap-caption`.
- The map's cells come from Task 2's `core/grid.ts` with `CELL = 4`, `GAP = 1` (exported from `blockMap.ts`) and its box is `MAX_HEIGHT = 260`; Task 5's Journal ring imports those three so the two strips read alike.
- Form: option values Variant `ext2`/`ext3` (default `ext3`), Size `4096`/`16384`/`65536`/`262144` labelled `4 MB`/`16 MB`/`64 MB`/`256 MB` (default `16384`), Journal mode `ordered`/`data` (default `ordered`); field labels `Variant`, `Size`, `Inodes per group`, `Label`, `Journal mode`, `Journal blocks`; the problem line is `<p class="format-check warn">`.
- Filesystem select: `<select id="format-family">` labelled `Filesystem`, options `value="fat16"` `FAT16` and `value="ext"` `ext`; it follows the mounted family whenever `volume.adapter.id` changes.
- `PANELS.ext.extras` is `[]`; Task 5 sets it to `[JournalPanel]`, which `App.svelte` already renders after the map.

---

### Task 5: the Journal panel (ring, facts, crash and recovery controls)

Spec: `docs/superpowers/specs/2026-09-24-ext-explorer-design.md` sections 2 (the journal capability: `state()` from the adapter's cache, `blocks()` read per call and memoised per epoch by callers, `recover()` run through `volume.run`; `VolumeStore.needsRecovery`), 5 (the `JournalPanel.svelte` bullet and the `StatusLine.svelte` bullet), 8 (`tests/fs/ext.test.ts`: the journal capability; `tests/layout.test.ts` covers the new panel's styles; the browser pass), 10 (the Journal panel memoises `journal.blocks()` per epoch).

The ext family's `extras` gains the Journal panel. `App.svelte` already renders extras (Task 4), so the panel appears under the block-group map. On ext2 it is one muted line. On ext3 it shows:

- the mode and the header facts;
- the needs-recovery line while recovery is needed;
- a ring strip of the journal's blocks: one 4 px cell per journal index in reading order, coloured by kind, with stale transactions at 35 % alpha;
- a hover caption, and a click that jumps the dump to the block;
- the crash controls: a phase select (replaced while armed by the armed line), one button that reads `Arm` or, while armed, `Disarm`, and `Recover`, enabled only while the volume needs recovery.

As with the block-group map, the panel's text and colours live in a plain module, `journalRing.ts`, so the node tests can pin them. The component only paints and wires events. Its cell geometry is not new code: the ring is one grid on Task 2's `core/grid.ts` (`gridCols`, `gridRows`, `cellRect`, `cellAt`, tested in `tests/grid.test.ts`) in the block-group map's `CELL`, `GAP`, and `MAX_HEIGHT` (imported from `blockMap.ts`), `prepareCanvas` (also `core/grid.ts`) sizes its canvas at `devicePixelRatio` as the block-group map's, and `core/observeWidth.ts` (`use:observeWidth`) tracks its width.

Task 2 already added the status line's `Volume needs recovery` line, the `NeedsRecovery` friendly text, and the store's armed crash phase (`volume.armedPhase`, `volume.setArmedPhase`), so this task does not touch `StatusLine.svelte` or `state/volume.svelte.ts`. The panel keeps no copy of the armed phase: it reads `volume.armedPhase` and arms through the store, and Task 6's `crash` arms through the store too, so a `crash` typed in the terminal shows in this panel immediately, and the panel's Arm shows in the terminal's next `crash` output. Task 3's `ExtJournal` and `ExtAdapter: caches and the journal capability` tests already pin the capability round trip (arm, phase, a crash, `needsRecovery`, `recover()`, clear). This task adds a ring-level version of it: what the panel draws before, during, and after a crash.

The test counts in the **Expected** lines assume Tasks 1 to 4 were applied in order first (with Task 2's `tests/grid.test.ts`, `tests/freeSpace.test.ts`, and `tests/scrollNonce.test.ts`): 45 files and 416 tests before this task, 45 files and 422 tests after it.

The spec's text did not settle these choices. Each one is visible in the code below:

- **Ring size:** the ring uses the block-group map's cell size and 260 px scroll box (its `CELL = 4`, `GAP = 1`, `MAX_HEIGHT = 260`).
- **Controls layout:** the phase select defaults to `after_commit` (the terminal's `crash` default) and takes a full row (the armed line takes it while armed). `Arm`/`Disarm` and `Recover` go on the row under it, because the 198 px column cannot fit the select's `during checkpoint` beside two buttons. `Arm` and `Disarm` are one button whose label changes, so keyboard focus stays on it.
- **Rewound view:** while rewound, the panel shows the other panels' `Shows the latest state, not the step you are viewing.` note, because its facts and ring describe the latest state.
- **Number format:** the facts print with `toLocaleString()` (`blocks 1,024`); the caption's block numbers, journal indices, and transaction ids are raw (`journal block 1023 (block 1105)`), like the dump's addresses.
- **Caption tail:** the spec's ` · stale` / ` · live` is appended with ` · transaction ${tid}`, so only to a block that belongs to a transaction (`tid` not null); the journal superblock and unused blocks have no transaction and end at their kind (`journal block 0 (block 82) · superblock`).
- **Hover reset:** the hover is dropped when a format or load binds a new capability.

**Files**

- Create: `web/ui/src/fs/ext/journalRing.ts` (1–54), `web/ui/src/fs/ext/JournalPanel.svelte` (1–115)
- Modify: `web/ui/src/fs/panels.ts` (7 import, 13–16 doc comment, 20 `ext` entry), `web/ui/src/app.css` (144–155)
- Test: `web/ui/tests/fs/ext.test.ts` (3, 13–15, 528–591), `web/ui/tests/layout.test.ts` (56–70)
- Not touched:
  - `crates/`, `web/ui/src/fs/fat16/**`, `web/ui/src/fs/ext/adapter.ts`, `web/ui/src/fs/ext/blockMap.ts` (only imported), `web/ui/src/fs/base.ts`, `web/ui/src/core/**`, `web/ui/src/shell/**`, `web/ui/src/state/**`.
  - `web/ui/src/components/**`: StatusLine already has the needs-recovery line and text.
  - `web/ui/tests/adapterBoundary.test.ts`: both new files are under `src/fs/ext/` and call no wasm method. The panel reaches the journal through `volume.adapter.journal`, the seam's `JournalCapability`.

**Interfaces**

Consumes:

- `fs/adapter.ts`: `CRASH_PHASES`, `CRASH_PHASE_LABELS`, `CrashPhase`, `JournalState`, `JournalRingBlock`, `FsAdapter.journal?: JournalCapability`.
- `core/palette.ts`: `COLOR_BOOT = 1`, `COLOR_TABLE = 2`.
- `state/volume.svelte.ts`: `volume.adapter`, `volume.epoch`, `volume.atLatest`, `volume.needsRecovery`, `volume.sectorSize`, `volume.run`, and Task 2's `volume.armedPhase: CrashPhase | null`, `volume.setArmedPhase(phase: CrashPhase | null): boolean` (null disarms).
- `core/grid.ts` (Task 2): `prepareCanvas`, `gridCols`, `gridRows`, `cellRect`, `cellAt`; `core/observeWidth.ts`: `observeWidth` (a Svelte action, `use:observeWidth={(w) => (width = w)}`).
- `fs/ext/blockMap.ts` (Task 4): `CELL = 4`, `GAP = 1`, `MAX_HEIGHT = 260`.
- `state/selection.svelte.ts`: `selection.jumpTo`.
- `fs/panels.ts`: `PANELS` with `extras`.
- Task 3's `ExtAdapter`, `ExtJournal`, `asExt`, and `ext` (tests only).
- The CSS tokens `--own-1`, `--own-2`, `--ink`, `--focus`, `--diff`, `--hairline`, and `--diff-ink`, and the existing `.facts`, `.stale-note`, and `.panel` rules.

Produces (exact):

```ts
// fs/ext/journalRing.ts (new; imported by JournalPanel.svelte and the tests only; the ring's geometry is core/grid over blockMap's CELL, GAP, MAX_HEIGHT)
export const STALE_ALPHA = 0.35;
export const NO_JOURNAL = "This volume has no journal.";
export const NEEDS_RECOVERY = "Needs recovery: the journal holds an unfinished transaction.";
export function journalHeading(s: JournalState): string;             // `Journal · ordered mode`
export function journalFacts(s: JournalState): [string, string][];   // [["sequence","1"],["head","1"],["start","0"],["blocks","1,024"]]
export function armedText(phase: CrashPhase): string;                // `Armed: the next change stops after commit`
export const RING_COLORS: Record<JournalRingBlock["kind"], string>;  // superblock "--own-1", descriptor "--own-2", copy "--ink", commit "--focus", revoke "--diff", unused "--hairline"
export function ringFill(b: JournalRingBlock): { token: string; alpha: number };   // alpha STALE_ALPHA when b.stale, else 1
export function ringCaption(b: JournalRingBlock): string;   // `journal block ${index} (block ${block}) · ${kind}` + ` · copy of block ${home}` (copy) + ` · transaction ${tid} · stale|live` (tid not null)

// fs/panels.ts
//   ext: { map: BlockGroupMap, format: ExtFormatForm, extras: [JournalPanel] }

// Svelte (not importable by node tests)
// fs/ext/JournalPanel.svelte: <section class="panel journal">;
//   no capability: <p class="muted journal-none">This volume has no journal.</p>
//   ext3: <h2>Journal · ${mode} mode</h2>, the stale note while rewound, <dl class="facts"> (dt sequence/head/start/blocks, dd.mono),
//   <p class="journal-flag"> NEEDS_RECOVERY while volume.needsRecovery, <div class="journal-wrap"> (max-height 260px) with <canvas aria-label="Journal ring">,
//   <p class="mono muted journal-caption">, <div class="journal-controls">: <select id="journal-phase" aria-label="Crash phase"> (values before_commit|after_commit|during_checkpoint,
//   labels from CRASH_PHASE_LABELS, default after_commit), or while volume.armedPhase <span class="journal-armed">Armed: …</span>; then one <button> reading
//   Arm (volume.setArmedPhase(phase)) or, while armed, Disarm (volume.setArmedPhase(null)); then <button>Recover</button> (disabled unless volume.needsRecovery);
//   every control disabled while !volume.atLatest.
// app.css: .journal .facts, .journal .facts dd, .journal-flag, .journal-wrap, .journal-wrap canvas, .journal-caption, .journal-controls, .journal-controls select, .journal-armed, .journal-none
```

Environment for every command below, from `web/ui`: any pnpm 10.26 or newer (nvm's pnpm 12 and Homebrew's 10.33 both work). Both web packages keep their pnpm settings in a tracked `pnpm-workspace.yaml`, so a frozen install must leave `web/ui/pnpm-lock.yaml` and `web/ui/pnpm-workspace.yaml` unmodified; if either shows as modified in `git status` at any point, restore it with `git checkout web/ui/pnpm-lock.yaml web/ui/pnpm-workspace.yaml` and run `CI=true pnpm install --frozen-lockfile` again (`CI=true` lets pnpm purge a stale modules directory without asking).

#### Round 1: the ring's model

- [ ] **Step 1: Build the wasm package and install it.** From the repo root:

```sh
wasm-pack build crates/wasm --target bundler
cd web/ui && CI=true pnpm install --frozen-lockfile && pnpm test
```

Expected: wasm-pack ends `Your wasm pkg is ready to publish at crates/wasm/pkg.`; `pnpm test` prints `Test Files  45 passed (45)` and `Tests  416 passed (416)`.

- [ ] **Step 2: Write the failing ring tests.** In `web/ui/tests/fs/ext.test.ts` replace the import line

```ts
import { isPseudoOwner, unitIsSector } from "../../src/fs/adapter";
```

with

```ts
import { CRASH_PHASES, isPseudoOwner, unitIsSector, type JournalRingBlock } from "../../src/fs/adapter";
```

add after `import { CORRUPT_NOTE, NOTES, touchesMetadata } from "../../src/fs/ext/metadata";`:

```ts
import {
  NEEDS_RECOVERY, NO_JOURNAL, RING_COLORS, STALE_ALPHA, armedText, journalFacts, journalHeading, ringCaption, ringFill,
} from "../../src/fs/ext/journalRing";
```

and append at the end of the file, after the closing `});` of `describe("ExtAdapter: caches and the journal capability", ...)`:

```ts

describe("the Journal panel's model", () => {
  /** A ring block with the fields a test does not name filled in as an unused block's. */
  const blk = (over: Partial<JournalRingBlock>): JournalRingBlock => ({ index: 0, block: 82, kind: "unused", tid: null, home: null, escaped: null, stale: false, ...over });

  it("heads the panel with the mode and lists the header facts", () => {
    const fs = asExt(ext.bind(ext.format()));
    expect(journalHeading(fs.journal!.state())).toBe("Journal · ordered mode");
    expect(journalFacts(fs.journal!.state())).toEqual([["sequence", "1"], ["head", "1"], ["start", "0"], ["blocks", "1,024"]]);
    const data = asExt(ext.bind(ext.format({ totalBlocks: 4096, journalMode: "data" })));
    expect(journalHeading(data.journal!.state())).toBe("Journal · data mode");
  });

  it("words the no-journal line, the needs-recovery line, and the armed line for each phase", () => {
    expect(NO_JOURNAL).toBe("This volume has no journal.");
    expect(NEEDS_RECOVERY).toBe("Needs recovery: the journal holds an unfinished transaction.");
    expect(CRASH_PHASES.map(armedText)).toEqual([
      "Armed: the next change stops before commit",
      "Armed: the next change stops after commit",
      "Armed: the next change stops during checkpoint",
    ]);
  });

  it("colours each kind by its token, stale blocks at 35 % alpha and live ones full", () => {
    expect(RING_COLORS).toEqual({ superblock: `--own-${COLOR_BOOT}`, descriptor: `--own-${COLOR_TABLE}`, copy: "--ink", commit: "--focus", revoke: "--diff", unused: "--hairline" });
    expect(STALE_ALPHA).toBe(0.35);
    expect(ringFill(blk({ kind: "superblock" }))).toEqual({ token: "--own-1", alpha: 1 });
    expect(ringFill(blk({ kind: "descriptor", tid: 1, stale: true }))).toEqual({ token: "--own-2", alpha: 0.35 });
    expect(ringFill(blk({ kind: "copy", tid: 1, home: 69, stale: true }))).toEqual({ token: "--ink", alpha: 0.35 });
    expect(ringFill(blk({ kind: "copy", tid: 2, home: 69, stale: false }))).toEqual({ token: "--ink", alpha: 1 });
    expect(ringFill(blk({ kind: "commit", tid: 2 }))).toEqual({ token: "--focus", alpha: 1 });
    expect(ringFill(blk({ kind: "revoke", tid: 3, stale: true }))).toEqual({ token: "--diff", alpha: 0.35 });
    expect(ringFill(blk({ kind: "unused" }))).toEqual({ token: "--hairline", alpha: 1 });
  });

  it("captions each kind: the home of a copy, the transaction and its staleness when there is one", () => {
    expect(ringCaption(blk({ index: 0, block: 82, kind: "superblock" }))).toBe("journal block 0 (block 82) · superblock");
    expect(ringCaption(blk({ index: 1, block: 83, kind: "descriptor", tid: 1, stale: true }))).toBe("journal block 1 (block 83) · descriptor · transaction 1 · stale");
    expect(ringCaption(blk({ index: 2, block: 84, kind: "copy", tid: 1, home: 1, escaped: false, stale: true }))).toBe("journal block 2 (block 84) · copy · copy of block 1 · transaction 1 · stale");
    expect(ringCaption(blk({ index: 17, block: 99, kind: "copy", tid: 2, home: 69, escaped: false }))).toBe("journal block 17 (block 99) · copy · copy of block 69 · transaction 2 · live");
    expect(ringCaption(blk({ index: 18, block: 100, kind: "commit", tid: 2 }))).toBe("journal block 18 (block 100) · commit · transaction 2 · live");
    expect(ringCaption(blk({ index: 12, block: 94, kind: "revoke", tid: 3, stale: true }))).toBe("journal block 12 (block 94) · revoke · transaction 3 · stale");
    expect(ringCaption(blk({ index: 1023, block: 1105, kind: "unused" }))).toBe("journal block 1023 (block 1105) · unused");
  });

  it("draws a checkpointed transaction stale and a crashed one live", () => {
    const vol = ext.format();
    const fs = asExt(ext.bind(vol));
    const drawn = () => fs.journal!.blocks().filter((b) => b.kind !== "unused").map((b) => `${b.index} ${ringFill(b).token} ${ringFill(b).alpha}`);
    expect(drawn()).toEqual(["0 --own-1 1"]); // an empty ring: the journal superblock alone
    vol.createFile("/hello.txt", enc("Hello, ext3!"));
    expect(drawn()).toEqual(["0 --own-1 1", "1 --own-2 0.35", ...range(2, 8).map((i) => `${i} --ink 0.35`), "9 --focus 0.35"]);
    fs.journal!.arm("after_commit");
    vol.createFile("/crash.txt", new Uint8Array(2 * BLOCK));
    fs.refresh();
    expect(fs.needsRecovery).toBe(true);
    expect(drawn().slice(10)).toEqual(["10 --own-2 1", ...range(11, 17).map((i) => `${i} --ink 1`), "18 --focus 1"]);
    expect(ringCaption(fs.journal!.blocks()[18])).toBe("journal block 18 (block 100) · commit · transaction 2 · live");
    fs.journal!.recover();
    fs.refresh();
    expect(fs.needsRecovery).toBe(false);
    expect(drawn().slice(10).every((d) => d.endsWith(" 0.35"))).toBe(true);
  });
});
```

`BLOCK`, `enc`, `range`, `ext`, `asExt`, `COLOR_BOOT`, and `COLOR_TABLE` already exist in this file. The ring's layout (one cell per journal index in reading order, 39 columns × 27 rows in the 198 px column) is Task 2's grid, pinned in `tests/grid.test.ts` with these very numbers, so it has no test here. The fifth test is the capability round trip as the panel sees it:

1. An empty ring.
2. A checkpointed transaction, drawn stale.
3. A crash after commit, which draws transaction 2 live and sets `needsRecovery` true.
4. `recover()`, which turns it stale again.

- [ ] **Step 3: Run them and watch them fail.**

```sh
pnpm exec vitest run tests/fs/ext.test.ts
```

Expected: `Test Files  1 failed (1)`, `Tests  no tests`, with `Error: Cannot find module '../../src/fs/ext/journalRing' imported from '.../web/ui/tests/fs/ext.test.ts'`.

- [ ] **Step 4: Write the model.** Create `web/ui/src/fs/ext/journalRing.ts`:

```ts
import { COLOR_BOOT, COLOR_TABLE } from "../../core/palette";
import { CRASH_PHASE_LABELS, type CrashPhase, type JournalRingBlock, type JournalState } from "../adapter";

/**
 * The Journal panel's model, kept out of the component so the node tests can check it: the
 * text it prints and the colour of each journal block's cell. The ring is drawn in reading
 * order (journal index 0 first), one cell per block, on the shared grid (core/grid.ts) in the
 * block-group map's cell size and scroll box, so the two strips read alike.
 */

/** A block of a checkpointed (or discarded) transaction is drawn at this alpha; live ones full. */
export const STALE_ALPHA = 0.35;

export const NO_JOURNAL = "This volume has no journal.";
export const NEEDS_RECOVERY = "Needs recovery: the journal holds an unfinished transaction.";

/** `Journal · ordered mode`. */
export function journalHeading(s: JournalState): string {
  return `Journal · ${s.mode} mode`;
}

/** The facts list: `sequence`, `head`, `start`, and `blocks` (the ring's length, `maxlen`). */
export function journalFacts(s: JournalState): [string, string][] {
  return [["sequence", s.sequence.toLocaleString()], ["head", s.head.toLocaleString()], ["start", s.start.toLocaleString()], ["blocks", s.maxlen.toLocaleString()]];
}

/** The line that replaces the phase select while a crash is armed. */
export function armedText(phase: CrashPhase): string {
  return `Armed: the next change stops ${CRASH_PHASE_LABELS[phase]}`;
}

/** The CSS custom property each kind of journal block is drawn in. */
export const RING_COLORS: Record<JournalRingBlock["kind"], string> = {
  superblock: `--own-${COLOR_BOOT}`,
  descriptor: `--own-${COLOR_TABLE}`,
  copy: "--ink",
  commit: "--focus",
  revoke: "--diff",
  unused: "--hairline",
};

/** A cell's colour token and alpha. */
export function ringFill(b: JournalRingBlock): { token: string; alpha: number } {
  return { token: RING_COLORS[b.kind], alpha: b.stale ? STALE_ALPHA : 1 };
}

/** The hover caption: `journal block 2 (block 84) · copy · copy of block 1 · transaction 1 ·
 *  stale`; the home only on a copy, the transaction and its staleness only when there is one. */
export function ringCaption(b: JournalRingBlock): string {
  let s = `journal block ${b.index} (block ${b.block}) · ${b.kind}`;
  if (b.kind === "copy" && b.home !== null) s += ` · copy of block ${b.home}`;
  if (b.tid !== null) s += ` · transaction ${b.tid} · ${b.stale ? "stale" : "live"}`;
  return s;
}
```

- [ ] **Step 5: Run them and watch them pass.**

```sh
pnpm exec vitest run tests/fs/ext.test.ts
```

Expected: `Test Files  1 passed (1)`, `Tests  44 passed (44)`.

#### Round 2: the panel

- [ ] **Step 6: Write the component.** Create `web/ui/src/fs/ext/JournalPanel.svelte`:

```svelte
<script lang="ts">
  import { cellAt, cellRect, gridCols, gridRows, prepareCanvas } from "../../core/grid";
  import { observeWidth } from "../../core/observeWidth";
  import { selection } from "../../state/selection.svelte";
  import { volume } from "../../state/volume.svelte";
  import { CRASH_PHASES, CRASH_PHASE_LABELS, type CrashPhase } from "../adapter";
  import { CELL, GAP, MAX_HEIGHT } from "./blockMap";
  import { NEEDS_RECOVERY, NO_JOURNAL, armedText, journalFacts, journalHeading, ringCaption, ringFill } from "./journalRing";

  let canvas = $state<HTMLCanvasElement>();
  // The panel's content width, kept current by `observeWidth` on the wrapper so the ring wraps
  // to the sidebar's actual size.
  let width = $state(0);
  let hoverIndex = $state<number | null>(null);
  let phase = $state<CrashPhase>("after_commit");

  // The capability's `state()` answers from the adapter's cache and `blocks()` reads the
  // volume; neither is reactive, so everything that calls them reads `volume.epoch` first
  // (the rule in fs/adapter.ts). `blocks()` runs once per epoch, not once per paint or hover.
  const journal = $derived((volume.epoch, volume.adapter.journal));
  const info = $derived((volume.epoch, journal?.state()));
  const ring = $derived((volume.epoch, journal?.blocks() ?? []));
  // One cell per journal block in reading order, in the block-group map's cells.
  const cols = $derived(gridCols(width, CELL, GAP));
  const height = $derived(gridRows(ring.length, cols) * (CELL + GAP));
  const controlsOff = $derived(!volume.atLatest);

  // A format or load binds a new capability (or none, on ext2) under a canvas the mouse may
  // never have left; drop the old hover so the caption does not describe the old ring.
  $effect(() => {
    journal;
    hoverIndex = null;
  });

  $effect(() => {
    ring; cols; height;
    paint();
  });

  function paint() {
    if (!canvas) return;
    const ctx = prepareCanvas(canvas, Math.max(1, Math.floor(width)), height);
    if (!ctx) return;
    // Read once per paint so a light/dark switch shows on the next repaint.
    const style = getComputedStyle(canvas);
    const colors = new Map<string, string>();
    const colorOf = (token: string) => {
      let c = colors.get(token);
      if (c === undefined) colors.set(token, (c = style.getPropertyValue(token).trim()));
      return c;
    };
    for (const b of ring) {
      const { x, y } = cellRect(b.index, cols, CELL, GAP);
      const { token, alpha } = ringFill(b);
      ctx.globalAlpha = alpha;
      ctx.fillStyle = colorOf(token);
      ctx.fillRect(x, y, CELL, CELL);
    }
    ctx.globalAlpha = 1;
  }

  function onMove(e: MouseEvent) {
    if (!canvas) return;
    const rect = canvas.getBoundingClientRect();
    hoverIndex = cellAt(e.clientX - rect.left, e.clientY - rect.top, cols, ring.length, CELL, GAP);
  }
  function onLeave() { hoverIndex = null; }
  function onClick() {
    const b = hoverIndex === null ? undefined : ring[hoverIndex];
    if (b) selection.jumpTo(b.block * volume.sectorSize);
  }
  const caption = $derived.by(() => {
    const b = hoverIndex === null ? undefined : ring[hoverIndex];
    return b ? ringCaption(b) : "";
  });

  // The armed phase is the store's (`volume.armedPhase`), which the terminal's `crash` moves
  // too, so an arm from either side shows here at once. One button arms and disarms, so keyboard
  // focus stays on it when its label flips.
  function toggleArm() {
    volume.setArmedPhase(volume.armedPhase ? null : phase);
  }
  function recover() {
    const j = journal;
    if (j) volume.run(() => j.recover());
  }
</script>

<section class="panel journal">
  {#if journal && info}
    <h2>{journalHeading(info)}</h2>
    {#if !volume.atLatest}<p class="muted stale-note">Shows the latest state, not the step you are viewing.</p>{/if}
    <dl class="facts">
      {#each journalFacts(info) as [name, value] (name)}<dt>{name}</dt><dd class="mono">{value}</dd>{/each}
    </dl>
    {#if volume.needsRecovery}<p class="journal-flag">{NEEDS_RECOVERY}</p>{/if}
    <div class="journal-wrap" use:observeWidth={(w) => (width = w)} style:max-height="{MAX_HEIGHT}px">
      <canvas bind:this={canvas} onmousemove={onMove} onmouseleave={onLeave} onclick={onClick} aria-label="Journal ring"></canvas>
    </div>
    <p class="mono muted journal-caption">{caption || " "}</p>
    <div class="journal-controls">
      {#if volume.armedPhase}
        <span class="journal-armed">{armedText(volume.armedPhase)}</span>
      {:else}
        <select id="journal-phase" aria-label="Crash phase" bind:value={phase} disabled={controlsOff}>
          {#each CRASH_PHASES as p (p)}<option value={p}>{CRASH_PHASE_LABELS[p]}</option>{/each}
        </select>
      {/if}
      <button onclick={toggleArm} disabled={controlsOff}>{volume.armedPhase ? "Disarm" : "Arm"}</button>
      <button onclick={recover} disabled={controlsOff || !volume.needsRecovery}>Recover</button>
    </div>
  {:else}
    <p class="muted journal-none">{NO_JOURNAL}</p>
  {/if}
</section>
```

How the component follows the reactivity rule in `fs/adapter.ts`:

- `journal`, `info`, and `ring` each read `volume.epoch` first. `state()` and `blocks()` therefore re-run once per op, format, load, or seek, and never per paint or hover.
- Arming is not an op and does not move the epoch, so the armed phase is not derived from `journal.phase()` here. It is Task 2's store state `volume.armedPhase`: the button calls `volume.setArmedPhase(phase)`, or `volume.setArmedPhase(null)` while armed, which sets it, `refreshMeta` re-reads it after every op, format, and load (a crash uses the phase up), and Task 6's `crash` goes through the same store method. A `crash` typed in the terminal therefore shows here immediately, and `crash --off` brings the select back.
- The Arm/Disarm button is one element whose label follows `volume.armedPhase`, so a keyboard user's focus stays on it when it flips.
- `journal` is the same object across refreshes (Task 3 pins that), so the hover-reset effect runs only when a format or load binds a new adapter.

- [ ] **Step 7: Type-check it.**

```sh
pnpm exec svelte-check --tsconfig ./tsconfig.json
```

Expected: `COMPLETED ... 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS` (the component is not rendered yet; the next round registers it).

#### Round 3: the panels entry and the styles

- [ ] **Step 8: Write the failing layout guard.** In `web/ui/tests/layout.test.ts` add right before `  it("lets the Format details' inputs and selects fill the Actions column", () => {`:

```ts
  it("scrolls the Journal panel's ring inside its own box, never sideways", () => {
    // The same two faults as the block-group map's: a canvas sized from the last measured width
    // can briefly be wider than the column, and a 256 MB disk's 8,192-block ring is taller than
    // the 260 px box it scrolls in.
    const wrap = rule(".journal-wrap");
    expect(wrap).toContain("overflow-y: auto");
    expect(wrap).toContain("overflow-x: hidden");
    expect(rule(".journal-wrap canvas")).toContain("display: block");
    // The phase select (or, while armed, the armed line) takes a full row of the 220 px column
    // and Arm/Disarm and Recover wrap onto the row under it, rather than pushing the column
    // sideways; the select may shrink.
    expect(rule(".journal-controls")).toContain("flex-wrap: wrap");
    expect(rule(".journal-controls select")).toContain("min-width: 0");
  });

```

- [ ] **Step 9: Run it and watch it fail.**

```sh
pnpm exec vitest run tests/layout.test.ts
```

Expected: `Tests  1 failed | 6 passed (7)`. The new test fails with `the given combination of arguments (null and string) is invalid for this assertion` because there is no `.journal-wrap` rule yet.

- [ ] **Step 10: Add the styles.** In `web/ui/src/app.css`, add the following before `/* Step panel */`, after the block-group map's last line (`.bgmap-caption { margin: 6px 0 0; font-size: 11px; min-height: 14px; }`) and its blank line:

```css
/* Journal panel (ext3) */
.journal .facts { margin: 0 0 6px; }
.journal .facts dd { margin: 0; }
.journal-flag { margin: 0 0 6px; font-size: 12px; color: var(--diff-ink); }
.journal-wrap { overflow-y: auto; overflow-x: hidden; }
.journal-wrap canvas { display: block; cursor: pointer; }
.journal-caption { margin: 6px 0 0; font-size: 11px; min-height: 14px; }
.journal-controls { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; margin-top: 6px; font-size: 12px; }
.journal-controls select { flex: 1 1 100%; min-width: 0; }
.journal-armed { flex-basis: 100%; }
.journal-none { margin: 0; font-size: 12px; }

```

- [ ] **Step 11: Register the panel.** In `web/ui/src/fs/panels.ts` add after `import ExtFormatForm from "./ext/ExtFormatForm.svelte";`:

```ts
import JournalPanel from "./ext/JournalPanel.svelte";
```

replace the doc comment's last four lines

```ts
 * family's further panels, rendered under the map in order (FAT has none; ext's Journal panel
 * goes here). They live here rather than on the adapter because `vitest.config.ts` has no Svelte
 * plugin: an adapter that imported a `.svelte` file could not be loaded by the node tests.
 */
```

with

```ts
 * family's further panels, rendered under the map in order (FAT has none; ext has the Journal
 * panel, which on ext2 says there is no journal). They live here rather than on the adapter
 * because `vitest.config.ts` has no Svelte plugin: an adapter that imported a `.svelte` file
 * could not be loaded by the node tests.
 */
```

and replace the ext entry with:

```ts
  ext: { map: BlockGroupMap, format: ExtFormatForm, extras: [JournalPanel] },
```

- [ ] **Step 12: Run the layout guards and watch them pass.**

```sh
pnpm exec vitest run tests/layout.test.ts
```

Expected: `Tests  7 passed (7)`.

#### Gates, browser, and commit

- [ ] **Step 13: Run the web gates.**

```sh
pnpm test && pnpm build
```

Expected:

- `pnpm test` prints `Test Files  45 passed (45)` and `Tests  422 passed (422)`.
- `pnpm build` prints svelte-check's `COMPLETED ... 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS` and vite's `✓ built in ...`.
- Afterwards, `git status --short` lists only the files in **Files**: no `pnpm-lock.yaml` and no `pnpm-workspace.yaml`.

- [ ] **Step 14: Check it in the browser.** In `web/ui`, run `pnpm exec vite --port <free port> --strictPort` and open the page. Check each of the following:
  1. On the FAT16 default, the left column is Files, `FAT map · 8,167 clusters`, then Actions. There is no journal panel.
  2. In Format details, choose Filesystem `ext` and Variant `ext3`, then click Format disk. The left column is Files, the block-group map, then the Journal panel. The panel shows:
     - `Journal · ordered mode`, with facts `sequence 1`, `head 1`, `start 0`, `blocks 1,024`.
     - A 198 × 135 px ring (39 columns × 27 rows). Only cell 0, the journal superblock, is boot grey; every other cell is hairline.
     - The phase select on its own row, reading `after commit` (options `before commit`, `after commit`, `during checkpoint`), then `Arm` and a disabled `Recover`.
  3. Add file. The facts become `sequence 2`, `head 10`, `start 0`. Cells 1..9 are drawn at 35 % alpha: the descriptor in violet, seven copies in ink, and the commit in blue.
     - Hover captions: cell 0 `journal block 0 (block 82) · superblock`; cell 1 `journal block 1 (block 83) · descriptor · transaction 1 · stale`; cell 2 `journal block 2 (block 84) · copy · copy of block 1 · transaction 1 · stale`; cell 8 `... copy of block 69 ...`; cell 10 `journal block 10 (block 92) · unused`.
     - Leaving the canvas blanks the caption.
     - Clicking cell 8 moves the dump's cursor to 0x16800 (Inspector `Block 90 · journal`).
  4. Click Arm with `after commit` selected. The select gives way to `Armed: the next change stops after commit` and the button reads `Disarm`, still focused (Tab moves on to Recover). Then add a second file:
     - The line `Needs recovery: the journal holds an unfinished transaction.` appears, and the status line shows `Volume needs recovery`.
     - `start` becomes 10.
     - Cells 10..18 (transaction 2) are drawn at full colour; cell 18 captions `journal block 18 (block 100) · commit · transaction 2 · live`.
     - The armed line is gone, because the crash used it.
     - `Recover` is enabled.
  5. Click Recover:
     - The flag line and the status line's `Volume needs recovery` vanish.
     - Every used cell is at 35 % alpha again, and `start` is 0.
     - The timeline gains a `recover` step, with events `recovery_scanned ...`, `replayed ...`, `journal_emptied ...`, and `recovery_flag_cleared ...`.
     - The second file is in the tree, and `Recover` is disabled.
  6. Pick `before commit`, click Arm, then Disarm; the select comes back reading `before commit`. Open the terminal and run `crash --at during-checkpoint`: the panel shows `Armed: the next change stops during checkpoint` and `Disarm` at once; `crash --off` brings the select and `Arm` back. Arm `before commit` again from the panel and add a file:
     - The volume needs recovery, with a live descriptor and seven live copies (cells 1..8) and no live commit.
     - Recover discards the transaction (`transaction_discarded discarded uncommitted transaction ...`), and the file is not in the tree.
  7. Click Prev on the timeline. The panel shows `Shows the latest state, not the step you are viewing.`, and the select, Arm, and Recover are disabled. Next restores them.
  8. Check the other formats:
     - Format ext2: the panel is the single muted line `This volume has no journal.`
     - Format ext3 with Journal mode `data`: the panel shows `Journal · data mode`, an empty ring, and a blank caption.
     - Format FAT16: the journal panel is gone (`FAT map · 8,167 clusters`, then Actions), and the console has no errors.

- [ ] **Step 15: Commit.** From the repo root:

```sh
git add web/ui/src/app.css web/ui/src/fs/panels.ts web/ui/src/fs/ext/JournalPanel.svelte web/ui/src/fs/ext/journalRing.ts web/ui/tests/fs/ext.test.ts web/ui/tests/layout.test.ts
git commit -m "feat(ui): the Journal panel with crash and recovery controls"
```

Facts later tasks rely on (pinned by this task's tests or visible in its markup):

- Strings:
  - `This volume has no journal.`
  - `Journal · ${mode} mode`
  - Facts `sequence`, `head`, `start`, `blocks`, with `toLocaleString()` values.
  - `Needs recovery: the journal holds an unfinished transaction.`
  - `Armed: the next change stops ${CRASH_PHASE_LABELS[phase]}`
  - Ring caption: `journal block ${index} (block ${block}) · ${kind}` + ` · copy of block ${home}` + ` · transaction ${tid} · stale|live`.
- DOM:
  - `<section class="panel journal">`.
  - `<select id="journal-phase" aria-label="Crash phase">`, with values `before_commit`, `after_commit` (default), and `during_checkpoint`.
  - One button reading `Arm` or `Disarm` (by `volume.armedPhase`), a `Recover` button, and `<canvas aria-label="Journal ring">`.
  - Classes `journal-flag`, `journal-wrap`, `journal-caption`, `journal-controls`, `journal-armed`, `journal-none`.
- Ring geometry: Task 2's grid with the block-group map's 4 px cells, 1 px gap, and 260 px scroll box. On the default disk the ring is 39 columns × 27 rows (135 px) in the 198 px column.
- Armed-line timing: the armed line is `volume.armedPhase`, store state that the panel's button, Task 6's `crash` (through the store host), and `refreshMeta` (after every op, format, and load) all update.
  - A `crash` armed or disarmed from the terminal (Task 6) shows in the panel immediately.
  - A lesson step that arms through the capability directly and creates in one action (Task 7) shows no armed line, because the create uses the phase up and `refreshMeta` reads `null` after it.
- Ring positions on the default disk:
  - A create after a clean format writes transaction 1 at journal indices 1..9 (blocks 83..91).
  - A crash after commit on the next create leaves transaction 2 live at indices 10..18 (blocks 92..100).

---

### Task 6: the shell (`crash`, `recover`, `mkfs --type`, re-registration)

Spec: `docs/superpowers/specs/2026-09-24-ext-explorer-design.md` sections 2 (the composed `mkfs` done line `formatted /dev/hda as ${host.vol.fsType()}; the timeline was cleared`; `AddrSpace` with `parseAddr`/`extraAddrHelp`), 6 in full (`crash`, `recover`, `mkfs --type`, `df`/`stat`/`seek` on ext, re-registration), 8 (the `tests/shell/*` bullet), 10 (`mkfs --type` chooses the family and variant; the terminal re-registers on family change because summaries and flag lists are captured at registration), and the Outcome bullet that sanctions the one FAT pin change (`--label` in `tests/shell/help-text.test.ts` becomes `volume label (fat16: up to 11 characters; ext: up to 16 bytes)`).

The terminal learns the journal and the second family. `crash` and `recover` live in a new `src/shell/journalCommands.ts` and exist only while the mounted adapter has a journal capability (ext3). `mkfs` becomes one command for every family: `--type fat16|ext2|ext3` (default: the mounted volume's own type), a flag list that is the union of every family's `mkfs.flags` with shared descriptions merged, and a named error for a flag the chosen type does not take. `TerminalPanel.svelte` swaps the whole registered command set whenever the mounted family or its journal changes, over the same `Vfs`, so the working directory survives. `df`, `stat`, `seek`, `write --at`, and `xxd --offset` already speak ext after Tasks 2 and 3; this task pins them on an ext3 host.

`crash` arms through a new `ShellHost.setArmedPhase(phase | null)`, inside `fsCall` like every wasm-backed call. The app's store host implements it with Task 2's `volume.setArmedPhase`, which keeps the reactive `volume.armedPhase`, so a phase armed in the terminal shows in the Journal panel (Task 5) at once, and the reverse; the test host arms the capability directly and records each call. The name is the store's, not the wasm's `armCrash`/`disarmCrash`, so the boundary scan (`tests/adapterBoundary.test.ts`) needs no exemption: `.armCrash(` and `.disarmCrash(` stay flagged everywhere outside `src/fs/ext/`, and this task does not touch the scan.

FAT-visible changes, all mandated by spec section 6 (and none pinned by an existing test except the one sanctioned `--label` pin):

- `help mkfs` on a FAT host: the summary changes from `Format /dev/hda as FAT16 (clears the timeline)` to `Format /dev/hda (clears the timeline); --type picks fat16, ext2, or ext3`.
- `mkfs` gains `--type` and ext's five other flags (`--blocks`, `--inodes-per-group`, `--uuid`, `--journal-blocks`, `--journal-mode`), and `--label`'s description becomes the merged `volume label (fat16: up to 11 characters; ext: up to 16 bytes)`: the one sanctioned FAT pin change, in `tests/shell/help-text.test.ts`.
- An ext flag given to a FAT format (`mkfs --blocks 4096` on the FAT16 default) is `--blocks is not a fat16 option` instead of browser-terminal's unknown-flag error.

Every other FAT output (`mkfs`'s FAT flags and done line, every other command) is unchanged. Since `mkfs` no longer reads a family's summary, `MkfsSpec.summary` leaves the seam and both families: `tests/fs/fat16-format.test.ts`'s pin of FAT's summary string becomes a `not.toHaveProperty("summary")` check beside the one for the removed `done` (a shape-only edit, like Task 2's), and so does ext's in `tests/fs/ext-format.test.ts`. `tests/shell/errors.test.ts`'s `ShellHost` stub gains a no-op `setArmedPhase`, the shape-only edit the new required member forces.

The test counts in the **Expected** lines assume Tasks 1 to 5 were applied in order first (with Task 2's `tests/grid.test.ts`, `tests/freeSpace.test.ts`, and `tests/scrollNonce.test.ts`): 45 files and 422 tests before this task, 46 files and 447 tests after it.

Choices made here that the spec's text did not settle, all pinned by tests:

- The foreign-flag error picks its article from the type: `--blocks is not a fat16 option`, `--spc is not an ext3 option` (`an` before a vowel letter; the spec's template wrote `a ${type}`).
- The re-registration key is the family plus the journal's presence (`commandSetOf(adapter)`: `fat16`, `ext`, `ext+journal`), not the family id alone, so `mkfs --type ext2` on an ext3 volume removes `crash` and `recover` (the id stays `ext` across that change). A definition that outlives its journal anyway (a test host, or a race) throws `/dev/hda has no journal (ext2)`.
- `rewoundWarning` and `warnIfRewound` move from `shell/commands.ts` to `shell/host.ts` (re-exported from `commands.ts` unchanged) so `journalCommands.ts` can use them without an import cycle.
- The registration loop becomes `registerCommands(term, host, vfs, previous)` in `commands.ts` so the node tests can drive it with a fake terminal; `TerminalPanel.svelte` calls it at creation and in the family effect.
- `mkfs`'s summary is the shell's, so `MkfsSpec` is just `{ flags }` (above). A family with several `fsTypes` gets the chosen type as `options.variant`; `--type` is matched case-insensitively (`--type FAT16` works).
- The address cases (`seek b:69` / `seek i:12`, `xxd --offset b:1111` / `i:12`, `write --at b:2000` / `i:20`) go through the commands in `tests/shell/read-commands.test.ts`; `tests/shell/addr.test.ts` already covers the parser forms and is not touched.
- `PHASE_HELP` stays private to `journalCommands.ts` (the tests pin its text through the error's `help`); `orList`, `mkfsTypes`, `mkfsTypeOf`, and `mkfsFlagsFor` stay exported and get a direct test in `tests/shell/mutations.test.ts`.

**Files**

- Create: `web/ui/src/shell/journalCommands.ts` (1–75)
- Modify: `web/ui/src/shell/commands.ts` (1 types import, 3–4 `FAMILIES`/seam imports, 10–11 host/journal imports, 18 re-export; old 24–32 `rewoundWarning`/`warnIfRewound` removed; 56–97 `orList`, `mkfsTypes`, `mkfsTypeOf`, `mkfsFlagsFor`; 617–660 `mkfs`; 764–801 `createCommands`, `commandSetOf`, `CommandRegistry`, `registerCommands`), `web/ui/src/shell/host.ts` (2–3, 26–29 `setArmedPhase`, 43–50), `web/ui/src/shell/storeHost.svelte.ts` (2, 12–14, 44–46), `web/ui/src/components/TerminalPanel.svelte` (12, 30–33, 81–86, 141–159, 164–165), `web/ui/src/fs/adapter.ts` (111–115 `MkfsSpec`), `web/ui/src/fs/fat16/format.ts` (57–61), `web/ui/src/fs/ext/format.ts` (81–86)
- Test: `web/ui/tests/shell/journal.test.ts` (1–216, new), `web/ui/tests/shell/mutations.test.ts` (2–4, 276–344), `web/ui/tests/shell/help-text.test.ts` (2, 9–10, 36, 41, 46–77), `web/ui/tests/shell/read-commands.test.ts` (2, 259–312), `web/ui/tests/shell/helpers.ts` (3, 24–25, 57–64), `web/ui/tests/shell/errors.test.ts` (66), `web/ui/tests/fs/fat16-format.test.ts` (43–44), `web/ui/tests/fs/ext-format.test.ts` (91)
- Not touched: anything under `web/ui/src/fs/` beyond the three `MkfsSpec` edits, `state/volume.svelte.ts` (Task 2 gave it `setArmedPhase`), `app.css`, every component other than `TerminalPanel.svelte`, `tests/shell/addr.test.ts`, `tests/adapterBoundary.test.ts`. The only `.recover(` outside `src/fs/ext/` is `journal.recover()`, on a receiver named `journal`, which the scan already allows, and nothing outside `src/fs/ext/` calls `.armCrash(` or `.disarmCrash(`.

**Interfaces**

Consumes: Task 2's `fs/adapter.ts` (`FsAdapter` with `journal?: JournalCapability`, `needsRecovery`, `extraAddrHelp?`; `FsFamily` with `fsTypes` and `mkfs: MkfsSpec`; `MkfsFlag { long; desc; kind: "int" | "str"; option }`; `CrashPhase`, `CRASH_PHASES`, `CRASH_PHASE_LABELS`, `JournalCapability`), `shell/addr.ts` (`parseAddr`, `addrHelp` already handle `b:N`, `s:N`, `i:N`), the composed `mkfs` done line. Task 3's `ext` family (`fsTypes ["ext2", "ext3"]`; `MKFS.flags` `blocks`, `inodes-per-group`, `label`, `uuid`, `journal-blocks`, `journal-mode`; `toWasmOptions` throws `Error("journalBlocks and journalMode need variant ext3")`; `ExtFamilyOptions.variant`), reached only through `FAMILIES` and the host's adapter. Task 4's `FAMILIES = { fat16, ext }` and `FsFamilyId = "fat16" | "ext"`. Task 2's `volume.setArmedPhase(phase: CrashPhase | null): boolean` (null disarms; false, with `volume.status` set, when the volume refuses). browser-terminal 0.3.0's `registerCommand(spec, fn)` and `unregisterCommand(name)`. `tests/shell/helpers.ts`'s `TestHost` (`format(family, options)` re-binds the adapter and records `formats`), `makeHost`, `call`, `callErr`.

Produces (exact):

```ts
// shell/journalCommands.ts (PHASE_HELP = "phases: before-commit, after-commit, during-checkpoint" is module-private)
export function journalCommands(host: ShellHost): CommandDef[];   // [crash, recover]; crash arms with fsCall("/dev/hda", () => host.setArmedPhase(phase | null))

// shell/host.ts
export interface ShellHost { /* ...unchanged... */ setArmedPhase(phase: CrashPhase | null): void }   // null disarms; not an op; throws { message, code? }
// shell/storeHost.svelte.ts: setArmedPhase -> volume.setArmedPhase(phase); a false return throws statusToError(volume.status)
// tests/shell/helpers.ts: TestHost.setArmedPhase arms the capability directly and pushes the phase (null for a disarm) onto TestHost.crashes
// fs/adapter.ts: export interface MkfsSpec { flags: MkfsFlag[] }   // summary removed; fat16's and ext's MKFS are { flags }
// shell/host.ts (moved from commands.ts; commands.ts re-exports both)
export function rewoundWarning(host: ShellHost): string;
export function warnIfRewound(host: ShellHost, ctx: CommandCtx): void;

// shell/commands.ts
export function orList(items: readonly string[]): string;   // "fat16, ext2, or ext3"; "a or b"
export function mkfsTypes(families: Readonly<Record<string, FsFamily>>): string[];   // ["fat16", "ext2", "ext3"]
export function mkfsTypeOf(host: ShellHost): string;   // host.vol.fsType().toLowerCase()
export function mkfsFlagsFor(families: Readonly<Record<string, FsFamily>>): Omit<MkfsFlag, "option">[];
export function createCommands(host: ShellHost, vfs?: Vfs): CommandDef[];   // crash, recover appended last iff host.adapter.journal
export function commandSetOf(adapter: FsAdapter): string;   // "fat16" | "ext" | "ext+journal"
export interface CommandRegistry { registerCommand(spec: CommandSpec, fn: CommandDef["fn"]): void; unregisterCommand(name: string): void }
export function registerCommands(term: CommandRegistry, host: ShellHost, vfs: Vfs, previous?: readonly string[]): string[];
export { rewoundWarning, selectPath, warnIfRewound } from "./host";
```

Strings (all pinned by this task's tests):

- `crash` summary `Arm a crash in the journal: the next change to /dev/hda stops mid-transaction`; flags `--at <str>` `where the change stops: before-commit, after-commit (default), during-checkpoint` and `--off` `disarm the armed crash`; output `armed: the next change to /dev/hda stops before commit|after commit|during checkpoint` or `disarmed`; errors `unknown crash phase '${at}'` (help `phases: before-commit, after-commit, during-checkpoint`) and `give --at or --off, not both`; the rewound warning on `ctx.err` while rewound.
- `recover` summary `Replay or discard the journal's unfinished transaction`; one `ctx.log` line per event `text` of the `recover` record, which lands in the timeline as op `recover`.
- Both on a volume without a journal: `/dev/hda has no journal (${fsType})`, help `crash and recover need an ext3 volume: mkfs --type ext3`.
- `mkfs` summary `Format /dev/hda (clears the timeline); --type picks fat16, ext2, or ext3`; `--type` desc `fat16, ext2, or ext3 (default: the mounted volume's type)`; `--label` desc `volume label (fat16: up to 11 characters; ext: up to 16 bytes)`; errors `--${long} is not a fat16 option` / `is not an ext2 option` / `is not an ext3 option`, `unknown type '${type}'` (help `types: fat16, ext2, ext3`), and the family's `Error` through `fsCall` as `/dev/hda: journalBlocks and journalMode need variant ext3`.

Environment for every command below, from `web/ui`: any pnpm 10.26 or newer (nvm's pnpm 12 and Homebrew's 10.33 both work). Both web packages keep their pnpm settings in a tracked `pnpm-workspace.yaml`, so a frozen install must leave `web/ui/pnpm-lock.yaml` and `web/ui/pnpm-workspace.yaml` unmodified; if either shows as modified in `git status` at any point, restore it with `git checkout web/ui/pnpm-lock.yaml web/ui/pnpm-workspace.yaml` and run `CI=true pnpm install --frozen-lockfile` again (`CI=true` lets pnpm purge a stale modules directory without asking). On a fresh checkout build the wasm package first (repo root: `wasm-pack build crates/wasm --target bundler`), then `CI=true pnpm install --frozen-lockfile` in `web/ui`. Baseline after Tasks 1 to 5: `pnpm test` prints `Test Files  45 passed (45)` and `Tests  422 passed (422)`.

#### Round 1: `crash` and `recover`

- [ ] **Step 1: Write the failing journal tests.** Create `web/ui/tests/shell/journal.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { createCommands } from "../../src/shell/commands";
import { journalCommands } from "../../src/shell/journalCommands";
import { Vfs } from "../../src/shell/vfs";
import { call, callErr, makeHost } from "./helpers";

const enc = (s: string) => new TextEncoder().encode(s);
const names = (defs: { spec: { name: string } }[]) => defs.map((d) => d.spec.name);

/** A fresh default ext3 disk (16,384 blocks, journal at 82..1105) behind a test host. */
function setup() {
  const host = makeHost(Volume.formatExt3(undefined));
  const vfs = new Vfs();
  return { host, vfs, defs: createCommands(host, vfs) };
}

describe("which hosts get crash and recover", () => {
  it("registers both on an ext3 host, after every other command", () => {
    const { defs } = setup();
    expect(names(defs).slice(-2)).toEqual(["crash", "recover"]);
  });

  it("registers neither on a FAT16 host or an ext2 host", () => {
    for (const vol of [Volume.formatFat16(undefined), Volume.formatExt2(undefined)]) {
      const defs = names(createCommands(makeHost(vol)));
      expect(defs, vol.fsType()).not.toContain("crash");
      expect(defs, vol.fsType()).not.toContain("recover");
    }
  });

  it("words the two summaries and crash's flags", () => {
    const [crash, recover] = journalCommands(setup().host).map((d) => d.spec);
    expect(crash.summary).toBe("Arm a crash in the journal: the next change to /dev/hda stops mid-transaction");
    expect(crash.flags).toEqual([
      { long: "at", shape: "str", desc: "where the change stops: before-commit, after-commit (default), during-checkpoint" },
      { long: "off", desc: "disarm the armed crash" },
    ]);
    expect(recover.summary).toBe("Replay or discard the journal's unfinished transaction");
  });
});

describe("crash", () => {
  it("arms after-commit by default, each --at phase by name, and --off disarms", async () => {
    const { host, defs } = setup();
    const journal = host.adapter.journal!;
    const armed = await call(defs, "crash");
    expect(armed.log).toEqual(["armed: the next change to /dev/hda stops after commit"]);
    expect(armed.value).toBeUndefined();
    expect(journal.phase()).toBe("after_commit");
    expect((await call(defs, "crash", { flags: { at: "before-commit" } })).log).toEqual(["armed: the next change to /dev/hda stops before commit"]);
    expect(journal.phase()).toBe("before_commit");
    expect((await call(defs, "crash", { flags: { at: "during-checkpoint" } })).log).toEqual(["armed: the next change to /dev/hda stops during checkpoint"]);
    expect(journal.phase()).toBe("during_checkpoint");
    expect((await call(defs, "crash", { flags: { at: "after-commit" } })).log).toEqual(["armed: the next change to /dev/hda stops after commit"]);
    expect(journal.phase()).toBe("after_commit");
    expect((await call(defs, "crash", { flags: { off: true } })).log).toEqual(["disarmed"]);
    expect(journal.phase()).toBeNull();
    expect(host.historyLength).toBe(0); // arming is not a recorded operation
    // Every arm and disarm went through the host (the store, in the app, so the Journal panel
    // shows it at once), not straight to the capability.
    expect(host.crashes).toEqual(["after_commit", "before_commit", "during_checkpoint", "after_commit", null]);
  });

  it("refuses an unknown phase, and --at with --off, leaving the journal alone", async () => {
    const { host, defs } = setup();
    for (const at of ["after_commit", "later", ""]) {
      const e = await callErr(defs, "crash", { flags: { at } });
      expect(e.message).toBe(`unknown crash phase '${at}'`);
      expect(e.help).toBe("phases: before-commit, after-commit, during-checkpoint");
    }
    expect((await callErr(defs, "crash", { flags: { at: "before-commit", off: true } })).message).toBe("give --at or --off, not both");
    expect(host.adapter.journal!.phase()).toBeNull();
  });

  it("arms while the timeline is rewound, with the rewound warning", async () => {
    const { host, defs } = setup();
    host.run((v) => v.createFile("/a.txt", enc("a")));
    host.run((v) => v.createFile("/b.txt", enc("b")));
    host.rewind(0);
    const r = await call(defs, "crash", { flags: { at: "before-commit" } });
    expect(r.err).toEqual(['showing the latest state, not step 1 of 2; click "Back to now" or run a write command']);
    expect(r.log).toEqual(["armed: the next change to /dev/hda stops before commit"]);
    expect(host.adapter.journal!.phase()).toBe("before_commit");
    expect((await call(defs, "crash", { flags: { off: true } })).err).toHaveLength(1);
    host.rewind(1);
    expect((await call(defs, "crash")).err).toEqual([]);
  });
});

describe("recover", () => {
  it("replays a transaction that crashed after its commit, one line per event, as a timeline step", async () => {
    const { host, defs } = setup();
    await call(defs, "crash");
    await call(defs, "write", { positionals: ["/mnt/x.txt"] }, "hi\n");
    expect(host.history.at(-1)!.events.at(-1)!.text).toBe("crashed after commit");
    expect(host.adapter.needsRecovery).toBe(true);
    expect(host.adapter.journal!.phase()).toBeNull(); // the crash used the armed phase up
    const r = await call(defs, "recover");
    expect(r.value).toBeUndefined();
    expect(r.log).toEqual([
      "scanned the journal from block 1, sequence 1: 1 committed transaction, 7 tagged blocks",
      "replayed block 1 from journal block 2 (transaction 1)",
      "replayed block 2 from journal block 3 (transaction 1)",
      "replayed block 3 from journal block 4 (transaction 1)",
      "replayed block 4 from journal block 5 (transaction 1)",
      "replayed block 5 from journal block 6 (transaction 1)",
      "replayed block 6 from journal block 7 (transaction 1)",
      "replayed block 69 from journal block 8 (transaction 1)",
      "journal emptied; next transaction 3",
      "cleared needs_recovery in the superblock",
    ]);
    expect(host.history.map((h) => h.op)).toEqual(["create_file /x.txt (crashed after commit)", "recover"]);
    expect(host.cursor).toBe(1);
    expect(host.adapter.needsRecovery).toBe(false);
    expect(host.vol.listDir("/").map((e) => e.name)).toEqual(["lost+found", "x.txt"]);
  });

  it("discards a transaction that crashed before its commit", async () => {
    const { host, defs } = setup();
    await call(defs, "crash", { flags: { at: "before-commit" } });
    await call(defs, "write", { positionals: ["/mnt/x.txt"] }, "hi\n");
    expect((await call(defs, "recover")).log).toEqual([
      "scanned the journal from block 1, sequence 1: 0 committed transactions, 0 tagged blocks",
      "discarded uncommitted transaction 1 (7 tagged blocks)",
      "journal emptied; next transaction 2",
      "cleared needs_recovery in the superblock",
    ]);
    expect(host.vol.listDir("/").map((e) => e.name)).toEqual(["lost+found"]);
  });

  it("prints the single line of a clean journal, and still records the step", async () => {
    const { host, defs } = setup();
    expect((await call(defs, "recover")).log).toEqual(["journal is clean; nothing to replay"]);
    expect(host.history.map((h) => h.op)).toEqual(["recover"]);
  });

  it("goes through fsCall: a change on a volume that needs recovery is refused with the wasm code", async () => {
    const { defs } = setup();
    await call(defs, "crash");
    await call(defs, "write", { positionals: ["/mnt/x.txt"] }, "hi\n");
    const e = await callErr(defs, "mkdir", { positionals: ["/mnt/D"] });
    expect(e.code).toBe("NeedsRecovery");
    expect(e.message).toBe("/mnt/D: needs recovery");
    await call(defs, "recover");
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
  });

  it("names the missing journal when the mounted volume lost it after registration", async () => {
    const { host, defs } = setup();
    host.format("ext", { variant: "ext2" }); // the app re-registers here; a stale def still explains itself
    for (const name of ["crash", "recover"]) {
      const e = await callErr(defs, name);
      expect(e.message, name).toBe("/dev/hda has no journal (ext2)");
      expect(e.help, name).toBe("crash and recover need an ext3 volume: mkfs --type ext3");
    }
  });
});
```

- [ ] **Step 2: Run it and watch it fail.**

```sh
pnpm exec vitest run tests/shell/journal.test.ts
```

Expected: `Test Files  1 failed (1)`, `Tests  no tests`, with `Error: Cannot find module '../../src/shell/journalCommands' imported from '.../web/ui/tests/shell/journal.test.ts'`.

- [ ] **Step 3: Move the rewound warning into `host.ts`, and let the host arm a crash.** `journalCommands.ts` needs the warning, and `commands.ts` will import `journalCommands.ts`, so the helper moves below the import cycle. In `web/ui/src/shell/host.ts` replace `import type { FsAdapter, FsFamilyId } from "../fs/adapter";` with

```ts
import type { CrashPhase, FsAdapter, FsFamilyId } from "../fs/adapter";
```

add the type import after it:

```ts
import type { CommandCtx } from "./types";
```

and add right after `export const atLatest = ...;`:

```ts

/** The one warning every read command emits while the timeline is rewound. */
export function rewoundWarning(host: ShellHost): string {
  return `showing the latest state, not step ${host.cursor + 1} of ${host.historyLength}; click "Back to now" or run a write command`;
}

export function warnIfRewound(host: ShellHost, ctx: CommandCtx): void {
  if (!atLatest(host)) ctx.err(rewoundWarning(host));
}
```

In `web/ui/src/shell/commands.ts` delete the two functions (the block from `/** The one warning every read command emits while the timeline is rewound. */` through the closing brace of `warnIfRewound`, and the blank line after it), then change the imports and the re-export. Replace

```ts
import type { CommandArgs, CommandCtx, CommandDef, CommandSpec, FlagSpec, PosArg } from "./types";
```

with

```ts
import type { CommandArgs, CommandDef, CommandSpec, FlagSpec, PosArg } from "./types";
```

replace

```ts
import { atLatest, corruptionOf, selectPath, type ShellHost } from "./host";
```

with

```ts
import { corruptionOf, selectPath, warnIfRewound, type ShellHost } from "./host";
import { journalCommands } from "./journalCommands";
```

and replace

```ts
export { selectPath } from "./host";
```

with

```ts
export { rewoundWarning, selectPath, warnIfRewound } from "./host";
```

(`tests/shell/read-commands.test.ts` keeps importing `rewoundWarning` from `commands.ts`.)

Arming goes through the host, so the app can arm through the store (Task 2's `volume.setArmedPhase`), where the Journal panel sees it. In `ShellHost` add after `format(family: FsFamilyId, options?: unknown): void;`:

```ts
  /** Arm a crash in the mounted journal (`null` disarms). Not an op: no timeline step. The app
   *  arms through the store (`volume.setArmedPhase`), so the Journal panel shows the phase at
   *  once. Throws `{ message, code? }` on failure. */
  setArmedPhase(phase: CrashPhase | null): void;
```

In `web/ui/src/shell/storeHost.svelte.ts` replace `import type { FsAdapter, FsFamilyId } from "../fs/adapter";` with

```ts
import type { CrashPhase, FsAdapter, FsFamilyId } from "../fs/adapter";
```

replace the doc comment's

```ts
 * `run` and `format` mirror the Actions panel: VolumeStore never throws, it reports
 * failure through `status` (and `run` returns null). The host turns that status back
```

with

```ts
 * `run` and `format` mirror the Actions panel, and `setArmedPhase` the Journal panel:
 * VolumeStore never throws, it reports failure through `status` (and `run` returns null,
 * `setArmedPhase` false). The host turns that status back
```

and add after the `format` method's closing `},`:

```ts
    setArmedPhase(phase: CrashPhase | null): void {
      if (!volume.setArmedPhase(phase)) throw statusToError(volume.status);
    },
```

In `web/ui/tests/shell/helpers.ts` replace `import type { FsAdapter, FsFamilyId } from "../../src/fs/adapter";` with

```ts
import type { CrashPhase, FsAdapter, FsFamilyId } from "../../src/fs/adapter";
```

add after the `formats` field:

```ts
  /** Every `setArmedPhase` call that succeeded, in order (`null` for a disarm). */
  crashes: (CrashPhase | null)[] = [];
```

and after the `format` method:

```ts

  /** Arms the capability directly (the app goes through the store, which does the same and
   *  also records the phase for the Journal panel); nothing to arm without a journal. */
  setArmedPhase(phase: CrashPhase | null): void {
    const journal = this.adapter.journal;
    if (phase === null) journal?.disarm();
    else journal?.arm(phase);
    this.crashes.push(phase);
  }
```

`tests/shell/errors.test.ts` builds a `ShellHost` literal in `describe("host helpers", ...)`; the new required member forces a shape-only edit there. Replace its `format: () => {}, select: () => {},` with `format: () => {}, setArmedPhase: () => {}, select: () => {},` (the line becomes `run: () => { throw new Error("unused"); }, format: () => {}, setArmedPhase: () => {}, select: () => {}, jumpTo: () => {}, setPrompt: () => {}, closeTerminal: () => {},`).

`tests/adapterBoundary.test.ts` stays as Task 3 left it: it flags any `.armCrash(` or `.disarmCrash(` outside `src/fs/ext/` as the wasm method, and nothing here calls one (the store host calls `volume.setArmedPhase`, the commands `host.setArmedPhase`, and the test host the capability's `arm`/`disarm`); its positive controls still flag `vol.armCrash(x)` and `vol.disarmCrash()`.

- [ ] **Step 4: Write the two commands.** Create `web/ui/src/shell/journalCommands.ts`:

```ts
import type { CommandDef } from "./types";
import { CRASH_PHASES, CRASH_PHASE_LABELS, type CrashPhase, type JournalCapability } from "../fs/adapter";
import { ShellError, fsCall } from "./errors";
import { warnIfRewound, type ShellHost } from "./host";

/** The shell's name for a crash phase: wasm's snake_case with hyphens (`after-commit`). */
const phaseName = (p: CrashPhase): string => p.replaceAll("_", "-");

const PHASE_HELP = `phases: ${CRASH_PHASES.map(phaseName).join(", ")}`;

/**
 * The mounted volume's journal, read on every call: the definitions outlive a format, and the
 * terminal re-registers only when the family or the journal's presence changes, so a stale
 * definition says what is missing instead of failing on `undefined`.
 */
function journalOf(host: ShellHost): JournalCapability {
  const journal = host.adapter.journal;
  if (!journal) {
    throw new ShellError(`/dev/hda has no journal (${host.vol.fsType()})`, { help: "crash and recover need an ext3 volume: mkfs --type ext3" });
  }
  return journal;
}

/**
 * `crash` and `recover`, the terminal's half of the Journal panel. `createCommands` includes
 * them only when the mounted adapter has a journal capability (ext3). Arming is not a recorded
 * operation, so `crash` works while the timeline is rewound (with the read commands' warning);
 * it arms through `host.setArmedPhase` (in the app, the store, so the Journal panel shows the phase
 * at once), inside `fsCall` like every wasm call. `recover` is the volume op and goes through
 * `host.run` like every other change.
 */
export function journalCommands(host: ShellHost): CommandDef[] {
  const crash: CommandDef = {
    spec: {
      name: "crash",
      summary: "Arm a crash in the journal: the next change to /dev/hda stops mid-transaction",
      flags: [
        {
          long: "at",
          shape: "str",
          desc: `where the change stops: ${CRASH_PHASES.map((p) => (p === "after_commit" ? `${phaseName(p)} (default)` : phaseName(p))).join(", ")}`,
        },
        { long: "off", desc: "disarm the armed crash" },
      ],
    },
    fn: (args, _input, ctx) => {
      journalOf(host); // a definition that outlived its journal says so
      const at = args.flags.at;
      const given = at !== undefined && at !== null;
      if (args.flags.off === true) {
        if (given) throw new ShellError("give --at or --off, not both");
        warnIfRewound(host, ctx);
        fsCall("/dev/hda", () => host.setArmedPhase(null));
        ctx.log("disarmed");
        return;
      }
      const phase = given ? CRASH_PHASES.find((p) => phaseName(p) === String(at)) : "after_commit";
      if (phase === undefined) throw new ShellError(`unknown crash phase '${String(at)}'`, { help: PHASE_HELP });
      warnIfRewound(host, ctx);
      fsCall("/dev/hda", () => host.setArmedPhase(phase));
      ctx.log(`armed: the next change to /dev/hda stops ${CRASH_PHASE_LABELS[phase]}`);
    },
  };

  const recover: CommandDef = {
    spec: { name: "recover", summary: "Replay or discard the journal's unfinished transaction" },
    fn: (_args, _input, ctx) => {
      const journal = journalOf(host);
      const rec = fsCall("/dev/hda", () => host.run(() => journal.recover()));
      for (const e of rec.events) ctx.log(e.text);
    },
  };

  return [crash, recover];
}
```

- [ ] **Step 5: Include them when the adapter has a journal.** In `web/ui/src/shell/commands.ts` replace `createCommands` and its doc comment with:

```ts
/**
 * Every command the terminal registers. `echo` is a browser-terminal builtin
 * and is not registered here. `vfs` holds the one working directory per page.
 * `crash` and `recover` come last, and only while the mounted adapter has a journal (ext3);
 * the terminal calls this again whenever that or the family changes.
 */
export function createCommands(host: ShellHost, vfs: Vfs = new Vfs()): CommandDef[] {
  const journal = host.adapter.journal ? journalCommands(host) : [];
  return [...readCommands(host, vfs), ...mutationCommands(host, vfs), ...ddXxdCommands(host, vfs), ...journal];
}
```

- [ ] **Step 6: Run the journal tests and watch them pass.**

```sh
pnpm exec vitest run tests/shell/journal.test.ts tests/shell/read-commands.test.ts
```

Expected: `Test Files  2 passed (2)`, `Tests  32 passed (32)` (11 journal, 21 read-commands).

#### Round 2: `mkfs --type`

- [ ] **Step 7: Write the failing `mkfs --type` tests.** In `web/ui/tests/shell/mutations.test.ts` add the wasm import as the second line:

```ts
import { Volume } from "../../src/lib/wasm";
```

replace `import { createCommands } from "../../src/shell/commands";` with

```ts
import { FAMILIES } from "../../src/fs";
import { createCommands, mkfsFlagsFor, mkfsTypeOf, mkfsTypes, orList } from "../../src/shell/commands";
```

and add this block right before `describe("format through the host", () => {`:

```ts
describe("mkfs --type", () => {
  it("formats ext3 from a FAT16 host: the ext family with its variant, the done line, cwd and prompt reset", async () => {
    const { host, vfs, defs } = setup();
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    vfs.cwd = "/mnt/D";
    const r = await call(defs, "mkfs", { flags: { type: "ext3", blocks: 4096, label: "shell" } });
    expect(r.log).toEqual(["formatted /dev/hda as ext3; the timeline was cleared"]);
    expect(host.formats).toEqual([{ family: "ext", options: { variant: "ext3", totalBlocks: 4096, label: "shell" } }]);
    expect(host.vol.fsType()).toBe("ext3");
    expect(host.vol.sectorCount()).toBe(4096);
    expect(host.adapter.id).toBe("ext");
    expect(host.adapter.journal).toBeDefined();
    expect(host.historyLength).toBe(0);
    expect(vfs.cwd).toBe("/mnt");
    expect(host.prompts).toEqual(["/mnt "]);
  });

  it("formats ext2 and back to fat16, naming each type in the done line", async () => {
    const { host, defs } = setup();
    expect((await call(defs, "mkfs", { flags: { type: "ext2" } })).log).toEqual(["formatted /dev/hda as ext2; the timeline was cleared"]);
    expect(host.adapter.journal).toBeUndefined();
    expect((await call(defs, "mkfs", { flags: { type: "FAT16", sectors: 8192, spc: 1 } })).log).toEqual(["formatted /dev/hda as FAT16; the timeline was cleared"]);
    expect(host.vol.geometry().totalSectors).toBe(8192);
    expect(host.formats).toEqual([
      { family: "ext", options: { variant: "ext2" } },
      { family: "fat16", options: { totalSectors: 8192, sectorsPerCluster: 1 } },
    ]);
  });

  it("defaults to the mounted volume's own type", async () => {
    const host = makeHost(Volume.formatExt3(undefined));
    const defs = createCommands(host, new Vfs());
    expect((await call(defs, "mkfs", { flags: { "journal-mode": "data" } })).log).toEqual(["formatted /dev/hda as ext3; the timeline was cleared"]);
    expect(host.formats).toEqual([{ family: "ext", options: { variant: "ext3", journalMode: "data" } }]);
    const ext2 = makeHost(Volume.formatExt2(undefined));
    await call(createCommands(ext2), "mkfs");
    expect(ext2.vol.fsType()).toBe("ext2");
    expect(ext2.formats).toEqual([{ family: "ext", options: { variant: "ext2" } }]);
  });

  it("refuses a flag the chosen type does not take, naming the type", async () => {
    const { host, defs } = setup();
    expect((await callErr(defs, "mkfs", { flags: { type: "fat16", blocks: 4096 } })).message).toBe("--blocks is not a fat16 option");
    expect((await callErr(defs, "mkfs", { flags: { blocks: 4096 } })).message).toBe("--blocks is not a fat16 option");
    expect((await callErr(defs, "mkfs", { flags: { type: "ext3", spc: 2 } })).message).toBe("--spc is not an ext3 option");
    expect((await callErr(defs, "mkfs", { flags: { type: "ext2", "root-entries": 64 } })).message).toBe("--root-entries is not an ext2 option");
    expect(host.formats).toEqual([]);
  });

  it("surfaces the family's own error through fsCall, and refuses an unknown type", async () => {
    const { host, defs } = setup();
    expect((await callErr(defs, "mkfs", { flags: { type: "ext2", "journal-mode": "data" } })).message).toBe("/dev/hda: journalBlocks and journalMode need variant ext3");
    expect((await callErr(defs, "mkfs", { flags: { type: "ext3", "journal-blocks": -1 } })).message).toBe("--journal-blocks must be a non-negative integer");
    const bad = await callErr(defs, "mkfs", { flags: { type: "ntfs" } });
    expect(bad.message).toBe("unknown type 'ntfs'");
    expect(bad.help).toBe("types: fat16, ext2, ext3");
    expect(host.vol.fsType()).toBe("FAT16");
  });

  it("builds its lists from the registry: the types, the default type, and the merged flags", () => {
    expect([orList(["a"]), orList(["a", "b"]), orList(["fat16", "ext2", "ext3"])]).toEqual(["a", "a or b", "fat16, ext2, or ext3"]);
    expect(mkfsTypes(FAMILIES)).toEqual(["fat16", "ext2", "ext3"]);
    expect(mkfsTypeOf(makeHost())).toBe("fat16");
    expect(mkfsTypeOf(makeHost(Volume.formatExt2(undefined)))).toBe("ext2");
    // One family keeps its own words; a flag two families share merges them.
    expect(mkfsFlagsFor({ fat16: FAMILIES.fat16 })).toEqual(FAMILIES.fat16.mkfs.flags.map(({ long, kind, desc }) => ({ long, kind, desc })));
    expect(mkfsFlagsFor(FAMILIES).find((f) => f.long === "label")).toEqual({ long: "label", kind: "str", desc: "volume label (fat16: up to 11 characters; ext: up to 16 bytes)" });
  });
});

```

The last test checks the four exported helpers directly: `orList`'s three shapes, the registry's types, the default type on a FAT16 and an ext2 host, and `mkfsFlagsFor` keeping one family's words and merging `--label`. The existing `describe("mkfs", ...)` block is unchanged: its FAT done line, `--sectors 5` geometry error, `--spc -1` message, and the `fsType()`-follows test must still pass.

- [ ] **Step 8: Update the help-text pins.** In `web/ui/tests/shell/help-text.test.ts` add the wasm import as the second line:

```ts
import { Volume } from "../../src/lib/wasm";
```

in the file's doc comment replace

```ts
 * `addrHelp`), and the six `mkfs` flag descriptions (`FsFamily.mkfs.flags`, fat16's own
 * words). The literals
```

with

```ts
 * `addrHelp`), and `mkfs`'s flags (every family's `FsFamily.mkfs.flags` in one list, with
 * `--label`, which fat16 and ext share, merged, plus `--type`). The literals
```

and replace the last test and the closing `});` of the describe (from `it("pins each of mkfs's six flag descriptions", () => {` to the end of the file) with the following. The `--label` line is the one sanctioned FAT expectation change (spec Outcome); the other five FAT descriptions are unchanged.

```ts
  it("pins each of FAT16's six mkfs flag descriptions, --label merged with ext's", () => {
    const flags = spec("mkfs").flags ?? [];
    const desc = (long: string) => flags.find((f) => f.long === long)?.desc;
    expect(desc("sectors")).toBe("total sectors (default 32768 = 16 MB)");
    expect(desc("spc")).toBe("sectors per cluster (default 4)");
    expect(desc("label")).toBe("volume label (fat16: up to 11 characters; ext: up to 16 bytes)");
    expect(desc("root-entries")).toBe("root directory entries (default 512)");
    expect(desc("fats")).toBe("FAT copies (default 2)");
    expect(desc("reserved")).toBe("reserved sectors (default 1)");
  });

  it("pins mkfs's summary, --type, and ext's flags, in registry order after --type", () => {
    const mkfs = spec("mkfs");
    expect(mkfs.summary).toBe("Format /dev/hda (clears the timeline); --type picks fat16, ext2, or ext3");
    expect(mkfs.flags).toEqual([
      { long: "type", shape: "str", desc: "fat16, ext2, or ext3 (default: the mounted volume's type)" },
      { long: "sectors", shape: "int", desc: "total sectors (default 32768 = 16 MB)" },
      { long: "spc", shape: "int", desc: "sectors per cluster (default 4)" },
      { long: "label", shape: "str", desc: "volume label (fat16: up to 11 characters; ext: up to 16 bytes)" },
      { long: "root-entries", shape: "int", desc: "root directory entries (default 512)" },
      { long: "fats", shape: "int", desc: "FAT copies (default 2)" },
      { long: "reserved", shape: "int", desc: "reserved sectors (default 1)" },
      { long: "blocks", shape: "int", desc: "total 1 KiB blocks (default 16384 = 16 MB)" },
      { long: "inodes-per-group", shape: "int", desc: "inodes per block group (default one per 16 KiB)" },
      { long: "uuid", shape: "str", desc: "32 hex digits, hyphens optional" },
      { long: "journal-blocks", shape: "int", desc: "journal size in blocks, ext3 only" },
      { long: "journal-mode", shape: "str", desc: "ordered or data, ext3 only" },
    ]);
    // The same list on any host: the flags are every family's, not the mounted one's.
    expect(createCommands(makeHost(Volume.formatExt3(undefined))).find((d) => d.spec.name === "mkfs")!.spec).toEqual(mkfs);
  });
});

describe("the help text on an ext3 host", () => {
  const defs = createCommands(makeHost(Volume.formatExt3(undefined)));
  const spec = (name: string) => defs.find((d) => d.spec.name === name)!.spec;

  it("names the block in df's summary and one address form per noun in write and xxd", () => {
    expect(spec("df").summary).toBe("Block usage of the mounted volume");
    expect(spec("write").flags?.find((f) => f.long === "at")?.desc).toBe("disk address for /dev/hda (addresses: 0x1f (hex), 512 (decimal), b:65 (block), i:11 (inode))");
    expect(spec("xxd").flags?.find((f) => f.long === "offset")?.desc).toBe("start address (addresses: 0x1f (hex), 512 (decimal), b:65 (block), i:11 (inode))");
  });
});
```

- [ ] **Step 9: Run them and watch them fail.**

```sh
pnpm exec vitest run tests/shell/help-text.test.ts tests/shell/mutations.test.ts
```

Expected: `Test Files  2 failed (2)`, `Tests  8 failed | 27 passed (35)`: the two `mkfs` help-text tests and the six `mkfs --type` tests fail (the helpers' test with `TypeError: (0 , orList) is not a function`); the ext3 help-text test already passes (Task 2 made `df`'s summary and the address help family-neutral; it pins that here).

- [ ] **Step 10: Add the union helpers.** In `web/ui/src/shell/commands.ts` add after the `import type { DateTime, EntryInfo, Volume } from "../lib/wasm";` line:

```ts
import { FAMILIES } from "../fs";
import type { FsFamily, MkfsFlag } from "../fs/adapter";
```

and add right before `/** "cluster" -> "Cluster": a summary that opens with the family's unit noun. */`:

```ts
/** `a, b, or c` (`a or b` for two): the way a summary lists choices. */
export function orList(items: readonly string[]): string {
  return items.length <= 2 ? items.join(" or ") : `${items.slice(0, -1).join(", ")}, or ${items.at(-1)}`;
}

/** `mkfs --type`'s values: every family's `fsTypes`, lower-cased, in registry order
 *  (`fat16`, `ext2`, `ext3`). */
export function mkfsTypes(families: Readonly<Record<string, FsFamily>>): string[] {
  return Object.values(families).flatMap((f) => f.fsTypes.map((t) => t.toLowerCase()));
}

/** `mkfs`'s default `--type`: the mounted volume's own type, lower-cased (`FAT16` is `fat16`). */
export function mkfsTypeOf(host: ShellHost): string {
  return host.vol.fsType().toLowerCase();
}

/**
 * Every family's `mkfs.flags` as one list, in registry order and deduplicated by `long`. A flag
 * two families share keeps the first one's kind and merges the descriptions, each split at its
 * first comma, as `${base} (fat16: ${restA}; ext: ${restB})`: `--label` reads
 * `volume label (fat16: up to 11 characters; ext: up to 16 bytes)`.
 */
export function mkfsFlagsFor(families: Readonly<Record<string, FsFamily>>): Omit<MkfsFlag, "option">[] {
  const split = (desc: string): [string, string] => {
    const i = desc.indexOf(",");
    return i < 0 ? [desc, ""] : [desc.slice(0, i), desc.slice(i + 1).trim()];
  };
  const byLong = new Map<string, { kind: MkfsFlag["kind"]; descs: { id: string; desc: string }[] }>();
  for (const family of Object.values(families)) {
    for (const f of family.mkfs.flags) {
      const seen = byLong.get(f.long);
      if (seen) seen.descs.push({ id: family.id, desc: f.desc });
      else byLong.set(f.long, { kind: f.kind, descs: [{ id: family.id, desc: f.desc }] });
    }
  }
  return [...byLong].map(([long, { kind, descs }]) => ({
    long,
    kind,
    desc: descs.length === 1 ? descs[0].desc : `${split(descs[0].desc)[0]} (${descs.map((d) => `${d.id}: ${split(d.desc)[1]}`).join("; ")})`,
  }));
}

```

- [ ] **Step 11: Rewrite `mkfs` over every family.** In `mutationCommands` replace the whole `mkfs` definition, from its comment line ``// The flags, their option keys, and the summary are the family's (`FsFamily.mkfs`); the`` down to the line `fsCall("/dev/hda", () => host.format(host.adapter.id, options));`, with the following (the three lines after it, the cwd reset, the prompt, and the done line, stay as they are):

```ts
  // One `mkfs` for every family: `--type` picks the family (and the variant, for a family with
  // several types), and the flag list is every family's, so the spec does not depend on the
  // family mounted at registration. The option keys come from the chosen family's own flags.
  // The done line names the type the new volume reports.
  const types = mkfsTypes(FAMILIES);
  const union = mkfsFlagsFor(FAMILIES);
  const mkfs: CommandDef = {
    spec: {
      name: "mkfs",
      summary: `Format /dev/hda (clears the timeline); --type picks ${orList(types)}`,
      flags: [
        F("type", `${orList(types)} (default: the mounted volume's type)`, { shape: "str" }),
        ...union.map((f) => F(f.long, f.desc, { shape: f.kind })),
      ],
    },
    fn: (args, _input, ctx) => {
      const typeFlag = args.flags.type;
      const type = flagGiven(typeFlag) ? String(typeFlag).toLowerCase() : mkfsTypeOf(host);
      const family = Object.values(FAMILIES).find((f) => f.fsTypes.some((t) => t.toLowerCase() === type));
      if (!family) throw new ShellError(`unknown type '${type}'`, { help: `types: ${types.join(", ")}` });
      const { flags } = family.mkfs;
      for (const f of union) {
        if (flagGiven(args.flags[f.long]) && !flags.some((g) => g.long === f.long)) {
          throw new ShellError(`--${f.long} is not ${/^[aeiou]/i.test(type) ? "an" : "a"} ${type} option`);
        }
      }
      // A family with several types (ext: ext2, ext3) takes the one asked for as its `variant`.
      const options: Record<string, number | string> = family.fsTypes.length > 1 ? { variant: type } : {};
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
      fsCall("/dev/hda", () => host.format(family.id, options));
```

After the edit the end of the command reads:

```ts
      fsCall("/dev/hda", () => host.format(family.id, options));
      vfs.cwd = "/mnt";
      host.setPrompt(promptFor(vfs.cwd));
      ctx.log(`formatted /dev/hda as ${host.vol.fsType()}; the timeline was cleared`);
    },
  };
```

Nothing reads a family's `mkfs.summary` any more, so it leaves the seam. In `web/ui/src/fs/adapter.ts` replace `MkfsSpec` and its doc comment with:

```ts
/** The shell's `mkfs` flags for one family. There is one `mkfs` for every family, so its summary
 *  is the shell's (it names every type `--type` takes), and so is its done line: `formatted
 *  /dev/hda as ${vol.fsType()}; the timeline was cleared`, composed from the new volume, so a
 *  family with two types (ext2, ext3) names the one it made. */
export interface MkfsSpec { flags: MkfsFlag[] }
```

In `web/ui/src/fs/fat16/format.ts` replace `MKFS` and its doc comment with:

```ts
/** The shell's `mkfs` for this family: the six flags. The summary and the done line are the
 *  shell's (one `mkfs` for every family; the done line from the new volume's `fsType()`). */
export const MKFS: MkfsSpec = {
  flags: MKFS_FLAGS,
};
```

and in `web/ui/src/fs/ext/format.ts`:

```ts
/** The shell's `mkfs` for this family: the six flags. The summary and the done line are the
 *  shell's (one `mkfs` for every family; the done line names the new volume's `fsType()`,
 *  `ext2` or `ext3`). */
export const MKFS: MkfsSpec = {
  flags: MKFS_FLAGS,
};
```

The two pins of the old summaries become shape checks. In `web/ui/tests/fs/fat16-format.test.ts` (`it("keeps the shell's mkfs strings", ...)`) replace

```ts
    expect(MKFS.summary).toBe("Format /dev/hda as FAT16 (clears the timeline)");
    expect(MKFS).not.toHaveProperty("done"); // the shell composes the done line from fsType() (tests/shell/mutations.test.ts)
```

with

```ts
    expect(MKFS).not.toHaveProperty("done"); // the shell composes the done line from fsType() (tests/shell/mutations.test.ts)
    expect(MKFS).not.toHaveProperty("summary"); // and the one summary of mkfs --type (tests/shell/help-text.test.ts)
```

and in `web/ui/tests/fs/ext-format.test.ts` (`it("keeps the six mkfs flags with their descriptions", ...)`) replace `expect(MKFS.summary).toBe("Format /dev/hda as ext2 or ext3 (clears the timeline)");` with

```ts
    expect(MKFS).not.toHaveProperty("summary"); // the shell's mkfs has one summary for every family
```

- [ ] **Step 12: Run the shell tests and watch them pass.**

```sh
pnpm exec vitest run tests/shell tests/fs/fat16-format.test.ts tests/fs/ext-format.test.ts
```

Expected: `Test Files  14 passed (14)`, `Tests  187 passed (187)` (167 in `tests/shell`, 5 in `fat16-format`, 15 in `ext-format`).

#### Round 3: re-registration when the family changes

- [ ] **Step 13: Write the failing re-registration tests.** In `web/ui/tests/shell/journal.test.ts` replace the import line

```ts
import { createCommands } from "../../src/shell/commands";
```

with

```ts
import { commandSetOf, createCommands, registerCommands, type CommandDef, type CommandRegistry } from "../../src/shell/commands";
```

and append at the end of the file:

```ts

describe("re-registration when the family changes", () => {
  /** browser-terminal's two registration calls over a map, logging each call in order. */
  function fakeTerminal() {
    const registered = new Map<string, CommandDef["fn"]>();
    const calls: string[] = [];
    const term: CommandRegistry = {
      registerCommand: (spec, fn) => {
        registered.set(spec.name, fn);
        calls.push(`+${spec.name}`);
      },
      unregisterCommand: (name) => {
        registered.delete(name);
        calls.push(`-${name}`);
      },
    };
    return { term, registered, calls };
  }

  it("keys the command set on the family and the journal's presence", () => {
    expect(commandSetOf(makeHost().adapter)).toBe("fat16");
    expect(commandSetOf(makeHost(Volume.formatExt2(undefined)).adapter)).toBe("ext");
    expect(commandSetOf(makeHost(Volume.formatExt3(undefined)).adapter)).toBe("ext+journal");
  });

  it("swaps every name for the new command set (fat16, ext3, ext2, fat16), keeping the working directory", async () => {
    const host = makeHost();
    const vfs = new Vfs();
    const { term, registered, calls } = fakeTerminal();
    const fat = registerCommands(term, host, vfs);
    expect(fat).toEqual(names(createCommands(host, vfs)));
    expect([...registered.keys()]).toEqual(fat);
    expect(fat).not.toContain("crash");

    await call(createCommands(host, vfs), "mkfs", { flags: { type: "ext3" } });
    await call(createCommands(host, vfs), "mkdir", { positionals: ["/mnt/d"] });
    vfs.cwd = "/mnt/d";
    calls.length = 0;
    const ext3 = registerCommands(term, host, vfs, fat);
    expect(calls.slice(0, fat.length)).toEqual(fat.map((n) => `-${n}`)); // every old name goes first
    expect(calls.slice(fat.length)).toEqual(ext3.map((n) => `+${n}`));
    expect([...registered.keys()]).toEqual(ext3);
    expect(ext3.slice(-2)).toEqual(["crash", "recover"]);
    const pwd = await call([{ spec: { name: "pwd", summary: "" }, fn: registered.get("pwd")! }], "pwd");
    expect(pwd.value).toBe("/mnt/d"); // the new definitions share the old working directory

    // ext3 to ext2 keeps the family but loses the journal: crash and recover go.
    host.format("ext", { variant: "ext2" });
    const ext2 = registerCommands(term, host, vfs, ext3);
    expect(ext2).toEqual(ext3.slice(0, -2));
    expect(registered.has("crash")).toBe(false);
    expect(registered.has("recover")).toBe(false);

    host.format("fat16");
    expect(registerCommands(term, host, vfs, ext2)).toEqual(fat);
    expect(registered.has("crash")).toBe(false);
  });
});
```

- [ ] **Step 14: Run them and watch them fail.**

```sh
pnpm exec vitest run tests/shell/journal.test.ts
```

Expected: `Test Files  1 failed (1)`, `Tests  2 failed | 11 passed (13)`, with `TypeError: (0 , commandSetOf) is not a function` and `TypeError: (0 , registerCommands) is not a function`.

- [ ] **Step 15: Add the command-set key and the registration loop.** In `web/ui/src/shell/commands.ts` change the seam import to

```ts
import type { FsAdapter, FsFamily, MkfsFlag } from "../fs/adapter";
```

and append after `createCommands`:

```ts

/**
 * Which command set `createCommands` builds for this adapter: the family, plus `+journal` when
 * it has a journal capability (`fat16`, `ext`, `ext+journal`). The terminal re-registers when
 * this changes, because the summaries and flag descriptions that name the family's nouns, and
 * the presence of `crash` and `recover`, are fixed when a command is registered.
 */
export function commandSetOf(adapter: FsAdapter): string {
  return adapter.journal ? `${adapter.id}+journal` : adapter.id;
}

/** browser-terminal's two registration calls, all `registerCommands` needs of the terminal. */
export interface CommandRegistry {
  registerCommand(spec: CommandSpec, fn: CommandDef["fn"]): void;
  unregisterCommand(name: string): void;
}

/**
 * Unregister the `previous` command names, register `createCommands(host, vfs)` in their place,
 * and return the names now registered for the next call. The same `vfs` keeps the working
 * directory across a re-registration.
 */
export function registerCommands(term: CommandRegistry, host: ShellHost, vfs: Vfs, previous: readonly string[] = []): string[] {
  for (const name of previous) term.unregisterCommand(name);
  const defs = createCommands(host, vfs);
  for (const { spec, fn } of defs) term.registerCommand(spec, fn);
  return defs.map((d) => d.spec.name);
}
```

- [ ] **Step 16: Run them and watch them pass.**

```sh
pnpm exec vitest run tests/shell/journal.test.ts
```

Expected: `Test Files  1 passed (1)`, `Tests  13 passed (13)`.

- [ ] **Step 17: Re-register in the terminal drawer.** In `web/ui/src/components/TerminalPanel.svelte` replace the import

```ts
  import { createCommands } from "../shell/commands";
```

with

```ts
  import { commandSetOf, registerCommands } from "../shell/commands";
```

add after `let host: ShellHost | null = null;`:

```ts
  // The names the terminal holds and the command set they were built for (`commandSetOf`),
  // so the family watcher below can swap them all. Plain fields, not state.
  let registered: string[] = [];
  let registeredSet: string | null = null;
```

in `ensureCreated` replace

```ts
        // Commands read live store fields through the host on every call, so registering
        // once is enough (same pattern as browser-terminal's Svelte demo).
        const created = createStoreHost(close, (prefix) => term.setPrompt(prefix));
        for (const { spec, fn } of createCommands(created, vfs)) term.registerCommand(spec, fn);
```

with

```ts
        // Commands read live store fields through the host on every call; only what is fixed
        // at registration (the command set, its summaries and flag descriptions) follows the
        // family, through the re-registration effect below.
        const created = createStoreHost(close, (prefix) => term.setPrompt(prefix));
        registered = registerCommands(term, created, vfs);
        registeredSet = commandSetOf(volume.adapter);
```

add this effect right after the volume watcher (the `$effect` that ends with `untrack(() => host?.setPrompt(promptFor(vfs.cwd)));` and `});`) and before `function disposeTerminal() {`:

```ts
  // Re-register when the mounted family (or its journal) changes. Every command reads the
  // live adapter when it runs, but browser-terminal keeps each spec as it was registered: the
  // summaries and flag descriptions that name the family's nouns (`df`'s "Cluster usage",
  // the `b:65 (block)` address help) and whether `crash` and `recover` exist at all are fixed
  // then. So swap the whole set: unregister every name, register `createCommands` again over
  // the same `vfs` (the working directory survives), and re-set the prompt. `volume.adapter`
  // is the only tracked read; the terminal and the names are plain fields, and the work is
  // untracked. Before the terminal exists there is nothing to swap: creation registers the
  // set of the family mounted then.
  $effect(() => {
    const set = commandSetOf(volume.adapter);
    untrack(() => {
      if (!bt || !host || set === registeredSet) return;
      registered = registerCommands(bt, host, vfs, registered);
      registeredSet = set;
      host.setPrompt(promptFor(vfs.cwd));
    });
  });

```

and in `disposeTerminal` add after `host = null;`:

```ts
    registered = [];
    registeredSet = null;
```

The effect's first run finds `bt` null (the drawer creates the terminal lazily) or `registeredSet` equal to the mounted set, so it does nothing; that is the "skip the first run" rule without a separate flag.

- [ ] **Step 18: Type-check the component.**

```sh
pnpm build
```

Expected: svelte-check's `COMPLETED ... 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS` and vite's `✓ built in ...`.

#### Round 4: the read commands on ext

- [ ] **Step 19: Pin `df`, `stat`, `seek`, `xxd --offset`, and `write --at` on an ext3 host.** Tasks 2 and 3 already made these family-neutral (`df`'s keys come from the unit noun, `stat` spreads `adapter.stat`, and `seek`, `xxd --offset`, and `write --at` use `parseAddr` with the adapter); this round pins them through the commands, with both of spec section 6's ext forms (`b:N`, `i:N`) for each address flag. In `web/ui/tests/shell/read-commands.test.ts` add the wasm import as the second line:

```ts
import { Volume } from "../../src/lib/wasm";
```

and add right before `describe("rewound timeline", () => {`:

```ts
describe("on an ext3 host", () => {
  /** The default ext3 disk with one 12-byte file: inode 12, data block 1111. */
  function extSetup() {
    const host = makeHost(Volume.formatExt3(undefined));
    host.run((v) => v.createFile("/hello.txt", enc("Hello, ext3!")));
    return { host, defs: createCommands(host, new Vfs()) };
  }

  it("df counts 1 KiB blocks from the superblock", async () => {
    const { defs } = extSetup();
    expect((await call(defs, "df")).value).toEqual([
      { filesystem: "/dev/hda", mounted: "/mnt", type: "ext3", blockSize: 1024, blocks: 16384, used: 1180, free: 15204, bytesUsed: 1180 * 1024, bytesFree: 15204 * 1024, use: "7%" },
    ]);
  });

  it("stat spreads the inode facts after the generic ones", async () => {
    const { defs } = extSetup();
    const r = (await call(defs, "stat", ["/mnt/hello.txt"])).value as Record<string, unknown>;
    expect(Object.keys(r)).toEqual(["path", "name", "type", "size", "created", "modified", "accessed", "inode", "inodeOffset", "mode", "links", "blocks512", "dataBlocks", "indirectBlocks", "dirEntryOffset"]);
    expect(r).toMatchObject({
      path: "/mnt/hello.txt", name: "hello.txt", type: "file", size: 12,
      inode: 12, inodeOffset: "0x1980", mode: "0100644", links: 1, blocks512: 2, dataBlocks: [1111], indirectBlocks: [], dirEntryOffset: "0x1142c",
    });
    expect((await call(defs, "stat", ["/mnt/lost+found"])).value).toMatchObject({
      type: "dir", inode: 11, inodeOffset: "0x1900", mode: "040700", links: 2, dataBlocks: [70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81], indirectBlocks: [], dirEntryOffset: "0x11418",
    });
    expect((await call(defs, "stat", ["/dev/hda"])).value).toEqual({
      path: "/dev/hda", type: "block", size: 16384 * 1024, sectorSize: 1024, sectors: 16384, fsType: "ext3", state: "ok",
    });
  });

  it("seek takes block and inode addresses", async () => {
    const { host, defs } = extSetup();
    await call(defs, "seek", ["b:69"]);
    await call(defs, "seek", ["i:12"]);
    await call(defs, "seek", ["s:2"]);
    expect(host.jumps).toEqual([69 * 1024, 6 * 1024 + 0x180, 2 * 1024]);
    const e = await callErr(defs, "seek", ["c:3"]);
    expect(e.message).toBe("bad address 'c:3'");
    expect(e.help).toBe("addresses: 0x1f (hex), 512 (decimal), b:65 (block), i:11 (inode)");
  });

  it("xxd --offset and write --at take block and inode addresses too", async () => {
    const { host, defs } = extSetup();
    const hello = await call(defs, "xxd", { positionals: ["/dev/hda"], flags: { offset: "b:1111", len: "16" } });
    expect((hello.value as string).startsWith("00115c00: ")).toBe(true); // block 1111 = 0x115c00
    expect(hello.value).toContain("Hello, ext3!");
    const inode = await call(defs, "xxd", { positionals: ["/dev/hda"], flags: { offset: "i:12", len: "16" } });
    expect((inode.value as string).startsWith("00001980: ")).toBe(true); // inode 12's slot, block 6 + 0x180
    expect((await call(defs, "write", { positionals: ["/dev/hda"], flags: { at: "b:2000" } }, "RAW")).log).toEqual(["3 bytes -> /dev/hda at 0x1f4000"]);
    expect((await call(defs, "write", { positionals: ["/dev/hda"], flags: { at: "i:20" } }, "RAW")).log).toEqual(["3 bytes -> /dev/hda at 0x1d80"]); // a free inode's slot, block 7 + 0x180
    expect(host.history.slice(-2).map((r) => r.op)).toEqual(["write_raw 0x1f4000 +3", "write_raw 0x1d80 +3"]);
  });
});

```

- [ ] **Step 20: Run them.**

```sh
pnpm exec vitest run tests/shell/read-commands.test.ts
```

Expected: `Test Files  1 passed (1)`, `Tests  25 passed (25)`. (If one fails, the fix belongs in the ext adapter's `stat`/`df` or in `parseAddr`, not in the test: these are the numbers of the default ext3 disk from the contract.)

#### Gates, browser, and commit

- [ ] **Step 21: Run the web gates.**

```sh
pnpm test && pnpm build
```

Expected: `pnpm test` prints `Test Files  46 passed (46)` and `Tests  447 passed (447)`; `pnpm build` prints svelte-check's `COMPLETED ... 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS` and vite's `✓ built in ...`. Then `git status --short` lists only the files in **Files** (no `pnpm-lock.yaml`, no `pnpm-workspace.yaml`).

- [ ] **Step 22: Check it in the browser.** In `web/ui`, `pnpm dev` (or `pnpm exec vite --port <free port> --strictPort`), open the page, open the terminal (the `Terminal` button), and wait for each prompt before typing the next line:
  1. `mkfs --type ext3` prints `formatted /dev/hda as ext3; the timeline was cleared`; the map becomes the block-group map and the tree shows `lost+found`.
  2. `df` prints one row `/dev/hda /mnt ext3 1024 16384 1179 15205 1207296 15569920 7%` under the headers `blockSize` and `blocks`; `help df` says `Block usage of the mounted volume`.
  3. `stat /mnt/lost+found` prints a row with `inode 11`, `inodeOffset 0x1900`, `mode 040700`, `links 2`, `blocks512 24`.
  4. `crash` prints `armed: the next change to /dev/hda stops after commit`, and the Journal panel (Task 5) shows `Armed: the next change stops after commit` and `Disarm` at once; `crash --off` prints `disarmed` and brings the panel's phase select back; `crash` again; `help crash` lists `--at` and `--off` with the descriptions above.
  5. `echo hi > /mnt/x.txt` adds a timeline step `create_file /x.txt (crashed after commit)` ending in `crashed crashed after commit`, and the top bar shows `Volume needs recovery`.
  6. `recover` prints the ten lines from `scanned the journal from block 1, sequence 1: 1 committed transaction, 7 tagged blocks` to `cleared needs_recovery in the superblock`; the timeline gains a `recover` step, `x.txt` appears in the tree, and `Volume needs recovery` goes away.
  7. `mkfs --type ext3 --spc 2` prints ``error: `mkfs`: --spc is not an ext3 option``. `mkfs --type ext2`, then `recover`: ``error: unknown command `recover` ``.
  8. `mkfs --type fat16` prints `formatted /dev/hda as FAT16; the timeline was cleared`; `crash` is then ``error: unknown command `crash` ``, and `help df` says `Cluster usage of the mounted volume` again. The console shows no errors other than browser-terminal's own logging of the thrown `ShellError`s.

- [ ] **Step 23: Commit.** From the repo root:

```sh
git add web/ui/src/components/TerminalPanel.svelte web/ui/src/shell/commands.ts web/ui/src/shell/host.ts web/ui/src/shell/journalCommands.ts web/ui/src/shell/storeHost.svelte.ts web/ui/src/fs/adapter.ts web/ui/src/fs/fat16/format.ts web/ui/src/fs/ext/format.ts web/ui/tests/fs/fat16-format.test.ts web/ui/tests/fs/ext-format.test.ts web/ui/tests/shell/errors.test.ts web/ui/tests/shell/helpers.ts web/ui/tests/shell/help-text.test.ts web/ui/tests/shell/journal.test.ts web/ui/tests/shell/mutations.test.ts web/ui/tests/shell/read-commands.test.ts
git commit -m "feat(ui): crash, recover, and mkfs --type in the terminal"
```

Facts later tasks rely on (pinned by this task's tests):

- `createCommands` ends with `crash`, `recover` exactly when `host.adapter.journal` is defined; `commandSetOf(adapter)` is `fat16` | `ext` | `ext+journal`, and `TerminalPanel.svelte` re-registers the whole set (via `registerCommands`) whenever it changes, re-setting the prompt from the unchanged `vfs.cwd`.
- `crash` output: `armed: the next change to /dev/hda stops ${CRASH_PHASE_LABELS[phase]}` or `disarmed`; `--at` values `before-commit`, `after-commit` (default), `during-checkpoint`; errors `unknown crash phase '${at}'` (help `phases: before-commit, after-commit, during-checkpoint`), `give --at or --off, not both`, `/dev/hda has no journal (${fsType})` (help `crash and recover need an ext3 volume: mkfs --type ext3`). Arming goes through `host.setArmedPhase` inside `fsCall` (in the app: `volume.setArmedPhase`, so `volume.armedPhase` and the Journal panel follow at once) and never adds a timeline step; while rewound it logs and also writes the rewound warning to `ctx.err`.
- `recover` logs each event `text` of the `recover` record in order (after an after-commit crash of a one-block create on the default disk: ten lines ending `journal emptied; next transaction 3`, `cleared needs_recovery in the superblock`; on a clean journal the one line `journal is clean; nothing to replay`), and the record lands in the timeline as op `recover`. The crashed op's name is `create_file /x.txt (crashed after commit)`.
- `mkfs`: summary `Format /dev/hda (clears the timeline); --type picks fat16, ext2, or ext3`; `--type` desc `fat16, ext2, or ext3 (default: the mounted volume's type)`; flag order `type`, the six FAT flags, then `blocks`, `inodes-per-group`, `uuid`, `journal-blocks`, `journal-mode` (`label` stays in FAT's position); `--type ext2|ext3` formats family `ext` with `options.variant` first (`{ variant: "ext3", totalBlocks: 4096, label: "shell" }`); errors `--${long} is not a fat16 option` / `is not an ext2 option` / `is not an ext3 option`, `unknown type '${type}'` (help `types: fat16, ext2, ext3`), and `/dev/hda: journalBlocks and journalMode need variant ext3`.
- `rewoundWarning` and `warnIfRewound` live in `shell/host.ts` and are re-exported from `shell/commands.ts`.
- `ShellHost` has `setArmedPhase(phase | null)`; `TestHost.crashes` records each call. `MkfsSpec` is `{ flags }`: a family's `MKFS` has no summary and no done line.
- The boundary scan flags `.armCrash(` and `.disarmCrash(` on every receiver outside `src/fs/ext/`: the store and the hosts arm with `setArmedPhase`, and a lesson (Task 7) arms through `asExt(fs).journal!.arm(...)`, which the scan never flagged.

---

### Task 7: the three ext lessons, the grouped picker, the docs, and the browser pass

Spec: `docs/superpowers/specs/2026-09-24-ext-explorer-design.md` sections 5 (the ScenarioPanel bullet: `<optgroup>`s by family display name in registry order), 7 (the three lessons, all `family: "ext"` on the default ext3 disk, and `scenarios/index.ts` appending them after the FAT lessons), 8 (`tests/scenarios.test.ts` and the three pinning tests; the browser pass), 9 (the `web/ui/README.md` and `docs/ROADMAP.md` updates), 10 (FAT16 stays the default; the ext lessons format ext3 themselves).

Three lessons join the nine FAT ones, each running on the default ext3 disk (`ext.format()`: ordered mode, 16,384 blocks) that `ScenarioRunner.start()` formats from the lesson's `family`:

- **`ext-fundamentals`, "The fundamentals (ext)"** (13 steps): its first step creates `/hello.txt` and `/bigger.txt`; the rest tour the boot block, the superblock, the group descriptors, both bitmaps, the inode table, the root directory, hello.txt's inode and data block, bigger.txt's single-indirect block, the journal, and group 1's backup superblock.
- **`journaled-write`, "A journaled write"** (9 steps): one create of `/notes.txt` (3,000 bytes), then the dump follows the create's writes in order: the needs-recovery flag, the data, the journal superblock, the descriptor, the copies, the commit, the checkpoint, the emptied journal, the cleared flag.
- **`crash-recover`, "Crash and recover"** (7 steps): a crash after the commit that recovery replays, and a crash before the commit that recovery discards, leaving the data in a free block.

Every number the copy quotes is the default disk's and is pinned by the lesson's test against a live volume, and every step's resolved focus is pinned too. The journaled-write lesson's journal positions (descriptor at journal block 1, seven copies, commit at journal block 9) are read by its test from the create's own record (`journal_block_written` events and byte changes), not assumed.

The picker groups options in one `<optgroup>` per family (`FAT16`, then `ext`), computed by a new plain function `scenarioGroups()` in `scenarios/index.ts` so the node tests can pin it; the first option is still the FAT fundamentals.

Decisions the spec left open, each visible in the code below:

- **Two creates, one step.** The spec has the ext tour create both files "in its first step". A step's action returns one `OpRecord`, and the store patches its cached image with that record's changes, so the action merges the two creates' records (`both(a, b)`: changes and events concatenated in order, op `create_file /hello.txt; create_file /bigger.txt`). The test checks that the merged changes applied forward reproduce the volume's image and applied in reverse give back the fresh disk, so rewinding the step undoes both.
- **`s_sequence` at +0x18, `s_start` at +0x1C.** The journal superblock's `s_sequence` is at +0x18 and `s_start` at +0x1C (the core's annotation labels them `sequence` and `start`; each journal-superblock write is one 8-byte change at +0x18 covering both), as spec section 7 says. The lesson's "The journal superblock" step focuses +0x1C (`s_start` pointing at the transaction); its "The journal empties" step focuses +0x18, where that 8-byte change starts, and its text names both fields (`s_sequence` becomes 2, `s_start` goes back to 0), where the spec's step names only `s_start` back to 0. The test pins both offsets to the annotation labels.
- **The dump shows the disk after the op.** Steps about a moment inside the transaction say what the bytes read then and what they read now (the flag word reads 0x02 now and read 0x06 while the create ran; `s_start` reads 0 now and read 1). The Step strip's events, in order, are the lesson's timeline.
- **Rewind-safe focuses.** `volume.seek` does not refresh the adapter (the adapter stays at the latest state), so a focus function must not read state that later steps change. Focuses read only facts that stay put: `inodeOffset`, the journal's `firstBlock`, the geometry, and chains and directory entries that no later step moves. The crash lesson's orphan block is the exported constant `LOST_BLOCK = 1113`, pinned against the create's `blocks_allocated` event.
- **Constants exported for the tests.** `journaledWrite.ts` exports `FLAG_WORD`, `S_SEQUENCE`, `S_START`, `DESCRIPTOR_INDEX`, `COPY_COUNT`, and `COMMIT_INDEX`; `crashRecover.ts` imports `FLAG_WORD` and `DESCRIPTOR_INDEX` from it rather than declaring its own, and exports `CRASH_BYTES`, `LOST_TEXT`, and `LOST_BLOCK`.
- **One copy of the test helpers.** The three lesson tests share `stepText`, `runThrough`, and `focusOf` from `tests/fixtures/lesson.ts` (`lessonHelpers(scenario)`, created with the first test); the FAT `tests/fundamentals.test.ts` keeps its own. `extFundamentals.ts` re-exports the FAT fundamentals' `biggerText` (numbered lines), and the other two lessons import it for their file contents.
- **The inode bitmap's padding.** Group 0 has 512 inodes, so its inode bitmap uses 64 bytes and the rest of block 4 is set to ones; the copy says so, because the dump shows FF from +0x40 on.
- **The kernel difference.** The ROADMAP asks a lesson to explain that the emulator checkpoints each transaction at once; the journaled write's "Checkpoint" step does.
- **The FAT shell pin.** `tests/scenarios.test.ts` pinned `all.length` to 9 with the shell lesson last. With the ext lessons appended, that test now pins the nine FAT lessons (`all.filter((s) => s.family === "fat16")`) and the shell lesson as the last of them; no FAT string, number, or colour changes.

**Files**

- Create: `web/ui/src/scenarios/extFundamentals.ts` (1–104), `web/ui/src/scenarios/journaledWrite.ts` (1–85), `web/ui/src/scenarios/crashRecover.ts` (1–78), `web/ui/tests/fixtures/lesson.ts` (1–42)
- Modify: `web/ui/src/scenarios/index.ts` (1–33, whole file), `web/ui/src/components/ScenarioPanel.svelte` (1–32, whole file), `web/ui/README.md` (intro 3–9, Panes 39–42 and 59–81, Learning scenarios 111–121, Terminal 163–168 and 179–185, "What is filesystem-specific" 269 and 294–346, shortcuts 359), `docs/ROADMAP.md` (65–68, 88–89, 112–133, 156–157, 161–163, 174–181, 188–191, 208–229)
- Test: `web/ui/tests/ext-fundamentals.test.ts` (1–248), `web/ui/tests/journaled-write.test.ts` (1–172), `web/ui/tests/crash-recover.test.ts` (1–146), `web/ui/tests/scenarios.test.ts` (4 import, 46–69 new tests, 134–137 the shell test)
- Not touched: `crates/`, `web/ui/src/fs/**`, `web/ui/src/state/**`, `web/ui/src/shell/**`, every other component, `crates/wasm/README.md` (Task 1's; its ext2 note already says the mke2fs recipe was checked against the loader with e2fsprogs 1.47.4, which this task's browser pass confirms), `tests/adapterBoundary.test.ts` (the lessons import `fs/ext` from `scenarios/`, which it allows, and call no ext-only wasm method: they reach the journal through `asExt(fs).journal!`, and `journal!.recover()` is on a receiver named `journal`).

**Interfaces**

Consumes:

- `state/scenarios.svelte.ts`: `Scenario`, `Step`, `StepFocus` (`{ offset?, sector?, unit?, path?, showRemnants?, strings? }`); the runner calls `volume.format(s.family)` in `start()`, then per step `volume.run((v) => step.action!(v, volume.adapter))` and resolves a function focus with `volume.adapter`.
- `fs/index.ts`: `FAMILIES` (`{ fat16, ext }`, in that order; `FAMILIES.ext.name` is `"ext"`, `FAMILIES.fat16.name` is `"FAT16"`), `adapterFor`.
- `fs/adapter.ts`: `FsAdapter` (`chain`, `dataStart`, `owners` with `role`, `journal?`, `needsRecovery`, `describeUnit`), `FsFamilyId` (`"fat16" | "ext"`).
- `fs/ext` (Task 3/4): `asExt(fs): ExtAdapter`; `ExtAdapter.inodeOffset(ino)`, `dirEntryOffset(path)`, `geo` (`ExtGeometry`), `journal` (`ExtJournal`: `state().firstBlock`, `arm(phase)`, `recover()`).
- `scenarios/fundamentals.ts`: `biggerText(bytes)`.
- `core/patch.ts`: `applyChanges` (tests only).
- The wasm `Volume` in tests only: `extGeometry`, `extSuperblock`, `extInode`, `inodeNumber`, `dirEntries`, `fileBlocks`, `blockOwners`, `journalInfo`, `annotateSector`, `sector`, `image`, `layout`, `readFile`, `listDir`, `stat`, `needsRecovery`.

Produces (exact):

```ts
// scenarios/extFundamentals.ts
export const HELLO = "/hello.txt";
export const HELLO_TEXT = "Hello, ext3!";
export const BIGGER = "/bigger.txt";
export const BIGGER_BYTES = 13312;
export { biggerText };                        // re-exported from ./fundamentals
export const scenario: Scenario;              // id "ext-fundamentals", title "The fundamentals (ext)", family "ext", 13 steps

// scenarios/journaledWrite.ts
export const NOTES_PATH = "/notes.txt";
export const NOTES_BYTES = 3000;
export const FLAG_WORD = 1024 + 0x60;
export const S_SEQUENCE = 0x18;
export const S_START = 0x1c;
export const DESCRIPTOR_INDEX = 1;
export const COPY_COUNT = 7;
export const COMMIT_INDEX = DESCRIPTOR_INDEX + COPY_COUNT + 1;   // 9
export const scenario: Scenario;              // id "journaled-write", title "A journaled write", family "ext", 9 steps

// scenarios/crashRecover.ts
export const CRASH_PATH = "/crash.txt";
export const CRASH_BYTES = 2048;
export const LOST_PATH = "/lost.txt";
export const LOST_TEXT = "Written before the crash, claimed by nothing.";
export const LOST_BLOCK = 1113;
export const scenario: Scenario;              // id "crash-recover", title "Crash and recover", family "ext", 7 steps

// scenarios/index.ts
export const all: Scenario[];                 // [nine FAT lessons..., extFundamentals, journaledWrite, crashRecover]
export interface ScenarioGroup { family: FsFamilyId; label: string; scenarios: Scenario[] }
export function scenarioGroups(list?: readonly Scenario[]): ScenarioGroup[];   // registry order, label = FAMILIES[id].name, empty families dropped

// tests/fixtures/lesson.ts (tests only)
export function lessonHelpers(scenario: Scenario): {
  stepText(title: string): string;                                              // throws `no step titled "${title}"`
  runThrough(title?: string): { vol: Volume; fs: FsAdapter; records: OpRecord[] };   // format the family, run actions (refresh after each), stop after `title` (all steps when omitted)
  focusOf(title: string, fs: FsAdapter): StepFocus | undefined;                 // a function focus resolved with fs
};

// components/ScenarioPanel.svelte: <select id="scenario-select" bind:value={selectedId}> with
//   {#each groups as g}<optgroup label={g.label}>{#each g.scenarios as s}<option value={s.id}>{s.title}</option>{/each}</optgroup>{/each}
```

Environment for every command below, from `web/ui` unless a step says otherwise: any pnpm 10.26 or newer (nvm's pnpm 12 and Homebrew's 10.33 both work). Both web packages keep their pnpm settings in a tracked `pnpm-workspace.yaml`, so a frozen install must leave `web/ui/pnpm-lock.yaml` and `web/ui/pnpm-workspace.yaml` unmodified; if either shows as modified in `git status` at any point, restore it with `git checkout web/ui/pnpm-lock.yaml web/ui/pnpm-workspace.yaml` and run `CI=true pnpm install --frozen-lockfile` again (`CI=true` lets pnpm purge a stale modules directory without asking).

#### Round 0: the baseline

- [ ] **Step 1: Build the wasm package and install it.** From the repo root:

```sh
wasm-pack build crates/wasm --target bundler
```

Expected: ends with `Your wasm pkg is ready to publish at crates/wasm/pkg.` Then, from `web/ui`:

```sh
CI=true pnpm install --frozen-lockfile
pnpm test
```

Expected: `Test Files  46 passed (46)` and `Tests  447 passed (447)` (the base branch with Tasks 1 to 6).

#### Round 1: the fundamentals (ext)

- [ ] **Step 2: Write the failing test.** The three lesson tests share their helpers; create `web/ui/tests/fixtures/lesson.ts` first:

```ts
import type { OpRecord, Volume } from "../../src/lib/wasm";
import { adapterFor, FAMILIES } from "../../src/fs";
import type { FsAdapter } from "../../src/fs/adapter";
import type { Scenario } from "../../src/state/scenarios.svelte";

/**
 * What the ext lesson tests share, bound to one lesson: a step's text by title, a run of the
 * lesson's actions the way the runner makes it, and a step's focus as the runner resolves it.
 */
export function lessonHelpers(scenario: Scenario) {
  /** The text of the step with that title; an unknown title throws, so a renamed step fails. */
  const stepText = (title: string): string => {
    const s = scenario.steps.find((st) => st.title === title);
    if (!s) throw new Error(`no step titled "${title}"`);
    return s.text;
  };

  /** Run the lesson the way the runner does: a format of its family, then each action in order
   *  with the adapter refreshed after it, stopping after the step with the given title (after
   *  the last step when none is given). */
  const runThrough = (title?: string): { vol: Volume; fs: FsAdapter; records: OpRecord[] } => {
    const vol = FAMILIES[scenario.family].format();
    const fs = adapterFor(vol);
    const records: OpRecord[] = [];
    for (const step of scenario.steps) {
      if (step.action) {
        records.push(step.action(vol, fs));
        fs.refresh();
      }
      if (step.title === title) break;
    }
    return { vol, fs, records };
  };

  /** A step's focus as the runner resolves it: the function form gets the bound adapter. */
  const focusOf = (title: string, fs: FsAdapter) => {
    const s = scenario.steps.find((st) => st.title === title)!;
    return typeof s.focus === "function" ? s.focus(fs) : s.focus;
  };

  return { stepText, runThrough, focusOf };
}
```

Then create `web/ui/tests/ext-fundamentals.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { applyChanges } from "../src/core/patch";
import { FAMILIES } from "../src/fs";
import { BIGGER, BIGGER_BYTES, biggerText, HELLO, HELLO_TEXT, scenario } from "../src/scenarios/extFundamentals";
import { lessonHelpers } from "./fixtures/lesson";

// "The fundamentals (ext)" quotes the default ext3 disk's numbers in its copy (block 69,
// 15,190 free blocks, inode 12 at block 6 + 0x180, ...). These tests pin every quoted number
// to the volume the lesson runs on, and every step's resolved focus, so a change to the default
// format cannot leave the lesson teaching stale arithmetic.

const n = (v: number) => v.toLocaleString("en-US");
const u32 = (b: Uint8Array, at: number) => new DataView(b.buffer, b.byteOffset, b.byteLength).getUint32(at, true);
const { stepText, runThrough, focusOf } = lessonHelpers(scenario);
/** The first offset where two images differ, or -1: a 16 MB `toEqual` is too slow to diff. */
const firstDiff = (a: Uint8Array, b: Uint8Array) => {
  if (a.length !== b.length) return Math.min(a.length, b.length);
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return i;
  return -1;
};
const setBits = (bytes: Uint8Array) => bytes.reduce((sum, b) => sum + b.toString(2).split("").filter((c) => c === "1").length, 0);

describe("the ext fundamentals lesson", () => {
  it("runs on the default ext3 disk", () => {
    expect(scenario.id).toBe("ext-fundamentals");
    expect(scenario.title).toBe("The fundamentals (ext)");
    expect(scenario.family).toBe("ext");
    expect(FAMILIES[scenario.family].format().fsType()).toBe("ext3");
  });

  it("creates both files in one timeline step that rewinds cleanly", () => {
    const fresh = FAMILIES.ext.format().image();
    const { vol, records } = runThrough("One disk, two block groups");
    expect(records).toHaveLength(1);
    const [rec] = records;
    expect(rec.op).toBe(`create_file ${HELLO}; create_file ${BIGGER}`);
    // The runner patches its cached image with the record's changes: forward must reach the
    // volume's bytes, and reverse must get back to the freshly formatted disk.
    const image = fresh.slice();
    applyChanges(image, rec.changes, "forward");
    expect(firstDiff(image, vol.image())).toBe(-1);
    applyChanges(image, rec.changes, "reverse");
    expect(firstDiff(image, fresh)).toBe(-1);
    expect(new TextDecoder().decode(vol.readFile(HELLO))).toBe(HELLO_TEXT);
    expect(vol.readFile(BIGGER)).toEqual(biggerText(BIGGER_BYTES));
  });

  it("quotes the disk's size, its two groups, and the zero boot block", () => {
    const { vol } = runThrough("One disk, two block groups");
    const g = vol.extGeometry();
    expect(g).toMatchObject({ blockSize: 1024, totalBlocks: 16384, blocksPerGroup: 8192, firstDataBlock: 1 });
    expect(g.groups.map((x) => [x.firstBlock, x.blockCount])).toEqual([[1, 8192], [8193, 8191]]);
    expect(vol.sector(0).every((b) => b === 0)).toBe(true);
    const text = stepText("One disk, two block groups");
    for (const s of [`${n(g.totalBlocks)} blocks of ${n(g.blockSize)} bytes: 16 MB`, `block groups of ${n(g.blocksPerGroup)}`, "this disk has two", `group 0 starts at block ${g.groups[0].firstBlock}`, `one block short, ${n(g.groups[1].blockCount)}`]) {
      expect(text).toContain(s);
    }
    expect(g.totalBlocks * g.blockSize).toBe(16 * 1024 * 1024);
  });

  it("quotes the superblock's magic, sizes, and free counts after the two creates", () => {
    const { vol } = runThrough("One disk, two block groups");
    const sb = vol.extSuperblock();
    expect(sb).toMatchObject({ magic: 0xef53, inodesCount: 1024, blocksCount: 16384, freeBlocks: 15190, freeInodes: 1011 });
    const block1 = vol.sector(1);
    expect([block1[0x38], block1[0x39]]).toEqual([0x53, 0xef]);
    const at = (offset: number) => vol.annotateSector(1).find((a) => a.range.start === offset)?.label;
    expect([at(0x00), at(0x04), at(0x0c), at(0x10), at(0x38)]).toEqual(["inodes count", "blocks count", "free blocks count", "free inodes count", "magic"]);
    // 15 blocks (1 for hello.txt, 13 data and 1 indirect for bigger.txt) and 2 inodes left the fresh counts.
    expect(FAMILIES.ext.format().extSuperblock()).toMatchObject({ freeBlocks: sb.freeBlocks + 15, freeInodes: sb.freeInodes + 2 });
    const text = stepText("Block 1 is the superblock");
    for (const s of ["At +0x38 is the magic number, 53 EF, which is 0xEF53", `${n(sb.inodesCount)} inodes (+0x00)`, `${n(sb.blocksCount)} blocks (+0x04)`, "+0x0C and +0x10 hold the free counts", `${n(sb.freeBlocks)} blocks and ${n(sb.freeInodes)} inodes`]) {
      expect(text).toContain(s);
    }
  });

  it("quotes each group's descriptor", () => {
    const { vol } = runThrough("One disk, two block groups");
    const [g0, g1] = vol.extGeometry().groups;
    expect(g0).toMatchObject({ descriptorsBlock: 2, blockBitmap: 3, inodeBitmap: 4, inodeTable: 5, freeBlocks: 7067, freeInodes: 499 });
    expect(g1).toMatchObject({ blockBitmap: 8195, inodeBitmap: 8196, inodeTable: 8197 });
    const block2 = vol.sector(2);
    expect([u32(block2, 0x00), u32(block2, 0x04), u32(block2, 0x08)]).toEqual([3, 4, 5]);
    expect([u32(block2, 0x20), u32(block2, 0x24), u32(block2, 0x28)]).toEqual([8195, 8196, 8197]);
    const text = stepText("The group descriptors");
    for (const s of ["one 32-byte descriptor per group", `block bitmap (block ${g0.blockBitmap})`, `inode bitmap (block ${g0.inodeBitmap})`, `inode table (block ${g0.inodeTable})`, `${n(g0.freeBlocks)} blocks and ${n(g0.freeInodes)} inodes`, `at +0x20, names blocks ${g1.blockBitmap}, ${g1.inodeBitmap}, and ${g1.inodeTable}`]) {
      expect(text).toContain(s);
    }
  });

  it("quotes the block bitmap: metadata and journal to 1110, the files to 1125", () => {
    const fresh = FAMILIES.ext.format();
    // Bit i is block i + 1: the first free block of a fresh disk is 1111.
    expect(setBits(fresh.sector(3))).toBe(1110);
    expect(fresh.sector(3)[138]).toBe(0x3f);
    const layout = fresh.layout();
    expect(layout.find((r) => r.kind === "journal")?.sectors).toEqual({ start: 82, end: 1106 });
    expect(fresh.blockOwners().filter((o) => o.path === "<journal>" && o.role === "indirect").map((o) => o.block)).toEqual([1106, 1107, 1108, 1109, 1110]);
    expect(fresh.fileBlocks("/lost+found").data).toEqual([70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81]);
    expect(fresh.fileBlocks("/").data).toEqual([69]);

    const { vol } = runThrough("One disk, two block groups");
    const bitmap = vol.sector(3);
    expect(setBits(bitmap)).toBe(1125);
    expect(bitmap.slice(0, 140).every((b) => b === 0xff)).toBe(true);
    expect(bitmap[0x8c]).toBe(0x1f);
    expect(bitmap.slice(0x8d).every((b) => b === 0)).toBe(true);
    const files = [...vol.fileBlocks(HELLO).data, ...vol.fileBlocks(BIGGER).data, ...vol.fileBlocks(BIGGER).indirect.map((i) => i.block)].sort((a, b) => a - b);
    expect(files).toEqual(Array.from({ length: 15 }, (_, i) => 1111 + i));
    const text = stepText("The block bitmap");
    for (const s of ["starting with block 1", "Blocks 1 to 1110", "(69 to 81)", "(82 to 1110)", "the next 15, 1111 to 1125", "140 bytes of FF", "at +0x8C, 1F", `${n(1125)} bits set`]) {
      expect(text).toContain(s);
    }
  });

  it("quotes the inode bitmap", () => {
    const { vol } = runThrough("One disk, two block groups");
    const sb = vol.extSuperblock();
    expect(sb).toMatchObject({ firstIno: 11, journalInum: 8, inodesPerGroup: 512 });
    expect([vol.inodeNumber("/"), vol.inodeNumber("/lost+found"), vol.inodeNumber(HELLO), vol.inodeNumber(BIGGER)]).toEqual([2, 11, 12, 13]);
    const bitmap = vol.sector(4);
    expect([bitmap[0], bitmap[1]]).toEqual([0xff, 0x1f]);
    // 512 inodes need 64 bytes; the rest of the block is padding, all ones.
    expect(setBits(bitmap.slice(0, 512 / 8))).toBe(13);
    expect(bitmap.slice(0x40).every((b) => b === 0xff)).toBe(true);
    const text = stepText("The inode bitmap");
    for (const s of ["group 0's 512 inodes", "starting with inode 1", "Inodes 1 to 10 are reserved (2 is the root directory, 8 the journal), 11 is lost+found", "took 12 and 13", "FF 1F: 13 bits set", "only 64 bytes", "+0x40 onward reads FF"]) {
      expect(text).toContain(s);
    }
  });

  it("quotes the inode table's size and the slots of inodes 2 and 11", () => {
    const { vol } = runThrough("One disk, two block groups");
    const g = vol.extGeometry();
    expect(g).toMatchObject({ inodesPerGroup: 512, inodeSize: 128, inodeTableBlocks: 64 });
    expect(g.groups[0].inodeTable + g.inodeTableBlocks - 1).toBe(68);
    expect(g.blockSize / g.inodeSize).toBe(8);
    expect(vol.extInode(2).slot).toEqual({ block: 5, offset: 5 * 1024 + 0x80 });
    expect(vol.extInode(11).slot).toEqual({ block: 6, offset: 6 * 1024 + 0x100 });
    expect(vol.extInode(2).mode.toString(8)).toBe("40755");
    expect(vol.extInode(2).block).toHaveLength(15);
    const text = stepText("The inode table");
    for (const s of ["Blocks 5 to 68", "512 inodes of 128 bytes, eight to a block", "(N − 1) × 128", "inode 2, the root directory, is at block 5 + 0x80", "inode 11, lost+found, at block 6 + 0x100", "15 block pointers"]) {
      expect(text).toContain(s);
    }
  });

  it("quotes the root directory's entries and rec_lens", () => {
    const { vol } = runThrough("One disk, two block groups");
    expect(vol.extInode(2).block[0]).toBe(69);
    const entries = vol.dirEntries("/");
    expect(entries.map((e) => [e.name, e.inode, e.recLen])).toEqual([[".", 2, 12], ["..", 2, 12], ["lost+found", 11, 20], ["hello.txt", 12, 20], ["bigger.txt", 13, 960]]);
    expect(entries.reduce((sum, e) => sum + e.recLen, 0)).toBe(1024);
    const text = stepText("The root directory");
    expect(text).toContain("Inode 2's first block pointer reads 69");
    expect(text).toContain(". and .. (both inode 2) take 12 bytes each, lost+found 20, hello.txt 20, and bigger.txt the remaining 960");
  });

  it("quotes hello.txt's inode and its one block", () => {
    const { vol } = runThrough("One disk, two block groups");
    const ino = vol.extInode(12);
    expect(ino.slot).toEqual({ block: 6, offset: 6 * 1024 + 0x180 });
    const slot = vol.sector(6).slice(0x180, 0x200);
    expect(u32(slot, 0x04)).toBe(HELLO_TEXT.length);
    expect(u32(slot, 0x28)).toBe(1111);
    expect(ino.block.slice(1)).toEqual(new Array(14).fill(0));
    // 1111 is the first block a fresh format leaves free.
    expect(FAMILIES.ext.format().sector(3)[138]).toBe(0x3f);
    const text = stepText("hello.txt's inode");
    for (const s of ["names inode 12, at block 6 + 0x180", `size field (+0x04) reads ${HELLO_TEXT.length}`, "first block pointer (+0x28) reads 1111", "the other 14 pointers are zero"]) {
      expect(text).toContain(s);
    }
    const data = vol.sector(1111);
    expect(new TextDecoder().decode(data.slice(0, HELLO_TEXT.length))).toBe(HELLO_TEXT);
    expect(data.slice(HELLO_TEXT.length).every((b) => b === 0)).toBe(true);
    const follow = stepText("Following the pointer");
    expect(follow).toContain(`Block 1111 holds the file's ${HELLO_TEXT.length} bytes, ${HELLO_TEXT}, and ${n(1024 - HELLO_TEXT.length)} zeros`);
    expect(follow).toContain("times 1,024");
  });

  it("quotes bigger.txt's thirteen blocks and its single-indirect block", () => {
    const { vol } = runThrough("One disk, two block groups");
    expect(BIGGER_BYTES).toBe(13 * 1024);
    const blocks = vol.fileBlocks(BIGGER);
    expect(blocks.data).toEqual(Array.from({ length: 13 }, (_, i) => 1112 + i));
    expect(blocks.indirect).toEqual([{ block: 1125, level: 1 }]);
    const ino = vol.extInode(13);
    expect(ino.block.slice(0, 12)).toEqual(blocks.data.slice(0, 12));
    expect(ino.block[12]).toBe(1125);
    expect(u32(vol.sector(6).slice(0x200, 0x280), 0x58)).toBe(1125);
    const pointers = vol.sector(1125);
    expect(u32(pointers, 0)).toBe(1124);
    expect(pointers.slice(4).every((b) => b === 0)).toBe(true);
    const text = stepText("A 13th block needs an indirect block");
    for (const s of [`${n(BIGGER_BYTES)} bytes, 13 blocks: 1112 to 1124`, "only 12 direct pointers", "inode 13's 13th pointer (+0x58)", "single-indirect block, 1125", `${1024 / 4} four-byte block numbers`, "1124, the file's 13th block"]) {
      expect(text).toContain(s);
    }
  });

  it("quotes the journal's blocks, inode, and superblock magic", () => {
    const { vol } = runThrough("One disk, two block groups");
    const info = vol.journalInfo()!;
    expect(info).toMatchObject({ inode: 8, firstBlock: 82, maxlen: 1024 });
    expect(vol.layout().find((r) => r.kind === "journal")?.sectors).toEqual({ start: 82, end: 1106 });
    expect(vol.blockOwners().filter((o) => o.path === "<journal>" && o.role === "indirect").map((o) => o.block)).toEqual([1106, 1107, 1108, 1109, 1110]);
    expect(Array.from(vol.sector(82).slice(0, 4))).toEqual([0xc0, 0x3b, 0x39, 0x98]);
    const text = stepText("The journal");
    for (const s of ["Blocks 82 to 1105", `${n(info.maxlen)} blocks of a hidden file, inode ${info.inode}`, "pointer blocks are 1106 to 1110", "Block 82 is the journal's superblock (magic C0 3B 39 98)"]) {
      expect(text).toContain(s);
    }
  });

  it("quotes group 1's backup superblock, tables, and free blocks", () => {
    const { vol } = runThrough("One disk, two block groups");
    const g = vol.extGeometry();
    const g1 = g.groups[1];
    expect(g1).toMatchObject({ firstBlock: 8193, superblockBlock: 8193, descriptorsBlock: 8194, blockBitmap: 8195, inodeBitmap: 8196, inodeTable: 8197, firstData: 8261, freeBlocks: 8123 });
    expect(g1.inodeTable + g.inodeTableBlocks - 1).toBe(8260);
    expect(g.totalBlocks - g1.firstData).toBe(g1.freeBlocks);
    const backup = vol.sector(8193);
    expect([backup[0x38], backup[0x39]]).toEqual([0x53, 0xef]);
    expect(vol.layout().filter((r) => r.sectors.start >= 8193).map((r) => r.name)).toEqual(["backup superblock (group 1)", "group descriptors (group 1)", "block bitmap (group 1)", "inode bitmap (group 1)", "inode table (group 1)", "data (group 1)"]);
    const text = stepText("Group 1 keeps a backup");
    for (const s of ["begins at block 8193", "the same 53 EF at +0x38", "in block 8194, of the descriptors", "blocks 8195 and 8196", "inode table 8197 to 8260", `8261 onward, are all free: ${n(g1.freeBlocks)} of them`]) {
      expect(text).toContain(s);
    }
  });

  it("resolves every step's focus against the live volume", () => {
    const { fs } = runThrough("One disk, two block groups");
    const focuses = scenario.steps.map((s) => [s.title, focusOf(s.title, fs)]);
    expect(focuses).toEqual([
      ["One disk, two block groups", { sector: 0 }],
      ["Block 1 is the superblock", { offset: 1024 + 0x38 }],
      ["The group descriptors", { sector: 2 }],
      ["The block bitmap", { offset: 3 * 1024 + 0x8c }],
      ["The inode bitmap", { sector: 4 }],
      ["The inode table", { offset: 5 * 1024 + 0x80 }],
      ["The root directory", { path: "/", offset: 69 * 1024 }],
      ["hello.txt's inode", { path: HELLO, offset: 6 * 1024 + 0x180 }],
      ["Following the pointer", { path: HELLO, sector: 1111 }],
      ["A 13th block needs an indirect block", { path: BIGGER, sector: 1125 }],
      ["The journal", { path: null, sector: 82 }],
      ["Group 1 keeps a backup", { sector: 8193 }],
      ["Putting it together", { path: null, sector: 1 }],
    ]);
  });
});
```

- [ ] **Step 3: Run it and watch it fail.**

```sh
pnpm exec vitest run tests/ext-fundamentals.test.ts
```

Expected: `FAIL  tests/ext-fundamentals.test.ts`, `Error: Cannot find module '../src/scenarios/extFundamentals'`, `Test Files  1 failed (1)`, `Tests  no tests`.

- [ ] **Step 4: Write the lesson.** Create `web/ui/src/scenarios/extFundamentals.ts`:

```ts
import type { OpRecord } from "../lib/wasm";
import { asExt } from "../fs/ext";
import type { Scenario } from "../state/scenarios.svelte";
import { biggerText } from "./fundamentals";

/**
 * The ext tour: the on-disk structure of the default ext3 volume (16,384 blocks of 1 KiB, two
 * block groups) and how a name reaches its bytes through an inode. The first step creates the
 * two files the tour points at; every other step only moves the dump. The numbers quoted in the
 * copy are the default disk's, and tests/ext-fundamentals.test.ts checks every one of them
 * against a freshly formatted Volume.
 *
 * Inode slots have no generic adapter method, so this file reaches past the seam with
 * `asExt(fs).inodeOffset(ino)`; the boundary test allows `src/scenarios/` to import `fs/ext`.
 */

export const HELLO = "/hello.txt";
export const HELLO_TEXT = "Hello, ext3!";
export const BIGGER = "/bigger.txt";
/** Thirteen blocks: one more than an inode's twelve direct pointers, so the file needs a
 *  single-indirect block. */
export const BIGGER_BYTES = 13312;
export { biggerText };

/** Two creates as one timeline step: the changes and events of `a` then `b`, in the order the
 *  core made them, so rewinding the step undoes both. */
function both(a: OpRecord, b: OpRecord): OpRecord {
  return { op: `${a.op}; ${b.op}`, changes: [...a.changes, ...b.changes], events: [...a.events, ...b.events] };
}

export const scenario: Scenario = {
  id: "ext-fundamentals",
  title: "The fundamentals (ext)",
  summary: "Tour the block groups of an ext3 disk and follow a name through its inode to its blocks.",
  family: "ext",
  steps: [
    {
      title: "One disk, two block groups",
      text: "This volume is 16,384 blocks of 1,024 bytes: 16 MB. ext cuts the blocks into block groups of 8,192, each with its own bitmaps and inode table beside the blocks they describe; this disk has two, and the block-group map on the left draws them as two bands. Block 0, the boot block, belongs to neither: it is left all zero for a boot loader, which is why group 0 starts at block 1 and group 1 is one block short, 8,191. hello.txt and bigger.txt are already on the disk so there is something to point at.",
      action: (v) => both(v.createFile(HELLO, new TextEncoder().encode(HELLO_TEXT)), v.createFile(BIGGER, biggerText(BIGGER_BYTES))),
      focus: { sector: 0 },
    },
    {
      title: "Block 1 is the superblock",
      text: "The superblock counts the whole disk. At +0x38 is the magic number, 53 EF, which is 0xEF53 read little-endian: that is how a tool knows the disk is ext. The first fields are 1,024 inodes (+0x00) and 16,384 blocks (+0x04); +0x0C and +0x10 hold the free counts, 15,190 blocks and 1,011 inodes now that the two files exist. Point at any field and the Inspector decodes it.",
      focus: { offset: 1024 + 0x38 },
    },
    {
      title: "The group descriptors",
      text: "Block 2 holds one 32-byte descriptor per group. Group 0's, at +0x00, names its block bitmap (block 3), its inode bitmap (block 4), and the first block of its inode table (block 5), then its free counts: 7,067 blocks and 499 inodes. Group 1's, at +0x20, names blocks 8195, 8196, and 8197. A driver reads this block to find any group's tables without scanning the disk.",
      focus: { sector: 2 },
    },
    {
      title: "The block bitmap",
      text: "Block 3 is group 0's block bitmap: one bit per block, starting with block 1, set when the block is in use. Blocks 1 to 1110 are taken before any file exists: the group's metadata, the root directory and lost+found (69 to 81), and the journal (82 to 1110). The two files took the next 15, 1111 to 1125, so the bitmap reads 140 bytes of FF and then, at +0x8C, 1F: 1,125 bits set.",
      focus: { offset: 3 * 1024 + 0x8c },
    },
    {
      title: "The inode bitmap",
      text: "Block 4 does the same for group 0's 512 inodes, starting with inode 1. Inodes 1 to 10 are reserved (2 is the root directory, 8 the journal), 11 is lost+found, and the two files took 12 and 13, so the bitmap starts FF 1F: 13 bits set. 512 inodes need only 64 bytes; the rest of the block is padded with ones (+0x40 onward reads FF), so no inode past 512 can ever be handed out.",
      focus: { sector: 4 },
    },
    {
      title: "The inode table",
      text: "Blocks 5 to 68 are group 0's inode table: 512 inodes of 128 bytes, eight to a block. Inode N sits (N − 1) × 128 bytes into the table, so inode 2, the root directory, is at block 5 + 0x80, and inode 11, lost+found, at block 6 + 0x100. An inode holds a file's type and permissions, its size, its timestamps, and 15 block pointers. It does not hold the name.",
      focus: (fs) => ({ offset: asExt(fs).inodeOffset(2) }),
    },
    {
      title: "The root directory",
      text: "Inode 2's first block pointer reads 69, and block 69 is the root directory: a run of entries, each an inode number, a record length (rec_len), a name length, a type, and the name. . and .. (both inode 2) take 12 bytes each, lost+found 20, hello.txt 20, and bigger.txt the remaining 960: the last entry's rec_len runs to the end of the block, which is how a directory records its free space.",
      focus: (fs) => ({ path: "/", offset: fs.dataStart("/") ?? undefined }),
    },
    {
      title: "hello.txt's inode",
      text: "The entry for hello.txt names inode 12, at block 6 + 0x180. Its size field (+0x04) reads 12, and its first block pointer (+0x28) reads 1111, the first block the format left free; the other 14 pointers are zero.",
      focus: (fs) => ({ path: HELLO, offset: asExt(fs).inodeOffset(12) }),
    },
    {
      title: "Following the pointer",
      text: "Block 1111 holds the file's 12 bytes, Hello, ext3!, and 1,012 zeros. Reading a file is three hops: the directory entry gives the inode number, the inode gives the block numbers, and a block number times 1,024 is the byte offset on the disk.",
      focus: (fs) => ({ path: HELLO, sector: fs.chain(HELLO)[0] }),
    },
    {
      title: "A 13th block needs an indirect block",
      text: "bigger.txt is 13,312 bytes, 13 blocks: 1112 to 1124. An inode has only 12 direct pointers, so inode 13's 13th pointer (+0x58) names a single-indirect block, 1125, allocated after the data. That block is an array of 256 four-byte block numbers, and only the first is used: 1124, the file's 13th block. The block-group map marks the indirect block with a dot.",
      focus: (fs) => ({ path: BIGGER, sector: fs.owners.find((o) => o.path === BIGGER && o.role === "indirect")?.unit }),
    },
    {
      title: "The journal",
      text: "Blocks 82 to 1105 are the journal: 1,024 blocks of a hidden file, inode 8, whose own pointer blocks are 1106 to 1110. Block 82 is the journal's superblock (magic C0 3B 39 98); the rest is a ring the driver writes every metadata change to before it touches the change's home block. The Journal panel under the map draws the ring, and \"A journaled write\" follows one change through it.",
      focus: (fs) => ({ path: null, sector: asExt(fs).journal!.state().firstBlock }),
    },
    {
      title: "Group 1 keeps a backup",
      text: "Group 1 begins at block 8193 with a backup of the superblock (the same 53 EF at +0x38) and, in block 8194, of the descriptors, so a damaged block 1 or 2 can be rebuilt. Its own bitmaps are blocks 8195 and 8196 and its inode table 8197 to 8260. Its data blocks, 8261 onward, are all free: 8,123 of them.",
      focus: { sector: 8193 },
    },
    {
      title: "Putting it together",
      text: "Every path through this disk starts at block 1: the superblock sizes the groups, the descriptors in block 2 locate each group's bitmaps and inode table, inode 2 names the root directory's block, a directory entry maps a name to an inode, and the inode's pointers, direct or through an indirect block, name the data. The journal sits beside all of it, so a change to any of those blocks can be finished or thrown away after a crash.",
      focus: { path: null, sector: 1 },
    },
  ],
};
```

- [ ] **Step 5: Run the test again.**

```sh
pnpm exec vitest run tests/ext-fundamentals.test.ts
```

Expected: `Tests  14 passed (14)`. (Comparing two 16 MiB images with `toEqual` runs the worker out of memory when they differ, which is why the test uses `firstDiff`.)

#### Round 2: a journaled write

- [ ] **Step 6: Write the failing test.** Create `web/ui/tests/journaled-write.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { OpRecord, Volume } from "../src/lib/wasm";
import { FAMILIES } from "../src/fs";
import type { FsAdapter } from "../src/fs/adapter";
import { biggerText } from "../src/scenarios/fundamentals";
import { COMMIT_INDEX, COPY_COUNT, DESCRIPTOR_INDEX, FLAG_WORD, NOTES_BYTES, NOTES_PATH, S_SEQUENCE, S_START, scenario } from "../src/scenarios/journaledWrite";
import { lessonHelpers } from "./fixtures/lesson";

// "A journaled write" follows one create through the journal and quotes where each piece
// landed (descriptor at block 83, seven copies, commit at block 91, ...). Those positions are
// read here from the create's own record, the events and byte changes the core reported, so the
// lesson's claims are the core's, not a guess about how it lays a transaction out.

const n = (v: number) => v.toLocaleString("en-US");
const be32 = (b: Uint8Array, at: number) => new DataView(b.buffer, b.byteOffset, b.byteLength).getUint32(at, false);
const { stepText, runThrough, focusOf } = lessonHelpers(scenario);

/** The whole lesson, run the way the runner does. Only the first step has an action. */
function run(): { vol: Volume; fs: FsAdapter; rec: OpRecord } {
  const { vol, fs, records } = runThrough();
  expect(records).toHaveLength(1);
  return { vol, fs, rec: records[0] };
}

/** The journal blocks the record wrote, from its `journal_block_written` events. */
function journalWrites(rec: OpRecord) {
  return rec.events.filter((e) => e.kind === "journal_block_written").map((e) => {
    const m = /^(?:wrote descriptor for transaction (\d+)|copied block (\d+) into|committed transaction (\d+)) .*journal block (\d+) \(block (\d+)\)$/.exec(e.text);
    if (!m) throw new Error(`unexpected event text: ${e.text}`);
    const kind = m[1] ? "descriptor" : m[2] ? "copy" : "commit";
    return { kind, home: m[2] ? Number(m[2]) : null, index: Number(m[4]), block: Number(m[5]) };
  });
}

describe("the journaled-write lesson", () => {
  it("runs on the default ext3 disk in ordered mode", () => {
    expect(scenario.id).toBe("journaled-write");
    expect(scenario.title).toBe("A journaled write");
    expect(scenario.family).toBe("ext");
    const vol = FAMILIES.ext.format();
    expect(vol.fsType()).toBe("ext3");
    expect(vol.journalInfo()).toMatchObject({ mode: "ordered", firstBlock: 82, start: 0, sequence: 1 });
    expect(stepText("One create, one transaction")).toContain("in ordered mode");
  });

  it("creates the quoted file in three blocks", () => {
    const { vol, rec } = run();
    expect(rec.op).toBe(`create_file ${NOTES_PATH}`);
    expect(vol.readFile(NOTES_PATH)).toEqual(biggerText(NOTES_BYTES));
    expect(vol.fileBlocks(NOTES_PATH).data).toEqual([1111, 1112, 1113]);
    expect(NOTES_BYTES - 2 * 1024).toBe(952);
    expect(stepText("One create, one transaction")).toContain(`created ${NOTES_PATH}, ${n(NOTES_BYTES)} bytes`);
    const data = stepText("Data first");
    expect(data).toContain("home blocks, 1111 to 1113");
    expect(data).toContain(`${n(NOTES_BYTES)} bytes fill two blocks and 952 bytes of the third`);
  });

  it("sets the needs_recovery flag first and clears it last", () => {
    const { vol, rec } = run();
    const flag = rec.changes.filter((c) => c.offset === FLAG_WORD);
    expect(flag.map((c) => [c.before[0], c.after[0]])).toEqual([[0x02, 0x06], [0x06, 0x02]]);
    expect(rec.changes[0].offset).toBe(FLAG_WORD);
    expect(rec.changes[rec.changes.length - 1].offset).toBe(FLAG_WORD);
    expect(vol.sector(1)[0x60]).toBe(0x02);
    expect(vol.annotateSector(1).find((a) => a.range.start === 0x60)?.label).toBe("incompatible features");
    const firstJournal = rec.changes.findIndex((c) => c.offset >= 82 * 1024 && c.offset < 1106 * 1024);
    expect(firstJournal).toBeGreaterThan(0);
    const text = stepText("One create, one transaction");
    expect(text).toContain("block 1 + 0x60 reads 0x02 now, but while the create ran it read 0x06: bit 0x04 is needs_recovery");
    expect(stepText("The flag clears")).toContain("0x06 back to 0x02");
  });

  it("writes the data blocks before anything touches the journal", () => {
    const { rec } = run();
    const at = (block: number) => rec.changes.findIndex((c) => Math.floor(c.offset / 1024) === block);
    const firstJournal = rec.changes.findIndex((c) => c.offset >= 82 * 1024 && c.offset < 1106 * 1024);
    for (const b of [1111, 1112, 1113]) expect(at(b)).toBeLessThan(firstJournal);
    const events = rec.events.map((e) => e.kind);
    expect(events.indexOf("data_written")).toBeLessThan(events.indexOf("transaction_started"));
  });

  it("moves s_start to 1 for the transaction and back to 0, and s_sequence to 2", () => {
    const { vol, rec } = run();
    const at = (label: string) => vol.annotateSector(82).find((a) => a.label === label)?.range.start;
    expect([at("sequence"), at("start")]).toEqual([S_SEQUENCE, S_START]);
    // Both journal-superblock writes cover s_sequence and s_start (8 bytes at +0x18).
    const jsb = rec.changes.filter((c) => c.offset === 82 * 1024 + S_SEQUENCE);
    expect(jsb.map((c) => [be32(c.before, 4), be32(c.after, 4)])).toEqual([[0, 1], [1, 0]]);
    expect(jsb.map((c) => be32(c.after, 0))).toEqual([1, 2]);
    expect(vol.journalInfo()).toMatchObject({ sequence: 2, start: 0 });
    const started = rec.events.find((e) => e.kind === "transaction_started")!;
    expect(started.text).toBe(`started transaction 1 at journal block ${DESCRIPTOR_INDEX} (${COPY_COUNT} tagged blocks)`);
    expect(started.region).toEqual({ start: 82 * 1024 + S_SEQUENCE, end: 82 * 1024 + S_SEQUENCE + 8 });
    const text = stepText("The journal superblock");
    for (const s of ["block 82, the journal's own superblock", "s_start field, at +0x1C, went from 0 to 1", "starts at journal block 1", "It reads 0 again now", "transaction_started event"]) {
      expect(text).toContain(s);
    }
    expect(stepText("The journal empties")).toContain("s_sequence at +0x18 becomes 2");
    expect(stepText("The journal empties")).toContain("s_start at +0x1C goes back to 0");
  });

  it("writes one descriptor, seven copies in tag order, and a commit, where the lesson says", () => {
    const { vol, rec } = run();
    const writes = journalWrites(rec);
    expect(writes.map((w) => w.kind)).toEqual(["descriptor", ...new Array(COPY_COUNT).fill("copy"), "commit"]);
    expect(writes[0]).toMatchObject({ index: DESCRIPTOR_INDEX, block: 83 });
    expect(writes.slice(1, -1).map((w) => w.block)).toEqual([84, 85, 86, 87, 88, 89, 90]);
    expect(writes.slice(1, -1).map((w) => w.home)).toEqual([1, 2, 3, 4, 5, 6, 69]);
    expect(writes[writes.length - 1]).toMatchObject({ index: COMMIT_INDEX, block: 91 });
    expect(COMMIT_INDEX).toBe(9);
    // The descriptor's tags, as the Inspector decodes them: the seven homes, ascending.
    const tags = vol.annotateSector(83).filter((a) => a.label.startsWith("tag ")).map((a) => Number(/^block (\d+)/.exec(a.value)![1]));
    expect(tags).toEqual([1, 2, 3, 4, 5, 6, 69]);
    expect(Array.from(vol.sector(83).slice(0, 12))).toEqual([0xc0, 0x3b, 0x39, 0x98, 0, 0, 0, 1, 0, 0, 0, 1]);
    // The two inode-table blocks: inode 2 in block 5, inode 12 in block 6.
    expect([vol.extInode(2).slot.block, vol.extInode(12).slot.block]).toEqual([5, 6]);
    // The first copy is the superblock with the flag set; the commit is type 2 for transaction 1.
    expect(vol.sector(84)[0x60]).toBe(0x06);
    expect(Array.from(vol.sector(91).slice(0, 12))).toEqual([0xc0, 0x3b, 0x39, 0x98, 0, 0, 0, 2, 0, 0, 0, 1]);
    const desc = stepText("The descriptor");
    for (const s of ["Journal block 1 is block 83", "magic C0 3B 39 98, block type 1, transaction 1", "in ascending order: 1, 2, 3, 4, 5, 6, and 69", "inodes 2 and 12", `${COPY_COUNT} tags`]) {
      expect(desc).toContain(s);
    }
    const copies = stepText("The copies");
    expect(copies).toContain(`Blocks 84 to 90 are the ${COPY_COUNT} copies, in tag order`);
    expect(copies).toContain("Block 84 is the superblock's image, and at +0x60 it reads 0x06");
    const commit = stepText("The commit");
    expect(commit).toContain("Block 91, journal block 9, is the commit block: the magic, block type 2, transaction 1");
  });

  it("checkpoints after the commit, naming the file in block 69", () => {
    const { fs, vol, rec } = run();
    const commitAt = rec.events.findIndex((e) => e.kind === "journal_block_written" && e.text.startsWith("committed"));
    const checkpoints = rec.events.map((e, i) => [e, i] as const).filter(([e]) => e.kind === "checkpointed");
    expect(checkpoints).toHaveLength(COPY_COUNT);
    for (const [, i] of checkpoints) expect(i).toBeGreaterThan(commitAt);
    expect(checkpoints.map(([e]) => Number(/^checkpointed block (\d+)/.exec(e.text)![1]))).toEqual([1, 2, 3, 4, 5, 6, 69]);
    const entry = vol.dirEntries("/").find((e) => e.name === "notes.txt")!;
    expect(entry).toMatchObject({ block: 69, offset: 69 * 1024 + 0x2c, inode: 12 });
    expect(stepText("Checkpoint")).toContain(`write the ${COPY_COUNT} blocks to their home locations`);
    expect(stepText("Checkpoint")).toContain("Block 69, the root directory, now names notes.txt at +0x2C, inode 12");
    // One transaction per operation, checkpointed at once: the journal is empty and the flag
    // clear when the create returns, which the copy calls a cleanly unmounted disk.
    expect(vol.journalInfo()).toMatchObject({ start: 0, needsRecovery: false });
    expect(stepText("Checkpoint")).toContain("this emulator checkpoints each transaction at once");
    expect(fs.chain("/")).toEqual([69]);
  });

  it("leaves transaction 1 stale at journal blocks 1 to 9", () => {
    const { fs } = run();
    const ring = fs.journal!.blocks();
    const tx = ring.filter((b) => b.tid === 1);
    expect(tx.map((b) => b.index)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9]);
    expect(tx.every((b) => b.stale)).toBe(true);
    expect(stepText("The flag clears")).toContain("transaction 1 at journal blocks 1 to 9, faded because it is stale");
  });

  it("resolves every step's focus against the live volume", () => {
    const { fs } = run();
    expect(scenario.steps.map((s) => [s.title, focusOf(s.title, fs)])).toEqual([
      ["One create, one transaction", { path: NOTES_PATH, offset: 1024 + 0x60 }],
      ["Data first", { path: NOTES_PATH, sector: 1111 }],
      ["The journal superblock", { offset: 82 * 1024 + 0x1c }],
      ["The descriptor", { sector: 83 }],
      ["The copies", { sector: 84 }],
      ["The commit", { sector: 91 }],
      ["Checkpoint", { path: NOTES_PATH, offset: 69 * 1024 + 0x2c }],
      ["The journal empties", { offset: 82 * 1024 + 0x18 }],
      ["The flag clears", { path: NOTES_PATH, offset: 1024 + 0x60 }],
    ]);
  });
});
```

- [ ] **Step 7: Run it and watch it fail.**

```sh
pnpm exec vitest run tests/journaled-write.test.ts
```

Expected: `Error: Cannot find module '../src/scenarios/journaledWrite'`, `Test Files  1 failed (1)`.

- [ ] **Step 8: Write the lesson.** Create `web/ui/src/scenarios/journaledWrite.ts`:

```ts
import { asExt } from "../fs/ext";
import type { FsAdapter } from "../fs/adapter";
import type { Scenario } from "../state/scenarios.svelte";
import { biggerText } from "./fundamentals";

/**
 * One create on the ordered-mode ext3 disk, followed through the journal in the order its bytes
 * were written. The first step runs the create; every later step only moves the dump, and the
 * Step strip's events list the writes in the same order. The dump shows the disk after the
 * create, so steps about a moment mid-transaction say what the bytes read then and what they
 * read now. tests/journaled-write.test.ts pins every quoted number, and the journal positions
 * below, against the create's own record.
 */

export const NOTES_PATH = "/notes.txt";
/** Three blocks: two full and 952 bytes of a third. */
export const NOTES_BYTES = 3000;
/** The superblock's incompatible-features word: block 1 + 0x60. */
export const FLAG_WORD = 1024 + 0x60;
/** The journal superblock's `s_sequence` and `s_start` fields. */
export const S_SEQUENCE = 0x18;
export const S_START = 0x1c;
/** Where the transaction sits in the journal (indexes from the journal's first block): the
 *  descriptor, the seven copies after it, then the commit. */
export const DESCRIPTOR_INDEX = 1;
export const COPY_COUNT = 7;
export const COMMIT_INDEX = DESCRIPTOR_INDEX + COPY_COUNT + 1;

/** The journal's first block (82 on the default disk), from the capability. */
const journalStart = (fs: FsAdapter) => asExt(fs).journal!.state().firstBlock;

export const scenario: Scenario = {
  id: "journaled-write",
  title: "A journaled write",
  summary: "Create one file on ext3 and follow its metadata through the journal: descriptor, copies, commit, checkpoint.",
  family: "ext",
  steps: [
    {
      title: "One create, one transaction",
      text: "This step created /notes.txt, 3,000 bytes, on an ext3 disk in ordered mode. The superblock's incompatible-features word at block 1 + 0x60 reads 0x02 now, but while the create ran it read 0x06: bit 0x04 is needs_recovery, set before the first journal write and cleared after the last, so a crash in between leaves a flag that says so. The Step strip lists the create's events in the order they happened; the next steps follow them.",
      action: (v) => v.createFile(NOTES_PATH, biggerText(NOTES_BYTES)),
      focus: { path: NOTES_PATH, offset: FLAG_WORD },
    },
    {
      title: "Data first",
      text: "In ordered mode the file's data goes straight to its home blocks, 1111 to 1113, before anything touches the journal: 3,000 bytes fill two blocks and 952 bytes of the third. If the machine stopped here the blocks would hold the bytes, but no inode or directory entry would point at them yet.",
      focus: (fs) => ({ path: NOTES_PATH, sector: fs.chain(NOTES_PATH)[0] }),
    },
    {
      title: "The journal superblock",
      text: "Next the driver wrote block 82, the journal's own superblock. Its s_start field, at +0x1C, went from 0 to 1: a live transaction starts at journal block 1. It reads 0 again now because the transaction finished; the transaction_started event is the moment it read 1.",
      focus: (fs) => ({ offset: journalStart(fs) * 1024 + S_START }),
    },
    {
      title: "The descriptor",
      text: "Journal block 1 is block 83, the descriptor: the magic C0 3B 39 98, block type 1, transaction 1, then one tag for each metadata block the create changed, in ascending order: 1, 2, 3, 4, 5, 6, and 69. That is the superblock, the group descriptors, both bitmaps, the two inode-table blocks holding inodes 2 and 12, and the root directory: 7 tags.",
      focus: (fs) => ({ sector: journalStart(fs) + DESCRIPTOR_INDEX }),
    },
    {
      title: "The copies",
      text: "Blocks 84 to 90 are the 7 copies, in tag order: whole-block images of the new metadata, written to the journal while the home blocks still hold the old versions. Block 84 is the superblock's image, and at +0x60 it reads 0x06, because the flag was already set when the copy was taken.",
      focus: (fs) => ({ sector: journalStart(fs) + DESCRIPTOR_INDEX + 1 }),
    },
    {
      title: "The commit",
      text: "Block 91, journal block 9, is the commit block: the magic, block type 2, transaction 1. Until it is on disk the transaction does not count. Recovery throws away a transaction with no commit block and replays one that has it.",
      focus: (fs) => ({ sector: journalStart(fs) + COMMIT_INDEX }),
    },
    {
      title: "Checkpoint",
      text: "Only after the commit does the driver write the 7 blocks to their home locations: the checkpoint. Block 69, the root directory, now names notes.txt at +0x2C, inode 12. A crash from here on loses nothing, because the journal still holds every copy. A kernel would put the checkpoint off and batch many transactions into one; this emulator checkpoints each transaction at once, so every step leaves a cleanly unmounted disk.",
      focus: (fs) => ({ path: NOTES_PATH, offset: asExt(fs).dirEntryOffset(NOTES_PATH) ?? undefined }),
    },
    {
      title: "The journal empties",
      text: "With every block home, the journal superblock is written once more: s_sequence at +0x18 becomes 2, the number the next transaction will use, and s_start at +0x1C goes back to 0, which means there is nothing to replay.",
      focus: (fs) => ({ offset: journalStart(fs) * 1024 + S_SEQUENCE }),
    },
    {
      title: "The flag clears",
      text: "Last, the superblock's needs_recovery bit clears: 0x06 back to 0x02. The Journal panel draws transaction 1 at journal blocks 1 to 9, faded because it is stale: already checkpointed, kept only until the next transaction writes over it.",
      focus: { path: NOTES_PATH, offset: FLAG_WORD },
    },
  ],
};
```

- [ ] **Step 9: Run the test again.**

```sh
pnpm exec vitest run tests/journaled-write.test.ts
```

Expected: `Tests  9 passed (9)`.

#### Round 3: crash and recover

- [ ] **Step 10: Write the failing test.** Create `web/ui/tests/crash-recover.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import type { Volume } from "../src/lib/wasm";
import { FAMILIES } from "../src/fs";
import { biggerText } from "../src/scenarios/fundamentals";
import { CRASH_BYTES, CRASH_PATH, LOST_BLOCK, LOST_PATH, LOST_TEXT, scenario } from "../src/scenarios/crashRecover";
import { lessonHelpers } from "./fixtures/lesson";

// "Crash and recover" arms two crashes and recovers from each. These tests run it the way the
// runner does and pin, after every action, the state the copy describes (the flag, the root
// directory, the bitmap, the journal ring, the orphaned bytes), every quoted number, and every
// step's resolved focus.

const { stepText, runThrough, focusOf } = lessonHelpers(scenario);
const names = (vol: Volume) => vol.dirEntries("/").map((e) => e.name);
/** Group 0's block-bitmap bit for block `b` (bit 0 is block 1). */
const inUse = (vol: Volume, b: number) => ((vol.sector(3)[Math.floor((b - 1) / 8)] >> ((b - 1) % 8)) & 1) === 1;

describe("the crash-recover lesson", () => {
  it("runs on the default ext3 disk", () => {
    expect(scenario.id).toBe("crash-recover");
    expect(scenario.title).toBe("Crash and recover");
    expect(scenario.family).toBe("ext");
    expect(FAMILIES.ext.format().journalInfo()).toMatchObject({ mode: "ordered" });
  });

  it("step 1: the create crashes after its commit and the volume needs recovery", () => {
    const { vol, fs, records } = runThrough("Crash after the commit");
    const [rec] = records;
    expect(rec.op).toBe(`create_file ${CRASH_PATH} (crashed after commit)`);
    expect(rec.events[rec.events.length - 1]).toMatchObject({ kind: "crashed", text: "crashed after commit" });
    expect(rec.events.some((e) => e.kind === "checkpointed")).toBe(false);
    expect(fs.needsRecovery).toBe(true);
    expect(vol.needsRecovery()).toBe(true);
    expect(fs.journal!.phase()).toBeNull();
    expect(vol.sector(1)[0x60]).toBe(0x06);
    expect(() => vol.stat(CRASH_PATH)).toThrow(expect.objectContaining({ code: "NotFound" }));
    expect(vol.listDir("/").map((e) => e.name)).toEqual(["lost+found"]);
    expect(Math.ceil(CRASH_BYTES / 1024)).toBe(2);
    const text = stepText("Crash after the commit");
    for (const s of ["armed a crash after commit", `created ${CRASH_PATH}, two blocks`, "block 1 + 0x60 reads 0x06", "does not list crash.txt"]) {
      expect(text).toContain(s);
    }
  });

  it("step 2: transaction 1 is live in the journal at blocks 83 to 91", () => {
    const { vol, fs } = runThrough("Crash after the commit");
    expect(fs.journal!.state()).toMatchObject({ firstBlock: 82, start: 1, sequence: 1, needsRecovery: true });
    const tx = fs.journal!.blocks().filter((b) => b.tid === 1);
    expect(tx.map((b) => [b.index, b.block, b.kind])).toEqual([
      [1, 83, "descriptor"], [2, 84, "copy"], [3, 85, "copy"], [4, 86, "copy"], [5, 87, "copy"], [6, 88, "copy"], [7, 89, "copy"], [8, 90, "copy"], [9, 91, "commit"],
    ]);
    expect(tx.every((b) => !b.stale)).toBe(true);
    expect(vol.annotateSector(83).filter((a) => a.label.startsWith("tag "))).toHaveLength(7);
    const text = stepText("A live transaction");
    for (const s of ["Journal block 1, block 83, is transaction 1's descriptor", "the same 7 blocks", "copies are blocks 84 to 90", "commit is block 91"]) {
      expect(text).toContain(s);
    }
  });

  it("step 3: the root directory has no entry yet, and the data blocks are written but free", () => {
    const { vol } = runThrough("Crash after the commit");
    expect(names(vol)).toEqual([".", "..", "lost+found"]);
    expect(vol.dirEntries("/")[2]).toMatchObject({ name: "lost+found", recLen: 1000, offset: 69 * 1024 + 24 });
    expect(24 + 1000).toBe(1024);
    expect(vol.sector(1111)).toEqual(biggerText(CRASH_BYTES).slice(0, 1024));
    expect(vol.sector(1112)).toEqual(biggerText(CRASH_BYTES).slice(1024, 2048));
    expect([inUse(vol, 1111), inUse(vol, 1112)]).toEqual([false, false]);
    const text = stepText("Home blocks still old");
    for (const s of ["Block 69, the root directory", "rec_len of 1000", "no entry for crash.txt", "blocks 1111 and 1112", "calls both free"]) {
      expect(text).toContain(s);
    }
  });

  it("step 4: recovery replays the transaction and the entry appears", () => {
    const { vol, fs, records } = runThrough("Recover: replay");
    const rec = records[1];
    expect(rec.op).toBe("recover");
    expect(rec.events[0]).toMatchObject({ kind: "recovery_scanned", text: "scanned the journal from block 1, sequence 1: 1 committed transaction, 7 tagged blocks" });
    expect(rec.events.filter((e) => e.kind === "replayed")).toHaveLength(7);
    expect(fs.needsRecovery).toBe(false);
    expect(vol.sector(1)[0x60]).toBe(0x02);
    expect(names(vol)).toEqual([".", "..", "lost+found", "crash.txt"]);
    expect(vol.dirEntries("/")[3]).toMatchObject({ offset: 69 * 1024 + 0x2c, inode: 12 });
    expect(fs.chain(CRASH_PATH)).toEqual([1111, 1112]);
    expect([inUse(vol, 1111), inUse(vol, 1112)]).toEqual([true, true]);
    expect(vol.readFile(CRASH_PATH)).toEqual(biggerText(CRASH_BYTES));
    const text = stepText("Recover: replay");
    for (const s of ["from block 1", "1 committed transaction with 7 tagged blocks", "names crash.txt at +0x2C, inode 12", "blocks 1111 and 1112", "back to 0x02"]) {
      expect(text).toContain(s);
    }
  });

  it("step 5: a crash before the commit leaves the data on disk and transaction 3 uncommitted", () => {
    const { vol, fs, records } = runThrough("Crash before the commit");
    const rec = records[2];
    expect(rec.op).toBe(`create_file ${LOST_PATH} (crashed before commit)`);
    expect(rec.events.find((e) => e.kind === "blocks_allocated")?.text).toBe(`allocated 1 block starting at ${LOST_BLOCK} in group 0`);
    expect(rec.events.find((e) => e.kind === "transaction_started")?.text).toBe("started transaction 3 at journal block 1 (7 tagged blocks)");
    expect(rec.events.filter((e) => e.kind === "journal_block_written").map((e) => e.text.split(" ")[0])).toEqual(["wrote", ...new Array(7).fill("copied")]);
    expect(LOST_TEXT.length).toBeLessThanOrEqual(1024);
    expect(fs.needsRecovery).toBe(true);
    expect(new TextDecoder().decode(vol.sector(LOST_BLOCK).slice(0, LOST_TEXT.length))).toBe(LOST_TEXT);
    const text = stepText("Crash before the commit");
    for (const s of ["crash before commit", `${LOST_PATH}, one block`, `block ${LOST_BLOCK} holds the text`, "descriptor and 7 copies", "transaction 3", "no commit block"]) {
      expect(text).toContain(s);
    }
  });

  it("step 6: recovery discards transaction 3 and the block stays free", () => {
    const { vol, fs, records } = runThrough("Recover: discard");
    const rec = records[3];
    expect(rec.op).toBe("recover");
    expect(rec.events.map((e) => e.kind)).toEqual(["recovery_scanned", "transaction_discarded", "journal_emptied", "recovery_flag_cleared"]);
    expect(rec.events[0].text).toContain("0 committed transactions");
    expect(rec.events[1].text).toBe("discarded uncommitted transaction 3 (7 tagged blocks)");
    expect(fs.needsRecovery).toBe(false);
    expect(inUse(vol, LOST_BLOCK)).toBe(false);
    expect(vol.sector(3)[0x8b]).toBe(0x00);
    expect(Math.floor((LOST_BLOCK - 1) / 8)).toBe(0x8b);
    expect(fs.describeUnit(LOST_BLOCK)).toBe("free");
    expect(() => vol.stat(LOST_PATH)).toThrow(expect.objectContaining({ code: "NotFound" }));
    expect(names(vol)).toEqual([".", "..", "lost+found", "crash.txt"]);
    const text = stepText("Recover: discard");
    for (const s of ["0 committed transactions", "discards transaction 3", "byte +0x8B of block 3 still reads 00", `block ${LOST_BLOCK} is free`, "names lost.txt"]) {
      expect(text).toContain(s);
    }
  });

  it("step 7: the orphaned bytes are still in the free block", () => {
    const { vol, fs } = runThrough("Bytes without an owner");
    expect(new TextDecoder().decode(vol.sector(LOST_BLOCK).slice(0, LOST_TEXT.length))).toBe(LOST_TEXT);
    expect(fs.owners.some((o) => o.unit === LOST_BLOCK)).toBe(false);
    expect(stepText("Bytes without an owner")).toContain(`Block ${LOST_BLOCK} still holds the text`);
  });

  it("resolves every step's focus against the volume that step leaves", () => {
    const at = (title: string) => focusOf(title, runThrough(title).fs);
    expect(at("Crash after the commit")).toEqual({ path: null, offset: 1024 + 0x60 });
    expect(at("A live transaction")).toEqual({ sector: 83 });
    expect(at("Home blocks still old")).toEqual({ offset: 69 * 1024 });
    expect(at("Recover: replay")).toEqual({ path: CRASH_PATH, offset: 69 * 1024 + 0x2c });
    expect(at("Crash before the commit")).toEqual({ path: null, sector: LOST_BLOCK });
    expect(at("Recover: discard")).toEqual({ offset: 3 * 1024 + 0x8b });
    expect(at("Bytes without an owner")).toEqual({ path: null, sector: LOST_BLOCK });
  });
});
```

- [ ] **Step 11: Run it and watch it fail.**

```sh
pnpm exec vitest run tests/crash-recover.test.ts
```

Expected: `Error: Cannot find module '../src/scenarios/crashRecover'`, `Test Files  1 failed (1)`.

- [ ] **Step 12: Write the lesson.** Create `web/ui/src/scenarios/crashRecover.ts` (it takes `FLAG_WORD` and `DESCRIPTOR_INDEX` from Step 8's `journaledWrite.ts`):

```ts
import { asExt } from "../fs/ext";
import type { Scenario } from "../state/scenarios.svelte";
import { biggerText } from "./fundamentals";
import { DESCRIPTOR_INDEX, FLAG_WORD } from "./journaledWrite";

/**
 * Two crashes on the ordered-mode ext3 disk: one after the commit, which recovery replays, and
 * one before it, which recovery throws away. The actions arm the crash through the journal
 * capability and then create a file; the crash fires inside that create, so the step's record
 * holds exactly the writes that reached the disk. tests/crash-recover.test.ts pins every quoted
 * number and the volume's state after each action.
 */

export const CRASH_PATH = "/crash.txt";
/** Two blocks. */
export const CRASH_BYTES = 2048;
export const LOST_PATH = "/lost.txt";
/** One block's worth of text, written by ordered mode before the crash. */
export const LOST_TEXT = "Written before the crash, claimed by nothing.";
/** The block the lost file's data landed in: the next free block after crash.txt's two. */
export const LOST_BLOCK = 1113;

export const scenario: Scenario = {
  id: "crash-recover",
  title: "Crash and recover",
  summary: "Stop an ext3 create mid-transaction twice, and watch recovery replay one and discard the other.",
  family: "ext",
  steps: [
    {
      title: "Crash after the commit",
      text: "This step armed a crash after commit and created /crash.txt, two blocks. The transaction reached the journal, commit block and all, but the machine stopped before a single metadata block went home. The superblock's flag word at block 1 + 0x60 reads 0x06: needs_recovery is set, and the status line says the volume needs recovery. The Files panel does not list crash.txt.",
      action: (v, fs) => {
        asExt(fs).journal!.arm("after_commit");
        return v.createFile(CRASH_PATH, biggerText(CRASH_BYTES));
      },
      focus: { path: null, offset: FLAG_WORD },
    },
    {
      title: "A live transaction",
      text: "Journal block 1, block 83, is transaction 1's descriptor, tagging the same 7 blocks any create changes; its copies are blocks 84 to 90 and its commit is block 91. The Journal panel draws them at full colour: live, committed but not yet checkpointed.",
      focus: (fs) => ({ sector: asExt(fs).journal!.state().firstBlock + DESCRIPTOR_INDEX }),
    },
    {
      title: "Home blocks still old",
      text: "Block 69, the root directory, still ends with lost+found, whose rec_len of 1000 runs to the end of the block: there is no entry for crash.txt. Its data is in blocks 1111 and 1112 (ordered mode wrote it first), but the block bitmap still calls both free.",
      focus: (fs) => ({ offset: fs.dataStart("/") ?? undefined }),
    },
    {
      title: "Recover: replay",
      text: "Recovery scans the journal from block 1, finds 1 committed transaction with 7 tagged blocks, and copies each one home. Block 69 now names crash.txt at +0x2C, inode 12; the bitmap has blocks 1111 and 1112, the flag is back to 0x02, and the Files panel lists the file.",
      action: (_v, fs) => asExt(fs).journal!.recover(),
      focus: (fs) => ({ path: CRASH_PATH, offset: asExt(fs).dirEntryOffset(CRASH_PATH) ?? undefined }),
    },
    {
      title: "Crash before the commit",
      text: "Now a crash before commit, on a create of /lost.txt, one block. Ordered mode writes the data first, so block 1113 holds the text; then the descriptor and 7 copies went to the journal as transaction 3, but no commit block followed. The volume needs recovery again.",
      action: (v, fs) => {
        asExt(fs).journal!.arm("before_commit");
        return v.createFile(LOST_PATH, new TextEncoder().encode(LOST_TEXT));
      },
      focus: { path: null, sector: LOST_BLOCK },
    },
    {
      title: "Recover: discard",
      text: "Recovery finds 0 committed transactions and discards transaction 3: none of its copies go home. In the block bitmap, byte +0x8B of block 3 still reads 00, so block 1113 is free, and no inode or directory entry names lost.txt.",
      action: (_v, fs) => asExt(fs).journal!.recover(),
      focus: (fs) => {
        const g = asExt(fs).geo;
        return { offset: g.groups[0].blockBitmap * g.blockSize + Math.floor((LOST_BLOCK - g.firstDataBlock) / 8) };
      },
    },
    {
      title: "Bytes without an owner",
      text: "Block 1113 still holds the text: the data was written, and the metadata that would have claimed it never was. The next file to allocate this block overwrites it. That is ordered mode's promise: after a crash, metadata never points at garbage, but a block can hold data nobody points at.",
      focus: { path: null, sector: LOST_BLOCK },
    },
  ],
};
```

- [ ] **Step 13: Run the test again.**

```sh
pnpm exec vitest run tests/crash-recover.test.ts
```

Expected: `Tests  9 passed (9)`.

#### Round 4: the registry and the grouped picker

- [ ] **Step 14: Extend `tests/scenarios.test.ts`.** Three edits.

Replace the import on line 4:

```ts
import { all } from "../src/scenarios";
```
with:
```ts
import { all, scenarioGroups } from "../src/scenarios";
```

Insert, directly above the comment `// The runner itself needs runes,` (inside `describe("scenario scripts", ...)`, after the `"every scenario names a registered family"` test):

```ts
  it("every scenario id is unique", () => {
    const ids = all.map((s) => s.id);
    expect(new Set(ids).size).toBe(ids.length);
  });

  // The three ext lessons follow the nine FAT ones, so the picker still opens on the FAT
  // fundamentals and each family's lessons stay together.
  it("lists the FAT lessons first, then the three ext lessons", () => {
    expect(all.map((s) => s.family)).toEqual([...new Array(9).fill("fat16"), "ext", "ext", "ext"]);
    expect(all.slice(9).map((s) => s.id)).toEqual(["ext-fundamentals", "journaled-write", "crash-recover"]);
    expect(all[0].id).toBe("fundamentals");
  });

  // ScenarioPanel renders one <optgroup> per group, labelled with the family's display name.
  it("groups the picker by family in registry order", () => {
    expect(scenarioGroups().map((g) => [g.family, g.label, g.scenarios.map((s) => s.id)])).toEqual([
      ["fat16", "FAT16", all.slice(0, 9).map((s) => s.id)],
      ["ext", "ext", ["ext-fundamentals", "journaled-write", "crash-recover"]],
    ]);
    expect(Object.keys(FAMILIES)).toEqual(["fat16", "ext"]);
    // A family with no lessons gets no empty group.
    expect(scenarioGroups(all.slice(0, 9)).map((g) => g.family)).toEqual(["fat16"]);
  });
```

In `describe("work from the shell", ...)`, replace:

```ts
    it("is registered last, after the eight explorer-driven scenarios", () => {
      expect(all.length).toBe(9);
      expect(all[all.length - 1]).toBe(shell);
```
with:
```ts
    it("is the last FAT lesson, after the eight explorer-driven scenarios", () => {
      const fat = all.filter((s) => s.family === "fat16");
      expect(fat.length).toBe(9);
      expect(fat[fat.length - 1]).toBe(shell);
```

The rest of that test (`shell.id`, `shell.steps.length`, the command pattern) is unchanged.

- [ ] **Step 15: Run it and watch it fail.**

```sh
pnpm exec vitest run tests/scenarios.test.ts
```

Expected: `Tests  2 failed | 18 passed (20)`: "lists the FAT lessons first, then the three ext lessons" (`all` still holds only the nine FAT lessons) and "groups the picker by family in registry order" (`scenarioGroups` is not exported yet).

- [ ] **Step 16: Register the lessons.** Replace `web/ui/src/scenarios/index.ts` with:

```ts
import type { Scenario } from "../state/scenarios.svelte";
import type { FsFamilyId } from "../fs/adapter";
import { FAMILIES } from "../fs";
import { scenario as crashRecover } from "./crashRecover";
import { scenario as directory } from "./directory";
import { scenario as deleteRemnants } from "./deleteRemnants";
import { scenario as extFundamentals } from "./extFundamentals";
import { scenario as fillDisk } from "./fillDisk";
import { scenario as format } from "./format";
import { scenario as fundamentals } from "./fundamentals";
import { scenario as journaledWrite } from "./journaledWrite";
import { scenario as longName } from "./longName";
import { scenario as overwriteGrows } from "./overwriteGrows";
import { scenario as shell } from "./shell";
import { scenario as smallFile } from "./smallFile";

// The fundamentals go first: the picker defaults to the first entry, and the tour is where a
// newcomer should start before watching individual operations. The ext lessons follow the FAT
// ones, the ext tour first for the same reason.
export const all: Scenario[] = [
  fundamentals, format, smallFile, longName, overwriteGrows, deleteRemnants, fillDisk, directory, shell,
  extFundamentals, journaledWrite, crashRecover,
];

export interface ScenarioGroup { family: FsFamilyId; label: string; scenarios: Scenario[] }

/** The picker's `<optgroup>`s: one per registered family that has lessons, in registry order,
 *  labelled with the family's display name, each holding its lessons in `list` order. */
export function scenarioGroups(list: readonly Scenario[] = all): ScenarioGroup[] {
  return Object.values(FAMILIES)
    .map((f) => ({ family: f.id, label: f.name, scenarios: list.filter((s) => s.family === f.id) }))
    .filter((g) => g.scenarios.length > 0);
}
```

- [ ] **Step 17: Group the picker.** Replace `web/ui/src/components/ScenarioPanel.svelte` with:

```svelte
<script lang="ts">
  import { all, scenarioGroups } from "../scenarios";
  import { scenarios } from "../state/scenarios.svelte";

  // One <optgroup> per family (FAT16, then ext), in registry order. The first option overall is
  // still the FAT fundamentals, so the picker's default is unchanged.
  const groups = scenarioGroups();
  let selectedId = $state(all[0]?.id ?? "");

  // The picker only starts a lesson. Everything about the running lesson — the step text,
  // Prev/Next/Close, and the `n`/`p` keys — lives in LessonPanel, a normal panel at the top
  // of the right column: no overlay, nothing dimmed, nothing made inert.
  function startScenario() {
    const s = all.find((s) => s.id === selectedId);
    if (s) scenarios.start(s);
  }
</script>

<div class="scenario-controls">
  <!-- The visible label names the select on its own; an `aria-label` would override it and
       leave the accessible name out of step with the words on screen. -->
  <label class="scenario-label" for="scenario-select">Learning scenarios</label>
  <select id="scenario-select" bind:value={selectedId}>
    {#each groups as g (g.family)}
      <optgroup label={g.label}>
        {#each g.scenarios as s (s.id)}<option value={s.id}>{s.title}</option>{/each}
      </optgroup>
    {/each}
  </select>
  <!-- LessonPanel returns focus here when the card closes, so the id is load-bearing. -->
  <button id="scenario-start" onclick={startScenario}>Start</button>
</div>
```

- [ ] **Step 18: Run the scenarios test and the whole suite.**

```sh
pnpm exec vitest run tests/scenarios.test.ts
pnpm test
```

Expected: `Tests  23 passed (23)` for the scenarios file (its end-to-end loop now also runs the three ext lessons through the runner's shape), then `Test Files  49 passed (49)` and `Tests  485 passed (485)`. `tests/fundamentals.test.ts`'s "is the first scenario, so the picker defaults to it" and `tests/adapterBoundary.test.ts` still pass.

#### Round 5: the docs

- [ ] **Step 19: Update `web/ui/README.md` and `docs/ROADMAP.md`.** Save this patch as `/tmp/t7-docs.patch` and apply it from the repo root with `git apply /tmp/t7-docs.patch` (it is the committed change to both files, with three lines of context):

````diff
diff --git a/docs/ROADMAP.md b/docs/ROADMAP.md
index 635bedb..9075789 100644
--- a/docs/ROADMAP.md
+++ b/docs/ROADMAP.md
@@ -62,10 +62,10 @@ with `Unsupported`. Still to do:
   JavaScript copy of the cluster-count formula, must follow the Rust one (or
   be replaced by a wasm call).
 
-## In progress: `crates/ext`
+## Landed: `crates/ext` and the ext explorer
 
 Decided 2026-09-23, in four slices, each with its spec under
-`docs/superpowers/specs/`. Slices 1 to 3 have landed:
+`docs/superpowers/specs/`. All four have landed:
 
 - **Slice 1, the adapter seam** (`2026-09-23-fs-adapter-design.md`): every
   FAT assumption in `web/ui` sits behind `FsAdapter`; see "What stays
@@ -85,7 +85,8 @@ Decided 2026-09-23, in four slices, each with its spec under
   signature (an image with neither throws `Unsupported`), `fsType()` of
   `"ext2"`, and the `NotFat` / `NotExt` codes (`blockGroupCount`, the one
   ext-only method the spec names for this slice: decision 3, section 8). The
-  explorer refuses an ext image with the status `no adapter for ext2`.
+  explorer refused an ext image with the status `no adapter for ext2`
+  until slice 4.
 - **Slice 3, the ext3 journal** (`2026-09-24-ext3-journal-design.md`, plan
   `docs/superpowers/plans/2026-09-24-ext3-journal.md`): a JBD2 version-2
   journal on the reserved inode 8 (`crates/ext/src/journal/`), in ordered or
@@ -108,23 +109,28 @@ Decided 2026-09-23, in four slices, each with its spec under
   ext-only (`NotExt` elsewhere); the `NeedsRecovery` error code; the
   `journal` region kind in the `RegionKind` union. `fromImage` detection is
   unchanged, with the FAT fallback for an image that carries the ext magic
-  but only parses as FAT. The explorer refuses an ext3 image with `no
-  adapter for ext3`.
-
-Still to do:
-
-- **Slice 4, the explorer:** an ext adapter under `web/ui/src/fs/`, ext-only
-  wasm DTOs (superblock, inodes, block owners), a block-group map that
-  replaces the FAT map through `PANELS`, an inode inspector, the terminal's
-  ext vocabulary, and scenarios that show indirect blocks and, for ext3, a
-  journaled write replaying. For ext3 it adds a journal panel over
-  `journalInfo()` and `journalBlocks()`, the `journal` layout region, the
-  `<journal>` owner of the journal's blocks, and the crash and recovery
-  methods (`armCrash`, `disarmCrash`, `crashPhase`, `needsRecovery`,
-  `recover`). It inherits the `web/ui` seams listed under "Deferred, by
-  area", decides whether an ext volume's labels say "block" instead of
-  "sector", and gives `Unsupported` from `fromImage` its own status text (an
-  unrecognised image now shows the generic "That isn't supported yet.").
+  but only parses as FAT.
+- **Slice 4, the explorer** (`2026-09-24-ext-explorer-design.md`): seven
+  ext-only wasm DTO methods (`extGeometry`, `extSuperblock`, `blockOwners`,
+  `inodeNumber`, `extInode`, `dirEntries`, `fileBlocks`); one `ext` family
+  under `web/ui/src/fs/ext/` binding both ext2 and ext3 (`fsTypes` on the
+  family, the adapter's `name` the volume's type), with journal blocks never
+  units and indirect blocks as owner rows; the seam's sector noun beside the
+  unit noun (ext says block for both: `b:69`, and `i:12` for an inode), the
+  Lesson card's `fileParts`, owner roles and fixed colours, `extras` panels,
+  and the optional journal capability (`FsAdapter.journal`,
+  `needsRecovery`). The explorer mounts ext2 and ext3 volumes, formatted from
+  the Actions panel's Filesystem select or `mkfs --type ext2|ext3`, or loaded
+  from an mke2fs image: a block-group map replaces the FAT map, the Inspector
+  explains inodes, indirect blocks, and journal blocks, and `stat`, `df`, and
+  `seek i:N` speak ext. On ext3 a Journal panel shows the ring with live and
+  stale transactions, arms a crash phase, and recovers; the terminal's
+  `crash` and `recover` do the same, registered only while the volume has a
+  journal (the terminal re-registers its commands when the family changes);
+  the status line explains `NeedsRecovery` and shows `Unsupported`'s own
+  wasm text. Three ext3 lessons (the fundamentals (ext), a journaled write,
+  crash and recover) pin their numbers by test, and the lesson picker groups
+  lessons by filesystem. FAT16 is unchanged and still the default volume.
 
 ## Core API additions
 
@@ -141,18 +147,20 @@ Known and accepted; not bugs.
 
 **`crates/ext`**: `s_wtime` (every operation) and the zeroed tails of the
 pointer blocks a shrinking `write_file` keeps change bytes with no event, so
-slice 4's attribution will see bytes no event explains. `fs_core::run_op` has
+the explorer's attribution sees bytes no event explains. `fs_core::run_op` has
 no caller yet (FAT and ext keep their own `run_op`). A last block group too
 small for its metadata throws `InvalidGeometry` from `formatExt2` and
 `CorruptImage` from `fromImage` (for example 8,194 to 8,260 blocks with 512
-inodes per group); `crates/wasm/README.md` gives the range and the intended
+inodes per group); `crates/wasm/README.md` gives the range and the
 `mke2fs -t ext2 -b 1024 -I 128 -O none,filetype,sparse_super` recipe for a
-loadable image, which has not yet been run against the loader.
+loadable image, checked against the loader and loaded into the explorer by
+the slice-4 browser pass (e2fsprogs 1.47.4, 16,384 blocks).
 
 **`crates/ext`, the journal** (slice 3): `annotate_sector` on a journal
 block classifies the whole journal on every call (about a thousand header
-reads on the default disk), so slice 4's panel should call `journalBlocks()`
-once and reuse it rather than annotating block by block. One mutation is
+reads on the default disk); the Journal panel calls `journalBlocks()` once
+per `volume.epoch` and reuses it, but the Inspector still annotates a journal
+block this way. One mutation is
 one transaction and is never split: a transaction that would tag more than
 `maxlen / 4` blocks (256 on the default 1,024-block journal) throws
 `Unsupported`, so in data mode a single write above roughly 250 KiB fails on
@@ -163,21 +171,24 @@ checksums, 64-bit tags, and writeback mode are `Unsupported`; fast commit,
 async commit, and an external journal device are not implemented either.
 One transaction is in the journal at a time and there is no lazy
 checkpointing: every operation checkpoints immediately and leaves a cleanly
-unmounted volume, the largest departure from a kernel, which a slice-4
-lesson has to explain. Mounts are not modelled (mount count, orphan list,
-`s_state`). Event fields reach JS only inside `text`: the wasm
-`EventRecord` is `{ kind, text, region }`, and the clean `recovery_scanned`
-event has the empty region `{ start: 0, end: 0 }`, so slice 4 either parses
-`text` or adds the event fields to `EventRecord`. `crates/wasm/README.md`
+unmounted volume, the largest departure from a kernel, which the "A
+journaled write" lesson explains at its checkpoint step. Mounts are not
+modelled (mount count, orphan list, `s_state`). Event fields reach JS only
+inside `text`: the wasm `EventRecord` is `{ kind, text, region }`, and the
+clean `recovery_scanned` event has the empty region `{ start: 0, end: 0 }`.
+Slice 4 kept it that way (a spec non-goal): the explorer shows the `text`,
+and the lesson tests parse it to pin journal positions. Adding the event
+fields to `EventRecord` is still open. `crates/wasm/README.md`
 gives the `mke2fs -t ext3 -b 1024 -I 128 -O
 none,has_journal,filetype,sparse_super -J size=1` recipe for a loadable ext3
 image, checked by hand against the loader; the leading `none,` matters,
 since without it mke2fs adds `ext_attr`, `resize_inode`, `dir_index`, and
 `large_file`, which the loader refuses. `JournalState::open` does not yet
 reject a journal that overlaps group metadata or maps a block twice; a
-crafted foreign image would then have transactions write over metadata;
-slice 4, which loads foreign images, should add the check (`CorruptImage`,
-as e2fsck reports). The ignored ext3 mount test in
+crafted foreign image would then have transactions write over metadata.
+The explorer now loads foreign images, and the check (`CorruptImage`, as
+e2fsck reports) is still to add; slice 4 left the loader alone (a spec
+non-goal). The ignored ext3 mount test in
 `crates/ext/tests/mount_linux.rs`
 (`linux_replays_a_crashed_ext3_image_like_recover`) has not been run.
 
@@ -194,24 +205,31 @@ minimum region width, so tiny regions can vanish at narrow widths;
 pre-existing blanket `catch` around `rawDirEntries` on purpose, so a corrupt
 volume falls back to "no range" or "skip this entry" instead of throwing.
 
-**`web/ui`, seams the ext slice inherits**: (a) the free-space model in
+**`web/ui`, after the ext slice** (the slice-4 browser pass): the `.warn`
+text (the Format check line) is hard to read in dark mode in both Format
+forms. By design, clicking one of the journal's pointer blocks (`<journal>`,
+1106 to 1110 on the default disk) on the block-group map only jumps the dump:
+`<journal>` is a pseudo-owner (`isPseudoOwner`), not a tree path, so there is
+nothing to select.
+
+**`web/ui`, seams the ext slice inherited**: (a) the free-space model in
 generic code assumes allocation units exist only in `data` regions and that
-an unowned unit is free, at `web/ui/src/core/attribution.ts` (the
-`region.kind !== "data"` early return and the `free` marking),
-`web/ui/src/components/Inspector.svelte` (the "free" owner fact) and
-`web/ui/src/components/Ribbon.svelte` (`freeLabel` counting unowned units);
-ext blocks span metadata regions and allocated-but-unowned blocks exist (the
-journal, indirect blocks), so the ext slice needs an adapter `isFree(unit)`
-and a `df()`-based free label; (b) the terminal's command help and the
-`mkfs` flags are captured once in `createCommands`, so a family change must
-re-register commands (the spec's non-goal); (c) `entrySlots(path)` returns
-one `Interval`, while an ext path has a directory entry and an inode, so
-slice 4 widens it to `Interval[]`; (d) `addrHelp` and the dump's `g` prompt
-cannot advertise a family's extra address forms such as `i:N`; (e)
-`Ribbon.metaLabel` maps a `directory` region to "root", true only for FAT;
-(f) `hexAddr`/`hex` formatting is duplicated in `fs/fat16/adapter.ts`,
-`shell/commands.ts`, `components/Inspector.svelte`, and `core/lesson.ts` (a
-`core/hex.ts` would serve all four).
+an unowned unit is free. Slice 4 keeps journal blocks out of the units and
+owns the indirect and journal pointer blocks, so the map and the Inspector
+are right, and the ribbon's free label reads the family's `freeUnits()`: FAT
+keeps its count of the clusters no owner row claims, and ext answers `df()`'s
+free count (a fresh ext3 disk reads `14.8 MB free`, 15,205 free blocks, where
+counting unowned blocks would say `16.0 MB free`). (b) Resolved: the terminal re-registers its commands
+when the family changes. (c) `entrySlots(path)`
+stays one `Interval` (on ext the inode's slot); the Inspector's trace names
+the directory entry. (d) Resolved: `extraAddrHelp` feeds `addrHelp` and the
+dump's `g` prompt. (e) `Ribbon.metaLabel` maps a `directory` region to
+"root", true only for FAT; ext has no such region, and its legend shows
+`boot` for the boot block, the superblock, and the backup superblock alike.
+(f) Hex formatting is still repeated: `fs/base.ts`'s `hexAddr` serves the
+adapters and the shell, while `components/StringsPanel.svelte`,
+`components/Inspector.svelte`, and `core/lesson.ts` each keep a `hex` of their
+own.
 
 **`web/ui` terminal** (decided 2026-09-22): the working directory is one
 value per page, not per shell session, because `setPrompt` is engine-wide and
diff --git a/web/ui/README.md b/web/ui/README.md
index 420d7fc..ec358f9 100644
--- a/web/ui/README.md
+++ b/web/ui/README.md
@@ -1,10 +1,12 @@
 # fs explorer UI
 
-A Svelte 5 (runes) app for exploring a filesystem byte by byte, FAT16 today:
-a whole-disk hex dump with ASCII and a strings overlay, a disk ribbon and FAT
-cluster map for seeing where files land, an operation timeline with
-byte-diff replay, nine guided scenarios, and a terminal drawer that mounts
-the volume at `/mnt` and the raw disk at `/dev/hda`. It runs entirely in the
+A Svelte 5 (runes) app for exploring a filesystem byte by byte, FAT16, ext2,
+and ext3: a whole-disk hex dump with ASCII and a strings overlay, a disk
+ribbon and a FAT cluster map or ext block-group map for seeing where files
+land, an ext3 journal panel with crash and recovery controls, an operation
+timeline with byte-diff replay, twelve guided scenarios, and a terminal
+drawer that mounts the volume at `/mnt` and the raw disk at `/dev/hda`. The
+app opens on a FAT16 disk. It runs entirely in the
 browser against the `fs-emulator-wasm` package — nothing is sent over the
 network, and there is no server component. The build from `main` is
 published at https://benjamin-small.github.io/fs-emulator/ by
@@ -34,9 +36,10 @@ pnpm preview   # serve the production build
 
 Layout: the current step runs as a strip under the top bar, next to the
 other controls; a disk ribbon runs full width above a three-column grid —
-files, FAT map, and actions on the left; the hex dump in the center; the
-lesson card, strings, and the byte inspector on the right — with an
-operation timeline along the bottom.
+files, the mounted family's map (the FAT map, or the block-group map and, on
+ext, the Journal panel under it), and actions on the left; the hex dump in
+the center; the lesson card, strings, and the byte inspector on the right —
+with an operation timeline along the bottom.
 
 - **Ribbon** — the entire disk as one strip, one column per pixel of its
   width, colored by owning file or region (or a hairline for free space),
@@ -53,9 +56,29 @@ operation timeline along the bottom.
 - **FAT map** — every cluster as a small cell: free, owned (in its
   file's hue), end-of-chain, or bad. Hover for the cluster number, FAT
   value, and owner; the selected file's chain draws as connected arrows.
+- **Block groups** (ext, in place of the FAT map) — one band per block
+  group headed `group 0 · blocks 1–8192 · 7,082 free`, every block a 4 px
+  cell: metadata in its region's colour, the journal in amber, owned data and
+  directory blocks in their file's hue, free blocks as a hairline, and a dot
+  on an indirect block. The selected file's blocks are outlined, the last
+  step's changes outlined in the diff colour, and the block under the dump's
+  hovered byte dashed. Hover for `block N · region · owner`; click to select
+  the owner and jump to the block.
+- **Journal** (ext, under the block-group map) — on ext3 the journal's mode
+  and header facts (sequence, head, start, size), a ring strip with one cell
+  per journal block coloured by kind (superblock, descriptor, copy, commit,
+  revoke, unused) with checkpointed transactions faded, and the crash
+  controls: pick a phase (before commit, after commit, during checkpoint),
+  **Arm** it so the next change stops there, and **Recover**, enabled while
+  the volume needs recovery, which replays a committed transaction or
+  discards an uncommitted one as a timeline step. While recovery is needed
+  the panel and the status line say so, and changes throw `NeedsRecovery`.
+  On ext2 the panel is one line: "This volume has no journal."
 - **Actions** — add, overwrite, or delete a file; make or remove a
-  folder; format a fresh disk with a chosen size and cluster size; load or
-  export a raw image. Every action records a step in the timeline.
+  folder; format a fresh disk (a Filesystem select picks FAT16, with a size
+  and cluster size, or ext, with ext2 or ext3, a size, inodes per group, a
+  label, and for ext3 the journal's mode and size); load or export a raw
+  image. Every action records a step in the timeline.
 - **Dump** — the virtualized whole-disk hex view: offset, 16 hex bytes,
   and a 16-character ASCII gutter per row. Bytes are tinted by owner, runs
   of zero sectors collapse into a single clickable row, and the header
@@ -63,8 +86,14 @@ operation timeline along the bottom.
 - **Inspector** — facts about the byte under the cursor (offset, sector,
   cluster, owner) plus the sector's decoded annotations — directory
   entries, FAT chain, boot sector fields — with the byte's range
-  highlighted. Offsets and integer fields are in hex to match the dump;
-  hover one for the decimal.
+  highlighted. On ext the address row is `Block`, with the block's own line
+  (`data block 0 of /hello.txt`, `single-indirect block of /bigger.txt`)
+  folded into it, since a block is both the sector and the unit; the
+  annotations decode the superblock, the group descriptors, the bitmaps,
+  inodes, directory entries, pointer blocks, and journal blocks, and the
+  selected file's trace runs from its directory entry through its inode and
+  pointer blocks to its first data block. Offsets and integer fields are in
+  hex to match the dump; hover one for the decimal.
 - **Strings** — printable runs (4+ bytes) in the visible window, or the
   whole disk on request, each with its offset and owner; click one to jump
   to it.
@@ -85,8 +114,17 @@ operation timeline along the bottom.
   disk, add a small file, add a long-named file (LFN entries), overwrite
   with a larger file (chain grows), delete and see what remains, fill the
   disk, make a directory, and work from the shell (the same operations typed
-  as commands, plus a raw-sector read and a raw patch of the volume label).
-  Pick one in the top bar and press **Start**; it formats a fresh disk and a
+  as commands, plus a raw-sector read and a raw patch of the volume label);
+  and, on ext3, the fundamentals (ext) (a tour of the block groups, the
+  superblock, descriptors, bitmaps, and inode table, and how a name reaches
+  its blocks through an inode, including a single-indirect block), a
+  journaled write (one create followed through the needs-recovery flag, the
+  data, the descriptor, the copies, the commit, and the checkpoint), and
+  crash and recover (a crash after the commit that recovery replays and one
+  before it that recovery discards, leaving bytes nobody owns). The picker
+  groups them by filesystem, FAT16 first.
+  Pick one in the top bar and press **Start**; it formats a fresh disk of the
+  lesson's filesystem (the default FAT16 or ext3 disk) and a
   **Lesson** card floats over the page, at the top right to begin with. Drag
   it by its bar to wherever it is out of the way of the bytes it talks about,
   or focus the ⋮⋮ handle and use the arrow keys (Shift for bigger steps); it
@@ -128,6 +166,15 @@ describe every command.
 The prompt shows the working directory: the shell hands it to the terminal on
 startup and after every `cd` or `mkfs`, so it reads `/mnt/DOCS ❯`.
 
+The command set follows the mounted volume: when a format, a load, or a
+lesson changes the family (or swaps ext3 for ext2), the terminal re-registers
+its commands. The address help follows the new family (`s:65 (sector), c:3
+(cluster)` on FAT, `b:65 (block), i:11 (inode)` on ext), so does `df`'s
+summary (cluster or block usage), and `crash` and `recover` exist only while
+the volume has a journal. `mkfs`'s flags do not change: every host lists
+every family's. The new commands share the old ones' working-directory
+state, and the prompt is set again.
+
 | Command | Does |
 |---|---|
 | `ls [path] [-l]` / `dir` | List a directory as a table of name, type, size (`-l` adds the modified time); entries keep their on-disk order. `ls /` shows `dev` and `mnt`; `ls /dev` shows `hda`, `zero`, `null` |
@@ -137,11 +184,13 @@ startup and after every `cd` or `mkfs`, so it reads `/mnt/DOCS ❯`.
 | `dd if=<src> of=<dst> bs=N count=N skip=N seek=N` | Copy bytes between files and the raw disk, in the classic operand form; `--if=<src>` and the rest work as flags too. At most 1 MiB per invocation; `/dev/zero` needs `count=` |
 | `xxd [path] [--offset --len --cols]` / `hexdump` | Hex dump of a path, piped bytes, or piped text; on `/dev/hda` one sector at absolute addresses that match the dump |
 | `mkdir`, `rmdir`, `rm`, `touch`, `cp` | The usual; `cp` into an existing directory keeps the source name |
-| `stat <path>` | Name, type, size, timestamps, first cluster, chain, entry offset, FAT entry offset, data offset; `stat /dev/hda` reports the sector size and count |
-| `df`, `mount` | Cluster usage; device, mount point, type, and `ok` or `corrupt` |
-| `seek <addr>` | Move the hex dump (`0x200`, `512`, `s:1`, `c:2`) |
+| `stat <path>` | Name, type, size, timestamps, then the family's facts: on FAT first cluster, chain, entry offset, FAT entry offset, data offset; on ext inode, inode offset, mode, links, 512-byte blocks, data blocks, indirect blocks, directory-entry offset. `stat /dev/hda` reports the sector size and count |
+| `df`, `mount` | Cluster usage on FAT, block usage (`blockSize`, `blocks`) on ext; device, mount point, type, and `ok` or `corrupt` |
+| `seek <addr>` | Move the hex dump (`0x200`, `512`, `s:1`, `c:2` on FAT; `b:69` for a block and `i:12` for an inode's slot on ext) |
 | `select [path]` | Select a file in every pane, or clear the selection |
-| `mkfs [--sectors --spc --label --root-entries --fats --reserved]` | Format a fresh disk; the timeline is cleared |
+| `mkfs [--type fat16\|ext2\|ext3] [flags]` | Format a fresh disk; the timeline is cleared. `--type` defaults to the mounted volume's type; FAT16 takes `--sectors --spc --label --root-entries --fats --reserved`, ext `--blocks --inodes-per-group --label --uuid --journal-blocks --journal-mode` (the last two ext3 only); a flag the type does not take is an error that names the type |
+| `crash [--at before-commit\|after-commit\|during-checkpoint] [--off]` | ext3 only: arm a crash so the next change to `/dev/hda` stops at that phase (default `after-commit`), or disarm it |
+| `recover` | ext3 only: replay the journal's committed transaction or discard an uncommitted one, as one timeline step, printing its events |
 | `exit` | Close the drawer |
 
 `>`, `>>`, and `<` resolve through the same tree:
@@ -225,7 +274,7 @@ Things to know:
 ## What is filesystem-specific
 
 The app is the fs explorer: one explorer for every filesystem family the wasm
-package can hold, FAT16 today. The hex dump, ribbon, byte attribution,
+package can hold, FAT16 and ext (ext2 and ext3) today. The hex dump, ribbon, byte attribution,
 zero-run collapsing, strings overlay, timeline, diff replay, and terminal read
 `layout()` regions and the operation journal from `fs-emulator-wasm`, so a
 second family gets all of them unchanged. Everything the UI knows about one
@@ -236,8 +285,8 @@ family lives behind the adapter in `src/fs/`:
   (`cluster`, `clusters`, `c` for FAT), the unit count and size, sector to
   unit and unit to byte range, the dump's row-label rule, and the region
   colours. `FsAdapter` is bound to one `Volume` and adds what needs the disk:
-  owners, chains, entry slots, remnants, `stat` and `df` facts, the
-  Inspector's trace, sector annotations, extra address forms, name matching,
+  owners, chains, entry slots, remnants, `stat` and `df` facts, the ribbon's
+  free count (`freeUnits`), the Inspector's trace, sector annotations, extra address forms, name matching,
   the "this write may have moved regions" trigger, and the notes the tree and
   the shell print. Its caches are plain fields that the store refreshes after
   every operation, so a `$derived` that calls an adapter method reads
@@ -250,26 +299,59 @@ family lives behind the adapter in `src/fs/`:
   wasm methods (`geometry`, `fatEntries`, `clusterOwners`,
   `annotateSectorWith`, `rawDirEntries`, `bootSector`, `clusterChain`,
   `formatFat16`) or names their types.
-- `src/fs/index.ts` is the registry: `FAMILIES`, `DEFAULT_FAMILY`,
-  `familyIdOf(fsType)`, and `adapterFor(vol)`, which picks the adapter from
-  `Volume.fsType()`. `src/fs/panels.ts` maps a family id to its map and
-  Format panels, and `App.svelte` and `ActionsPanel.svelte` render whichever
-  the mounted family names. The panels sit in that table rather than on the
-  adapter because the node tests have no Svelte plugin and the adapter must
-  stay importable from them.
-- The scenarios keep their FAT16 copy (they teach FAT16) and declare
-  `family: "fat16"`; a step reaches generic facts through the adapter it is
-  handed and FAT-only ones through `asFat16(fs)`.
+- `src/fs/ext/` is the ext implementation, one adapter for ext2 and ext3:
+  `ExtAdapter` over the ext-only DTOs (`extGeometry`, `extSuperblock`,
+  `blockOwners`, `inodeNumber`, `extInode`, `dirEntries`, `fileBlocks`), the
+  block geometry (`extSpace`), the Format model (`checkFormat`, the size and
+  journal defaults, the `mkfs` flags), `ExtJournal`, the metadata trigger,
+  `BlockGroupMap.svelte`, `JournalPanel.svelte`, and `ExtFormatForm.svelte`.
+  Journal blocks are never allocation units (the journal has its own region
+  kind and amber colour; its pointer blocks show as owned by `<journal>`),
+  and a file's indirect blocks are owner rows with `role: "indirect"`, so the
+  map, the dump, and the Inspector name them without a second table. It is
+  the only place, besides `src/lib/wasm.ts`, that calls the ext-only wasm
+  methods (those seven, the journal's `journalInfo`, `journalBlocks`,
+  `armCrash`, `disarmCrash`, `crashPhase`, `needsRecovery`, and `recover`,
+  and `formatExt2`/`formatExt3`) or names their types.
+- The **journal capability** is the one optional part of the seam:
+  `FsAdapter.journal?: JournalCapability` (`state`, `blocks`, `arm`,
+  `disarm`, `phase`, `recover`), present on ext3 and absent on FAT16 and
+  ext2, with `FsAdapter.needsRecovery` beside it. Its types live in
+  `fs/adapter.ts`, so the Journal panel, the status line, and the `crash` and
+  `recover` commands depend on the capability, never on the ext family, and
+  appear whenever the mounted adapter has one. `recover()` returns the
+  volume's op, which the caller runs through the timeline like any change.
+- **Vocabulary:** every noun the chrome prints comes from the adapter. The
+  sector noun (`UnitSpace.sector`) names one disk sector and the unit noun
+  (`UnitSpace.unit`) the allocation unit: FAT says sector and cluster
+  (`s:65`, `c:2`), ext says block for both, because its disk sector is the
+  block (`b:69`, plus `i:12` for an inode), and where the two nouns coincide
+  the chrome shows one name (the Inspector folds the unit row into the
+  address row, and the dump's `g` prompt names one noun). The Lesson card's
+  file clause is `unit.fileParts` ("its entry, chain, and clusters" on FAT,
+  "its inode, block map, and blocks" on ext).
+- `src/fs/index.ts` is the registry: `FAMILIES` (`fat16`, then `ext`),
+  `DEFAULT_FAMILY` (`fat16`), `familyIdOf(fsType)`, which returns the family
+  whose `fsTypes` lists the string (`FAT16`; `ext2` and `ext3`), and
+  `adapterFor(vol)`. `src/fs/panels.ts` maps a family id to its map and
+  Format panels and its `extras` (the Journal panel for ext), and `App.svelte`
+  and `ActionsPanel.svelte` render whichever the mounted family names. The
+  panels sit in that table rather than on the adapter because the node tests
+  have no Svelte plugin and the adapter must stay importable from them.
+- Each scenario declares its `family`; `start()` formats that family's
+  default disk. A step reaches generic facts through the adapter it is
+  handed and family-only ones through `asFat16(fs)` or `asExt(fs)`.
 
 `tests/adapterBoundary.test.ts` keeps it that way: it scans
 `src/**/*.{ts,svelte}` and fails on a FAT-only wasm call or type outside
-`src/fs/fat16/` and `src/lib/wasm.ts`, and on an import from `fs/fat16`
-anywhere but `src/fs/index.ts`, `src/fs/panels.ts`, and `src/scenarios/`.
-Adding a family means extending `FsFamilyId` in `fs/adapter.ts`, an adapter
-under `src/fs/<family>/`, an entry in `fs/index.ts` and `fs/panels.ts`, a map
-panel in its own words ("FAT map" stays; ext will get "Block groups"), and
-scenarios that teach what is different about it. `docs/ROADMAP.md` has the
-checklist.
+`src/fs/fat16/` and `src/lib/wasm.ts`, on an ext-only one outside
+`src/fs/ext/` and `src/lib/wasm.ts`, and on an import from `fs/fat16` or
+`fs/ext` anywhere but `src/fs/index.ts`, `src/fs/panels.ts`, and
+`src/scenarios/`. Adding a family means extending `FsFamilyId` in
+`fs/adapter.ts`, an adapter under `src/fs/<family>/`, an entry in
+`fs/index.ts` and `fs/panels.ts`, a map panel in its own words ("FAT map",
+"Block groups"), and scenarios that teach what is different about it.
+`docs/ROADMAP.md` has the checklist.
 
 ## Keyboard shortcuts
 
@@ -282,7 +364,7 @@ checklist.
 | Arrow keys | Move the dump's byte cursor, or seek one ribbon column (ribbon focused) |
 | `Page Up` / `Page Down` | Scroll the dump by a page |
 | `Home` / `End` | Jump to the start / end of the disk |
-| `g` | Jump to an offset, sector, or cluster (dump focused) |
+| `g` | Jump to an offset, sector, or cluster, or on ext an offset or block (dump focused) |
 | `s` | Toggle string highlighting (dump focused) |
 | `` ` `` | Toggle the terminal |
 
````

Expected: `git apply` prints nothing, and `git diff --stat` shows `docs/ROADMAP.md` and `web/ui/README.md` changed. What the patch says, so a reviewer can check it without the diff:

- README: the intro names FAT16, ext2, and ext3, the block-group map, the journal panel, and twelve scenarios; Panes gains **Block groups** and **Journal** bullets, the Format form's Filesystem select, and the three ext lessons with the grouped picker; Terminal gains the re-registration paragraph, ext `stat`/`df`/`seek` (`b:69`, `i:12`), `mkfs --type` with both families' flags, and `crash`/`recover` rows; "What is filesystem-specific" describes `src/fs/ext/`, the journal capability, and the vocabulary rule (sector noun versus unit noun, one name on ext, `fileParts`), and the boundary test's ext half.
- ROADMAP: the ext section is "Landed" with a slice-4 bullet listing its surface; the deferred items keep the loader overlap check and the event-field note (both reworded as still open, spec non-goals), record that the mke2fs ext2 recipe loaded in the browser pass, and say the journaled-write lesson explains immediate checkpointing; a new "`web/ui`, after the ext slice" paragraph records what the browser pass found: the `.warn` text is hard to read in dark mode in both Format forms, and clicking a `<journal>` pointer block on the map selects nothing; the inherited-seams paragraph marks (b) and (d) resolved, notes under (a) that the ribbon's free label reads `df()` (`14.8 MB free` on a fresh ext3 disk), and keeps (e), the legend's `boot` label for the boot block, the superblock, and the backup superblock alike.

#### Round 6: the browser pass (spec section 8)

- [ ] **Step 20: Drive the app.** From `web/ui`, run `pnpm exec vite --port <a free port> --strictPort` and open it in a browser at 1440 × 900 or larger. Check each item; the expected text is what this task's pass observed.

1. **The picker.** The Learning scenarios select shows two groups, `FAT16` (the nine FAT lessons in order) and `ext` (`The fundamentals (ext)`, `A journaled write`, `Crash and recover`); it still opens on `The fundamentals`.
2. **Format ext3 from the form.** Format details → Filesystem `ext`, Variant `ext3`, Format disk. The left column reads Files, `Block groups · 2 groups · 16,384 blocks`, `Journal · ordered mode`, Actions; the tree's root row reads `0 B · first block 69`, and the ribbon's legend ends `14.8 MB free`.
3. **Add, overwrite, delete a file and a folder.** Add `/hello.txt` with `Hello, ext3!`: the tree shows `hello.txt 12 B · first block 1111`; the Inspector's Selected file rows read `directory entry in / (block 69 + 0x2c)`, `inode 12 (block 6 + 0x180)`, `data block 1111 (first of 1)`; the Step strip shows `create_file /hello.txt`, Block buttons 1–6, 69, 82–91, 1111, and the journal events, stopping at 210 px with a scrollbar; the Journal facts become `sequence 2`, `head 10`, `start 0`. Overwrite it with 14,000 bytes: the trace gains `single-indirect block 1125` and `data block 1111 (first of 14)`. New folder `/docs` (`first block 1126`), `/docs/n.txt` inside it (`directory entry in /docs (block 1126 + 0x18)`, `inode 14 (block 6 + 0x280)`), then Delete it, Remove folder `/docs`, Delete `/hello.txt`: the tree is back to `/` and `lost+found`, and the timeline reads `7 of 7`. Watch the rest of the chrome at each of these steps too: after the Add the ribbon's legend gains a `hello.txt` swatch and its label stays `14.8 MB free` (15,204 free blocks; no step here moves it by 0.1 MB), the block-group map fills block 1111 in hello.txt's hue with the selection's outline and draws the diff outline around the step's other changed blocks (1–6, 69, 82–91), and the dump (`seek b:1111` in the terminal, or the Step strip's `Block 1111`) shows row `00115c00` tinted in the same hue, labelled `block 1111 · /hello.txt`, with the new bytes highlighted; after the Overwrite the map outlines the chain 1111–1124 joined in order and dots the indirect block 1125; after each Delete and Remove folder the file's cells go back to the hairline and its swatch leaves the legend.
4. **`mkfs --type ext3` and the read commands.** Open the terminal: `mkfs --type fat16` prints `formatted /dev/hda as FAT16; the timeline was cleared`, the map is the FAT map, and the Actions panel's Filesystem select (Format details) re-syncs to `FAT16`; `mkfs --type ext3` prints `formatted /dev/hda as ext3; the timeline was cleared`, the block-group map and the Journal panel return, and the select re-syncs to `ext` with the ext form (Variant `ext3`) under it; `echo 'Hello, ext3!' > /mnt/hello.txt`; `stat /mnt/hello.txt` shows `inode 12`, `inodeOffset 0x1980`, `mode 0100644`, `links 1`, `blocks512 2`, `dirEntryOffset 0x1142c`; `df` shows `type ext3`, `blockSize 1024`, `blocks 16384`, `used 1180`, `free 15204`; `seek i:12` puts the Inspector on `Offset 0x1980` in `Block 6 · inode table (group 0)` with the `inode 12: mode 100644 (file), size 12 ...` annotation; `xxd /dev/hda --offset b:69 --len 64` prints four rows from `00011400` ending in `hello.txt`.
5. **Crash and recover from the terminal, both phases.** `crash --at after-commit` prints `armed: the next change to /dev/hda stops after commit`; `echo two > /mnt/two.txt`; `ls /mnt` does not list it; the status line shows `Volume needs recovery` and the Journal panel `Needs recovery: the journal holds an unfinished transaction.` with `start 10`. `recover` prints the recovery events; `two.txt` appears. `crash --at before-commit`, `echo three > /mnt/three.txt`, `recover`: prints `discarded uncommitted transaction ...`, and `three.txt` never appears.
6. **Crash and recover from the panel, both phases.** The phase select defaults to `after commit`; Arm shows `Armed: the next change stops after commit`; Add file crashes it (flag line shown, `Recover` enabled, armed line gone); Recover adds the file and disables `Recover`. Pick `before commit`, Arm, Add file, Recover: the Step strip shows `transaction_discarded discarded uncommitted transaction ...` and the file is absent.
7. **The three lessons, Next to the end and Prev back.** For each step, compare the card, the Inspector, and the dump row at the cursor:
   - The fundamentals (ext): step 1 `Look at: Block 0, boot block`; step 2 row `00000430` holds `53ef` at 0x438; step 3 row `00000800` starts `03000000 04000000 05000000`; step 4 the cursor byte at 0xc8c is `1f` after twelve `ff`s (Inspector `blocks 1..1125: used`); step 5 row `00001000` starts `ff1f`; step 6 cursor 0x1480 (`ed41...`, inode 2); step 7 rows from `00011400` show `.`, `..`, `lost+found`, `hello.txt`, `bigger.txt` with rec_lens `0c`, `0c`, `14`, `14`, `3c0`; step 8 cursor 0x1980 (`a481 0000 0c00...`); step 9 row `00115c00` reads `Hello, ext3!`; step 10 row `00119400` starts `64040000` (1124) with the Inspector `single-indirect block of /bigger.txt`; step 11 row `00014800` starts `c03b3998`; step 12 row `00800400` is the backup superblock; step 13 row `00000400` holds free counts `563b0000` (15,190) and `f3030000` (1,011). Prev back to step 1 shows block 0 again.
   - A journaled write: step 1 row `00000460` starts `02000000`; step 2 row `00115c00` reads `line 0001: the q`; step 3 cursor 0x1481c in row `00014810` = `00000400 00000001 00000002 00000000` (maxlen, first, sequence 2, start 0); step 4 row `00014c00` = `c03b3998 00000001 00000001 00000001`; step 5 row `00015000` is the superblock image (Inspector `copy of block 1 for transaction 1 (stale)`); step 6 row `00016c00` = `c03b3998 00000002 00000001`; step 7 cursor 0x1142c in the root directory, `0c000000 d403 09 01 notes.txt`; step 8 cursor 0x14818; step 9 back on 0x460. The Journal panel ends at `sequence 2 head 10 start 0`.
   - Crash and recover: step 1 row `00000460` starts `06000000`, `Volume needs recovery` shows, the tree has only `lost+found`, Journal `sequence 1 head 1 start 1`; step 2 row `00014c00` is the descriptor; step 3 rows from `00011400` end with `lost+found` at rec_len `e803` and zeros after; step 4 cursor 0x1142c reads `crash.txt`, the flag lines are gone, the tree lists `crash.txt 2,048 B · first block 1111`; step 5 row `00116400` reads `Written before t` with the Inspector `data (group 0) · free`, needs recovery again; step 6 cursor 0xc8b, row `00000c80` = eleven `ff`s then `00`; step 7 the same `Written before t` row. Prev back through step 1 shows each rewound state (step 3 without the entry, step 4 with it), with the Journal panel's `Shows the latest state, not the step you are viewing.` note.
   - Restart: start A journaled write, scroll the dump far down, Close it at step 1, and start Crash and recover: the dump scrolls back to row `00000460`.
8. **An mke2fs image.** With Homebrew's `/opt/homebrew/opt/e2fsprogs/sbin/mke2fs` (e2fsprogs 1.47.4, which Task 1 Step 26 already requires), make a 16 MiB zeroed image and run the wasm README's `mke2fs -t ext3 -b 1024 -I 128 -O none,has_journal,filetype,sparse_super -J size=1 <image>`; load it with Load image: `Block groups · 2 groups · 16,384 blocks`, `Journal · ordered mode` (`sequence 1 head 1 start 0 blocks 1,024`), `lost+found` at `first block 262`; add a file (`directory entry in / (block 261 + 0x2c)`), crash and recover it. The ext2 recipe (`mke2fs -t ext2 -b 1024 -I 128 -O none,filetype,sparse_super`) loads too, and its Journal panel reads `This volume has no journal.`
9. **Back to FAT16.** Filesystem `FAT16`, Format disk: `FAT map · 8,167 clusters`, no journal panel, and the ribbon's `16.0 MB free`. Run every FAT lesson to the end (The fundamentals, Format an empty disk, Add a small file, A long file name, Overwrite with a bigger file, Delete and see what remains, Fill the disk, Directories are files too, Work from the shell): each formats FAT16, and its cards, `Look at:` lines, Inspector rows, FAT map, ribbon, and dump read as they did before this plan, in `Sector` and `cluster`. For The fundamentals: `Sector 0, reserved (boot sector)`, `Offset 0xb in reserved (boot sector)`, `Sector 65, root directory`, `Offset 0x204 in FAT 0`, `Offset 0x4204 in FAT 1`, `Files: /HELLO.TXT, its entry, chain, and clusters; ...`, `cluster 2 in the data region (offset 0xc200)`, `cluster 5 in the data region (offset 0xda00)`, `Offset 0x20c in FAT 0`. The console has no errors.

What this task's pass found wrong, all outside this task's files and recorded in the ROADMAP (step 19): the `.warn` text is hard to read in dark mode in both Format forms; the legend labels the boot block, superblock, and backup superblock all `boot`. Clicking a `<journal>` pointer block on the map only jumps the dump and selects nothing, which is Task 4's design (`<journal>` is a pseudo-owner, not a tree path), and the ROADMAP records it as such. The pass also found four faults that Tasks 2 and 3 now fix, so a pass on the finished plan sees them right (items 2, 3, and 7 check them): the ribbon's free label (`14.8 MB free` from the family's `freeUnits()`, ext's `df().free`, where counting unowned blocks said `16.0 MB free`; FAT keeps that count), the Step strip's height (capped at 210 px; an ext3 create lists 18 block buttons and 30 events, the ext tour's merged first step twice that), the dump's scroll nonce across `selection.reset()` (it no longer restarts at 1, which HexView's `lastNonce` swallowed), and the tree's root row (`0 B · first block 69` on ext, not `0 B · no data`).

#### Gates and commit

- [ ] **Step 21: Run the gates.** From `web/ui`:

```sh
pnpm test
pnpm build
```

Expected: `Test Files  49 passed (49)`, `Tests  485 passed (485)`; svelte-check `0 ERRORS 0 WARNINGS`, then `✓ built in ...`. Then the second consumer, from the repo root:

```sh
(cd web/demo && CI=true pnpm install --frozen-lockfile && pnpm build)
```

Expected: `✓ built in ...`. Finally `git status --short` lists only the files in **Files**: no `pnpm-lock.yaml`, no `pnpm-workspace.yaml`, and no image or `public/` directory left from the browser pass.

- [ ] **Step 22: Commit.** From the repo root:

```sh
git add docs/ROADMAP.md web/ui/README.md web/ui/src/components/ScenarioPanel.svelte web/ui/src/scenarios/index.ts web/ui/src/scenarios/extFundamentals.ts web/ui/src/scenarios/journaledWrite.ts web/ui/src/scenarios/crashRecover.ts web/ui/tests/scenarios.test.ts web/ui/tests/fixtures/lesson.ts web/ui/tests/ext-fundamentals.test.ts web/ui/tests/journaled-write.test.ts web/ui/tests/crash-recover.test.ts
git commit -m "feat(ui): three ext3 lessons, a grouped scenario picker, and the ext docs"
```

Facts pinned by this task's tests, for anyone who changes the default ext3 format:

- After the ext tour's first step: free counts 15,190 blocks and 1,011 inodes; group 0 7,067 free blocks and 499 free inodes; files in blocks 1111 (hello.txt) and 1112–1124 plus indirect block 1125 (bigger.txt); inodes 12 and 13; root rec_lens 12, 12, 20, 20, 960; block bitmap 140 bytes of FF then 1F at +0x8C (1,125 bits); inode bitmap FF 1F with ones from +0x40.
- One create on a fresh disk: the flag word goes 0x02 → 0x06 first and back last; data before the first journal write; `s_start` 0 → 1 → 0 and `s_sequence` 1 → 2 in two 8-byte writes at block 82 + 0x18; descriptor at block 83 (journal block 1) tagging blocks 1, 2, 3, 4, 5, 6, 69; copies at 84–90; commit at 91 (journal block 9); seven checkpoints after the commit; the new entry at block 69 + 0x2C.
- Crash lesson: after the after-commit crash, transaction 1 is live at journal blocks 1–9 and lost+found's rec_len is 1000; recovery replays 7 blocks and puts `crash.txt` at 69 + 0x2C (inode 12, blocks 1111 and 1112); the before-commit crash is transaction 3, allocating block 1113; its recovery discards it and bitmap byte 3·1024 + 0x8B stays 00.
