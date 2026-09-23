import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { CORRUPT_HELP, ShellError, fsCall, fsPhrase, wrapFs } from "../../src/shell/errors";
import { atLatest, corruptionOf, statusToError, type ShellHost } from "../../src/shell/host";

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
    expect(fsPhrase("constructor", "raw text")).toBe("raw text"); // an inherited key is not a phrase
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
  it("attaches the recovery hint to every CorruptImage error", () => {
    const w = wrapFs("/mnt/A.TXT", { message: "corrupt image: x", code: "CorruptImage" });
    expect(w.message).toBe("/mnt/A.TXT: corrupt image: x");
    expect(w.code).toBe("CorruptImage");
    expect(CORRUPT_HELP).toContain("dd --of=/dev/hda"); // the constant itself, not just "both undefined"
    expect(w.help).toBe(CORRUPT_HELP);
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
  it("corruptionOf returns null when corruption() itself throws (a future NotFat volume)", () => {
    const vol = { corruption: () => { throw new Error("NotFat"); } };
    expect(corruptionOf(vol as unknown as Volume)).toBeNull();
    expect(corruptionOf(Volume.formatFat16(undefined))).toBeNull();
  });
  it("statusToError turns the store's status into a throwable with the code", () => {
    const e = statusToError({ text: "disk full", code: "DiskFull" });
    expect(e).toBeInstanceOf(Error);
    expect(e.message).toBe("disk full");
    expect(e.code).toBe("DiskFull");
    expect(statusToError(null).message).toBe("the operation failed without a message");
  });
  it("statusToError stays wrappable, so a command's fsCall still adds the path and the phrase", () => {
    // The store host throws it from inside a command's fsCall, exactly where a raw wasm
    // error would land. A ShellError would pass through wrapFs untouched and print the
    // bare store text, so the app would say "already exists" where the tests say
    // "/mnt/A.TXT: File exists".
    let caught: unknown;
    try {
      fsCall("/mnt/A.TXT", () => {
        throw statusToError({ text: "already exists", code: "AlreadyExists" });
      });
    } catch (e) {
      caught = e;
    }
    expect(caught).toBeInstanceOf(ShellError);
    expect((caught as ShellError).message).toBe("/mnt/A.TXT: File exists");
    expect((caught as ShellError).code).toBe("AlreadyExists");
  });
});
