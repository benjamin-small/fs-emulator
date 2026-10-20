import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { PATH_COMMANDS, completePath, type PathCompletionContext } from "../../src/shell/complete";
import { createCommands } from "../../src/shell/register";
import { Vfs } from "../../src/shell/vfs";
import { makeHost } from "./helpers";

const enc = (s: string) => new TextEncoder().encode(s);

function setup(vol?: Volume) {
  const host = makeHost(vol);
  const vfs = new Vfs();
  return { host, vfs };
}

/** A FAT16 host with a directory, a file in it, and a long-named root file. */
function fat() {
  const s = setup();
  s.host.run((v) => v.createDir("/DOCS"));
  s.host.run((v) => v.createFile("/DOCS/N.TXT", new Uint8Array(10)));
  s.host.run((v) => v.createFile("/Hello world.txt", enc("hi")));
  return s;
}

const ctx = (command: string, prefix: string, more: Partial<PathCompletionContext> = {}): PathCompletionContext => ({
  command,
  prefix,
  argumentIndex: 0,
  kind: "argument",
  ...more,
});

const values = (items: { value: string }[]) => items.map((i) => i.value).sort();

describe("completePath", () => {
  it("completes the virtual root and /dev", () => {
    const { host, vfs } = fat();
    expect(completePath(host, vfs, ctx("ls", "/"))).toEqual([
      { value: "/dev/", directory: true },
      { value: "/mnt/", directory: true },
    ]);
    expect(completePath(host, vfs, ctx("cat", "/m"))).toEqual([{ value: "/mnt/", directory: true }]);
    expect(values(completePath(host, vfs, ctx("cat", "/dev/")))).toEqual(["/dev/hda", "/dev/null", "/dev/zero"]);
    expect(completePath(host, vfs, ctx("xxd", "/dev/h"))).toEqual([{ value: "/dev/hda", directory: false }]);
  });

  it("lists the mounted volume, marking directories and skipping . and ..", () => {
    const { host, vfs } = fat();
    expect(values(completePath(host, vfs, ctx("ls", "/mnt/")))).toEqual(["/mnt/DOCS/", "/mnt/Hello world.txt"]);
    expect(completePath(host, vfs, ctx("cat", "/mnt/DOCS/"))).toEqual([{ value: "/mnt/DOCS/N.TXT", directory: false }]);
  });

  it("resolves a relative prefix against the working directory and keeps it relative", () => {
    const { host, vfs } = fat();
    expect(completePath(host, vfs, ctx("cat", "DOCS/N"))).toEqual([{ value: "DOCS/N.TXT", directory: false }]);
    vfs.cwd = "/mnt/DOCS";
    expect(completePath(host, vfs, ctx("cat", ""))).toEqual([{ value: "N.TXT", directory: false }]);
    expect(values(completePath(host, vfs, ctx("ls", "../")))).toEqual(["../DOCS/", "../Hello world.txt"]);
  });

  it("matches FAT names case-insensitively and continues in the case the user typed", () => {
    const { host, vfs } = fat();
    expect(completePath(host, vfs, ctx("cd", "/mnt/do"))).toEqual([{ value: "/mnt/docs/", directory: true }]);
    expect(completePath(host, vfs, ctx("cat", "/mnt/DOCS/n"))).toEqual([{ value: "/mnt/DOCS/n.txt", directory: false }]);
    expect(completePath(host, vfs, ctx("cat", "/mnt/D"))).toEqual([{ value: "/mnt/DOCS/", directory: true }]);
  });

  it("matches ext names case-sensitively", () => {
    const { host, vfs } = setup(Volume.formatExt3(undefined));
    host.run((v) => v.createFile("/Notes.txt", enc("x")));
    expect(completePath(host, vfs, ctx("cat", "/mnt/no"))).toEqual([]);
    expect(completePath(host, vfs, ctx("cat", "/mnt/No"))).toEqual([{ value: "/mnt/Notes.txt", directory: false }]);
  });

  it("offers only directories to cd, mkdir and rmdir", () => {
    const { host, vfs } = fat();
    for (const command of ["cd", "mkdir", "rmdir"]) {
      expect(values(completePath(host, vfs, ctx(command, "/mnt/")))).toEqual(["/mnt/DOCS/"]);
      expect(values(completePath(host, vfs, ctx(command, "/dev/")))).toEqual([]);
    }
  });

  it("completes both of cp's paths, dd's if= and of= operands, and redirect targets", () => {
    const { host, vfs } = fat();
    expect(completePath(host, vfs, ctx("cp", "/mnt/DOCS/N", { argumentIndex: 1 }))).toEqual([{ value: "/mnt/DOCS/N.TXT", directory: false }]);
    expect(completePath(host, vfs, ctx("dd", "if=/dev/h", { argumentIndex: 2 }))).toEqual([{ value: "if=/dev/hda", directory: false }]);
    expect(completePath(host, vfs, ctx("dd", "of=/mnt/DO"))).toEqual([{ value: "of=/mnt/DOCS/", directory: true }]);
    expect(completePath(host, vfs, ctx("dd", "bs=5"))).toEqual([]);
    expect(completePath(host, vfs, ctx("echo", "/mnt/H", { kind: "redirect" }))).toEqual([{ value: "/mnt/Hello world.txt", directory: false }]);
  });

  it("offers nothing for flags, non-path commands, extra arguments, or missing directories", () => {
    const { host, vfs } = fat();
    expect(completePath(host, vfs, ctx("cat", "/m", { flag: "bytes" }))).toEqual([]);
    expect(completePath(host, vfs, ctx("seek", "/m"))).toEqual([]);
    expect(completePath(host, vfs, ctx("cat", "/m", { argumentIndex: 1 }))).toEqual([]);
    expect(completePath(host, vfs, ctx("cat", "/mnt/NOPE/"))).toEqual([]);
    expect(completePath(host, vfs, ctx("cat", "/mnt/DOCS/N.TXT/"))).toEqual([]);
    expect(completePath(host, vfs, ctx("cat", "/nowhere/"))).toEqual([]);
  });

  it("offers nothing under /mnt on a corrupt volume", () => {
    const { host, vfs } = fat();
    host.run((v) => v.writeRaw(0, new Uint8Array(512)));
    expect(completePath(host, vfs, ctx("ls", "/mnt/"))).toEqual([]);
    expect(completePath(host, vfs, ctx("ls", "/"))).toHaveLength(2);
  });
});

describe("PATH_COMMANDS", () => {
  it("names exactly the commands whose positionals are all paths, on every family", () => {
    for (const vol of [Volume.formatFat16(undefined), Volume.formatExt2(undefined), Volume.formatExt3(undefined)]) {
      const host = makeHost(vol);
      for (const { spec } of createCommands(host, new Vfs())) {
        const positionals = [...(spec.required ?? []), ...(spec.optional ?? [])];
        const pathLike = positionals.length > 0 && !spec.rest && positionals.every((p) => ["path", "src", "dst"].includes(p.name));
        expect(spec.name in PATH_COMMANDS, spec.name).toBe(pathLike);
        if (pathLike) expect(PATH_COMMANDS[spec.name].count, spec.name).toBe(positionals.length);
      }
    }
  });
});
