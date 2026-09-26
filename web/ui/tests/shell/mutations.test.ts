import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { FAMILIES } from "../../src/fs";
import { familyOfType, mkfsFlagsFor, mkfsTypeOf, mkfsTypes, orList } from "../../src/shell/mkfs";
import { createCommands } from "../../src/shell/register";
import { Vfs } from "../../src/shell/vfs";
import { call, callErr, makeHost } from "./helpers";

const text = (b: Uint8Array) => new TextDecoder().decode(b);
function setup() {
  const host = makeHost();
  const vfs = new Vfs();
  return { host, vfs, defs: createCommands(host, vfs) };
}
const ops = (host: ReturnType<typeof makeHost>) => host.history.map((r) => r.op);

describe("write", () => {
  it("creates a file from piped text, then rewrites it, selecting the file each time", async () => {
    const { host, defs } = setup();
    const first = await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "hi");
    expect(first.value).toBeUndefined();
    expect(first.log).toEqual(["2 bytes -> /mnt/a.txt"]);
    expect(ops(host)).toEqual(["create_file /a.txt"]);
    expect(text(host.vol.readFile("/a.txt"))).toBe("hi");
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "hello");
    expect(ops(host)).toEqual(["create_file /a.txt", "write_file /a.txt"]);
    expect(text(host.vol.readFile("/a.txt"))).toBe("hello");
    expect(host.selected).toEqual(["/A.TXT", "/A.TXT"]); // "a.txt" fits an 8.3 short name; the core stores it uppercased
  });

  it("resolves relative paths against the working directory and keeps the on-disk case", async () => {
    const { host, vfs, defs } = setup();
    await call(defs, "mkdir", { positionals: ["/mnt/DOCS"] });
    vfs.cwd = "/mnt/docs";
    await call(defs, "write", { positionals: ["Note.txt"] }, "n");
    expect(ops(host).at(-1)).toBe("create_file /docs/Note.txt");
    expect(host.selected.at(-1)).toBe("/DOCS/NOTE.TXT"); // "Note.txt" fits an 8.3 short name; stored uppercased
    expect(text(host.vol.readFile("/DOCS/NOTE.TXT"))).toBe("n");
  });

  it("--append reads, concatenates and rewrites the whole file, and says so", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "hi");
    const r = await call(defs, "write", { positionals: ["/mnt/a.txt"], flags: { append: true } }, "hi");
    expect(text(host.vol.readFile("/a.txt"))).toBe("hihi");
    expect(ops(host)).toEqual(["create_file /a.txt", "write_file /a.txt"]);
    expect(r.log).toEqual([
      "appended 2 bytes by rewriting the whole file (FAT has no append; the old chain is freed and reallocated)",
      "4 bytes -> /mnt/a.txt",
    ]);
    const fresh = await call(defs, "write", { positionals: ["/mnt/new.txt"], flags: { append: true } }, "x");
    expect(fresh.log).toEqual(["1 bytes -> /mnt/new.txt"]);
    expect(ops(host).at(-1)).toBe("create_file /new.txt");
  });

  it("accepts echo's forms: a number, and a list of words joined by spaces", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/n.txt"] }, 12345);
    expect(text(host.vol.readFile("/n.txt"))).toBe("12345");
    await call(defs, "write", { positionals: ["/mnt/w.txt"] }, ["hello", "world"]);
    expect(text(host.vol.readFile("/w.txt"))).toBe("hello world");
  });

  it("writes piped bytes verbatim, including a NUL and a byte no UTF-8 decoder would keep", async () => {
    const { host, defs } = setup();
    const bytes = new Uint8Array([0x00, 0xff, 0x41, 0x80]);
    const r = await call(defs, "write", { positionals: ["/mnt/raw.bin"] }, bytes);
    expect(r.log).toEqual(["4 bytes -> /mnt/raw.bin"]);
    expect(Array.from(host.vol.readFile("/RAW.BIN"))).toEqual([0x00, 0xff, 0x41, 0x80]);
    // And --append concatenates buffers the same way it concatenates text.
    await call(defs, "write", { positionals: ["/mnt/raw.bin"], flags: { append: true } }, new Uint8Array([0x01]));
    expect(Array.from(host.vol.readFile("/RAW.BIN"))).toEqual([0x00, 0xff, 0x41, 0x80, 0x01]);
  });

  it("writes raw bytes to /dev/hda at --at and does not select anything", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "write", { positionals: ["/dev/hda"], flags: { at: "s:65" } }, "RAW");
    expect(ops(host)).toEqual(["write_raw 0x8200 +3"]);
    expect(text(host.vol.readRaw(33280, 3))).toBe("RAW");
    expect(r.log).toEqual(["3 bytes -> /dev/hda at 0x8200"]);
    expect(host.selected).toEqual([]);
    await call(defs, "write", { positionals: ["/dev/hda"], flags: { at: "43" } }, "SHELLDISK  ");
    expect(host.vol.bootSector().volumeLabel.startsWith("SHELLDISK")).toBe(true);
  });

  it("/dev/null discards without journaling", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "write", { positionals: ["/dev/null"] }, "gone");
    expect(r.log).toEqual(["4 bytes -> /dev/null"]);
    expect(host.history).toEqual([]);
  });

  it("rejects the mistakes it can see before touching the disk", async () => {
    const { host, defs } = setup();
    const none = await callErr(defs, "write", { positionals: ["/mnt/a.txt"] });
    expect(none.message).toBe("nothing to write");
    expect(none.help).toContain("echo hi | write");
    // A caller driving `write` directly could still pass an explicit null.
    expect((await callErr(defs, "write", { positionals: ["/mnt/a.txt"] }, null)).message).toBe("nothing to write");
    expect((await callErr(defs, "write", { positionals: ["/dev/hda"] }, "x")).message).toBe("/dev/hda: give --at <addr>");
    expect((await callErr(defs, "write", { positionals: ["/dev/hda"], flags: { at: "0", append: true } }, "x")).message).toBe("--append is not supported on /dev/hda");
    expect((await callErr(defs, "write", { positionals: ["/dev/hda"], flags: { at: "0" } }, "")).message).toBe("nothing to write");
    expect((await callErr(defs, "write", { positionals: ["/dev/hda"], flags: { at: "0x1000000" } }, "x")).message).toBe("/dev/hda: Range runs past the end of the disk");
    expect((await callErr(defs, "write", { positionals: ["/mnt/a.txt"], flags: { at: "0" } }, "x")).message).toBe("--at only applies to /dev/hda");
    expect((await callErr(defs, "write", { positionals: ["/mnt"] }, "x")).message).toBe("/mnt: Is a directory");
    expect((await callErr(defs, "write", { positionals: ["/"] }, "x")).message).toBe("/: Is a directory");
    expect((await callErr(defs, "write", { positionals: ["/dev/zero"] }, "x")).message).toBe("/dev/zero: cannot write to /dev/zero");
    expect((await callErr(defs, "write", { positionals: ["/mnt/nope/a.txt"] }, "x")).message).toBe("/mnt/nope/a.txt: No such file or directory");
    expect(host.history).toEqual([]);
  });

  it("treats an empty piped list, what the real engine sends for no pipe, as no input", async () => {
    const { host, defs } = setup();
    // browser-terminal's stream collector turns "nothing piped" into Value::List([]), not null
    // (crates/bterm-core/src/stream.rs) -- `write /mnt/x` with nothing piped must still refuse,
    // not silently create a 0-byte file.
    const err = await callErr(defs, "write", { positionals: ["/mnt/x"] }, []);
    expect(err.message).toBe("nothing to write");
    expect(host.history).toEqual([]);
  });
});

describe("mkdir, rmdir, rm, touch", () => {
  it("mkdir then rmdir go through the journal and mirror the explorer's selection", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    expect(r.value).toBeUndefined();
    expect(ops(host)).toEqual(["create_dir /D"]);
    expect(host.vol.stat("/D").isDir).toBe(true);
    expect((await callErr(defs, "mkdir", { positionals: ["/mnt/D"] })).message).toBe("/mnt/D: File exists");
    await call(defs, "rmdir", { positionals: ["/mnt/D"] });
    expect(ops(host)).toEqual(["create_dir /D", "remove_dir /D"]);
    expect(host.selected).toEqual(["/D", null]);
  });

  it("mkdir resolves relative to the working directory and refuses non-volume targets", async () => {
    const { host, vfs, defs } = setup();
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    vfs.cwd = "/mnt/D";
    await call(defs, "mkdir", { positionals: ["E"] });
    expect(ops(host).at(-1)).toBe("create_dir /D/E");
    expect(host.selected.at(-1)).toBe("/D/E");
    expect((await callErr(defs, "mkdir", { positionals: ["/mnt"] })).message).toBe("/mnt: File exists");
    expect((await callErr(defs, "mkdir", { positionals: ["/dev"] })).message).toBe("/dev: File exists");
    expect((await callErr(defs, "mkdir", { positionals: ["/dev/hda"] })).message).toBe("/dev/hda: cannot create a directory on a device");
    expect((await callErr(defs, "mkdir", { positionals: ["/mnt/nope/X"] })).message).toBe("/mnt/nope/X: No such file or directory");
  });

  it("rmdir refuses a non-empty directory, a file, the mount point and devices", async () => {
    const { host, defs } = setup();
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    await call(defs, "write", { positionals: ["/mnt/D/f.txt"] }, "f");
    expect((await callErr(defs, "rmdir", { positionals: ["/mnt/D"] })).message).toBe("/mnt/D: Directory not empty");
    expect((await callErr(defs, "rmdir", { positionals: ["/mnt/D/f.txt"] })).message).toBe("/mnt/D/f.txt: Not a directory");
    expect((await callErr(defs, "rmdir", { positionals: ["/mnt"] })).message).toBe("/mnt: cannot remove the mount point");
    expect((await callErr(defs, "rmdir", { positionals: ["/dev/hda"] })).message).toBe("/dev/hda: Not a directory");
    expect((await callErr(defs, "rmdir", { positionals: ["/mnt/nope"] })).message).toBe("/mnt/nope: No such file or directory");
    expect(ops(host)).toEqual(["create_dir /D", "create_file /D/f.txt"]);
  });

  it("rm deletes a file, clears the selection, and refuses directories and devices", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "a");
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    const r = await call(defs, "rm", { positionals: ["/mnt/a.txt"] });
    expect(r.value).toBeUndefined();
    expect(ops(host)).toEqual(["create_file /a.txt", "create_dir /D", "delete_file /a.txt"]);
    expect(host.selected).toEqual(["/A.TXT", "/D", null]); // "a.txt" fits an 8.3 short name; stored uppercased
    const dir = await callErr(defs, "rm", { positionals: ["/mnt/D"] });
    expect(dir.message).toBe("/mnt/D: Is a directory");
    expect(dir.help).toBe("use rmdir");
    expect((await callErr(defs, "rm", { positionals: ["/mnt"] })).message).toBe("/mnt: Is a directory");
    expect((await callErr(defs, "rm", { positionals: ["/dev/hda"] })).message).toBe("/dev/hda: cannot remove a device");
    expect((await callErr(defs, "rm", { positionals: ["/mnt/nope"] })).message).toBe("/mnt/nope: No such file or directory");
    expect(ops(host)).toHaveLength(3);
  });

  it("touch creates an empty file once and is a logged no-op afterwards", async () => {
    const { host, defs } = setup();
    const first = await call(defs, "touch", { positionals: ["/mnt/e.txt"] });
    expect(first.log).toEqual(["created empty /mnt/e.txt"]);
    expect(ops(host)).toEqual(["create_file /e.txt"]);
    expect(host.vol.stat("/e.txt").size).toBe(0);
    const again = await call(defs, "touch", { positionals: ["/mnt/e.txt"] });
    expect(again.log).toEqual(["/mnt/e.txt exists; fs explorer has no timestamp-only update, nothing written"]);
    expect(ops(host)).toEqual(["create_file /e.txt"]);
    expect(host.selected).toEqual(["/E.TXT", "/E.TXT"]); // "e.txt" fits an 8.3 short name; stored uppercased
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    expect((await callErr(defs, "touch", { positionals: ["/mnt/D"] })).message).toBe("/mnt/D: Is a directory");
    expect((await callErr(defs, "touch", { positionals: ["/dev/hda"] })).message).toBe("/dev/hda: cannot touch a device");
  });
});

describe("cp", () => {
  it("copies a file into a new cluster chain", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "alpha");
    const r = await call(defs, "cp", { positionals: ["/mnt/a.txt", "/mnt/b.txt"] });
    expect(r.value).toBeUndefined();
    expect(r.log).toEqual(["5 bytes -> /mnt/b.txt"]);
    expect(text(host.vol.readFile("/b.txt"))).toBe("alpha");
    expect(ops(host)).toEqual(["create_file /a.txt", "create_file /b.txt"]);
    // "a.txt"/"b.txt" fit 8.3 short names; the core stores them uppercased.
    const owners = host.vol.clusterOwners();
    const a = owners.find((o) => o.path === "/A.TXT")!;
    const b = owners.find((o) => o.path === "/B.TXT")!;
    expect(a.firstCluster).toBe(2);
    expect(b.firstCluster).toBe(3);
    expect(host.selected.at(-1)).toBe("/B.TXT");
  });

  it("into an existing directory uses the source's on-disk basename; onto a file overwrites", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/Alpha.txt"] }, "alpha");
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    const r = await call(defs, "cp", { positionals: ["/mnt/alpha.txt", "/mnt/d"] });
    // "Alpha.txt" fits an 8.3 short name; the core stores it (and the basename cp derives) uppercased.
    expect(r.log).toEqual(["5 bytes -> /mnt/D/ALPHA.TXT"]);
    expect(ops(host).at(-1)).toBe("create_file /D/ALPHA.TXT");
    expect(host.selected.at(-1)).toBe("/D/ALPHA.TXT");
    await call(defs, "write", { positionals: ["/mnt/Alpha.txt"] }, "beta!");
    await call(defs, "cp", { positionals: ["/mnt/Alpha.txt", "/mnt/D/Alpha.txt"] });
    expect(ops(host).at(-1)).toBe("write_file /D/Alpha.txt");
    expect(text(host.vol.readFile("/D/Alpha.txt"))).toBe("beta!");
  });

  it("refuses devices and directories as the source", async () => {
    const { defs } = setup();
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    expect((await callErr(defs, "cp", { positionals: ["/dev/hda", "/mnt/x"] })).message).toBe("/dev/hda: use dd for devices");
    expect((await callErr(defs, "cp", { positionals: ["/mnt/D", "/mnt/x"] })).message).toBe("/mnt/D: Is a directory");
    expect((await callErr(defs, "cp", { positionals: ["/mnt/nope", "/mnt/x"] })).message).toBe("/mnt/nope: No such file or directory");
    expect((await callErr(defs, "cp", { positionals: ["/mnt/D", "/dev/null"] })).message).toBe("/mnt/D: Is a directory");
  });
});

describe("mkfs", () => {
  it("formats with the given geometry, resets cwd and clears the timeline", async () => {
    const { host, vfs, defs } = setup();
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    vfs.cwd = "/mnt/D";
    const r = await call(defs, "mkfs", { flags: { sectors: 8192, spc: 1, label: "SHELLDISK" } });
    expect(r.value).toBeUndefined();
    expect(r.log).toEqual(["formatted /dev/hda as FAT16; the timeline was cleared"]);
    expect(host.vol.geometry().totalSectors).toBe(8192);
    expect(host.vol.geometry().sectorsPerCluster).toBe(1);
    expect(host.vol.bootSector().volumeLabel.startsWith("SHELLDISK")).toBe(true);
    expect(host.historyLength).toBe(0);
    expect(host.vol.listDir("/")).toEqual([]);
    expect(vfs.cwd).toBe("/mnt");
    expect(host.prompts).toEqual(["/mnt "]); // the prompt follows the reset cwd
    expect(host.selected.at(-1)).toBe("/D"); // mkfs selects nothing; the host adapter resets the selection
  });

  it("names the type the new volume reports in its done line", async () => {
    const { host, defs } = setup();
    // A host whose freshly formatted volume reports another type: the line follows
    // `fsType()`, not a string of the family's.
    const format = host.format.bind(host);
    host.format = (family, options) => {
      format(family, options);
      host.vol = Object.assign(Object.create(host.vol), { fsType: () => "ext3" });
    };
    expect((await call(defs, "mkfs")).log).toEqual(["formatted /dev/hda as ext3; the timeline was cleared"]);
  });

  it("surfaces the core's geometry error with its code and leaves the volume alone", async () => {
    const { host, defs } = setup();
    const err = await callErr(defs, "mkfs", { flags: { sectors: 5 } });
    expect(err.message).toMatch(/^\/dev\/hda: invalid geometry: /);
    expect(err.code).toBe("InvalidGeometry");
    expect(host.vol.geometry().totalSectors).toBe(32768);
    expect((await callErr(defs, "mkfs", { flags: { spc: -1 } })).message).toBe("--spc must be a non-negative integer");
  });
});

describe("mkfs --type", () => {
  it("formats ext3 from an ext2 host: the ext family with its variant, the done line, cwd and prompt reset", async () => {
    const host = makeHost(Volume.formatExt2(undefined));
    const vfs = new Vfs();
    const defs = createCommands(host, vfs);
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
    vfs.cwd = "/mnt/D";
    const r = await call(defs, "mkfs", { flags: { type: "ext3", blocks: 4096, label: "shell" } });
    expect(r.log).toEqual(["formatted /dev/hda as ext3; the timeline was cleared"]);
    expect(host.formats).toEqual([{ family: "ext", options: { variant: "ext3", totalBlocks: 4096, label: "shell" } }]);
    expect(host.vol.fsType()).toBe("ext3");
    expect(host.vol.sectorCount()).toBe(4096);
    expect(host.adapter.id).toBe("ext");
    expect(host.adapter.journal).toBeDefined();
    expect(host.historyLength).toBe(0);
    expect(vfs.cwd).toBe("/mnt");
    expect(host.prompts).toEqual(["/mnt "]);
  });

  it("formats ext2 on an ext3 host and FAT16 on a FAT16 host, naming each type in the done line", async () => {
    const ext = makeHost(Volume.formatExt3(undefined));
    expect((await call(createCommands(ext), "mkfs", { flags: { type: "ext2" } })).log).toEqual(["formatted /dev/hda as ext2; the timeline was cleared"]);
    expect(ext.adapter.journal).toBeUndefined();
    expect(ext.formats).toEqual([{ family: "ext", options: { variant: "ext2" } }]);
    const { host, defs } = setup();
    expect((await call(defs, "mkfs", { flags: { type: "FAT16", sectors: 8192, spc: 1 } })).log).toEqual(["formatted /dev/hda as FAT16; the timeline was cleared"]);
    expect(host.vol.geometry().totalSectors).toBe(8192);
    expect(host.formats).toEqual([{ family: "fat16", options: { totalSectors: 8192, sectorsPerCluster: 1 } }]);
  });

  it("refuses another family's type, naming the tab that formats it, and leaves the volume alone", async () => {
    const { host, vfs, defs } = setup();
    vfs.cwd = "/mnt/D";
    const e = await callErr(defs, "mkfs", { flags: { type: "ext3" } });
    expect(e).toEqual({ message: "'ext3' is an ext type: switch to the ext tab to format one", help: "types: fat16", code: undefined });
    expect((await callErr(defs, "mkfs", { flags: { type: "EXT2", blocks: 4096 } })).message).toBe("'ext2' is an ext type: switch to the ext tab to format one");
    expect(host.vol.fsType()).toBe("FAT16");
    expect(host.formats).toEqual([]);
    expect(vfs.cwd).toBe("/mnt/D");
    expect(host.prompts).toEqual([]);

    const ext = makeHost(Volume.formatExt3(undefined));
    const mirror = await callErr(createCommands(ext), "mkfs", { flags: { type: "fat16" } });
    expect(mirror).toEqual({ message: "'fat16' is a FAT16 type: switch to the FAT16 tab to format one", help: "types: ext2, ext3", code: undefined });
    expect(ext.vol.fsType()).toBe("ext3");
    expect(ext.formats).toEqual([]);
  });

  it("defaults to the mounted volume's own type", async () => {
    const host = makeHost(Volume.formatExt3(undefined));
    const defs = createCommands(host, new Vfs());
    expect((await call(defs, "mkfs", { flags: { "journal-mode": "data" } })).log).toEqual(["formatted /dev/hda as ext3; the timeline was cleared"]);
    expect(host.formats).toEqual([{ family: "ext", options: { variant: "ext3", journalMode: "data" } }]);
    const ext2 = makeHost(Volume.formatExt2(undefined));
    await call(createCommands(ext2), "mkfs");
    expect(ext2.vol.fsType()).toBe("ext2");
    expect(ext2.formats).toEqual([{ family: "ext", options: { variant: "ext2" } }]);
  });

  it("refuses a flag the chosen type does not take, naming the type", async () => {
    const { host, defs } = setup();
    expect((await callErr(defs, "mkfs", { flags: { type: "fat16", blocks: 4096 } })).message).toBe("--blocks is not a fat16 option");
    expect((await callErr(defs, "mkfs", { flags: { blocks: 4096 } })).message).toBe("--blocks is not a fat16 option");
    expect(host.formats).toEqual([]);
    const ext = makeHost(Volume.formatExt3(undefined));
    expect((await callErr(createCommands(ext), "mkfs", { flags: { type: "ext3", spc: 2 } })).message).toBe("--spc is not an ext3 option");
    expect((await callErr(createCommands(ext), "mkfs", { flags: { type: "ext2", "root-entries": 64 } })).message).toBe("--root-entries is not an ext2 option");
    expect(ext.formats).toEqual([]);
  });

  it("surfaces the family's own error through fsCall, and refuses an unknown type", async () => {
    const { host, defs } = setup();
    const ext = createCommands(makeHost(Volume.formatExt3(undefined)));
    expect((await callErr(ext, "mkfs", { flags: { type: "ext2", "journal-mode": "data" } })).message).toBe("/dev/hda: journalBlocks and journalMode need variant ext3");
    expect((await callErr(ext, "mkfs", { flags: { type: "ext3", "journal-blocks": -1 } })).message).toBe("--journal-blocks must be a non-negative integer");
    const bad = await callErr(defs, "mkfs", { flags: { type: "ntfs" } });
    expect(bad.message).toBe("unknown type 'ntfs'");
    expect(bad.help).toBe("types: fat16");
    const badExt = await callErr(ext, "mkfs", { flags: { type: "ntfs" } });
    expect(badExt.message).toBe("unknown type 'ntfs'");
    expect(badExt.help).toBe("types: ext2, ext3");
    expect(host.vol.fsType()).toBe("FAT16");
  });

  it("builds its lists from the registry: the types per family, the family of a type, the default type, and the flags", () => {
    expect([orList(["a"]), orList(["a", "b"]), orList(["fat16", "ext2", "ext3"])]).toEqual(["a", "a or b", "fat16, ext2, or ext3"]);
    expect(mkfsTypes({ fat16: FAMILIES.fat16 })).toEqual(["fat16"]);
    expect(mkfsTypes({ ext: FAMILIES.ext })).toEqual(["ext2", "ext3"]);
    expect(mkfsTypes(FAMILIES)).toEqual(["fat16", "ext2", "ext3"]);
    expect(familyOfType("fat16")?.id).toBe("fat16");
    expect(familyOfType("FAT16")?.id).toBe("fat16");
    expect(familyOfType("ext2")?.id).toBe("ext");
    expect(familyOfType("Ext3")?.id).toBe("ext");
    expect(familyOfType("ntfs")).toBeUndefined();
    expect(familyOfType("constructor")).toBeUndefined();
    expect(mkfsTypeOf(makeHost())).toBe("fat16");
    expect(mkfsTypeOf(makeHost(Volume.formatExt2(undefined)))).toBe("ext2");
    // One family keeps its own words; a flag two families share merges them.
    expect(mkfsFlagsFor({ fat16: FAMILIES.fat16 })).toEqual(FAMILIES.fat16.mkfs.flags.map(({ long, kind, desc }) => ({ long, kind, desc })));
    expect(mkfsFlagsFor({ ext: FAMILIES.ext })).toEqual(FAMILIES.ext.mkfs.flags.map(({ long, kind, desc }) => ({ long, kind, desc })));
    expect(mkfsFlagsFor(FAMILIES).find((f) => f.long === "label")).toEqual({ long: "label", kind: "str", desc: "volume label (fat16: up to 11 characters; ext: up to 16 bytes)" });
  });
});

describe("format through the host", () => {
  it("refuses an unregistered family with a named error instead of a TypeError", () => {
    const { host } = setup();
    expect(() => host.format("nope" as never)).toThrow('no filesystem family "nope"');
  });

  it("refuses an inherited key like \"constructor\" with the same named error", () => {
    const { host } = setup();
    expect(() => host.format("constructor" as never)).toThrow('no filesystem family "constructor"');
  });

  it("refuses another family, like the tab's VolumeStore, and leaves the volume alone", () => {
    const { host } = setup();
    expect(() => host.format("ext", { variant: "ext3" })).toThrow("this tab formats fat16, not ext");
    expect(() => makeHost(Volume.formatExt3(undefined)).format("fat16")).toThrow("this tab formats ext, not fat16");
    expect(host.vol.fsType()).toBe("FAT16");
    expect(host.formats).toEqual([]);
  });
});
