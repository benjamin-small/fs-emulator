import { describe, expect, it } from "vitest";
import { buildChain, clusterState } from "../src/fs/fat16/fatchain";
import type { FatEntry } from "../src/lib/wasm";

const fat: FatEntry[] = [
  { kind: "reserved" }, { kind: "reserved" },
  { kind: "next", cluster: 3 }, { kind: "next", cluster: 4 }, { kind: "endOfChain" },
  { kind: "free" }, { kind: "next", cluster: 7 }, { kind: "next", cluster: 6 }, { kind: "bad" },
];

describe("fatchain", () => {
  it("follows a chain to its end", () => { expect(buildChain(fat, 2)).toEqual([2, 3, 4]); });
  it("stops on a loop without hanging", () => { expect(buildChain(fat, 6).length).toBeLessThanOrEqual(fat.length); });
  it("returns [] for a free or invalid start", () => {
    expect(buildChain(fat, 5)).toEqual([]);
    expect(buildChain(fat, 0)).toEqual([]);
    expect(buildChain(fat, 99)).toEqual([]);
  });
  it("classifies entries", () => {
    expect(clusterState({ kind: "free" })).toBe("free");
    expect(clusterState({ kind: "next", cluster: 9 })).toBe("used");
    expect(clusterState({ kind: "endOfChain" })).toBe("end");
    expect(clusterState({ kind: "bad" })).toBe("bad");
  });
});
