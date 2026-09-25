import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { addrHelp } from "../../src/shell/addr";
import { createCommands, rewoundWarning } from "../../src/shell/commands";
import { DD_MAX_BYTES } from "../../src/shell/dd";
import { Vfs } from "../../src/shell/vfs";
import { call, callErr, makeHost } from "./helpers";

const enc = (s: string) => new TextEncoder().encode(s);
const DISK_BYTES = 32768 * 512; // the default 16 MB volume

/** A host with three steps: a directory, a two-cluster file in it, and a long-named root file. */
function setup() {
  const host = makeHost();
  host.run((v) => v.createDir("/DOCS"));
  host.run((v) => v.createFile("/DOCS/N.TXT", new Uint8Array(3000)));
  host.run((v) => v.createFile("/Hello world.txt", enc("hello from the shell\n")));
  const vfs = new Vfs();
  return { host, vfs, defs: createCommands(host, vfs) };
}

describe("ls and dir", () => {
  it("lists the virtual root and /dev with the disk size on hda", async () => {
    const { defs } = setup();
    expect((await call(defs, "ls", ["/"])).value).toEqual([
      { name: "dev", type: "dir", size: 0 },
      { name: "mnt", type: "dir", size: 0 },
    ]);
    expect((await call(defs, "ls", ["/dev"])).value).toEqual([
      { name: "hda", type: "block", size: DISK_BYTES },
      { name: "zero", type: "char", size: 0 },
      { name: "null", type: "char", size: 0 },
    ]);
    expect((await call(defs, "ls", ["/dev/hda"])).value).toEqual([{ name: "hda", type: "block", size: DISK_BYTES }]);
  });

  it("lists a volume directory in on-disk order and a file as one row", async () => {
    const { defs } = setup();
    expect((await call(defs, "ls", ["/mnt"])).value).toEqual([
      { name: "DOCS", type: "dir", size: 0 },
      { name: "Hello world.txt", type: "file", size: 21 },
    ]);
    expect((await call(defs, "ls", ["/mnt/docs/n.txt"])).value).toEqual([{ name: "N.TXT", type: "file", size: 3000 }]);
    expect((await call(defs, "ls", ["/mnt/DOCS"])).value).toEqual([{ name: "N.TXT", type: "file", size: 3000 }]);
  });

  it("defaults to the working directory and honours -l", async () => {
    const { host, defs } = setup();
    host.vol.setNow({ year: 2026, month: 9, day: 22, hour: 10, minute: 30, second: 0 });
    host.run((v) => v.createFile("/DOCS/T.TXT", enc("t")));
    await call(defs, "cd", ["/mnt/DOCS"]);
    const plain = (await call(defs, "ls")).value as { name: string; modified?: string }[];
    expect(plain.map((r) => r.name)).toEqual(["N.TXT", "T.TXT"]);
    expect(plain[0].modified).toBeUndefined();
    const long = (await call(defs, "ls", { flags: { long: true } })).value as { name: string; modified: string }[];
    expect(long[1]).toEqual({ name: "T.TXT", type: "file", size: 1, modified: "2026-09-22 10:30:00" });
    expect(typeof long[0].modified).toBe("string");
  });

  it("reports missing and non-directory paths with coreutils phrases and no command prefix", async () => {
    const { defs } = setup();
    expect(await callErr(defs, "ls", ["/mnt/nope"])).toMatchObject({ message: "/mnt/nope: No such file or directory", code: "NotFound" });
    expect(await callErr(defs, "ls", ["/mnt/DOCS/N.TXT/x"])).toMatchObject({ message: "/mnt/DOCS/N.TXT/x: Not a directory", code: "NotADirectory" });
    const e = await callErr(defs, "ls", ["/foo"]);
    expect(e.message).toContain("/foo");
    expect(e.help).toContain("/mnt");
  });

  it("dir is an alias of ls", async () => {
    const { defs } = setup();
    expect(defs.find((d) => d.spec.name === "dir")?.spec.summary).toBe("alias of ls");
    expect((await call(defs, "dir", ["/"])).value).toEqual((await call(defs, "ls", ["/"])).value);
  });
});

describe("cd and pwd", () => {
  it("starts at /mnt, stores the canonical case, and walks .. up to the root", async () => {
    const { defs } = setup();
    expect((await call(defs, "pwd")).value).toBe("/mnt");
    await call(defs, "cd", ["/mnt/docs"]);
    expect((await call(defs, "pwd")).value).toBe("/mnt/DOCS");
    await call(defs, "cd", [".."]);
    expect((await call(defs, "pwd")).value).toBe("/mnt");
    await call(defs, "cd", [".."]);
    expect((await call(defs, "pwd")).value).toBe("/");
    await call(defs, "cd", ["dev"]);
    expect((await call(defs, "pwd")).value).toBe("/dev");
    await call(defs, "cd");
    expect((await call(defs, "pwd")).value).toBe("/mnt");
  });

  it("hands the terminal a new prompt prefix for every directory it lands in", async () => {
    const { host, defs } = setup();
    await call(defs, "cd", ["/mnt/docs"]);
    expect(host.prompts).toEqual(["/mnt/DOCS "]); // the canonical case, as pwd reports it
    await call(defs, "cd", [".."]);
    await call(defs, "cd", ["/"]);
    await call(defs, "cd", ["dev"]);
    await call(defs, "cd");
    expect(host.prompts).toEqual(["/mnt/DOCS ", "/mnt ", "/ ", "/dev ", "/mnt "]);
  });

  it("refuses files, devices, and missing paths", async () => {
    const { host, defs } = setup();
    expect(await callErr(defs, "cd", ["/mnt/Hello world.txt"])).toMatchObject({ message: "/mnt/Hello world.txt: Not a directory", code: "NotADirectory" });
    expect(await callErr(defs, "cd", ["/dev/hda"])).toMatchObject({ message: "/dev/hda: Not a directory", code: "NotADirectory" });
    expect(await callErr(defs, "cd", ["/mnt/nope"])).toMatchObject({ message: "/mnt/nope: No such file or directory", code: "NotFound" });
    expect((await call(defs, "pwd")).value).toBe("/mnt"); // a failed cd leaves cwd alone
    expect(host.prompts).toEqual([]); // ... and leaves the prompt alone with it
  });
});

describe("cat", () => {
  it("prints UTF-8 text, or the raw bytes with --bytes", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "cat", ["/mnt/hello world.txt"]);
    expect(r.value).toBe("hello from the shell\n");
    expect(r.err).toEqual([]);
    const bytes = await call(defs, "cat", { positionals: ["/mnt/Hello world.txt"], flags: { bytes: true } });
    expect(bytes.value).toBeInstanceOf(Uint8Array);
    expect(bytes.value).toEqual(host.vol.readFile("/Hello world.txt"));
    expect((await call(defs, "cat", ["/dev/null"])).value).toBe("");
    // /dev/null with --bytes is an empty buffer, not the empty string, so a pipe into
    // `xxd` or a redirect still sees bytes.
    expect((await call(defs, "cat", { positionals: ["/dev/null"], flags: { bytes: true } })).value).toEqual(new Uint8Array(0));
  });

  it("warns once about binary content and refuses files over DD_MAX_BYTES", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "cat", ["/mnt/DOCS/N.TXT"]);
    expect(r.err).toEqual(["binary file; try cat --bytes /mnt/DOCS/N.TXT | xxd"]);
    expect((r.value as string).length).toBe(3000);
    host.run((v) => v.createFile("/BIG.BIN", new Uint8Array(DD_MAX_BYTES + 1)));
    const steps = host.history.length;
    const e = await callErr(defs, "cat", ["/mnt/BIG.BIN"]);
    expect(e.message).toBe(`/mnt/BIG.BIN: file is ${DD_MAX_BYTES + 1} bytes; cat prints at most ${DD_MAX_BYTES} bytes`);
    expect(e.help).toContain("dd --if=/mnt/BIG.BIN");
    // --bytes is capped the same way: the size comes from stat, so nothing is read.
    const asBytes = await callErr(defs, "cat", { positionals: ["/mnt/BIG.BIN"], flags: { bytes: true } });
    expect(asBytes.message).toBe(e.message);
    expect(asBytes.help).toBe(e.help);
    expect(host.history.length).toBe(steps);
  });

  it("points devices at dd and directories at their nature", async () => {
    const { defs } = setup();
    expect(await callErr(defs, "cat", ["/mnt"])).toMatchObject({ message: "/mnt: Is a directory", code: "IsADirectory" });
    expect(await callErr(defs, "cat", ["/mnt/DOCS"])).toMatchObject({ message: "/mnt/DOCS: Is a directory", code: "IsADirectory" });
    expect(await callErr(defs, "cat", ["/"])).toMatchObject({ message: "/: Is a directory" });
    const hda = await callErr(defs, "cat", ["/dev/hda"]);
    expect(hda.message).toBe("/dev/hda: is a raw device");
    expect(hda.help).toBe("read a range with: dd --if=/dev/hda --bs=512 --skip=0 --count=1 | xxd");
    expect((await callErr(defs, "cat", ["/dev/zero"])).message).toBe("/dev/zero: is a raw device");
    expect(await callErr(defs, "cat", ["/mnt/nope"])).toMatchObject({ message: "/mnt/nope: No such file or directory", code: "NotFound" });
  });
});

describe("stat", () => {
  it("reports where a file lives: entry slot, FAT entry, chain, data", async () => {
    const { host, defs } = setup();
    const fs = host.adapter;
    const g = host.vol.geometry();
    const first = fs.ownerOf("/DOCS/N.TXT")!.firstUnit;
    const slots = fs.entrySlots("/DOCS/N.TXT")!;
    const r = (await call(defs, "stat", ["/mnt/docs/n.txt"])).value as Record<string, unknown>;
    expect(r).toMatchObject({
      path: "/mnt/DOCS/N.TXT",
      name: "N.TXT",
      type: "file",
      size: 3000,
      firstCluster: first,
      chain: [first, first + 1],
      clusters: 2,
      entryOffset: `0x${slots.start.toString(16)}`,
      entrySlots: `0x${slots.start.toString(16)}-0x${slots.end.toString(16)}`,
      fatEntryOffset: `0x${(g.reservedSectors * g.bytesPerSector + first * 2).toString(16)}`,
      dataOffset: `0x${fs.unitByteRange(first).start.toString(16)}`,
    });
    expect(typeof r.modified).toBe("string");
    expect(typeof r.created).toBe("string");
    expect(typeof r.accessed).toBe("string");
  });

  it("describes the root, an empty file, and the devices", async () => {
    const { host, defs } = setup();
    const g = host.vol.geometry();
    expect((await call(defs, "stat", ["/mnt"])).value).toMatchObject({
      path: "/mnt", name: "/", type: "dir", size: 0, firstCluster: 0, chain: [], clusters: 0,
      entryOffset: "", fatEntryOffset: "", dataOffset: `0x${(g.firstRootDirSector * g.bytesPerSector).toString(16)}`,
    });
    host.run((v) => v.createFile("/EMPTY", new Uint8Array(0)));
    const empty = (await call(defs, "stat", ["/mnt/empty"])).value as Record<string, unknown>;
    expect(empty).toMatchObject({ path: "/mnt/EMPTY", size: 0, firstCluster: 0, chain: [], fatEntryOffset: "", dataOffset: "" });
    expect(empty.entryOffset).toMatch(/^0x[0-9a-f]+$/);
    expect((await call(defs, "stat", ["/dev/hda"])).value).toEqual({
      path: "/dev/hda", type: "block", size: DISK_BYTES, sectorSize: 512, sectors: 32768, fsType: "FAT16", state: "ok",
    });
    expect((await call(defs, "stat", ["/dev/zero"])).value).toEqual({ path: "/dev/zero", type: "char", size: 0 });
    expect((await call(defs, "stat", ["/"])).value).toEqual({ path: "/", type: "dir", size: 0 });
    expect(await callErr(defs, "stat", ["/mnt/nope"])).toMatchObject({ message: "/mnt/nope: No such file or directory", code: "NotFound" });
  });
});

describe("df and mount", () => {
  it("df counts used clusters from FAT 0", async () => {
    const { host, defs } = setup();
    const g = host.vol.geometry();
    const rows = (await call(defs, "df")).value as Record<string, unknown>[];
    expect(rows).toHaveLength(1);
    // DOCS (1 cluster) + N.TXT (2) + Hello world.txt (1)
    expect(rows[0]).toMatchObject({ filesystem: "/dev/hda", mounted: "/mnt", type: "FAT16", clusterSize: 2048, clusters: g.clusterCount, used: 4, free: g.clusterCount - 4, bytesUsed: 4 * 2048, bytesFree: (g.clusterCount - 4) * 2048 });
    expect(rows[0].use).toMatch(/^\d+%$/);
  });

  it("mount shows one healthy row", async () => {
    const { defs } = setup();
    expect((await call(defs, "mount")).value).toEqual([
      { device: "/dev/hda", mount: "/mnt", type: "FAT16", sectorSize: 512, sectors: 32768, bytes: DISK_BYTES, state: "ok" },
    ]);
  });
});

describe("seek, select, exit", () => {
  it("seek jumps the dump to sector, cluster, hex, and decimal addresses", async () => {
    const { host, defs } = setup();
    await call(defs, "seek", ["s:65"]);
    await call(defs, "seek", ["c:3"]);
    await call(defs, "seek", ["0x200"]);
    await call(defs, "seek", ["512"]);
    expect(host.jumps).toEqual([65 * 512, host.adapter.unitByteRange(3).start, 512, 512]);
  });

  it("seek rejects bad and out-of-range addresses", async () => {
    const { host, defs } = setup();
    expect((await callErr(defs, "seek", ["nope"])).help).toBe(addrHelp(host.adapter));
    expect((await callErr(defs, "seek", ["0x1000000"])).message).toBe(`0x1000000 is past the end of the disk (${DISK_BYTES} bytes)`);
    expect(host.jumps).toEqual([]);
  });

  it("select records the canonical volume path, or null", async () => {
    const { host, defs } = setup();
    await call(defs, "select", ["/mnt/docs/n.txt"]);
    await call(defs, "select");
    await call(defs, "select", ["/mnt"]);
    expect(host.selected).toEqual(["/DOCS/N.TXT", null, null]);
    expect(await callErr(defs, "select", ["/dev/hda"])).toMatchObject({ message: "/dev/hda: only volume paths can be selected" });
    expect(await callErr(defs, "select", ["/mnt/nope"])).toMatchObject({ message: "/mnt/nope: No such file or directory", code: "NotFound" });
    expect(host.selected).toHaveLength(3);
  });

  it("exit closes the terminal", async () => {
    const { host, defs } = setup();
    expect(host.closed).toBe(false);
    await call(defs, "exit");
    expect(host.closed).toBe(true);
  });
});

describe("on an ext3 host", () => {
  /** The default ext3 disk with one 12-byte file: inode 12, data block 1111. */
  function extSetup() {
    const host = makeHost(Volume.formatExt3(undefined));
    host.run((v) => v.createFile("/hello.txt", enc("Hello, ext3!")));
    return { host, defs: createCommands(host, new Vfs()) };
  }

  it("df counts 1 KiB blocks from the superblock", async () => {
    const { defs } = extSetup();
    expect((await call(defs, "df")).value).toEqual([
      { filesystem: "/dev/hda", mounted: "/mnt", type: "ext3", blockSize: 1024, blocks: 16384, used: 1180, free: 15204, bytesUsed: 1180 * 1024, bytesFree: 15204 * 1024, use: "7%" },
    ]);
  });

  it("stat spreads the inode facts after the generic ones", async () => {
    const { defs } = extSetup();
    const r = (await call(defs, "stat", ["/mnt/hello.txt"])).value as Record<string, unknown>;
    expect(Object.keys(r)).toEqual(["path", "name", "type", "size", "created", "modified", "accessed", "inode", "inodeOffset", "mode", "links", "blocks512", "dataBlocks", "indirectBlocks", "dirEntryOffset"]);
    expect(r).toMatchObject({
      path: "/mnt/hello.txt", name: "hello.txt", type: "file", size: 12,
      inode: 12, inodeOffset: "0x1980", mode: "0100644", links: 1, blocks512: 2, dataBlocks: [1111], indirectBlocks: [], dirEntryOffset: "0x1142c",
    });
    expect((await call(defs, "stat", ["/mnt/lost+found"])).value).toMatchObject({
      type: "dir", inode: 11, inodeOffset: "0x1900", mode: "040700", links: 2, dataBlocks: [70, 71, 72, 73, 74, 75, 76, 77, 78, 79, 80, 81], indirectBlocks: [], dirEntryOffset: "0x11418",
    });
    expect((await call(defs, "stat", ["/dev/hda"])).value).toEqual({
      path: "/dev/hda", type: "block", size: 16384 * 1024, sectorSize: 1024, sectors: 16384, fsType: "ext3", state: "ok",
    });
  });

  it("seek takes block and inode addresses", async () => {
    const { host, defs } = extSetup();
    await call(defs, "seek", ["b:69"]);
    await call(defs, "seek", ["i:12"]);
    await call(defs, "seek", ["s:2"]);
    expect(host.jumps).toEqual([69 * 1024, 6 * 1024 + 0x180, 2 * 1024]);
    const e = await callErr(defs, "seek", ["c:3"]);
    expect(e.message).toBe("bad address 'c:3'");
    expect(e.help).toBe("addresses: 0x1f (hex), 512 (decimal), b:65 (block), i:11 (inode)");
  });

  it("xxd --offset and write --at take block and inode addresses too", async () => {
    const { host, defs } = extSetup();
    const hello = await call(defs, "xxd", { positionals: ["/dev/hda"], flags: { offset: "b:1111", len: "16" } });
    expect((hello.value as string).startsWith("00115c00: ")).toBe(true); // block 1111 = 0x115c00
    expect(hello.value).toContain("Hello, ext3!");
    const inode = await call(defs, "xxd", { positionals: ["/dev/hda"], flags: { offset: "i:12", len: "16" } });
    expect((inode.value as string).startsWith("00001980: ")).toBe(true); // inode 12's slot, block 6 + 0x180
    expect((await call(defs, "write", { positionals: ["/dev/hda"], flags: { at: "b:2000" } }, "RAW")).log).toEqual(["3 bytes -> /dev/hda at 0x1f4000"]);
    expect((await call(defs, "write", { positionals: ["/dev/hda"], flags: { at: "i:20" } }, "RAW")).log).toEqual(["3 bytes -> /dev/hda at 0x1d80"]); // a free inode's slot, block 7 + 0x180
    expect(host.history.slice(-2).map((r) => r.op)).toEqual(["write_raw 0x1f4000 +3", "write_raw 0x1d80 +3"]);
  });
});

describe("rewound timeline", () => {
  it("read commands warn once and still show the latest state; navigation does not", async () => {
    const { host, defs } = setup();
    host.rewind(0);
    const warning = 'showing the latest state, not step 1 of 3; click "Back to now" or run a write command';
    expect(rewoundWarning(host)).toBe(warning);
    const ls = await call(defs, "ls", ["/mnt"]);
    expect(ls.err).toEqual([warning]);
    expect((ls.value as { name: string }[]).map((r) => r.name)).toEqual(["DOCS", "Hello world.txt"]);
    for (const [name, args] of [["dir", ["/mnt"]], ["cat", ["/mnt/Hello world.txt"]], ["stat", ["/mnt"]], ["df", []], ["mount", []], ["select", ["/mnt/DOCS"]]] as const) {
      expect((await call(defs, name, [...args])).err, name).toEqual([warning]);
    }
    expect((await call(defs, "pwd")).err).toEqual([]);
    expect((await call(defs, "cd", ["/mnt/DOCS"])).err).toEqual([]);
    host.rewind(-1);
    expect((await call(defs, "ls", ["/"])).err).toEqual(['showing the latest state, not step 0 of 3; click "Back to now" or run a write command']);
    host.rewind(2);
    expect((await call(defs, "ls", ["/"])).err).toEqual([]);
  });
});

describe("corruption", () => {
  it("ls, df, stat report a wiped boot sector; mount says corrupt; restoring it clears the state", async () => {
    const { host, defs } = setup();
    const boot = host.vol.sector(0);
    host.run((v) => v.writeRaw(0, new Uint8Array(512)));
    expect((await call(defs, "mount")).value).toMatchObject([{ state: "corrupt" }]);
    expect((await call(defs, "stat", ["/dev/hda"])).value).toMatchObject({ state: "corrupt" });
    for (const [name, args] of [["ls", ["/mnt"]], ["df", []], ["stat", ["/mnt/DOCS"]]] as const) {
      const e = await callErr(defs, name, [...args]);
      expect(e.message, name).toContain("boot sector no longer parses");
      expect(e.code, name).toBe("CorruptImage");
      expect(e.help, name).toContain("dd --of=/dev/hda");
    }
    expect((await callErr(defs, "ls", ["/mnt"])).message.startsWith("/mnt: ")).toBe(true);
    expect((await call(defs, "ls", ["/dev"])).value).toHaveLength(3); // /dev never touches the volume
    host.run((v) => v.writeRaw(0, boot));
    expect((await call(defs, "mount")).value).toMatchObject([{ state: "ok" }]);
    expect((await call(defs, "ls", ["/mnt"])).value).toHaveLength(2);
  });
});
