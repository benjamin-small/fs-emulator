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
  if (typeof focus.path === "string") parts.push(`Files: ${focus.path}, its entry, chain, and ${space.unit.plural}`);
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
