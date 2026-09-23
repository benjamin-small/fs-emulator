import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { adapterFor } from "../../src/fs";
import { ShellError } from "../../src/shell/errors";
import { DEVICE_HELP, MOUNT, PATH_HELP, Vfs, basename, canonicalize, joinVolume, promptFor } from "../../src/shell/vfs";

function thrown(fn: () => unknown): ShellError {
  try { fn(); } catch (e) { return e as ShellError; }
  throw new Error("expected a throw");
}

describe("Vfs.normalize", () => {
  it("starts at /mnt and joins relative paths to cwd", () => {
    const v = new Vfs();
    expect(v.cwd).toBe(MOUNT);
    expect(v.normalize("")).toBe("/mnt");
    expect(v.normalize("a")).toBe("/mnt/a");
    expect(v.normalize("a/b")).toBe("/mnt/a/b");
    expect(v.normalize("./a")).toBe("/mnt/a");
    expect(v.normalize(".")).toBe("/mnt");
  });
  it("drops ., pops .., and keeps / at the root", () => {
    const v = new Vfs();
    expect(v.normalize("..")).toBe("/");
    expect(v.normalize("../..")).toBe("/");
    expect(v.normalize("/..")).toBe("/");
    expect(v.normalize("/mnt/a/../b")).toBe("/mnt/b");
    v.cwd = "/mnt/DOCS";
    expect(v.normalize("..")).toBe("/mnt");
    expect(v.normalize("../X")).toBe("/mnt/X");
  });
  it("treats \\ as /, collapses separators, and strips a trailing /", () => {
    const v = new Vfs();
    expect(v.normalize("/mnt//a/./b/")).toBe("/mnt/a/b");
    expect(v.normalize("\\mnt\\a")).toBe("/mnt/a");
    expect(v.normalize("/")).toBe("/");
    expect(v.normalize("///")).toBe("/");
    expect(v.normalize("/mnt/")).toBe("/mnt");
  });
  it("preserves case as typed", () => {
    expect(new Vfs().normalize("/mnt/Hello World.txt")).toBe("/mnt/Hello World.txt");
  });
});

describe("Vfs.resolve", () => {
  const v = new Vfs();
  it("classifies the virtual root, /dev, the devices, and volume paths", () => {
    expect(v.resolve("/")).toEqual({ kind: "root" });
    expect(v.resolve("/dev")).toEqual({ kind: "dev" });
    expect(v.resolve("/dev/")).toEqual({ kind: "dev" });
    expect(v.resolve("/dev/hda")).toEqual({ kind: "raw" });
    expect(v.resolve("/dev/zero")).toEqual({ kind: "zero" });
    expect(v.resolve("/dev/null")).toEqual({ kind: "null" });
    expect(v.resolve("/mnt")).toEqual({ kind: "volume", path: "/" });
    expect(v.resolve("/mnt/")).toEqual({ kind: "volume", path: "/" });
    expect(v.resolve("/mnt/A/B")).toEqual({ kind: "volume", path: "/A/B" });
    expect(v.resolve("A/B")).toEqual({ kind: "volume", path: "/A/B" });
    expect(v.resolve("..")).toEqual({ kind: "root" });
  });
  it("rejects unknown devices and paths outside /mnt and /dev, with help", () => {
    const dev = thrown(() => v.resolve("/dev/hdb"));
    expect(dev).toBeInstanceOf(ShellError);
    expect(dev.message).toBe("no such device: /dev/hdb");
    expect(dev.help).toBe(DEVICE_HELP);
    const foo = thrown(() => v.resolve("/foo"));
    expect(foo.message).toBe("no such file or directory: /foo");
    expect(foo.help).toBe(PATH_HELP);
    expect(thrown(() => v.resolve("/mnt2/x")).message).toBe("no such file or directory: /mnt2/x");
    expect(thrown(() => v.resolve("/dev/hda/x")).message).toBe("no such device: /dev/hda/x");
    expect(thrown(() => v.resolve("/MNT/x")).message).toBe("no such file or directory: /MNT/x");
  });
  it("display and toVirtual invert resolve", () => {
    for (const p of ["/", "/dev", "/dev/hda", "/dev/zero", "/dev/null", "/mnt", "/mnt/A/B"]) expect(v.display(v.resolve(p))).toBe(p);
    expect(v.toVirtual("/")).toBe("/mnt");
    expect(v.toVirtual("/A/B")).toBe("/mnt/A/B");
  });
});

describe("canonicalize over a real volume", () => {
  it("fixes the case of every component to the on-disk name", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createFile("/Hello world.txt", new TextEncoder().encode("hi"));
    vol.createDir("/DOCS");
    vol.createFile("/DOCS/N.TXT", new Uint8Array(3));
    const fs = adapterFor(vol);
    expect(canonicalize(fs, "/hello world.txt")).toBe("/Hello world.txt");
    expect(canonicalize(fs, "/HELLO WORLD.TXT")).toBe("/Hello world.txt");
    expect(canonicalize(fs, "/docs/n.txt")).toBe("/DOCS/N.TXT");
    expect(canonicalize(fs, "/docs")).toBe("/DOCS");
    expect(canonicalize(fs, "/")).toBe("/");
  });
  it("keeps the typed spelling for components it cannot find or read", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createDir("/DOCS");
    const fs = adapterFor(vol);
    expect(canonicalize(fs, "/docs/missing.txt")).toBe("/DOCS/missing.txt");
    expect(canonicalize(fs, "/nope/deeper")).toBe("/nope/deeper");
  });
});

describe("path helpers", () => {
  it("basename and joinVolume", () => {
    expect(basename("/A/B.TXT")).toBe("B.TXT");
    expect(basename("B.TXT")).toBe("B.TXT");
    expect(basename("/")).toBe("");
    expect(joinVolume("/", "A")).toBe("/A");
    expect(joinVolume("/D", "A")).toBe("/D/A");
  });
  it("promptFor puts one space between the working directory and the terminal's ❯", () => {
    expect(promptFor("/mnt")).toBe("/mnt ");
    expect(promptFor("/mnt/DOCS")).toBe("/mnt/DOCS ");
    expect(promptFor("/")).toBe("/ ");
  });
});
