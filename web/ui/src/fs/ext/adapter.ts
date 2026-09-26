import { Volume, type Annotation, type ExtBlockOwner, type ExtFileBlocks, type ExtGeometry, type ExtIndirectBlock, type ExtInode, type ExtSuperblock, type JournalInfo, type Region } from "../../lib/wasm";
import type { Interval } from "../../core/intervals";
import { COLOR_JOURNAL } from "../../core/palette";
import type { ByteChangeLike } from "../../core/patch";
import { megabytes, unitSizeLabel, type DfFacts, type FsAdapter, type FsFamily, type FsFamilyId, type StatFacts, type TraceRow, type UnitOwner, type UnitSpace } from "../adapter";
import { SpaceAdapter, hexAddr } from "../base";
import { MKFS, toWasmOptions, type ExtFamilyOptions } from "./format";
import { extSpace } from "./geometry";
import { ExtJournal } from "./journal";
import { CORRUPT_NOTE, NOTES, touchesMetadata as changesTouchMetadata } from "./metadata";

/** The path `blockOwners()` gives the journal's blocks (inode 8's data and pointer blocks), and
 *  the owner of the pointer-block rows in `owners`: a pseudo-owner (`isPseudoOwner`). */
export const JOURNAL_OWNER = "<journal>";

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
const isPathRow = (r: ExtBlockOwner): r is PathRow => r.path !== JOURNAL_OWNER && r.role !== "journal";

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
 * Every journal block is in `journalBlocks`; the ones inside `journal` regions never become
 * units, and the pointer blocks (in a data region) stay in `owners` as `<journal>` rows.
 * The unit arithmetic, `ownerOf`, and `regionStart` are the shared `SpaceAdapter`'s (fs/base.ts).
 */
export class ExtAdapter extends SpaceAdapter implements FsAdapter {
  readonly id: FsFamilyId = "ext";
  /** The bound volume's type: "ext3" with a journal, else "ext2". */
  name: "ext2" | "ext3" = "ext3";
  readonly family: FsFamily<ExtFamilyOptions> = ext;
  readonly vol: Volume;
  geo!: ExtGeometry;
  sb!: ExtSuperblock;
  /** The directory, data, and indirect blocks of every path, in block order, and the journal's
   *  pointer blocks as `<journal>` rows in `COLOR_JOURNAL`; never the journal's data blocks. */
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
    // On ext a region's sectors are blocks (the disk's sector is the block; see extSpace).
    this.owners = toUnitOwners(rows, (path) => this.blocksOf(path)?.data[0], this.vol.layout().filter((r) => r.kind === "journal"));
    this.byUnit = new Map(this.owners.map((o) => [o.unit, o]));
    this.indirectBlocks = this.owners.filter((o) => o.role === "indirect").map((o) => o.unit);
    this.journalBlocks = rows.filter((r) => r.path === JOURNAL_OWNER).map((r) => r.block);
    this.journalSet = new Set(this.journalBlocks);
    this.info = this.vol.journalInfo();
    this.needsRecovery = this.vol.needsRecovery();
    this.journal = this.info ? (this.journal ?? new ExtJournal(this.vol, () => this.journalInfo())) : undefined;
  }

  /** "ext3 · 16 MB · 16,384 1 KiB blocks in 2 groups · 1,024-block journal, ordered mode" (or
   *  "· no journal" on ext2), from the cached geometry and journal header. */
  summary(): string {
    const g = this.geo;
    const n = (v: number) => v.toLocaleString("en-US");
    const groups = `${g.groups.length} ${g.groups.length === 1 ? "group" : "groups"}`;
    const journal = this.info ? `${n(this.info.maxlen)}-block journal, ${this.info.mode} mode` : "no journal";
    return `${this.name} · ${megabytes(g.totalBlocks * g.blockSize)} · ${n(g.totalBlocks)} ${unitSizeLabel(g.blockSize)} blocks in ${groups} · ${journal}`;
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

  /** The superblock's free count, not the blocks without an owner row: those include the inode
   *  tables, the bitmaps, and the journal. */
  freeUnits(): number {
    return this.df().free;
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
  id: "ext",
  name: "ext",
  fsTypes: ["ext2", "ext3"],
  format: (options) => {
    const w = toWasmOptions(options ?? {});
    return w.variant === "ext2" ? Volume.formatExt2(w.options) : Volume.formatExt3(w.options);
  },
  bind: (vol) => new ExtAdapter(vol),
  mkfs: MKFS,
};
