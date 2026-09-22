export type ColorIndex = number;
export const FILE_HUE_COUNT = 6;
export const COLOR_FREE = 0, COLOR_BOOT = 1, COLOR_FAT = 2, COLOR_DIR = 3, COLOR_FILE_BASE = 4;
/** The second FAT copy: the same violet as `COLOR_FAT`, a shade lighter, so the two
 *  copies read as related but are told apart at a glance in the ribbon and dump. */
export const COLOR_FAT_ALT = 10;

/** FNV-1a over the path so a file keeps its hue across renders and sessions. */
export function colorIndexForPath(path: string): ColorIndex {
  let h = 0x811c9dc5;
  for (let i = 0; i < path.length; i++) { h ^= path.charCodeAt(i); h = Math.imul(h, 0x01000193) >>> 0; }
  return COLOR_FILE_BASE + (h % FILE_HUE_COUNT);
}

export function cssVarForColor(index: ColorIndex): string { return `var(--own-${index})`; }
