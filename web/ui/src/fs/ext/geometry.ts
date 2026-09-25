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
