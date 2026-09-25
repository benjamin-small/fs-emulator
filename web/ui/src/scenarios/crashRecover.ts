import { asExt } from "../fs/ext";
import type { Scenario } from "../state/scenarios.svelte";
import { biggerText } from "./fundamentals";
import { DESCRIPTOR_INDEX, FLAG_WORD } from "./journaledWrite";

/**
 * Two crashes on the ordered-mode ext3 disk: one after the commit, which recovery replays, and
 * one before it, which recovery throws away. The actions arm the crash through the journal
 * capability and then create a file; the crash fires inside that create, so the step's record
 * holds exactly the writes that reached the disk. tests/crash-recover.test.ts pins every quoted
 * number and the volume's state after each action.
 */

export const CRASH_PATH = "/crash.txt";
/** Two blocks. */
export const CRASH_BYTES = 2048;
export const LOST_PATH = "/lost.txt";
/** One block's worth of text, written by ordered mode before the crash. */
export const LOST_TEXT = "Written before the crash, claimed by nothing.";
/** The block the lost file's data landed in: the next free block after crash.txt's two. */
export const LOST_BLOCK = 1113;

export const scenario: Scenario = {
  id: "crash-recover",
  title: "Crash and recover",
  summary: "Stop an ext3 create mid-transaction twice, and watch recovery replay one and discard the other.",
  family: "ext",
  steps: [
    {
      title: "Crash after the commit",
      text: "This step armed a crash after commit and created /crash.txt, two blocks. The transaction reached the journal, commit block and all, but the machine stopped before a single metadata block went home. The superblock's flag word at block 1 + 0x60 reads 0x06: needs_recovery is set, and the status line says the volume needs recovery. The Files panel does not list crash.txt.",
      action: (v, fs) => {
        asExt(fs).journal!.arm("after_commit");
        return v.createFile(CRASH_PATH, biggerText(CRASH_BYTES));
      },
      focus: { path: null, offset: FLAG_WORD },
    },
    {
      title: "A live transaction",
      text: "Journal block 1, block 83, is transaction 1's descriptor, tagging the same 7 blocks any create changes; its copies are blocks 84 to 90 and its commit is block 91. The Journal panel draws them at full colour: live, committed but not yet checkpointed.",
      focus: (fs) => ({ sector: asExt(fs).journal!.state().firstBlock + DESCRIPTOR_INDEX }),
    },
    {
      title: "Home blocks still old",
      text: "Block 69, the root directory, still ends with lost+found, whose rec_len of 1000 runs to the end of the block: there is no entry for crash.txt. Its data is in blocks 1111 and 1112 (ordered mode wrote it first), but the block bitmap still calls both free.",
      focus: (fs) => ({ offset: fs.dataStart("/") ?? undefined }),
    },
    {
      title: "Recover: replay",
      text: "Recovery scans the journal from block 1, finds 1 committed transaction with 7 tagged blocks, and copies each one home. Block 69 now names crash.txt at +0x2C, inode 12; the bitmap has blocks 1111 and 1112, the flag is back to 0x02, and the Files panel lists the file.",
      action: (_v, fs) => asExt(fs).journal!.recover(),
      focus: (fs) => ({ path: CRASH_PATH, offset: asExt(fs).dirEntryOffset(CRASH_PATH) ?? undefined }),
    },
    {
      title: "Crash before the commit",
      text: "Now a crash before commit, on a create of /lost.txt, one block. Ordered mode writes the data first, so block 1113 holds the text; then the descriptor and 7 copies went to the journal as transaction 3, but no commit block followed. The volume needs recovery again.",
      action: (v, fs) => {
        asExt(fs).journal!.arm("before_commit");
        return v.createFile(LOST_PATH, new TextEncoder().encode(LOST_TEXT));
      },
      focus: { path: null, sector: LOST_BLOCK },
    },
    {
      title: "Recover: discard",
      text: "Recovery finds 0 committed transactions and discards transaction 3: none of its copies go home. In the block bitmap, byte +0x8B of block 3 still reads 00, so block 1113 is free, and no inode or directory entry names lost.txt.",
      action: (_v, fs) => asExt(fs).journal!.recover(),
      focus: (fs) => {
        const g = asExt(fs).geo;
        return { offset: g.groups[0].blockBitmap * g.blockSize + Math.floor((LOST_BLOCK - g.firstDataBlock) / 8) };
      },
    },
    {
      title: "Bytes without an owner",
      text: "Block 1113 still holds the text: the data was written, and the metadata that would have claimed it never was. The next file to allocate this block overwrites it. That is ordered mode's promise: after a crash, metadata never points at garbage, but a block can hold data nobody points at.",
      focus: { path: null, sector: LOST_BLOCK },
    },
  ],
};
