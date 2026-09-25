import { describe, expect, it } from "vitest";
import type { OpRecord, Volume } from "../src/lib/wasm";
import { FAMILIES } from "../src/fs";
import type { FsAdapter } from "../src/fs/adapter";
import { biggerText } from "../src/scenarios/fundamentals";
import { COMMIT_INDEX, COPY_COUNT, DESCRIPTOR_INDEX, FLAG_WORD, NOTES_BYTES, NOTES_PATH, S_SEQUENCE, S_START, scenario } from "../src/scenarios/journaledWrite";
import { lessonHelpers } from "./fixtures/lesson";

// "A journaled write" follows one create through the journal and quotes where each piece
// landed (descriptor at block 83, seven copies, commit at block 91, ...). Those positions are
// read here from the create's own record, the events and byte changes the core reported, so the
// lesson's claims are the core's, not a guess about how it lays a transaction out.

const n = (v: number) => v.toLocaleString("en-US");
const be32 = (b: Uint8Array, at: number) => new DataView(b.buffer, b.byteOffset, b.byteLength).getUint32(at, false);
const { stepText, runThrough, focusOf } = lessonHelpers(scenario);

/** The whole lesson, run the way the runner does. Only the first step has an action. */
function run(): { vol: Volume; fs: FsAdapter; rec: OpRecord } {
  const { vol, fs, records } = runThrough();
  expect(records).toHaveLength(1);
  return { vol, fs, rec: records[0] };
}

/** The journal blocks the record wrote, from its `journal_block_written` events. */
function journalWrites(rec: OpRecord) {
  return rec.events.filter((e) => e.kind === "journal_block_written").map((e) => {
    const m = /^(?:wrote descriptor for transaction (\d+)|copied block (\d+) into|committed transaction (\d+)) .*journal block (\d+) \(block (\d+)\)$/.exec(e.text);
    if (!m) throw new Error(`unexpected event text: ${e.text}`);
    const kind = m[1] ? "descriptor" : m[2] ? "copy" : "commit";
    return { kind, home: m[2] ? Number(m[2]) : null, index: Number(m[4]), block: Number(m[5]) };
  });
}

describe("the journaled-write lesson", () => {
  it("runs on the default ext3 disk in ordered mode", () => {
    expect(scenario.id).toBe("journaled-write");
    expect(scenario.title).toBe("A journaled write");
    expect(scenario.family).toBe("ext");
    const vol = FAMILIES.ext.format();
    expect(vol.fsType()).toBe("ext3");
    expect(vol.journalInfo()).toMatchObject({ mode: "ordered", firstBlock: 82, start: 0, sequence: 1 });
    expect(stepText("One create, one transaction")).toContain("in ordered mode");
  });

  it("creates the quoted file in three blocks", () => {
    const { vol, rec } = run();
    expect(rec.op).toBe(`create_file ${NOTES_PATH}`);
    expect(vol.readFile(NOTES_PATH)).toEqual(biggerText(NOTES_BYTES));
    expect(vol.fileBlocks(NOTES_PATH).data).toEqual([1111, 1112, 1113]);
    expect(NOTES_BYTES - 2 * 1024).toBe(952);
    expect(stepText("One create, one transaction")).toContain(`created ${NOTES_PATH}, ${n(NOTES_BYTES)} bytes`);
    const data = stepText("Data first");
    expect(data).toContain("home blocks, 1111 to 1113");
    expect(data).toContain(`${n(NOTES_BYTES)} bytes fill two blocks and 952 bytes of the third`);
  });

  it("sets the needs_recovery flag first and clears it last", () => {
    const { vol, rec } = run();
    const flag = rec.changes.filter((c) => c.offset === FLAG_WORD);
    expect(flag.map((c) => [c.before[0], c.after[0]])).toEqual([[0x02, 0x06], [0x06, 0x02]]);
    expect(rec.changes[0].offset).toBe(FLAG_WORD);
    expect(rec.changes[rec.changes.length - 1].offset).toBe(FLAG_WORD);
    expect(vol.sector(1)[0x60]).toBe(0x02);
    expect(vol.annotateSector(1).find((a) => a.range.start === 0x60)?.label).toBe("incompatible features");
    const firstJournal = rec.changes.findIndex((c) => c.offset >= 82 * 1024 && c.offset < 1106 * 1024);
    expect(firstJournal).toBeGreaterThan(0);
    const text = stepText("One create, one transaction");
    expect(text).toContain("block 1 + 0x60 reads 0x02 now, but while the create ran it read 0x06: bit 0x04 is needs_recovery");
    expect(stepText("The flag clears")).toContain("0x06 back to 0x02");
  });

  it("writes the data blocks before anything touches the journal", () => {
    const { rec } = run();
    const at = (block: number) => rec.changes.findIndex((c) => Math.floor(c.offset / 1024) === block);
    const firstJournal = rec.changes.findIndex((c) => c.offset >= 82 * 1024 && c.offset < 1106 * 1024);
    for (const b of [1111, 1112, 1113]) expect(at(b)).toBeLessThan(firstJournal);
    const events = rec.events.map((e) => e.kind);
    expect(events.indexOf("data_written")).toBeLessThan(events.indexOf("transaction_started"));
  });

  it("moves s_start to 1 for the transaction and back to 0, and s_sequence to 2", () => {
    const { vol, rec } = run();
    const at = (label: string) => vol.annotateSector(82).find((a) => a.label === label)?.range.start;
    expect([at("sequence"), at("start")]).toEqual([S_SEQUENCE, S_START]);
    // Both journal-superblock writes cover s_sequence and s_start (8 bytes at +0x18).
    const jsb = rec.changes.filter((c) => c.offset === 82 * 1024 + S_SEQUENCE);
    expect(jsb.map((c) => [be32(c.before, 4), be32(c.after, 4)])).toEqual([[0, 1], [1, 0]]);
    expect(jsb.map((c) => be32(c.after, 0))).toEqual([1, 2]);
    expect(vol.journalInfo()).toMatchObject({ sequence: 2, start: 0 });
    const started = rec.events.find((e) => e.kind === "transaction_started")!;
    expect(started.text).toBe(`started transaction 1 at journal block ${DESCRIPTOR_INDEX} (${COPY_COUNT} tagged blocks)`);
    expect(started.region).toEqual({ start: 82 * 1024 + S_SEQUENCE, end: 82 * 1024 + S_SEQUENCE + 8 });
    const text = stepText("The journal superblock");
    for (const s of ["block 82, the journal's own superblock", "s_start field, at +0x1C, went from 0 to 1", "starts at journal block 1", "It reads 0 again now", "transaction_started event"]) {
      expect(text).toContain(s);
    }
    expect(stepText("The journal empties")).toContain("s_sequence at +0x18 becomes 2");
    expect(stepText("The journal empties")).toContain("s_start at +0x1C goes back to 0");
  });

  it("writes one descriptor, seven copies in tag order, and a commit, where the lesson says", () => {
    const { vol, rec } = run();
    const writes = journalWrites(rec);
    expect(writes.map((w) => w.kind)).toEqual(["descriptor", ...new Array(COPY_COUNT).fill("copy"), "commit"]);
    expect(writes[0]).toMatchObject({ index: DESCRIPTOR_INDEX, block: 83 });
    expect(writes.slice(1, -1).map((w) => w.block)).toEqual([84, 85, 86, 87, 88, 89, 90]);
    expect(writes.slice(1, -1).map((w) => w.home)).toEqual([1, 2, 3, 4, 5, 6, 69]);
    expect(writes[writes.length - 1]).toMatchObject({ index: COMMIT_INDEX, block: 91 });
    expect(COMMIT_INDEX).toBe(9);
    // The descriptor's tags, as the Inspector decodes them: the seven homes, ascending.
    const tags = vol.annotateSector(83).filter((a) => a.label.startsWith("tag ")).map((a) => Number(/^block (\d+)/.exec(a.value)![1]));
    expect(tags).toEqual([1, 2, 3, 4, 5, 6, 69]);
    expect(Array.from(vol.sector(83).slice(0, 12))).toEqual([0xc0, 0x3b, 0x39, 0x98, 0, 0, 0, 1, 0, 0, 0, 1]);
    // The two inode-table blocks: inode 2 in block 5, inode 12 in block 6.
    expect([vol.extInode(2).slot.block, vol.extInode(12).slot.block]).toEqual([5, 6]);
    // The first copy is the superblock with the flag set; the commit is type 2 for transaction 1.
    expect(vol.sector(84)[0x60]).toBe(0x06);
    expect(Array.from(vol.sector(91).slice(0, 12))).toEqual([0xc0, 0x3b, 0x39, 0x98, 0, 0, 0, 2, 0, 0, 0, 1]);
    const desc = stepText("The descriptor");
    for (const s of ["Journal block 1 is block 83", "magic C0 3B 39 98, block type 1, transaction 1", "in ascending order: 1, 2, 3, 4, 5, 6, and 69", "inodes 2 and 12", `${COPY_COUNT} tags`]) {
      expect(desc).toContain(s);
    }
    const copies = stepText("The copies");
    expect(copies).toContain(`Blocks 84 to 90 are the ${COPY_COUNT} copies, in tag order`);
    expect(copies).toContain("Block 84 is the superblock's image, and at +0x60 it reads 0x06");
    const commit = stepText("The commit");
    expect(commit).toContain("Block 91, journal block 9, is the commit block: the magic, block type 2, transaction 1");
  });

  it("checkpoints after the commit, naming the file in block 69", () => {
    const { fs, vol, rec } = run();
    const commitAt = rec.events.findIndex((e) => e.kind === "journal_block_written" && e.text.startsWith("committed"));
    const checkpoints = rec.events.map((e, i) => [e, i] as const).filter(([e]) => e.kind === "checkpointed");
    expect(checkpoints).toHaveLength(COPY_COUNT);
    for (const [, i] of checkpoints) expect(i).toBeGreaterThan(commitAt);
    expect(checkpoints.map(([e]) => Number(/^checkpointed block (\d+)/.exec(e.text)![1]))).toEqual([1, 2, 3, 4, 5, 6, 69]);
    const entry = vol.dirEntries("/").find((e) => e.name === "notes.txt")!;
    expect(entry).toMatchObject({ block: 69, offset: 69 * 1024 + 0x2c, inode: 12 });
    expect(stepText("Checkpoint")).toContain(`write the ${COPY_COUNT} blocks to their home locations`);
    expect(stepText("Checkpoint")).toContain("Block 69, the root directory, now names notes.txt at +0x2C, inode 12");
    // One transaction per operation, checkpointed at once: the journal is empty and the flag
    // clear when the create returns, which the copy calls a cleanly unmounted disk.
    expect(vol.journalInfo()).toMatchObject({ start: 0, needsRecovery: false });
    expect(stepText("Checkpoint")).toContain("this emulator checkpoints each transaction at once");
    expect(fs.chain("/")).toEqual([69]);
  });

  it("leaves transaction 1 stale at journal blocks 1 to 9", () => {
    const { fs } = run();
    const ring = fs.journal!.blocks();
    const tx = ring.filter((b) => b.tid === 1);
    expect(tx.map((b) => b.index)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9]);
    expect(tx.every((b) => b.stale)).toBe(true);
    expect(stepText("The flag clears")).toContain("transaction 1 at journal blocks 1 to 9, faded because it is stale");
  });

  it("resolves every step's focus against the live volume", () => {
    const { fs } = run();
    expect(scenario.steps.map((s) => [s.title, focusOf(s.title, fs)])).toEqual([
      ["One create, one transaction", { path: NOTES_PATH, offset: 1024 + 0x60 }],
      ["Data first", { path: NOTES_PATH, sector: 1111 }],
      ["The journal superblock", { offset: 82 * 1024 + 0x1c }],
      ["The descriptor", { sector: 83 }],
      ["The copies", { sector: 84 }],
      ["The commit", { sector: 91 }],
      ["Checkpoint", { path: NOTES_PATH, offset: 69 * 1024 + 0x2c }],
      ["The journal empties", { offset: 82 * 1024 + 0x18 }],
      ["The flag clears", { path: NOTES_PATH, offset: 1024 + 0x60 }],
    ]);
  });
});
