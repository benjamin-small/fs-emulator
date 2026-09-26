import { describe, expect, it } from "vitest";
import { PHASE_LABELS, groupEvents, type PhaseEvent } from "../src/core/eventPhases";
import { Volume } from "../src/lib/wasm";

const enc = (s: string) => new TextEncoder().encode(s);
const HELLO = enc("Hello world\n");
/** Each phase as `[id, label, how many events]`, in the order the panel lists them. */
const phases = (events: readonly PhaseEvent[]) => groupEvents(events).map((p) => [p.id, p.label, p.events.length]);
/** Events of the given kinds, with no text and no region. */
const kinds = (...list: string[]): PhaseEvent[] => list.map((kind) => ({ kind, text: "", region: null }));

describe("PHASE_LABELS", () => {
  it("names every phase the way the What changed panel's summaries read", () => {
    expect(PHASE_LABELS).toEqual({
      directory: "Directory entry",
      allocation: "Allocation",
      data: "Data",
      body: "Filesystem writes",
      journal: "Journal",
      checkpoint: "Checkpoint",
      cleanup: "Cleanup",
      recovery: "Recovery",
      crash: "Crash",
      other: "Other",
    });
  });
});

describe("groupEvents on real records", () => {
  it("groups a FAT16 create FAT-style: the table, the data, then the directory entry", () => {
    const rec = Volume.formatFat16(undefined).createFile("/hello.txt", HELLO);
    // Phases follow the first event of each: the FAT entries come before the data and the slot.
    expect(phases(rec.events)).toEqual([
      ["allocation", "Allocation", 3],
      ["data", "Data", 1],
      ["directory", "Directory entry", 1],
    ]);
    // Every event keeps its place in the record, so the panel can key and order by it.
    expect(groupEvents(rec.events).flatMap((p) => p.events.map((e) => e.index))).toEqual([0, 1, 2, 3, 4]);
    expect(groupEvents(rec.events)[0].events[0]).toEqual({ ...rec.events[0], index: 0 });
  });

  it("puts a FAT16 delete's slot, a raw write, and a grown directory where they belong", () => {
    const vol = Volume.formatFat16(undefined);
    vol.createFile("/a.txt", enc("x"));
    expect(phases(vol.deleteFile("/a.txt").events)).toEqual([["directory", "Directory entry", 1], ["allocation", "Allocation", 3]]);
    expect(phases(vol.writeRaw(100, enc("ab")).events)).toEqual([["data", "Data", 1]]);
    // A subdirectory's first cluster holds 64 slots, `.` and `..` included: the 63rd file grows it.
    vol.createDir("/d");
    for (let i = 0; i < 62; i++) vol.createFile(`/d/f${i}.txt`, enc("x"));
    const grown = vol.createFile("/d/f62.txt", enc("x"));
    expect(grown.events.some((e) => e.kind === "directory_grown")).toBe(true);
    expect(phases(grown.events)).toEqual([["allocation", "Allocation", 8], ["data", "Data", 1], ["directory", "Directory entry", 2]]);
  });

  it("groups an ext3 create ext3-style: the writes, the journal, the checkpoint, the cleanup", () => {
    const rec = Volume.formatExt3(undefined).createFile("/hello.txt", HELLO);
    expect(rec.events).toHaveLength(30);
    expect(phases(rec.events)).toEqual([
      ["body", "Filesystem writes", 10],
      ["journal", "Journal", 11],
      ["checkpoint", "Checkpoint", 7],
      ["cleanup", "Cleanup", 2],
    ]);
    // The Journal phase: the flag, the transaction, then its descriptor, seven copies, and the commit.
    const journal = groupEvents(rec.events)[1].events;
    expect(journal.slice(0, 2).map((e) => e.kind)).toEqual(["recovery_flag_set", "transaction_started"]);
    const texts = journal.slice(2).map((e) => e.text);
    expect(texts.filter((t) => t.startsWith("wrote descriptor"))).toHaveLength(1);
    expect(texts.filter((t) => t.startsWith("copied block"))).toHaveLength(7);
    expect(texts.filter((t) => t.startsWith("committed transaction"))).toHaveLength(1);
    expect(texts).toHaveLength(9);
  });

  it("groups an ext2 create FAT-style: no journal, so it reads like FAT's", () => {
    const rec = Volume.formatExt2(undefined).createFile("/hello.txt", HELLO);
    expect(phases(rec.events)).toEqual([
      ["allocation", "Allocation", 8],
      ["data", "Data", 1],
      ["directory", "Directory entry", 1],
    ]);
  });

  it("ends a crashed record with a Crash phase", () => {
    const vol = Volume.formatExt3(undefined);
    vol.armCrash("after_commit");
    const rec = vol.createFile("/c.txt", HELLO);
    expect(phases(rec.events)).toEqual([
      ["body", "Filesystem writes", 10],
      ["journal", "Journal", 11],
      ["crash", "Crash", 1],
    ]);
    expect(groupEvents(rec.events).at(-1)!.events[0].text).toBe("crashed after commit");
  });

  it("reads recover() as Recovery then Cleanup, and a discarded transaction as Recovery too", () => {
    const vol = Volume.formatExt3(undefined);
    vol.armCrash("after_commit");
    vol.createFile("/c.txt", HELLO);
    // The scan, then seven blocks replayed; the journal emptied and the flag cleared.
    expect(phases(vol.recover().events)).toEqual([["recovery", "Recovery", 8], ["cleanup", "Cleanup", 2]]);

    const before = Volume.formatExt3(undefined);
    before.armCrash("before_commit");
    before.createFile("/c.txt", HELLO);
    expect(phases(before.recover().events)).toEqual([["recovery", "Recovery", 2], ["cleanup", "Cleanup", 2]]);
    // A clean journal: one scan, nothing to clean up.
    expect(phases(Volume.formatExt3(undefined).recover().events)).toEqual([["recovery", "Recovery", 1]]);
  });
});

describe("groupEvents rules", () => {
  it("lists phases in the order of their first event, each phase's events in record order", () => {
    const grouped = groupEvents(kinds("data_written", "fat_entry_set", "data_written", "dir_entry_written"));
    expect(grouped.map((p) => p.id)).toEqual(["data", "allocation", "directory"]);
    expect(grouped[0].events.map((e) => e.index)).toEqual([0, 2]);
  });

  it("puts a kind it does not know in Other on a record without a journal", () => {
    expect(phases(kinds("dir_entry_written", "mystery", "data_written", "mystery"))).toEqual([
      ["directory", "Directory entry", 1],
      ["other", "Other", 2],
      ["data", "Data", 1],
    ]);
  });

  it("counts every non-journal kind as a filesystem write once any journal kind is present", () => {
    expect(phases(kinds("mystery", "dir_entry_written", "checkpointed"))).toEqual([["body", "Filesystem writes", 2], ["checkpoint", "Checkpoint", 1]]);
    // Any one journal kind is enough to switch the whole record to the ext3 grouping.
    expect(phases(kinds("data_written", "crashed"))).toEqual([["body", "Filesystem writes", 1], ["crash", "Crash", 1]]);
  });

  it("gives nothing for a record with no events", () => {
    expect(groupEvents([])).toEqual([]);
  });
});
