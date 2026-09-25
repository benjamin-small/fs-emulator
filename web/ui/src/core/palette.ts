export type ColorIndex = number;
export const FILE_HUE_COUNT = 6;
export const COLOR_FREE = 0, COLOR_BOOT = 1, COLOR_TABLE = 2, COLOR_DIR = 3, COLOR_FILE_BASE = 4;
/** A second copy of the allocation table (FAT's mirror): the same violet as `COLOR_TABLE`, a
 *  shade lighter, so the two copies read as related but are told apart at a glance in the
 *  ribbon and dump. */
export const COLOR_TABLE_ALT = 10;

/** A journal's blocks: a warm amber apart from every file hue and from the table violet, so
 *  "written twice, once here first" reads as its own thing in the ribbon, the dump, and the map.
 *  The rule its `--own-11` values keep (tests/theme.test.ts checks it): in each theme, at least
 *  25° of hue or 20 % of HSL lightness from every file hue (`--own-4`..`--own-9`) and from the
 *  table violets (`--own-2`, `--own-10`). Dark `#d4a017` (hue 44°, lightness 46 %: 28° from
 *  the salmon `--own-5`, 26 % darker than the pale amber `--own-9`); light `#b7791f` (hue 36°,
 *  lightness 42 %: 29 % darker than `--own-5`, 30 % darker than `--own-9`). */
export const COLOR_JOURNAL = 11;

/** FNV-1a over the path so a file keeps its hue across renders and sessions. */
export function colorIndexForPath(path: string): ColorIndex {
  let h = 0x811c9dc5;
  for (let i = 0; i < path.length; i++) { h ^= path.charCodeAt(i); h = Math.imul(h, 0x01000193) >>> 0; }
  return COLOR_FILE_BASE + (h % FILE_HUE_COUNT);
}

export function cssVarForColor(index: ColorIndex): string { return `var(--own-${index})`; }
