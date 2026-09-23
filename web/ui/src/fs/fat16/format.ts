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
