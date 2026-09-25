import { asExt } from "../fs/ext";
import type { FsAdapter } from "../fs/adapter";
import type { Scenario } from "../state/scenarios.svelte";
import { biggerText } from "./fundamentals";

/**
 * One create on the ordered-mode ext3 disk, followed through the journal in the order its bytes
 * were written. The first step runs the create; every later step only moves the dump, and the
 * Step strip's events list the writes in the same order. The dump shows the disk after the
 * create, so steps about a moment mid-transaction say what the bytes read then and what they
 * read now. tests/journaled-write.test.ts pins every quoted number, and the journal positions
 * below, against the create's own record.
 */

export const NOTES_PATH = "/notes.txt";
/** Three blocks: two full and 952 bytes of a third. */
export const NOTES_BYTES = 3000;
/** The superblock's incompatible-features word: block 1 + 0x60. */
export const FLAG_WORD = 1024 + 0x60;
/** The journal superblock's `s_sequence` and `s_start` fields. */
export const S_SEQUENCE = 0x18;
export const S_START = 0x1c;
/** Where the transaction sits in the journal (indexes from the journal's first block): the
 *  descriptor, the seven copies after it, then the commit. */
export const DESCRIPTOR_INDEX = 1;
export const COPY_COUNT = 7;
export const COMMIT_INDEX = DESCRIPTOR_INDEX + COPY_COUNT + 1;

/** The journal's first block (82 on the default disk), from the capability. */
const journalStart = (fs: FsAdapter) => asExt(fs).journal!.state().firstBlock;

export const scenario: Scenario = {
  id: "journaled-write",
  title: "A journaled write",
  summary: "Create one file on ext3 and follow its metadata through the journal: descriptor, copies, commit, checkpoint.",
  family: "ext",
  steps: [
    {
      title: "One create, one transaction",
      text: "This step created /notes.txt, 3,000 bytes, on an ext3 disk in ordered mode. The superblock's incompatible-features word at block 1 + 0x60 reads 0x02 now, but while the create ran it read 0x06: bit 0x04 is needs_recovery, set before the first journal write and cleared after the last, so a crash in between leaves a flag that says so. The Step strip lists the create's events in the order they happened; the next steps follow them.",
      action: (v) => v.createFile(NOTES_PATH, biggerText(NOTES_BYTES)),
      focus: { path: NOTES_PATH, offset: FLAG_WORD },
    },
    {
      title: "Data first",
      text: "In ordered mode the file's data goes straight to its home blocks, 1111 to 1113, before anything touches the journal: 3,000 bytes fill two blocks and 952 bytes of the third. If the machine stopped here the blocks would hold the bytes, but no inode or directory entry would point at them yet.",
      focus: (fs) => ({ path: NOTES_PATH, sector: fs.chain(NOTES_PATH)[0] }),
    },
    {
      title: "The journal superblock",
      text: "Next the driver wrote block 82, the journal's own superblock. Its s_start field, at +0x1C, went from 0 to 1: a live transaction starts at journal block 1. It reads 0 again now because the transaction finished; the transaction_started event is the moment it read 1.",
      focus: (fs) => ({ offset: journalStart(fs) * 1024 + S_START }),
    },
    {
      title: "The descriptor",
      text: "Journal block 1 is block 83, the descriptor: the magic C0 3B 39 98, block type 1, transaction 1, then one tag for each metadata block the create changed, in ascending order: 1, 2, 3, 4, 5, 6, and 69. That is the superblock, the group descriptors, both bitmaps, the two inode-table blocks holding inodes 2 and 12, and the root directory: 7 tags.",
      focus: (fs) => ({ sector: journalStart(fs) + DESCRIPTOR_INDEX }),
    },
    {
      title: "The copies",
      text: "Blocks 84 to 90 are the 7 copies, in tag order: whole-block images of the new metadata, written to the journal while the home blocks still hold the old versions. Block 84 is the superblock's image, and at +0x60 it reads 0x06, because the flag was already set when the copy was taken.",
      focus: (fs) => ({ sector: journalStart(fs) + DESCRIPTOR_INDEX + 1 }),
    },
    {
      title: "The commit",
      text: "Block 91, journal block 9, is the commit block: the magic, block type 2, transaction 1. Until it is on disk the transaction does not count. Recovery throws away a transaction with no commit block and replays one that has it.",
      focus: (fs) => ({ sector: journalStart(fs) + COMMIT_INDEX }),
    },
    {
      title: "Checkpoint",
      text: "Only after the commit does the driver write the 7 blocks to their home locations: the checkpoint. Block 69, the root directory, now names notes.txt at +0x2C, inode 12. A crash from here on loses nothing, because the journal still holds every copy. A kernel would put the checkpoint off and batch many transactions into one; this emulator checkpoints each transaction at once, so every step leaves a cleanly unmounted disk.",
      focus: (fs) => ({ path: NOTES_PATH, offset: asExt(fs).dirEntryOffset(NOTES_PATH) ?? undefined }),
    },
    {
      title: "The journal empties",
      text: "With every block home, the journal superblock is written once more: s_sequence at +0x18 becomes 2, the number the next transaction will use, and s_start at +0x1C goes back to 0, which means there is nothing to replay.",
      focus: (fs) => ({ offset: journalStart(fs) * 1024 + S_SEQUENCE }),
    },
    {
      title: "The flag clears",
      text: "Last, the superblock's needs_recovery bit clears: 0x06 back to 0x02. The Journal panel draws transaction 1 at journal blocks 1 to 9, faded because it is stale: already checkpointed, kept only until the next transaction writes over it.",
      focus: { path: NOTES_PATH, offset: FLAG_WORD },
    },
  ],
};
