/**
 * Narrow test for the one wasm error code the tree/direntry/remnant helpers are allowed
 * to swallow: a raw write left the boot sector unparsable (`Volume.corruption()` is set
 * and every path-based method throws `CorruptImage`). Every other error must still
 * propagate — this helper exists so "swallow only this" stays a single, reusable check
 * instead of ad hoc code comparisons scattered across `src/core/*.ts`.
 */
export function isCorrupt(e: unknown): boolean {
  return typeof e === "object" && e !== null && (e as { code?: unknown }).code === "CorruptImage";
}
