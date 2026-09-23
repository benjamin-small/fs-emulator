# Terminal Access to the Emulated Filesystem — Design

Date: 2026-09-22. Status: approved (brainstormed in plan mode; design panel and
critique applied). Implements the raw-disk half in Rust and the shell half in
`web/ui`.

## Purpose

Let a learner drive the emulator from a command line inside the explorer: the
volume mounted at `/mnt`, the raw disk at `/dev/hda`, and the usual tools
(`ls`, `dir`, `cd`, `pwd`, `cat`, `echo`, `dd`, `xxd`, `mkdir`, `rm`, ...)
so that every command shows up in the hex dump, ribbon, timeline, and tree
exactly like a form action does. The terminal is the user's own
`@benjamin-small/browser-terminal` (Rust core compiled to wasm, xterm.js
panes, structured-value pipes, commands registered from TypeScript).

Why it matters: `dd --if=/dev/hda --bs=512 --skip=1 --count=1 | xxd` reads
the FAT the way a real tool would, and `dd --if=/dev/zero --of=/dev/hda
--count=1` lets a learner wipe the boot sector on purpose, watch path
operations fail with `CorruptImage`, then rewind on the timeline or write it
back.

## Decisions made with the user

1. **Redirection is staged.** This work ships pipe-based commands
   (`echo hi | write /mnt/a.txt`, `cat --bytes /mnt/a | write /mnt/b`,
   `dd --if= --of=`). browser-terminal's parser rejects `>`, `>>`, `<` as
   reserved syntax; adding them with a host-pluggable file hook is a later
   change in that repository. When it ships, the explorer registers its VFS
   as the hook and the same commands gain real redirection.

   *2026-09-22, later the same day:* browser-terminal 0.3.0 shipped it, as
   `bt.setRedirectHandler({ read, write })`. The explorer installs one
   (`src/shell/redirect.ts`), so `>` and `>>` are the same journaled write
   `write` performs and `<` reads a file as text under `cat`'s 1 MiB cap.
   Neither hook receives a `CommandCtx`, so they cannot log byte counts or
   warn about a rewound timeline; everything else is the commands' behaviour,
   because both hooks call the helpers the commands were refactored onto.
2. **Dependency is the published npm package**, pinned exactly to
   `@benjamin-small/browser-terminal@0.2.0`. CI and GitHub Pages keep working
   unchanged. *(2026-09-22: the pin moved to 0.3.0, still exact.)*
3. **Placement is a bottom drawer** owned by the explorer via
   `BrowserTerminal.create({ mount })`, toggled by a key and a button. No
   tmux chrome. *(2026-09-23: on viewports 1600px and wider the same
   panel is a column down the right side spanning every row; the drawer stays
   for narrower ones. `placementFor` in `src/core/terminalPlacement.ts`
   decides from `window.innerWidth`.)*

## Non-goals

- `>`/`>>`/`<` redirection, `&` jobs, and `key=value` barewords (all need
  browser-terminal changes; listed as follow-ups in `docs/ROADMAP.md`).
  *(2026-09-22: 0.3.0 delivered redirection and barewords; `&` jobs remain a
  non-goal.)*
- Per-session working directories (commands cannot learn their session id
  from `ctx`; one instance per page anyway). *(2026-09-22: `ctx.session` and
  `ctx.pane` exist in 0.3.0, but `setPrompt` is engine-wide, so a per-session
  directory could not be shown in the prompt; still one per page.)*
- `mv`/rename, true append, truncate (no core operation; `write --append`
  rewrites and says so).
- Reading the disk as it was at an earlier timeline step from the shell
  (reads show the latest state and warn; a `--at-step` flag is a follow-up).
- Rust-side capping of raw write size (the journal invariant is "every byte
  journaled"); the shell caps `dd` at 1 MiB per invocation instead.

## Constraints from browser-terminal 0.3.0 (verified)

The design was written against 0.2.0; the pin moved to 0.3.0 on 2026-09-22
and these are the facts as of that release, with the 0.2.0 constraint noted
where it changed.

- `Value` is `null | bool | int | float | str | bytes | list | record`. A
  `Uint8Array` survives pipes, variables and `run().value` unchanged, renders
  as `<N bytes>` (including in table cells), and `length` counts its bytes.
  *(0.2.0 had no bytes type; the shell carried a `{ bytes: "<hex>", length }`
  record instead.)* Rendered strings pass through an escape stripper (control
  bytes dropped). A list of records renders as a table, a record as a
  key/value block, a scalar as one line with newlines preserved.
- `bt.setRedirectHandler({ read(target, ctx), write(target, value, ctx) })`
  makes `<`, `>` and `>>` the host's to define; there is no built-in
  filesystem and no automatic conversion. A write receives the pipeline's
  whole collected value (an empty stream is `[]`) and consumes it, so the run
  returns `null` and the terminal prints nothing; a failed pipeline never
  calls the writer. A read's value feeds the first command. Both contexts
  carry `signal`, `session` and `pane`, the writer's also `append`. A thrown
  `{ message, help }` keeps its help. *(0.2.0 rejected all three as reserved
  syntax.)*
- `echo` is a builtin: one argument returns that value, several return a
  list, none returns empty. Registering a builtin's name errors.
- Unquoted `key=value` barewords are positional strings: `dd if=/dev/hda
  count=1` arrives as `['if=/dev/hda', 'count=1']`. They neither set
  variables nor bind flags; declared flags still take `--name=value`. Quote
  the whole operand for a space or an empty value. *(0.2.0 did not lex `=` in
  a bareword at all.)* `'raw'` strings have no escapes; `"interpolated"`
  strings support `\n \t \" \\ \$` and `$var`.
- Commands get `(args, input, ctx)`; `ctx` has `signal`, `log`, `err`, `emit`
  and the read-only ids `session` and `pane`. A thrown `{ message, help? }`
  renders a rich error and the engine prefixes the command name itself.
- `bt.setPrompt(prefix)` sets the text before the `❯` for every pane, so it
  is engine-wide rather than per session. `bt.focus()` / `bt.blur()` move
  keyboard focus to and from the active pane's input.
  `CreateOptions.terminal` takes `{ theme, fontFamily, fontSize }` and
  `bt.setTheme(theme)` replaces the theme for every pane; `ITheme` and
  `TerminalOptions` are exported types. *(0.2.0 had none of these: the prompt
  was a bare `❯`, focus meant querying `.xterm-helper-textarea`, and the
  theme was `!important` CSS over xterm's DOM.)*
- `create({ mount })` renders into a host element with no shadow DOM and no
  xterm stylesheet; `show`/`hide`/`setPanelMode` are no-ops. One instance
  per page; `dispose()` before re-creating (HMR). The `.wasm` loads via
  `new URL(..., import.meta.url)`; the package must be excluded from Vite's
  dependency pre-bundling.
- xterm cancels ordinary keydowns, so typing in the terminal never reaches
  `window` listeners; only the Ctrl-B chord and the key after it escape.

## Architecture

```
TerminalPanel.svelte  (drawer; create({mount}); registers commands)
        │  createStoreHost()  → ShellHost (runes; the only shell file touching stores)
web/ui/src/shell/     (pure TS: vfs, bytes, addr, dd, xxd, errors, commands; node-tested)
        │  host.run(v => v.writeRaw(...)) / v.createFile(...) ...
crates/wasm  Volume.writeRaw / readRaw / corruption()
        │
crates/fat   FatFs::write_raw (run_op; boot-sector re-parse; CorruptImage gate)
crates/fs-core  raw_write helper, RawWrite event, Error::OutOfBounds, FileSystem::write_raw
```

### 1. Rust core: journaled raw writes

`crates/fs-core`

- `Error::OutOfBounds { offset: u64, len: u64, disk_len: u64 }`, appended
  last. Display:
  `out of bounds: {len} bytes at offset {offset} run past the end of the {disk_len}-byte disk`.
- `trace::RawWrite { offset: usize, len: usize }`, the first concrete
  `Event` in fs-core: kind `raw_write`, region `offset..offset+len`,
  Display `wrote {len} raw bytes at 0x{offset:x}`.
- `disk::raw_write_op(offset: u64, len: usize) -> String` returns
  `write_raw 0x{offset:x} +{len}`.
- `disk::raw_write(disk: &mut Disk, offset: u64, bytes: &[u8]) -> Result<Range<usize>>`:
  `checked_add` bounds check first (nothing is written on failure, so a
  caller's rollback has nothing to undo), then `disk.write` and the event.
  Empty `bytes` returns `Ok(offset..offset)` without a `ByteChange` or an
  event. House style of `table::allocate_chain(disk: &mut Disk, ..)`.
- `FileSystem::write_raw(&mut self, offset: u64, bytes: &[u8]) -> Result<OpRecord>`
  is a required trait method (default methods cannot journal: `disk()` and
  `history()` are immutable). The trait test's `NullFs` implements it.
- Re-exports: `raw_write`, `raw_write_op`, `RawWrite`.

`crates/fat`

- `BootSector::decode(bytes: &[u8]) -> BootSector` reads fields without
  validating; `parse` calls it then validates. `pub const BOOT_SECTOR_LEN: usize = 512`.
- `FatFs` gains `corrupt: Option<Error>` and
  `pub fn corruption(&self) -> Option<&Error>`.
- `FatFs::write_raw` runs through `run_op` with the op string above. When
  the written range starts below 512 it calls `reparse_boot_sector`: parse
  plus geometry; adopt if the geometry fits the physical disk (same bytes
  per sector, `total_sectors <= sector_count`), otherwise keep the last
  good `boot`/`geo` and set `corrupt`. Disk is truth: a valid boot sector
  with a different root-entry count or cluster size is adopted and
  `layout()` follows.
- `ensure_mounted()` is the first line of every path-based method:
  `list_dir`, `stat`, `read_file`, `create_file`, `create_dir`,
  `delete_file`, `remove_dir`, `write_file`, `raw_dir_entries`. It returns
  `CorruptImage("boot sector no longer parses after a raw write: {cause}")`.
  Not gated, by design: `layout`, `annotate_sector*`, `cluster_owners`,
  `fat_entries`, `cluster_chain`, `boot_sector`, `geometry`, `disk`,
  `history`, `write_raw`.
- `annotate_boot_sector` decodes the live bytes and reads the real
  signature bytes, so sector-0 annotations always describe what is on
  screen, mounted or not.

### 2. wasm

- `code_of`: `OutOfBounds => "OutOfBounds"`.
- Generic `impl Volume`, after `sector`:
  `writeRaw(offset: u32, bytes: Uint8Array): OpRecord` (template
  `writeFile`) and `readRaw(offset: u32, len: u32): Uint8Array` (template
  `sector`; `BadArgument` past the end; `checked_add`).
- FAT block, next to `bootSector`: `corruption(): string | null`.
- `u32` suffices under the 256 MiB volume cap. `types.rs` is unchanged.
  wasm-bindgen does not validate `u32`, so callers pass non-negative
  integers only.

### 3. UI store

- `VolumeStore.run`: after a successful op, if any change's offset is below
  512, re-read `geometry` and `layout` from the volume. Bytes per sector
  cannot change (rejected in Rust), so `sectorSize` and `zeros` stay valid.
  `seek()` keeps the latest layout while showing older bytes, the caveat
  the tree and layers already carry.
- `changedSectors` skips changes whose `after` is empty.
- `StatusLine`'s friendly map gains `OutOfBounds`.

### 4. Shell layer (`web/ui/src/shell/`)

Pure TypeScript, tested under node through the real wasm package. Nothing
imports browser-terminal at runtime; only
`import type { CommandSpec, CommandArgs, CommandCtx, Value } from "@benjamin-small/browser-terminal"`.

`host.ts` is the seam:

```ts
export interface ShellHost {
  readonly vol: Volume;            // always the latest state
  readonly cursor: number;         // -1 before any op
  readonly historyLength: number;
  run(fn: (v: Volume) => OpRecord): OpRecord;   // throws { message, code? } on failure
  format(options: FormatOptions): void;         // throws { message, code? }
  select(path: string | null): void;            // volume path "/A/B", never "/mnt/A/B"
  jumpTo(offset: number): void;
  closeTerminal(): void;
}
export const atLatest = (h: ShellHost) => h.cursor === h.historyLength - 1;
export function statusToError(s: { text: string; code?: string } | null): Error & { code?: string };
```

`storeHost.svelte.ts` (the only shell file touching runes; not imported by
tests): `run` calls `volume.run` and throws `statusToError(volume.status)`
on `null`; `format` calls `volume.format`, throws if `volume.status` is
set, and only then `selection.select(null)` (which clears status).

`errors.ts`: `ShellError extends Error { help?; code? }`;
`wrapFs(display, e)` yields `${display}: ${phrase}` with a coreutils phrase
per code (`NotFound` → `No such file or directory`, `AlreadyExists` →
`File exists`, `IsADirectory` → `Is a directory`, `NotADirectory` →
`Not a directory`, `DirectoryNotEmpty` → `Directory not empty`, `DiskFull`
→ `No space left on device`, `OutOfBounds` → `Range runs past the end of
the disk`, otherwise the wasm message). Never a command prefix: the engine
adds it.

`bytes.ts`, the pipe convention: strings are UTF-8 text;
`BytesBlob { bytes: <lowercase hex>, length }` carries raw bytes
losslessly. `toBytes(v)`: string → UTF-8; number or boolean → `String(v)`;
list of scalars → items joined by one space (echo semantics); blob →
unhex; `null` → empty; anything else → `ShellError` with help. A blob
returned to the terminal renders as a key/value record; users pipe it into
`xxd` or `write`.

`addr.ts`: `parseAddr(v, geometry)` (`0x..`, decimal, `s:N`, `c:N`),
extracted from `HexView.svelte`'s `jump` (HexView keeps its own bounds
guard), and `parseSize` with `k`/`M` suffixes. Both return non-negative
integers or throw with help.

`vfs.ts`: `Resolved = root | dev | raw | zero | null | { volume, path }`;
`class Vfs { cwd = "/mnt"; normalize; resolve; display; toVirtual }`.
Normalization is client-side because fs-core rejects `.` and `..`: `\` as
`/`, collapse separators, join relative to cwd, drop `.`, pop `..` (root
stays root), strip trailing `/`. `canonicalize(vol, path)` fixes case by
walking `listDir`; `cd` stores the canonical path. Unknown `/dev/x` and
`/foo` throw with help naming `/mnt`, `/dev/hda`, `/dev/zero`, `/dev/null`.
cwd is one value per page.

`xxd.ts`: `formatXxd(bytes, base, cols = 16)` in classic xxd layout
(8-hex-digit address, byte pairs, padded hex area, ASCII with `.` for
non-printables), no trailing newline.

`dd.ts`: `parseDd(flags, operands)` merges `--if/--of/--bs/--count/--skip/--seek`
(the documented form is `--if=/dev/hda`) and quoted `'if=/dev/hda'`
operands; duplicates and unknown operands error with help. `bs` defaults
to 512. `DD_MAX_BYTES = 1 << 20` per invocation, checked on the read window
before any write. `/dev/zero` requires `--count`. Copy plan: source window
`skip*bs` for `count*bs` (or to end); logs `N+P records in`,
`N+P records out`, `B bytes copied`; sink `/dev/hda` →
`host.run(v => v.writeRaw(seek*bs, data))`; volume file → read existing,
overlay at `seek*bs` (zero-padded), rewrite, and log that FAT has no
partial writes; `/dev/null` discards; no `--of` returns a blob.

`commands.ts`: `createCommands(host, vfs = new Vfs()): CommandDef[]`,
`CommandDef { spec, fn }`. Aliases are separate defs sharing a handler.
`echo` is not registered. Every read command emits one warning via
`ctx.err` when `!atLatest(host)`:
`showing the latest state, not step N of M; click "Back to now" or run a write command`.
`ls`, `df`, `mount`, `stat` call `vol.corruption()` once and report it.
`cat` refuses files over `DD_MAX_BYTES` in both forms, text and `--bytes`,
pointing at `dd` for a range of a bigger file, and warns when
more than 10% of bytes are non-text. `ls` keeps on-disk order.

| Command | Signature | Behaviour |
|---|---|---|
| `ls` / `dir` | `[path] -l` | Table of `{ name, type, size }` (+ `modified` with `-l`); `/` lists `dev`, `mnt`; `/dev` lists `hda` (block, disk size), `zero`, `null` |
| `cd`, `pwd` | `[path]` | `cd` with no arg → `/mnt`; target must be root, `/dev`, or a volume directory; stores the canonical path |
| `cat` | `<path> --bytes` | UTF-8 text, or a blob with `--bytes`; devices point at `dd` |
| `write` | `<path> --append --at <addr>` | Piped input via `toBytes`; create or overwrite; `--append` reads, concatenates, rewrites and says so; `/dev/hda` requires `--at`; `/dev/null` discards; selects the file |
| `dd` | flags above, quoted operands | As above |
| `xxd` / `hexdump` | `[path] --offset --len --cols` | Path or piped bytes; `/dev/hda` defaults to one sector at absolute addresses matching the dump |
| `mkdir`, `rmdir`, `rm`, `touch`, `cp` | paths | Through `host.run`; `cp` into an existing directory uses the source basename; `touch` on an existing file logs that there is no timestamp-only update |
| `stat` | `<path>` | Record: name, type, size, timestamps, first cluster, chain, cluster count, entry offset, FAT entry offset, data offset; `/dev/hda` reports sector size, sectors, fs type |
| `df`, `mount` | — | One-row tables: cluster usage; device, mount point, type, state `ok`/`corrupt` |
| `seek` | `<addr>` | `host.jumpTo` within `sectorCount * sectorSize` |
| `select` | `[path]` | `host.select(canonical)` or `null` |
| `mkfs` | `--sectors --spc --label --root-entries --fats --reserved` | `host.format`; resets cwd; logs that the timeline was cleared |
| `exit` | — | `host.closeTerminal()` |

### 5. UI integration

- Dependencies: `@benjamin-small/browser-terminal` at exactly `0.3.0`,
  `@xterm/xterm` `^6.0.0` (for its stylesheet; the range the library pins).
  `vite.config.ts` excludes the package from `optimizeDeps`.
  `vitest.config.ts` is unchanged.
- `src/core/keys.ts`: `inTextEntry(e)` using `e.composedPath()[0]` (input,
  textarea, select, contenteditable, or inside `.terminal-drawer`), used by
  `App.svelte` and `ScenarioPanel.svelte`. Backtick toggles the drawer
  outside text entry.
- `src/state/terminal.svelte.ts`: `open`, `height` (persisted, clamped by a
  pure `clampHeight(px, innerHeight)` to 120px..60%), `focusNonce`, `ready`
  (`idle | loading | ready | error`), `error`; `toggle`, `openAndFocus`,
  `close`.
- `TerminalPanel.svelte`: a `panel` section with a drag bar
  (`Terminal — /mnt is the volume, /dev/hda the raw disk. Type help.` plus
  Close) and the mount div. On first open, after `tick()`, it lazy-imports
  the library, calls `create({ mount, terminal })` with the theme and font
  built from the design tokens, registers
  `createCommands(createStoreHost(close))` and the redirect handler, sets
  `ready`, and calls `bt.focus()`. HMR dispose and unmount call
  `bt.dispose()`.
  Never calls `show`/`hide`. Escape on the bar closes; `exit` closes and
  returns focus to the topbar button.
- Layout: `.app` adds `grid-auto-rows: var(--term-h, 220px)` and
  `App.svelte` sets `--term-h` on `.app` (custom properties inherit
  downward only). A hidden drawer adds no track. Under 760px the height
  caps at 40vh. On viewports 1600px and wider `App.svelte` adds `.term-side`
  and `--term-w` to `.app`: the drawer is placed in an implicit second column
  spanning every row, 320px to half the viewport (default 500px), resized by
  a grip on its left edge and remembered separately from the height. The
  panes' own colours and font come from
  `CreateOptions.terminal` and `bt.setTheme`, built from the tokens by
  `src/core/terminalTheme.ts`: background (`--panel`), text (`--ink`), cursor
  and selection (`--focus`), `--font-mono` at 12px. `app.css` keeps only the
  drawer layout and the focus-ring suppression; dark mode is the default and
  the theme switch in the top bar sets `data-theme` on the root; the panel
  re-pushes the theme whenever the switch flips.
- Topbar: `<button id="terminal-toggle" aria-pressed aria-controls="terminal-drawer">Terminal</button>`
  after the title.

### 6. Scenario and docs

- Eighth scenario "Work from the shell" (`src/scenarios/shell.ts`): step
  text shows the command; `action` performs the equivalent op (no engine
  change). Steps: open the terminal and `ls /mnt`; `echo 'Hello from the
  shell' | write /mnt/HELLO.TXT`; `cat`, `stat`, `seek c:2`; `mkdir
  /mnt/DOCS`, `cd`, `ls`; `cp`; `dd --if=/dev/hda --bs=512 --count=1 |
  xxd`; `echo 'SHELLDISK  ' | dd --of=/dev/hda --bs=1 --seek=43` (action
  `v.writeRaw(43, ...)`); `rm` and show remnants.
- `web/ui/README.md` Terminal section; root README; `crates/wasm/README.md`;
  `docs/ROADMAP.md` (raw writes in the journal rule; deferred items;
  browser-terminal follow-ups: `>`/`>>`/`<` with host hooks, `key=value`
  barewords, a bytes value, session id on `ctx`, terminal theme options, a
  public `focus()`).

## Testing

- fs-core: `OutOfBounds` Display; `RawWrite` kind and region; `raw_write`
  journals before/after, writes nothing on `OutOfBounds`, does not overflow
  at `u64::MAX`, records nothing for an empty write; the trait test.
- fat: raw write appears in history and rolls back byte for byte; wiping
  sector 0 makes `list_dir` fail with the gate message while `write_raw`
  and `layout` still work and restoring the sector clears `corruption()`;
  a boot sector with 32 root entries is adopted and `layout()` grows; a
  boot sector describing more sectors than the disk is rejected as corrupt;
  boot annotations follow the live bytes.
- wasm (`wasm-pack test --node`): `writeRaw` record shape (op string, one
  change with `Uint8Array` before/after, one `raw_write` event with
  region), `OutOfBounds` code, `readRaw` round trip and `BadArgument`,
  `corruption()` null then a message after zeroing sector 0; code
  uniqueness.
- web/ui (vitest, real wasm): store seek round trip with a `writeRaw` op;
  layout refresh after a root-entries change; bytes conversions including
  echo's list form; addr and size parsing; vfs normalization,
  classification, canonicalization; xxd format; dd parsing and the 1 MiB
  cap; every command's happy path and error messages (no command prefix);
  the rewound warning; the corruption round trip through `dd`; `write
  --append` rewrites; `cp` allocates a new cluster; `inTextEntry` and
  `clampHeight`; the scenario runs on a fresh volume.
- Manual (built-in browser): the drawer, shortcuts, the write/cat/stat
  loop, rewound reads, raw sector reads and the label patch, the corruption
  round trip, error display, drag/resize/persist/dark mode/HMR, and the
  `VITE_BASE=/fs-emulator/` preview showing the second `.wasm` under the
  base path.

## Future work (out of scope)

- browser-terminal: redirection with host hooks, `key=value` barewords, a
  bytes value, session id on `ctx`, theme options, `focus()`.
- Shell: `--at-step` reads of the cached image, `mv` once rename exists in
  the core, `truncate`, tab completion of `/mnt` paths.
