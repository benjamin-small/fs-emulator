import { describe, expect, it } from "vitest";
import { createCommands } from "../../src/shell/commands";
import { makeHost } from "./helpers";

/**
 * Pins the help text the terminal composes from the mounted family today: df's summary (the
 * unit noun, capitalised), seek's and the write/xxd address flags' address-form text (from
 * `addrHelp`), and the six `mkfs` flag descriptions (`FsFamily.mkfs.flags`, fat16's own
 * words). The literals here are typed out, not computed from `addrHelp` or the adapter, so a
 * change to any of those seams shows up as a diff against a fixed string, not silently.
 */
describe("the composed help text the terminal registers", () => {
  const host = makeHost();
  const defs = createCommands(host);
  const spec = (name: string) => defs.find((d) => d.spec.name === name)!.spec;

  it("pins df's summary", () => {
    expect(spec("df").summary).toBe("Cluster usage of the mounted volume");
  });

  it("pins seek's addr argument description", () => {
    expect(spec("seek").required?.[0]?.desc).toBe("0x1f, 512, s:65 (sector), or c:3 (cluster)");
  });

  it("pins write's --at flag description", () => {
    const at = spec("write").flags?.find((f) => f.long === "at");
    expect(at?.desc).toBe("disk address for /dev/hda (addresses: 0x1f (hex), 512 (decimal), s:65 (sector), c:3 (cluster))");
  });

  it("pins xxd's --offset flag description", () => {
    const offset = spec("xxd").flags?.find((f) => f.long === "offset");
    expect(offset?.desc).toBe("start address (addresses: 0x1f (hex), 512 (decimal), s:65 (sector), c:3 (cluster))");
  });

  it("pins each of mkfs's six flag descriptions", () => {
    const flags = spec("mkfs").flags ?? [];
    const desc = (long: string) => flags.find((f) => f.long === long)?.desc;
    expect(desc("sectors")).toBe("total sectors (default 32768 = 16 MB)");
    expect(desc("spc")).toBe("sectors per cluster (default 4)");
    expect(desc("label")).toBe("volume label, up to 11 characters");
    expect(desc("root-entries")).toBe("root directory entries (default 512)");
    expect(desc("fats")).toBe("FAT copies (default 2)");
    expect(desc("reserved")).toBe("reserved sectors (default 1)");
  });
});
