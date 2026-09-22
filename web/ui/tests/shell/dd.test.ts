import { describe, expect, it } from "vitest";
import { SIZE_HELP } from "../../src/shell/addr";
import { DD_MAX_BYTES, OPERAND_HELP, formatRecords, parseDd, planWindow } from "../../src/shell/dd";
import { ShellError } from "../../src/shell/errors";
import { isBlob, toBytes, type BytesBlob } from "../../src/shell/bytes";
import { createCommands } from "../../src/shell/commands";
import { applyChanges } from "../../src/core/patch";
import { Vfs } from "../../src/shell/vfs";
import { call, callErr, makeHost } from "./helpers";

function thrown(fn: () => unknown): ShellError {
  try { fn(); } catch (e) { return e as ShellError; }
  throw new Error("expected a throw");
}

describe("parseDd", () => {
  it("defaults: bs 512, skip 0, seek 0, no if/of/count", () => {
    expect(parseDd({}, [])).toEqual({ bs: 512, skip: 0, seek: 0 });
  });
  it("reads --if/--of/--bs/--count/--skip/--seek flags (strings from the 'str' shape, or ints)", () => {
    expect(parseDd({ if: "/dev/hda", of: "/mnt/boot.bin", bs: "512", count: "1", skip: "65", seek: 2 }, [])).toEqual({
      if: "/dev/hda", of: "/mnt/boot.bin", bs: 512, count: 1, skip: 65, seek: 2,
    });
    expect(parseDd({ bs: "1k", count: "2" }, [])).toEqual({ bs: 1024, count: 2, skip: 0, seek: 0 });
  });
  it("accepts quoted classic key=value operands and merges them with flags", () => {
    expect(parseDd({}, ["if=/dev/zero", "of=/dev/hda", "bs=512", "seek=0", "count=1"])).toEqual({
      if: "/dev/zero", of: "/dev/hda", bs: 512, count: 1, skip: 0, seek: 0,
    });
    expect(parseDd({ if: "/dev/hda" }, ["count=1"])).toEqual({ if: "/dev/hda", bs: 512, count: 1, skip: 0, seek: 0 });
  });
  it("rejects a key given twice, across flags and operands", () => {
    const e = thrown(() => parseDd({ if: "/dev/hda" }, ["if=/dev/zero"]));
    expect(e).toBeInstanceOf(ShellError);
    expect(e.message).toBe("'if' given twice");
    expect(thrown(() => parseDd({}, ["bs=1", "bs=2"])).message).toBe("'bs' given twice");
  });
  it("rejects unknown or malformed operands with help", () => {
    for (const op of ["foo", "conv=sync", "if", "=x"]) {
      const e = thrown(() => parseDd({}, [op]));
      expect(e.message).toBe(`unrecognized operand '${op}'`);
      expect(e.help).toBe(OPERAND_HELP);
    }
    expect(thrown(() => parseDd({}, [7])).message).toBe("unrecognized operand '7'");
  });
  it("validates sizes and paths", () => {
    const bs = thrown(() => parseDd({ bs: "x" }, []));
    expect(bs.message).toBe("invalid block size 'x'");
    expect(bs.help).toBe(SIZE_HELP);
    expect(thrown(() => parseDd({ bs: "0" }, [])).message).toBe("invalid block size '0'");
    expect(thrown(() => parseDd({}, ["count=-1"])).message).toBe("invalid count '-1'");
    expect(thrown(() => parseDd({ skip: "1.5" }, [])).message).toBe("invalid skip '1.5'");
    expect(thrown(() => parseDd({ seek: "abc" }, [])).message).toBe("invalid seek 'abc'");
    expect(thrown(() => parseDd({ if: "" }, [])).message).toBe("'if' needs a path");
    expect(thrown(() => parseDd({}, ["of="])).message).toBe("'of' needs a path");
    expect(thrown(() => parseDd({ if: null }, [])).message).toBe("'if' needs a path");
  });
});

describe("planWindow and the 1 MiB cap", () => {
  it("DD_MAX_BYTES is 1 MiB", () => {
    expect(DD_MAX_BYTES).toBe(1048576);
  });
  it("computes skip*bs and count*bs clipped to what is available", () => {
    expect(planWindow({ bs: 512, count: 1, skip: 65 }, 16 * 1048576)).toEqual({ start: 65 * 512, len: 512 });
    expect(planWindow({ bs: 512, count: undefined, skip: 0 }, 1000)).toEqual({ start: 0, len: 1000 });
    expect(planWindow({ bs: 512, count: 4, skip: 1 }, 1000)).toEqual({ start: 512, len: 488 });
    expect(planWindow({ bs: 512, count: 1, skip: 10 }, 1000)).toEqual({ start: 5120, len: 0 }); // skip past the end
    expect(planWindow({ bs: 512, count: 3, skip: 0 }, Infinity)).toEqual({ start: 0, len: 1536 }); // /dev/zero
  });
  it("refuses a read window over DD_MAX_BYTES before anything is written", () => {
    expect(planWindow({ bs: 1048576, count: 1, skip: 0 }, Infinity).len).toBe(DD_MAX_BYTES);
    const e = thrown(() => planWindow({ bs: 512, count: 2049, skip: 0 }, Infinity));
    expect(e).toBeInstanceOf(ShellError);
    expect(e.message).toBe("refusing to copy 1049088 bytes in one dd; the limit is 1048576 (1 MiB)");
    expect(e.help).toBe("lower --count or --bs, or copy in several runs with --skip and --seek");
    expect(thrown(() => planWindow({ bs: 512, count: undefined, skip: 0 }, 16 * 1048576)).message).toMatch(/^refusing to copy 16777216 bytes/);
  });
});

describe("formatRecords", () => {
  it("counts full and partial blocks like dd", () => {
    expect(formatRecords(512, 512)).toBe("1+0 records");
    expect(formatRecords(1000, 512)).toBe("1+1 records");
    expect(formatRecords(0, 512)).toBe("0+0 records");
    expect(formatRecords(3, 512)).toBe("0+1 records");
  });
});

const same = (a: Uint8Array, b: Uint8Array) => Buffer.compare(Buffer.from(a), Buffer.from(b)) === 0;
const text = (b: Uint8Array) => new TextDecoder().decode(b);
function setup() {
  const host = makeHost();
  const vfs = new Vfs();
  return { host, vfs, defs: createCommands(host, vfs) };
}

describe("dd copies", () => {
  it("reads sector 0 into a blob and logs the record counts without touching the journal", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "dd", { flags: { if: "/dev/hda", bs: "512", count: "1" } });
    const blob = r.value as BytesBlob;
    expect(isBlob(blob)).toBe(true);
    expect(blob.length).toBe(512);
    expect(same(toBytes(blob), host.vol.sector(0))).toBe(true);
    expect(r.log).toEqual(["1+0 records in", "1+0 records out", "512 bytes copied"]);
    expect(host.history).toEqual([]);
  });

  it("accepts the quoted classic operand form", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "dd", { positionals: ["if=/dev/hda", "bs=256", "skip=2", "count=1"] });
    const blob = r.value as BytesBlob;
    expect(blob.length).toBe(256);
    expect(same(toBytes(blob), host.vol.sector(1).subarray(0, 256))).toBe(true);
  });

  it("copies a file into a new file at --seek, zero-padding the gap", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "alpha");
    const r = await call(defs, "dd", { flags: { if: "/mnt/a.txt", of: "/mnt/b.txt", seek: "2" } });
    expect(r.value).toBeUndefined();
    const b = host.vol.readFile("/b.txt");
    expect(b.length).toBe(1024 + 5);
    expect(b.subarray(0, 1024).every((x) => x === 0)).toBe(true);
    expect(text(b.subarray(1024))).toBe("alpha");
    expect(r.log).toContain("FAT has no partial writes; the whole file was rewritten");
    expect(r.log.slice(-3)).toEqual(["0+1 records in", "0+1 records out", "5 bytes copied"]);
    expect(host.history.map((h) => h.op)).toEqual(["create_file /a.txt", "create_file /b.txt"]);
    expect(host.selected.at(-1)).toBe("/B.TXT"); // "b.txt" fits an 8.3 short name; the core stores it uppercased
  });

  it("overlays piped bytes onto an existing file at --seek and rewrites it", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "hello world");
    const r = await call(defs, "dd", { flags: { of: "/mnt/a.txt", bs: "1", seek: "6" } }, "XX");
    expect(text(host.vol.readFile("/a.txt"))).toBe("hello XXrld");
    expect(host.history.map((h) => h.op)).toEqual(["create_file /a.txt", "write_file /a.txt"]);
    expect(r.log.slice(-3)).toEqual(["2+0 records in", "2+0 records out", "2 bytes copied"]);
  });

  it("zeroes one sector of /dev/hda and journals exactly that range", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "dd", { flags: { if: "/dev/zero", of: "/dev/hda", seek: "65", count: "1" } });
    expect(r.value).toBeUndefined();
    expect(host.history).toHaveLength(1);
    const rec = host.history[0];
    expect(rec.op).toBe("write_raw 0x8200 +512");
    expect(rec.changes).toHaveLength(1);
    expect(rec.changes[0].offset).toBe(33280);
    expect(rec.changes[0].after.length).toBe(512);
    expect(rec.changes[0].after.every((x) => x === 0)).toBe(true);
    expect(host.vol.readRaw(33280, 512).every((x) => x === 0)).toBe(true);
    expect(r.log).toEqual(["1+0 records in", "1+0 records out", "512 bytes copied"]);
    expect(host.selected).toEqual([]);
  });

  it("refuses a raw write past the end of the disk before journaling anything", async () => {
    const { host, defs } = setup();
    const err = await callErr(defs, "dd", { flags: { if: "/dev/zero", of: "/dev/hda", seek: "32768", count: "1" } });
    expect(err.message).toBe("/dev/hda: Range runs past the end of the disk");
    expect(host.history).toEqual([]);
  });

  it("refuses a huge --seek onto a volume file before allocating anything", async () => {
    const { host, defs } = setup();
    // seek*bs (default bs=512) is far past the disk, and would previously overflow
    // Uint8Array's max length or allocate tens of megabytes before the core's DiskFull check.
    const err = await callErr(defs, "dd", { flags: { of: "/mnt/b.txt", seek: "100000000" } }, "x");
    expect(err.message).toBe("/mnt/b.txt: Range runs past the end of the disk");
    expect(err.help).toContain("--seek");
    expect(host.history).toEqual([]);
  });

  it("skipping past the end of the source reads nothing", async () => {
    const { defs } = setup();
    const r = await call(defs, "dd", { flags: { if: "/dev/hda", skip: "40000" } });
    expect((r.value as BytesBlob).length).toBe(0);
    expect(r.log).toEqual(["0+0 records in", "0+0 records out", "0 bytes copied"]);
  });

  it("/dev/zero needs --count", async () => {
    const { defs } = setup();
    const err = await callErr(defs, "dd", { flags: { if: "/dev/zero", of: "/dev/null" } });
    expect(err.message).toBe("/dev/zero is endless; give --count");
  });

  it("caps the read window at DD_MAX_BYTES before any write", async () => {
    const { host, defs } = setup();
    const err = await callErr(defs, "dd", { flags: { if: "/dev/zero", of: "/dev/hda", count: "2049" } });
    expect(err.message).toBe(`refusing to copy ${2049 * 512} bytes in one dd; the limit is ${DD_MAX_BYTES} (1 MiB)`);
    expect(err.help).toContain("--count");
    expect(host.history).toEqual([]);
    const whole = await callErr(defs, "dd", { flags: { if: "/dev/hda" } });
    expect(whole.message).toBe(`refusing to copy ${16 * 1024 * 1024} bytes in one dd; the limit is ${DD_MAX_BYTES} (1 MiB)`);
    const ok = await call(defs, "dd", { flags: { if: "/dev/zero", count: "2048" } });
    expect((ok.value as BytesBlob).length).toBe(DD_MAX_BYTES);
  });

  it("rejects --if together with piped input, and no input at all", async () => {
    const { defs } = setup();
    expect((await callErr(defs, "dd", { flags: { if: "/dev/hda", count: "1" } }, "x")).message).toBe("both --if and piped input given");
    const none = await callErr(defs, "dd", {});
    expect(none.message).toBe("no input");
    expect(none.help).toContain("--if=");
  });

  it("discards into /dev/null and refuses /dev/zero and directories as sinks", async () => {
    const { host, defs } = setup();
    const r = await call(defs, "dd", { flags: { of: "/dev/null" } }, "bye");
    expect(r.value).toBeUndefined();
    expect(r.log).toEqual(["0+1 records in", "0+1 records out", "3 bytes copied"]);
    expect((await callErr(defs, "dd", { flags: { of: "/dev/zero" } }, "x")).message).toBe("/dev/zero: cannot write to /dev/zero");
    expect((await callErr(defs, "dd", { flags: { of: "/mnt" } }, "x")).message).toBe("/mnt: Is a directory");
    expect((await callErr(defs, "dd", { flags: { if: "/mnt" } })).message).toBe("/mnt: Is a directory");
    expect((await callErr(defs, "dd", { flags: { if: "/dev" } })).message).toBe("/dev: Is a directory");
    expect(host.history).toEqual([]);
  });

  it("warns once when reading the disk while rewound", async () => {
    const { host, defs } = setup();
    await call(defs, "dd", { flags: { if: "/dev/zero", of: "/dev/hda", seek: "65", count: "1" } });
    host.rewind(-1);
    const r = await call(defs, "dd", { flags: { if: "/dev/hda", count: "1" } });
    expect(r.err).toHaveLength(1);
    expect(r.err[0]).toContain("showing the latest state");
    expect((r.value as BytesBlob).length).toBe(512);
  });

  it("corruption round trip: wipe sector 0, watch path ops fail, write it back", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "alpha");
    const saved = (await call(defs, "dd", { flags: { if: "/dev/hda", count: "1" } })).value as BytesBlob;
    expect(saved.length).toBe(512);

    await call(defs, "dd", { flags: { if: "/dev/zero", of: "/dev/hda", count: "1" } });
    expect(host.vol.corruption()).toContain("boot sector no longer parses after a raw write");
    const err = await callErr(defs, "ls", { positionals: ["/mnt"] });
    expect(err.message).toContain("boot sector no longer parses after a raw write");
    const corrupt = await call(defs, "mount");
    expect(corrupt.value).toMatchObject([{ state: "corrupt" }]);
    expect(host.vol.layout().length).toBeGreaterThan(0); // unmounted inspection still works

    await call(defs, "dd", { flags: { of: "/dev/hda" } }, saved);
    expect(host.vol.corruption()).toBeNull();
    expect(host.history.map((h) => h.op)).toEqual(["create_file /a.txt", "write_raw 0x0 +512", "write_raw 0x0 +512"]);
    const ls = await call(defs, "ls", { positionals: ["/mnt"] });
    expect(ls.value).toMatchObject([{ name: "A.TXT", type: "file", size: 5 }]); // "a.txt" fits an 8.3 short name; stored uppercased
    expect((await call(defs, "mount")).value).toMatchObject([{ state: "ok" }]);

    // The two raw writes rewind byte for byte, the way VolumeStore.seek does it.
    const image = host.vol.image();
    applyChanges(image, host.history[2].changes, "reverse");
    expect(image.subarray(0, 512).every((x) => x === 0)).toBe(true);
    applyChanges(image, host.history[1].changes, "reverse");
    expect(same(image.subarray(0, 512), toBytes(saved))).toBe(true);
    expect(same(image, host.vol.image())).toBe(true);
  });
});
