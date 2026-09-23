import { describe, expect, it } from "vitest";
import { BYTES_HELP } from "../../src/shell/bytes";
import { RAW_DEVICE_HELP } from "../../src/shell/commands";
import { DD_MAX_BYTES } from "../../src/shell/dd";
import { CORRUPT_HELP } from "../../src/shell/errors";
import { createRedirectHandler } from "../../src/shell/redirect";
import type { RedirectContext, Value } from "../../src/shell/types";
import { Vfs } from "../../src/shell/vfs";
import { makeHost } from "./helpers";

const text = (b: Uint8Array) => new TextDecoder().decode(b);

function setup() {
  const host = makeHost();
  const vfs = new Vfs();
  return { host, vfs, hook: createRedirectHandler(host, vfs) };
}

/** What browser-terminal passes: read-only ids and the pipeline's abort signal. */
const ctx = (append = false): RedirectContext & { append: boolean } => ({
  signal: new AbortController().signal,
  session: 1,
  pane: 1,
  append,
});

interface Failure { message: string; help?: string; code?: string }
async function failed(fn: () => unknown): Promise<Failure> {
  try {
    await fn();
  } catch (e) {
    const f = e as Partial<Failure>;
    return { message: String(f.message ?? e), help: f.help, code: f.code };
  }
  throw new Error("expected a throw");
}

describe("write (> and >>)", () => {
  it("creates the file, then overwrites it, journaling and selecting each time", async () => {
    const { host, hook } = setup();
    await hook.write("/mnt/a.txt", "hi", ctx());
    expect(text(host.vol.readFile("/a.txt"))).toBe("hi");
    expect(host.history.map((r) => r.op)).toEqual(["create_file /a.txt"]);
    expect(host.selected).toEqual(["/A.TXT"]); // 8.3 short name; the core stores it uppercased

    await hook.write("/mnt/a.txt", "hello", ctx());
    expect(text(host.vol.readFile("/a.txt"))).toBe("hello");
    expect(host.history.map((r) => r.op)).toEqual(["create_file /a.txt", "write_file /a.txt"]);
    expect(host.selected).toEqual(["/A.TXT", "/A.TXT"]);
  });

  it("appends by reading, concatenating and rewriting, and creates the file when absent", async () => {
    const { host, hook } = setup();
    await hook.write("/mnt/log.txt", "one", ctx());
    await hook.write("/mnt/log.txt", "two", ctx(true));
    expect(text(host.vol.readFile("/log.txt"))).toBe("onetwo");
    expect(host.history.map((r) => r.op)).toEqual(["create_file /log.txt", "write_file /log.txt"]);

    await hook.write("/mnt/fresh.txt", "x", ctx(true));
    expect(text(host.vol.readFile("/fresh.txt"))).toBe("x");
    expect(host.history.map((r) => r.op).at(-1)).toBe("create_file /fresh.txt");
  });

  it("resolves the target against the working directory", async () => {
    const { host, vfs, hook } = setup();
    host.run((v) => v.createDir("/DOCS"));
    vfs.cwd = "/mnt/docs";
    await hook.write("Note.txt", "n", ctx());
    expect(text(host.vol.readFile("/DOCS/NOTE.TXT"))).toBe("n");
  });

  it("writes a Uint8Array as raw bytes and a list of words joined by one space", async () => {
    const { host, hook } = setup();
    await hook.write("/mnt/raw.bin", new Uint8Array([0x00, 0xff, 0x41]), ctx());
    expect(Array.from(host.vol.readFile("/RAW.BIN"))).toEqual([0x00, 0xff, 0x41]);
    await hook.write("/mnt/w.txt", ["hello", "world"], ctx());
    expect(text(host.vol.readFile("/w.txt"))).toBe("hello world");
    // An empty stream is `[]`, and `> f` on one makes an empty file rather than failing.
    await hook.write("/mnt/empty.txt", [], ctx());
    expect(host.vol.readFile("/empty.txt").length).toBe(0);
  });

  it("refuses a record, with the same help the pipe commands give", async () => {
    const { host, hook } = setup();
    const e = await failed(() => hook.write("/mnt/a.txt", { a: 1 } as Value, ctx()));
    expect(e.message).toBe("expected text or bytes, found a record");
    expect(e.help).toBe(BYTES_HELP);
    expect(host.history).toEqual([]);
  });

  it("refuses the raw devices and points at dd", async () => {
    const { host, hook } = setup();
    for (const target of ["/dev/hda", "/dev/zero"]) {
      const e = await failed(() => hook.write(target, "x", ctx()));
      expect(e.message).toBe(`${target}: cannot redirect into a raw device`);
      // `--seek` is in blocks, so the help names the block size alongside it.
      expect(e.help).toContain("dd --of=/dev/hda");
      expect(e.help).toContain("--seek=");
    }
    expect(host.history).toEqual([]);
  });

  it("refuses the virtual directories and a directory under /mnt", async () => {
    const { host, hook } = setup();
    host.run((v) => v.createDir("/DOCS"));
    const steps = host.history.length;
    for (const target of ["/", "/dev", "/mnt", "/mnt/DOCS"]) {
      const e = await failed(() => hook.write(target, "x", ctx()));
      expect(e.message).toMatch(/: Is a directory$/);
      expect(e.code).toBe("IsADirectory");
    }
    expect(host.history.length).toBe(steps);
  });

  it("discards into /dev/null without journaling or selecting", async () => {
    const { host, hook } = setup();
    await hook.write("/dev/null", "gone", ctx());
    await hook.write("/dev/null", new Uint8Array([1, 2, 3]), ctx(true));
    expect(host.history).toEqual([]);
    expect(host.selected).toEqual([]);
  });

  it("reports an unknown path the way every other command does", async () => {
    const { hook } = setup();
    const e = await failed(() => hook.write("/nope/a.txt", "x", ctx()));
    expect(e.message).toBe("no such file or directory: /nope/a.txt");
  });
});

describe("read (<)", () => {
  it("returns a volume file as UTF-8 text", async () => {
    const { host, hook } = setup();
    host.run((v) => v.createFile("/A.TXT", new TextEncoder().encode("hello\n")));
    expect(await hook.read("/mnt/A.TXT", ctx())).toBe("hello\n");
  });

  it("reads the latest state while the timeline is rewound, silently", async () => {
    const { host, hook } = setup();
    host.run((v) => v.createFile("/A.TXT", new TextEncoder().encode("now")));
    host.rewind(-1);
    // No `ctx.err` in a redirect hook, so there is nowhere to warn; the rule that reads
    // show the latest state still holds.
    expect(await hook.read("/mnt/A.TXT", ctx())).toBe("now");
  });

  it("returns the empty string for /dev/null", async () => {
    const { hook } = setup();
    expect(await hook.read("/dev/null", ctx())).toBe("");
  });

  it("refuses the raw devices and points at dd", async () => {
    const { hook } = setup();
    for (const target of ["/dev/hda", "/dev/zero"]) {
      const e = await failed(() => hook.read(target, ctx()));
      expect(e.message).toBe(`${target}: is a raw device`);
      expect(e.help).toBe(RAW_DEVICE_HELP);
    }
  });

  it("refuses directories and points at cat", async () => {
    const { host, hook } = setup();
    host.run((v) => v.createDir("/DOCS"));
    for (const target of ["/", "/dev", "/mnt", "/mnt/DOCS"]) {
      const e = await failed(() => hook.read(target, ctx()));
      expect(e.message).toMatch(/: Is a directory$/);
      expect(e.code).toBe("IsADirectory");
      expect(e.help).toContain("cat ");
    }
  });

  it("refuses a file over the 1 MiB cap, before reading it", async () => {
    const { host, hook } = setup();
    host.run((v) => v.createFile("/BIG.BIN", new Uint8Array(DD_MAX_BYTES + 1)));
    const e = await failed(() => hook.read("/mnt/BIG.BIN", ctx()));
    expect(e.message).toBe(`/mnt/BIG.BIN: file is ${DD_MAX_BYTES + 1} bytes; cat prints at most ${DD_MAX_BYTES} bytes`);
    expect(e.help).toContain("dd --if=/mnt/BIG.BIN");
  });

  it("reports a missing file the way cat does", async () => {
    const { hook } = setup();
    const e = await failed(() => hook.read("/mnt/NOPE.TXT", ctx()));
    expect(e.message).toBe("/mnt/NOPE.TXT: No such file or directory");
    expect(e.code).toBe("NotFound");
  });
});

describe("the corruption gate", () => {
  it("fails both hooks with the CorruptImage hint once the boot sector is gone", async () => {
    const { host, hook } = setup();
    host.run((v) => v.createFile("/A.TXT", new TextEncoder().encode("hi")));
    host.run((v) => v.writeRaw(0, new Uint8Array(512)));
    expect(host.vol.corruption()).toContain("boot sector no longer parses");
    const steps = host.history.length;

    const onRead = await failed(() => hook.read("/mnt/A.TXT", ctx()));
    expect(onRead.code).toBe("CorruptImage");
    expect(onRead.help).toBe(CORRUPT_HELP);

    const onWrite = await failed(() => hook.write("/mnt/A.TXT", "x", ctx()));
    expect(onWrite.code).toBe("CorruptImage");
    expect(onWrite.help).toBe(CORRUPT_HELP);
    expect(host.history.length).toBe(steps);
  });
});
