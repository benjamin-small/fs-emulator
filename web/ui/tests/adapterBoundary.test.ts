import { readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

// The adapter seam, enforced as a source scan (spec 2026-09-23-fs-adapter-design.md, section 6).
// Outside src/fs/fat16/ and src/lib/wasm.ts nothing may call a FAT-only wasm method or import a
// FAT-only wasm type, and only the registry, the panel table and the scenarios may import from
// fs/fat16. The scan is textual, like layout.test.ts: a comment that spells `vol.geometry()`
// trips it too, so reword the comment rather than the rule.
const SRC = fileURLToPath(new URL("../src/", import.meta.url));

/** Files the FAT-only wasm surface is allowed in. */
const FAT_ONLY_ALLOWED = (rel: string) => rel.startsWith("fs/fat16/") || rel === "lib/wasm.ts";
/** Files that may import from fs/fat16 (besides fs/fat16 itself). */
const FAT16_IMPORTERS = (rel: string) => rel === "fs/index.ts" || rel === "fs/panels.ts" || rel.startsWith("scenarios/");

const FAT_ONLY_CALL = /\.(geometry|fatEntries|clusterOwners|annotateSectorWith|rawDirEntries|bootSector|clusterChain|formatFat16)\s*\(/g;
const FAT_ONLY_TYPE = /\b(ClusterOwner|FatEntry|Geometry|RawEntry|BootSector|FormatOptions)\b/;
const WASM_IMPORT = /(?:import\s+(?:type\s+)?|export\s+type\s+)\{([^}]*)\}\s+from\s+["'](?:[^"']*lib\/wasm|fs-emulator-wasm)["']/g;
const FAT16_IMPORT = /from\s+["'](?:\.{1,2}\/)+(?:fs\/)?fat16(?:\/[^"']*)?["']/g;

/** Every .ts and .svelte file under `dir`, as posix paths relative to src/, sorted. */
function walk(dir: string, rel = ""): string[] {
  const out: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const child = rel ? `${rel}/${entry.name}` : entry.name;
    if (entry.isDirectory()) out.push(...walk(join(dir, entry.name), child));
    else if (/\.(ts|svelte)$/.test(entry.name)) out.push(child);
  }
  return out.sort();
}

/** Every rule `text` breaks for a file at `rel` (posix path under src/), as "rel: what". */
export function violations(rel: string, text: string): string[] {
  const found: string[] = [];
  if (!FAT_ONLY_ALLOWED(rel)) {
    for (const m of text.matchAll(FAT_ONLY_CALL)) found.push(`${rel}: calls .${m[1]}( (FAT-only wasm method; go through the adapter)`);
    for (const m of text.matchAll(WASM_IMPORT)) {
      const t = FAT_ONLY_TYPE.exec(m[1]);
      if (t) found.push(`${rel}: imports ${t[1]} from lib/wasm (FAT-only wasm type; use the fs/adapter types)`);
    }
  }
  if (!rel.startsWith("fs/fat16/") && !FAT16_IMPORTERS(rel)) {
    for (const m of text.matchAll(FAT16_IMPORT)) found.push(`${rel}: imports from fs/fat16 (${m[0].trim()}); only fs/index.ts, fs/panels.ts and scenarios/ may`);
  }
  return found;
}

describe("the adapter boundary", () => {
  const files = walk(SRC);

  it("scans the tree it is meant to guard", () => {
    expect(files).toContain("App.svelte");
    expect(files).toContain("shell/commands.ts");
    expect(files).toContain("state/volume.svelte.ts");
    expect(files).toContain("fs/fat16/adapter.ts");
    expect(files.length).toBeGreaterThan(50);
  });

  it("recognises each kind of leak (positive controls)", () => {
    expect(violations("state/volume.svelte.ts", "const g = vol.geometry();")).toEqual([
      "state/volume.svelte.ts: calls .geometry( (FAT-only wasm method; go through the adapter)",
    ]);
    expect(violations("shell/host.ts", 'import { Volume, type FormatOptions } from "../lib/wasm";')).toEqual([
      "shell/host.ts: imports FormatOptions from lib/wasm (FAT-only wasm type; use the fs/adapter types)",
    ]);
    expect(violations("shell/host.ts", 'export type { Geometry } from "../lib/wasm";')).toEqual([
      "shell/host.ts: imports Geometry from lib/wasm (FAT-only wasm type; use the fs/adapter types)",
    ]);
    expect(violations("shell/host.ts", 'import type { FatEntry } from "fs-emulator-wasm";')).toEqual([
      "shell/host.ts: imports FatEntry from lib/wasm (FAT-only wasm type; use the fs/adapter types)",
    ]);
    expect(violations("core/tree.ts", 'import type {\n  ClusterOwner,\n  Volume,\n} from "../lib/wasm";')).toHaveLength(1);
    expect(violations("components/Inspector.svelte", 'import { asFat16 } from "../fs/fat16";')).toEqual([
      'components/Inspector.svelte: imports from fs/fat16 (from "../fs/fat16"); only fs/index.ts, fs/panels.ts and scenarios/ may',
    ]);
    expect(violations("state/layers.svelte.ts", 'import { buildChain } from "../fs/fat16/fatchain";')).toHaveLength(1);
    // The allowed places, and the generic surface, pass.
    expect(violations("fs/fat16/adapter.ts", 'this.geo = vol.geometry(); import type { Geometry } from "../../lib/wasm";')).toEqual([]);
    expect(violations("lib/wasm.ts", 'export type { Geometry } from "fs-emulator-wasm";')).toEqual([]);
    expect(violations("fs/index.ts", 'import { fat16 } from "./fat16";')).toEqual([]);
    expect(violations("fs/panels.ts", 'import FatMap from "./fat16/FatMap.svelte";')).toEqual([]);
    expect(violations("scenarios/fundamentals.ts", 'import { asFat16 } from "../fs/fat16";')).toEqual([]);
    expect(violations("shell/commands.ts", "vol.annotateSector(3); vol.layout(); vol.corruption(); vol.fsType();")).toEqual([]);
    expect(violations("state/volume.svelte.ts", 'import { Volume, type FsError, type OpRecord, type Region } from "../lib/wasm";')).toEqual([]);
  });

  it("keeps FAT-only wasm calls, FAT-only wasm types and fs/fat16 imports out of the generic code", () => {
    const found = files.flatMap((rel) => violations(rel, readFileSync(join(SRC, ...rel.split("/")), "utf8")));
    expect(found).toEqual([]);
  });
});
