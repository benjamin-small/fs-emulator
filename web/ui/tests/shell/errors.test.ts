import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { ShellError, fsPhrase, wrapFs } from "../../src/shell/errors";
import { atLatest, statusToError, type ShellHost } from "../../src/shell/host";

describe("ShellError", () => {
  it("is an Error carrying help and code", () => {
    const e = new ShellError("nothing to write", { help: "pipe text in", code: "X" });
    expect(e).toBeInstanceOf(Error);
    expect(e.message).toBe("nothing to write");
    expect(e.help).toBe("pipe text in");
    expect(e.code).toBe("X");
    expect(e.name).toBe("ShellError");
  });
  it("leaves help and code unset when not given (the engine reads them by Reflect.get)", () => {
    const e = new ShellError("plain");
    expect("help" in e).toBe(false);
    expect("code" in e).toBe(false);
  });
});

describe("wrapFs", () => {
  it("uses coreutils phrases for the mapped codes and the raw message otherwise", () => {
    expect(fsPhrase("NotFound", "x")).toBe("No such file or directory");
    expect(fsPhrase("AlreadyExists", "x")).toBe("File exists");
    expect(fsPhrase("IsADirectory", "x")).toBe("Is a directory");
    expect(fsPhrase("NotADirectory", "x")).toBe("Not a directory");
    expect(fsPhrase("DirectoryNotEmpty", "x")).toBe("Directory not empty");
    expect(fsPhrase("DiskFull", "x")).toBe("No space left on device");
    expect(fsPhrase("OutOfBounds", "x")).toBe("Range runs past the end of the disk");
    expect(fsPhrase("CorruptImage", "boot sector no longer parses")).toBe("boot sector no longer parses");
    expect(fsPhrase(undefined, "raw text")).toBe("raw text");
  });
  it("wraps a real wasm error as `display: phrase` with no command prefix and keeps the code", () => {
    const vol = Volume.formatFat16(undefined);
    let caught: unknown;
    try { vol.readFile("/NOPE.TXT"); } catch (e) { caught = e; }
    const w = wrapFs("/mnt/NOPE.TXT", caught);
    expect(w).toBeInstanceOf(ShellError);
    expect(w.message).toBe("/mnt/NOPE.TXT: No such file or directory");
    expect(w.code).toBe("NotFound");
  });
  it("passes a ShellError through untouched and stringifies non-errors", () => {
    const own = new ShellError("give --at <addr>", { help: "h" });
    expect(wrapFs("/dev/hda", own)).toBe(own);
    expect(wrapFs("/mnt/a", "boom").message).toBe("/mnt/a: boom");
    expect(wrapFs("/mnt/a", { message: "custom", code: "Weird" }).message).toBe("/mnt/a: custom");
    expect(wrapFs("/mnt/a", { message: "custom", code: "Weird" }).code).toBe("Weird");
  });
});

describe("host helpers", () => {
  const stub = (cursor: number, historyLength: number): ShellHost => ({
    vol: Volume.formatFat16(undefined), cursor, historyLength,
    run: () => { throw new Error("unused"); }, format: () => {}, select: () => {}, jumpTo: () => {}, closeTerminal: () => {},
  });
  it("atLatest is true before any op and at the last step only", () => {
    expect(atLatest(stub(-1, 0))).toBe(true);
    expect(atLatest(stub(2, 3))).toBe(true);
    expect(atLatest(stub(1, 3))).toBe(false);
  });
  it("statusToError turns the store's status into a throwable with the code", () => {
    const e = statusToError({ text: "disk full", code: "DiskFull" });
    expect(e).toBeInstanceOf(Error);
    expect(e.message).toBe("disk full");
    expect(e.code).toBe("DiskFull");
    expect(statusToError(null).message).toBe("the operation failed without a message");
  });
});
