import { describe, expect, it } from "vitest";
import type { Volume } from "../src/lib/wasm";
import { FAMILIES } from "../src/fs";
import { biggerText } from "../src/scenarios/fundamentals";
import { CRASH_BYTES, CRASH_PATH, LOST_BLOCK, LOST_PATH, LOST_TEXT, scenario } from "../src/scenarios/crashRecover";
import { lessonHelpers } from "./fixtures/lesson";

// "Crash and recover" arms two crashes and recovers from each. These tests run it the way the
// runner does and pin, after every action, the state the copy describes (the flag, the root
// directory, the bitmap, the journal ring, the orphaned bytes), every quoted number, and every
// step's resolved focus.

const { stepText, runThrough, focusOf } = lessonHelpers(scenario);
const names = (vol: Volume) => vol.dirEntries("/").map((e) => e.name);
/** Group 0's block-bitmap bit for block `b` (bit 0 is block 1). */
const inUse = (vol: Volume, b: number) => ((vol.sector(3)[Math.floor((b - 1) / 8)] >> ((b - 1) % 8)) & 1) === 1;

describe("the crash-recover lesson", () => {
  it("runs on the default ext3 disk", () => {
    expect(scenario.id).toBe("crash-recover");
    expect(scenario.title).toBe("Crash and recover");
    expect(scenario.family).toBe("ext");
    expect(FAMILIES.ext.format().journalInfo()).toMatchObject({ mode: "ordered" });
  });

  it("step 1: the create crashes after its commit and the volume needs recovery", () => {
    const { vol, fs, records } = runThrough("Crash after the commit");
    const [rec] = records;
    expect(rec.op).toBe(`create_file ${CRASH_PATH} (crashed after commit)`);
    expect(rec.events[rec.events.length - 1]).toMatchObject({ kind: "crashed", text: "crashed after commit" });
    expect(rec.events.some((e) => e.kind === "checkpointed")).toBe(false);
    expect(fs.needsRecovery).toBe(true);
    expect(vol.needsRecovery()).toBe(true);
    expect(fs.journal!.phase()).toBeNull();
    expect(vol.sector(1)[0x60]).toBe(0x06);
    expect(() => vol.stat(CRASH_PATH)).toThrow(expect.objectContaining({ code: "NotFound" }));
    expect(vol.listDir("/").map((e) => e.name)).toEqual(["lost+found"]);
    expect(Math.ceil(CRASH_BYTES / 1024)).toBe(2);
    const text = stepText("Crash after the commit");
    for (const s of ["armed a crash after commit", `created ${CRASH_PATH}, two blocks`, "block 1 + 0x60 reads 0x06", "does not list crash.txt"]) {
      expect(text).toContain(s);
    }
  });

  it("step 2: transaction 1 is live in the journal at blocks 83 to 91", () => {
    const { vol, fs } = runThrough("Crash after the commit");
    expect(fs.journal!.state()).toMatchObject({ firstBlock: 82, start: 1, sequence: 1, needsRecovery: true });
    const tx = fs.journal!.blocks().filter((b) => b.tid === 1);
    expect(tx.map((b) => [b.index, b.block, b.kind])).toEqual([
      [1, 83, "descriptor"], [2, 84, "copy"], [3, 85, "copy"], [4, 86, "copy"], [5, 87, "copy"], [6, 88, "copy"], [7, 89, "copy"], [8, 90, "copy"], [9, 91, "commit"],
    ]);
    expect(tx.every((b) => !b.stale)).toBe(true);
    expect(vol.annotateSector(83).filter((a) => a.label.startsWith("tag "))).toHaveLength(7);
    const text = stepText("A live transaction");
    for (const s of ["Journal block 1, block 83, is transaction 1's descriptor", "the same 7 blocks", "copies are blocks 84 to 90", "commit is block 91"]) {
      expect(text).toContain(s);
    }
  });

  it("step 3: the root directory has no entry yet, and the data blocks are written but free", () => {
    const { vol } = runThrough("Crash after the commit");
    expect(names(vol)).toEqual([".", "..", "lost+found"]);
    expect(vol.dirEntries("/")[2]).toMatchObject({ name: "lost+found", recLen: 1000, offset: 69 * 1024 + 24 });
    expect(24 + 1000).toBe(1024);
    expect(vol.sector(1111)).toEqual(biggerText(CRASH_BYTES).slice(0, 1024));
    expect(vol.sector(1112)).toEqual(biggerText(CRASH_BYTES).slice(1024, 2048));
    expect([inUse(vol, 1111), inUse(vol, 1112)]).toEqual([false, false]);
    const text = stepText("Home blocks still old");
    for (const s of ["Block 69, the root directory", "rec_len of 1000", "no entry for crash.txt", "blocks 1111 and 1112", "calls both free"]) {
      expect(text).toContain(s);
    }
  });

  it("step 4: recovery replays the transaction and the entry appears", () => {
    const { vol, fs, records } = runThrough("Recover: replay");
    const rec = records[1];
    expect(rec.op).toBe("recover");
    expect(rec.events[0]).toMatchObject({ kind: "recovery_scanned", text: "scanned the journal from block 1, sequence 1: 1 committed transaction, 7 tagged blocks" });
    expect(rec.events.filter((e) => e.kind === "replayed")).toHaveLength(7);
    expect(fs.needsRecovery).toBe(false);
    expect(vol.sector(1)[0x60]).toBe(0x02);
    expect(names(vol)).toEqual([".", "..", "lost+found", "crash.txt"]);
    expect(vol.dirEntries("/")[3]).toMatchObject({ offset: 69 * 1024 + 0x2c, inode: 12 });
    expect(fs.chain(CRASH_PATH)).toEqual([1111, 1112]);
    expect([inUse(vol, 1111), inUse(vol, 1112)]).toEqual([true, true]);
    expect(vol.readFile(CRASH_PATH)).toEqual(biggerText(CRASH_BYTES));
    const text = stepText("Recover: replay");
    for (const s of ["from block 1", "1 committed transaction with 7 tagged blocks", "names crash.txt at +0x2C, inode 12", "blocks 1111 and 1112", "back to 0x02"]) {
      expect(text).toContain(s);
    }
  });

  it("step 5: a crash before the commit leaves the data on disk and transaction 3 uncommitted", () => {
    const { vol, fs, records } = runThrough("Crash before the commit");
    const rec = records[2];
    expect(rec.op).toBe(`create_file ${LOST_PATH} (crashed before commit)`);
    expect(rec.events.find((e) => e.kind === "blocks_allocated")?.text).toBe(`allocated 1 block starting at ${LOST_BLOCK} in group 0`);
    expect(rec.events.find((e) => e.kind === "transaction_started")?.text).toBe("started transaction 3 at journal block 1 (7 tagged blocks)");
    expect(rec.events.filter((e) => e.kind === "journal_block_written").map((e) => e.text.split(" ")[0])).toEqual(["wrote", ...new Array(7).fill("copied")]);
    expect(LOST_TEXT.length).toBeLessThanOrEqual(1024);
    expect(fs.needsRecovery).toBe(true);
    expect(new TextDecoder().decode(vol.sector(LOST_BLOCK).slice(0, LOST_TEXT.length))).toBe(LOST_TEXT);
    const text = stepText("Crash before the commit");
    for (const s of ["crash before commit", `${LOST_PATH}, one block`, `block ${LOST_BLOCK} holds the text`, "descriptor and 7 copies", "transaction 3", "no commit block"]) {
      expect(text).toContain(s);
    }
  });

  it("step 6: recovery discards transaction 3 and the block stays free", () => {
    const { vol, fs, records } = runThrough("Recover: discard");
    const rec = records[3];
    expect(rec.op).toBe("recover");
    expect(rec.events.map((e) => e.kind)).toEqual(["recovery_scanned", "transaction_discarded", "journal_emptied", "recovery_flag_cleared"]);
    expect(rec.events[0].text).toContain("0 committed transactions");
    expect(rec.events[1].text).toBe("discarded uncommitted transaction 3 (7 tagged blocks)");
    expect(fs.needsRecovery).toBe(false);
    expect(inUse(vol, LOST_BLOCK)).toBe(false);
    expect(vol.sector(3)[0x8b]).toBe(0x00);
    expect(Math.floor((LOST_BLOCK - 1) / 8)).toBe(0x8b);
    expect(fs.describeUnit(LOST_BLOCK)).toBe("free");
    expect(() => vol.stat(LOST_PATH)).toThrow(expect.objectContaining({ code: "NotFound" }));
    expect(names(vol)).toEqual([".", "..", "lost+found", "crash.txt"]);
    const text = stepText("Recover: discard");
    for (const s of ["0 committed transactions", "discards transaction 3", "byte +0x8B of block 3 still reads 00", `block ${LOST_BLOCK} is free`, "names lost.txt"]) {
      expect(text).toContain(s);
    }
  });

  it("step 7: the orphaned bytes are still in the free block", () => {
    const { vol, fs } = runThrough("Bytes without an owner");
    expect(new TextDecoder().decode(vol.sector(LOST_BLOCK).slice(0, LOST_TEXT.length))).toBe(LOST_TEXT);
    expect(fs.owners.some((o) => o.unit === LOST_BLOCK)).toBe(false);
    expect(stepText("Bytes without an owner")).toContain(`Block ${LOST_BLOCK} still holds the text`);
  });

  it("resolves every step's focus against the volume that step leaves", () => {
    const at = (title: string) => focusOf(title, runThrough(title).fs);
    expect(at("Crash after the commit")).toEqual({ path: null, offset: 1024 + 0x60 });
    expect(at("A live transaction")).toEqual({ sector: 83 });
    expect(at("Home blocks still old")).toEqual({ offset: 69 * 1024 });
    expect(at("Recover: replay")).toEqual({ path: CRASH_PATH, offset: 69 * 1024 + 0x2c });
    expect(at("Crash before the commit")).toEqual({ path: null, sector: LOST_BLOCK });
    expect(at("Recover: discard")).toEqual({ offset: 3 * 1024 + 0x8b });
    expect(at("Bytes without an owner")).toEqual({ path: null, sector: LOST_BLOCK });
  });
});
