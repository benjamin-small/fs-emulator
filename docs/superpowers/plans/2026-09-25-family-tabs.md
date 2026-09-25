# Family Tabs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give FAT16 and ext each their own tab and workspace in the fs explorer (volume, timeline, selection, dump position, lesson, terminal cwd), remove every control that could flip the family inside a tab, and replace the Step strip and Timeline footer with a one-line Operation bar and a What changed panel that shows a step's changed blocks as ranges and its events grouped into phases.

**Architecture:** The four stores (`VolumeStore`, `SelectionStore`, `LayersStore`, `ScenarioRunner`) take their siblings by constructor and live once per family inside a `Workspace`; a `WorkspaceRegistry` creates the ext workspace lazily and names the active one; each opened workspace renders its own `WorkspaceView` subtree (kept mounted, hidden when inactive) that provides its stores through Svelte context, so components keep their bodies and swap one import line. A `TabBar` in the top bar switches workspaces, the URL hash names the tab, and one terminal re-registers its commands per workspace. Inside a tab the Format form, the lesson picker, and `mkfs` know only that tab's family; Load image routes an image to its family's tab. The Operation bar merges the timeline controls with the step summary, and the What changed panel groups a record's events by the phases the emulator already reports.

**Tech Stack:** Svelte 5.57 runes + TypeScript strict (`verbatimModuleSyntax`), Vite 6, Vitest 3 in node loading the real wasm package, pnpm 12 (any pnpm 10.26 or newer), `@benjamin-small/browser-terminal@0.3.0` (exact). `crates/` is untouched.

**Spec:** `docs/superpowers/specs/2026-09-25-family-tabs-design.md` (binding), with `docs/superpowers/specs/2026-09-24-ext-explorer-design.md` and `2026-09-23-fs-adapter-design.md` binding for the seam and the ext panels except where the new spec's section 7 amends them.

## Global Constraints

1. FAT16 behaviour, every FAT shell output string, every lesson string, and every attribution colour stay identical below the components. Sanctioned expectation changes, and only these: the `mkfs` summary and `--label` description becoming per family (`Format /dev/hda (clears the timeline); --type picks fat16`; `volume label, up to 11 characters`), the `mkfs --type` help on FAT reading `types: fat16`, the `mkfs --type` flag description on FAT reading `fat16 (default: the mounted volume's type)` and FAT's `mkfs` flag list holding only FAT16's six flags after `--type` (no ext flags), the FAT/ext `mkfs` spec equality pin in `tests/shell/help-text.test.ts` flipping to inequality, cross-family `mkfs --type` cases rebased on a host of the target family, `TestHost.format` refusing another family, the ids `action-path`/`step-heading` becoming per tab (`action-path-{id}`, `opbar-heading-{id}`), `extFundamentals.title` becoming `The fundamentals`, and the two lesson sentences that named the removed Step strip naming the What changed panel instead (`fundamentals.ts` "Putting it together": `…watch the ribbon and the What changed panel to see the same three places, slot, table, and data, move each time.`; `journaledWrite.ts` "One create, one transaction": `The What changed panel lists the create's events, grouped by phase, in the order they happened; the next steps follow them.`). FAT16 stays `DEFAULT_FAMILY` and the default tab.
2. Names, signatures, file paths, strings, and DTO shapes are the spec's and the contract's, verbatim. `fs/ext` is imported only by `fs/index.ts`, `fs/panels.ts`, and `scenarios/**`; the ext-only wasm surface is called only under `src/fs/ext/**` and `src/lib/wasm.ts`; `tests/adapterBoundary.test.ts` stays green unchanged.
3. Reactivity rule: adapter caches are plain fields; every `$derived` or `$effect` that calls an adapter method reads `volume.epoch` first. Store instances are provided through Svelte context (`getWorkspace()`), never through module singletons; a workspace is created only inside `WorkspaceRegistry.activate`, never in a getter or a `$derived`.
4. One `LessonPanel`, one `TerminalPanel`, one `ScenarioPanel`, and one `StatusLine` exist per page, scoped to the active workspace; every opened workspace's `WorkspaceView` stays mounted and is hidden with the `hidden` attribute (`.workspace:not([hidden]) { display: contents; }`).
5. `crates/` is not changed by this plan. `web/ui`: Svelte 5 runes, TypeScript strict with `verbatimModuleSyntax`, Vitest in the node environment loading the real wasm package (nothing a node test imports may import a `.svelte` or `.svelte.ts` file); no new dependencies; `pnpm test` and `pnpm build` green at every task's commit.
6. pnpm: any pnpm 10.26 or newer (nvm's pnpm 12 is first on PATH; fine). Use `CI=true pnpm install --frozen-lockfile`; a frozen install must leave `pnpm-lock.yaml` and `pnpm-workspace.yaml` unmodified. The wasm package is unchanged by this plan; on a fresh checkout build it once (`wasm-pack build crates/wasm --target bundler`) before the install.
7. The terminal's tab banner goes through browser-terminal 0.3.0's private `paneManager.handleEvent`, guarded so a missing internal skips the banner and breaks nothing else; nothing else reaches into the library's internals.
8. One conventional commit per task, no attribution lines. Specs are binding: where task text and spec differ, the spec wins.
9. Reading the tasks: the `plan/tabs-t*` branches are the writers' proofs and `plan/tabs-fix-a` carries the review fixes; the task text is the authority and already includes those fixes, so the seven texts replayed in order reproduce `plan/tabs-fix-a` exactly. Line ranges in a task's **Files** block refer to the tree that task's text produces; later tasks shift them, so use the anchor text. Expected test counts are the ones observed on the task's base branch in sequence (Task 1 on the spec commit; Tasks 2, 3, 4 each on Task 1; Task 5 on the merge of Tasks 1 to 4; Task 6 on Task 5; Task 7 on Task 6); when Tasks 2, 3, and 4 run in sequence the later ones see the earlier ones' tests too, so their counts are higher than printed by exactly the earlier tasks' additions (Task 2 adds 13 tests, Task 3 adds 7, Task 4 adds 7), and svelte-check's file count is higher by the earlier tasks' new files (Task 2 adds 4, Task 4 adds 2).

---

### Task 1: Per-family workspaces with explicit context (FAT16 only, no visible change)

Spec: `docs/superpowers/specs/2026-09-25-family-tabs-design.md` sections 2 and 3 (the `Workspace`/`WorkspaceRegistry` state, the store constructors, `core/route.ts`, `FAMILY_IDS`, `WorkspaceView`/`WorkspaceScope`, App's scoped chrome and `{#each}` bodies, the per-tab ids, the import-line swaps). The tab bar, the hash writing, `keepScroll`, `observeWidth`, `loadImage`, and the terminal's per-workspace hosts are later tasks. At the end of this task only the FAT16 workspace is ever opened (a `#ext` hash at load opens the ext one instead, with no tab bar yet), and the page looks and behaves as before for everything FAT16.

**Files**

- Create: `web/ui/src/core/route.ts` (1-19)
- Create: `web/ui/src/state/workspace.svelte.ts` (1-89)
- Create: `web/ui/src/components/WorkspaceView.svelte` (1-43)
- Create: `web/ui/src/components/WorkspaceScope.svelte` (1-15)
- Create (test): `web/ui/tests/route.test.ts` (1-47)
- Delete: `web/ui/src/state/navigate.svelte.ts`
- Modify: `web/ui/src/fs/index.ts` (6-15)
- Modify: `web/ui/src/state/selection.svelte.ts` (whole file, 1-42)
- Modify: `web/ui/src/state/volume.svelte.ts` (whole file, 1-196)
- Modify: `web/ui/src/state/layers.svelte.ts` (whole file, 1-55)
- Modify: `web/ui/src/state/scenarios.svelte.ts` (1-5, 38-130)
- Modify: `web/ui/src/App.svelte` (whole file, 1-82)
- Modify: `web/ui/src/app.css` (30-33)
- Modify (import line only): `web/ui/src/components/{DirTree,HexView,Inspector,Ribbon,StatusLine,StringsPanel,TreeNodeView}.svelte`, `web/ui/src/fs/fat16/{FatMap,FormatForm}.svelte`, `web/ui/src/fs/ext/{BlockGroupMap,ExtFormatForm,JournalPanel}.svelte` (the first 12 lines of each script)
- Modify: `web/ui/src/components/LessonPanel.svelte` (8-10, 91-93, 99)
- Modify: `web/ui/src/components/ActionsPanel.svelte` (1-8, 86)
- Modify: `web/ui/src/components/StepPanel.svelte` (1-6, 16-17)
- Modify: `web/ui/src/components/Timeline.svelte` (1-6, 22)
- Modify: `web/ui/src/components/ScenarioPanel.svelte` (1-5, 17-24)
- Modify: `web/ui/src/shell/storeHost.svelte.ts` (1-18)
- Modify: `web/ui/src/components/TerminalPanel.svelte` (19, 83-87, 129-134, 148-153)
- Modify (test): `web/ui/tests/fs/registry.test.ts` (2-3, 29-41)
- Modify (test): `web/ui/tests/layout.test.ts` (20-26)
- Modify (test): `web/ui/tests/shell/helpers.ts` (12-13, 49-50)
- Modify (test): `web/ui/tests/shell/mutations.test.ts` (278-281, 296-314, 339-340, 369-376)
- Modify (test): `web/ui/tests/shell/journal.test.ts` (194-199, 208-209)

**Interfaces**

Consumes (existing): `FAMILIES`, `DEFAULT_FAMILY`, `familyIdOf`, `adapterFor` (`src/fs/index.ts`); `MOUNT`, `Vfs` (`src/shell/vfs.ts`); `stepFocusOffset` (`src/core/stepFocus.ts`); `createContext` (Svelte 5.57, `svelte`); `commandSetOf`, `registerCommands` (`src/shell/register.ts`).

Produces:

```ts
// src/core/route.ts
export function parseRoute<T extends string>(hash: string, known: readonly T[], aliases: Readonly<Record<string, T>> = {}): T | null;
export function formatRoute(id: string): string;
// src/fs/index.ts
export const FAMILY_IDS: FsFamilyId[];                                  // ["fat16", "ext"]
export const ROUTE_ALIASES: Readonly<Record<string, FsFamilyId>>;       // { fat: "fat16", ext2: "ext", ext3: "ext" }
// src/state/selection.svelte.ts
export class SelectionStore { constructor(clearMessages: () => void); /* select() calls clearMessages */ }
// src/state/volume.svelte.ts
export class VolumeStore {
  readonly family: FsFamilyId;
  notice: string | null;                                                 // $state
  constructor(family: FsFamilyId, selection: SelectionStore, onAdopt: () => void);
  format(family?: FsFamilyId, options?: unknown): void;                  // another family -> status "this tab formats fat16, not ext"
  mount(vol: Volume): void;                                              // throws "this tab mounts fat16, not ext" for another family
  load(bytes: Uint8Array): void;
  report(e: unknown): void;
  clearMessages(): void;
}
// src/state/layers.svelte.ts
export class LayersStore { constructor(volume: VolumeStore, selection: SelectionStore) }
// src/state/scenarios.svelte.ts
export class ScenarioRunner { constructor(volume: VolumeStore, selection: SelectionStore) }  // start(s) throws "lesson <id> is for <family>; this tab is <family>"
// src/state/workspace.svelte.ts
export class Workspace {
  readonly id: FsFamilyId; readonly vfs: Vfs; readonly selection: SelectionStore; readonly volume: VolumeStore; readonly layers: LayersStore; readonly scenarios: ScenarioRunner;
  constructor(id: FsFamilyId, registry: WorkspaceRegistry);
  get active(): boolean;
  goToStep(step: number): void;
}
export class WorkspaceRegistry {
  activeId: FsFamilyId;                 // $state
  opened: readonly Workspace[];         // $state.raw, creation order
  constructor(initial: FsFamilyId);
  get active(): Workspace;
  activate(id: FsFamilyId): Workspace;
}
export const workspaces: WorkspaceRegistry;
export const getWorkspace: () => Workspace;
export const setWorkspace: (ws: Workspace) => Workspace;
// src/components/WorkspaceView.svelte   props { ws: Workspace }
// src/components/WorkspaceScope.svelte  props { ws: Workspace; children: Snippet }
// src/shell/storeHost.svelte.ts
export function createStoreHost(ws: Workspace, closeTerminal: () => void, applyPrompt: (prefix: string) => void): ShellHost;
// DOM ids: action-path-{ws.id}, step-heading-{ws.id}, workspace-{ws.id}
// tests/shell/helpers.ts: TestHost.format(family, options) throws `this tab formats ${adapter.id}, not ${family}` for another family
```

The module singletons `volume`, `selection`, `layers`, `scenarios` and `src/state/navigate.svelte.ts` (`focusHistoryStep`) are gone; `terminal`, `theme`, and `lessonWindow` stay page-wide singletons.

Before the first test run in a fresh worktree: from the repo root `wasm-pack build crates/wasm --target bundler`, then in `web/ui` `CI=true pnpm install --frozen-lockfile`. Baseline in `web/ui`: `pnpm test` gives `Test Files 50 passed (50)`, `Tests 496 passed (496)`. All commands below run in `web/ui`.

- [ ] **Step 1: Write the failing route tests and the registry pins**

Create `web/ui/tests/route.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { formatRoute, parseRoute } from "../src/core/route";

const KNOWN = ["fat16", "ext"] as const;
const ALIASES = { fat: "fat16", ext2: "ext", ext3: "ext" } as const;

describe("parseRoute", () => {
  it("reads a known name after the hash, in any case", () => {
    expect(parseRoute("#fat16", KNOWN)).toBe("fat16");
    expect(parseRoute("#ext", KNOWN)).toBe("ext");
    expect(parseRoute("#EXT", KNOWN)).toBe("ext");
    expect(parseRoute("ext", KNOWN)).toBe("ext"); // the hash itself is optional
  });

  it("strips one leading and one trailing slash", () => {
    expect(parseRoute("#/ext", KNOWN)).toBe("ext");
    expect(parseRoute("#ext/", KNOWN)).toBe("ext");
    expect(parseRoute("#/fat16/", KNOWN)).toBe("fat16");
    expect(parseRoute("#//ext", KNOWN)).toBeNull();
    expect(parseRoute("##ext", KNOWN)).toBeNull();
  });

  it("looks the name up in the aliases before the known names", () => {
    expect(parseRoute("#fat", KNOWN, ALIASES)).toBe("fat16");
    expect(parseRoute("#ext2", KNOWN, ALIASES)).toBe("ext");
    expect(parseRoute("#EXT3", KNOWN, ALIASES)).toBe("ext");
    expect(parseRoute("#ext3", KNOWN)).toBeNull(); // no aliases given, none apply
    expect(parseRoute("#fat16", KNOWN, { fat16: "ext" })).toBe("ext");
  });

  it("answers null for an empty hash, an unknown name, or an inherited key", () => {
    expect(parseRoute("", KNOWN, ALIASES)).toBeNull();
    expect(parseRoute("#", KNOWN, ALIASES)).toBeNull();
    expect(parseRoute("#/", KNOWN, ALIASES)).toBeNull();
    expect(parseRoute("#ntfs", KNOWN, ALIASES)).toBeNull();
    expect(parseRoute("#constructor", KNOWN, ALIASES)).toBeNull();
    expect(parseRoute("#toString", KNOWN, ALIASES)).toBeNull();
  });
});

describe("formatRoute", () => {
  it("writes the id after a hash, and parseRoute reads it back", () => {
    expect(formatRoute("ext")).toBe("#ext");
    expect(formatRoute("fat16")).toBe("#fat16");
    for (const id of KNOWN) expect(parseRoute(formatRoute(id), KNOWN, ALIASES)).toBe(id);
  });
});
```

In `web/ui/tests/fs/registry.test.ts`, replace the first registry import:

```ts
import { DEFAULT_FAMILY, FAMILIES, adapterFor, familyIdOf } from "../../src/fs";
```

with:

```ts
import { DEFAULT_FAMILY, FAMILIES, FAMILY_IDS, ROUTE_ALIASES, adapterFor, familyIdOf } from "../../src/fs";
import { formatRoute, parseRoute } from "../../src/core/route";
```

and insert, right before `  it("registers FAT16 as the default family, whose format() yields a volume of its own name", () => {`:

```ts
  it("lists the family ids in registry order, the order of the tabs", () => {
    expect(FAMILY_IDS).toEqual(["fat16", "ext"]);
    expect(FAMILY_IDS).toEqual(Object.keys(FAMILIES));
  });

  it("routes the short and the type names to a family's tab, and each id to its own", () => {
    expect(ROUTE_ALIASES).toEqual({ fat: "fat16", ext2: "ext", ext3: "ext" });
    for (const id of FAMILY_IDS) expect(parseRoute(formatRoute(id), FAMILY_IDS, ROUTE_ALIASES)).toBe(id);
    expect(parseRoute("#ext3", FAMILY_IDS, ROUTE_ALIASES)).toBe("ext");
    expect(parseRoute("#FAT", FAMILY_IDS, ROUTE_ALIASES)).toBe("fat16");
    expect(parseRoute("", FAMILY_IDS, ROUTE_ALIASES)).toBeNull();
  });

```

Run: `pnpm test`
Expected: `FAIL  tests/route.test.ts` and `FAIL  tests/fs/registry.test.ts` (the `src/core/route` import does not resolve), `Test Files  2 failed | 49 passed (51)`, `Tests  487 passed (487)`.

- [ ] **Step 2: Implement `core/route.ts`, `FAMILY_IDS`, and `ROUTE_ALIASES`**

Create `web/ui/src/core/route.ts`:

```ts
/**
 * The URL hash names a tab: `#fat16`, `#ext`. `parseRoute` reads a hash forgivingly (the `#`
 * is optional, one leading and one trailing `/` are dropped, case is ignored, and `aliases`
 * map other spellings such as `#ext3` onto an id); anything else is null, and the caller
 * falls back to its default. `formatRoute` writes the canonical form back.
 */
export function parseRoute<T extends string>(hash: string, known: readonly T[], aliases: Readonly<Record<string, T>> = {}): T | null {
  let name = hash.startsWith("#") ? hash.slice(1) : hash;
  if (name.startsWith("/")) name = name.slice(1);
  if (name.endsWith("/")) name = name.slice(0, -1);
  name = name.toLowerCase();
  // `Object.hasOwn`, not `in`: `#constructor` must not find Object.prototype's.
  if (Object.hasOwn(aliases, name)) return aliases[name];
  return (known as readonly string[]).includes(name) ? (name as T) : null;
}

export function formatRoute(id: string): string {
  return `#${id}`;
}
```

In `web/ui/src/fs/index.ts`, replace:

```ts
/** Every registered family, by id, in the order the Format details' Filesystem select lists
 *  them. A new family is added here and in `fs/panels.ts`. */
export const FAMILIES: Record<FsFamilyId, FsFamily> = { fat16, ext };
export const DEFAULT_FAMILY: FsFamilyId = "fat16";
```

with:

```ts
/** Every registered family, by id, in the order of the top bar's tabs. A new family is added
 *  here and in `fs/panels.ts`. */
export const FAMILIES: Record<FsFamilyId, FsFamily> = { fat16, ext };
/** The tab opened when the URL hash names none. */
export const DEFAULT_FAMILY: FsFamilyId = "fat16";
/** The family ids in tab order; a family's workspace is created the first time its tab opens. */
export const FAMILY_IDS = Object.keys(FAMILIES) as FsFamilyId[];
/** Other hash spellings that open a family's tab (`#fat`, `#ext2`, `#ext3`); `parseRoute`
 *  looks these up before the ids themselves. */
export const ROUTE_ALIASES: Readonly<Record<string, FsFamilyId>> = { fat: "fat16", ext2: "ext", ext3: "ext" };
```

Run: `pnpm test`
Expected: `Test Files  51 passed (51)`, `Tests  503 passed (503)`.

- [ ] **Step 3: Write the failing test: the test host refuses another family**

In `web/ui/tests/shell/mutations.test.ts`, at the end of `describe("format through the host", ...)`, after the `"refuses an inherited key like \"constructor\" with the same named error"` test, add:

```ts

  it("refuses another family, like the tab's VolumeStore, and leaves the volume alone", () => {
    const { host } = setup();
    expect(() => host.format("ext", { variant: "ext3" })).toThrow("this tab formats fat16, not ext");
    expect(() => makeHost(Volume.formatExt3(undefined)).format("fat16")).toThrow("this tab formats ext, not fat16");
    expect(host.vol.fsType()).toBe("FAT16");
    expect(host.formats).toEqual([]);
  });
```

Run: `pnpm test`
Expected: `FAIL  tests/shell/mutations.test.ts > format through the host > refuses another family, like the tab's VolumeStore, and leaves the volume alone`, `Tests  1 failed | 503 passed (504)`.

- [ ] **Step 4: Make `TestHost.format` refuse another family**

In `web/ui/tests/shell/helpers.ts`, replace the class comment's line pair:

```ts
 * cursor at the end, and refreshes the adapter. `format` re-binds the adapter
 * the way `VolumeStore.adopt` does. `rewind` only moves the cursor (the volume
```

with:

```ts
 * cursor at the end, and refreshes the adapter. `format` re-binds the adapter
 * the way `VolumeStore.adopt` does, and like the tab's store refuses another
 * family. `rewind` only moves the cursor (the volume
```

and in `format`, replace the `Object.hasOwn` line with these three lines (the first is that line, unchanged):

```ts
    if (!Object.hasOwn(FAMILIES, family)) throw new Error(`no filesystem family "${family}"`);
    // A tab formats its own family only, as VolumeStore.format does.
    if (family !== this.adapter.id) throw new Error(`this tab formats ${this.adapter.id}, not ${family}`);
```

Run: `pnpm test`
Expected: the new test passes and four tests that format across families on one host fail: `journal.test.ts > re-registration when the family changes > swaps every name for the new command set (fat16, ext3, ext2, fat16), keeping the working directory`, `mutations.test.ts > mkfs --type > formats ext3 from a FAT16 host: …`, `mutations.test.ts > mkfs --type > formats ext2 and back to fat16, naming each type in the done line`, `mutations.test.ts > mkfs --type > surfaces the family's own error through fsCall, and refuses an unknown type` (`expected '/dev/hda: this tab formats fat16, not…' to be '/dev/hda: journalBlocks and journalMo…'`); `Tests  4 failed | 500 passed (504)`.

- [ ] **Step 5: Rebase the four cross-family tests on a host of the family each formats (two files)**

`mkfs` still offers every family's types until Task 3; until then another family's type reaches the host and the host refuses it. In `web/ui/tests/shell/mutations.test.ts`, replace:

```ts
  it("formats ext3 from a FAT16 host: the ext family with its variant, the done line, cwd and prompt reset", async () => {
    const { host, vfs, defs } = setup();
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
```

with:

```ts
  it("formats ext3 from an ext2 host: the ext family with its variant, the done line, cwd and prompt reset", async () => {
    const host = makeHost(Volume.formatExt2(undefined));
    const vfs = new Vfs();
    const defs = createCommands(host, vfs);
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
```

replace the whole `"formats ext2 and back to fat16, naming each type in the done line"` test:

```ts
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
```

with these two tests:

```ts
  it("formats ext2 on an ext3 host and FAT16 on a FAT16 host, naming each type in the done line", async () => {
    const ext = makeHost(Volume.formatExt3(undefined));
    expect((await call(createCommands(ext), "mkfs", { flags: { type: "ext2" } })).log).toEqual(["formatted /dev/hda as ext2; the timeline was cleared"]);
    expect(ext.adapter.journal).toBeUndefined();
    expect(ext.formats).toEqual([{ family: "ext", options: { variant: "ext2" } }]);
    const { host, defs } = setup();
    expect((await call(defs, "mkfs", { flags: { type: "FAT16", sectors: 8192, spc: 1 } })).log).toEqual(["formatted /dev/hda as FAT16; the timeline was cleared"]);
    expect(host.vol.geometry().totalSectors).toBe(8192);
    expect(host.formats).toEqual([{ family: "fat16", options: { totalSectors: 8192, sectorsPerCluster: 1 } }]);
  });

  it("passes another family's type to the host, which refuses it and leaves the volume alone", async () => {
    const { host, vfs, defs } = setup();
    vfs.cwd = "/mnt/D";
    const e = await callErr(defs, "mkfs", { flags: { type: "ext3" } });
    expect(e.message).toBe("/dev/hda: this tab formats fat16, not ext");
    expect(host.vol.fsType()).toBe("FAT16");
    expect(host.formats).toEqual([]);
    expect(vfs.cwd).toBe("/mnt/D");
  });
```

and in `"surfaces the family's own error through fsCall, and refuses an unknown type"` replace:

```ts
    const { host, defs } = setup();
    expect((await callErr(defs, "mkfs", { flags: { type: "ext2", "journal-mode": "data" } })).message).toBe("/dev/hda: journalBlocks and journalMode need variant ext3");
```

with:

```ts
    const { host, defs } = setup();
    const ext = createCommands(makeHost(Volume.formatExt3(undefined)));
    expect((await callErr(ext, "mkfs", { flags: { type: "ext2", "journal-mode": "data" } })).message).toBe("/dev/hda: journalBlocks and journalMode need variant ext3");
```

In `web/ui/tests/shell/journal.test.ts` (`"swaps every name for the new command set (fat16, ext3, ext2, fat16), keeping the working directory"`), replace:

```ts
    await call(createCommands(host, vfs), "mkfs", { flags: { type: "ext3" } });
    await call(createCommands(host, vfs), "mkdir", { positionals: ["/mnt/d"] });
    vfs.cwd = "/mnt/d";
    calls.length = 0;
    const ext3 = registerCommands(term, host, vfs, fat);
```

with:

```ts
    // Each tab formats its own family only, so the ext3 set comes from an ext host.
    const ext = makeHost(Volume.formatExt3(undefined));
    await call(createCommands(ext, vfs), "mkdir", { positionals: ["/mnt/d"] });
    vfs.cwd = "/mnt/d";
    calls.length = 0;
    const ext3 = registerCommands(term, ext, vfs, fat);
```

and replace:

```ts
    host.format("ext", { variant: "ext2" });
    const ext2 = registerCommands(term, host, vfs, ext3);
    expect(ext2).toEqual(ext3.slice(0, -2));
    expect(registered.has("crash")).toBe(false);
    expect(registered.has("recover")).toBe(false);

    host.format("fat16");
    expect(registerCommands(term, host, vfs, ext2)).toEqual(fat);
```

with:

```ts
    ext.format("ext", { variant: "ext2" });
    const ext2 = registerCommands(term, ext, vfs, ext3);
    expect(ext2).toEqual(ext3.slice(0, -2));
    expect(registered.has("crash")).toBe(false);
    expect(registered.has("recover")).toBe(false);

    expect(registerCommands(term, host, vfs, ext2)).toEqual(fat);
```

Run: `pnpm test`
Expected: `Test Files  51 passed (51)`, `Tests  505 passed (505)`.

- [ ] **Step 6: Write the failing layout guard for the tab body wrapper**

In `web/ui/tests/layout.test.ts`, insert before `  it("gives the side terminal column an explicit width without redefining the main column", () => {`:

```ts
  it("lays a shown tab's body out in the app grid, and leaves a hidden one to the UA's rule", () => {
    // Each tab's body sits in a `.workspace` wrapper; as a box it would be one grid item and the
    // step strip, ribbon, grid, and footer would lose their rows. `:not([hidden])`, because an
    // author `display` would override the UA's `[hidden] { display: none }`.
    expect(rule(".workspace:not([hidden])")).toBe(" display: contents; ");
  });

```

Run: `pnpm test`
Expected: `FAIL  tests/layout.test.ts > app.css layout guards > lays a shown tab's body out in the app grid, and leaves a hidden one to the UA's rule`, `Tests  1 failed | 505 passed (506)`.

- [ ] **Step 7: Add the `.workspace` rule**

In `web/ui/src/app.css`, after `.app.term-side { grid-auto-columns: var(--term-w, 500px); }` insert:

```css
/* A tab's body (App.svelte's `.workspace` wrapper) adds no box: its step strip, ribbon, grid,
   and footer are the app grid's rows. `:not([hidden])`, since an author `display` would
   override the UA's `[hidden] { display: none }` and show every tab at once. */
.workspace:not([hidden]) { display: contents; }
```

Run: `pnpm test`
Expected: `Test Files  51 passed (51)`, `Tests  506 passed (506)`.

Steps 8 to 17 change `.svelte.ts` and `.svelte` files, which no node test may import, so `pnpm test` stays at `Tests  506 passed (506)` throughout; `svelte-check` fails from Step 8 until Step 17 is done (the singleton exports disappear before their importers change), and Step 18 runs the gates. The store behaviour Steps 8 to 12 add cannot be node-tested for the same reason (the shell side of the family guard is pinned through `TestHost` in Steps 3 to 5); Step 19's browser checks are its verification, each naming the step it covers.

- [ ] **Step 8: `SelectionStore` takes `clearMessages`; no singleton**

Replace `web/ui/src/state/selection.svelte.ts` with:

```ts
import { ScrollNonces } from "../core/scrollNonce";

export class SelectionStore {
  path = $state<string | null>(null);
  cursorOffset = $state<number | null>(null);
  hoverOffset = $state<number | null>(null);
  stringsOn = $state(false);
  showRemnants = $state(false);
  expandedGaps = $state(new Set<number>());
  scrollTarget = $state<{ offset: number; nonce: number } | null>(null);
  // One counter for the store's life: `reset()` leaves it running, because the dump remembers
  // the last nonce it scrolled to across a reset (see `ScrollNonces`).
  private readonly nonces = new ScrollNonces();
  /** Clears the tab's status line (its VolumeStore's `clearMessages`). Injected, because the
   *  workspace builds the volume store after this one. */
  private readonly clearMessages: () => void;

  constructor(clearMessages: () => void) {
    this.clearMessages = clearMessages;
  }

  /** Move the byte cursor and ask the dump to scroll there, even to the offset it is at: every
   *  request carries a fresh nonce. Reads no state, so an `$effect` may call it. */
  jumpTo(offset: number) { this.cursorOffset = offset; this.scrollTarget = { offset, nonce: this.nonces.next() }; }

  select(path: string | null) {
    this.path = path;
    // The old error described the action that failed, not this selection.
    this.clearMessages();
  }

  expandGap(startSector: number) { const s = new Set(this.expandedGaps); s.add(startSector); this.expandedGaps = s; }

  /** Drop everything tied to the bytes of a particular disk (after a format or load). */
  reset() {
    this.path = null;
    this.cursorOffset = null;
    this.hoverOffset = null;
    this.expandedGaps = new Set();
    this.scrollTarget = null;
  }
}
```

(`tests/scrollNonce.test.ts` reads this file's source; it stays green.)

- [ ] **Step 9: `VolumeStore` is one family's: `family`, `notice`, `mount`, `report`, `clearMessages`**

Replace `web/ui/src/state/volume.svelte.ts` with:

```ts
import { Volume, type FsError, type OpRecord, type Region } from "../lib/wasm";
import { buildAttribution, type AttributionTable } from "../core/attribution";
import { applyChanges, changedSectors } from "../core/patch";
import { rescanSectors, scanZeroSectors } from "../core/zeros";
import { FAMILIES, adapterFor, familyIdOf } from "../fs";
import type { CrashPhase, FsAdapter, FsFamilyId } from "../fs/adapter";
import type { SelectionStore } from "./selection.svelte";

/** One tab's disk: the mounted volume, its cached image, and its timeline. Every format and
 *  every mount is of the tab's `family`; the constructor mounts that family's default disk. */
export class VolumeStore {
  readonly family: FsFamilyId;
  // `vol` and `adapter` are set by `adopt` in the constructor, before anything can read them.
  vol = $state.raw<Volume>()!;
  /** The per-family view of `vol`: owners, tables, unit arithmetic. Its caches are plain
   *  fields, not runes, so a `$derived` that calls a method on it must read `epoch` first or
   *  it keeps answering from the previous op (the rule stated in fs/adapter.ts). `adopt`
   *  re-binds it for a new volume; `refreshMeta` re-reads it after every op. */
  adapter = $state.raw<FsAdapter>()!;
  image = $state.raw<Uint8Array>(new Uint8Array(0));
  epoch = $state(0);
  layout = $state.raw<Region[]>([]);
  sectorSize = $state(0);
  totalSectors = $state(0);
  zeros = $state.raw<Uint8Array>(new Uint8Array(0));
  history = $state.raw<OpRecord[]>([]);
  cursor = $state(-1);
  status = $state<{ text: string; code?: string } | null>(null);
  /** News for the status line that is not an error; cleared with `status`. */
  notice = $state<string | null>(null);
  /** The `CorruptImage` message while a raw write has left the on-disk metadata unparsable,
   *  `null` while mounted. Refreshed alongside the adapter, so it tracks the volume
   *  through every op, format, and load. */
  corruption = $state<string | null>(null);
  /** True while the journal holds an unfinished transaction (the adapter's `needsRecovery`;
   *  always false on FAT and ext2). Refreshed with the adapter, like `corruption`. */
  needsRecovery = $state(false);
  /** The crash phase armed in the mounted journal, or null (always null without a journal).
   *  Arming is not an op, so it moves neither the timeline nor `epoch`: this field is how the
   *  Journal panel shows a phase the terminal's `crash` armed, and the reverse. `refreshMeta`
   *  re-reads it, because the next op uses the phase up and a format or load binds a new journal. */
  armedPhase = $state<CrashPhase | null>(null);

  // `epoch` is read first on purpose: `adapter.owners` is a plain field, so nothing else
  // in this expression would re-run it after an op.
  attribution: AttributionTable = $derived.by(() => { this.epoch; return buildAttribution(this.adapter, this.layout, this.adapter.owners); });
  atLatest = $derived(this.cursor === this.history.length - 1);

  private readonly selection: SelectionStore;
  /** Called after every adopt (a format or a mount): the workspace puts its shell back at /mnt. */
  private readonly onAdopt: () => void;

  constructor(family: FsFamilyId, selection: SelectionStore, onAdopt: () => void) {
    this.family = family;
    this.selection = selection;
    this.onAdopt = onAdopt;
    this.adopt(FAMILIES[family].format());
  }

  /** Take over a fresh Volume: bind its adapter, full image copy, full scans, empty history.
   *  The family and the adapter are resolved before any field is assigned, so a `fsType()` of
   *  another family, or with no registered adapter, throws before the store is half-switched
   *  to the new volume. */
  private adopt(vol: Volume) {
    const id = familyIdOf(vol.fsType());
    if (id !== this.family) throw new Error(`this tab mounts ${this.family}, not ${id}`);
    const adapter = adapterFor(vol);
    this.vol = vol;
    this.adapter = adapter;
    this.image = vol.image();
    this.layout = vol.layout();
    this.sectorSize = vol.sectorSize();
    this.totalSectors = vol.sectorCount();
    this.zeros = scanZeroSectors(this.image, this.sectorSize);
    this.history = [];
    this.cursor = -1;
    this.refreshMeta();
    this.epoch++;
    this.onAdopt();
  }

  private refreshMeta() {
    this.adapter.refresh();
    // `corruption()` is on the `FileSystem` trait, so every family answers it and it cannot
    // throw `NotFat`; no guard is needed.
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
    } catch (e) { this.report(e); return false; }
    this.armedPhase = journal.phase();
    return true;
  }

  /** A fresh volume of the tab's family with that family's options. `family` is kept for the
   *  callers that name one (the Format forms, `mkfs`, a lesson step); another family is refused
   *  into `status`, like any other format failure. */
  format(family: FsFamilyId = this.family, options?: unknown) {
    try {
      if (!Object.hasOwn(FAMILIES, family)) throw new Error(`no filesystem family "${family}"`);
      if (family !== this.family) throw new Error(`this tab formats ${this.family}, not ${family}`);
      this.mount(FAMILIES[family].format(options));
    } catch (e) {
      this.report(e);
    }
  }

  /** Take over `vol` as the tab's disk, clearing the messages and the selection. Throws, and
   *  leaves the store alone, when `vol` is another family's or no adapter handles its type. */
  mount(vol: Volume) {
    this.adopt(vol);
    this.clearMessages();
    this.selection.reset();
  }

  /** Mount an image. An unregistered or another family's `fsType()` makes `mount` throw, which
   *  lands in `status` like any other load failure. */
  load(bytes: Uint8Array) {
    try { this.mount(Volume.fromImage(bytes)); } catch (e) { this.report(e); }
  }

  export(): Uint8Array { return this.vol.image(); }

  /** Run a mutation at the latest state. Returns the record, or null on failure (status set). */
  run(fn: (v: Volume) => OpRecord): OpRecord | null {
    if (!this.atLatest) this.backToNow();
    let rec: OpRecord;
    try { rec = fn(this.vol); } catch (e) { this.report(e); return null; }
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
    this.clearMessages();
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
    this.clearMessages();
    this.epoch++;
  }

  backToNow() { this.seek(this.history.length - 1); }

  /** Put `e` (a wasm `FsError` or any Error) on the status line. */
  report(e: unknown) {
    const err = e as Partial<FsError>;
    this.status = { text: err.message ?? String(e), code: err.code };
  }

  /** Clear the status line: the error and the notice. */
  clearMessages() {
    this.status = null;
    this.notice = null;
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
```

`$state.raw<Volume>()!` compiles to an empty state cell (the TS `!` is stripped before Svelte sees it); `adopt` fills it in the constructor. `this.adopt(...)` runs after every field initializer, so `this.selection` and `this.onAdopt` are assigned first; the `$derived` fields are lazy.

- [ ] **Step 10: `LayersStore` takes the tab's volume and selection**

Replace `web/ui/src/state/layers.svelte.ts` with:

```ts
import { intervalsToSectors, normalize, type Interval } from "../core/intervals";
import { GAP_REVEAL } from "../core/segments";
import type { SelectionStore } from "./selection.svelte";
import type { VolumeStore } from "./volume.svelte";

export class LayersStore {
  /** Extra selection ranges other stores contribute (Task 6 adds the entry slot). */
  extraSel = $state.raw<Interval[]>([]);
  str = $state.raw<Interval[]>([]);
  visible = $state.raw<Interval>({ start: 0, end: 0 });

  // The tab's stores. The deriveds below read them lazily, after the constructor has run; they
  // are all `$derived.by` so the type checker sees the reads happen inside a function.
  private readonly volume: VolumeStore;
  private readonly selection: SelectionStore;

  constructor(volume: VolumeStore, selection: SelectionStore) {
    this.volume = volume;
    this.selection = selection;
  }

  diff: Interval[] = $derived.by(() => {
    const rec = this.volume.history[this.volume.cursor];
    return rec ? normalize(rec.changes.map((c) => ({ start: c.offset, end: c.offset + c.after.length }))) : [];
  });

  // Every derived below calls the adapter, whose caches are plain fields, so each one reads
  // `volume.epoch` first (the rule in fs/adapter.ts). Without it a delete would leave the old
  // chain highlighted until something else changed.
  chain: number[] = $derived.by(() => {
    const path = this.selection.path;
    if (!path) return [];
    this.volume.epoch;
    return this.volume.adapter.chain(path);
  });

  /** Byte range of the selected path's directory entry slots (FAT: LFN + short), if any. */
  entry: Interval | null = $derived.by(() => (this.selection.path ? (this.volume.epoch, this.volume.adapter.entrySlots(this.selection.path)) : null));

  /** Deleted directory slots and dirty free units, when the "Show remnants" toggle is on. A
   *  family without `adapter.remnants` has none. Empty while the timeline is rewound: the
   *  directory walk asks the live volume, so its slots would be from the latest state while
   *  the bytes on screen are from an older one. */
  remnant: Interval[] = $derived.by(() => (this.selection.showRemnants && this.volume.atLatest ? (this.volume.epoch, this.volume.adapter.remnants?.(this.volume.zeros) ?? []) : []));

  sel: Interval[] = $derived.by(() => (this.volume.epoch, normalize([...this.chain.map((u) => this.volume.adapter.unitByteRange(u)), ...(this.entry ? [this.entry] : []), ...this.extraSel])));

  pinnedSectors: Set<number> = $derived.by(() => {
    const { volume, selection } = this;
    const s = new Set<number>([...intervalsToSectors(this.diff, volume.sectorSize), ...intervalsToSectors(this.sel, volume.sectorSize), ...intervalsToSectors(this.remnant, volume.sectorSize)]);
    if (selection.cursorOffset !== null) s.add(Math.floor(selection.cursorOffset / volume.sectorSize));
    for (const g of selection.expandedGaps) for (let i = 0; i < GAP_REVEAL; i++) s.add(g + i);
    return s;
  });
}
```

(The plain `$derived(expr)` form of `entry`, `remnant`, and `sel` would make svelte-check report `Property 'volume' is used before its initialization`; `$derived.by` is the same derived.)

- [ ] **Step 11: `ScenarioRunner` takes the tab's volume and selection and refuses another family's lesson**

In `web/ui/src/state/scenarios.svelte.ts`, replace:

```ts
import { selection } from "./selection.svelte";
import { volume } from "./volume.svelte";
```

with:

```ts
import type { SelectionStore } from "./selection.svelte";
import type { VolumeStore } from "./volume.svelte";
```

delete the last line `export const scenarios = new ScenarioRunner();` (and the blank line before it), and replace the class from its comment to its end with:

```ts
/** Drives a tab's `volume`/`selection` through a scenario's steps, one at a time. */
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

  private readonly volume: VolumeStore;
  private readonly selection: SelectionStore;

  constructor(volume: VolumeStore, selection: SelectionStore) {
    this.volume = volume;
    this.selection = selection;
  }

  /** Begin `s` from a clean default disk of its family, then apply its first step. A lesson
   *  runs in its own family's tab only. */
  start(s: Scenario) {
    if (s.family !== this.volume.family) throw new Error(`lesson ${s.id} is for ${s.family}; this tab is ${this.volume.family}`);
    this.volume.format(s.family); // also resets the selection
    this.selection.reset();
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
      this.volume.seek(plan.seekTo);
      this.applyFocus(step);
      return;
    }
    this.applyStep(step);
    this.cursor.advance(this.volume.cursor);
  }

  /** Step back: show the disk as it was after the previous step and re-apply its focus.
   *  A step that formatted the disk cannot be rewound through (the format threw the old
   *  history away), so Prev clamps there. */
  prev() {
    if (!this.current || !this.cursor || this.index <= 0) return;
    if (this.current.steps[this.index].format) return;
    const target = this.cursor.back();
    this.index = this.cursor.index;
    if (target !== null) this.volume.seek(target);
    this.applyFocus(this.current.steps[this.index]);
  }

  stop() {
    this.current = null;
    this.index = -1;
    this.cursor = null;
    this.focus = null;
    this.selection.showRemnants = false;
    this.selection.stringsOn = false;
  }

  private applyStep(step: Step) {
    // `next()` is the only caller and returns early without a current scenario.
    if (step.format) this.volume.format(this.current!.family, step.format);
    if (step.action) this.volume.run((v) => step.action!(v, this.volume.adapter));
    this.applyFocus(step);
  }

  /** Every path that lands on a step ends here — a fresh run, a Next that seeks to a step
   *  that already ran, and Prev — so this is where the resolved focus is published. */
  private applyFocus(step: Step) {
    const focus = typeof step.focus === "function" ? step.focus(this.volume.adapter) : step.focus;
    this.focus = focus ?? null;
    if (!focus) return;
    if (focus.path !== undefined) this.selection.select(focus.path);
    if (focus.offset !== undefined) this.selection.jumpTo(focus.offset);
    else if (focus.sector !== undefined) this.selection.jumpTo(focus.sector * this.volume.sectorSize);
    else if (focus.unit !== undefined) this.selection.jumpTo(this.volume.adapter.unitByteRange(focus.unit).start);
    if (focus.showRemnants !== undefined) this.selection.showRemnants = focus.showRemnants;
    if (focus.strings !== undefined) this.selection.stringsOn = focus.strings;
  }
}
```

The file keeps `StepFocus`, `Step`, and `Scenario` above the class unchanged (the `scenarios/*.ts` files and `core/lesson.ts` import those types).

- [ ] **Step 12: Add `workspace.svelte.ts`; delete `navigate.svelte.ts`**

Create `web/ui/src/state/workspace.svelte.ts`:

```ts
import { createContext } from "svelte";
import { parseRoute } from "../core/route";
import { stepFocusOffset } from "../core/stepFocus";
import { DEFAULT_FAMILY, FAMILY_IDS, ROUTE_ALIASES } from "../fs";
import type { FsFamilyId } from "../fs/adapter";
import { MOUNT, Vfs } from "../shell/vfs";
import { LayersStore } from "./layers.svelte";
import { ScenarioRunner } from "./scenarios.svelte";
import { SelectionStore } from "./selection.svelte";
import { VolumeStore } from "./volume.svelte";

/**
 * One family's tab: its own disk and timeline, selection, layers, running lesson, and the
 * shell's working directory in it. Components reach it through `getWorkspace()`, which the
 * nearest `WorkspaceView` or `WorkspaceScope` sets, never through a module singleton, so each
 * tab's components talk to their own stores.
 */
export class Workspace {
  readonly id: FsFamilyId;
  readonly vfs = new Vfs();
  readonly selection: SelectionStore;
  readonly volume: VolumeStore;
  readonly layers: LayersStore;
  readonly scenarios: ScenarioRunner;
  private readonly registry: WorkspaceRegistry;

  constructor(id: FsFamilyId, registry: WorkspaceRegistry) {
    this.id = id;
    this.registry = registry;
    // In dependency order. The selection's callback reads `this.volume` only when it runs,
    // after the constructor has assigned it.
    this.selection = new SelectionStore(() => this.volume.clearMessages());
    this.volume = new VolumeStore(id, this.selection, () => { this.vfs.cwd = MOUNT; });
    this.layers = new LayersStore(this.volume, this.selection);
    this.scenarios = new ScenarioRunner(this.volume, this.selection);
  }

  /** Whether this is the tab on screen. */
  get active(): boolean {
    return this.registry.activeId === this.id;
  }

  /**
   * Scrub to `step` and recenter the dump on what that step changed. Running an operation
   * never moves the dump — only explicit navigation does — so this is called from the
   * Timeline's controls and from App.svelte's `[`/`]` shortcuts. A step with no changes (or an
   * index off either end of the history) leaves the dump alone.
   */
  goToStep(step: number) {
    this.volume.seek(step);
    const offset = stepFocusOffset(this.volume.history[step]);
    if (offset !== null) this.selection.jumpTo(offset);
  }
}

/** The page's workspaces, one per family, each created the first time it is activated. */
export class WorkspaceRegistry {
  activeId = $state<FsFamilyId>(DEFAULT_FAMILY);
  /** Every workspace created so far, in creation order. None is ever disposed. */
  opened = $state.raw<readonly Workspace[]>([]);

  constructor(initial: FsFamilyId) {
    this.activate(initial);
  }

  get active(): Workspace {
    return this.opened.find((w) => w.id === this.activeId)!;
  }

  /** Show `id`'s workspace, creating it first if this is its first time. An event handler or
   *  the constructor calls this, never a getter or a `$derived`: creating a workspace writes
   *  state. */
  activate(id: FsFamilyId): Workspace {
    let ws = this.opened.find((w) => w.id === id);
    if (!ws) {
      ws = new Workspace(id, this);
      this.opened = [...this.opened, ws];
    }
    this.activeId = id;
    return ws;
  }
}

/** The page's registry: the tab the URL hash names, else the default family's. */
export const workspaces = new WorkspaceRegistry(parseRoute(location.hash, FAMILY_IDS, ROUTE_ALIASES) ?? DEFAULT_FAMILY);

/** The workspace of the tab a component belongs to; `setWorkspace` is called by
 *  `WorkspaceView` and `WorkspaceScope` during their initialisation. */
export const [getWorkspace, setWorkspace] = createContext<Workspace>();
```

Delete the old navigation helper (its body is `Workspace.goToStep` now):

```sh
git rm web/ui/src/state/navigate.svelte.ts
```

- [ ] **Step 13: Add `WorkspaceScope.svelte` and `WorkspaceView.svelte`**

Create `web/ui/src/components/WorkspaceScope.svelte`:

```svelte
<script lang="ts">
  import type { Snippet } from "svelte";
  import { setWorkspace, type Workspace } from "../state/workspace.svelte";

  /** Gives `children` the workspace `ws` through `getWorkspace()`, without any DOM of its own:
   *  App uses it for the pieces of the page chrome that belong to the active tab (the lesson
   *  picker, the status line, the lesson card), keyed on the tab so they are rebuilt on a
   *  switch. */
  let { ws, children }: { ws: Workspace; children: Snippet } = $props();
  // Context is set once, at init; App's `{#key}` makes a new scope for another workspace.
  // svelte-ignore state_referenced_locally
  setWorkspace(ws);
</script>

{@render children()}
```

Create `web/ui/src/components/WorkspaceView.svelte` (today's App body below the top bar, unchanged markup):

```svelte
<script lang="ts">
  import { PANELS } from "../fs/panels";
  import { setWorkspace, type Workspace } from "../state/workspace.svelte";
  import ActionsPanel from "./ActionsPanel.svelte";
  import DirTree from "./DirTree.svelte";
  import HexView from "./HexView.svelte";
  import Inspector from "./Inspector.svelte";
  import Ribbon from "./Ribbon.svelte";
  import StepPanel from "./StepPanel.svelte";
  import StringsPanel from "./StringsPanel.svelte";
  import Timeline from "./Timeline.svelte";

  /** One tab's body: everything under the top bar, over the tab's own stores. App keeps one
   *  per opened tab mounted and hides the others, so each keeps its component state. */
  let { ws }: { ws: Workspace } = $props();
  // A workspace is bound to its view for the view's life (App keys the list on `ws.id`), so
  // context is set once, at init.
  // svelte-ignore state_referenced_locally
  setWorkspace(ws);

  /** The map panel of the tab's family (the FAT map, the block-group map), from the panel
   *  registry. The family's extra panels follow it in order. */
  const MapPanel = $derived(PANELS[ws.id].map);
</script>

<!-- The current step sits with the other controls, under the picker and the Terminal
     button, so the right column is left to state and data (Lesson, Strings, Inspector). -->
<div id="step-slot" class="step-slot"><StepPanel /></div>
<div id="ribbon-slot" class="ribbon-slot"><Ribbon /></div>
<div class="grid">
  <aside class="col left">
    <DirTree />
    <MapPanel />
    {#each PANELS[ws.id].extras as Extra}<Extra />{/each}
    <ActionsPanel />
  </aside>
  <main class="col center"><HexView /></main>
  <aside class="col right">
    <StringsPanel />
    <Inspector />
  </aside>
</div>
<footer id="timeline-slot"><Timeline /></footer>
```

A tab mounts its own family only (Step 9), so `PANELS[ws.id]` is the mounted family's panels. The two `svelte-ignore state_referenced_locally` comments are needed: without them svelte-check warns `This reference only captures the initial value of \`ws\``, which is the intent.

- [ ] **Step 14: Restructure `App.svelte`**

Replace `web/ui/src/App.svelte` with:

```svelte
<script lang="ts">
  import LessonPanel from "./components/LessonPanel.svelte";
  import ScenarioPanel from "./components/ScenarioPanel.svelte";
  import StatusLine from "./components/StatusLine.svelte";
  import TerminalPanel from "./components/TerminalPanel.svelte";
  import WorkspaceScope from "./components/WorkspaceScope.svelte";
  import WorkspaceView from "./components/WorkspaceView.svelte";
  import { inTextEntry } from "./core/keys";
  import { terminal } from "./state/terminal.svelte";
  import { theme } from "./state/theme.svelte";
  import { workspaces } from "./state/workspace.svelte";

  // `[` / `]` scrub the active tab's timeline and `/` jumps to its path field, all from
  // anywhere except text entry, so typing a path or file content in the Actions panel isn't
  // hijacked. `n` / `p` (scenario step) are handled by LessonPanel itself, which only
  // exists while a lesson is running. The terminal drawer counts as text
  // entry too (see core/keys.ts for the Ctrl-B chord case).
  function onKeydown(e: KeyboardEvent) {
    // A modifier means the chord belongs to the browser or the OS (Cmd-` cycles windows on
    // macOS, Ctrl-[ is Escape in some setups), so none of these are ours to swallow.
    if (e.metaKey || e.ctrlKey || e.altKey || inTextEntry(e)) return;
    const ws = workspaces.active;
    if (e.key === "`") {
      // Toggle the terminal from anywhere outside text entry. Inside the terminal the
      // key is typed (xterm cancels it), so closing is exit / Close / Escape on the bar.
      e.preventDefault();
      terminal.toggle();
    } else if (e.key === "[") ws.goToStep(Math.max(0, ws.volume.cursor - 1));
    else if (e.key === "]") ws.goToStep(ws.volume.cursor + 1);
    else if (e.key === "/") {
      e.preventDefault();
      document.getElementById(`action-path-${workspaces.activeId}`)?.focus();
    }
  }
</script>
<svelte:window onkeydown={onKeydown} />
<div class="app" class:term-side={terminal.placement === "side"} style:--term-h="{terminal.height}px" style:--term-w="{terminal.width}px">
  <header class="topbar">
    <h1>fs explorer</h1>
    <button
      id="terminal-toggle"
      type="button"
      aria-pressed={terminal.open}
      aria-controls="terminal-drawer"
      title="Toggle the terminal (`)"
      onclick={() => terminal.toggle()}
    >Terminal</button>
    <!-- The picker and the status line belong to the active tab: rebuilt over its workspace on
         a switch. -->
    {#key workspaces.active}
      <WorkspaceScope ws={workspaces.active}>
        <div id="scenario-slot"><ScenarioPanel /></div>
        <StatusLine />
      </WorkspaceScope>
    {/key}
    <button
      id="theme-toggle"
      type="button"
      role="switch"
      class="switch"
      aria-checked={theme.isDark}
      title="Switch between dark and light"
      onclick={() => theme.toggle()}
    >
      <span>Dark mode</span>
      <span class="switch-track" aria-hidden="true"><span class="switch-knob"></span></span>
    </button>
  </header>
  <!-- One body per opened tab, kept mounted so each keeps its panels' state; only the active
       one is shown. -->
  {#each workspaces.opened as ws (ws.id)}
    <div id="workspace-{ws.id}" class="workspace" hidden={!ws.active}><WorkspaceView {ws} /></div>
  {/each}
  <TerminalPanel />
  <!-- Floating (position: fixed), so its place in the DOM is only reading order: after
       everything it talks about. One card for the page, over the active tab's lesson. -->
  {#key workspaces.active}
    <WorkspaceScope ws={workspaces.active}>
      {#if workspaces.active.scenarios.current}<LessonPanel />{/if}
    </WorkspaceScope>
  {/key}
</div>
```

`TerminalPanel` and the lesson card exist once per page; the `.workspace` wrapper is `display: contents` (Step 7), so the step strip, ribbon, grid, and footer keep their rows of `.app`.

- [ ] **Step 15: Swap the store imports for `getWorkspace()` in the import-only components**

In each file below, replace the first listed store import with `import { getWorkspace } from "<prefix>state/workspace.svelte";` (same place, `<prefix>` as in the old line), delete the other listed store imports, and add after the script's last `import` line a blank line and the destructure. Bodies are unchanged: the store fields are runes, so reading `volume.epoch` off the destructured object stays reactive.

| File | Old lines (removed) | New destructure (after the last import) |
|---|---|---|
| `src/components/DirTree.svelte` | `import { selection } from "../state/selection.svelte";` `import { volume } from "../state/volume.svelte";` | `const { volume, selection } = getWorkspace();` |
| `src/components/HexView.svelte` | `import { volume } from "../state/volume.svelte";` `import { selection } from "../state/selection.svelte";` `import { layers } from "../state/layers.svelte";` | `const { volume, selection, layers } = getWorkspace();` |
| `src/components/Inspector.svelte` | `import { volume } from "../state/volume.svelte";` `import { selection } from "../state/selection.svelte";` | `const { volume, selection } = getWorkspace();` |
| `src/components/Ribbon.svelte` | `import { layers } from "../state/layers.svelte";` `import { selection } from "../state/selection.svelte";` `import { volume } from "../state/volume.svelte";` | `const { volume, selection, layers } = getWorkspace();` |
| `src/components/StatusLine.svelte` | `import { volume } from "../state/volume.svelte";` | `const { volume } = getWorkspace();` |
| `src/components/StringsPanel.svelte` | `import { volume } from "../state/volume.svelte";` `import { selection } from "../state/selection.svelte";` `import { layers } from "../state/layers.svelte";` | `const { volume, selection, layers } = getWorkspace();` |
| `src/components/TreeNodeView.svelte` | `import { selection } from "../state/selection.svelte";` `import { volume } from "../state/volume.svelte";` | `const { volume, selection } = getWorkspace();` |
| `src/fs/fat16/FatMap.svelte` | `import { layers } from "../../state/layers.svelte";` `import { selection } from "../../state/selection.svelte";` `import { volume } from "../../state/volume.svelte";` | `const { volume, selection, layers } = getWorkspace();` |
| `src/fs/fat16/FormatForm.svelte` | `import { selection } from "../../state/selection.svelte";` `import { volume } from "../../state/volume.svelte";` | `const { volume, selection } = getWorkspace();` |
| `src/fs/ext/BlockGroupMap.svelte` | `import { layers } from "../../state/layers.svelte";` `import { selection } from "../../state/selection.svelte";` `import { volume } from "../../state/volume.svelte";` | `const { volume, selection, layers } = getWorkspace();` |
| `src/fs/ext/ExtFormatForm.svelte` | `import { selection } from "../../state/selection.svelte";` `import { volume } from "../../state/volume.svelte";` | `const { volume, selection } = getWorkspace();` |
| `src/fs/ext/JournalPanel.svelte` | `import { selection } from "../../state/selection.svelte";` `import { volume } from "../../state/volume.svelte";` | `const { volume, selection } = getWorkspace();` |

The new head of `web/ui/src/fs/fat16/FatMap.svelte`, as a representative:

```svelte
<script lang="ts">
  import { attrAtOffset } from "../../core/attribution";
  import { cellAt, cellRect, dashCell, dotCell, drawChain, gridCols, gridRows, outlineCell, prepareCanvas } from "../../core/grid";
  import { observeWidth } from "../../core/observeWidth";
  import { getWorkspace } from "../../state/workspace.svelte";
  import { clusterState } from "./fatchain";
  import { asFat16 } from "./index";

  const { volume, selection, layers } = getWorkspace();

  const CELL = 6, GAP = 1, MAX_HEIGHT = 260;
```

`src/components/LessonPanel.svelte` swaps its imports the same way (the removed lines are `import { scenarios } from "../state/scenarios.svelte";` and `import { volume } from "../state/volume.svelte";`, after `import { lessonWindow } from "../state/lessonWindow.svelte";`, which stays):

```svelte
  import { lessonWindow } from "../state/lessonWindow.svelte";
  import { getWorkspace } from "../state/workspace.svelte";

  const { volume, scenarios } = getWorkspace();
```

and its default spot measures the shown tab's grid; replace:

```ts
  // The default spot is measured from the layout (the grid's top edge), after the card has
  // its size; re-measured whenever the viewport changes and the user has not moved it yet.
```

with:

```ts
  // The default spot is measured from the layout (the shown tab's grid's top edge; a hidden
  // tab's grid has no box), after the card has its size; re-measured whenever the viewport
  // changes and the user has not moved it yet.
```

and:

```ts
      const gridTop = document.querySelector(".grid")?.getBoundingClientRect().top ?? 96;
```

with:

```ts
      const gridTop = document.querySelector(".workspace:not([hidden]) .grid")?.getBoundingClientRect().top ?? 96;
```

- [ ] **Step 16a: `ActionsPanel.svelte` takes the workspace itself, for the per-tab path id**

In `web/ui/src/components/ActionsPanel.svelte`, replace the head

```svelte
<script lang="ts">
  import { volume } from "../state/volume.svelte";
  import { selection } from "../state/selection.svelte";
  import { FAMILIES } from "../fs";
  import type { FsFamilyId } from "../fs/adapter";
  import { PANELS } from "../fs/panels";
```

with:

```svelte
<script lang="ts">
  import { FAMILIES } from "../fs";
  import type { FsFamilyId } from "../fs/adapter";
  import { PANELS } from "../fs/panels";
  import { getWorkspace } from "../state/workspace.svelte";

  const ws = getWorkspace();
  const { volume, selection } = ws;
```

and the path field's id:

```svelte
      <input id="action-path" class="mono" type="text" bind:value={path} />
```

with:

```svelte
      <input id="action-path-{ws.id}" class="mono" type="text" bind:value={path} />
```

- [ ] **Step 16b: `StepPanel.svelte`'s heading id is per tab**

In `web/ui/src/components/StepPanel.svelte`, replace

```svelte
  import { volume } from "../state/volume.svelte";
  import { selection } from "../state/selection.svelte";
  import { changedSectors } from "../core/patch";
```

with:

```svelte
  import { changedSectors } from "../core/patch";
  import { getWorkspace } from "../state/workspace.svelte";

  const ws = getWorkspace();
  const { volume, selection } = ws;
```

and

```svelte
<section class="panel step" aria-labelledby="step-heading">
  <h2 id="step-heading">Step</h2>
```

with:

```svelte
<section class="panel step" aria-labelledby="step-heading-{ws.id}">
  <h2 id="step-heading-{ws.id}">Step</h2>
```

- [ ] **Step 16c: `Timeline.svelte` steps through `goToStep`**

In `web/ui/src/components/Timeline.svelte`, replace

```svelte
  import { focusHistoryStep } from "../state/navigate.svelte";
  import { volume } from "../state/volume.svelte";
```

with:

```svelte
  import { getWorkspace } from "../state/workspace.svelte";

  const ws = getWorkspace();
  const { volume } = ws;
```

and in `goTo`:

```ts
  function goTo(step: number) {
    volume.seek(step);
    focusHistoryStep(step);
  }
```

with:

```ts
  function goTo(step: number) {
    ws.goToStep(step);
  }
```

- [ ] **Step 16d: `ScenarioPanel.svelte` reports the lesson refusal**

`web/ui/src/components/ScenarioPanel.svelte` still lists both families' lessons (Task 5 narrows it); a lesson of the other family is refused onto the status line instead of throwing out of the click handler. Replace

```svelte
  import { scenarios } from "../state/scenarios.svelte";
```

with:

```svelte
  import { getWorkspace } from "../state/workspace.svelte";

  const { volume, scenarios } = getWorkspace();
```

and in `startScenario`:

```ts
    const s = all.find((s) => s.id === selectedId);
    if (s) scenarios.start(s);
```

with:

```ts
    const s = all.find((s) => s.id === selectedId);
    if (!s) return;
    // A lesson runs in its own family's tab; another family's lesson is refused onto the
    // status line until the picker lists the tab's lessons only.
    try {
      scenarios.start(s);
    } catch (e) {
      volume.report(e);
    }
```

- [ ] **Step 17a: `createStoreHost(ws, …)` reads one workspace's stores**

In `web/ui/src/shell/storeHost.svelte.ts`, replace

```ts
import { selection } from "../state/selection.svelte";
import { volume } from "../state/volume.svelte";
import { statusToError, type ShellHost } from "./host";

/**
 * The app's ShellHost. Commands close over the returned object and read live store
 * fields through its getters on every call, so a host created once when the
 * terminal starts never goes stale.
```

with:

```ts
import type { Workspace } from "../state/workspace.svelte";
import { statusToError, type ShellHost } from "./host";

/**
 * The app's ShellHost over one tab's workspace. Commands close over the returned object and
 * read live store fields through its getters on every call, so a host created once for a
 * workspace never goes stale.
```

and

```ts
export function createStoreHost(closeTerminal: () => void, applyPrompt: (prefix: string) => void): ShellHost {
  return {
```

with:

```ts
export function createStoreHost(ws: Workspace, closeTerminal: () => void, applyPrompt: (prefix: string) => void): ShellHost {
  const { volume, selection } = ws;
  return {
```

(the object's body is unchanged: it reads `volume` and `selection`, now the workspace's).

- [ ] **Step 17b: `TerminalPanel.svelte` hosts the active workspace**

`web/ui/src/components/TerminalPanel.svelte` keeps its module `vfs` and both effects for now (Task 3 gives each workspace its own host and cwd). Replace

```svelte
  import { volume } from "../state/volume.svelte";
```

with:

```svelte
  import { workspaces } from "../state/workspace.svelte";
```

in `ensureCreated` replace

```ts
        // family, through the re-registration effect below.
        const created = createStoreHost(close, (prefix) => term.setPrompt(prefix));
        registered = registerCommands(term, created, vfs);
        registeredSet = commandSetOf(volume.adapter);
```

with:

```ts
        // family, through the re-registration effect below. The host is the active tab's
        // workspace's (the page opens one workspace for now).
        const created = createStoreHost(workspaces.active, close, (prefix) => term.setPrompt(prefix));
        registered = registerCommands(term, created, vfs);
        registeredSet = commandSetOf(workspaces.active.volume.adapter);
```

in the cwd-reset effect replace

```ts
  // prompt. `volume.vol` is the only tracked read: `seenVol` and `vfs.cwd` are plain fields,
  // and the prompt call is untracked so this effect can never depend on what it writes.
  let seenVol: Volume | null = null;
  $effect(() => {
    const vol = volume.vol;
```

with:

```ts
  // prompt. The active workspace's `volume.vol` is the only tracked read: `seenVol` and
  // `vfs.cwd` are plain fields, and the prompt call is untracked so this effect can never
  // depend on what it writes.
  let seenVol: Volume | null = null;
  $effect(() => {
    const vol = workspaces.active.volume.vol;
```

and in the re-registration effect replace

```ts
  // the same `vfs` (the working directory survives), and re-set the prompt. `volume.adapter`
  // is the only tracked read; the terminal and the names are plain fields, and the work is
  // untracked. Before the terminal exists there is nothing to swap: creation registers the
  // set of the family mounted then.
  $effect(() => {
    const set = commandSetOf(volume.adapter);
```

with:

```ts
  // the same `vfs` (the working directory survives), and re-set the prompt. The active
  // workspace's `volume.adapter` is the only tracked read; the terminal and the names are
  // plain fields, and the work is untracked. Before the terminal exists there is nothing to
  // swap: creation registers the set of the family mounted then.
  $effect(() => {
    const set = commandSetOf(workspaces.active.volume.adapter);
```

- [ ] **Step 18: Gates**

Run: `grep -rn 'state/\(volume\|selection\|layers\|scenarios\|navigate\)\.svelte"' src | grep -v "import type"`
Expected: no output (only type imports of the store classes remain).

Run: `pnpm test`
Expected: `Test Files  51 passed (51)`, `Tests  506 passed (506)`.

Run: `pnpm build`
Expected: `svelte-check` reports `COMPLETED 458 FILES 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS`, then `✓ built in …`.

Run (from the repo root): `git status --short`
Expected: only the files of this task; `pnpm-lock.yaml` and `pnpm-workspace.yaml` untouched; nothing under `crates/` or `web/demo/`.

- [ ] **Step 19: Browser check (dev server `ui`, 1440 × 900)**

Nothing may look different from before this task. Check: the page has one `div#workspace-fat16.workspace` (computed `display: contents`) and `.app`'s five rows; Add file puts `1 of 1` and `create_file /Hello world.txt` in the Step strip (the wasm package's op name) and the file in Files, with its chain highlighted on the FAT map (Steps 8 and 10: the selection and the layers over the tab's volume); after a second Add file, `[` shows `Viewing step 1 of 2` and `]` returns (Step 12: `goToStep`); the dump scrolls; `/` focuses `#action-path-fat16`; Start on `The fundamentals` opens one lesson card and `n` steps it (`Lesson · step 3 of 12` after two presses; Step 11: the runner over the tab's stores); the terminal runs `ls /mnt` (a table with the lesson's file) and `mkfs` (`formatted /dev/hda as FAT16; the timeline was cleared`); Export image produces a 16 MiB `volume.img` and loading it back through Load image mounts it with an empty timeline, nothing selected, and an empty status line (Step 9: `mount`); the theme switch goes light and back. Transitional until later tasks, and the check of Step 9's and Step 11's refusals: `mkfs --type ext3` prints `/dev/hda: this tab formats fat16, not ext`; in the Actions panel's Format details, picking `ext` in the Filesystem select and pressing Format puts `this tab formats fat16, not ext` on the status line (Task 5 removes the select); and starting an ext lesson from the picker shows `lesson ext-fundamentals is for ext; this tab is fat16` in the status line. No console errors other than the terminal library's own log of a refused command.

- [ ] **Step 20: Commit**

```sh
git add -A web/ui
git commit -m "refactor(ui): one workspace per filesystem family, reached through context" -m "The stores become per-family: a Workspace owns its selection, volume, layers, scenario runner, and shell cwd, built with its siblings injected, and components read it through getWorkspace() instead of module singletons. VolumeStore formats and mounts its own family only. App renders one WorkspaceView per opened workspace and scopes the picker, status line, and lesson card to the active one. Only the FAT16 workspace is opened, so the page looks and behaves as before."
```


### Task 2: The tabs, the lazy ext workspace, the URL, per-tab scroll

Spec: `docs/superpowers/specs/2026-09-25-family-tabs-design.md` section 3 (`TabBar.svelte`, App's tab bar, tabpanels, `document.title`, the hash, the `.tabbar`/`.tab`/`.tab-badge`/`.sr-only` rules and the 1100 px / 760 px top bar), section 5 (what a switch changes and keeps; focus stays on the activated tab), and section 2's pure modules `core/keepScroll.ts` and `core/observeWidth.ts`. Task 1 left the `WorkspaceRegistry` (lazy `activate`), the `.workspace:not([hidden]) { display: contents; }` rule and its guard, and the per-tab ids `action-path-{id}`/`step-heading-{id}`; this task does not add them again. The Journal panel stays where Task 1 left it (`PANELS[id].extras`, under the map); Task 5 moves it. The terminal is still Task 1's transitional one (Task 3).

**Files**

- Create: `web/ui/src/components/TabBar.svelte` (1-59)
- Create: `web/ui/src/core/keepScroll.ts` (1-53)
- Create (test): `web/ui/tests/keepScroll.test.ts` (1-133)
- Create (test): `web/ui/tests/observeWidth.test.ts` (1-48)
- Modify: `web/ui/src/core/observeWidth.ts` (whole file, 1-23)
- Modify: `web/ui/src/App.svelte` (5, 10-11, 16-33, 57, 61, 92-94)
- Modify: `web/ui/src/app.css` (37-43, 61-79)
- Modify: `web/ui/src/components/WorkspaceView.svelte` (27-30, 44)
- Modify: `web/ui/src/components/HexView.svelte` (5, 12-13, 133-134)
- Modify: `web/ui/src/fs/ext/BlockGroupMap.svelte` (3, 9-10, 180-181)
- Modify: `web/ui/src/components/Timeline.svelte` (51-55)
- Modify: `web/ui/src/components/LessonPanel.svelte` (36-39, 42-45)
- Modify (test): `web/ui/tests/layout.test.ts` (13-17, 32-62)

**Interfaces**

Consumes (Task 1): `workspaces: WorkspaceRegistry` (`activeId`, `opened`, `active`, `activate(id)`), `Workspace.active`, `Workspace.scenarios.current`, `getWorkspace()` (`src/state/workspace.svelte.ts`); `parseRoute`, `formatRoute` (`src/core/route.ts`); `FAMILIES`, `FAMILY_IDS`, `ROUTE_ALIASES` (`src/fs/index.ts`); `FsFamilyId` (`src/fs/adapter.ts`).

Produces:

```ts
// src/core/keepScroll.ts
export interface Scrollable { scrollTop: number; addEventListener(type: "scroll", fn: () => void): void; removeEventListener(type: "scroll", fn: () => void): void }
export function createScrollKeeper(el: Scrollable, shown: boolean, raf: (fn: () => void) => void = (fn) => requestAnimationFrame(fn)): { update(shown: boolean): void; destroy(): void };
export const keepScroll: Action<HTMLElement, boolean>;
// src/core/observeWidth.ts (signature unchanged): a 0 width is never reported
export function observeWidth(el: HTMLElement, onWidth: (width: number) => void): ActionReturn<(width: number) => void>;
// src/components/TabBar.svelte: no props. <div class="tabbar" role="tablist" aria-label="Filesystem">, one
//   <button class="tab" role="tab" id="tab-{id}" aria-selected aria-controls="workspace-{id}" (only once that workspace exists)
//   tabindex={active ? 0 : -1}> per FAMILY_IDS, labelled FAMILIES[id].name, with
//   <span class="tab-badge">lesson<span class="sr-only"> running</span></span> while its workspace's scenarios.current is set
// App.svelte: <TabBar /> after the h1; each body <div id="workspace-{id}" class="workspace" role="tabpanel" aria-labelledby="tab-{id}" hidden>;
//   document.title `fs explorer · ${FAMILIES[activeId].name}`; the hash `#${activeId}` (history.replaceState); onhashchange activates the named tab
// WorkspaceView.svelte: the slots are classes only (`step-slot`, `ribbon-slot`, `timeline-slot`); the ids `step-slot`, `ribbon-slot`, `timeline-slot` are gone
```

Before the first test run in a fresh worktree: from the repo root `wasm-pack build crates/wasm --target bundler`, then in `web/ui` `CI=true pnpm install --frozen-lockfile`. Baseline in `web/ui` on Task 1's commit: `pnpm test` gives `Test Files  51 passed (51)`, `Tests  506 passed (506)`. All commands below run in `web/ui`.

- [ ] **Step 1: Write the failing `keepScroll` tests**

Create `web/ui/tests/keepScroll.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { createScrollKeeper } from "../src/core/keepScroll";

/** A scroll box: `scrollTop` plus the one event the keeper listens to. `scroll(to)` is a user
 *  scroll (moves and fires); assigning `scrollTop` directly is what `display: none` does. */
function box(top = 0) {
  const listeners = new Set<() => void>();
  return {
    scrollTop: top,
    listeners,
    addEventListener(_type: "scroll", fn: () => void) { listeners.add(fn); },
    removeEventListener(_type: "scroll", fn: () => void) { listeners.delete(fn); },
    scroll(to: number) {
      this.scrollTop = to;
      for (const fn of listeners) fn();
    },
  };
}

/** An injected `requestAnimationFrame`: callbacks wait until `flush()`. */
function frames() {
  const queue: (() => void)[] = [];
  return {
    raf: (fn: () => void) => { queue.push(fn); },
    get pending() { return queue.length; },
    flush() { while (queue.length) queue.shift()!(); },
  };
}

describe("createScrollKeeper", () => {
  it("restores the position a hide zeroed, in the frame after the box is shown again", () => {
    const el = box();
    const f = frames();
    const keeper = createScrollKeeper(el, true, f.raf);
    el.scroll(340);
    keeper.update(false);
    el.scrollTop = 0; // display: none zeroes a scroll box without a scroll event
    keeper.update(true);
    expect(el.scrollTop).toBe(0); // not before layout: the box has no height yet
    f.flush();
    expect(el.scrollTop).toBe(340);
  });

  it("starts from the box's position when it is created", () => {
    const el = box(120);
    const f = frames();
    const keeper = createScrollKeeper(el, true, f.raf);
    keeper.update(false);
    el.scrollTop = 0;
    keeper.update(true);
    f.flush();
    expect(el.scrollTop).toBe(120);
  });

  it("ignores scroll events while hidden and before the restore runs", () => {
    const el = box();
    const f = frames();
    const keeper = createScrollKeeper(el, true, f.raf);
    el.scroll(340);
    keeper.update(false);
    el.scroll(0);
    keeper.update(true);
    el.scroll(0); // a scroll event at the zeroed position, before the frame
    f.flush();
    expect(el.scrollTop).toBe(340);
  });

  it("keeps recording once restored", () => {
    const el = box();
    const f = frames();
    const keeper = createScrollKeeper(el, true, f.raf);
    el.scroll(340);
    keeper.update(false);
    keeper.update(true);
    f.flush();
    el.scroll(500);
    keeper.update(false);
    el.scrollTop = 0;
    keeper.update(true);
    f.flush();
    expect(el.scrollTop).toBe(500);
  });

  it("schedules nothing unless the box goes from hidden to shown", () => {
    const el = box();
    const f = frames();
    const keeper = createScrollKeeper(el, true, f.raf);
    keeper.update(true);
    expect(f.pending).toBe(0);
    keeper.update(false);
    keeper.update(false);
    expect(f.pending).toBe(0);
    keeper.update(true);
    expect(f.pending).toBe(1);
  });

  it("skips the restore when the box is hidden again before the frame", () => {
    const el = box();
    const f = frames();
    const keeper = createScrollKeeper(el, true, f.raf);
    el.scroll(340);
    keeper.update(false);
    el.scrollTop = 0;
    keeper.update(true);
    keeper.update(false);
    f.flush();
    expect(el.scrollTop).toBe(0);
  });

  it("restores a box created hidden to where it was when created", () => {
    const el = box(0);
    const f = frames();
    const keeper = createScrollKeeper(el, false, f.raf);
    el.scroll(90); // ignored: hidden
    keeper.update(true);
    f.flush();
    expect(el.scrollTop).toBe(0);
  });

  it("stops listening and drops a pending restore on destroy", () => {
    const el = box();
    const f = frames();
    const keeper = createScrollKeeper(el, true, f.raf);
    el.scroll(340);
    keeper.update(false);
    el.scrollTop = 0;
    keeper.update(true);
    keeper.destroy();
    expect(el.listeners.size).toBe(0);
    f.flush();
    expect(el.scrollTop).toBe(0);
  });
});
```

Run: `pnpm exec vitest run tests/keepScroll.test.ts`
Expected: FAIL, `Error: Cannot find module '../src/core/keepScroll'`, `Test Files  1 failed (1)`, `Tests  no tests`.

- [ ] **Step 2: Implement `core/keepScroll.ts`**

Create `web/ui/src/core/keepScroll.ts`:

```ts
import type { Action } from "svelte/action";

/** What `createScrollKeeper` needs of a scroll box; an `HTMLElement` is one. */
export interface Scrollable {
  scrollTop: number;
  addEventListener(type: "scroll", fn: () => void): void;
  removeEventListener(type: "scroll", fn: () => void): void;
}

/**
 * Keeps a scroll box's position across a hide. A tab's body is hidden with `display: none`,
 * which zeroes every scroll box in it without a scroll event, so the position is recorded on
 * scroll only while the box is shown and put back when it is shown again. The restore waits a
 * frame (`raf`): straight after `hidden` is removed the box has no layout yet and would clamp
 * the position to 0. Scroll events between showing and the restore are ignored for the same
 * reason; a hide before the frame skips the restore.
 */
export function createScrollKeeper(
  el: Scrollable,
  shown: boolean,
  raf: (fn: () => void) => void = (fn) => requestAnimationFrame(fn),
): { update(shown: boolean): void; destroy(): void } {
  let saved = el.scrollTop;
  let visible = shown;
  let restoring = false;
  let destroyed = false;

  const onScroll = () => {
    if (visible && !restoring) saved = el.scrollTop;
  };
  el.addEventListener("scroll", onScroll);

  return {
    update(next: boolean) {
      const wasVisible = visible;
      visible = next;
      if (!next || wasVisible) return;
      restoring = true;
      raf(() => {
        if (destroyed || !visible || !restoring) return;
        restoring = false;
        el.scrollTop = saved;
      });
    },
    destroy() {
      destroyed = true;
      el.removeEventListener("scroll", onScroll);
    },
  };
}

/** `<div use:keepScroll={ws.active}>`: `createScrollKeeper` as a Svelte action. */
export const keepScroll: Action<HTMLElement, boolean> = (el, shown) => createScrollKeeper(el, shown);
```

Run: `pnpm exec vitest run tests/keepScroll.test.ts`
Expected: `Test Files  1 passed (1)`, `Tests  8 passed (8)`.

- [ ] **Step 3: Write the failing `observeWidth` tests**

Create `web/ui/tests/observeWidth.test.ts`:

```ts
import { afterEach, describe, expect, it, vi } from "vitest";
import { observeWidth } from "../src/core/observeWidth";

/** The observer the action creates, so a test can report a resize to it. */
let observer: { fire(width: number): void; disconnected: boolean } | null = null;

class FakeResizeObserver {
  disconnected = false;
  private readonly cb: (entries: { contentRect: { width: number } }[]) => void;
  constructor(cb: (entries: { contentRect: { width: number } }[]) => void) {
    this.cb = cb;
    observer = this;
  }
  fire(width: number) { this.cb([{ contentRect: { width } }]); }
  observe() {}
  disconnect() { this.disconnected = true; }
}

function element(width: number): HTMLElement {
  return { getBoundingClientRect: () => ({ width }) } as unknown as HTMLElement;
}

describe("observeWidth", () => {
  afterEach(() => {
    vi.unstubAllGlobals();
    observer = null;
  });

  it("reports the width now and the content width on every resize", () => {
    vi.stubGlobal("ResizeObserver", FakeResizeObserver);
    const seen: number[] = [];
    const action = observeWidth(element(220), (w) => seen.push(w));
    observer!.fire(180);
    expect(seen).toEqual([220, 180]);
    action.destroy?.();
    expect(observer!.disconnected).toBe(true);
  });

  it("ignores a 0 width, so a map in a hidden tab keeps its last width", () => {
    vi.stubGlobal("ResizeObserver", FakeResizeObserver);
    const seen: number[] = [];
    observeWidth(element(0), (w) => seen.push(w)); // mounted inside a hidden tab
    observer!.fire(220);
    observer!.fire(0); // its tab was hidden: display: none
    observer!.fire(220);
    expect(seen).toEqual([220, 220]);
  });
});
```

Run: `pnpm exec vitest run tests/observeWidth.test.ts`
Expected: `Tests  1 failed | 1 passed (2)`; the failure is `ignores a 0 width, so a map in a hidden tab keeps its last width` (`AssertionError: expected [ +0, 220, +0, 220 ] to deeply equal [ 220, 220 ]`).

- [ ] **Step 4: `observeWidth` ignores a 0 width**

Replace `web/ui/src/core/observeWidth.ts` with:

```ts
import type { ActionReturn } from "svelte/action";

/**
 * A Svelte action that reports an element's width now and its content width on every resize,
 * so a canvas map wraps to the column it sits in (a resizable layout, a narrower viewport)
 * rather than to a width read once at mount: `<div use:observeWidth={(w) => (width = w)}>`.
 * A 0 width is never reported: it is what an element in a hidden tab (`display: none`)
 * measures, and the map keeps its last width instead of repainting 1 px wide. The observer
 * goes when the element does.
 */
export function observeWidth(el: HTMLElement, onWidth: (width: number) => void): ActionReturn<(width: number) => void> {
  let report = onWidth;
  const measured = (width: number) => {
    if (width > 0) report(width);
  };
  measured(el.getBoundingClientRect().width);
  const ro = new ResizeObserver((entries) => measured(entries[0].contentRect.width));
  ro.observe(el);
  return {
    update(next) { report = next; },
    destroy() { ro.disconnect(); },
  };
}
```

Run: `pnpm exec vitest run tests/observeWidth.test.ts`
Expected: `Tests  2 passed (2)`.

- [ ] **Step 5: Write the failing layout guards for the tab rules and the two breakpoints**

In `web/ui/tests/layout.test.ts`, after the `rule` helper, replace

```ts
    return m ? m[1] : null;
  };
```

with:

```ts
    return m ? m[1] : null;
  };
  /** The body of `@media (query) { … }`, which runs to the first line that is a lone `}`. */
  const media = (query: string) => {
    const start = css.indexOf(`@media (${query}) {\n`);
    return start < 0 ? null : css.slice(start, css.indexOf("\n}\n", start));
  };
```

and insert before `  it("gives the side terminal column an explicit width without redefining the main column", () => {`:

```ts
  it("draws the family tabs as one segmented control in the top bar, the selected tab underlined", () => {
    // The spec's rules verbatim: `.tab` drops the global button chrome, the selected tab reads as
    // a raised panel with a focus-coloured underline, and the badge's words are all caps.
    for (const line of [
      ".tabbar { display: inline-flex; gap: 2px; padding: 2px; border: 1px solid var(--hairline); border-radius: var(--radius); background: var(--canvas); }",
      ".tab { display: inline-flex; align-items: center; gap: 6px; border: 0; background: none; padding: 3px 14px; border-radius: calc(var(--radius) - 1px); font: 500 13px var(--font-display); color: var(--ink-muted); }",
      ".tab:hover { color: var(--ink); }",
      '.tab[aria-selected="true"] { background: var(--panel); color: var(--ink); box-shadow: inset 0 -2px 0 var(--focus); }',
      ".tab-badge { padding: 0 4px; border: 1px solid var(--focus); border-radius: var(--radius); font: 500 10px var(--font-display); text-transform: uppercase; letter-spacing: .04em; color: var(--focus); }",
    ]) expect(css).toContain(line);
    // The badge's " running" is for screen readers only; without the clip it would widen the tab.
    expect(rule(".sr-only")).toBe(" position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; ");
  });

  it("wraps the top bar under 1100 px, with the status line on a row of its own", () => {
    // With the tabs added, the h1, tabs, Terminal, picker, status line, and theme switch do not
    // fit one row at 1100 px; the status line's messages are the longest item, so it wraps last.
    const narrow = media("max-width: 1100px");
    expect(narrow).toContain(".topbar { flex-wrap: wrap; row-gap: 6px; }");
    expect(narrow).toContain(".topbar > .status { order: 1; flex-basis: 100%; }");
  });

  it("gives the tabs a full row of equal 36 px tabs under 760 px, then Terminal and the picker, then the status line", () => {
    const phone = media("max-width: 760px");
    expect(phone).toContain(".topbar > .switch { order: 1; }");
    expect(phone).toContain(".topbar > .tabbar { order: 2; display: flex; flex: 1 0 100%; }");
    expect(phone).toContain(".tab { flex: 1 1 0; justify-content: center; min-height: 36px; }");
    expect(phone).toContain("#terminal-toggle, #scenario-slot { order: 3; }");
    expect(phone).toContain(".topbar > .status { order: 4; }");
  });

```

Run: `pnpm exec vitest run tests/layout.test.ts`
Expected: `Tests  3 failed | 10 passed (13)` (the three new guards).

- [ ] **Step 6: Add the tab rules and the top bar's breakpoints to `app.css`**

In `web/ui/src/app.css`, replace

```css
.topbar h1 { font: 500 18px var(--font-display); margin: 0; }
```

with:

```css
.topbar h1 { font: 500 18px var(--font-display); margin: 0; }
/* The family tabs (TabBar.svelte): a segmented control right after the h1, no row of its own. */
.tabbar { display: inline-flex; gap: 2px; padding: 2px; border: 1px solid var(--hairline); border-radius: var(--radius); background: var(--canvas); }
.tab { display: inline-flex; align-items: center; gap: 6px; border: 0; background: none; padding: 3px 14px; border-radius: calc(var(--radius) - 1px); font: 500 13px var(--font-display); color: var(--ink-muted); }
.tab:hover { color: var(--ink); }
.tab[aria-selected="true"] { background: var(--panel); color: var(--ink); box-shadow: inset 0 -2px 0 var(--focus); }
.tab-badge { padding: 0 4px; border: 1px solid var(--focus); border-radius: var(--radius); font: 500 10px var(--font-display); text-transform: uppercase; letter-spacing: .04em; color: var(--focus); }
.sr-only { position: absolute; width: 1px; height: 1px; overflow: hidden; clip: rect(0 0 0 0); white-space: nowrap; }
```

and replace

```css
@media (max-width: 1100px) {
  .grid { grid-template-columns: 220px minmax(0, 1fr); }
  .right { grid-column: 1; }
}
@media (max-width: 760px) {
  .grid { grid-template-columns: minmax(0, 1fr); }
  .left, .right { max-height: 40vh; }
  .app { grid-auto-rows: min(var(--term-h, 220px), 40vh); }
}
```

with:

```css
/* The top bar wraps: under 1100 px the status line takes a row of its own under the rest; under
   760 px the rows are the h1 and theme switch, the tabs (equal widths, 36 px tall for touch),
   Terminal and the picker, then the status line. */
@media (max-width: 1100px) {
  .grid { grid-template-columns: 220px minmax(0, 1fr); }
  .right { grid-column: 1; }
  .topbar { flex-wrap: wrap; row-gap: 6px; }
  .topbar > .status { order: 1; flex-basis: 100%; }
}
@media (max-width: 760px) {
  .grid { grid-template-columns: minmax(0, 1fr); }
  .left, .right { max-height: 40vh; }
  .app { grid-auto-rows: min(var(--term-h, 220px), 40vh); }
  .topbar > .switch { order: 1; }
  .topbar > .tabbar { order: 2; display: flex; flex: 1 0 100%; }
  .tab { flex: 1 1 0; justify-content: center; min-height: 36px; }
  #terminal-toggle, #scenario-slot { order: 3; }
  .topbar > .status { order: 4; }
}
```

`.app`'s `grid-template-rows` stays as it is (Task 6 drops the footer row), and `.workspace:not([hidden]) { display: contents; }` is Task 1's, already there.

Run: `pnpm exec vitest run tests/layout.test.ts`
Expected: `Tests  13 passed (13)`.

- [ ] **Step 7: Add `TabBar.svelte`**

Create `web/ui/src/components/TabBar.svelte`:

```svelte
<script lang="ts">
  import { FAMILIES, FAMILY_IDS } from "../fs";
  import type { FsFamilyId } from "../fs/adapter";
  import { workspaces } from "../state/workspace.svelte";

  /** The top bar's family tabs, one per `FAMILY_IDS`, in the WAI-ARIA tabs pattern with
   *  automatic activation: the focused tab is the shown one. Only the active tab is in the Tab
   *  order (roving tabindex); Left/Right move between tabs and wrap, Home/End go to the ends. A
   *  tab whose workspace is running a lesson carries a `lesson` badge, the active tab too, so
   *  the tabs' widths do not jump on a switch. */
  const buttons: HTMLButtonElement[] = $state([]);

  /** Whether `id`'s workspace exists and has a lesson running. Reading `opened` does not
   *  create a workspace: that happens only in `workspaces.activate`. */
  function lessonRunning(id: FsFamilyId): boolean {
    return workspaces.opened.find((w) => w.id === id)?.scenarios.current != null;
  }

  function onKeydown(e: KeyboardEvent, i: number) {
    const n = FAMILY_IDS.length;
    let next: number;
    if (e.key === "ArrowLeft") next = (i - 1 + n) % n;
    else if (e.key === "ArrowRight") next = (i + 1) % n;
    else if (e.key === "Home") next = 0;
    else if (e.key === "End") next = n - 1;
    else return;
    e.preventDefault();
    workspaces.activate(FAMILY_IDS[next]);
    buttons[next]?.focus();
  }
</script>

<div class="tabbar" role="tablist" aria-label="Filesystem">
  {#each FAMILY_IDS as id, i (id)}
    {@const selected = workspaces.activeId === id}
    <!-- A tab whose workspace does not exist yet has no panel to point at until it is first
         activated. -->
    <button
      bind:this={buttons[i]}
      type="button"
      class="tab"
      role="tab"
      id="tab-{id}"
      aria-selected={selected}
      aria-controls={workspaces.opened.some((w) => w.id === id) ? `workspace-${id}` : undefined}
      tabindex={selected ? 0 : -1}
      onclick={() => {
        workspaces.activate(id);
        // Safari and Firefox on macOS do not focus a button on click; the tabs pattern keeps focus
        // on the activated tab, and the Lesson card leaves it there only if it is.
        buttons[i]?.focus();
      }}
      onkeydown={(e) => onKeydown(e, i)}
    >
      {FAMILIES[id].name}
      {#if lessonRunning(id)}<span class="tab-badge">lesson<span class="sr-only"> running</span></span>{/if}
    </button>
  {/each}
</div>
```

`aria-controls` names the panel only once that workspace exists: before the ext tab is first opened there is no `#workspace-ext` to point at. The click handler focuses the tab itself because Safari and Firefox on macOS do not focus a `<button>` on click; it does so before the keyed Lesson card remounts, so Step 12's check finds focus on the tab.

- [ ] **Step 8: `App.svelte`: the tab bar, the tabpanels, the title, and the hash**

In `web/ui/src/App.svelte`, replace

```svelte
  import StatusLine from "./components/StatusLine.svelte";
  import TerminalPanel from "./components/TerminalPanel.svelte";
  import WorkspaceScope from "./components/WorkspaceScope.svelte";
  import WorkspaceView from "./components/WorkspaceView.svelte";
  import { inTextEntry } from "./core/keys";
  import { terminal } from "./state/terminal.svelte";
  import { theme } from "./state/theme.svelte";
  import { workspaces } from "./state/workspace.svelte";
```

with:

```svelte
  import StatusLine from "./components/StatusLine.svelte";
  import TabBar from "./components/TabBar.svelte";
  import TerminalPanel from "./components/TerminalPanel.svelte";
  import WorkspaceScope from "./components/WorkspaceScope.svelte";
  import WorkspaceView from "./components/WorkspaceView.svelte";
  import { inTextEntry } from "./core/keys";
  import { formatRoute, parseRoute } from "./core/route";
  import { FAMILIES, FAMILY_IDS, ROUTE_ALIASES } from "./fs";
  import { terminal } from "./state/terminal.svelte";
  import { theme } from "./state/theme.svelte";
  import { workspaces } from "./state/workspace.svelte";

  // The tab is in the URL (`#fat16`, `#ext`), so a link or a reload opens it; the hash is
  // replaced, not pushed, so switching tabs adds no history entries. The registry read the hash
  // at load; a bare URL opened the default tab and gets its hash here.
  function syncHash() {
    const want = formatRoute(workspaces.activeId);
    if (location.hash !== want) history.replaceState(null, "", want);
  }
  $effect(syncHash);
  $effect(() => {
    document.title = `fs explorer · ${FAMILIES[workspaces.activeId].name}`;
  });
  // A hash typed or pasted into the address bar: open the tab it names. An unknown name keeps the
  // current tab, and its hash is put back.
  function onHashChange() {
    workspaces.activate(parseRoute(location.hash, FAMILY_IDS, ROUTE_ALIASES) ?? workspaces.activeId);
    syncHash();
  }
```

replace

```svelte
<svelte:window onkeydown={onKeydown} />
<div class="app" class:term-side={terminal.placement === "side"} style:--term-h="{terminal.height}px" style:--term-w="{terminal.width}px">
  <header class="topbar">
    <h1>fs explorer</h1>
    <button
```

with:

```svelte
<svelte:window onkeydown={onKeydown} onhashchange={onHashChange} />
<div class="app" class:term-side={terminal.placement === "side"} style:--term-h="{terminal.height}px" style:--term-w="{terminal.width}px">
  <header class="topbar">
    <h1>fs explorer</h1>
    <TabBar />
    <button
```

and replace

```svelte
  <!-- One body per opened tab, kept mounted so each keeps its panels' state; only the active
       one is shown. -->
  {#each workspaces.opened as ws (ws.id)}
    <div id="workspace-{ws.id}" class="workspace" hidden={!ws.active}><WorkspaceView {ws} /></div>
  {/each}
```

with:

```svelte
  <!-- One body per opened tab, kept mounted so each keeps its panels' state; only the active
       one is shown. Each is the panel of its TabBar tab. -->
  {#each workspaces.opened as ws (ws.id)}
    <div id="workspace-{ws.id}" class="workspace" role="tabpanel" aria-labelledby="tab-{ws.id}" hidden={!ws.active}><WorkspaceView {ws} /></div>
  {/each}
```

The ext workspace is created by `workspaces.activate("ext")` (a tab click, an arrow key, or a `#ext` hash), which pushes it to `opened`; the `{#each}` then mounts its `WorkspaceView`: a fresh ext3 disk with the Block groups map and the Journal panel under it.

- [ ] **Step 9: `WorkspaceView.svelte`: the slots are classes, one set per tab**

In `web/ui/src/components/WorkspaceView.svelte`, replace

```svelte
<!-- The current step sits with the other controls, under the picker and the Terminal
     button, so the right column is left to state and data (Lesson, Strings, Inspector). -->
<div id="step-slot" class="step-slot"><StepPanel /></div>
<div id="ribbon-slot" class="ribbon-slot"><Ribbon /></div>
```

with:

```svelte
<!-- The current step sits with the other controls, under the picker and the Terminal
     button, so the right column is left to state and data (Lesson, Strings, Inspector). The
     slots are classes, not ids: every opened tab has its own. -->
<div class="step-slot"><StepPanel /></div>
<div class="ribbon-slot"><Ribbon /></div>
```

and replace

```svelte
<footer id="timeline-slot"><Timeline /></footer>
```

with:

```svelte
<footer class="timeline-slot"><Timeline /></footer>
```

No CSS selector or test used the three ids (`grep -rnE "#(step|ribbon|timeline)-slot" src tests` prints nothing); `.ribbon-slot` was already the class the CSS uses.

- [ ] **Step 10: The dump and the block-group map keep their scroll across a hide**

In `web/ui/src/components/HexView.svelte`, replace

```svelte
  import { getWorkspace } from "../state/workspace.svelte";
  import { attrAtOffset } from "../core/attribution";
```

with:

```svelte
  import { getWorkspace } from "../state/workspace.svelte";
  import { attrAtOffset } from "../core/attribution";
  import { keepScroll } from "../core/keepScroll";
```

replace

```svelte
  const { volume, selection, layers } = getWorkspace();
```

with:

```svelte
  const ws = getWorkspace();
  const { volume, selection, layers } = ws;
```

and replace

```svelte
<!-- svelte-ignore a11y_mouse_events_have_key_events -->
<div class="hexview panel" bind:this={container}
```

with (the rest of the line unchanged):

```svelte
<!-- svelte-ignore a11y_mouse_events_have_key_events -->
<!-- `keepScroll`: hiding the tab zeroes the dump's scroll; showing it again puts it back. -->
<div class="hexview panel" use:keepScroll={ws.active} bind:this={container}
```

In `web/ui/src/fs/ext/BlockGroupMap.svelte`, replace

```svelte
  import { cellRect, dashCell, dotCell, drawChain, outlineCell, prepareCanvas } from "../../core/grid";
  import { observeWidth } from "../../core/observeWidth";
```

with:

```svelte
  import { cellRect, dashCell, dotCell, drawChain, outlineCell, prepareCanvas } from "../../core/grid";
  import { keepScroll } from "../../core/keepScroll";
  import { observeWidth } from "../../core/observeWidth";
```

replace

```svelte
  const { volume, selection, layers } = getWorkspace();
```

with:

```svelte
  const ws = getWorkspace();
  const { volume, selection, layers } = ws;
```

and replace

```svelte
  <div class="bgmap-wrap" bind:this={wrap} use:observeWidth
```

with (the rest of the line unchanged):

```svelte
  <!-- `keepScroll`: hiding the tab zeroes the map's scroll; showing it again puts it back. -->
  <div class="bgmap-wrap" bind:this={wrap} use:keepScroll={ws.active} use:observeWidth
```

Both components already set their own `scrollTop` state from `onscroll`, so the restore's scroll event brings their rendered window back with it.

- [ ] **Step 11: `Timeline.svelte` stops playing when its tab hides**

In `web/ui/src/components/Timeline.svelte`, replace

```svelte
  onDestroy(stop);
```

with:

```svelte
  onDestroy(stop);
  // Playback belongs to the tab on screen: hiding the tab stops it, so a hidden timeline never
  // steps on (and never moves its dump) behind another tab.
  $effect(() => {
    if (!ws.active) stop();
  });
```

(`ws` is already `getWorkspace()` in this component since Task 1.)

- [ ] **Step 12: `LessonPanel.svelte` leaves focus on the activated tab**

App keys the lesson card on the active tab, so switching back to a tab with a running lesson remounts the card, and its focus effect's first run would pull focus from the tab to `#lesson-title`. In `web/ui/src/components/LessonPanel.svelte`, replace

```svelte
  // from the hidden elements, so the new step is announced either way.
  $effect(() => {
    void scenarios.index;
    void tick().then(() => (lessonWindow.minimized ? cardEl : titleEl)?.focus());
```

with:

```svelte
  // from the hidden elements, so the new step is announced either way.
  // A tab switch remounts the card over a lesson already under way (App keys it on the active
  // tab): focus stays on the tab that was activated, as the tabs pattern expects, and moves
  // only on the lesson's own steps after that.
  let onTab = document.activeElement?.getAttribute("role") === "tab";
  $effect(() => {
    void scenarios.index;
    if (onTab) {
      onTab = false;
      return;
    }
    void tick().then(() => (lessonWindow.minimized ? cardEl : titleEl)?.focus());
```

Starting a lesson from the picker (focus on `#scenario-start`) still moves focus to the card's title.

- [ ] **Step 13: Gates**

Run: `pnpm test`
Expected: `Test Files  53 passed (53)`, `Tests  519 passed (519)`.

Run: `pnpm build`
Expected: `svelte-check` reports `COMPLETED 462 FILES 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS`, then `✓ built in …`.

Run (from the repo root): `git status --short`
Expected: only the 13 files of this task; `pnpm-lock.yaml` and `pnpm-workspace.yaml` untouched; nothing under `crates/` or `web/demo/`.

- [ ] **Step 14: Browser check (1440 × 900)**

In a fresh worktree `preview_start ui` serves the main checkout: run `pnpm exec vite --port <free port> --strictPort` from the worktree's `web/ui` in the background and open that URL with `preview_start`; stop both afterwards. `requestAnimationFrame` (and so the scroll restore and every ResizeObserver) only runs while the tab is the pane's fronted one: front it before checking scroll or widths.

Check: the bare URL opens FAT16 with the hash `#fat16` and title `fs explorer · FAT16`, only `#workspace-fat16` in the DOM, `#tab-ext` with `tabindex=-1` and no `aria-controls`. Add file, scroll the dump to 2400. Click `ext`: a fresh ext3 disk (Files `/`, `lost+found`; `Block groups · 2 groups · 16,384 blocks`; `Journal · ordered mode` under it; timeline `No operations yet`), hash `#ext`, title `fs explorer · ext`, focus on `#tab-ext`, `#workspace-fat16` `display: none` and `#workspace-ext` `display: contents` (never both shown). Add file on ext; scroll the block-group map to 700 and the dump to 1500. Click `FAT16`: its timeline and file are there and the dump is at 2400 (it read 0 while hidden); back on ext the map is at 700, the dump at 1500, and the map canvas still 198 px wide. Start `The fundamentals` on FAT16 and press `n` twice (`Lesson · step 3 of 12`); switch to ext: the `lesson` badge on FAT16, no lesson card; back: the card at step 3, focus on `#tab-fat16`. With focus on a tab, Right/Left wrap between the two, Home and End go to FAT16 and ext, each switching the tab, hash, and title with focus following. Setting the hash to `#fat` switches to FAT16 and rewrites it to `#fat16`; `#bogus` keeps the tab and puts its hash back; `#EXT3` opens ext as `#ext`. A reload on `#ext` opens only `#workspace-ext`, fresh, with no badge. On ext with three steps, Play from step 1 then switching to FAT16 for 2 s leaves ext at step 1 with `Play` showing. At 1100 px the top bar is one row (h1, tabs, Terminal, picker, theme switch) and the status line a row under it, no horizontal scroll; at 760 px the rows are h1 + theme switch, the two tabs at 368 × 36 px each, Terminal + picker, then the status line; at 375 px the picker wraps under Terminal. No console errors. Transitional until Task 5: once the ext tab has opened, both tabs' Actions panels render `<select id="format-family">`, so the page has that id twice; Task 5 removes the select, so this is not a defect of this task.

- [ ] **Step 15: Commit**

```sh
git add -A web/ui
git commit -m "feat(ui): a tab per filesystem family, linked from the URL hash" -m "The top bar gains FAT16 and ext tabs (WAI-ARIA tabs with automatic activation, roving tabindex, Left/Right/Home/End, a lesson badge on a tab running one). The ext workspace is created the first time its tab opens. Each opened tab's body is a tabpanel kept mounted; the hash and the document title follow the active tab, and a pasted hash switches to it. The dump and the block-group map keep their scroll across a hide, a hidden map keeps its last width, the timeline stops playing when its tab hides, the lesson card leaves focus on the activated tab, and the top bar wraps under 1100 px and 760 px."
```


### Task 3: One terminal, per-workspace host and cwd, the banner, `mkfs` per family

Spec: `docs/superpowers/specs/2026-09-25-family-tabs-design.md` section 4 (the Terminal paragraph), section 5 (what a switch changes: the terminal's command set and cwd; what stays: its open state, size, placement), section 8 (the `mkfs` tests). The tab bar is Task 2's; until it lands nothing on the page switches tabs, so the switch is unit-tested through the pure `registrationChange`/`tabBanner` helpers and browser-checked by activating a workspace from the dev server's console.

**Files**

- Modify: `web/ui/src/shell/mkfs.ts` (whole file, 1-121)
- Modify: `web/ui/src/shell/register.ts` (1-15, 36-71)
- Modify: `web/ui/src/components/TerminalPanel.svelte` (the `<script>` block from line 1 through `onMount(() => disposeTerminal);`, 1-200; the rest unchanged)
- Modify (doc comments only): `web/ui/src/shell/vfs.ts` (18-24), `web/ui/src/fs/adapter.ts` (111-114), `web/ui/src/fs/fat16/format.ts` (57-59), `web/ui/src/fs/ext/format.ts` (87-89)
- Modify (test): `web/ui/tests/shell/mutations.test.ts` (4, 307-323, 336-378)
- Modify (test): `web/ui/tests/shell/help-text.test.ts` (6-13, 37-62, 79-93)
- Modify (test): `web/ui/tests/shell/journal.test.ts` (3, 218-266)
- Unchanged but consumed: `web/ui/src/shell/storeHost.svelte.ts` (Task 1 already gave it `createStoreHost(ws, closeTerminal, applyPrompt)`)

**Interfaces**

Consumes (Task 1): `workspaces: WorkspaceRegistry` (`active`, `activate`), `Workspace` (`id`, `vfs`, `volume`), and `VolumeStore`'s `onAdopt` (`() => { this.vfs.cwd = MOUNT; }`) from `src/state/workspace.svelte.ts`; `createStoreHost(ws: Workspace, closeTerminal: () => void, applyPrompt: (prefix: string) => void): ShellHost` (`src/shell/storeHost.svelte.ts`); `TestHost.format` refusing another family (`tests/shell/helpers.ts`). Existing: `FAMILIES` (`src/fs/index.ts`), `commandSetOf`, `registerCommands`, `CommandRegistry` (`src/shell/register.ts`), `createRedirectHandler` (`src/shell/redirect.ts`), `promptFor` (`src/shell/vfs.ts`), `BrowserTerminal` 0.3.0 (`setPrompt`, `setRedirectHandler`, `snapshot.active_pane`).

Produces:

```ts
// src/shell/mkfs.ts
export function orList(items: readonly string[]): string;                                   // unchanged
export function mkfsTypes(families: Readonly<Record<string, FsFamily>>): string[];         // unchanged; mkfs passes its one family
export function familyOfType(type: string): FsFamily | undefined;                          // new: over FAMILIES, lower-cased match
export function mkfsTypeOf(host: ShellHost): string;                                        // unchanged
export function mkfsFlagsFor(families: Readonly<Record<string, FsFamily>>): Omit<MkfsFlag, "option">[];  // unchanged; mkfs passes its one family
export function mkfsCommand(host: ShellHost, vfs: Vfs): CommandDef;
  // spec from own = { [host.adapter.id]: FAMILIES[host.adapter.id] } only:
  //   summary "Format /dev/hda (clears the timeline); --type picks fat16" | "…; --type picks ext2 or ext3"
  //   --type desc "fat16 (default: the mounted volume's type)" | "ext2 or ext3 (default: …)"; --label in the family's own words
  // another family's type: ShellError("'ext3' is an ext type: switch to the ext tab to format one", { help: "types: fat16" })
  //                        ShellError("'fat16' is a FAT16 type: switch to the FAT16 tab to format one", { help: "types: ext2, ext3" })
  // unknown type: ShellError("unknown type 'ntfs'", { help: "types: fat16" | "types: ext2, ext3" })
// src/shell/register.ts
export interface Registration<W> { ws: W; set: string }
export function registrationChange<W>(prev: Registration<W> | null, next: Registration<W>): { register: boolean; banner: boolean };
  // null -> { register: true, banner: false }; tab changed -> { true, true }; same tab, set changed -> { true, false }; neither -> { false, false }
export function tabBanner(id: FsFamilyId, fsType: string): string;   // "-- ext tab: /dev/hda is ext3 --", "-- FAT16 tab: /dev/hda is FAT16 --"
// src/components/TerminalPanel.svelte: no module `vfs`, no `seenVol` effect; `hosts = new Map<Workspace, ShellHost>()`;
//   one effect over workspaces.active, commandSetOf(ws.volume.adapter), ws.volume.vol -> follow(bt, ws, set)
```

All commands below run in `web/ui` (a fresh worktree first needs, from the repo root, `wasm-pack build crates/wasm --target bundler`, then in `web/ui` `CI=true pnpm install --frozen-lockfile`). Baseline on Task 1's commit: `pnpm test` gives `Test Files  51 passed (51)`, `Tests  506 passed (506)`.

- [ ] **Step 1: Write the failing `mkfs` tests: the cross-family refusal, the per-family types and unknown-type help, `familyOfType`**

In `web/ui/tests/shell/mutations.test.ts`, replace the import

```ts
import { mkfsFlagsFor, mkfsTypeOf, mkfsTypes, orList } from "../../src/shell/mkfs";
```

with:

```ts
import { familyOfType, mkfsFlagsFor, mkfsTypeOf, mkfsTypes, orList } from "../../src/shell/mkfs";
```

Replace Task 1's transitional pin

```ts
  it("passes another family's type to the host, which refuses it and leaves the volume alone", async () => {
    const { host, vfs, defs } = setup();
    vfs.cwd = "/mnt/D";
    const e = await callErr(defs, "mkfs", { flags: { type: "ext3" } });
    expect(e.message).toBe("/dev/hda: this tab formats fat16, not ext");
    expect(host.vol.fsType()).toBe("FAT16");
    expect(host.formats).toEqual([]);
    expect(vfs.cwd).toBe("/mnt/D");
  });
```

with:

```ts
  it("refuses another family's type, naming the tab that formats it, and leaves the volume alone", async () => {
    const { host, vfs, defs } = setup();
    vfs.cwd = "/mnt/D";
    const e = await callErr(defs, "mkfs", { flags: { type: "ext3" } });
    expect(e).toEqual({ message: "'ext3' is an ext type: switch to the ext tab to format one", help: "types: fat16", code: undefined });
    expect((await callErr(defs, "mkfs", { flags: { type: "EXT2", blocks: 4096 } })).message).toBe("'ext2' is an ext type: switch to the ext tab to format one");
    expect(host.vol.fsType()).toBe("FAT16");
    expect(host.formats).toEqual([]);
    expect(vfs.cwd).toBe("/mnt/D");
    expect(host.prompts).toEqual([]);

    const ext = makeHost(Volume.formatExt3(undefined));
    const mirror = await callErr(createCommands(ext), "mkfs", { flags: { type: "fat16" } });
    expect(mirror).toEqual({ message: "'fat16' is a FAT16 type: switch to the FAT16 tab to format one", help: "types: ext2, ext3", code: undefined });
    expect(ext.vol.fsType()).toBe("ext3");
    expect(ext.formats).toEqual([]);
  });
```

Replace the last three tests of `describe("mkfs --type", …)`

```ts
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
    const ext = createCommands(makeHost(Volume.formatExt3(undefined)));
    expect((await callErr(ext, "mkfs", { flags: { type: "ext2", "journal-mode": "data" } })).message).toBe("/dev/hda: journalBlocks and journalMode need variant ext3");
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
```

with (the ext-typed cases move to an ext host, since a FAT host now refuses the type before it looks at the flags):

```ts
  it("refuses a flag the chosen type does not take, naming the type", async () => {
    const { host, defs } = setup();
    expect((await callErr(defs, "mkfs", { flags: { type: "fat16", blocks: 4096 } })).message).toBe("--blocks is not a fat16 option");
    expect((await callErr(defs, "mkfs", { flags: { blocks: 4096 } })).message).toBe("--blocks is not a fat16 option");
    expect(host.formats).toEqual([]);
    const ext = makeHost(Volume.formatExt3(undefined));
    expect((await callErr(createCommands(ext), "mkfs", { flags: { type: "ext3", spc: 2 } })).message).toBe("--spc is not an ext3 option");
    expect((await callErr(createCommands(ext), "mkfs", { flags: { type: "ext2", "root-entries": 64 } })).message).toBe("--root-entries is not an ext2 option");
    expect(ext.formats).toEqual([]);
  });

  it("surfaces the family's own error through fsCall, and refuses an unknown type", async () => {
    const { host, defs } = setup();
    const ext = createCommands(makeHost(Volume.formatExt3(undefined)));
    expect((await callErr(ext, "mkfs", { flags: { type: "ext2", "journal-mode": "data" } })).message).toBe("/dev/hda: journalBlocks and journalMode need variant ext3");
    expect((await callErr(ext, "mkfs", { flags: { type: "ext3", "journal-blocks": -1 } })).message).toBe("--journal-blocks must be a non-negative integer");
    const bad = await callErr(defs, "mkfs", { flags: { type: "ntfs" } });
    expect(bad.message).toBe("unknown type 'ntfs'");
    expect(bad.help).toBe("types: fat16");
    const badExt = await callErr(ext, "mkfs", { flags: { type: "ntfs" } });
    expect(badExt.message).toBe("unknown type 'ntfs'");
    expect(badExt.help).toBe("types: ext2, ext3");
    expect(host.vol.fsType()).toBe("FAT16");
  });

  it("builds its lists from the registry: the types per family, the family of a type, the default type, and the flags", () => {
    expect([orList(["a"]), orList(["a", "b"]), orList(["fat16", "ext2", "ext3"])]).toEqual(["a", "a or b", "fat16, ext2, or ext3"]);
    expect(mkfsTypes({ fat16: FAMILIES.fat16 })).toEqual(["fat16"]);
    expect(mkfsTypes({ ext: FAMILIES.ext })).toEqual(["ext2", "ext3"]);
    expect(mkfsTypes(FAMILIES)).toEqual(["fat16", "ext2", "ext3"]);
    expect(familyOfType("fat16")?.id).toBe("fat16");
    expect(familyOfType("FAT16")?.id).toBe("fat16");
    expect(familyOfType("ext2")?.id).toBe("ext");
    expect(familyOfType("Ext3")?.id).toBe("ext");
    expect(familyOfType("ntfs")).toBeUndefined();
    expect(familyOfType("constructor")).toBeUndefined();
    expect(mkfsTypeOf(makeHost())).toBe("fat16");
    expect(mkfsTypeOf(makeHost(Volume.formatExt2(undefined)))).toBe("ext2");
    // One family keeps its own words; a flag two families share merges them.
    expect(mkfsFlagsFor({ fat16: FAMILIES.fat16 })).toEqual(FAMILIES.fat16.mkfs.flags.map(({ long, kind, desc }) => ({ long, kind, desc })));
    expect(mkfsFlagsFor({ ext: FAMILIES.ext })).toEqual(FAMILIES.ext.mkfs.flags.map(({ long, kind, desc }) => ({ long, kind, desc })));
    expect(mkfsFlagsFor(FAMILIES).find((f) => f.long === "label")).toEqual({ long: "label", kind: "str", desc: "volume label (fat16: up to 11 characters; ext: up to 16 bytes)" });
  });
```

- [ ] **Step 2: Write the failing help-text pins: both summaries, `--label` unmerged, the FAT/ext equality pin flipped**

In `web/ui/tests/shell/help-text.test.ts`, replace the header comment's lines

```ts
 * `addrHelp`), and `mkfs`'s flags (every family's `FsFamily.mkfs.flags` in one list, with
 * `--label`, which fat16 and ext share, merged, plus `--type`). The literals here are typed
```

with:

```ts
 * `addrHelp`), and `mkfs`'s summary and flags (the tab's own family's `FsFamily.mkfs.flags`
 * after `--type`, which lists only that family's types). The literals here are typed
```

Replace the two `mkfs` tests of `describe("the composed help text the terminal registers", …)`

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
```

with:

```ts
  it("pins each of FAT16's six mkfs flag descriptions, --label its own", () => {
    const flags = spec("mkfs").flags ?? [];
    const desc = (long: string) => flags.find((f) => f.long === long)?.desc;
    expect(desc("sectors")).toBe("total sectors (default 32768 = 16 MB)");
    expect(desc("spc")).toBe("sectors per cluster (default 4)");
    expect(desc("label")).toBe("volume label, up to 11 characters");
    expect(desc("root-entries")).toBe("root directory entries (default 512)");
    expect(desc("fats")).toBe("FAT copies (default 2)");
    expect(desc("reserved")).toBe("reserved sectors (default 1)");
  });

  it("pins mkfs's summary, --type, and FAT16's flags only, in their order after --type", () => {
    const mkfs = spec("mkfs");
    expect(mkfs.summary).toBe("Format /dev/hda (clears the timeline); --type picks fat16");
    expect(mkfs.flags).toEqual([
      { long: "type", shape: "str", desc: "fat16 (default: the mounted volume's type)" },
      { long: "sectors", shape: "int", desc: "total sectors (default 32768 = 16 MB)" },
      { long: "spc", shape: "int", desc: "sectors per cluster (default 4)" },
      { long: "label", shape: "str", desc: "volume label, up to 11 characters" },
      { long: "root-entries", shape: "int", desc: "root directory entries (default 512)" },
      { long: "fats", shape: "int", desc: "FAT copies (default 2)" },
      { long: "reserved", shape: "int", desc: "reserved sectors (default 1)" },
    ]);
    // Not the same list on an ext host: each tab's mkfs knows its own family only.
    expect(createCommands(makeHost(Volume.formatExt3(undefined))).find((d) => d.spec.name === "mkfs")!.spec).not.toEqual(mkfs);
  });
```

At the end of `describe("the help text on an ext3 host", …)`, right after the `seek` test, add (the block starts with a blank line):

```ts

  it("pins mkfs's summary, --type, and ext's flags only, --label its own", () => {
    const mkfs = spec("mkfs");
    expect(mkfs.summary).toBe("Format /dev/hda (clears the timeline); --type picks ext2 or ext3");
    expect(mkfs.flags).toEqual([
      { long: "type", shape: "str", desc: "ext2 or ext3 (default: the mounted volume's type)" },
      { long: "blocks", shape: "int", desc: "total 1 KiB blocks (default 16384 = 16 MB)" },
      { long: "inodes-per-group", shape: "int", desc: "inodes per block group (default one per 16 KiB)" },
      { long: "label", shape: "str", desc: "volume label, up to 16 bytes" },
      { long: "uuid", shape: "str", desc: "32 hex digits, hyphens optional" },
      { long: "journal-blocks", shape: "int", desc: "journal size in blocks, ext3 only" },
      { long: "journal-mode", shape: "str", desc: "ordered or data, ext3 only" },
    ]);
    // The same list on an ext2 host: the family's, not the mounted variant's.
    expect(createCommands(makeHost(Volume.formatExt2(undefined))).find((d) => d.spec.name === "mkfs")!.spec).toEqual(mkfs);
  });
```

- [ ] **Step 3: Write the failing tests for following the active tab: the registration decision, the banner, each tab's own cwd**

In `web/ui/tests/shell/journal.test.ts`, replace the import

```ts
import { commandSetOf, createCommands, registerCommands, type CommandRegistry } from "../../src/shell/register";
```

with:

```ts
import { commandSetOf, createCommands, registerCommands, registrationChange, tabBanner, type CommandRegistry } from "../../src/shell/register";
```

At the end of `describe("re-registration when the family changes", …)` (after the test that swaps fat16, ext3, ext2, fat16, still inside the `describe`, so it can use that block's `fakeTerminal`), add (the block starts with a blank line):

```ts

  it("keeps each tab's working directory: a switch registers over that tab's own vfs", async () => {
    const fat = { host: makeHost(), vfs: new Vfs() };
    const ext = { host: makeHost(Volume.formatExt3(undefined)), vfs: new Vfs() };
    const { term, registered } = fakeTerminal();
    const pwd = async () => (await call([{ spec: { name: "pwd", summary: "" }, fn: registered.get("pwd")! }], "pwd")).value;

    let names = registerCommands(term, fat.host, fat.vfs);
    await call(createCommands(fat.host, fat.vfs), "mkdir", { positionals: ["/mnt/D"] });
    fat.vfs.cwd = "/mnt/D";
    expect(await pwd()).toBe("/mnt/D");

    names = registerCommands(term, ext.host, ext.vfs, names);
    expect(await pwd()).toBe("/mnt"); // the ext tab's shell starts at the mount point
    expect(registered.has("crash")).toBe(true);

    registerCommands(term, fat.host, fat.vfs, names);
    expect(await pwd()).toBe("/mnt/D"); // back on FAT16, where it was left
    expect(registered.has("crash")).toBe(false);
  });
```

At the end of the file, add (the block starts with a blank line):

```ts

describe("one terminal following the active tab", () => {
  const fatTab = { id: "fat16" };
  const extTab = { id: "ext" };

  it("registers on first creation without a banner", () => {
    expect(registrationChange(null, { ws: fatTab, set: "fat16" })).toEqual({ register: true, banner: false });
  });

  it("re-registers and prints the banner when the tab changes, whatever the sets", () => {
    expect(registrationChange({ ws: fatTab, set: "fat16" }, { ws: extTab, set: "ext+journal" })).toEqual({ register: true, banner: true });
    expect(registrationChange({ ws: extTab, set: "ext+journal" }, { ws: fatTab, set: "fat16" })).toEqual({ register: true, banner: true });
    expect(registrationChange({ ws: fatTab, set: "ext" }, { ws: extTab, set: "ext" })).toEqual({ register: true, banner: true });
  });

  it("re-registers without a banner when the tab's own set changes (ext3 formatted as ext2)", () => {
    expect(registrationChange({ ws: extTab, set: "ext+journal" }, { ws: extTab, set: "ext" })).toEqual({ register: true, banner: false });
  });

  it("leaves the registration alone when neither changed (a format of the same type, a load)", () => {
    expect(registrationChange({ ws: fatTab, set: "fat16" }, { ws: fatTab, set: "fat16" })).toEqual({ register: false, banner: false });
  });

  it("names the tab and the type its disk reports in the banner", () => {
    expect(tabBanner("ext", "ext3")).toBe("-- ext tab: /dev/hda is ext3 --");
    expect(tabBanner("ext", "ext2")).toBe("-- ext tab: /dev/hda is ext2 --");
    expect(tabBanner("fat16", "FAT16")).toBe("-- FAT16 tab: /dev/hda is FAT16 --");
  });
});
```

- [ ] **Step 4: Watch them fail**

Run: `pnpm test`
Expected: `Test Files  3 failed | 48 passed (51)`, `Tests  11 failed | 502 passed (513)`. The failures are the five `one terminal following the active tab` tests (`registrationChange`/`tabBanner` are not exported yet), the three help-text `mkfs` pins (the summary still lists `fat16, ext2, or ext3` and `--label` is merged), and three `mkfs --type` tests (the cross-family refusal, the unknown type's `types: fat16` help, `familyOfType`). `refuses a flag the chosen type does not take` and `keeps each tab's working directory` already pass: they pin behaviour this task keeps.

- [ ] **Step 5: Build `mkfs` from the tab's family only**

Replace `web/ui/src/shell/mkfs.ts` with:

```ts
import type { CommandDef } from "./types";
import { FAMILIES } from "../fs";
import type { FsFamily, MkfsFlag } from "../fs/adapter";
import { flagGiven } from "./bytes";
import { ShellError, fsCall } from "./errors";
import type { ShellHost } from "./host";
import { Vfs, promptFor } from "./vfs";

/**
 * The `mkfs` command and the helpers that build its spec from a family's `mkfs` flags. Each tab
 * formats its own family only, so the spec is built from the mounted family alone: `--type`
 * picks the variant (ext2 or ext3 on the ext tab), and another family's type names the tab
 * that formats it.
 */

/** `a, b, or c` (`a or b` for two): the way a summary lists choices. */
export function orList(items: readonly string[]): string {
  return items.length <= 2 ? items.join(" or ") : `${items.slice(0, -1).join(", ")}, or ${items.at(-1)}`;
}

/** `mkfs --type`'s values: the given families' `fsTypes`, lower-cased, in registry order
 *  (`fat16` on the FAT16 tab; `ext2`, `ext3` on the ext tab). */
export function mkfsTypes(families: Readonly<Record<string, FsFamily>>): string[] {
  return Object.values(families).flatMap((f) => f.fsTypes.map((t) => t.toLowerCase()));
}

/** The registered family one of whose `fsTypes` is `type`, compared lower-cased (`ext3` is
 *  ext's, `FAT16` is fat16's); `undefined` for a type no family formats. */
export function familyOfType(type: string): FsFamily | undefined {
  const wanted = type.toLowerCase();
  return Object.values(FAMILIES).find((f) => f.fsTypes.some((t) => t.toLowerCase() === wanted));
}

/** `mkfs`'s default `--type`: the mounted volume's own type, lower-cased (`FAT16` is `fat16`). */
export function mkfsTypeOf(host: ShellHost): string {
  return host.vol.fsType().toLowerCase();
}

/**
 * The given families' `mkfs.flags` as one list, in registry order and deduplicated by `long`.
 * `mkfs` passes its one family, whose flags come back in their own words. A flag two families
 * share keeps the first one's kind and merges the descriptions, each split at its first comma,
 * as `${base} (fat16: ${restA}; ext: ${restB})`.
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

/** `a` or `an` before `word`, by its first letter (`an ext3`, `a FAT16`). */
const article = (word: string): string => (/^[aeiou]/i.test(word) ? "an" : "a");

/** `mkfs`: format `/dev/hda` as `--type` (default: the mounted volume's type), one of the tab's
 *  family's types, with that family's flags; the done line names the type the new volume reports. */
export function mkfsCommand(host: ShellHost, vfs: Vfs): CommandDef {
  // The spec is the mounted family's alone: a tab formats its own family, so the summary, the
  // `--type` values, and the flags (each in the family's own words) are that family's. A
  // registration never outlives its family: the terminal re-registers on a tab switch.
  const family = FAMILIES[host.adapter.id];
  const own = { [family.id]: family };
  const types = mkfsTypes(own);
  const flags = mkfsFlagsFor(own);
  const typesHelp = `types: ${types.join(", ")}`;
  return {
    spec: {
      name: "mkfs",
      summary: `Format /dev/hda (clears the timeline); --type picks ${orList(types)}`,
      flags: [
        { long: "type", desc: `${orList(types)} (default: the mounted volume's type)`, shape: "str" },
        ...flags.map((f) => ({ long: f.long, desc: f.desc, shape: f.kind })),
      ],
    },
    fn: (args, _input, ctx) => {
      const typeFlag = args.flags.type;
      const type = flagGiven(typeFlag) ? String(typeFlag).toLowerCase() : mkfsTypeOf(host);
      const owner = familyOfType(type);
      if (!owner) throw new ShellError(`unknown type '${type}'`, { help: typesHelp });
      if (owner.id !== family.id) {
        throw new ShellError(`'${type}' is ${article(owner.name)} ${owner.name} type: switch to the ${owner.name} tab to format one`, { help: typesHelp });
      }
      // browser-terminal refuses a flag the spec does not list before this runs; a caller that
      // bypasses the parser (the tests) still gets a refusal naming the type.
      for (const [long, v] of Object.entries(args.flags)) {
        if (long !== "type" && flagGiven(v) && !flags.some((f) => f.long === long)) {
          throw new ShellError(`--${long} is not ${article(type)} ${type} option`);
        }
      }
      // A family with several types (ext: ext2, ext3) takes the one asked for as its `variant`.
      const options: Record<string, number | string> = family.fsTypes.length > 1 ? { variant: type } : {};
      for (const f of family.mkfs.flags) {
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
      vfs.cwd = "/mnt";
      host.setPrompt(promptFor(vfs.cwd));
      ctx.log(`formatted /dev/hda as ${host.vol.fsType()}; the timeline was cleared`);
    },
  };
}
```

(The refusal is a `ShellError` thrown before `fsCall`, so it prints without the `/dev/hda:` prefix the host's own refusal carried in Task 1's transitional pin. `mkfs` still resets `vfs.cwd` itself: the test host has no `onAdopt`, and in the app the workspace's `onAdopt` has already done the same. The local `article` helper is temporary: Task 4, which runs beside this task, adds the same rule as `articleFor` in `src/core/loadNotice.ts`, and Task 5 Step 9 makes `mkfs.ts` import that one and deletes `article`.)

- [ ] **Step 6: Add the pure decision and banner helpers to `register.ts`**

In `web/ui/src/shell/register.ts`, replace the imports' second line

```ts
import type { FsAdapter } from "../fs/adapter";
```

with:

```ts
import { FAMILIES } from "../fs";
import type { FsAdapter, FsFamilyId } from "../fs/adapter";
```

In `createCommands`' doc comment replace

```ts
 * and is not registered here. `vfs` holds the one working directory per page.
```

with:

```ts
 * and is not registered here. `vfs` holds the working directory of the tab `host` belongs to.
```

In `registerCommands`' doc comment replace

```ts
 * directory across a re-registration.
```

with:

```ts
 * directory across a re-registration; each tab passes its own, so a switch keeps each tab's.
```

and append at the end of the file:

```ts

/** What the page's one terminal has registered: whose tab (`ws`, compared by identity) and
 *  which command set (`commandSetOf`). */
export interface Registration<W> {
  ws: W;
  set: string;
}

/**
 * What the terminal does to follow the active tab from `prev` (`null` before its first
 * registration) to `next`: `register` the tab's commands again when the tab or its command set
 * changed, and print the tab's `banner` when the tab changed, never on the first registration.
 * Anything else (a format of the same type, a load) only re-sets the prompt.
 */
export function registrationChange<W>(prev: Registration<W> | null, next: Registration<W>): { register: boolean; banner: boolean } {
  if (prev === null) return { register: true, banner: false };
  const tabChanged = prev.ws !== next.ws;
  return { register: tabChanged || prev.set !== next.set, banner: tabChanged };
}

/** The line the terminal prints when it follows a switch: `-- ext tab: /dev/hda is ext3 --`. The
 *  scrollback is shared by every tab, so this marks where one tab's output ends. */
export function tabBanner(id: FsFamilyId, fsType: string): string {
  return `-- ${FAMILIES[id].name} tab: /dev/hda is ${fsType} --`;
}
```

- [ ] **Step 7: Watch them pass**

Run: `pnpm test`
Expected: `Test Files  51 passed (51)`, `Tests  513 passed (513)`.

- [ ] **Step 8: `TerminalPanel` follows the active tab: a host per workspace, one effect, the banner**

browser-terminal 0.3.0 has no public call that prints a host line (`run()` returns its output instead of printing it), so the banner goes through the pane manager's `paneOutput` event, guarded so a later library version that renames it only loses the banner. It replaces the pane's current prompt line (`\r\x1b[2K`) and clears the prefix, because an idle pane redraws its prompt only when the prefix changes; `follow` then sets the prefix again, which redraws the prompt and any typed input on the fresh line.

In `web/ui/src/components/TerminalPanel.svelte`, replace everything from `<script lang="ts">` through `  onMount(() => disposeTerminal);` (the drag handlers, `onKeydown`, and the markup below stay as they are) with:

```svelte
<script lang="ts">
  import { onMount, tick, untrack } from "svelte";
  // browser-terminal renders into our element with no shadow DOM and no stylesheet of
  // its own, so the app loads xterm's CSS. (@xterm/xterm has no `exports` map; the
  // deep path resolves.)
  import "@xterm/xterm/css/xterm.css";
  // Type-only: the runtime import is the lazy `import()` in ensureCreated, so the
  // library's wasm loads only when someone opens the drawer.
  import type { BrowserTerminal } from "@benjamin-small/browser-terminal";
  import { themeFromTokens } from "../core/terminalTheme";
  import { commandSetOf, registerCommands, registrationChange, tabBanner, type Registration } from "../shell/register";
  import type { ShellHost } from "../shell/host";
  import { createRedirectHandler } from "../shell/redirect";
  import { createStoreHost } from "../shell/storeHost.svelte";
  import { promptFor } from "../shell/vfs";
  import { terminal } from "../state/terminal.svelte";
  import { theme } from "../state/theme.svelte";
  import { workspaces, type Workspace } from "../state/workspace.svelte";

  let mountEl = $state<HTMLDivElement>();
  let bt: BrowserTerminal | null = null;
  let creating: Promise<void> | null = null;

  // One terminal for the page, following the active tab. Each tab's workspace gets its own
  // host the first time the terminal follows it, kept for the life of the terminal: a host
  // reads its workspace's stores live, so it never goes stale. The working directory is the
  // workspace's own `vfs`, so a switch keeps each tab's.
  const hosts = new Map<Workspace, ShellHost>();
  // The names the terminal holds, and whose tab and which command set (`commandSetOf`) they
  // were built for, so the effect below can tell a switch from a format. Plain fields, not state.
  let registered: string[] = [];
  let current: Registration<Workspace> | null = null;

  function hostFor(term: BrowserTerminal, ws: Workspace): ShellHost {
    let host = hosts.get(ws);
    if (!host) {
      host = createStoreHost(ws, close, (prefix) => term.setPrompt(prefix));
      hosts.set(ws, host);
    }
    return host;
  }

  /**
   * Print `line` in the active pane in place of its prompt line, leaving the cursor on a fresh
   * line for the caller's `setPrompt` to redraw the prompt (and any typed input) on.
   * browser-terminal 0.3.0 has no call for a host line (`run()` hands its output back instead
   * of printing it), so this gives the pane manager the `paneOutput` event the engine itself
   * sends. An idle pane redraws its prompt only when the prefix changes, so the prefix is
   * cleared here and the caller's `setPrompt` sets it again. Guarded: if a later version
   * renames that internal, the banner is skipped and nothing else changes.
   */
  function printLine(term: BrowserTerminal, line: string) {
    type PaneOutput = { type: "paneOutput"; pane: number; data: string };
    const panes = (term as unknown as { paneManager?: { handleEvent?: (e: PaneOutput) => void } }).paneManager;
    const pane = term.snapshot?.active_pane;
    if (typeof panes?.handleEvent !== "function" || pane === undefined) return;
    panes.handleEvent({ type: "paneOutput", pane, data: `\r\x1b[2K${line}\r\n` });
    term.setPrompt("");
  }

  /**
   * Point the terminal at `ws`, whose command set is `set`: register that tab's commands over
   * its own `vfs` and its redirect handler when the tab or the set changed, print the banner
   * when the tab changed (not on the first registration), and always re-set the prompt from
   * the tab's cwd, which `VolumeStore`'s `onAdopt` puts back at /mnt after a format or a load.
   */
  function follow(term: BrowserTerminal, ws: Workspace, set: string) {
    const next = { ws, set };
    const change = registrationChange(current, next);
    const host = hostFor(term, ws);
    if (change.register) {
      registered = registerCommands(term, host, ws.vfs, registered);
      // `>`, `>>` and `<` resolve through the same VFS the commands do, so
      // `echo hi > /mnt/A.TXT` is the journaled write `echo hi | write /mnt/A.TXT` is.
      term.setRedirectHandler(createRedirectHandler(host, ws.vfs));
      current = next;
    }
    if (change.banner) printLine(term, tabBanner(ws.id, ws.volume.vol.fsType()));
    host.setPrompt(promptFor(ws.vfs.cwd));
  }

  /** Close the drawer and hand focus to the topbar button, since the element that had
   *  focus (the active pane's terminal input) is about to be hidden. `exit`, the Close
   *  button, and Escape on the bar all come through here. */
  function close() {
    terminal.close();
    document.getElementById("terminal-toggle")?.focus();
  }

  function focusShell() {
    bt?.focus();
  }

  /** The app's tokens as xterm settings. Read off the document each time, so the values
   *  are whatever `data-theme` on the root currently resolves them to. */
  function currentTheme() {
    return themeFromTokens(getComputedStyle(document.documentElement));
  }

  // The tokens swap when the switch sets `data-theme`, but xterm holds its theme in JS
  // rather than reading CSS, so the swap has to be pushed in. The store puts the attribute
  // on the root before this effect runs, so the computed tokens are already the new ones.
  $effect(() => {
    void theme.current;
    untrack(() => bt?.setTheme(currentTheme().theme));
  });

  /**
   * Create the terminal on first use. Called after the drawer is visible so the
   * library's ResizeObserver fits real dimensions on its first pass. One instance per
   * page is the library's rule; `bt` is the instance, `creating` the in-flight
   * promise, so a second open during the first load reuses it.
   */
  function ensureCreated(): Promise<void> {
    if (bt) return Promise.resolve();
    if (creating) return creating;
    const mount = mountEl;
    if (!mount) return Promise.resolve();
    terminal.ready = "loading";
    terminal.error = null;
    creating = (async () => {
      const { BrowserTerminal } = await import("@benjamin-small/browser-terminal");
      const { theme: xtermTheme, fontFamily } = currentTheme();
      // 12px matches the dump's `--dump-size` neighbourhood and keeps a usable number of
      // columns in a 220px drawer; the library's own default is 13.
      const term = await BrowserTerminal.create({ mount, terminal: { theme: xtermTheme, fontFamily, fontSize: 12 } });
      try {
        // Commands read live store fields through the host on every call; only what is fixed
        // at registration (the command set, its summaries and flag descriptions) follows the
        // tab and its family, through the effect below. Register the active tab's and seed
        // the prompt with its cwd; `cd`, `mkfs`, and the effect keep it in step from there.
        const ws = workspaces.active;
        follow(term, ws, commandSetOf(ws.volume.adapter));
      } catch (e) {
        // A half-registered instance would still hold the library's one-per-page slot, so
        // every later open would fail to create and sit behind a permanent error banner.
        // Dispose it and leave `bt` null: the next open starts over.
        term.dispose();
        forget();
        throw e;
      }
      bt = term;
      terminal.ready = "ready";
    })()
      .catch((e: unknown) => {
        terminal.ready = "error";
        terminal.error = e instanceof Error ? e.message : String(e);
      })
      .finally(() => {
        creating = null;
      });
    return creating;
  }

  // Opening (and every openAndFocus while open) creates on first use, then focuses the
  // shell. The library focuses the pane itself on creation; later opens need this.
  $effect(() => {
    void terminal.focusNonce;
    if (!terminal.open) return;
    tick()
      .then(ensureCreated)
      .then(() => requestAnimationFrame(focusShell));
  });

  // Follow the active tab. Every command reads the live adapter when it runs, but
  // browser-terminal keeps each spec as it was registered: the summaries and flag descriptions
  // that name the family's nouns (`df`'s "Cluster usage", the `b:65 (block)` address help,
  // `mkfs`'s types) and whether `crash` and `recover` exist at all are fixed then. So on a tab
  // switch, or when the tab's own set changes (ext3 formatted as ext2), swap the whole set over
  // that tab's `vfs`, with the banner on a switch. A format or a load replaces the volume and
  // `onAdopt` has put that tab's cwd back at /mnt, so the prompt is re-set every time. The
  // tracked reads are the active workspace, its adapter's command set, and its `vol`; the
  // terminal, the hosts, and the registration are plain fields, and the work is untracked.
  // Before the terminal exists there is nothing to follow: creation registers the tab active then.
  $effect(() => {
    const ws = workspaces.active;
    const set = commandSetOf(ws.volume.adapter);
    void ws.volume.vol;
    untrack(() => {
      if (bt) follow(bt, ws, set);
    });
  });

  /** Drop what belonged to a terminal instance: its hosts (their prompt callback holds it) and
   *  the registration. */
  function forget() {
    hosts.clear();
    registered = [];
    current = null;
  }

  function disposeTerminal() {
    bt?.dispose();
    bt = null;
    forget();
  }
  // HMR replaces this module: dispose first or the next create() throws "one instance
  // per page". The unmount cleanup covers the non-HMR teardown. dispose() is idempotent.
  if (import.meta.hot) import.meta.hot.dispose(disposeTerminal);
  onMount(() => disposeTerminal);
```

(Gone: the `Volume` type import, `MOUNT` and `Vfs`, the module `vfs`, the single `host`, `registeredSet`, the `seenVol` cwd-reset effect, and the family re-registration effect; the cwd now resets only through the workspace's `onAdopt`.)

- [ ] **Step 9: Bring the doc comments that described one `mkfs` for every family and one cwd per page up to date**

In `web/ui/src/shell/vfs.ts`, replace

```ts
 * because fs-core rejects `.` and `..`. One cwd per page: commands cannot learn their session
 * from browser-terminal's ctx, and there is one terminal instance anyway.
```

with:

```ts
 * because fs-core rejects `.` and `..`. One cwd per tab (each workspace owns a `Vfs`), shared by
 * every session of the page's one terminal: commands cannot learn their session from
 * browser-terminal's ctx.
```

In `web/ui/src/fs/adapter.ts`, replace

```ts
/** The shell's `mkfs` flags for one family. There is one `mkfs` for every family, so its summary
 *  is the shell's (it names every type `--type` takes), and so is its done line: `formatted
```

with:

```ts
/** The shell's `mkfs` flags for one family. Each tab's `mkfs` lists its own family's flags; its
 *  summary is the shell's (it names the types `--type` takes), and so is its done line: `formatted
```

In `web/ui/src/fs/fat16/format.ts`, replace

```ts
 *  shell's (one `mkfs` for every family; the done line from the new volume's `fsType()`). */
```

with:

```ts
 *  shell's (the FAT16 tab's `mkfs --type` takes `fat16`; the done line from the new volume's
 *  `fsType()`). */
```

In `web/ui/src/fs/ext/format.ts`, replace

```ts
 *  shell's (one `mkfs` for every family; the done line names the new volume's `fsType()`,
 *  `ext2` or `ext3`). */
```

with:

```ts
 *  shell's (the ext tab's `mkfs --type` takes `ext2` or `ext3`; the done line names the new
 *  volume's `fsType()`, `ext2` or `ext3`). */
```

- [ ] **Step 10: Gates**

Run: `pnpm test`
Expected: `Test Files  51 passed (51)`, `Tests  513 passed (513)`.

Run: `pnpm build`
Expected: `svelte-check` reports `COMPLETED 458 FILES 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS`, then `✓ built in …`.

Run (from the repo root): `git status --short`
Expected: exactly the ten files of this task (`TerminalPanel.svelte`, `fs/adapter.ts`, `fs/ext/format.ts`, `fs/fat16/format.ts`, `shell/mkfs.ts`, `shell/register.ts`, `shell/vfs.ts`, and the three `tests/shell` files); `pnpm-lock.yaml`, `pnpm-workspace.yaml`, `crates/`, `web/demo/` untouched.

- [ ] **Step 11: Browser check (1440 × 900)**

In a fresh worktree `preview_start ui` serves the main checkout, so run `pnpm exec vite --port <free port> --strictPort` from this worktree's `web/ui` and open that URL. Open the terminal on the FAT16 tab: `mkdir /mnt/D`, `cd D` (prompt `/mnt/D ❯`), `echo hi > /mnt/a.txt` then `cat /mnt/a.txt` prints `hi`, `echo there | write /mnt/b.txt` prints `5 bytes -> /mnt/b.txt`, `ls /mnt` lists `D`, `A.TXT` (2), `B.TXT` (5); `help mkfs` lists `--type` as `fat16 (default: the mounted volume's type)`, the five FAT flags, and `--label` as `volume label, up to 11 characters`; `mkfs --type ext3` prints `'ext3' is an ext type: switch to the ext tab to format one` with `help: types: fat16`; `mkfs --type ntfs` prints `unknown type 'ntfs'` with `help: types: fat16`; `mkfs --label SHELL` prints `formatted /dev/hda as FAT16; the timeline was cleared`; `cd D` then the Actions panel's Format disk puts the prompt back at `/mnt ❯`. Until Task 2's tab bar lands, switch from the console with `const m = await import("/src/state/workspace.svelte.ts"); m.workspaces.activate("ext")` (the dev server serves the same module instance): the pane's prompt line becomes `-- ext tab: /dev/hda is ext3 --` with `/mnt ❯` below it, any typed input kept; `mkfs --type fat16` prints `'fat16' is a FAT16 type: switch to the FAT16 tab to format one` with `help: types: ext2, ext3`; `df` shows `ext3`; `mkfs --type ext2` then `crash` is `unknown command`; `activate("fat16")` prints `-- FAT16 tab: /dev/hda is FAT16 --` and the prompt `/mnt/D ❯` (the FAT tab's cwd kept); with both tabs at `/mnt`, a switch still draws the prompt under the banner. No console errors other than the terminal library's own log of each refused command.

- [ ] **Step 12: Commit**

```sh
git add -A web/ui
git commit -m "feat(ui): one terminal follows the active tab; mkfs formats the tab's family only" -m "Each tab keeps its own shell host and working directory: the terminal memoises a host per workspace and one effect re-registers the commands over the active tab's vfs when the tab or its command set changes, prints a banner naming the tab and its disk on a switch, and re-sets the prompt from that tab's cwd, which VolumeStore's onAdopt resets after a format or a load. mkfs builds its summary, --type values, and flags from the mounted family alone, so --label is no longer merged, and another family's type names the tab that formats it."
```


### Task 4: Load image opens an image in its own tab

Spec: `docs/superpowers/specs/2026-09-25-family-tabs-design.md` section 2 (`WorkspaceRegistry.loadImage`), section 3 (`StatusLine` renders `volume.notice` as `<span class="note">` in normal ink inside its `role="status"` region), section 4 (the Load image bullet: the muted hint and the call), section 5 (a routed Load image switches the tab). Base: Task 1 (`plan/tabs-t1`, 8f5e89a). Runs in parallel with Tasks 2 and 3; it touches none of their files (`App.svelte`, `WorkspaceView.svelte`, `TabBar.svelte`, `tests/layout.test.ts`, `src/shell/*`, `TerminalPanel.svelte`). With no tab bar yet, a routed load shows the other workspace through Task 1's `{#each}` bodies (the hidden one keeps its timeline).

**Files**

- Create: `web/ui/src/core/loadNotice.ts` (1-18)
- Create (test): `web/ui/tests/loadNotice.test.ts` (1-61)
- Modify: `web/ui/src/fs/index.ts` (1, 28-35)
- Modify: `web/ui/src/state/workspace.svelte.ts` (2, 5, 7, 85-105)
- Modify: `web/ui/src/components/ActionsPanel.svelte` (5, 59-69, 121-123)
- Modify: `web/ui/src/components/StatusLine.svelte` (48)
- Modify: `web/ui/src/app.css` (230, one line at the end of the Actions section)

**Interfaces**

Consumes (Task 1): `WorkspaceRegistry.activate(id)`, `Workspace.{id, volume, scenarios}`, `VolumeStore.{mount, report, clearMessages, notice}`, `ScenarioRunner.{current, stop}`, `workspaces`, `getWorkspace` (`src/state/workspace.svelte.ts`, `src/state/volume.svelte.ts`, `src/state/scenarios.svelte.ts`); existing `FAMILIES`, `familyIdOf` (`src/fs/index.ts`), `Volume.fromImage` (`src/lib/wasm.ts`).

Produces:

```ts
// src/core/loadNotice.ts (pure, node-tested)
export function articleFor(word: string): "a" | "an";   // "an" before a vowel (either case), else "a"
export function loadNotice(fileName: string, targetName: string, fsType: string, fromName: string): string;
  // `Opened ${fileName} in the ${targetName} tab: it is ${articleFor(fsType)} ${fsType} image. The ${fromName} tab is as you left it.`
export function loadPlan(fromId: string, targetId: string, lessonRunning: boolean): { stopLesson: boolean; notice: boolean };
  // routed = fromId !== targetId; { stopLesson: routed && lessonRunning, notice: routed }
// src/fs/index.ts
export function detectFamily(bytes: Uint8Array): { vol: Volume; id: FsFamilyId };
  // Volume.fromImage(bytes) and familyIdOf(vol.fsType()); throws "unsupported: no recognisable filesystem signature" for garbage
// src/state/workspace.svelte.ts
class WorkspaceRegistry {
  loadImage(bytes: Uint8Array, fileName: string, from: Workspace): void;
    // detectFamily inside try -> from.volume.clearMessages(); from.volume.report(e); return
    // else t = activate(id); plan = loadPlan(from.id, t.id, t.scenarios.current !== null); if (plan.stopLesson) t.scenarios.stop();
    // t.volume.mount(vol); if (plan.notice) t.volume.notice = loadNotice(fileName, FAMILIES[id].name, vol.fsType(), FAMILIES[from.id].name)
}
// ActionsPanel.svelte: onLoadImage -> workspaces.loadImage(bytes, file.name, ws); the file input gets aria-describedby="load-hint-{ws.id}";
//   <p id="load-hint-{ws.id}" class="muted load-hint">FAT16 and ext images each open in their own tab.</p> right after the Load image label
// StatusLine.svelte: {#if volume.notice}<span class="note">{volume.notice}</span>{/if} before the error span, inside role="status"
// DOM id: load-hint-{ws.id}
```

`VolumeStore.load` (Task 1) stays, but no component calls it any more. No `.note` CSS rule is needed: `.status` spans inherit `--ink`, the spec's "normal ink". The routing rule (a routed load closes a lesson running in the target tab and leaves a notice there; a same-tab load does neither) is the pure `loadPlan`, so node tests pin it; `workspace.svelte.ts` cannot be imported by them.

Spec section 5's "on a routed Load image focus stays on the file input" is not met by this task (the input's tab is hidden, so focus drops to the page); Task 7 Step 7 meets it with the per-tab id `load-image-{id}` and a focus move after `tick()`.

All commands below run in `web/ui`. Baseline on the base: `pnpm test` gives `Test Files 51 passed (51)`, `Tests 506 passed (506)`.

- [ ] **Step 1: Write the failing tests for the notice, the routing rule, and the family detection**

Create `web/ui/tests/loadNotice.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { articleFor, loadNotice, loadPlan } from "../src/core/loadNotice";
import { FAMILIES, detectFamily } from "../src/fs";
import { Volume } from "../src/lib/wasm";

describe("articleFor", () => {
  it("answers an before a vowel and a before anything else, in either case", () => {
    expect(articleFor("ext3")).toBe("an");
    expect(articleFor("ext2")).toBe("an");
    expect(articleFor("Image")).toBe("an");
    expect(articleFor("FAT16")).toBe("a");
    expect(articleFor("fat16")).toBe("a");
    expect(articleFor("")).toBe("a");
  });
});

describe("loadNotice", () => {
  it("names the file, the tab it opened in, its type, and the tab left behind", () => {
    expect(loadNotice("disk.img", "ext", "ext3", "FAT16")).toBe(
      "Opened disk.img in the ext tab: it is an ext3 image. The FAT16 tab is as you left it.",
    );
    expect(loadNotice("old.img", "FAT16", "FAT16", "ext")).toBe(
      "Opened old.img in the FAT16 tab: it is a FAT16 image. The ext tab is as you left it.",
    );
  });
});

describe("loadPlan", () => {
  it("leaves a same-tab load alone: no notice, and a running lesson keeps its card", () => {
    expect(loadPlan("fat16", "fat16", true)).toEqual({ stopLesson: false, notice: false });
    expect(loadPlan("ext", "ext", false)).toEqual({ stopLesson: false, notice: false });
  });

  it("gives a routed load a notice, and closes a lesson running in the target tab", () => {
    expect(loadPlan("fat16", "ext", true)).toEqual({ stopLesson: true, notice: true });
    expect(loadPlan("ext", "fat16", false)).toEqual({ stopLesson: false, notice: true });
  });
});

describe("detectFamily", () => {
  it("mounts a FAT16 image and names the fat16 family", () => {
    const { vol, id } = detectFamily(FAMILIES.fat16.format().image());
    expect(id).toBe("fat16");
    expect(vol.fsType()).toBe("FAT16");
  });

  it("mounts an ext3 or ext2 image and names the one ext family", () => {
    const ext3 = detectFamily(Volume.formatExt3(undefined).image());
    expect(ext3.id).toBe("ext");
    expect(ext3.vol.fsType()).toBe("ext3");
    const ext2 = detectFamily(Volume.formatExt2(undefined).image());
    expect(ext2.id).toBe("ext");
    expect(ext2.vol.fsType()).toBe("ext2");
  });

  it("throws today's load error for bytes no family recognises", () => {
    const garbage = new TextEncoder().encode("not a disk image ".repeat(64));
    expect(() => detectFamily(garbage)).toThrow("unsupported: no recognisable filesystem signature");
    expect(() => detectFamily(new Uint8Array(0))).toThrow("unsupported: no recognisable filesystem signature");
  });
});
```

Run: `pnpm exec vitest run tests/loadNotice.test.ts`
Expected: FAIL, `Error: Cannot find module '../src/core/loadNotice'`, `Test Files 1 failed (1)`, `Tests no tests`.

- [ ] **Step 2: Create the pure notice module**

Create `web/ui/src/core/loadNotice.ts`:

```ts
/** The indefinite article for `word`, by its first letter: "an ext3", "a FAT16". */
export function articleFor(word: string): "a" | "an" {
  return /^[aeiou]/i.test(word) ? "an" : "a";
}

/** The status line's notice after Load image opened `fileName` in another family's tab:
 *  where it went, what it is, and that the tab it was loaded from is untouched. */
export function loadNotice(fileName: string, targetName: string, fsType: string, fromName: string): string {
  return `Opened ${fileName} in the ${targetName} tab: it is ${articleFor(fsType)} ${fsType} image. The ${fromName} tab is as you left it.`;
}

/** What Load image does besides mounting, for an image of `targetId`'s family loaded from the
 *  `fromId` tab: a routed load (another tab) closes a lesson running in the target tab, whose
 *  disk is about to go, and leaves a notice there; a same-tab load does neither. */
export function loadPlan(fromId: string, targetId: string, lessonRunning: boolean): { stopLesson: boolean; notice: boolean } {
  const routed = fromId !== targetId;
  return { stopLesson: routed && lessonRunning, notice: routed };
}
```

- [ ] **Step 3: Add `detectFamily` to the family registry**

In `web/ui/src/fs/index.ts`, line 1:

```ts
import type { Volume } from "../lib/wasm";
```

becomes a value import:

```ts
import { Volume } from "../lib/wasm";
```

and after `adapterFor` (the end of the file) append:

```ts

/** Mount an image and name the family it belongs to, whichever tab it was loaded from. Throws
 *  the loader's error for bytes no family recognises (`unsupported: no recognisable filesystem
 *  signature`), and `familyIdOf`'s for a type with no adapter. */
export function detectFamily(bytes: Uint8Array): { vol: Volume; id: FsFamilyId } {
  const vol = Volume.fromImage(bytes);
  return { vol, id: familyIdOf(vol.fsType()) };
}
```

(`src/fs/index.ts` already imports `FsFamilyId` from `./adapter`; `tests/adapterBoundary.test.ts` does not restrict `fromImage`.)

- [ ] **Step 4: Watch the tests pass**

Run: `pnpm exec vitest run tests/loadNotice.test.ts`
Expected: `Test Files 1 passed (1)`, `Tests 7 passed (7)`.

- [ ] **Step 5: Route Load image in `WorkspaceRegistry`**

In `web/ui/src/state/workspace.svelte.ts`, the imports:

```ts
import { createContext } from "svelte";
import { parseRoute } from "../core/route";
import { stepFocusOffset } from "../core/stepFocus";
import { DEFAULT_FAMILY, FAMILY_IDS, ROUTE_ALIASES } from "../fs";
import type { FsFamilyId } from "../fs/adapter";
import { MOUNT, Vfs } from "../shell/vfs";
```

become

```ts
import { createContext } from "svelte";
import { loadNotice, loadPlan } from "../core/loadNotice";
import { parseRoute } from "../core/route";
import { stepFocusOffset } from "../core/stepFocus";
import { DEFAULT_FAMILY, FAMILIES, FAMILY_IDS, ROUTE_ALIASES, detectFamily } from "../fs";
import type { FsFamilyId } from "../fs/adapter";
import type { Volume } from "../lib/wasm";
import { MOUNT, Vfs } from "../shell/vfs";
```

and in `class WorkspaceRegistry`, after `activate` (whose body ends `this.activeId = id;` / `return ws;` / `}`) and before the class's closing brace, add:

```ts

  /** Load image, from `from`'s Actions panel: the image opens in its own family's tab, whichever
   *  tab it was loaded from. Bytes no family recognises are `from`'s error, as before. Another
   *  tab is activated (created first if need be), a lesson running there is closed (its disk is
   *  about to go), and its status line says where the image went; `from` is left as it was. */
  loadImage(bytes: Uint8Array, fileName: string, from: Workspace): void {
    let vol: Volume, id: FsFamilyId;
    try {
      ({ vol, id } = detectFamily(bytes));
    } catch (e) {
      // A notice left by an earlier routed load no longer describes the last load.
      from.volume.clearMessages();
      from.volume.report(e);
      return;
    }
    const t = this.activate(id);
    const plan = loadPlan(from.id, t.id, t.scenarios.current !== null);
    if (plan.stopLesson) t.scenarios.stop();
    t.volume.mount(vol);
    if (plan.notice) t.volume.notice = loadNotice(fileName, FAMILIES[id].name, vol.fsType(), FAMILIES[from.id].name);
  }
```

`mount` clears `status` and `notice` before the notice is set, so the notice is the target tab's only message. A same-tab load (`t === from`) behaves as `volume.load` did: mount, messages cleared, no notice, a running lesson left to its card. The `clearMessages()` before `report` matters on a tab that still shows a notice from an earlier routed load: without it the line would show that notice next to the new error.

- [ ] **Step 6: Call it from the Actions panel and add the hint**

In `web/ui/src/components/ActionsPanel.svelte`, line 5:

```svelte
  import { getWorkspace } from "../state/workspace.svelte";
```

becomes

```svelte
  import { getWorkspace, workspaces } from "../state/workspace.svelte";
```

`onLoadImage`:

```svelte
  async function onLoadImage(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0] ?? null;
    if (!file) return;
    try {
      volume.load(new Uint8Array(await file.arrayBuffer()));
    } catch (err) {
      volume.status = { text: err instanceof Error ? err.message : String(err) };
    }
  }
```

becomes

```svelte
  async function onLoadImage(e: Event) {
    const file = (e.currentTarget as HTMLInputElement).files?.[0] ?? null;
    if (!file) return;
    // An image of the other family opens in that family's tab (`loadImage` says so there).
    try {
      workspaces.loadImage(new Uint8Array(await file.arrayBuffer()), file.name, ws);
    } catch (err) {
      volume.report(err);
    }
  }
```

and the Load image field:

```svelte
    <label class="field">
      Load image
      <input type="file" onchange={onLoadImage} />
    </label>
    <button onclick={exportImage}>Export image</button>
```

becomes

```svelte
    <label class="field">
      Load image
      <input type="file" onchange={onLoadImage} aria-describedby="load-hint-{ws.id}" />
    </label>
    <p id="load-hint-{ws.id}" class="muted load-hint">FAT16 and ext images each open in their own tab.</p>
    <button onclick={exportImage}>Export image</button>
```

(`ws` is the component's existing `const ws = getWorkspace();`. The `<p>` sits outside the `<label>`, which allows phrasing content only; `aria-describedby` reads it with the input.)

- [ ] **Step 7: Show the notice on the status line**

In `web/ui/src/components/StatusLine.svelte`, inside `<div class="status" role="status">`:

```svelte
  {#if volume.needsRecovery}<span>Volume needs recovery</span>{/if}
  {#if volume.status}<span class="err">
```

becomes

```svelte
  {#if volume.needsRecovery}<span>Volume needs recovery</span>{/if}
  {#if volume.notice}<span class="note">{volume.notice}</span>{/if}
  {#if volume.status}<span class="err">
```

(the rest of the `volume.status` line is unchanged).

- [ ] **Step 8: Space the hint**

In `web/ui/src/app.css`, at the end of the Actions section, after

```css
.actions .using-bytes { display: flex; align-items: center; gap: 8px; margin: 0; font-size: 12px; }
```

add one line:

```css
.actions .load-hint { margin: -4px 0 0; font-size: 12px; }
```

(The fieldset's 8 px column gap less 4 px tucks the hint under its input.)

- [ ] **Step 9: Run the gates**

Run: `pnpm test`
Expected: `Test Files 52 passed (52)`, `Tests 513 passed (513)`.

Run: `pnpm build`
Expected: svelte-check `COMPLETED 460 FILES 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS`, then `✓ built in …`.

`git status --short` shows only the seven files above (never `pnpm-lock.yaml` or `pnpm-workspace.yaml`).

- [ ] **Step 10: Check it in the browser**

From `web/ui`: `pnpm exec vite --port 5194 --strictPort` (Bash, background; any free port), then open `http://localhost:5194/` with `preview_start` by URL. There is no tab bar until Task 2, so drive the real file input from the page with `javascript_exec`: build bytes in the page (`const { Volume } = await import('/src/lib/wasm.ts'); Volume.formatExt3(undefined).image()`; a FAT image from `(await import('/src/fs/index.ts')).FAMILIES.fat16.format().image()`; garbage `new TextEncoder().encode('not a disk image '.repeat(64))`), wrap it as `new File([bytes], name)` in a `new DataTransfer()`, assign `dt.files` to `document.querySelector('#workspace-<id> input[aria-describedby="load-hint-<id>"]').files`, and dispatch a bubbling `change`.

Expected (as observed):
- FAT16 tab, Add file, then Load image of garbage: stays on `#workspace-fat16`; status `unsupported: no recognisable filesystem signature Unsupported` (today's error).
- Load `mke2fs-ext3.img` from the FAT16 tab: `#workspace-fat16` becomes hidden, `#workspace-ext` is created and shown (Block groups, `Journal · ordered mode`), and the status line reads `Opened mke2fs-ext3.img in the ext tab: it is an ext3 image. The FAT16 tab is as you left it.`; the hidden FAT16 workspace's Step strip still reads `1 of 1 create_file /Hello world.txt`.
- Garbage on the ext tab: stays on ext; the notice is replaced by the same error alone.
- A FAT16 image loaded from the ext tab routes back to FAT16 with `Opened fat.img in the FAT16 tab: it is a FAT16 image. The ext tab is as you left it.` and the image's file in Files; a FAT lesson started, then routed away from (ext image) and back (FAT image), is closed (no `#lesson-title`).
- No console errors. Stop the dev server and close the browser tab afterwards.

- [ ] **Step 11: Commit**

```sh
git add web/ui/src/app.css web/ui/src/components/ActionsPanel.svelte web/ui/src/components/StatusLine.svelte web/ui/src/fs/index.ts web/ui/src/state/workspace.svelte.ts web/ui/src/core/loadNotice.ts web/ui/tests/loadNotice.test.ts
git commit -m "feat(ui): Load image opens an image in its own family's tab" -m "WorkspaceRegistry.loadImage detects the image's family (detectFamily in fs/index.ts), activates that family's workspace, closes a lesson running there, mounts the volume, and leaves a notice on its status line naming the file, the type, and the untouched tab. The routing rule is the pure, tested loadPlan in core/loadNotice.ts. Bytes no family recognises stay the loading tab's error. The Actions panel hints that each family's images open in their own tab."
```


### Task 5: Per-tab lessons, the tab-bound Format form, the Journal on the right, `summary()`

Spec: `docs/superpowers/specs/2026-09-25-family-tabs-design.md` section 4 (the per-tab column lists with the Journal in the right column and `PANELS[id].aside`; `summary()` and its three strings; the Format details bullet: no Filesystem select, `Format (FAT16)` / `Format (ext2 / ext3)`; the lesson picker bullet: `lessonsFor`, `defaultLessonFor`, the per-tab choice, `extFundamentals.title`), section 5 (the picker list and choice change on a switch), section 8 (`tests/scenarios.test.ts`, the adapter `summary()` strings, `tests/ext-fundamentals.test.ts`'s title pin). Base: `plan/tabs-base-5` (Tasks 1 to 4 merged). The Operation bar that shows `summary()` is Task 6's; this task only adds the method. The What changed panel that will sit above the aside panels is Task 6's too.

**Files**

- Modify: `web/ui/src/scenarios/index.ts` (1-2, 16-34)
- Modify: `web/ui/src/scenarios/extFundamentals.ts` (33)
- Modify: `web/ui/src/fs/adapter.ts` (104-118, 209-213)
- Modify: `web/ui/src/fs/fat16/adapter.ts` (4, 57-62)
- Modify: `web/ui/src/fs/ext/adapter.ts` (5, 119-127)
- Modify: `web/ui/src/fs/panels.ts` (9-21)
- Modify: `web/ui/src/components/WorkspaceView.svelte` (21-23, 26-29, 33-43)
- Modify: `web/ui/src/components/ActionsPanel.svelte` (1-12, 103-106)
- Modify: `web/ui/src/components/ScenarioPanel.svelte` (whole file, 1-47)
- Modify: `web/ui/src/shell/mkfs.ts` (2; the local `article` helper removed)
- Modify (test): `web/ui/tests/scenarios.test.ts` (4, 51-81)
- Modify (test): `web/ui/tests/ext-fundamentals.test.ts` (7, 26)
- Modify (test): `web/ui/tests/fs/fat16.test.ts` (44-52)
- Modify (test): `web/ui/tests/fs/ext.test.ts` (192-202)

`mkfs.ts` is Task 3's file: Tasks 3 and 4 ran side by side, each with its own copy of the a/an rule, and this is the first task that has both, so it keeps Task 4's `articleFor` and drops Task 3's `article`. `tests/shell/helpers.ts` needs no change: its `TestHost` holds a real adapter from `adapterFor`, which gains `summary()` with the rest.

**Interfaces**

Consumes (Task 1): `getWorkspace()` and `Workspace.{id, volume, scenarios}` (`src/state/workspace.svelte.ts`), `VolumeStore.{family, report}` (`src/state/volume.svelte.ts`), `ScenarioRunner.start` (refuses another family's lesson; `src/state/scenarios.svelte.ts`), `FAMILY_IDS` (`src/fs/index.ts`). (Task 4): `articleFor` (`src/core/loadNotice.ts`). Existing: `FAMILIES` (`src/fs/index.ts`), `FsFamilyId`, `FsAdapter` (`src/fs/adapter.ts`), `Scenario` (`src/state/scenarios.svelte.ts`), `all` (`src/scenarios/index.ts`), wasm's `Geometry` (`bytesPerSector`, `sectorsPerCluster`, `totalSectors`), `ExtGeometry` (`blockSize`, `totalBlocks`, `groups`), `JournalInfo` (`maxlen`, `mode`).

Produces:

```ts
// src/scenarios/index.ts
export function lessonsFor(family: FsFamilyId, list: readonly Scenario[] = all): Scenario[];   // `list` filtered to `family`, in order
export function defaultLessonFor(family: FsFamilyId, list: readonly Scenario[] = all): Scenario;   // the first; throws Error(`no lessons for ${family}`)
// scenarioGroups and ScenarioGroup are removed. extFundamentals.title === "The fundamentals"
// src/fs/adapter.ts
export function megabytes(bytes: number): string;       // MB of 2^20 bytes, no decimals when whole, else one: "16 MB", "10.5 MB"
export function unitSizeLabel(bytes: number): string;   // "2 KiB", "1 KiB" from 1 KiB up, else "512-byte"
interface FsAdapter { summary(): string }               // from the mounted geometry:
  // "FAT16 · 16 MB · 512-byte sectors · 2 KiB clusters"
  // "ext3 · 16 MB · 16,384 1 KiB blocks in 2 groups · 1,024-block journal, ordered mode"
  // "ext2 · 16 MB · 16,384 1 KiB blocks in 2 groups · no journal"   ("1 group" when there is one)
// src/fs/panels.ts
export const PANELS: Record<FsFamilyId, { map: Component; format: Component; aside: Component[] }>;   // fat16: aside []; ext: aside [JournalPanel]
// WorkspaceView.svelte: {#each PANELS[ws.id].aside as Aside}<Aside />{/each} first in .col.right; nothing after the map in .col.left
// ActionsPanel.svelte: no #format-family select, no `family` state or effect; FormatPanel = PANELS[volume.family].format;
//   <summary>Format ({FAMILIES[volume.family].fsTypes.join(" / ")})</summary>
// ScenarioPanel.svelte: flat <option>s from lessonsFor(ws.id); a module-level
//   picked = $state<Record<FsFamilyId, string>>(defaultLessonFor(id).id for each of FAMILY_IDS), bound as picked[ws.id]
```

`picked` lives in the component's `<script module>`: App rebuilds the picker inside `{#key workspaces.active}` on every switch, so instance state would reset to the default each time. Task 1's transitional `try`/`catch` around `scenarios.start` stays (it still compiles; the list makes it unreachable). Numbers in `summary()` use `toLocaleString("en-US")` so the pins hold in any locale.

All commands below run in `web/ui` (a fresh worktree first needs, from the repo root, `wasm-pack build crates/wasm --target bundler`, then in `web/ui` `CI=true pnpm install --frozen-lockfile`). Baseline on `plan/tabs-base-5`: `pnpm test` gives `Test Files  54 passed (54)`, `Tests  533 passed (533)`.

- [ ] **Step 1: Write the failing lesson-list tests and the title pin**

In `web/ui/tests/scenarios.test.ts`, line 4:

```ts
import { all, scenarioGroups } from "../src/scenarios";
```

becomes

```ts
import { all, defaultLessonFor, lessonsFor } from "../src/scenarios";
```

and the two tests

```ts
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

become

```ts
  // `all` keeps each family's lessons together, FAT first, each family's tour first.
  it("lists the FAT lessons first, then the three ext lessons", () => {
    expect(all.map((s) => s.family)).toEqual([...new Array(9).fill("fat16"), "ext", "ext", "ext"]);
    expect(all.slice(9).map((s) => s.id)).toEqual(["ext-fundamentals", "journaled-write", "crash-recover"]);
    expect(all[0].id).toBe("fundamentals");
  });

  // Each tab's picker lists its own family's lessons only, flat, in `all` order.
  it("gives each tab its own lessons, in order", () => {
    expect(lessonsFor("fat16").map((s) => s.id)).toEqual([
      "fundamentals", "format", "small-file", "long-name", "overwrite-grows", "delete-remnants", "fill-disk", "directory", "shell",
    ]);
    expect(lessonsFor("ext").map((s) => s.id)).toEqual(["ext-fundamentals", "journaled-write", "crash-recover"]);
    expect(lessonsFor("ext").map((s) => s.title)).toEqual(["The fundamentals", "A journaled write", "Crash and recover"]);
    // A list without a family's lessons gives that family none.
    expect(lessonsFor("ext", all.slice(0, 9))).toEqual([]);
  });

  it("puts every lesson in exactly one tab's list", () => {
    expect(Object.keys(FAMILIES)).toEqual(["fat16", "ext"]);
    const listed = Object.values(FAMILIES).flatMap((f) => lessonsFor(f.id));
    expect(listed).toHaveLength(all.length);
    expect(new Set(listed)).toEqual(new Set(all));
  });

  // The picker preselects the first of the tab's lessons: each family's fundamentals.
  it("defaults each tab to its first lesson, the fundamentals", () => {
    expect(defaultLessonFor("fat16")).toBe(lessonsFor("fat16")[0]);
    expect(defaultLessonFor("fat16").title).toBe("The fundamentals");
    expect(defaultLessonFor("ext")).toBe(lessonsFor("ext")[0]);
    expect(defaultLessonFor("ext").id).toBe("ext-fundamentals");
    expect(() => defaultLessonFor("ext", all.slice(0, 9))).toThrow("no lessons for ext");
  });
```

In `web/ui/tests/ext-fundamentals.test.ts`, line 7:

```ts
// "The fundamentals (ext)" quotes the default ext3 disk's numbers in its copy (block 69,
```

becomes

```ts
// The ext tab's "The fundamentals" quotes the default ext3 disk's numbers in its copy (block 69,
```

and line 26:

```ts
    expect(scenario.title).toBe("The fundamentals (ext)");
```

becomes

```ts
    expect(scenario.title).toBe("The fundamentals");
```

Run: `pnpm exec vitest run tests/scenarios.test.ts tests/ext-fundamentals.test.ts`
Expected: FAIL, `TypeError: (0 , lessonsFor) is not a function` (twice), `TypeError: (0 , defaultLessonFor) is not a function`, `AssertionError: expected 'The fundamentals (ext)' to be 'The fundamentals'`; `Test Files  2 failed (2)`, `Tests  4 failed | 35 passed (39)`.

- [ ] **Step 2: Write the failing `summary()` pins**

In `web/ui/tests/fs/fat16.test.ts`, in `describe("Fat16Adapter: identity and unit space")`, before `it("names its sector apart from its cluster, and has no journal to recover", …)` insert:

```ts
  // The Operation bar's empty state: the mounted disk in one line, from its geometry.
  it("sums the disk up in one line from the mounted geometry", () => {
    expect(fixture().fs.summary()).toBe("FAT16 · 16 MB · 512-byte sectors · 2 KiB clusters");
    // 8,192 sectors need one-sector clusters to stay in the FAT16 range; 21,504 is 10.5 MB.
    expect(fat16.bind(fat16.format({ totalSectors: 8192, sectorsPerCluster: 1 })).summary()).toBe("FAT16 · 4 MB · 512-byte sectors · 512-byte clusters");
    expect(fat16.bind(fat16.format({ totalSectors: 21504 })).summary()).toBe("FAT16 · 10.5 MB · 512-byte sectors · 2 KiB clusters");
    expect(fat16.bind(fat16.format({ totalSectors: 65536, sectorsPerCluster: 8 })).summary()).toBe("FAT16 · 32 MB · 512-byte sectors · 4 KiB clusters");
  });

```

In `web/ui/tests/fs/ext.test.ts`, in `describe("ExtAdapter: identity and unit space")`, before `it("does block arithmetic over the volume's geometry", …)` insert:

```ts
  // The Operation bar's empty state: the mounted disk in one line, from its geometry and journal.
  it("sums the disk up in one line: ext3 with its journal, ext2 without", () => {
    expect(fixture().fs.summary()).toBe("ext3 · 16 MB · 16,384 1 KiB blocks in 2 groups · 1,024-block journal, ordered mode");
    expect(ext.bind(ext.format({ variant: "ext2" })).summary()).toBe("ext2 · 16 MB · 16,384 1 KiB blocks in 2 groups · no journal");
    expect(ext.bind(ext.format({ totalBlocks: 65536 })).summary()).toBe("ext3 · 64 MB · 65,536 1 KiB blocks in 8 groups · 4,096-block journal, ordered mode");
    // One group is "group"; a data-mode journal says so; 10,752 blocks is 10.5 MB.
    expect(ext.bind(ext.format({ variant: "ext2", totalBlocks: 4096 })).summary()).toBe("ext2 · 4 MB · 4,096 1 KiB blocks in 1 group · no journal");
    expect(ext.bind(ext.format({ totalBlocks: 4096, journalMode: "data" })).summary()).toBe("ext3 · 4 MB · 4,096 1 KiB blocks in 1 group · 1,024-block journal, data mode");
    expect(ext.bind(ext.format({ totalBlocks: 10752 })).summary()).toBe("ext3 · 10.5 MB · 10,752 1 KiB blocks in 2 groups · 1,024-block journal, ordered mode");
  });

```

(Both files already import what these use: `fat16` from `../../src/fs/fat16`; `ext` from `../../src/fs/ext`; each file's `fixture()` is the default disk.)

Run: `pnpm exec vitest run tests/fs/fat16.test.ts tests/fs/ext.test.ts`
Expected: FAIL, `TypeError: fixture(...).fs.summary is not a function` in each; `Test Files  2 failed (2)`, `Tests  2 failed | 66 passed (68)`.

- [ ] **Step 3: `lessonsFor` and `defaultLessonFor` replace the groups; the ext tour is "The fundamentals"**

In `web/ui/src/scenarios/index.ts`, the imports lose the registry:

```ts
import type { FsFamilyId } from "../fs/adapter";
import { FAMILIES } from "../fs";
```

become

```ts
import type { FsFamilyId } from "../fs/adapter";
```

and everything from the comment above `all` to the end of the file:

```ts
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

becomes

```ts
// Each family's fundamentals go first: a tab's picker defaults to its first lesson, and the
// tour is where a newcomer should start before watching individual operations. The ext
// lessons follow the FAT ones.
export const all: Scenario[] = [
  fundamentals, format, smallFile, longName, overwriteGrows, deleteRemnants, fillDisk, directory, shell,
  extFundamentals, journaledWrite, crashRecover,
];

/** A tab's lessons: the ones for `family`, in `list` order. The tab's picker lists these only. */
export function lessonsFor(family: FsFamilyId, list: readonly Scenario[] = all): Scenario[] {
  return list.filter((s) => s.family === family);
}

/** The lesson a tab's picker starts on: the first of its lessons, the family's fundamentals. */
export function defaultLessonFor(family: FsFamilyId, list: readonly Scenario[] = all): Scenario {
  const first = lessonsFor(family, list)[0];
  if (!first) throw new Error(`no lessons for ${family}`);
  return first;
}
```

In `web/ui/src/scenarios/extFundamentals.ts`, line 33:

```ts
  title: "The fundamentals (ext)",
```

becomes

```ts
  title: "The fundamentals",
```

(The picker lists one family's lessons, so the family suffix no longer tells two entries apart.)

Run: `pnpm exec vitest run tests/scenarios.test.ts tests/ext-fundamentals.test.ts`
Expected: `Test Files  2 passed (2)`, `Tests  39 passed (39)`.

- [ ] **Step 4: `FsAdapter.summary()` and its two implementations**

In `web/ui/src/fs/adapter.ts`, before `/** One row of the Inspector's "Selected file" trace. */` insert:

```ts
/** `n` with no decimals when it is whole to one decimal place, else with one: 16, 10.5. */
function oneDecimal(n: number): string {
  const r = Math.round(n * 10) / 10;
  return Number.isInteger(r) ? String(r) : r.toFixed(1);
}

/** A disk's byte size as `summary()` gives it, in MB of 2^20 bytes: "16 MB", "10.5 MB". */
export function megabytes(bytes: number): string {
  return `${oneDecimal(bytes / 2 ** 20)} MB`;
}

/** A cluster's or a block's size as `summary()` gives it: "2 KiB" from 1 KiB up, else "512-byte". */
export function unitSizeLabel(bytes: number): string {
  return bytes >= 1024 ? `${oneDecimal(bytes / 1024)} KiB` : `${bytes}-byte`;
}

```

and in `interface FsAdapter`, after

```ts
  /** Re-read owners, tables and geometry from `vol`. Called by the store after every op and on bind. */
  refresh(): void;
```

add

```ts
  /** The mounted disk in one line, from the cached geometry: its type, its size, its units, and
   *  (ext) its journal. FAT16: "FAT16 · 16 MB · 512-byte sectors · 2 KiB clusters"; ext3:
   *  "ext3 · 16 MB · 16,384 1 KiB blocks in 2 groups · 1,024-block journal, ordered mode"; ext2:
   *  "ext2 · 16 MB · 16,384 1 KiB blocks in 2 groups · no journal". The Operation bar's empty state. */
  summary(): string;
```

In `web/ui/src/fs/fat16/adapter.ts`, line 4:

```ts
import type { DfFacts, FsAdapter, FsFamily, FsFamilyId, StatFacts, TraceRow, UnitOwner, UnitSpace } from "../adapter";
```

becomes

```ts
import { megabytes, unitSizeLabel, type DfFacts, type FsAdapter, type FsFamily, type FsFamilyId, type StatFacts, type TraceRow, type UnitOwner, type UnitSpace } from "../adapter";
```

and after `refresh()` (whose body ends `this.fat = this.vol.fatEntries(0);` / `}`) add:

```ts

  /** "FAT16 · 16 MB · 512-byte sectors · 2 KiB clusters", from the boot sector's geometry. */
  summary(): string {
    const g = this.geo;
    const size = megabytes(g.totalSectors * g.bytesPerSector);
    return `${this.name} · ${size} · ${g.bytesPerSector}-byte sectors · ${unitSizeLabel(g.bytesPerSector * g.sectorsPerCluster)} clusters`;
  }
```

In `web/ui/src/fs/ext/adapter.ts`, line 5 gets the same import change:

```ts
import type { DfFacts, FsAdapter, FsFamily, FsFamilyId, StatFacts, TraceRow, UnitOwner, UnitSpace } from "../adapter";
```

becomes

```ts
import { megabytes, unitSizeLabel, type DfFacts, type FsAdapter, type FsFamily, type FsFamilyId, type StatFacts, type TraceRow, type UnitOwner, type UnitSpace } from "../adapter";
```

and before `/** The cached journal header; the capability's `state()` reads it. */` insert:

```ts
  /** "ext3 · 16 MB · 16,384 1 KiB blocks in 2 groups · 1,024-block journal, ordered mode" (or
   *  "· no journal" on ext2), from the cached geometry and journal header. */
  summary(): string {
    const g = this.geo;
    const n = (v: number) => v.toLocaleString("en-US");
    const groups = `${g.groups.length} ${g.groups.length === 1 ? "group" : "groups"}`;
    const journal = this.info ? `${n(this.info.maxlen)}-block journal, ${this.info.mode} mode` : "no journal";
    return `${this.name} · ${megabytes(g.totalBlocks * g.blockSize)} · ${n(g.totalBlocks)} ${unitSizeLabel(g.blockSize)} blocks in ${groups} · ${journal}`;
  }

```

(`this.info` is the cached `JournalInfo`, undefined on ext2; `this.name` is `ext2`/`ext3` from the last `refresh()`.)

Run: `pnpm exec vitest run tests/fs/fat16.test.ts tests/fs/ext.test.ts tests/adapterBoundary.test.ts`
Expected: `Test Files  3 passed (3)`, `Tests  72 passed (72)`.

- [ ] **Step 5: `PANELS[id].extras` becomes `aside`**

In `web/ui/src/fs/panels.ts`, the doc comment and the export:

```ts
/**
 * The Svelte panels of each family, looked up by `volume.adapter.id`: `map` is the panel App
 * renders under the Files tree (the FAT map, the block-group map), `format` the body of the
 * Actions panel's Format details for the family its Filesystem select names, and `extras` the
 * family's further panels, rendered under the map in order (FAT has none; ext has the Journal
 * panel, which on ext2 says there is no journal). They live here rather than on the adapter
 * because `vitest.config.ts` has no Svelte plugin: an adapter that imported a `.svelte` file
 * could not be loaded by the node tests.
 */
export const PANELS: Record<FsFamilyId, { map: Component; format: Component; extras: Component[] }> = {
  fat16: { map: FatMap, format: FormatForm, extras: [] },
  ext: { map: BlockGroupMap, format: ExtFormatForm, extras: [JournalPanel] },
};
```

become

```ts
/**
 * The Svelte panels of each family, looked up by the tab's family id: `map` is the panel
 * WorkspaceView renders under the Files tree (the FAT map, the block-group map), `format` the
 * body of the Actions panel's Format details (a tab formats its own family), and `aside` the
 * family's further panels, rendered in order at the top of the right column (FAT has none; ext
 * has the Journal panel, which on ext2 says there is no journal). They live here rather than on
 * the adapter because `vitest.config.ts` has no Svelte plugin: an adapter that imported a
 * `.svelte` file could not be loaded by the node tests.
 */
export const PANELS: Record<FsFamilyId, { map: Component; format: Component; aside: Component[] }> = {
  fat16: { map: FatMap, format: FormatForm, aside: [] },
  ext: { map: BlockGroupMap, format: ExtFormatForm, aside: [JournalPanel] },
};
```

- [ ] **Step 6: `WorkspaceView` renders the aside panels at the top of the right column**

In `web/ui/src/components/WorkspaceView.svelte`:

```svelte
  /** The map panel of the tab's family (the FAT map, the block-group map), from the panel
   *  registry. The family's extra panels follow it in order. */
  const MapPanel = $derived(PANELS[ws.id].map);
</script>

<!-- The current step sits with the other controls, under the picker and the Terminal
     button, so the right column is left to state and data (Lesson, Strings, Inspector). The
     slots are classes, not ids: every opened tab has its own. -->
```

becomes

```svelte
  /** The map panel of the tab's family (the FAT map, the block-group map), from the panel
   *  registry. The family's aside panels (ext: the Journal) head the right column. */
  const MapPanel = $derived(PANELS[ws.id].map);
</script>

<!-- The current step sits with the other controls, under the picker and the Terminal
     button, so the right column is left to state and data (the family's aside panels, such as
     ext's Journal, then Strings and the Inspector). The slots are classes, not ids: every
     opened tab has its own. -->
```

and the grid:

```svelte
  <aside class="col left">
    <DirTree />
    <MapPanel />
    {#each PANELS[ws.id].extras as Extra}<Extra />{/each}
    <ActionsPanel />
  </aside>
  <main class="col center"><HexView /></main>
  <aside class="col right">
    <StringsPanel />
    <Inspector />
  </aside>
```

becomes

```svelte
  <aside class="col left">
    <DirTree />
    <MapPanel />
    <ActionsPanel />
  </aside>
  <main class="col center"><HexView /></main>
  <aside class="col right">
    {#each PANELS[ws.id].aside as Aside}<Aside />{/each}
    <StringsPanel />
    <Inspector />
  </aside>
```

(The Journal's CSS is column-agnostic: `.journal-controls select` is `flex: 1 1 100%` and the ring canvas follows its container's width. Under 1100 px `.right { grid-column: 1; }` stacks the right column under the left, Journal first.)

- [ ] **Step 7: The Format details format the tab's family, named in the summary**

In `web/ui/src/components/ActionsPanel.svelte`, the top of the script:

```svelte
  import { FAMILIES } from "../fs";
  import type { FsFamilyId } from "../fs/adapter";
  import { PANELS } from "../fs/panels";
  import { getWorkspace, workspaces } from "../state/workspace.svelte";

  const ws = getWorkspace();
  const { volume, selection } = ws;

  /** The family the Format details will format: the mounted one until the Filesystem select
   *  picks another, and back to the mounted one whenever that changes (a format, a load). */
  const mounted = $derived(volume.adapter.id);
  let family = $state<FsFamilyId>(volume.adapter.id);
  $effect(() => { family = mounted; });

  /** The chosen family's Format form, from the panel registry. */
  const FormatPanel = $derived(PANELS[family].format);
```

becomes

```svelte
  import { FAMILIES } from "../fs";
  import { PANELS } from "../fs/panels";
  import { getWorkspace, workspaces } from "../state/workspace.svelte";

  const ws = getWorkspace();
  const { volume, selection } = ws;

  /** The tab's Format form, from the panel registry: a tab formats its own family only. */
  const FormatPanel = PANELS[volume.family].format;
  /** The types that form makes: "FAT16", "ext2 / ext3". */
  const formatTypes = FAMILIES[volume.family].fsTypes.join(" / ");
```

and the Format details:

```svelte
    <details class="format">
      <summary>Format</summary>
      <label class="field">
        Filesystem
        <select id="format-family" bind:value={family}>
          {#each Object.values(FAMILIES) as f}<option value={f.id}>{f.name}</option>{/each}
        </select>
      </label>
      <FormatPanel />
    </details>
```

become

```svelte
    <details class="format">
      <summary>Format ({formatTypes})</summary>
      <FormatPanel />
    </details>
```

(`volume.family` is the `readonly` family the tab's `VolumeStore` was built for, so neither needs `$derived`.)

- [ ] **Step 8: The picker lists the tab's lessons and keeps each tab's pick**

Replace `web/ui/src/components/ScenarioPanel.svelte` with:

```svelte
<script module lang="ts">
  import { FAMILY_IDS } from "../fs";
  import type { FsFamilyId } from "../fs/adapter";
  import { defaultLessonFor, lessonsFor } from "../scenarios";

  /** Each tab's picked lesson, seeded with its fundamentals. Module state because App rebuilds
   *  the picker over the active workspace on every switch: coming back to a tab restores its
   *  choice. */
  const picked = $state<Record<FsFamilyId, string>>(
    Object.fromEntries(FAMILY_IDS.map((id) => [id, defaultLessonFor(id).id])) as Record<FsFamilyId, string>,
  );
</script>

<script lang="ts">
  import { getWorkspace } from "../state/workspace.svelte";

  const ws = getWorkspace();
  const { volume, scenarios } = ws;

  // The tab's own lessons, flat, in registry order; the first is its fundamentals.
  const lessons = lessonsFor(ws.id);

  // The picker only starts a lesson. Everything about the running lesson — the step text,
  // Prev/Next/Close, and the `n`/`p` keys — lives in LessonPanel, the floating Lesson card.
  function startScenario() {
    const s = lessons.find((s) => s.id === picked[ws.id]);
    if (!s) return;
    // The list holds the tab's lessons only, so the runner's refusal of another family's
    // lesson cannot fire from here; it still lands on the status line if it ever does.
    try {
      scenarios.start(s);
    } catch (e) {
      volume.report(e);
    }
  }
</script>

<div class="scenario-controls">
  <!-- The visible label names the select on its own; an `aria-label` would override it and
       leave the accessible name out of step with the words on screen. -->
  <label class="scenario-label" for="scenario-select">Learning scenarios</label>
  <select id="scenario-select" bind:value={picked[ws.id]}>
    {#each lessons as s (s.id)}<option value={s.id}>{s.title}</option>{/each}
  </select>
  <!-- LessonPanel returns focus here when the card closes, so the id is load-bearing. -->
  <button id="scenario-start" onclick={startScenario}>Start</button>
</div>
```

(`scenarioGroups` and `all` are no longer imported. Only one picker exists at a time, inside App's `{#key workspaces.active}` scope, so the id `scenario-select` stays page-unique.)

- [ ] **Step 9: `mkfs` uses `articleFor`, not its own copy of the rule**

In `web/ui/src/shell/mkfs.ts`, replace the first import line

```ts
import type { CommandDef } from "./types";
```

with:

```ts
import type { CommandDef } from "./types";
import { articleFor } from "../core/loadNotice";
```

delete the helper (and the blank line after it):

```ts
/** `a` or `an` before `word`, by its first letter (`an ext3`, `a FAT16`). */
const article = (word: string): string => (/^[aeiou]/i.test(word) ? "an" : "a");

```

and in `mkfsCommand` replace

```ts
        throw new ShellError(`'${type}' is ${article(owner.name)} ${owner.name} type: switch to the ${owner.name} tab to format one`, { help: typesHelp });
```

with:

```ts
        throw new ShellError(`'${type}' is ${articleFor(owner.name)} ${owner.name} type: switch to the ${owner.name} tab to format one`, { help: typesHelp });
```

and

```ts
          throw new ShellError(`--${long} is not ${article(type)} ${type} option`);
```

with:

```ts
          throw new ShellError(`--${long} is not ${articleFor(type)} ${type} option`);
```

(`src/core/loadNotice.ts` is pure, so the node tests that load `mkfs.ts` are unaffected.)

Run: `pnpm exec vitest run tests/shell tests/loadNotice.test.ts`
Expected: `Test Files  13 passed (13)`, `Tests  190 passed (190)` (the `mkfs` refusals still read `'ext3' is an ext type: …`, `'fat16' is a FAT16 type: …`, and `--spc is not an ext3 option`).

- [ ] **Step 10: Run the gates**

Run: `git grep -n "scenarioGroups\|ScenarioGroup\|format-family\|\.extras\|extras:" -- web/ui/src web/ui/tests`
Expected: no output.

Run: `pnpm test`
Expected: `Test Files  54 passed (54)`, `Tests  535 passed (535)`.

Run: `pnpm build`
Expected: svelte-check `COMPLETED 464 FILES 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS`, then `✓ built in …`.

`git status --short` shows only the fourteen files above (never `pnpm-lock.yaml` or `pnpm-workspace.yaml`).

- [ ] **Step 11: Check it in the browser (1440 × 900)**

From `web/ui`: `pnpm exec vite --port 5195 --strictPort` (Bash, background; any free port), then open `http://localhost:5195/` with `preview_start` by URL and set the viewport to 1440 × 900.

Expected (as observed):
- FAT16 tab: the picker lists nine lessons (`The fundamentals` … `Work from the shell`) with `The fundamentals` selected; the Format details read `Format (FAT16)` with the FAT form (Size, Sectors per cluster, Volume label) and no Filesystem select; the right column is Strings, then the Inspector (`At this byte`).
- Click `ext`: three lessons (`The fundamentals`, `A journaled write`, `Crash and recover`) with `The fundamentals` selected; `Format (ext2 / ext3)` with the Variant select first; the right column is `Journal · ordered mode`, Strings, Inspector; the left column is Files, `Block groups · 2 groups · 16,384 blocks`, Actions, with the Actions heading at about y = 700, above the fold.
- Pick `A journaled write` on ext, switch to FAT16 (it shows `The fundamentals`), pick `Fill the disk`, then switch ext → FAT16 → ext: each tab shows its own pick every time.
- On ext, pick `Crash and recover`, Start, and Next through to step 7 of 7 (`Bytes without an owner`; the card shows Prev, Finish, Close); the ext tab carries the `lesson` badge; no console errors.
- At 1100 × 900 the right column sits in the first grid column under the left one (x = 8), Journal first, then Strings and the Inspector.

Reset the viewport, stop the dev server, and close the browser tab.

- [ ] **Step 12: Commit**

```sh
git add web/ui/src/components/ActionsPanel.svelte web/ui/src/components/ScenarioPanel.svelte web/ui/src/components/WorkspaceView.svelte web/ui/src/fs/adapter.ts web/ui/src/fs/ext/adapter.ts web/ui/src/fs/fat16/adapter.ts web/ui/src/fs/panels.ts web/ui/src/scenarios/extFundamentals.ts web/ui/src/scenarios/index.ts web/ui/src/shell/mkfs.ts web/ui/tests/ext-fundamentals.test.ts web/ui/tests/fs/ext.test.ts web/ui/tests/fs/fat16.test.ts web/ui/tests/scenarios.test.ts
git commit -m "feat(ui): each tab lists its own lessons, formats its own family, and keeps the Journal on the right" -m "lessonsFor/defaultLessonFor replace the picker's optgroups; the picker keeps each tab's pick. The Format details lose the Filesystem select and read Format (FAT16) / Format (ext2 / ext3). PANELS[id].extras becomes aside, rendered at the top of the right column, so ext's Journal no longer pushes Actions below the fold. FsAdapter gains summary(), the mounted disk in one line. mkfs uses loadNotice's articleFor instead of its own copy of the a/an rule."
```


### Task 6: The Operation bar and the What changed panel

Spec: `docs/superpowers/specs/2026-09-25-family-tabs-design.md` section 6 (`OperationBar.svelte` replacing `StepPanel.svelte` and `Timeline.svelte`; `WhatChangedPanel.svelte` with the ranges, the phases, and the hover outlines; `core/ranges.ts`, `core/eventPhases.ts`, `core/timelineSlider.ts`), section 2 (`LayersStore.hover`), section 3 (`WorkspaceView` holds the Operation bar, the Ribbon, and the grid with What changed first in the right column, no footer; `.app { grid-template-rows: auto auto auto minmax(0, 1fr); }`; the `.step`/`.timeline` rules go; the per-tab id `opbar-heading-{id}`), section 4 (`summary()` is the bar's empty state), section 8 (`tests/ranges.test.ts`, `tests/eventPhases.test.ts`, `tests/timelineSlider.test.ts`, the `tests/layout.test.ts` `.opbar` wrap guard). Base: `plan/tabs-t5` (Task 5, 4247b75), which gives `FsAdapter.summary()` and puts `PANELS[id].aside` at the top of the right column. This task also closes the gap recorded after Task 2: `Ribbon.svelte` measures its width with `observeWidth` (whose 0-width guard keeps a hidden tab's ribbon at its last width) instead of its own `ResizeObserver`.

**Files**

- Create: `web/ui/src/core/ranges.ts` (1-17)
- Create: `web/ui/src/core/eventPhases.ts` (1-84)
- Create: `web/ui/src/core/timelineSlider.ts` (1-10)
- Create: `web/ui/src/components/OperationBar.svelte` (1-121)
- Create: `web/ui/src/components/WhatChangedPanel.svelte` (1-89)
- Create (test): `web/ui/tests/ranges.test.ts` (1-40)
- Create (test): `web/ui/tests/eventPhases.test.ts` (1-136)
- Create (test): `web/ui/tests/timelineSlider.test.ts` (1-28)
- Delete: `web/ui/src/components/StepPanel.svelte`, `web/ui/src/components/Timeline.svelte` (no test referenced either)
- Modify: `web/ui/src/state/layers.svelte.ts` (11-13)
- Modify: `web/ui/src/components/WorkspaceView.svelte` (8-11, 22-23, 27-31, 41; the footer line removed after 45)
- Modify: `web/ui/src/components/Ribbon.svelte` (5, 7, 30-33, 39, 58-66, 70, 160, 176-185, 271)
- Modify: `web/ui/src/fs/fat16/FatMap.svelte` (4, 32, 46-50, 71-72, 89-92)
- Modify: `web/ui/src/fs/ext/BlockGroupMap.svelte` (49, 140-148)
- Modify: `web/ui/src/app.css` (24, 30-31, 79, 179-197)
- Modify (doc comment only): `web/ui/src/state/workspace.svelte.ts` (48)
- Modify (test): `web/ui/tests/layout.test.ts` (25-30, 33, 75-86)

**Interfaces**

Consumes (Task 1): `getWorkspace()`, `Workspace.{id, active, goToStep, volume, selection, layers}` (`src/state/workspace.svelte.ts`); `LayersStore` (`src/state/layers.svelte.ts`). (Task 2): `observeWidth` with its 0-width guard (`src/core/observeWidth.ts`); `.sr-only` in `app.css`; the `ws.active` stop-on-hide that `Timeline.svelte` had. (Task 5): `FsAdapter.summary()`; `PANELS[ws.id].aside` rendered first in `.col.right`. Existing: `changedSectors` (`src/core/patch.ts`), `Interval` (`src/core/intervals.ts`), `outlineCell` (`src/core/grid.ts`), `blocksTouched`, `cellOf` (`src/fs/ext/blockMap.ts`), `volume.{history, cursor, atLatest, epoch, sectorSize, adapter.sector}`, `selection.jumpTo`, wasm's `OpRecord.events` (`EventRecord { kind: string; text: string; region: Range | null }`, the region in bytes), and in the tests `Volume.{formatFat16, formatExt2, formatExt3, armCrash, recover, writeRaw}`.

Produces:

```ts
// src/core/ranges.ts
export interface SectorRange { label: string; first: number; last: number }
export function formatRanges(sorted: readonly number[]): SectorRange[];   // "1–6" (en dash), "69", "82–91"
// src/core/eventPhases.ts
export type PhaseId = "directory" | "allocation" | "data" | "body" | "journal" | "checkpoint" | "cleanup" | "recovery" | "crash" | "other";
export interface PhaseEvent { kind: string; text: string; region: Interval | null }
export interface Phase { id: PhaseId; label: string; events: (PhaseEvent & { index: number })[] }
export const PHASE_LABELS: Record<PhaseId, string>;
export function groupEvents(events: readonly PhaseEvent[]): Phase[];
  // any journal kind -> ext3-style (journal kinds to journal/checkpoint/cleanup/recovery/crash, every other kind to body);
  // else FAT-style (directory/allocation/data; a kind the table does not know to other); phases in order of first event
// src/core/timelineSlider.ts
export function stepAtPointer(x: number, width: number, steps: number): number;   // nearest tick, clamped; 0 when steps <= 1 or width <= 0
// src/state/layers.svelte.ts
class LayersStore { hover: Interval | null }   // $state.raw, null
// src/components/OperationBar.svelte: no props; <section class="panel opbar" aria-labelledby="opbar-heading-{ws.id}">,
//   <h2 id="opbar-heading-{ws.id}" class="sr-only">Operation</h2>, Prev/Play|Pause/Next, <input class="tl-range" list="opbar-ticks-{ws.id}">,
//   <datalist id="opbar-ticks-{ws.id}">, <p class="opbar-summary">
// src/components/WhatChangedPanel.svelte: no props; <section class="panel changed"><h2>What changed</h2>, .ranges buttons, <details class="phase">
// WorkspaceView.svelte: <div class="opbar-slot"><OperationBar /></div> where .step-slot was; <WhatChangedPanel /> first in .col.right; no footer
// DOM ids: opbar-heading-{id}, opbar-ticks-{id} (step-heading-{id} is gone)
```

Two decisions the spec leaves open, pinned by the tests below. The real wasm emits three kinds the spec's FAT-style lists do not name: FAT's delete writes `dir_entry_deleted` (the spec lists ext's `dir_entry_removed`), a FAT subdirectory growing a cluster writes `directory_grown`, and any family's raw write is `raw_write`; they are grouped as Directory entry, Directory entry, and Data rather than Other. In an ext3-style record, "every non-journal kind → Filesystem writes" is read literally, so a kind the table does not know lands there, and Other appears only in FAT-style records. The slot wrapper is the class `opbar-slot` (the contract's `#opbar-slot`), since Task 2 made every slot a class so each tab has its own. The op names in the bar are wasm's (`create_file /hello.txt`), not the spec example's `createFile`. The bar keeps Prev/Play/Next and the slider in its empty state, disabled, so its height does not change on the first action. Two lesson texts (`fundamentals.ts` line 94, `journaledWrite.ts` line 40) still name the Step strip until Task 7 rewrites them, which Global Constraint 1 sanctions.

All commands below run in `web/ui` (a fresh worktree first needs, from the repo root, `wasm-pack build crates/wasm --target bundler`, then in `web/ui` `CI=true pnpm install --frozen-lockfile`). Baseline on `plan/tabs-t5`: `pnpm test` gives `Test Files  54 passed (54)`, `Tests  537 passed (537)`.


- [ ] **Step 1: Write the failing tests for the three pure modules**

Create `web/ui/tests/ranges.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { formatRanges } from "../src/core/ranges";
import { changedSectors } from "../src/core/patch";
import { Volume } from "../src/lib/wasm";

const enc = (s: string) => new TextEncoder().encode(s);

describe("formatRanges", () => {
  it("folds runs of consecutive numbers into one range, a lone number into itself", () => {
    expect(formatRanges([1, 2, 3, 4, 5, 6, 69, 82, 83, 84, 85, 86, 87, 88, 89, 90, 91])).toEqual([
      { label: "1–6", first: 1, last: 6 },
      { label: "69", first: 69, last: 69 },
      { label: "82–91", first: 82, last: 91 },
    ]);
    // The dash is an en dash, not a hyphen.
    expect(formatRanges([7, 8])[0].label).toBe("7–8");
  });

  it("gives nothing for nothing, and sector 0 its own label", () => {
    expect(formatRanges([])).toEqual([]);
    expect(formatRanges([0])).toEqual([{ label: "0", first: 0, last: 0 }]);
    expect(formatRanges([0, 2])).toEqual([{ label: "0", first: 0, last: 0 }, { label: "2", first: 2, last: 2 }]);
  });

  it("reads a real ext3 create as the What changed panel shows it", () => {
    // The default ext3 disk's Add file: the superblock and group descriptors, the bitmaps and
    // inode table (1–6), the root directory (69), the journal (82–91), and the data block.
    const vol = Volume.formatExt3(undefined);
    const rec = vol.createFile("/hello.txt", enc("Hello world\n"));
    const sectors = changedSectors(rec.changes, vol.sectorSize());
    expect(sectors).toHaveLength(18);
    expect(formatRanges(sectors).map((r) => r.label)).toEqual(["1–6", "69", "82–91", "1111"]);
  });

  it("reads a real FAT16 create: both FATs, the root directory, the data cluster", () => {
    const vol = Volume.formatFat16(undefined);
    const rec = vol.createFile("/Hello world.txt", enc("Hello, world!\n"));
    expect(formatRanges(changedSectors(rec.changes, vol.sectorSize())).map((r) => r.label)).toEqual(["1", "33", "65", "97–100"]);
  });
});
```

Create `web/ui/tests/timelineSlider.test.ts`:

```ts
import { describe, expect, it } from "vitest";
import { stepAtPointer } from "../src/core/timelineSlider";

describe("stepAtPointer", () => {
  it("picks the nearest of the ticks spread across the width", () => {
    // Seven steps over 600 px: a tick every 100 px, at 0, 100, …, 600.
    expect(stepAtPointer(0, 600, 7)).toBe(0);
    expect(stepAtPointer(49, 600, 7)).toBe(0);
    expect(stepAtPointer(51, 600, 7)).toBe(1);
    expect(stepAtPointer(149, 600, 7)).toBe(1);
    expect(stepAtPointer(151, 600, 7)).toBe(2);
    expect(stepAtPointer(600, 600, 7)).toBe(6);
    // Two steps: the halfway point is where the second takes over.
    expect(stepAtPointer(99, 200, 2)).toBe(0);
    expect(stepAtPointer(101, 200, 2)).toBe(1);
  });

  it("clamps a pointer past either end of the slider to the first or last step", () => {
    expect(stepAtPointer(-20, 600, 7)).toBe(0);
    expect(stepAtPointer(640, 600, 7)).toBe(6);
  });

  it("is step 0 with one step or none, or before the slider has a width", () => {
    expect(stepAtPointer(300, 600, 1)).toBe(0);
    expect(stepAtPointer(300, 600, 0)).toBe(0);
    expect(stepAtPointer(10, 0, 7)).toBe(0);
  });
});
```

Create `web/ui/tests/eventPhases.test.ts` (it runs the real wasm: a FAT16 create, delete, raw write, and grown directory; an ext3 create; an ext2 create; a crash armed after the commit; `recover()` after a crash after and before the commit and on a clean journal):

```ts
import { describe, expect, it } from "vitest";
import { PHASE_LABELS, groupEvents, type PhaseEvent } from "../src/core/eventPhases";
import { Volume } from "../src/lib/wasm";

const enc = (s: string) => new TextEncoder().encode(s);
const HELLO = enc("Hello world\n");
/** Each phase as `[id, label, how many events]`, in the order the panel lists them. */
const phases = (events: readonly PhaseEvent[]) => groupEvents(events).map((p) => [p.id, p.label, p.events.length]);
/** Events of the given kinds, with no text and no region. */
const kinds = (...list: string[]): PhaseEvent[] => list.map((kind) => ({ kind, text: "", region: null }));

describe("PHASE_LABELS", () => {
  it("names every phase the way the What changed panel's summaries read", () => {
    expect(PHASE_LABELS).toEqual({
      directory: "Directory entry",
      allocation: "Allocation",
      data: "Data",
      body: "Filesystem writes",
      journal: "Journal",
      checkpoint: "Checkpoint",
      cleanup: "Cleanup",
      recovery: "Recovery",
      crash: "Crash",
      other: "Other",
    });
  });
});

describe("groupEvents on real records", () => {
  it("groups a FAT16 create FAT-style: the table, the data, then the directory entry", () => {
    const rec = Volume.formatFat16(undefined).createFile("/hello.txt", HELLO);
    // Phases follow the first event of each: the FAT entries come before the data and the slot.
    expect(phases(rec.events)).toEqual([
      ["allocation", "Allocation", 3],
      ["data", "Data", 1],
      ["directory", "Directory entry", 1],
    ]);
    // Every event keeps its place in the record, so the panel can key and order by it.
    expect(groupEvents(rec.events).flatMap((p) => p.events.map((e) => e.index))).toEqual([0, 1, 2, 3, 4]);
    expect(groupEvents(rec.events)[0].events[0]).toEqual({ ...rec.events[0], index: 0 });
  });

  it("puts a FAT16 delete's slot, a raw write, and a grown directory where they belong", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createFile("/a.txt", enc("x"));
    expect(phases(vol.deleteFile("/a.txt").events)).toEqual([["directory", "Directory entry", 1], ["allocation", "Allocation", 3]]);
    expect(phases(vol.writeRaw(100, enc("ab")).events)).toEqual([["data", "Data", 1]]);
    // A subdirectory's first cluster holds 64 slots, `.` and `..` included: the 63rd file grows it.
    vol.createDir("/d");
    for (let i = 0; i < 62; i++) vol.createFile(`/d/f${i}.txt`, enc("x"));
    const grown = vol.createFile("/d/f62.txt", enc("x"));
    expect(grown.events.some((e) => e.kind === "directory_grown")).toBe(true);
    expect(phases(grown.events)).toEqual([["allocation", "Allocation", 8], ["data", "Data", 1], ["directory", "Directory entry", 2]]);
  });

  it("groups an ext3 create ext3-style: the writes, the journal, the checkpoint, the cleanup", () => {
    const rec = Volume.formatExt3(undefined).createFile("/hello.txt", HELLO);
    expect(rec.events).toHaveLength(30);
    expect(phases(rec.events)).toEqual([
      ["body", "Filesystem writes", 10],
      ["journal", "Journal", 11],
      ["checkpoint", "Checkpoint", 7],
      ["cleanup", "Cleanup", 2],
    ]);
    // The Journal phase: the flag, the transaction, then its descriptor, seven copies, and the commit.
    const journal = groupEvents(rec.events)[1].events;
    expect(journal.slice(0, 2).map((e) => e.kind)).toEqual(["recovery_flag_set", "transaction_started"]);
    const texts = journal.slice(2).map((e) => e.text);
    expect(texts.filter((t) => t.startsWith("wrote descriptor"))).toHaveLength(1);
    expect(texts.filter((t) => t.startsWith("copied block"))).toHaveLength(7);
    expect(texts.filter((t) => t.startsWith("committed transaction"))).toHaveLength(1);
    expect(texts).toHaveLength(9);
  });

  it("groups an ext2 create FAT-style: no journal, so it reads like FAT's", () => {
    const rec = Volume.formatExt2(undefined).createFile("/hello.txt", HELLO);
    expect(phases(rec.events)).toEqual([
      ["allocation", "Allocation", 8],
      ["data", "Data", 1],
      ["directory", "Directory entry", 1],
    ]);
  });

  it("ends a crashed record with a Crash phase", () => {
    const vol = Volume.formatExt3(undefined);
    vol.armCrash("after_commit");
    const rec = vol.createFile("/c.txt", HELLO);
    expect(phases(rec.events)).toEqual([
      ["body", "Filesystem writes", 10],
      ["journal", "Journal", 11],
      ["crash", "Crash", 1],
    ]);
    expect(groupEvents(rec.events).at(-1)!.events[0].text).toBe("crashed after commit");
  });

  it("reads recover() as Recovery then Cleanup, and a discarded transaction as Recovery too", () => {
    const vol = Volume.formatExt3(undefined);
    vol.armCrash("after_commit");
    vol.createFile("/c.txt", HELLO);
    // The scan, then seven blocks replayed; the journal emptied and the flag cleared.
    expect(phases(vol.recover().events)).toEqual([["recovery", "Recovery", 8], ["cleanup", "Cleanup", 2]]);

    const before = Volume.formatExt3(undefined);
    before.armCrash("before_commit");
    before.createFile("/c.txt", HELLO);
    expect(phases(before.recover().events)).toEqual([["recovery", "Recovery", 2], ["cleanup", "Cleanup", 2]]);
    // A clean journal: one scan, nothing to clean up.
    expect(phases(Volume.formatExt3(undefined).recover().events)).toEqual([["recovery", "Recovery", 1]]);
  });
});

describe("groupEvents rules", () => {
  it("lists phases in the order of their first event, each phase's events in record order", () => {
    const grouped = groupEvents(kinds("data_written", "fat_entry_set", "data_written", "dir_entry_written"));
    expect(grouped.map((p) => p.id)).toEqual(["data", "allocation", "directory"]);
    expect(grouped[0].events.map((e) => e.index)).toEqual([0, 2]);
  });

  it("puts a kind it does not know in Other on a record without a journal", () => {
    expect(phases(kinds("dir_entry_written", "mystery", "data_written", "mystery"))).toEqual([
      ["directory", "Directory entry", 1],
      ["other", "Other", 2],
      ["data", "Data", 1],
    ]);
  });

  it("counts every non-journal kind as a filesystem write once any journal kind is present", () => {
    expect(phases(kinds("mystery", "dir_entry_written", "checkpointed"))).toEqual([["body", "Filesystem writes", 2], ["checkpoint", "Checkpoint", 1]]);
    // Any one journal kind is enough to switch the whole record to the ext3 grouping.
    expect(phases(kinds("data_written", "crashed"))).toEqual([["body", "Filesystem writes", 1], ["crash", "Crash", 1]]);
  });

  it("gives nothing for a record with no events", () => {
    expect(groupEvents([])).toEqual([]);
  });
});
```

Run: `pnpm exec vitest run tests/ranges.test.ts tests/eventPhases.test.ts tests/timelineSlider.test.ts`
Expected: FAIL, `Error: Cannot find module '../src/core/ranges' imported from '…/tests/ranges.test.ts'` then `Caused by: Error: Failed to load url ../src/core/ranges (resolved id: ../src/core/ranges) in …/tests/ranges.test.ts. Does the file exist?`, and the same for `eventPhases` and `timelineSlider`; `Test Files  3 failed (3)`, `Tests  no tests`.

- [ ] **Step 2: Implement `ranges.ts`, `timelineSlider.ts`, and `eventPhases.ts`**

Create `web/ui/src/core/ranges.ts`:

```ts
/** A run of consecutive sectors (blocks on ext) as the What changed panel labels it. */
export interface SectorRange { label: string; first: number; last: number }

/**
 * Fold sorted, de-duplicated sector numbers (`changedSectors`'s output) into runs of consecutive
 * numbers: `[1, 2, 3, 4, 5, 6, 69, 82, …, 91]` becomes `1–6`, `69`, `82–91`, with an en dash.
 * An ext3 create touches 18 blocks in four runs, so four buttons stand in for eighteen.
 */
export function formatRanges(sorted: readonly number[]): SectorRange[] {
  const runs: { first: number; last: number }[] = [];
  for (const n of sorted) {
    const run = runs[runs.length - 1];
    if (run && n <= run.last + 1) run.last = Math.max(run.last, n);
    else runs.push({ first: n, last: n });
  }
  return runs.map(({ first, last }) => ({ label: first === last ? `${first}` : `${first}–${last}`, first, last }));
}
```

Create `web/ui/src/core/timelineSlider.ts`:

```ts
/**
 * The step under the pointer on the Operation bar's slider: `steps` ticks spread evenly across
 * `width` pixels (the first at 0, the last at `width`), and `x` picks the nearest, clamped to the
 * ends. With one step or none, or before the slider has a width, it is step 0.
 */
export function stepAtPointer(x: number, width: number, steps: number): number {
  if (steps <= 1 || width <= 0) return 0;
  const step = Math.round((x / width) * (steps - 1));
  return Math.max(0, Math.min(steps - 1, step));
}
```

Create `web/ui/src/core/eventPhases.ts`:

```ts
import type { Interval } from "./intervals";

/** The phases the What changed panel groups a record's events into. */
export type PhaseId = "directory" | "allocation" | "data" | "body" | "journal" | "checkpoint" | "cleanup" | "recovery" | "crash" | "other";

/** One event of an operation record: wasm's `EventRecord`, its region in bytes. */
export interface PhaseEvent { kind: string; text: string; region: Interval | null }

/** A phase and its events in record order, each with its index in the record. */
export interface Phase { id: PhaseId; label: string; events: (PhaseEvent & { index: number })[] }

/** Each phase's name, the start of its `<summary>`: "Journal · 9 events". */
export const PHASE_LABELS: Record<PhaseId, string> = {
  directory: "Directory entry",
  allocation: "Allocation",
  data: "Data",
  body: "Filesystem writes",
  journal: "Journal",
  checkpoint: "Checkpoint",
  cleanup: "Cleanup",
  recovery: "Recovery",
  crash: "Crash",
  other: "Other",
};

/** The journal's kinds (ext3) and their phases. A record with any of them is grouped ext3-style. */
const JOURNAL_PHASES: Readonly<Record<string, PhaseId>> = {
  recovery_flag_set: "journal",
  transaction_started: "journal",
  journal_block_written: "journal",
  checkpointed: "checkpoint",
  journal_emptied: "cleanup",
  recovery_flag_cleared: "cleanup",
  recovery_scanned: "recovery",
  replayed: "recovery",
  transaction_discarded: "recovery",
  crashed: "crash",
};

/** The filesystem's own kinds (FAT16, ext2, and ext3's writes before it journals them), grouped
 *  FAT-style. `dir_entry_deleted` and `directory_grown` are FAT's, `raw_write` any family's. */
const FS_PHASES: Readonly<Record<string, PhaseId>> = {
  dir_entry_written: "directory",
  dir_entry_removed: "directory",
  dir_entry_deleted: "directory",
  directory_grown: "directory",
  fat_entry_set: "allocation",
  cluster_allocated: "allocation",
  cluster_freed: "allocation",
  blocks_allocated: "allocation",
  blocks_freed: "allocation",
  bitmap_updated: "allocation",
  counters_updated: "allocation",
  inode_allocated: "allocation",
  inode_freed: "allocation",
  inode_written: "allocation",
  indirect_written: "allocation",
  data_written: "data",
  raw_write: "data",
};

/**
 * Group an operation's events into phases, listed in the order of each phase's first event.
 * A record with any journal kind is grouped ext3-style: every other kind is one "Filesystem
 * writes" phase (the writes the transaction then journals), and the journal's kinds are
 * Journal, Checkpoint, Cleanup, Recovery, or Crash. A record without one (FAT16, ext2, a raw
 * write) is grouped FAT-style: Directory entry, Allocation, Data, and Other for a kind this
 * table does not know.
 */
export function groupEvents(events: readonly PhaseEvent[]): Phase[] {
  const journaled = events.some((e) => e.kind in JOURNAL_PHASES);
  const phaseOf = (kind: string): PhaseId => {
    if (journaled) return JOURNAL_PHASES[kind] ?? "body";
    return FS_PHASES[kind] ?? "other";
  };
  const byId = new Map<PhaseId, Phase>();
  events.forEach((e, index) => {
    const id = phaseOf(e.kind);
    let phase = byId.get(id);
    if (!phase) byId.set(id, (phase = { id, label: PHASE_LABELS[id], events: [] }));
    phase.events.push({ ...e, index });
  });
  return [...byId.values()];
}
```

Run: `pnpm exec vitest run tests/ranges.test.ts tests/eventPhases.test.ts tests/timelineSlider.test.ts`
Expected: `Test Files  3 passed (3)`, `Tests  18 passed (18)`.

- [ ] **Step 3: Write the failing layout guards: four app rows, the `.opbar` wrap**

In `web/ui/tests/layout.test.ts`, after the test `it("bounds the app column so children with pixel widths cannot push the page sideways", …)` (which ends `expect(rule(".app")).toContain("grid-template-columns: minmax(0, 1fr)");` / `  });`) insert:

```ts

  it("gives the app four rows: top bar, Operation bar, ribbon, and the grid, which takes the rest", () => {
    // The Timeline footer is gone, so a fifth template row would leave an empty 8 px gap under the
    // grid and push the terminal drawer (the implicit row after these) down by it.
    expect(rule(".app")).toContain("grid-template-rows: auto auto auto minmax(0, 1fr);");
  });
```

In the next test's comment,

```ts
    // Each tab's body sits in a `.workspace` wrapper; as a box it would be one grid item and the
    // step strip, ribbon, grid, and footer would lose their rows. `:not([hidden])`, because an
```

becomes

```ts
    // Each tab's body sits in a `.workspace` wrapper; as a box it would be one grid item and the
    // Operation bar, ribbon, and grid would lose their rows. `:not([hidden])`, because an
```

and the Step strip's guard

```ts
  it("caps the Step strip's height and scrolls it, so a long step cannot push the grid down", () => {
    // An ext3 Add file lists 18 block buttons and 30 events; uncapped, the strip grew to 571 px in a
    // 760 px window. 210 px is about six rows of buttons, more than the FAT actions fill.
    const step = rule(".step");
    expect(step).toContain("max-height: 210px");
    expect(step).toContain("overflow: auto");
  });
```

becomes

```ts
  it("keeps the Operation bar to one wrapping row, with the summary on a line of its own under 760 px", () => {
    // The bar holds only the controls and a one-line summary (the blocks and events moved to the
    // What changed panel), so it needs no height cap; it wraps rather than pushing the page
    // sideways, and the slider takes the slack but never shrinks below a usable 120 px.
    expect(rule(".opbar")).toBe(" display: flex; flex-wrap: wrap; align-items: center; gap: 4px 12px; padding: 5px 10px; ");
    expect(rule(".opbar .tl-range")).toBe(" flex: 1 1 160px; min-width: 120px; ");
    expect(media("max-width: 760px")).toContain(".opbar-summary { flex-basis: 100%; }");
    // The Step strip's and the Timeline footer's rules went with them.
    expect(rule(".step")).toBeNull();
    expect(rule(".timeline")).toBeNull();
    expect(css).not.toContain(".tl-step");
  });
```

Run: `pnpm exec vitest run tests/layout.test.ts`
Expected: FAIL, `gives the app four rows…` (`expected '…auto auto auto minmax(0, 1fr) auto…' to contain 'grid-template-rows: auto auto auto minmax(0, 1fr);'`) and `keeps the Operation bar to one wrapping row…` (`expected null to be ' display: flex; …'`); `Tests  2 failed | 12 passed (14)`.

- [ ] **Step 4: `app.css`: four rows, the `.opbar` and `.changed` rules replace `.step` and `.timeline`**

In `web/ui/src/app.css`, line 24:

```css
.app { display: grid; grid-template-columns: minmax(0, 1fr); grid-template-rows: auto auto auto minmax(0, 1fr) auto; grid-auto-rows: var(--term-h, 220px); height: 100vh; gap: 8px; padding: 8px; }
```

becomes

```css
.app { display: grid; grid-template-columns: minmax(0, 1fr); grid-template-rows: auto auto auto minmax(0, 1fr); grid-auto-rows: var(--term-h, 220px); height: 100vh; gap: 8px; padding: 8px; }
```

(The terminal drawer is now really the implicit fifth row the comment above `.app` describes.) Lines 30-31:

```css
/* A tab's body (App.svelte's `.workspace` wrapper) adds no box: its step strip, ribbon, grid,
   and footer are the app grid's rows. `:not([hidden])`, since an author `display` would
```

become

```css
/* A tab's body (App.svelte's `.workspace` wrapper) adds no box: its Operation bar, ribbon, and
   grid are the app grid's rows. `:not([hidden])`, since an author `display` would
```

In `@media (max-width: 760px)`, after `  .topbar > .status { order: 4; }` add:

```css
  .opbar-summary { flex-basis: 100%; }
```

and replace everything from `/* Step panel */` up to (not including) `/* Strings panel */`, the blank line before it included:

```css
/* Step panel */
/* Step strip (under the top bar): one wrapping row of heading, counter, op, sector buttons,
   and events. It scrolls past about six rows of buttons (210 px), which the FAT actions never
   reach; an ext3 Add file's 18 block buttons and 30 events would push the grid down. */
.step { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 14px; padding: 5px 10px; max-height: 210px; overflow: auto; }
.step > h2 { margin: 0; font: 500 13px var(--font-display); }
.step p { margin: 0; }
.step .counter { color: var(--ink-muted); }
.step .btn-row { display: flex; flex-wrap: wrap; gap: 6px; }
.step .events { list-style: none; padding: 0; margin: 0; display: flex; flex-wrap: wrap; gap: 2px 14px; font-size: 12px; }
.step .events .link { text-align: left; }

/* Timeline */
.timeline { padding: 4px 2px; }
.tl-controls { display: flex; align-items: center; gap: 6px; }
.tl-range { flex: 1; }
.tl-steps { display: flex; margin-top: 4px; }
.tl-step { flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; font-size: 10px; text-align: center; border-radius: 0; border-width: 1px 0 0 1px; }
.tl-step:last-child { border-right-width: 1px; }
.tl-step.current { background: color-mix(in srgb, var(--focus) 15%, transparent); border-color: var(--focus); }

```

with (ending in a blank line too, so `/* Strings panel */` keeps the blank line above it)

```css
/* Operation bar (under the top bar): Prev, Play, Next, the slider, then the step in one line
   (`3 of 7 · create_file /hello.txt · 18 blocks · 30 events`). The blocks and events themselves
   are the What changed panel's, so the bar never grows past two lines. */
.opbar { display: flex; flex-wrap: wrap; align-items: center; gap: 4px 12px; padding: 5px 10px; }
.opbar-controls { display: flex; align-items: center; gap: 6px; }
.opbar .tl-range { flex: 1 1 160px; min-width: 120px; }
.opbar-summary { margin: 0; min-width: 0; overflow-wrap: anywhere; }
.opbar-summary .counter { color: var(--ink-muted); }

/* What changed panel (top of the right column): the step's changed blocks as ranges, then its
   events grouped into phases, each a disclosure. */
.changed p { margin: 0; }
.changed .changed-label { margin-bottom: 4px; font-size: 12px; }
.changed .ranges { display: flex; flex-wrap: wrap; gap: 4px; margin-bottom: 8px; }
.changed .ranges button { padding: 1px 6px; }
.changed .phase { border-top: 1px solid var(--hairline); padding: 4px 0; }
.changed .phase summary { cursor: pointer; font: 500 12px var(--font-display); }
.changed .phase ol { list-style: none; padding: 0; margin: 4px 0 2px; display: flex; flex-direction: column; gap: 2px; font-size: 12px; }
.changed .phase .link { text-align: left; }
.changed .warn { color: var(--warn); }

```

Run: `pnpm exec vitest run tests/layout.test.ts`
Expected: `Test Files  1 passed (1)`, `Tests  14 passed (14)`.

- [ ] **Step 5: `LayersStore.hover`**

In `web/ui/src/state/layers.svelte.ts`, after `  visible = $state.raw<Interval>({ start: 0, end: 0 });` add:

```ts
  /** The bytes under the pointer or focus in the What changed panel (a range of blocks, an
   *  event's region): the ribbon and the map outline them in `--focus`. Null when none. */
  hover = $state.raw<Interval | null>(null);
```

- [ ] **Step 6: The Operation bar**

Create `web/ui/src/components/OperationBar.svelte` (Timeline's playback, `goTo`, and stop-on-hide, moved over unchanged; its step buttons and `truncate` go, the slider's ticks and pointer tooltip replace them; StepPanel's counter and op become the one-line summary):

```svelte
<script lang="ts">
  import { onDestroy } from "svelte";
  import { changedSectors } from "../core/patch";
  import { stepAtPointer } from "../core/timelineSlider";
  import { getWorkspace } from "../state/workspace.svelte";

  const ws = getWorkspace();
  const { volume } = ws;

  const PLAY_MS = 700;
  const PLAY_MS_REDUCED = 1500;

  let playing = $state(false);
  let timer: ReturnType<typeof setInterval> | null = null;
  /** The slider's tooltip for the step under the pointer; null until the pointer moves over it. */
  let pointerTitle = $state<string | null>(null);

  const rec = $derived(volume.history[volume.cursor]);
  const steps = $derived(volume.history.length);
  const sectorCount = $derived(rec ? changedSectors(rec.changes, volume.sectorSize).length : 0);
  // The family's noun for a sector: "sector"/"sectors" on FAT, "block"/"blocks" on ext.
  const noun = $derived.by(() => { volume.epoch; return volume.adapter.sector; });
  // The empty state names the mounted disk: "FAT16 · 16 MB · 512-byte sectors · 2 KiB clusters".
  const summary = $derived.by(() => { volume.epoch; return volume.adapter.summary(); });

  /** "3 of 7 · create_file /hello.txt": the slider's tooltip for step `i`. */
  function stepTitle(i: number): string {
    return `${i + 1} of ${steps} · ${volume.history[i].op}`;
  }

  // Every explicit navigation in this bar goes through here: seek, then recenter the dump on
  // what that step changed. There is deliberately no effect on `volume.cursor` doing this —
  // running a command (from Actions or the terminal) moves the cursor too, and an operation
  // must never yank the dump away from what someone was reading. It highlights its changes in
  // place instead; the dump moves only for a step someone picked here, for the `[`/`]`
  // shortcuts, or for a click on the ribbon, the tree, or the What changed panel.
  function goTo(step: number) {
    ws.goToStep(step);
  }

  function reducedMotion(): boolean {
    return typeof matchMedia !== "undefined" && matchMedia("(prefers-reduced-motion: reduce)").matches;
  }

  function stop() {
    playing = false;
    if (timer !== null) { clearInterval(timer); timer = null; }
  }

  function tick() {
    if (volume.atLatest) { stop(); return; }
    goTo(volume.cursor + 1);
    if (volume.atLatest) stop();
  }

  function play() {
    if (volume.atLatest) return;
    playing = true;
    timer = setInterval(tick, reducedMotion() ? PLAY_MS_REDUCED : PLAY_MS);
  }

  function togglePlay() {
    if (playing) stop(); else play();
  }

  onDestroy(stop);
  // Playback belongs to the tab on screen: hiding the tab stops it, so a hidden timeline never
  // steps on (and never moves its dump) behind another tab.
  $effect(() => {
    if (!ws.active) stop();
  });

  function onSlide(e: Event) {
    goTo(Number((e.currentTarget as HTMLInputElement).value));
  }

  /** Name the step under the pointer in the slider's tooltip, so a scrub can aim before it moves. */
  function onPointer(e: PointerEvent) {
    if (!steps) return;
    const rect = (e.currentTarget as HTMLInputElement).getBoundingClientRect();
    pointerTitle = stepTitle(stepAtPointer(e.clientX - rect.left, rect.width, steps));
  }
</script>

<!-- One row under the top bar: the timeline's controls, then the step in one line. What the
     step wrote is the What changed panel's, at the top of the right column. -->
<section class="panel opbar" aria-labelledby="opbar-heading-{ws.id}">
  <h2 id="opbar-heading-{ws.id}" class="sr-only">Operation</h2>
  <div class="opbar-controls">
    <button onclick={() => goTo(volume.cursor - 1)} disabled={volume.cursor <= 0}>Prev</button>
    <button onclick={togglePlay} disabled={!playing && volume.atLatest}>{playing ? "Pause" : "Play"}</button>
    <button onclick={() => goTo(volume.cursor + 1)} disabled={volume.atLatest}>Next</button>
  </div>
  <input
    class="tl-range"
    type="range"
    min="0"
    max={Math.max(0, steps - 1)}
    value={Math.max(0, volume.cursor)}
    disabled={steps === 0}
    list="opbar-ticks-{ws.id}"
    oninput={onSlide}
    onpointermove={onPointer}
    onpointerleave={() => (pointerTitle = null)}
    title={pointerTitle ?? (rec ? stepTitle(volume.cursor) : undefined)}
    aria-label="Timeline step"
  />
  <datalist id="opbar-ticks-{ws.id}">
    {#each volume.history as _, i (i)}<option value={i}></option>{/each}
  </datalist>
  <p class="opbar-summary">
    {#if !rec}
      <span class="mono">{summary}</span> · Run an action to see what it changes.
    {:else}
      <span class="counter">{volume.cursor + 1} of {steps}</span>
      · <span class="mono op">{rec.op}</span>
      {#if sectorCount}· <span>{sectorCount} {sectorCount === 1 ? noun.singular : noun.plural}</span>{/if}
      {#if rec.events.length}· <span>{rec.events.length} {rec.events.length === 1 ? "event" : "events"}</span>{/if}
    {/if}
  </p>
</section>
```

- [ ] **Step 7: The What changed panel**

Create `web/ui/src/components/WhatChangedPanel.svelte` (StepPanel's sector buttons become range buttons; its event markup is kept inside each phase):

```svelte
<script lang="ts">
  import { groupEvents, type PhaseId } from "../core/eventPhases";
  import type { Interval } from "../core/intervals";
  import { changedSectors } from "../core/patch";
  import { formatRanges } from "../core/ranges";
  import { getWorkspace } from "../state/workspace.svelte";

  const { volume, selection, layers } = getWorkspace();

  const rec = $derived(volume.history[volume.cursor]);
  const ranges = $derived(rec ? formatRanges(changedSectors(rec.changes, volume.sectorSize)) : []);
  const phases = $derived(rec ? groupEvents(rec.events) : []);
  // The family's noun for a sector: "Sectors" on FAT, "Blocks" on ext.
  const noun = $derived.by(() => { volume.epoch; return volume.adapter.sector; });
  const cap = (s: string) => s[0].toUpperCase() + s.slice(1);

  /** The phases someone opened or closed, by id, kept while stepping: a phase not in here is open
   *  when it is the step's first and closed otherwise. */
  let toggled = $state<Partial<Record<PhaseId, boolean>>>({});
  const isOpen = (id: PhaseId, i: number) => toggled[id] ?? i === 0;

  /** Record a toggle someone made. Setting `open` from `isOpen` fires `toggle` too, but then the
   *  element already agrees with `isOpen` and nothing is recorded. */
  function onToggle(e: Event, id: PhaseId, i: number) {
    const open = (e.currentTarget as HTMLDetailsElement).open;
    if (open !== isOpen(id, i)) toggled[id] = open;
  }

  /** Outline `iv` on the ribbon and the map while the pointer or focus is on its button. */
  function hover(iv: Interval | null) {
    layers.hover = iv;
  }

  // A button can vanish from under the pointer when the step changes (a `[`/`]` scrub, a new
  // operation) without a `mouseleave`: clear the outline with the step it belonged to.
  $effect(() => {
    rec;
    return () => { layers.hover = null; };
  });
</script>

<section class="panel changed">
  <h2>What changed</h2>
  {#if !rec}
    <p class="muted">Nothing yet.</p>
  {:else}
    {#if ranges.length}
      <p class="muted changed-label">{cap(noun.plural)}</p>
      <div class="ranges">
        {#each ranges as r (r.first)}
          {@const bytes = { start: r.first * volume.sectorSize, end: (r.last + 1) * volume.sectorSize }}
          <button
            class="mono"
            aria-label="{cap(r.first === r.last ? noun.singular : noun.plural)} {r.label}"
            onclick={() => selection.jumpTo(bytes.start)}
            onmouseenter={() => hover(bytes)}
            onmouseleave={() => hover(null)}
            onfocus={() => hover(bytes)}
            onblur={() => hover(null)}
          >{r.label}</button>
        {/each}
      </div>
    {/if}
    {#each phases as phase, i (phase.id)}
      <details class="phase" open={isOpen(phase.id, i)} ontoggle={(e) => onToggle(e, phase.id, i)}>
        <summary class:warn={phase.id === "crash"}>{phase.label} · {phase.events.length} {phase.events.length === 1 ? "event" : "events"}</summary>
        <ol>
          {#each phase.events as ev (ev.index)}
            <li>
              {#if ev.region}
                {@const region = ev.region}
                <button
                  class="link"
                  onclick={() => selection.jumpTo(region.start)}
                  onmouseenter={() => hover(region)}
                  onmouseleave={() => hover(null)}
                  onfocus={() => hover(region)}
                  onblur={() => hover(null)}
                ><code>{ev.kind}</code> {ev.text}</button>
              {:else}
                <code>{ev.kind}</code> {ev.text}
              {/if}
            </li>
          {/each}
        </ol>
      </details>
    {/each}
  {/if}
</section>
```

- [ ] **Step 8: `WorkspaceView` renders them; `StepPanel` and `Timeline` go**

In `web/ui/src/components/WorkspaceView.svelte`, the imports

```svelte
  import Inspector from "./Inspector.svelte";
  import Ribbon from "./Ribbon.svelte";
  import StepPanel from "./StepPanel.svelte";
  import StringsPanel from "./StringsPanel.svelte";
  import Timeline from "./Timeline.svelte";
```

become

```svelte
  import Inspector from "./Inspector.svelte";
  import OperationBar from "./OperationBar.svelte";
  import Ribbon from "./Ribbon.svelte";
  import StringsPanel from "./StringsPanel.svelte";
  import WhatChangedPanel from "./WhatChangedPanel.svelte";
```

the `MapPanel` comment

```svelte
  /** The map panel of the tab's family (the FAT map, the block-group map), from the panel
   *  registry. The family's aside panels (ext: the Journal) head the right column. */
```

becomes

```svelte
  /** The map panel of the tab's family (the FAT map, the block-group map), from the panel
   *  registry. The family's aside panels (ext: the Journal) follow What changed at the top of
   *  the right column. */
```

and the markup

```svelte
<!-- The current step sits with the other controls, under the picker and the Terminal
     button, so the right column is left to state and data (the family's aside panels, such as
     ext's Journal, then Strings and the Inspector). The slots are classes, not ids: every
     opened tab has its own. -->
<div class="step-slot"><StepPanel /></div>
<div class="ribbon-slot"><Ribbon /></div>
<div class="grid">
  <aside class="col left">
    <DirTree />
    <MapPanel />
    <ActionsPanel />
  </aside>
  <main class="col center"><HexView /></main>
  <aside class="col right">
    {#each PANELS[ws.id].aside as Aside}<Aside />{/each}
    <StringsPanel />
    <Inspector />
  </aside>
</div>
<footer class="timeline-slot"><Timeline /></footer>
```

becomes

```svelte
<!-- The Operation bar (the timeline's controls and the step in one line) sits under the top
     bar; what the step wrote heads the right column (What changed), then the family's aside
     panels (ext's Journal), Strings, and the Inspector. The slots are classes, not ids: every
     opened tab has its own. -->
<div class="opbar-slot"><OperationBar /></div>
<div class="ribbon-slot"><Ribbon /></div>
<div class="grid">
  <aside class="col left">
    <DirTree />
    <MapPanel />
    <ActionsPanel />
  </aside>
  <main class="col center"><HexView /></main>
  <aside class="col right">
    <WhatChangedPanel />
    {#each PANELS[ws.id].aside as Aside}<Aside />{/each}
    <StringsPanel />
    <Inspector />
  </aside>
</div>
```

Delete the two components (no test imports either; `git grep -n "StepPanel\|Timeline.svelte" -- src tests` finds nothing afterwards):

```sh
git rm web/ui/src/components/StepPanel.svelte web/ui/src/components/Timeline.svelte
```

In `web/ui/src/state/workspace.svelte.ts`, `goToStep`'s doc comment line 48:

```ts
   * Timeline's controls and from App.svelte's `[`/`]` shortcuts. A step with no changes (or an
```

becomes

```ts
   * Operation bar's controls and from App.svelte's `[`/`]` shortcuts. A step with no changes (or an
```

- [ ] **Step 9: The Ribbon: `observeWidth`, and the hover outline**

In `web/ui/src/components/Ribbon.svelte`, the imports gain two lines:

```ts
  import { freeSpaceLabel } from "../core/freeSpace";
  import type { Interval } from "../core/intervals";
  import { metaLabel } from "../core/legend";
  import { observeWidth } from "../core/observeWidth";
```

Delete `  let wrap = $state<HTMLDivElement>();` and replace the width effect

```ts
  // Track the ribbon's own on-screen width so `cols` follows a resize (breakpoint
  // change, window resize) rather than a value baked in at mount — same pattern as
  // FatMap.
  $effect(() => {
    if (!wrap) return;
    width = wrap.getBoundingClientRect().width;
    const ro = new ResizeObserver((entries) => { width = entries[0].contentRect.width; });
    ro.observe(wrap);
    return () => ro.disconnect();
  });
```

with

```ts
  // The ribbon's own on-screen width, kept current by `observeWidth` on the wrapper so `cols`
  // follows a resize (breakpoint change, window resize) rather than a value baked in at mount —
  // the maps' pattern. A hidden tab's ribbon measures 0; `observeWidth` ignores that, so the
  // ribbon keeps its width instead of repainting 1 px wide and jumping back when shown.
```

and the wrapper

```svelte
  <div class="ribbon-canvas-wrap" bind:this={wrap}>
```

with

```svelte
  <div class="ribbon-canvas-wrap" use:observeWidth={(w) => (width = w)}>
```

The paint effect reads the hover:

```ts
    volume.epoch; layers.visible; layers.diff; cols;
```

becomes

```ts
    volume.epoch; layers.visible; layers.diff; layers.hover; cols;
```

The column arithmetic the flash and the viewport bracket each repeated becomes one helper (the same maths: `cssWidth` is `cols`, and `c1 >= c0` already held for the flash). `computeFlashCols`

```ts
  function computeFlashCols(): Set<number> {
    const out = new Set<number>();
    for (const iv of layers.diff) {
      const s0 = Math.floor(iv.start / volume.sectorSize);
      const s1 = Math.max(s0, Math.ceil(iv.end / volume.sectorSize) - 1);
      const c0 = Math.max(0, Math.min(cols - 1, Math.floor(s0 / sectorsPerCol)));
      const c1 = Math.max(0, Math.min(cols - 1, Math.floor(s1 / sectorsPerCol)));
      for (let c = c0; c <= c1; c++) out.add(c);
    }
    return out;
  }
```

becomes

```ts
  /** The first and last columns a byte range covers. */
  function colSpan(iv: Interval): { c0: number; c1: number } {
    const s0 = Math.floor(iv.start / volume.sectorSize);
    const s1 = Math.max(s0, Math.ceil(iv.end / volume.sectorSize) - 1);
    const c0 = Math.max(0, Math.min(cols - 1, Math.floor(s0 / sectorsPerCol)));
    const c1 = Math.max(c0, Math.min(cols - 1, Math.floor(s1 / sectorsPerCol)));
    return { c0, c1 };
  }

  function computeFlashCols(): Set<number> {
    const out = new Set<number>();
    for (const iv of layers.diff) {
      const { c0, c1 } = colSpan(iv);
      for (let c = c0; c <= c1; c++) out.add(c);
    }
    return out;
  }
```

In `paint()`, the viewport bracket's

```ts
      const s0 = Math.floor(v.start / volume.sectorSize);
      const s1 = Math.max(s0, Math.ceil(v.end / volume.sectorSize) - 1);
      const c0 = Math.max(0, Math.min(cssWidth - 1, Math.floor(s0 / sectorsPerCol)));
      const c1 = Math.max(c0, Math.min(cssWidth - 1, Math.floor(s1 / sectorsPerCol)));
```

becomes

```ts
      const { c0, c1 } = colSpan(v);
```

and right after the diff flash block (which ends `        ctx.globalAlpha = 1;` / `      }` / `    }`), before `paint()`'s closing brace, add (the block starts with a blank line):

```ts

    // What changed's hovered range or event: a 1 px outline around the columns it covers, drawn
    // just outside them so a one-column range stays visible.
    const h = layers.hover;
    if (h && h.end > h.start) {
      const { c0, c1 } = colSpan(h);
      ctx.strokeStyle = focus;
      ctx.lineWidth = 1;
      ctx.strokeRect(c0 - 0.5, 0.5, c1 - c0 + 2, HEIGHT - 1);
    }
```

- [ ] **Step 10: The maps outline the hovered cells after the diff pass**

In `web/ui/src/fs/fat16/FatMap.svelte`, the imports gain the interval type:

```ts
  import { cellAt, cellRect, dashCell, dotCell, drawChain, gridCols, gridRows, outlineCell, prepareCanvas } from "../../core/grid";
```

becomes

```ts
  import { cellAt, cellRect, dashCell, dotCell, drawChain, gridCols, gridRows, outlineCell, prepareCanvas } from "../../core/grid";
  import type { Interval } from "../../core/intervals";
```

the paint effect

```ts
    volume.epoch; layers.chain; layers.diff; selection.hoverOffset; canvasWidth; canvasHeight;
```

becomes

```ts
    volume.epoch; layers.chain; layers.diff; layers.hover; selection.hoverOffset; canvasWidth; canvasHeight;
```

`overlapsDiff` becomes one test for any list of byte ranges, so the diff and the hover share it:

```ts
  function overlapsDiff(c: number): boolean {
    if (!layers.diff.length) return false;
    const { start, end } = fs.unitByteRange(c);
    for (const iv of layers.diff) if (iv.start < end && iv.end > start) return true;
    return false;
  }
```

becomes

```ts
  /** Whether cluster `c`'s bytes overlap any of `ivs` (the step's diff, the hovered range). */
  function overlapsAny(c: number, ivs: readonly Interval[]): boolean {
    if (!ivs.length) return false;
    const { start, end } = fs.unitByteRange(c);
    for (const iv of ivs) if (iv.start < end && iv.end > start) return true;
    return false;
  }
```

in `paint()`,

```ts
    const fat = fs.fat;
    const attribution = volume.attribution;
```

becomes

```ts
    const fat = fs.fat;
    const attribution = volume.attribution;
    // What changed's hovered range or event, as a list for `overlapsAny`.
    const hover = layers.hover ? [layers.hover] : [];
```

and the cluster loop's last line

```ts
      if (overlapsDiff(c)) outlineCell(ctx, x, y, CELL, diffColor);
```

becomes

```ts
      if (overlapsAny(c, layers.diff)) outlineCell(ctx, x, y, CELL, diffColor);
      // The hovered range's clusters, outlined in the focus colour over the diff's. A range in the
      // FATs or the root directory touches none.
      if (overlapsAny(c, hover)) outlineCell(ctx, x, y, CELL, focus);
```

(The hover is outlined in the same pass as the diff, after each cell's diff outline, so the map walks its clusters once per paint.)

In `web/ui/src/fs/ext/BlockGroupMap.svelte`, the paint effect

```ts
    volume.epoch; layers.chain; layers.diff; selection.hoverOffset; headers; map; fills; scrollOffset;
```

becomes

```ts
    volume.epoch; layers.chain; layers.diff; layers.hover; selection.hoverOffset; headers; map; fills; scrollOffset;
```

and right after the diff loop (`for (const b of blocksTouched(layers.diff, geo.blockSize, geo.totalBlocks)) { … outlineCell(ctx, c.x, c.y, CELL, diffColor); }`) add this block, which starts with a blank line (the blank line above `const chain = layers.chain…` stays):

```ts

    // What changed's hovered range or event: the blocks it touches, outlined in the focus colour
    // over the diff's.
    if (layers.hover) {
      for (const b of blocksTouched([layers.hover], geo.blockSize, geo.totalBlocks)) {
        const c = cellOf(map, b);
        if (c && isVisible(c.y)) outlineCell(ctx, c.x, c.y, CELL, focus);
      }
    }
```

(Both already import `outlineCell`; `focus` is read at the top of each `paint()`. The selected file's chain is drawn after these in the same `--focus`, so hovering its own clusters changes nothing on the map; the ribbon still shows it.)

- [ ] **Step 11: Run the gates**

Run: `git grep -n "StepPanel\|Timeline\.svelte\|timeline-slot\|step-slot\|step-heading\|tl-controls" -- src tests`
Expected: no output.

Run: `pnpm test`
Expected: `Test Files  57 passed (57)`, `Tests  556 passed (556)` (537 + 18 in the three new files + 1 new layout guard).

Run: `pnpm build`
Expected: svelte-check `COMPLETED 470 FILES 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS`, then `✓ built in …`.

`git status --short` shows only the files listed above (never `pnpm-lock.yaml` or `pnpm-workspace.yaml`).

- [ ] **Step 12: Check it in the browser (1440 × 900)**

From `web/ui`: `pnpm exec vite --port 5196 --strictPort` (Bash, background; any free port), then open `http://localhost:5196/` with `preview_start` by URL and set the viewport to 1440 × 900.

Expected (as observed):
- FAT16: the bar reads `FAT16 · 16 MB · 512-byte sectors · 2 KiB clusters · Run an action to see what it changes.` with Prev/Play/Next disabled; What changed reads `Nothing yet.` above Strings. Add file (`/Hello world.txt`): `1 of 1 · create_file /Hello world.txt · 7 sectors · 7 events`; What changed: `Sectors` `1` `33` `65` `97–100`, then `Allocation · 3 events` (open), `Data · 1 event`, `Directory entry · 3 events` (closed); the slider's `list` is `opbar-ticks-fat16` with one option per step.
- Hover `97–100`: the ribbon outlines its columns around sector 97, and leaving clears it; the FAT map does not change, because the selected `/Hello world.txt`'s chain is already drawn in `--focus`.
- Set the path to `/b.txt` and Add file: `2 of 2 · create_file /b.txt · 7 sectors · 5 events`, ranges `1` `33` `65` `101–104` (Add file selects `/b.txt`).
- Press `[` (`1 of 2 · create_file /Hello world.txt · 7 sectors · 7 events`) and hover `97–100`: the ribbon outlines the columns around sector 97 and the FAT map outlines cluster 2's cell; leaving clears both. Click `97–100`: the dump's cursor moves to `0000c200`.
- Move the pointer over the slider: the tooltip reads `1 of 2 · create_file /Hello world.txt` at its left end and `2 of 2 · create_file /b.txt` at its right. Prev/Next, Play (label `Pause` while playing, stops at the last step), and `[`/`]` scrub the tab.
- ext: `ext3 · 16 MB · 16,384 1 KiB blocks in 2 groups · 1,024-block journal, ordered mode · Run an action to see what it changes.`; right column What changed, `Journal · ordered mode`, Strings, `At this byte`. Add file: `1 of 1 · create_file /Hello world.txt · 18 blocks · 30 events`, ranges `1–6 69 82–91 1111`, `Filesystem writes · 10 events` open, `Journal · 11 events`, `Checkpoint · 7 events`, `Cleanup · 2 events` closed. Expand the `Journal · 11 events` phase, then set the path to `/two.txt` and Add file: Filesystem writes and Journal are both open (a phase you open stays open while you step). Hovering `69` or `1–6` outlines those blocks on the block-group map.
- Journal panel: `after commit`, Arm, Add file `/crash.txt`: `3 of 3 · create_file /crash.txt (crashed after commit) · 12 blocks · 22 events`, phases Filesystem writes, Journal, `Crash · 1 event` in `--warn`. Open the terminal (the Terminal button) and type `recover`: `4 of 4 · recover · 8 blocks · 10 events`, `Recovery · 8 events` open, `Cleanup · 2 events`.
- With the ext tab shown, `getComputedStyle(document.querySelector('#workspace-fat16 .ribbon canvas')).width` is still `1402px` (before this task the hidden tab's ribbon repainted 1 px wide). At 760 × 900 the bar is two lines (controls and slider at y ≈ 135, the summary at y ≈ 167), no sideways scroll. No console errors.

Reset the viewport, stop the dev server, and close the browser tab.

- [ ] **Step 13: Commit**

```sh
git add web/ui/src/core/ranges.ts web/ui/src/core/eventPhases.ts web/ui/src/core/timelineSlider.ts web/ui/src/components/OperationBar.svelte web/ui/src/components/WhatChangedPanel.svelte web/ui/src/components/WorkspaceView.svelte web/ui/src/components/Ribbon.svelte web/ui/src/fs/fat16/FatMap.svelte web/ui/src/fs/ext/BlockGroupMap.svelte web/ui/src/state/layers.svelte.ts web/ui/src/state/workspace.svelte.ts web/ui/src/app.css web/ui/tests/ranges.test.ts web/ui/tests/eventPhases.test.ts web/ui/tests/timelineSlider.test.ts web/ui/tests/layout.test.ts
git commit -m "feat(ui): an Operation bar and a What changed panel replace the Step strip and the Timeline" -m "OperationBar holds Prev/Play/Next, a ticked slider whose tooltip names the step under the pointer, and the step in one line (or the disk's summary() before any action). WhatChangedPanel heads the right column: the changed blocks as ranges (formatRanges) and the events grouped into phases (groupEvents), FAT-style or ext3-style. Hovering a range or an event outlines it on the ribbon and the map (layers.hover). The ribbon measures its width with observeWidth, so a hidden tab's ribbon keeps it."
```

(The two deletions are already staged by Step 8's `git rm`; the sixteen paths above are the rest of the Files block.)


### Task 7: Docs, the spec amendments, the lesson sentences, and the browser pass

Spec: `docs/superpowers/specs/2026-09-25-family-tabs-design.md` section 7 (the amendments to `2026-09-24-ext-explorer-design.md` sections 5 and 6; the `web/ui/README.md` Panes section; `docs/ROADMAP.md`) and section 8 (the browser pass), plus section 5 (focus on a routed Load image), which the pass found unmet and this task fixes in `ActionsPanel.svelte` (Task 4's file). Base: `plan/tabs-t6` (Task 6, 1b4b51b). The contract's ruling for Task 7 sanctions the two lesson sentences that named the Step strip; Global Constraint 1 lists them. No test pins either sentence, and no test changes in this task.

**Files**

- Modify: `web/ui/README.md` (8-10, 38-91, 104-137, 156-178, 219-234, 249, 323-329, 350-353, 398-424, 440-448)
- Modify: `docs/ROADMAP.md` (68-69, 124-126, 135-158, 242-253, 274-276, 324-326)
- Modify: `docs/superpowers/specs/2026-09-24-ext-explorer-design.md` (38, 94-96, 250, 282-292, 312-314, 344-349, 358-360, 368-369)
- Modify: `docs/superpowers/specs/2026-09-25-family-tabs-design.md` (27, 69, 73, 77, 87)
- Modify: `docs/testing.md` (61-66)
- Modify: `web/ui/src/scenarios/fundamentals.ts` (94)
- Modify: `web/ui/src/scenarios/journaledWrite.ts` (9-10, 40)
- Modify (browser-pass fix): `web/ui/src/components/ActionsPanel.svelte` (2, 63-69, 118)

The root `README.md` does not describe the Step strip or the picker, so it is unchanged. `docs/testing.md` names none of the renamed ids (`#format-family`, `action-path`, `step-heading`); it only gains the new tested modules in its list. The ROADMAP records the renamed ids and the removed `#format-family` select (spec section 7).

**Interfaces**

Consumes (Task 1): `workspaces` (`activeId`, `loadImage`), `getWorkspace()`, `Workspace.id` (`src/state/workspace.svelte.ts`). (Task 4): the Load image `<input>` with `aria-describedby="load-hint-{ws.id}"` in `ActionsPanel.svelte`. Existing: `tick` from `svelte`.

Produces:

```ts
// ActionsPanel.svelte: the Load image input gets the per-tab id load-image-{ws.id}; after a routed load
//   (workspaces.activeId !== ws.id once loadImage returns) it awaits tick() and focuses load-image-{activeId}
// DOM id: load-image-{id}
```

All commands below run in `web/ui` unless they say otherwise (a fresh worktree first needs, from the repo root, `wasm-pack build crates/wasm --target bundler`, then in `web/ui` `CI=true pnpm install --frozen-lockfile`). Baseline on `plan/tabs-t6`: `pnpm test` gives `Test Files  57 passed (57)`, `Tests  556 passed (556)`.

- [ ] **Step 1: Rewrite the two lesson sentences that named the Step strip**

In `web/ui/src/scenarios/fundamentals.ts` (step "Putting it together"), replace:

```
watch the ribbon and the Step strip to see the same three places, slot, table, and data, move each time.
```

with:

```
watch the ribbon and the What changed panel to see the same three places, slot, table, and data, move each time.
```

In `web/ui/src/scenarios/journaledWrite.ts` (step "One create, one transaction"), replace:

```
The Step strip lists the create's events in the order they happened; the next steps follow them.
```

with:

```
The What changed panel lists the create's events, grouped by phase, in the order they happened; the next steps follow them.
```

and in the same file's doc comment, replace:

```ts
 * were written. The first step runs the create; every later step only moves the dump, and the
 * Step strip's events list the writes in the same order. The dump shows the disk after the
 * create, so steps about a moment mid-transaction say what the bytes read then and what they
 * read now. tests/journaled-write.test.ts pins every quoted number, and the journal positions
```

with:

```ts
 * were written. The first step runs the create; every later step only moves the dump, and the
 * What changed panel's events list the writes in the same order. The dump shows the disk after
 * the create, so steps about a moment mid-transaction say what the bytes read then and what they
 * read now. tests/journaled-write.test.ts pins every quoted number, and the journal positions
```

- [ ] **Step 2: Confirm no lesson test pinned them**

Run: `pnpm exec vitest run tests/fundamentals.test.ts tests/journaled-write.test.ts tests/scenarios.test.ts`
Expected: `Test Files  3 passed (3)`, `Tests  40 passed (40)`.

Run: `git grep -n -i "step strip" -- src`
Expected: no output.

- [ ] **Step 3: `web/ui/README.md` and `docs/ROADMAP.md`**

Save this patch as `/tmp/task7-docs.patch` and apply it from the repo root with `git apply /tmp/task7-docs.patch` (check first with `git apply --check`). The README's Panes section is rewritten whole (the two tabs as workspaces, the hash, the per-tab columns, the shared and family-only panes, the Operation bar and What changed in place of the Step/Timeline bullet, Actions with `Format (FAT16)` / `Format (ext2 / ext3)` and Load image routing, lessons per tab, the card following its tab); the Terminal section's command-set paragraph, the `mkfs` row, and the working-directory bullet become per tab (the banner, `mkfs --type` limited to the tab's types); "What is filesystem-specific" gains `summary()`, `aside` in place of `extras`, and a paragraph on the workspace/context model; the shortcuts table gets `[`/`]` and `/` on the active tab and Left/Right/Home/End on the tabs. The opening sentence's "twelve guided scenarios" stays (nine FAT16 and three ext lessons). The ROADMAP gains the family-tabs bullet under "Landed", a deferred note `web/ui, after the family tabs` (the private `paneManager.handleEvent` banner, `FatMap`'s scroll, never-disposed workspaces of about 33 MB, the shared scrollback), and its two "one per page" working-directory notes become per tab. The family-tabs bullet ends with the per-tab DOM ids (`action-path-{id}`, `opbar-heading-{id}`, `opbar-ticks-{id}`, `load-hint-{id}`, `load-image-{id}`) and the removed `#format-family` select, and slice 4's bullet marks its Filesystem select and grouped picker as replaced by the tabs. The README's What changed bullet says a kind the grouping does not know lands in Other only without the journal (with it, in Filesystem writes, as `eventPhases.test.ts` pins). The ROADMAP had no `.step` cap or Step strip note to drop, and "`[` and `]` are not scenario-aware" is still true (`App.svelte`'s keys call `workspaces.active.goToStep` whether or not a lesson runs), so it stays.

````diff
diff --git a/docs/ROADMAP.md b/docs/ROADMAP.md
index ee5ed00..9d02087 100644
--- a/docs/ROADMAP.md
+++ b/docs/ROADMAP.md
@@ -65,7 +65,8 @@ with `Unsupported`. Still to do:
 ## Landed: `crates/ext` and the ext explorer
 
 Decided 2026-09-23, in four slices, each with its spec under
-`docs/superpowers/specs/`. All four have landed:
+`docs/superpowers/specs/`. All four have landed, followed by the family tabs
+(2026-09-25), which reshaped slice 4's chrome:
 
 - **Slice 1, the adapter seam** (`2026-09-23-fs-adapter-design.md`): every
   FAT assumption in `web/ui` sits behind `FsAdapter`; see "What stays
@@ -120,8 +121,9 @@ Decided 2026-09-23, in four slices, each with its spec under
   Lesson card's `fileParts`, owner roles and fixed colours, `extras` panels,
   and the optional journal capability (`FsAdapter.journal`,
   `needsRecovery`). The explorer mounts ext2 and ext3 volumes, formatted from
-  the Actions panel's Filesystem select or `mkfs --type ext2|ext3`, or loaded
-  from an mke2fs image: a block-group map replaces the FAT map, the Inspector
+  the Actions panel's Filesystem select (replaced by the family tabs below)
+  or `mkfs --type ext2|ext3`, or loaded from an mke2fs image: a block-group
+  map replaces the FAT map, the Inspector
   explains inodes, indirect blocks, and journal blocks, and `stat`, `df`, and
   `seek i:N` speak ext. On ext3 a Journal panel shows the ring with live and
   stale transactions, arms a crash phase, and recovers; the terminal's
@@ -130,7 +132,30 @@ Decided 2026-09-23, in four slices, each with its spec under
   the status line explains `NeedsRecovery` and shows `Unsupported`'s own
   wasm text. Three ext3 lessons (the fundamentals (ext), a journaled write,
   crash and recover) pin their numbers by test, and the lesson picker groups
-  lessons by filesystem. FAT16 is unchanged and still the default volume.
+  lessons by filesystem (replaced by the family tabs below). FAT16 is
+  unchanged and still the default volume.
+- **Family tabs** (`2026-09-25-family-tabs-design.md`): the explorer has a
+  tab per family, `FAT16` and `ext`, in the top bar, and each tab is a
+  workspace (`web/ui/src/state/workspace.svelte.ts`): its own volume and
+  timeline, selection, layers, lesson runner, and shell cwd, built with
+  their siblings injected and reached through Svelte context
+  (`getWorkspace()`) instead of module singletons. Every opened tab's body
+  stays mounted and hidden, so a switch keeps the dump's and the block-group
+  map's scroll, the panels' state, and a running lesson (a `lesson` badge
+  marks it); the ext workspace is created on first use; the URL hash names
+  the tab. Inside a tab nothing flips the family: the Filesystem select is
+  gone (`Format (FAT16)` / `Format (ext2 / ext3)`), the picker lists the
+  tab's lessons (`lessonsFor`), `mkfs --type` takes only the tab's types, and
+  Load image opens an image in its own family's tab. One terminal follows the
+  active tab with a banner on each switch. The Step strip and the Timeline
+  footer became the Operation bar (the timeline controls, a ticked slider,
+  and the step in one line, or the adapter's `summary()` before any action)
+  and the What changed panel at the top of the right column (the changed
+  blocks as ranges and the events grouped into phases); the Journal panel
+  moved to the right column (`PANELS[id].aside`). DOM ids inside a tab are
+  per tab (`action-path-{id}`, `opbar-heading-{id}`, `opbar-ticks-{id}`,
+  `load-hint-{id}`, `load-image-{id}`; `action-path` and `step-heading` are
+  gone), and the Format details' `#format-family` select is removed.
 
 ## Core API additions
 
@@ -214,6 +239,18 @@ clicking one of the journal's pointer blocks (`<journal>`,
 `<journal>` is a pseudo-owner (`isPseudoOwner`), not a tree path, so there is
 nothing to select.
 
+**`web/ui`, after the family tabs** (the tabs plan's browser pass): the
+terminal's tab banner is printed through browser-terminal 0.3.0's private
+`paneManager.handleEvent` (a `paneOutput` event), guarded so a missing
+internal only drops the banner, until the library has a public call that
+prints a host line; `FatMap`'s scroll is not kept across a tab switch (only
+the dump and the block-group map use `keepScroll`), so a FAT map scrolled
+down comes back at the top; workspaces are never disposed, about 33 MB each
+(the 16 MB image in wasm memory, a 16 MB JavaScript copy, the zero map), so
+the page holds both disks once the ext tab has opened; and the xterm
+scrollback is shared across tabs by design, with the banner marking each
+switch.
+
 **`web/ui`, seams the ext slice inherited**: (a) the free-space model in
 generic code assumes allocation units exist only in `data` regions and that
 an unowned unit is free. Slice 4 keeps journal blocks out of the units and
@@ -234,8 +271,9 @@ adapters and the shell, while `components/StringsPanel.svelte`,
 own.
 
 **`web/ui` terminal** (decided 2026-09-22): the working directory is one
-value per page, not per shell session, because `setPrompt` is engine-wide and
-two sessions could not show different directories in their prompts anyway;
+value per tab (per page until the family tabs), not per shell session,
+because `setPrompt` is engine-wide and two sessions could not show different
+directories in their prompts anyway;
 reads while the timeline is rewound show the latest state and warn (an
 `--at-step` flag reading the cached image is the follow-up), except through a
 `<` redirect, which has no channel to warn on; `dd` and `cat` are capped at
@@ -283,9 +321,9 @@ explorer adopted them on 2026-09-22. Each workaround they replaced is gone:
 5. A session or pane id on `ctx`, so each shell could keep its own working
    directory: https://github.com/benjamin-small/browser-terminal/issues/16.
    `ctx.session` and `ctx.pane` are available, but the working directory
-   stays one per page: `setPrompt` is engine-wide, so per-session directories
-   could not be shown in their prompts. The ids are there for whenever that
-   changes.
+   stays one per tab (the family tabs made it per tab; it was one per page):
+   `setPrompt` is engine-wide, so per-session directories could not be shown
+   in their prompts. The ids are there for whenever that changes.
 6. `CreateOptions.terminal` (theme, font family, font size) and `setTheme`,
    replacing the `!important` CSS overrides:
    https://github.com/benjamin-small/browser-terminal/issues/17.
diff --git a/web/ui/README.md b/web/ui/README.md
index ec358f9..ab162b9 100644
--- a/web/ui/README.md
+++ b/web/ui/README.md
@@ -5,8 +5,9 @@ and ext3: a whole-disk hex dump with ASCII and a strings overlay, a disk
 ribbon and a FAT cluster map or ext block-group map for seeing where files
 land, an ext3 journal panel with crash and recovery controls, an operation
 timeline with byte-diff replay, twelve guided scenarios, and a terminal
-drawer that mounts the volume at `/mnt` and the raw disk at `/dev/hda`. The
-app opens on a FAT16 disk. It runs entirely in the
+drawer that mounts the volume at `/mnt` and the raw disk at `/dev/hda`. Each
+family has its own tab, FAT16 and ext, and the app opens on the FAT16 tab. It
+runs entirely in the
 browser against the `fs-emulator-wasm` package — nothing is sent over the
 network, and there is no server component. The build from `main` is
 published at https://benjamin-small.github.io/fs-emulator/ by
@@ -34,13 +35,60 @@ pnpm preview   # serve the production build
 
 ## Panes
 
-Layout: the current step runs as a strip under the top bar, next to the
-other controls; a disk ribbon runs full width above a three-column grid —
-files, the mounted family's map (the FAT map, or the block-group map and, on
-ext, the Journal panel under it), and actions on the left; the hex dump in
-the center; the lesson card, strings, and the byte inspector on the right —
-with an operation timeline along the bottom.
-
+The top bar holds two tabs, **FAT16** and **ext**, and each tab is a
+workspace of its own: its own disk and timeline, selection, dump position,
+panel state, running lesson, and the terminal's working directory in it.
+Switching tabs keeps both exactly as they were (both disks stay in memory),
+and a tab that is running a lesson carries a `lesson` badge. The URL hash
+names the tab (`#fat16`, `#ext`; `#fat`, `#ext2`, and `#ext3` work too), so a
+link or a reload opens it, and a bare URL opens FAT16. The ext tab's disk is
+created the first time the tab opens: a fresh ext3 disk. Inside a tab nothing
+changes the family: the Format details, the lesson picker, and `mkfs` know
+only the tab's own, and an image of the other family opens in its own tab.
+
+Layout, per tab: the Operation bar runs under the top bar and a disk ribbon
+full width under it, above a three-column grid:
+
+- **FAT16 tab:** left Files, FAT map, Actions; center the dump; right What
+  changed, Strings, Inspector.
+- **ext tab:** left Files, Block groups, Actions; center the dump; right What
+  changed, Journal, Strings, Inspector.
+
+The right column scrolls, and the Lesson card floats over the page. Both tabs
+have the Ribbon, Files, Dump, Inspector, Strings, Operation bar, and What
+changed (each tab its own). FAT16-only: FAT map. ext-only: Block groups,
+Journal (right column).
+
+- **Operation bar** (under the top bar) — the timeline and the current step
+  in one line: **Prev**, **Play** / **Pause**, **Next**, a slider with one
+  tick per step (its tooltip names the step under the pointer, `2 of 2 ·
+  create_file /b.txt`), then the step, `1 of 1 · create_file /Hello world.txt
+  · 7 sectors · 7 events` (blocks on ext; the operation names are the wasm
+  package's). Before any action it describes the disk instead: `FAT16 · 16 MB
+  · 512-byte sectors · 2 KiB clusters` or `ext3 · 16 MB · 16,384 1 KiB blocks
+  in 2 groups · 1,024-block journal, ordered mode`, then "Run an action to see
+  what it changes." Play stops at the last step, or when its tab is hidden.
+  Replay reconstructs the disk as it was after any step and highlights what
+  that step changed, in amber, in both the dump and the ribbon. Running a
+  command never moves the dump: it highlights the changed bytes and leaves
+  the view where you left it. Any explicit navigation does move it — a
+  timeline step (Prev/Next/Play, the slider, `[`/`]`), the ribbon, a tree
+  node or a map cell, an Inspector, Strings, or What changed link, the dump's
+  own keys, or `seek`.
+- **What changed** (top of the right column) — what the current step wrote.
+  First the changed sectors (blocks on ext) as ranges, `1` `33` `65` `97–100`
+  for a FAT16 create and `1–6` `69` `82–91` `1111` for an ext3 one: click a
+  range to jump the dump to its first sector, hover or focus it to outline
+  it on the ribbon and the map. Then the step's events grouped into phases,
+  in the order each phase first appears, each a collapsible list headed by
+  its count (`Allocation · 3 events`). FAT16 and ext2 steps group into
+  Directory entry, Allocation, and Data; a step that touches the ext3 journal
+  groups into Filesystem writes, Journal, Checkpoint, Cleanup, Recovery, and
+  Crash (in the warning colour). In a step without the journal a kind the
+  grouping does not know lands in Other (with the journal, in Filesystem
+  writes). The first phase starts open and the rest closed, and a phase you
+  open or close stays that way while you step. An event with a region is a
+  button: click to jump, hover to outline.
 - **Ribbon** — the entire disk as one strip, one column per pixel of its
   width, colored by owning file or region (or a hairline for free space),
   with region-boundary ticks and a bracket showing the dump's current
@@ -53,10 +101,10 @@ with an operation timeline along the bottom.
   directory slot, its FAT chain, and its clusters everywhere else in the
   UI. A "Show remnants" toggle marks deleted entries and the data they left
   behind.
-- **FAT map** — every cluster as a small cell: free, owned (in its
+- **FAT map** (FAT16 tab) — every cluster as a small cell: free, owned (in its
   file's hue), end-of-chain, or bad. Hover for the cluster number, FAT
   value, and owner; the selected file's chain draws as connected arrows.
-- **Block groups** (ext, in place of the FAT map) — one band per block
+- **Block groups** (ext tab, in place of the FAT map) — one band per block
   group headed `group 0 · blocks 1–8192 · 7,082 free`, every block a 4 px
   cell: metadata in its region's colour, the journal in amber, owned data and
   directory blocks in their file's hue, free blocks as a hairline, and a dot
@@ -64,7 +112,8 @@ with an operation timeline along the bottom.
   step's changes outlined in the diff colour, and the block under the dump's
   hovered byte dashed. Hover for `block N · region · owner`; click to select
   the owner and jump to the block.
-- **Journal** (ext, under the block-group map) — on ext3 the journal's mode
+- **Journal** (ext tab, right column, under What changed) — on ext3 the
+  journal's mode
   and header facts (sequence, head, start, size), a ring strip with one cell
   per journal block coloured by kind (superblock, descriptor, copy, commit,
   revoke, unused) with checkpointed transactions faded, and the crash
@@ -75,10 +124,17 @@ with an operation timeline along the bottom.
   the panel and the status line say so, and changes throw `NeedsRecovery`.
   On ext2 the panel is one line: "This volume has no journal."
 - **Actions** — add, overwrite, or delete a file; make or remove a
-  folder; format a fresh disk (a Filesystem select picks FAT16, with a size
-  and cluster size, or ext, with ext2 or ext3, a size, inodes per group, a
-  label, and for ext3 the journal's mode and size); load or export a raw
-  image. Every action records a step in the timeline.
+  folder; format a fresh disk of the tab's family (**Format (FAT16)**: a size
+  and cluster size; **Format (ext2 / ext3)**: ext2 or ext3, a size, inodes
+  per group, a label, and for ext3 the journal's mode and size); load or
+  export a raw image. Load image opens an image in its own family's tab,
+  whichever tab it was loaded from: an ext image loaded from the FAT16 tab
+  switches to the ext tab, mounts there (closing a lesson running there),
+  says so on the status line (`Opened mke2fs-ext3.img in the ext tab: it is
+  an ext3 image. The FAT16 tab is as you left it.`), and leaves the keyboard
+  on that tab's Load image; the FAT16 tab is untouched. Bytes neither family
+  recognises are the loading tab's error. Every action records a step in the
+  timeline.
 - **Dump** — the virtualized whole-disk hex view: offset, 16 hex bytes,
   and a 16-character ASCII gutter per row. Bytes are tinted by owner, runs
   of zero sectors collapse into a single clickable row, and the header
@@ -97,35 +153,29 @@ with an operation timeline along the bottom.
 - **Strings** — printable runs (4+ bytes) in the visible window, or the
   whole disk on request, each with its offset and owner; click one to jump
   to it.
-- **Step** (strip under the top bar) **/ Timeline** (bottom) — the current
-  operation's plain-language events and changed sectors, with Prev/Next/Play
-  scrubbing through history. Replay
-  reconstructs the disk as it was after any step and highlights what that
-  step changed, in amber, in both the dump and the ribbon. Running a command
-  never moves the dump: it highlights the changed bytes and leaves the view
-  where you left it. Any explicit navigation does move it — a timeline step
-  (a step button, Prev/Next/Play, the slider, `[`/`]`), the ribbon, a tree
-  node or a FAT-map cluster, an inspector, strings, or step-panel link, the
-  dump's own keys, or `seek`.
-- **Learning scenarios** (top bar) — guided walkthroughs: the fundamentals
+- **Learning scenarios** (top bar) — guided walkthroughs. The picker lists
+  the active tab's lessons, starting on that tab's fundamentals, and each tab
+  remembers its own pick. On the FAT16 tab: the fundamentals
   (a tour of the regions, the allocation table, the root directory, and how
   a file's slot, chain, and clusters link together, on a disk that already
   holds a small file and a three-cluster one), format an empty
   disk, add a small file, add a long-named file (LFN entries), overwrite
   with a larger file (chain grows), delete and see what remains, fill the
   disk, make a directory, and work from the shell (the same operations typed
-  as commands, plus a raw-sector read and a raw patch of the volume label);
-  and, on ext3, the fundamentals (ext) (a tour of the block groups, the
-  superblock, descriptors, bitmaps, and inode table, and how a name reaches
-  its blocks through an inode, including a single-indirect block), a
+  as commands, plus a raw-sector read and a raw patch of the volume label).
+  On the ext tab, all on ext3: the fundamentals (a tour of the block groups,
+  the superblock, descriptors, bitmaps, and inode table, and how a name
+  reaches its blocks through an inode, including a single-indirect block), a
   journaled write (one create followed through the needs-recovery flag, the
   data, the descriptor, the copies, the commit, and the checkpoint), and
   crash and recover (a crash after the commit that recovery replays and one
-  before it that recovery discards, leaving bytes nobody owns). The picker
-  groups them by filesystem, FAT16 first.
+  before it that recovery discards, leaving bytes nobody owns).
   Pick one in the top bar and press **Start**; it formats a fresh disk of the
-  lesson's filesystem (the default FAT16 or ext3 disk) and a
-  **Lesson** card floats over the page, at the top right to begin with. Drag
+  tab's family (the default FAT16 or ext3 disk) and a
+  **Lesson** card floats over the page, at the top right to begin with. The
+  card belongs to its tab: switching away hides it while the lesson keeps
+  running (the tab's `lesson` badge says so), and coming back shows it at the
+  same step. Drag
   it by its bar to wherever it is out of the way of the bytes it talks about,
   or focus the ⋮⋮ handle and use the arrow keys (Shift for bigger steps); it
   remembers where you left it, and `Escape` inside it closes it. The − / +
@@ -166,14 +216,22 @@ describe every command.
 The prompt shows the working directory: the shell hands it to the terminal on
 startup and after every `cd` or `mkfs`, so it reads `/mnt/DOCS ❯`.
 
-The command set follows the mounted volume: when a format, a load, or a
-lesson changes the family (or swaps ext3 for ext2), the terminal re-registers
-its commands. The address help follows the new family (`s:65 (sector), c:3
-(cluster)` on FAT, `b:65 (block), i:11 (inode)` on ext), so does `df`'s
-summary (cluster or block usage), and `crash` and `recover` exist only while
-the volume has a journal. `mkfs`'s flags do not change: every host lists
-every family's. The new commands share the old ones' working-directory
-state, and the prompt is set again.
+There is one terminal for the page, and it follows the active tab. Each
+tab keeps its own working directory; switching tabs re-registers the commands
+over that tab's disk, prints a banner naming the tab and its disk
+(`-- ext tab: /dev/hda is ext3 --`, `-- FAT16 tab: /dev/hda is FAT16 --`),
+and sets the prompt from that tab's directory. The scrollback is shared
+across tabs by design, so the banner marks where one tab's output ends.
+Within a tab the commands are re-registered too when a format or a load
+swaps ext3 for ext2 or back. The address help follows the family (`s:65
+(sector), c:3 (cluster)` on FAT, `b:65 (block), i:11 (inode)` on ext), so
+does `df`'s summary (cluster or block usage), and `crash` and `recover` exist
+only while the volume has a journal. `mkfs` knows only the tab's family:
+`--type` takes `fat16` on the FAT16 tab and `ext2` or `ext3` on the ext tab,
+its flags are that family's alone (`--label` is `volume label, up to 11
+characters` on FAT16 and `volume label, up to 16 bytes` on ext), and the
+other family's type is an error that names its tab (`'ext3' is an ext type:
+switch to the ext tab to format one`).
 
 | Command | Does |
 |---|---|
@@ -188,7 +246,7 @@ state, and the prompt is set again.
 | `df`, `mount` | Cluster usage on FAT, block usage (`blockSize`, `blocks`) on ext; device, mount point, type, and `ok` or `corrupt` |
 | `seek <addr>` | Move the hex dump (`0x200`, `512`, `s:1`, `c:2` on FAT; `b:69` for a block and `i:12` for an inode's slot on ext) |
 | `select [path]` | Select a file in every pane, or clear the selection |
-| `mkfs [--type fat16\|ext2\|ext3] [flags]` | Format a fresh disk; the timeline is cleared. `--type` defaults to the mounted volume's type; FAT16 takes `--sectors --spc --label --root-entries --fats --reserved`, ext `--blocks --inodes-per-group --label --uuid --journal-blocks --journal-mode` (the last two ext3 only); a flag the type does not take is an error that names the type |
+| `mkfs [--type <type>] [flags]` | Format a fresh disk of the tab's family; the timeline is cleared. `--type` is `fat16` on the FAT16 tab and `ext2` or `ext3` on the ext tab, defaulting to the mounted volume's type; FAT16 takes `--sectors --spc --label --root-entries --fats --reserved`, ext `--blocks --inodes-per-group --label --uuid --journal-blocks --journal-mode` (the last two ext3 only); a flag the type does not take is an error that names the type, and the other family's type is an error that names its tab |
 | `crash [--at before-commit\|after-commit\|during-checkpoint] [--off]` | ext3 only: arm a crash so the next change to `/dev/hda` stops at that phase (default `after-commit`), or disarm it |
 | `recover` | ext3 only: replay the journal's committed transaction or discard an uncommitted one, as one timeline step, printing its events |
 | `exit` | Close the drawer |
@@ -262,11 +320,13 @@ Things to know:
   warn on.
 - `dd` and `cat` refuse more than 1 MiB per invocation: every byte is
   journaled twice in Rust and again in the UI's history.
-- The working directory is one value per page, not per shell session: the
-  prompt prefix is engine-wide, so two sessions could not show different
-  directories anyway. Anything that replaces the volume — `mkfs`, the Actions
-  panel's Format, starting a learning scenario, or loading a raw image —
-  returns the shell to `/mnt`, since the directory it was in no longer exists.
+- The working directory is one value per tab, not per shell session: the
+  prompt prefix is engine-wide, so two sessions in the drawer could not show
+  different directories anyway, and the prompt shows the active tab's.
+  Anything that replaces a tab's volume — `mkfs`, the Actions panel's Format,
+  starting a learning scenario, or loading a raw image into it — returns that
+  tab's shell to `/mnt`, since the directory it was in no longer exists;
+  switching tabs never does.
 - `echo` is the shell's own builtin and behaves as in browser-terminal.
 - The terminal and its wasm load on first open, so the initial page load is
   unchanged.
@@ -287,8 +347,10 @@ family lives behind the adapter in `src/fs/`:
   colours. `FsAdapter` is bound to one `Volume` and adds what needs the disk:
   owners, chains, entry slots, remnants, `stat` and `df` facts, the ribbon's
   free count (`freeUnits`), the Inspector's trace, sector annotations, extra address forms, name matching,
-  the "this write may have moved regions" trigger, and the notes the tree and
-  the shell print. Its caches are plain fields that the store refreshes after
+  the "this write may have moved regions" trigger, the notes the tree and
+  the shell print, and `summary()`, the disk in one line, which the Operation
+  bar shows before any action. Its caches are plain fields that the store
+  refreshes after
   every operation, so a `$derived` that calls an adapter method reads
   `volume.epoch` first.
 - `src/fs/fat16/` is the FAT16 implementation: `Fat16Adapter`, the cluster
@@ -333,15 +395,33 @@ family lives behind the adapter in `src/fs/`:
 - `src/fs/index.ts` is the registry: `FAMILIES` (`fat16`, then `ext`),
   `DEFAULT_FAMILY` (`fat16`), `familyIdOf(fsType)`, which returns the family
   whose `fsTypes` lists the string (`FAT16`; `ext2` and `ext3`), and
-  `adapterFor(vol)`. `src/fs/panels.ts` maps a family id to its map and
-  Format panels and its `extras` (the Journal panel for ext), and `App.svelte`
-  and `ActionsPanel.svelte` render whichever the mounted family names. The
-  panels sit in that table rather than on the adapter because the node tests
+  `adapterFor(vol)`; `FAMILIES` is in the order of the tabs, and
+  `DEFAULT_FAMILY` is the tab opened when the URL hash names none.
+  `src/fs/panels.ts` maps a family id to its map and Format panels and its
+  `aside` panels (the Journal panel for ext, at the top of the right column
+  under What changed), and `WorkspaceView.svelte` and `ActionsPanel.svelte`
+  render whichever the tab's family names. The panels sit in that table
+  rather than on the adapter because the node tests
   have no Svelte plugin and the adapter must stay importable from them.
-- Each scenario declares its `family`; `start()` formats that family's
-  default disk. A step reaches generic facts through the adapter it is
+- Each scenario declares its `family`, which is also the tab that lists it
+  (`lessonsFor(family)`, with `defaultLessonFor(family)` the first);
+  `start()` formats that family's default disk. A step reaches generic
+  facts through the adapter it is
   handed and family-only ones through `asFat16(fs)` or `asExt(fs)`.
 
+Each tab is a `Workspace` (`src/state/workspace.svelte.ts`): its own
+`VolumeStore`, `SelectionStore`, `LayersStore`, and `ScenarioRunner`, each
+built with its siblings passed in, and the shell's `Vfs`. A
+`WorkspaceRegistry` creates a workspace the first time its tab is activated,
+names the active one, and routes Load image to the image's family. A
+workspace formats and mounts its own family only (`VolumeStore` refuses
+another, and `ScenarioRunner.start` refuses another family's lesson).
+Components reach their tab's stores through Svelte context, `getWorkspace()`,
+set by `WorkspaceView` for a tab's body and by `WorkspaceScope` for the top
+bar's picker and status line and the Lesson card, never through module
+singletons; every opened tab's body stays mounted and is hidden while
+another tab is active, so each keeps its panels' own state.
+
 `tests/adapterBoundary.test.ts` keeps it that way: it scans
 `src/**/*.{ts,svelte}` and fails on a FAT-only wasm call or type outside
 `src/fs/fat16/` and `src/lib/wasm.ts`, on an ext-only one outside
@@ -357,13 +437,15 @@ family lives behind the adapter in `src/fs/`:
 
 | Key | Effect |
 |---|---|
-| `[` / `]` | Step the timeline back / forward |
+| `[` / `]` | Step the active tab's timeline back / forward |
 | `n` / `p` | Next / previous step on the Lesson card (while a lesson is running) |
 | `Escape` (inside the Lesson card) | Close the lesson |
-| `/` | Focus the path field in Actions |
+| `/` | Focus the active tab's path field in Actions |
+| `Left` / `Right` (a tab focused) | Switch to the previous / next tab, wrapping |
+| `Home` / `End` (a tab focused) | Switch to the first / last tab |
 | Arrow keys | Move the dump's byte cursor, or seek one ribbon column (ribbon focused) |
 | `Page Up` / `Page Down` | Scroll the dump by a page |
-| `Home` / `End` | Jump to the start / end of the disk |
+| `Home` / `End` (dump or ribbon focused) | Jump to the start / end of the disk |
 | `g` | Jump to an offset, sector, or cluster, or on ext an offset or block (dump focused) |
 | `s` | Toggle string highlighting (dump focused) |
 | `` ` `` | Toggle the terminal |
````

- [ ] **Step 4: Amend `2026-09-24-ext-explorer-design.md`'s Outcome and sections 1, 5, 6, and 7 with notes, not rewrites**

Each note is italic and starts `Amended 2026-09-25 by the family-tabs spec:`, under the bullet it amends. Beyond the list in the new spec's section 7, the Outcome's Format bullet, section 1's `PANELS` bullet, section 6's Re-registration bullet, and section 7's `ext-fundamentals` title get a note too, since the tabs changed them as well.

(1) Replace:

```markdown
- `PANELS` gains `extras: Component[]`, rendered by `App.svelte` under the
  map panel in order (`fat16: []`, `ext: [JournalPanel]`).
```

with:

```markdown
- `PANELS` gains `extras: Component[]`, rendered by `App.svelte` under the
  map panel in order (`fat16: []`, `ext: [JournalPanel]`).
  *Amended 2026-09-25 by the family-tabs spec: `extras` is renamed `aside` and rendered by `WorkspaceView.svelte`
  at the top of the right column, under What changed (`fat16: []`,
  `ext: [JournalPanel]`).*
```

(2) Replace:

```markdown
- **`JournalPanel.svelte`** (`PANELS.ext.extras[0]`). On ext2 a single
```

with:

```markdown
- **`JournalPanel.svelte`** (`PANELS.ext.extras[0]`; *Amended 2026-09-25 by the family-tabs spec: `PANELS.ext.aside[0]`, in the right column*). On ext2 a single
```

(3) Replace:

```markdown
  shown is `PANELS[selected].format`.
```

with:

```markdown
  shown is `PANELS[selected].format`.
  *Amended 2026-09-25 by the family-tabs spec: the Filesystem select is gone. Each tab formats its own family:
  the form is `PANELS[volume.family].format`, and the details' summary reads
  `Format (FAT16)` or `Format (ext2 / ext3)` (`FAMILIES[id].fsTypes.join(" / ")`).
  Load image routes the image to its family's tab.*
```

(4) Replace:

```markdown
- **`App.svelte`** renders `PANELS[id].extras` after the map.
```

with:

```markdown
- **`App.svelte`** renders `PANELS[id].extras` after the map.
  *Amended 2026-09-25 by the family-tabs spec: `WorkspaceView.svelte` renders `PANELS[id].aside` at the top of
  the right column, after What changed; nothing follows the map.*
- *Amended 2026-09-25 by the family-tabs spec: **`TabBar.svelte`** (new), in the top bar after the `h1`: a
  `role="tablist"` of one tab per family (`FAT16`, `ext`), each switching the
  whole workspace (disk, timeline, selection, panels, lesson, terminal cwd),
  with a `lesson` badge on a tab running one; the URL hash names the tab.*
```

(5) Replace:

```markdown
  display name in registry order (`FAT16`, then `ext`), each family's
  scenarios in their `all` order.
```

with:

```markdown
  display name in registry order (`FAT16`, then `ext`), each family's
  scenarios in their `all` order.
  *Amended 2026-09-25 by the family-tabs spec: no `<optgroup>`s: the picker lists `lessonsFor(tab)`, the active
  tab's lessons in their `all` order, defaulting to `defaultLessonFor(tab)`
  (its fundamentals), and keeps each tab's pick.*
```

(6) Replace:

```markdown
  /dev/hda (clears the timeline); --type picks fat16, ext2, or ext3`. Done
  line composed as in section 2.
```

with:

```markdown
  /dev/hda (clears the timeline); --type picks fat16, ext2, or ext3`. Done
  line composed as in section 2.
  *Amended 2026-09-25 by the family-tabs spec: `mkfs --type` accepts only the tab's family's types, and the flags
  are that family's alone (`--label` is no longer merged: `volume label, up
  to 11 characters` on FAT16). Summaries `Format /dev/hda (clears the
  timeline); --type picks fat16` and `…; --type picks ext2 or ext3`; the
  other family's type is `ShellError("'ext3' is an ext type: switch to the
  ext tab to format one", { help: "types: fat16" })` and its mirror on ext.*
```

(7) Replace:

```markdown
  `vfs` (so the working directory and prompt survive), then re-sets the
  prompt.
```

with:

```markdown
  `vfs` (so the working directory and prompt survive), then re-sets the
  prompt.
  *Amended 2026-09-25 by the family-tabs spec: one terminal follows the active tab: each tab has its own host
  and `vfs`, and a tab switch re-registers over that tab's, prints the tab
  banner, and re-sets the prompt from its cwd.*
```

(8) Replace:

```markdown
**`ext-fundamentals`, "The fundamentals (ext)".**
```

with:

```markdown
**`ext-fundamentals`, "The fundamentals (ext)".** *Amended 2026-09-25 by the family-tabs spec: the title is
`The fundamentals`: the ext tab's picker lists only ext lessons.*
```

(9) In the Outcome, replace:

```markdown
  panel, strings, timeline, diff, and terminal all work on it.
```

with:

```markdown
  panel, strings, timeline, diff, and terminal all work on it.
  *Amended 2026-09-25 by the family-tabs spec: from the ext tab's Format (ext2 / ext3) details; there is no Filesystem select.*
```


- [ ] **Step 5: Correct the family-tabs spec where the build found it wrong**

Each correction is italic and starts `Corrected during implementation:`, left beside the original text. In `docs/superpowers/specs/2026-09-25-family-tabs-design.md`:

(1) Replace:

```markdown
A lesson running in the target tab is closed (its disk is gone), as a same-tab load does today.
```

with:

```markdown
A lesson running in the target tab is closed (its disk is gone), as a same-tab load does today.
  *Corrected during implementation: a same-tab Load image does not close a running lesson today; only a routed load (`t !== from`) closes the one running in the target tab. On a routed load the keyboard moves to the target tab's Load image input (`load-image-{id}`), since the input it was on is now hidden.*
```

(2) At the end of the `OperationBar.svelte` bullet, where the op name appears, replace:

```markdown
under 760 px the summary wraps to a second line.
```

with:

```markdown
under 760 px the summary wraps to a second line. *Corrected during implementation: the bar prints the wasm package's op names (`1 of 1 · create_file /Hello world.txt · 7 sectors · 7 events`), not `createFile`.*
```

(3) Replace:

```markdown
A `Crash` phase's summary uses the `.warn` colour.
```

with:

```markdown
A `Crash` phase's summary uses the `.warn` colour.
    *Corrected during implementation: `groupEvents` also maps FAT's `dir_entry_deleted` and `directory_grown` to Directory entry and any family's `raw_write` to Data. Because phases follow first appearance, a FAT16 create reads Allocation, Data, Directory entry (its first event is a FAT entry). In an ext3-style record every non-journal kind, known or not, is Filesystem writes, so Other appears only in FAT-style records.*
```

(4) Replace:

```markdown
`tests/eventPhases.test.ts` (a FAT16 create: Directory entry, Allocation, Data;
```

with:

```markdown
`tests/eventPhases.test.ts` (a FAT16 create: Directory entry, Allocation, Data (*Corrected during implementation: Allocation, Data, Directory entry, by first appearance; `dir_entry_deleted`/`directory_grown` → Directory entry and `raw_write` → Data are pinned too*);
```

(5) Replace:

```markdown
Focus stays on the activated tab; on a routed Load image focus stays on the file input.
```

with:

```markdown
Focus stays on the activated tab; on a routed Load image focus stays on the file input. *Corrected during implementation: the input the image was loaded from is hidden with its tab, so focus moves to the target tab's Load image input (`load-image-{id}`, a per-tab id beside `action-path-{id}` and `load-hint-{id}`).*
```

- [ ] **Step 6: `docs/testing.md` lists the new tested modules**

Replace:

```markdown
virtual filesystem behavior, redirects, `dd`, `xxd`, theming, and layout
helpers. `pnpm --dir web/ui build` adds Svelte type-checking and a production
bundle gate for the component layer. The demo build checks the package from a
second consumer.
```

with:

```markdown
virtual filesystem behavior, redirects, `dd`, `xxd`, theming, layout
helpers, the tab route, per-tab scroll keeping and the maps' 0-width guard,
per-tab lesson lists, `mkfs` per tab and the terminal's tab switch, Load
image's family detection and routing, the adapters' one-line summaries, the
Operation bar's slider, and the What changed panel's sector ranges and event
phases. `pnpm --dir web/ui build` adds Svelte type-checking and a production
bundle gate for the component layer. The demo build checks the package from a
second consumer.
```

- [ ] **Step 7: The browser-pass fix: a routed Load image keeps the keyboard on Load image**

The pass (Step 9, item 15) found that loading an ext image from the FAT16 tab left focus on `BODY`: the input it was loaded from is inside `#workspace-fat16`, which is hidden as the ext tab activates, and a hidden element loses focus. Spec section 5 wants focus on the file input, so it moves to the target tab's own Load image input. In `web/ui/src/components/ActionsPanel.svelte`, replace:

```svelte
<script lang="ts">
  import { FAMILIES } from "../fs";
```

with:

```svelte
<script lang="ts">
  import { tick } from "svelte";
  import { FAMILIES } from "../fs";
```

replace:

```ts
    try {
      workspaces.loadImage(new Uint8Array(await file.arrayBuffer()), file.name, ws);
    } catch (err) {
      volume.report(err);
    }
  }
```

with:

```ts
    try {
      workspaces.loadImage(new Uint8Array(await file.arrayBuffer()), file.name, ws);
    } catch (err) {
      volume.report(err);
      return;
    }
    // This input's tab is hidden now, which drops its focus to the page: keep the keyboard on
    // Load image, in the tab that opened (mounted by the time `tick` resolves).
    if (workspaces.activeId !== ws.id) {
      await tick();
      document.getElementById(`load-image-${workspaces.activeId}`)?.focus();
    }
  }
```

and replace:

```svelte
      <input type="file" onchange={onLoadImage} aria-describedby="load-hint-{ws.id}" />
```

with:

```svelte
      <input id="load-image-{ws.id}" type="file" onchange={onLoadImage} aria-describedby="load-hint-{ws.id}" />
```

- [ ] **Step 8: Run the gates**

Run: `pnpm test`
Expected: `Test Files  57 passed (57)`, `Tests  556 passed (556)` (no test changes).

Run: `pnpm build`
Expected: svelte-check `COMPLETED 470 FILES 0 ERRORS 0 WARNINGS 0 FILES_WITH_PROBLEMS`, then `✓ built in …`.

Run (in `web/demo`, once; it is the wasm package's second consumer and nothing in it changes): `CI=true pnpm install --frozen-lockfile && pnpm build`
Expected: `dist/assets/fs_emulator_wasm_bg-….wasm  441.50 kB`, then `✓ built in …`.

`git status --short` shows only the eight files above (never `pnpm-lock.yaml` or `pnpm-workspace.yaml`, nothing under `crates/` or `web/demo/`).

- [ ] **Step 9: The browser pass (spec section 8), at 1440 × 900, then 1100 and 760 px**

Setup:
- From `web/ui`: `pnpm exec vite --port 5197 --strictPort` (Bash, background; any free port). Open `http://localhost:5197/` with `preview_start` by URL and `resize_window` to 1440 × 900.
- The ext3 image needs e2fsprogs' `mke2fs` (the root README's prerequisite): the Homebrew path is macOS's, where the keg-only formula (`brew install e2fsprogs`) puts it at `$(brew --prefix e2fsprogs)/sbin/mke2fs` (`/opt/homebrew/opt/e2fsprogs/sbin/mke2fs` on Apple Silicon); on Linux it is `mke2fs` on PATH. In any temporary directory outside the repo: `dd if=/dev/zero of=mke2fs-ext3.img bs=1048576 count=16`, then `<mke2fs> -q -F -t ext3 -b 1024 -I 128 -O none,has_journal,filetype,sparse_super -J size=1 mke2fs-ext3.img` (it warns that 128-byte inodes cannot handle dates beyond 2038; fine). Copy it to `web/ui/node_modules/.t7/mke2fs-ext3.img` (ignored by git) so the page can `fetch('/node_modules/.t7/mke2fs-ext3.img')`; delete that folder afterwards.
- If the Browser pane is not on screen (`document.visibilityState` is `hidden`), `computer` clicks and screenshots fail and the page runs no `requestAnimationFrame` or `ResizeObserver` callbacks. Then drive everything with `javascript_exec` (`button.click()`, `dispatchEvent(new KeyboardEvent('keydown', { key, bubbles: true }))`, `input.value = …` plus a bubbling `input` event), and after every page load run `window.requestAnimationFrame = (fn) => setTimeout(() => fn(performance.now()), 16)` so the scroll restore, the canvases' repaint, and xterm's renderer run. Canvases then keep the width they measured at load, so a horizontal-scroll check must hide the ribbon canvas first (see items 16 and 17). Type into the terminal by focusing `#terminal-drawer .xterm-helper-textarea`, dispatching `new InputEvent('input', { data: 'mkfs --type fat16', inputType: 'insertText', bubbles: true })`, then a `keydown` with `key: 'Enter', keyCode: 13`; read it from `#terminal-drawer .xterm-rows > div`. Canvas outlines are checked by diffing `getImageData` before and after a hover (the canvases are 2× device pixels).

Expected (as observed):

1. Bare URL: hash `#fat16`, title `fs explorer · FAT16`, only `#workspace-fat16` in the DOM; `#tab-fat16` `aria-selected="true"`, `tabindex=0`, `aria-controls="workspace-fat16"`; `#tab-ext` `tabindex=-1`, no `aria-controls`. The picker lists 9 lessons with `The fundamentals` selected. The bar reads `FAT16 · 16 MB · 512-byte sectors · 2 KiB clusters · Run an action to see what it changes.`; What changed reads `Nothing yet.`
2. Add file (path `/Hello world.txt`): the bar reads `1 of 1 · create_file /Hello world.txt · 7 sectors · 7 events`; What changed shows `Sectors` then `1` `33` `65` `97–100`, then `Allocation · 3 events` (open), `Data · 1 event`, `Directory entry · 3 events` (closed); the slider's `list` is `opbar-ticks-fat16` with 1 option.
3. Hover `97–100` (`mouseenter`): the ribbon canvas changes in device columns 6 to 11 only and is restored on `mouseleave`; the FAT map does not change, because the selected `/Hello world.txt`'s chain is already drawn in `--focus`. Set the path to `/b.txt` and Add file: `2 of 2 · create_file /b.txt · 7 sectors · 5 events`, ranges `1` `33` `65` `101–104`, `Allocation · 3 events`, `Data · 1 event`, `Directory entry · 1 event`. Press `[` (`1 of 2 · create_file /Hello world.txt · …`) and hover `97–100`: now the FAT map changes in device pixels x 0 to 11, y 0 to 11 (cluster 2's cell outlined) and is restored on leave. Click `97–100`: the Inspector reads `Offset 0xc200 · 49,664`, `Sector 97 · data`, `Cluster 2 · FAT: end of chain`, owner `/Hello world.txt`.
4. The slider (866 px wide): a `pointermove` 2 px from its left end sets its title to `1 of 2 · create_file /Hello world.txt`, 2 px from its right end `2 of 2 · create_file /b.txt`; after `pointerleave` it names the current step again.
5. `]` returns to `2 of 2 · create_file /b.txt · 7 sectors · 5 events`. Start `The fundamentals` and press `n` twice: the card reads `Lesson · step 3 of 12`, `The fundamentals`, `Where each region starts`; the bar `1 of 1 · create_file /HELLO.TXT · 7 sectors · 5 events`. Scroll the FAT16 dump (`.hexview`) to 2400.
6. Open the terminal with the Terminal button: the pane shows the library's greeting and `/mnt ❯`.
7. Click `ext`: `#workspace-ext` is created with computed `display: contents` and `#workspace-fat16` is `hidden` (`display: none`); hash `#ext`, title `fs explorer · ext`, focus on `#tab-ext`. Files: `/` (`first block 69`) and `lost+found 12,288 B · first block 70`. Column headings in order: `Files`, `Block groups · 2 groups · 16,384 blocks`, `Actions`, `What changed`, `Journal · ordered mode`, `Strings`, `At this byte`. The bar reads `ext3 · 16 MB · 16,384 1 KiB blocks in 2 groups · 1,024-block journal, ordered mode · Run an action to see what it changes.`; the picker lists 3 lessons with `The fundamentals` selected; no lesson card; the `lesson` badge (`lesson running` to a screen reader) is on `FAT16` only. The ribbon's legend lists `journal`, and its pixel at device x = 100 is amber `rgb(212, 160, 23)`. The terminal's last lines are `-- ext tab: /dev/hda is ext3 --` and `/mnt ❯`.
8. Add file on ext: `1 of 1 · create_file /Hello world.txt · 18 blocks · 30 events`; `Blocks` `1–6` `69` `82–91` `1111`; `Filesystem writes · 10 events` open, `Journal · 11 events`, `Checkpoint · 7 events`, `Cleanup · 2 events` closed.
9. Start ext's `The fundamentals` and press `n` to the end: 13 steps (`step 13 of 13`), and the card's buttons are Prev, Finish, Close. The bar reads `1 of 1 · create_file /hello.txt; create_file /bigger.txt · 42 blocks · 66 events`, with `Filesystem writes · 24 events` (open), `Journal · 23 events`, `Checkpoint · 15 events`, `Cleanup · 4 events`.
10. Crash and recover, with the lesson still open: in the Journal panel pick `after commit` and press Arm (`Armed: the next change stops after commit`, a Disarm button); Add file `/crash.txt`: `2 of 2 · create_file /crash.txt (crashed after commit) · 12 blocks · 22 events`, ranges `1` `82` `102–110` `1126`, `Filesystem writes · 10 events` (open), `Journal · 11 events`, `Crash · 1 event` in `rgb(250, 199, 117)` (`--warn`, `#fac775`); the status line reads `Volume needs recovery`. Press Recover: `3 of 3 · recover · 8 blocks · 10 events`, ranges `1–6` `69` `82`, `Recovery · 8 events` (open), `Cleanup · 2 events`; the status line is empty.
11. Scroll the ext dump to 1500 and the block-group map (`.bgmap-wrap`, 1,922 px of scroll) to 700. Click `FAT16`: hash `#fat16`, title `fs explorer · FAT16`, focus on `#tab-fat16`; the card is back at `step 3 of 12`; the FAT16 dump is at 2400 (the hidden ext dump reads 0); both tabs carry the `lesson` badge; the picker shows `The fundamentals`; the terminal adds `-- FAT16 tab: /dev/hda is FAT16 --` and `/mnt ❯`. Click `ext` again: its dump is at 1500, the map at 700, the map canvas still 198 px wide, and its card at `step 13 of 13`.
12. On ext, type `mkfs --type fat16` in the terminal: ``error: `mkfs`: 'fat16' is a FAT16 type: switch to the FAT16 tab to format one``, the caret line under `mkfs`, `help: types: ext2, ext3`, then `/mnt ❯`.
13. With focus on `#tab-ext`: `ArrowRight` → FAT16 (hash `#fat16`, title `fs explorer · FAT16`, focus on `#tab-fat16`); `ArrowRight` → ext; `ArrowLeft` → FAT16; `Home` → FAT16; `End` → ext; each switch moves the hash, the title, and focus together. Blur, then `/` focuses `#action-path-ext`. Blur, then `[` on ext gives `2 of 3 · create_file /crash.txt (crashed after commit) · 12 blocks · 22 events`, `[` again `1 of 3 · create_file /hello.txt; create_file /bigger.txt · 42 blocks · 66 events`, `]` `]` back to `3 of 3 · recover · 8 blocks · 10 events`; the FAT16 bar still reads `1 of 1 · create_file /HELLO.TXT · 7 sectors · 5 events`.
14. Reload on `#ext`: only `#workspace-ext` exists, hash `#ext`, title `fs explorer · ext`, the bar shows the fresh ext3 summary, no badge, no card, `#tab-fat16` has no `aria-controls`.
15. Reload the bare URL (only `#workspace-fat16`), Add file, then put `new File([bytes], 'mke2fs-ext3.img')` (16,777,216 bytes) into a `DataTransfer`, assign its `files` to `#load-image-fat16` (the Load image input, `aria-describedby="load-hint-fat16"`, hint `FAT16 and ext images each open in their own tab.`; not the "Use a file's bytes" input above it), focus it, and dispatch a bubbling `change`. The ext tab is created and shown (`#workspace-fat16` hidden), hash `#ext`, title `fs explorer · ext`; the status line reads `Opened mke2fs-ext3.img in the ext tab: it is an ext3 image. The FAT16 tab is as you left it.`; ext's Files read `/ 0 B · first block 261` and `lost+found 12,288 B · first block 262`, the headings as in item 7, and the bar the same ext3 summary line; focus is on `#load-image-ext` (before Step 7's fix it was `BODY`); the hidden FAT16 bar still reads `1 of 1 · create_file /Hello world.txt · 7 sectors · 7 events`.
16. At 1100 × 900 (ext tab, notice on the status line): the top bar is one row, `h1` at (8, 11), the tab bar at (104, 8) 115 × 29, Terminal at (234, 8), the picker at (323, 8), the theme switch at (996, 13); the status line is its own row at y = 43, 1,084 px wide; the bar at y = 70 is 64 px tall (controls at y = 79, the summary at y = 109); What changed heads the right column, which sits under the left one at (8, 572). With the ribbon canvas hidden, `document.documentElement.scrollWidth` is 1100 (1,421 with it, the width it measured at 1440 while the pane was off screen).
17. At 760 × 900: `h1` (8, 8) and the theme switch (656, 10) share the first row; the tab bar is a full row at y = 38 with the two tabs 368 × 36 px each at x = 11 and x = 381; Terminal (8, 86) and the picker (97, 86) the next row; the status line at y = 121. On ext the bar starts at y = 147 (controls y = 157, the summary y = 186, itself two lines, 38 px); on FAT16 it starts at y = 129 (controls y = 138, the summary `1 of 1 · create_file /Hello world.txt · 7 sectors · 7 events` one line at y = 167). With the ribbon canvas hidden, `scrollWidth` is 760.
18. `read_console_messages` at the end, filtered to drop `[debug] [vite]` lines, the `Canvas2D … willReadFrequently` warnings (from the pass's own `getImageData` calls), and the `ShellError: 'fat16' is a FAT16 type: …` entry that browser-terminal logs for the refused command in item 12 (plus, if you tried one, a harness `fetch` of your own that failed): no errors or warnings remain from the app.

Reset the viewport (`resize_window` preset `desktop`), stop the dev server, close the browser tab, and delete `web/ui/node_modules/.t7`.

- [ ] **Step 10: Commit**

```sh
git add docs/ROADMAP.md docs/testing.md docs/superpowers/specs/2026-09-24-ext-explorer-design.md docs/superpowers/specs/2026-09-25-family-tabs-design.md web/ui/README.md web/ui/src/components/ActionsPanel.svelte web/ui/src/scenarios/fundamentals.ts web/ui/src/scenarios/journaledWrite.ts
git commit -m "docs: describe the family tabs, the Operation bar, and What changed" -m "Also fixes: a routed Load image dropped focus to the page (its input's tab is hidden), found by the browser pass; focus now moves to the target tab's Load image input (load-image-{id}). web/ui/README.md's Panes section describes the two tabs as workspaces, the per-tab columns, the tab-bound Format details and Load image routing, the per-tab lessons, and the Operation bar and What changed panel in place of the Step strip and the Timeline; the Terminal section, the shortcuts, and the filesystem-specific notes follow. The ext explorer spec gets its Outcome and section 1, 5, 6, and 7 amendments as notes, the family-tabs spec five corrections found while building it (the routed-load lesson and focus, the op names, the phase mapping), docs/testing.md the newly tested modules, and the ROADMAP a family-tabs entry with the per-tab ids and the deferred notes it leaves. Two lessons that named the Step strip now name the What changed panel."
```

