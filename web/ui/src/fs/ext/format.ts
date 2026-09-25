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
