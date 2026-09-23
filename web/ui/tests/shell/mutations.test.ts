import { describe, expect, it } from "vitest";
import { createCommands } from "../../src/shell/commands";
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

  it("surfaces the core's geometry error with its code and leaves the volume alone", async () => {
    const { host, defs } = setup();
    const err = await callErr(defs, "mkfs", { flags: { sectors: 5 } });
    expect(err.message).toMatch(/^\/dev\/hda: invalid geometry: /);
    expect(err.code).toBe("InvalidGeometry");
    expect(host.vol.geometry().totalSectors).toBe(32768);
    expect((await callErr(defs, "mkfs", { flags: { spc: -1 } })).message).toBe("--spc must be a non-negative integer");
  });
});
