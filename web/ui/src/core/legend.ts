import type { Region } from "../lib/wasm";

/**
 * The Ribbon legend's label for a metadata region. The boot area (FAT's `reserved (boot
 * sector)`, ext's `boot block`) reads `boot` and FAT's fixed `root directory` reads `root`;
 * every other region keeps its name, the ext superblock copies included, which share the
 * `boot` kind with the boot block but are not it.
 */
export function metaLabel(r: Region): string {
  if (r.kind === "boot" && /\bboot\b/.test(r.name)) return "boot";
  if (r.kind === "directory") return "root";
  return r.name;
}
