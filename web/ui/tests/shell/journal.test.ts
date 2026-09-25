import { describe, expect, it } from "vitest";
import { Volume } from "../../src/lib/wasm";
import { commandSetOf, createCommands, registerCommands, registrationChange, tabBanner, type CommandRegistry } from "../../src/shell/register";
import type { CommandDef } from "../../src/shell/types";
import { journalCommands } from "../../src/shell/journalCommands";
import { Vfs } from "../../src/shell/vfs";
import { call, callErr, makeHost } from "./helpers";

const enc = (s: string) => new TextEncoder().encode(s);
const names = (defs: { spec: { name: string } }[]) => defs.map((d) => d.spec.name);

/** A fresh default ext3 disk (16,384 blocks, journal at 82..1105) behind a test host. */
function setup() {
  const host = makeHost(Volume.formatExt3(undefined));
  const vfs = new Vfs();
  return { host, vfs, defs: createCommands(host, vfs) };
}

describe("which hosts get crash and recover", () => {
  it("registers both on an ext3 host, after every other command", () => {
    const { defs } = setup();
    expect(names(defs).slice(-2)).toEqual(["crash", "recover"]);
  });

  it("registers neither on a FAT16 host or an ext2 host", () => {
    for (const vol of [Volume.formatFat16(undefined), Volume.formatExt2(undefined)]) {
      const defs = names(createCommands(makeHost(vol)));
      expect(defs, vol.fsType()).not.toContain("crash");
      expect(defs, vol.fsType()).not.toContain("recover");
    }
  });

  it("words the two summaries and crash's flags", () => {
    const [crash, recover] = journalCommands(setup().host).map((d) => d.spec);
    expect(crash.summary).toBe("Arm a crash in the journal: the next change to /dev/hda stops mid-transaction");
    expect(crash.flags).toEqual([
      { long: "at", shape: "str", desc: "where the change stops: before-commit, after-commit (default), during-checkpoint" },
      { long: "off", desc: "disarm the armed crash" },
    ]);
    expect(recover.summary).toBe("Replay or discard the journal's unfinished transaction");
  });
});

describe("crash", () => {
  it("arms after-commit by default, each --at phase by name, and --off disarms", async () => {
    const { host, defs } = setup();
    const journal = host.adapter.journal!;
    const armed = await call(defs, "crash");
    expect(armed.log).toEqual(["armed: the next change to /dev/hda stops after commit"]);
    expect(armed.value).toBeUndefined();
    expect(journal.phase()).toBe("after_commit");
    expect((await call(defs, "crash", { flags: { at: "before-commit" } })).log).toEqual(["armed: the next change to /dev/hda stops before commit"]);
    expect(journal.phase()).toBe("before_commit");
    expect((await call(defs, "crash", { flags: { at: "during-checkpoint" } })).log).toEqual(["armed: the next change to /dev/hda stops during checkpoint"]);
    expect(journal.phase()).toBe("during_checkpoint");
    expect((await call(defs, "crash", { flags: { at: "after-commit" } })).log).toEqual(["armed: the next change to /dev/hda stops after commit"]);
    expect(journal.phase()).toBe("after_commit");
    expect((await call(defs, "crash", { flags: { off: true } })).log).toEqual(["disarmed"]);
    expect(journal.phase()).toBeNull();
    expect(host.historyLength).toBe(0); // arming is not a recorded operation
    // Every arm and disarm went through the host (the store, in the app, so the Journal panel
    // shows it at once), not straight to the capability.
    expect(host.crashes).toEqual(["after_commit", "before_commit", "during_checkpoint", "after_commit", null]);
  });

  it("refuses an unknown phase, and --at with --off, leaving the journal alone", async () => {
    const { host, defs } = setup();
    for (const at of ["after_commit", "later", ""]) {
      const e = await callErr(defs, "crash", { flags: { at } });
      expect(e.message).toBe(`unknown crash phase '${at}'`);
      expect(e.help).toBe("phases: before-commit, after-commit, during-checkpoint");
    }
    expect((await callErr(defs, "crash", { flags: { at: "before-commit", off: true } })).message).toBe("give --at or --off, not both");
    expect(host.adapter.journal!.phase()).toBeNull();
  });

  it("arms while the timeline is rewound, with the rewound warning", async () => {
    const { host, defs } = setup();
    host.run((v) => v.createFile("/a.txt", enc("a")));
    host.run((v) => v.createFile("/b.txt", enc("b")));
    host.rewind(0);
    const r = await call(defs, "crash", { flags: { at: "before-commit" } });
    expect(r.err).toEqual(['showing the latest state, not step 1 of 2; click "Back to now" or run a write command']);
    expect(r.log).toEqual(["armed: the next change to /dev/hda stops before commit"]);
    expect(host.adapter.journal!.phase()).toBe("before_commit");
    expect((await call(defs, "crash", { flags: { off: true } })).err).toHaveLength(1);
    host.rewind(1);
    expect((await call(defs, "crash")).err).toEqual([]);
  });
});

describe("recover", () => {
  it("replays a transaction that crashed after its commit, one line per event, as a timeline step", async () => {
    const { host, defs } = setup();
    await call(defs, "crash");
    await call(defs, "write", { positionals: ["/mnt/x.txt"] }, "hi\n");
    expect(host.history.at(-1)!.events.at(-1)!.text).toBe("crashed after commit");
    expect(host.adapter.needsRecovery).toBe(true);
    expect(host.adapter.journal!.phase()).toBeNull(); // the crash used the armed phase up
    const r = await call(defs, "recover");
    expect(r.value).toBeUndefined();
    expect(r.log).toEqual([
      "scanned the journal from block 1, sequence 1: 1 committed transaction, 7 tagged blocks",
      "replayed block 1 from journal block 2 (transaction 1)",
      "replayed block 2 from journal block 3 (transaction 1)",
      "replayed block 3 from journal block 4 (transaction 1)",
      "replayed block 4 from journal block 5 (transaction 1)",
      "replayed block 5 from journal block 6 (transaction 1)",
      "replayed block 6 from journal block 7 (transaction 1)",
      "replayed block 69 from journal block 8 (transaction 1)",
      "journal emptied; next transaction 3",
      "cleared needs_recovery in the superblock",
    ]);
    expect(host.history.map((h) => h.op)).toEqual(["create_file /x.txt (crashed after commit)", "recover"]);
    expect(host.cursor).toBe(1);
    expect(host.adapter.needsRecovery).toBe(false);
    expect(host.vol.listDir("/").map((e) => e.name)).toEqual(["lost+found", "x.txt"]);
  });

  it("discards a transaction that crashed before its commit", async () => {
    const { host, defs } = setup();
    await call(defs, "crash", { flags: { at: "before-commit" } });
    await call(defs, "write", { positionals: ["/mnt/x.txt"] }, "hi\n");
    expect((await call(defs, "recover")).log).toEqual([
      "scanned the journal from block 1, sequence 1: 0 committed transactions, 0 tagged blocks",
      "discarded uncommitted transaction 1 (7 tagged blocks)",
      "journal emptied; next transaction 2",
      "cleared needs_recovery in the superblock",
    ]);
    expect(host.vol.listDir("/").map((e) => e.name)).toEqual(["lost+found"]);
  });

  it("prints the single line of a clean journal, and still records the step", async () => {
    const { host, defs } = setup();
    expect((await call(defs, "recover")).log).toEqual(["journal is clean; nothing to replay"]);
    expect(host.history.map((h) => h.op)).toEqual(["recover"]);
  });

  it("goes through fsCall: a change on a volume that needs recovery is refused with the wasm code", async () => {
    const { defs } = setup();
    await call(defs, "crash");
    await call(defs, "write", { positionals: ["/mnt/x.txt"] }, "hi\n");
    const e = await callErr(defs, "mkdir", { positionals: ["/mnt/D"] });
    expect(e.code).toBe("NeedsRecovery");
    expect(e.message).toBe("/mnt/D: needs recovery");
    await call(defs, "recover");
    await call(defs, "mkdir", { positionals: ["/mnt/D"] });
  });

  it("names the missing journal when the mounted volume lost it after registration", async () => {
    const { host, defs } = setup();
    host.format("ext", { variant: "ext2" }); // the app re-registers here; a stale def still explains itself
    for (const name of ["crash", "recover"]) {
      const e = await callErr(defs, name);
      expect(e.message, name).toBe("/dev/hda has no journal (ext2)");
      expect(e.help, name).toBe("crash and recover need an ext3 volume: mkfs --type ext3");
    }
  });
});

describe("re-registration when the family changes", () => {
  /** browser-terminal's two registration calls over a map, logging each call in order. */
  function fakeTerminal() {
    const registered = new Map<string, CommandDef["fn"]>();
    const calls: string[] = [];
    const term: CommandRegistry = {
      registerCommand: (spec, fn) => {
        registered.set(spec.name, fn);
        calls.push(`+${spec.name}`);
      },
      unregisterCommand: (name) => {
        registered.delete(name);
        calls.push(`-${name}`);
      },
    };
    return { term, registered, calls };
  }

  it("keys the command set on the family and the journal's presence", () => {
    expect(commandSetOf(makeHost().adapter)).toBe("fat16");
    expect(commandSetOf(makeHost(Volume.formatExt2(undefined)).adapter)).toBe("ext");
    expect(commandSetOf(makeHost(Volume.formatExt3(undefined)).adapter)).toBe("ext+journal");
  });

  it("swaps every name for the new command set (fat16, ext3, ext2, fat16), keeping the working directory", async () => {
    const host = makeHost();
    const vfs = new Vfs();
    const { term, registered, calls } = fakeTerminal();
    const fat = registerCommands(term, host, vfs);
    expect(fat).toEqual(names(createCommands(host, vfs)));
    expect([...registered.keys()]).toEqual(fat);
    expect(fat).not.toContain("crash");

    // Each tab formats its own family only, so the ext3 set comes from an ext host.
    const ext = makeHost(Volume.formatExt3(undefined));
    await call(createCommands(ext, vfs), "mkdir", { positionals: ["/mnt/d"] });
    vfs.cwd = "/mnt/d";
    calls.length = 0;
    const ext3 = registerCommands(term, ext, vfs, fat);
    expect(calls.slice(0, fat.length)).toEqual(fat.map((n) => `-${n}`)); // every old name goes first
    expect(calls.slice(fat.length)).toEqual(ext3.map((n) => `+${n}`));
    expect([...registered.keys()]).toEqual(ext3);
    expect(ext3.slice(-2)).toEqual(["crash", "recover"]);
    const pwd = await call([{ spec: { name: "pwd", summary: "" }, fn: registered.get("pwd")! }], "pwd");
    expect(pwd.value).toBe("/mnt/d"); // the new definitions share the old working directory

    // ext3 to ext2 keeps the family but loses the journal: crash and recover go.
    ext.format("ext", { variant: "ext2" });
    const ext2 = registerCommands(term, ext, vfs, ext3);
    expect(ext2).toEqual(ext3.slice(0, -2));
    expect(registered.has("crash")).toBe(false);
    expect(registered.has("recover")).toBe(false);

    expect(registerCommands(term, host, vfs, ext2)).toEqual(fat);
    expect(registered.has("crash")).toBe(false);
  });

  it("keeps each tab's working directory: a switch registers over that tab's own vfs", async () => {
    const fat = { host: makeHost(), vfs: new Vfs() };
    const ext = { host: makeHost(Volume.formatExt3(undefined)), vfs: new Vfs() };
    const { term, registered } = fakeTerminal();
    const pwd = async () => (await call([{ spec: { name: "pwd", summary: "" }, fn: registered.get("pwd")! }], "pwd")).value;

    let names = registerCommands(term, fat.host, fat.vfs);
    await call(createCommands(fat.host, fat.vfs), "mkdir", { positionals: ["/mnt/D"] });
    fat.vfs.cwd = "/mnt/D";
    expect(await pwd()).toBe("/mnt/D");

    names = registerCommands(term, ext.host, ext.vfs, names);
    expect(await pwd()).toBe("/mnt"); // the ext tab's shell starts at the mount point
    expect(registered.has("crash")).toBe(true);

    registerCommands(term, fat.host, fat.vfs, names);
    expect(await pwd()).toBe("/mnt/D"); // back on FAT16, where it was left
    expect(registered.has("crash")).toBe(false);
  });
});

describe("one terminal following the active tab", () => {
  const fatTab = { id: "fat16" };
  const extTab = { id: "ext" };

  it("registers on first creation without a banner", () => {
    expect(registrationChange(null, { ws: fatTab, set: "fat16" })).toEqual({ register: true, banner: false });
  });

  it("re-registers and prints the banner when the tab changes, whatever the sets", () => {
    expect(registrationChange({ ws: fatTab, set: "fat16" }, { ws: extTab, set: "ext+journal" })).toEqual({ register: true, banner: true });
    expect(registrationChange({ ws: extTab, set: "ext+journal" }, { ws: fatTab, set: "fat16" })).toEqual({ register: true, banner: true });
    expect(registrationChange({ ws: fatTab, set: "ext" }, { ws: extTab, set: "ext" })).toEqual({ register: true, banner: true });
  });

  it("re-registers without a banner when the tab's own set changes (ext3 formatted as ext2)", () => {
    expect(registrationChange({ ws: extTab, set: "ext+journal" }, { ws: extTab, set: "ext" })).toEqual({ register: true, banner: false });
  });

  it("leaves the registration alone when neither changed (a format of the same type, a load)", () => {
    expect(registrationChange({ ws: fatTab, set: "fat16" }, { ws: fatTab, set: "fat16" })).toEqual({ register: false, banner: false });
  });

  it("names the tab and the type its disk reports in the banner", () => {
    expect(tabBanner("ext", "ext3")).toBe("-- ext tab: /dev/hda is ext3 --");
    expect(tabBanner("ext", "ext2")).toBe("-- ext tab: /dev/hda is ext2 --");
    expect(tabBanner("fat16", "FAT16")).toBe("-- FAT16 tab: /dev/hda is FAT16 --");
  });
});
