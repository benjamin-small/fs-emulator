import type { FatEntry } from "../../lib/wasm";

export type ClusterState = "free" | "used" | "end" | "bad" | "reserved";

export function clusterState(e: FatEntry): ClusterState {
  switch (e.kind) {
    case "free": return "free";
    case "next": return "used";
    case "endOfChain": return "end";
    case "bad": return "bad";
    default: return "reserved";
  }
}

/** Clusters in chain order starting at `first`; [] if the start is not an allocated cluster. */
export function buildChain(fat: FatEntry[], first: number): number[] {
  const out: number[] = [];
  let c = first;
  while (c >= 2 && c < fat.length && out.length < fat.length) {
    const e = fat[c];
    if (e.kind !== "next" && e.kind !== "endOfChain") break;
    out.push(c);
    if (e.kind === "endOfChain") break;
    c = e.cluster;
  }
  return out;
}

/** One FAT entry in words, as the Inspector's "At this byte" card prints it after "FAT: ". */
export function describeFatEntry(e: FatEntry): string {
  switch (e.kind) {
    case "free": return "free";
    case "next": return `next → ${e.cluster}`;
    case "endOfChain": return "end of chain";
    case "bad": return "bad";
    case "reserved": return "reserved";
  }
}
