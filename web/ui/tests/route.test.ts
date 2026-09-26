import { describe, expect, it } from "vitest";
import { formatRoute, parseRoute } from "../src/core/route";

const KNOWN = ["fat16", "ext"] as const;
const ALIASES = { fat: "fat16", ext2: "ext", ext3: "ext" } as const;

describe("parseRoute", () => {
  it("reads a known name after the hash, in any case", () => {
    expect(parseRoute("#fat16", KNOWN)).toBe("fat16");
    expect(parseRoute("#ext", KNOWN)).toBe("ext");
    expect(parseRoute("#EXT", KNOWN)).toBe("ext");
    expect(parseRoute("ext", KNOWN)).toBe("ext"); // the hash itself is optional
  });

  it("strips one leading and one trailing slash", () => {
    expect(parseRoute("#/ext", KNOWN)).toBe("ext");
    expect(parseRoute("#ext/", KNOWN)).toBe("ext");
    expect(parseRoute("#/fat16/", KNOWN)).toBe("fat16");
    expect(parseRoute("#//ext", KNOWN)).toBeNull();
    expect(parseRoute("##ext", KNOWN)).toBeNull();
  });

  it("looks the name up in the aliases before the known names", () => {
    expect(parseRoute("#fat", KNOWN, ALIASES)).toBe("fat16");
    expect(parseRoute("#ext2", KNOWN, ALIASES)).toBe("ext");
    expect(parseRoute("#EXT3", KNOWN, ALIASES)).toBe("ext");
    expect(parseRoute("#ext3", KNOWN)).toBeNull(); // no aliases given, none apply
    expect(parseRoute("#fat16", KNOWN, { fat16: "ext" })).toBe("ext");
  });

  it("answers null for an empty hash, an unknown name, or an inherited key", () => {
    expect(parseRoute("", KNOWN, ALIASES)).toBeNull();
    expect(parseRoute("#", KNOWN, ALIASES)).toBeNull();
    expect(parseRoute("#/", KNOWN, ALIASES)).toBeNull();
    expect(parseRoute("#ntfs", KNOWN, ALIASES)).toBeNull();
    expect(parseRoute("#constructor", KNOWN, ALIASES)).toBeNull();
    expect(parseRoute("#toString", KNOWN, ALIASES)).toBeNull();
  });
});

describe("formatRoute", () => {
  it("writes the id after a hash, and parseRoute reads it back", () => {
    expect(formatRoute("ext")).toBe("#ext");
    expect(formatRoute("fat16")).toBe("#fat16");
    for (const id of KNOWN) expect(parseRoute(formatRoute(id), KNOWN, ALIASES)).toBe(id);
  });
});
