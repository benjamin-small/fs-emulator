import { describe, expect, it } from "vitest";
import { createCommands } from "../../src/shell/commands";
import { Vfs } from "../../src/shell/vfs";
import { formatXxd } from "../../src/shell/xxd";
import { call, callErr, makeHost } from "./helpers";

function setup() {
  const host = makeHost();
  const vfs = new Vfs();
  return { host, vfs, defs: createCommands(host, vfs) };
}
const lines = (v: unknown) => (v as string).split("\n");

describe("xxd command", () => {
  it("dumps one sector of /dev/hda at absolute addresses by default", async () => {
    const { defs } = setup();
    const r = await call(defs, "xxd", { positionals: ["/dev/hda"] });
    const out = lines(r.value);
    expect(out).toHaveLength(32);
    expect(out[0]).toMatch(/^00000000: eb3c 90/);
    expect(out[31]).toMatch(/^000001f0: /);
    expect(out[31]).toContain("55aa");
    expect(out[31].endsWith("U.")).toBe(true);
    expect(r.err).toEqual([]);
  });

  it("--offset and --len pick a window of the disk; addresses stay absolute", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "Hello, FAT16!");
    const g = host.vol.geometry();
    const cluster2 = g.firstDataSector * g.bytesPerSector;
    expect(cluster2).toBe(0xc200);
    const r = await call(defs, "xxd", { positionals: ["/dev/hda"], flags: { offset: "c:2", len: "16" } });
    expect(r.value).toBe(formatXxd(host.vol.readRaw(cluster2, 16), cluster2, 16));
    expect(lines(r.value)).toHaveLength(1);
    expect((r.value as string).startsWith("0000c200: ")).toBe(true);
    expect(r.value).toContain("Hello, FAT16!");
    const root = await call(defs, "xxd", { positionals: ["/dev/hda"], flags: { offset: "s:65", len: "32" } });
    const out = lines(root.value);
    expect(out).toHaveLength(2);
    expect(out[0].startsWith("00008200: ")).toBe(true);
    expect(out[1].startsWith("00008210: ")).toBe(true);
    const tail = await call(defs, "xxd", { positionals: ["/dev/hda"], flags: { offset: "0xfffff0", len: "1k" } });
    expect(lines(tail.value)).toHaveLength(1); // clipped at the end of the disk
  });

  it("dumps a volume file, piped bytes, and a piped string from address 0", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "Hello, FAT16!");
    const expected = formatXxd(host.vol.readFile("/a.txt"), 0, 16);
    expect((await call(defs, "xxd", { positionals: ["/mnt/a.txt"] })).value).toBe(expected);
    const piped = (await call(defs, "dd", { flags: { if: "/mnt/a.txt" } })).value as Uint8Array;
    expect(piped).toBeInstanceOf(Uint8Array);
    expect((await call(defs, "xxd", {}, piped)).value).toBe(expected);
    expect((await call(defs, "xxd", {}, "Hello, FAT16!")).value).toBe(expected);
    expect(lines((await call(defs, "xxd", { positionals: ["/mnt/a.txt"], flags: { cols: 8 } })).value)).toHaveLength(2);
    expect((await call(defs, "xxd", { positionals: ["/mnt/a.txt"], flags: { offset: "7", len: "5" } })).value)
      .toBe(formatXxd(new TextEncoder().encode("FAT16"), 7, 16));
  });

  it("/dev/zero dumps zeros for --len; hexdump is the same command", async () => {
    const { defs } = setup();
    const z = await call(defs, "xxd", { positionals: ["/dev/zero"], flags: { len: "4" } });
    expect(z.value).toBe(formatXxd(new Uint8Array(4), 0, 16));
    const a = await call(defs, "xxd", { positionals: ["/dev/hda"] });
    const b = await call(defs, "hexdump", { positionals: ["/dev/hda"] });
    expect(b.value).toBe(a.value);
  });

  it("reports the mistakes it can see", async () => {
    const { defs } = setup();
    const none = await callErr(defs, "xxd", {});
    expect(none.message).toBe("nothing to dump");
    expect(none.help).toContain("| xxd");
    // A caller driving `xxd` directly could still pass an explicit null.
    expect((await callErr(defs, "xxd", {}, null)).message).toBe("nothing to dump");
    expect((await callErr(defs, "xxd", { positionals: ["/dev/zero"] })).message).toBe("/dev/zero: give --len");
    expect((await callErr(defs, "xxd", { positionals: ["/mnt"] })).message).toBe("/mnt: Is a directory");
    expect((await callErr(defs, "xxd", { positionals: ["/dev"] })).message).toBe("/dev: Is a directory");
    expect((await callErr(defs, "xxd", { positionals: ["/mnt/nope"] })).message).toBe("/mnt/nope: No such file or directory");
    expect((await callErr(defs, "xxd", { positionals: ["/dev/hda"], flags: { cols: 0 } })).message).toBe("--cols must be 1..64");
    expect((await callErr(defs, "xxd", { positionals: ["/dev/hda"], flags: { offset: "0x1000000" } })).message)
      .toBe("0x1000000 is past the end of the disk (16777216 bytes)");
    expect((await callErr(defs, "xxd", { positionals: ["/dev/hda"], flags: { len: "2M" } })).help).toContain("--len");
  });

  it("treats an empty piped list, what the real engine sends for no pipe, as no input", async () => {
    const { defs } = setup();
    // browser-terminal's stream collector turns "nothing piped" into Value::List([]), not null
    // (crates/bterm-core/src/stream.rs) -- `xxd` with no path and nothing piped must still refuse,
    // not silently dump zero bytes.
    const err = await callErr(defs, "xxd", {}, []);
    expect(err.message).toBe("nothing to dump");
  });

  it("warns when dumping the disk or a file while rewound", async () => {
    const { host, defs } = setup();
    await call(defs, "write", { positionals: ["/mnt/a.txt"] }, "x");
    host.rewind(-1);
    expect((await call(defs, "xxd", { positionals: ["/dev/hda"] })).err[0]).toContain("showing the latest state");
    expect((await call(defs, "xxd", { positionals: ["/mnt/a.txt"] })).err[0]).toContain("showing the latest state");
    expect((await call(defs, "xxd", {}, "piped")).err).toEqual([]);
  });
});
