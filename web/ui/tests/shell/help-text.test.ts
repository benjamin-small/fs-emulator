import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { createCommands } from "../../src/shell/register";
import { makeHost } from "./helpers";

/**
 * Pins the help text the terminal composes from the mounted family today: df's summary (the
 * unit noun, capitalised), seek's and the write/xxd address flags' address-form text (from
 * `addrHelp`), and `mkfs`'s summary and flags (the tab's own family's `FsFamily.mkfs.flags`
 * after `--type`, which lists only that family's types). The literals here are typed
 * out, not computed from `addrHelp` or the adapter, so a
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

  it("pins each of FAT16's six mkfs flag descriptions, --label its own", () => {
    const flags = spec("mkfs").flags ?? [];
    const desc = (long: string) => flags.find((f) => f.long === long)?.desc;
    expect(desc("sectors")).toBe("total sectors (default 32768 = 16 MB)");
    expect(desc("spc")).toBe("sectors per cluster (default 4)");
    expect(desc("label")).toBe("volume label, up to 11 characters");
    expect(desc("root-entries")).toBe("root directory entries (default 512)");
    expect(desc("fats")).toBe("FAT copies (default 2)");
    expect(desc("reserved")).toBe("reserved sectors (default 1)");
  });

  it("pins mkfs's summary, --type, and FAT16's flags only, in their order after --type", () => {
    const mkfs = spec("mkfs");
    expect(mkfs.summary).toBe("Format /dev/hda (clears the timeline); --type picks fat16");
    expect(mkfs.flags).toEqual([
      { long: "type", shape: "str", desc: "fat16 (default: the mounted volume's type)" },
      { long: "sectors", shape: "int", desc: "total sectors (default 32768 = 16 MB)" },
      { long: "spc", shape: "int", desc: "sectors per cluster (default 4)" },
      { long: "label", shape: "str", desc: "volume label, up to 11 characters" },
      { long: "root-entries", shape: "int", desc: "root directory entries (default 512)" },
      { long: "fats", shape: "int", desc: "FAT copies (default 2)" },
      { long: "reserved", shape: "int", desc: "reserved sectors (default 1)" },
    ]);
    // Not the same list on an ext host: each tab's mkfs knows its own family only.
    expect(createCommands(makeHost(Volume.formatExt3(undefined))).find((d) => d.spec.name === "mkfs")!.spec).not.toEqual(mkfs);
  });
});

describe("the help text on an ext3 host", () => {
  const defs = createCommands(makeHost(Volume.formatExt3(undefined)));
  const spec = (name: string) => defs.find((d) => d.spec.name === name)!.spec;

  it("names the block in df's summary and one address form per noun in write and xxd", () => {
    expect(spec("df").summary).toBe("Block usage of the mounted volume");
    expect(spec("write").flags?.find((f) => f.long === "at")?.desc).toBe("disk address for /dev/hda (addresses: 0x1f (hex), 512 (decimal), b:65 (block), i:11 (inode))");
    expect(spec("xxd").flags?.find((f) => f.long === "offset")?.desc).toBe("start address (addresses: 0x1f (hex), 512 (decimal), b:65 (block), i:11 (inode))");
  });

  it("pins seek's addr argument description: the block form once, plus i:N, no leftover 'sector' noun", () => {
    expect(spec("seek").required?.[0]?.desc).toBe("0x1f, 512, b:65 (block), i:11 (inode)");
  });

  it("pins mkfs's summary, --type, and ext's flags only, --label its own", () => {
    const mkfs = spec("mkfs");
    expect(mkfs.summary).toBe("Format /dev/hda (clears the timeline); --type picks ext2 or ext3");
    expect(mkfs.flags).toEqual([
      { long: "type", shape: "str", desc: "ext2 or ext3 (default: the mounted volume's type)" },
      { long: "blocks", shape: "int", desc: "total 1 KiB blocks (default 16384 = 16 MB)" },
      { long: "inodes-per-group", shape: "int", desc: "inodes per block group (default one per 16 KiB)" },
      { long: "label", shape: "str", desc: "volume label, up to 16 bytes" },
      { long: "uuid", shape: "str", desc: "32 hex digits, hyphens optional" },
      { long: "journal-blocks", shape: "int", desc: "journal size in blocks, ext3 only" },
      { long: "journal-mode", shape: "str", desc: "ordered or data, ext3 only" },
    ]);
    // The same list on an ext2 host: the family's, not the mounted variant's.
    expect(createCommands(makeHost(Volume.formatExt2(undefined))).find((d) => d.spec.name === "mkfs")!.spec).toEqual(mkfs);
  });
});
