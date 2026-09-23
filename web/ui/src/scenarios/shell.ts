import type { Scenario } from "../state/scenarios.svelte";

const FILE = "/HELLO.TXT";
const DIR = "/DOCS";
const COPY = "/DOCS/COPY.TXT";
const GREETING = "Hello from the shell";

// The volume label lives at boot-sector offset 43, 11 bytes, space padded.
const LABEL_OFFSET = 43;
const LABEL = "SHELLDISK  ";

const enc = (s: string) => new TextEncoder().encode(s);

export const scenario: Scenario = {
  id: "shell",
  title: "Work from the shell",
  summary: "Do the same operations from the terminal drawer: /mnt is the volume, /dev/hda the raw disk.",
  steps: [
    {
      title: "Open the terminal",
      text: "Press ` (backtick) or click Terminal to open the drawer. Type `ls /mnt`: a fresh disk has an empty root. `pwd` prints /mnt, and `mount` and `df` describe the volume.",
      focus: { path: null },
    },
    {
      title: "Write a file from a pipe",
      text: "Type `echo 'Hello from the shell' | write /mnt/HELLO.TXT`. Redirection does the same thing: `echo 'Hello from the shell' > /mnt/HELLO.TXT` is the same journaled write. The same three places change as with Add file: a directory entry, a FAT entry, and a data cluster.",
      action: (v) => v.createFile(FILE, enc(GREETING)),
      focus: { path: FILE },
    },
    {
      title: "Read it back",
      text: "`cat /mnt/HELLO.TXT` prints the text. `stat /mnt/HELLO.TXT` shows the entry offset, first cluster, chain, and data offset, and `seek c:2` moves the dump to that cluster.",
      focus: (v) => ({ cluster: v.clusterOwners().find((o) => o.path === FILE)?.firstCluster }),
    },
    {
      title: "Make a directory",
      text: "`mkdir /mnt/DOCS` allocates a cluster for the directory and writes its `.` and `..` entries there. `cd /mnt/DOCS` then `ls` shows an empty listing (the dot entries are hidden, as on a real shell); `xxd /dev/hda --offset c:3 --len 64` shows the two entries on disk.",
      action: (v) => v.createDir(DIR),
      focus: { path: DIR },
    },
    {
      title: "Copy a file",
      text: "`cp /mnt/HELLO.TXT /mnt/DOCS/COPY.TXT` reads the bytes and creates the copy inside DOCS. A new cluster is allocated for it, right after the directory's own.",
      action: (v) => v.createFile(COPY, v.readFile(FILE)),
      focus: { path: COPY },
    },
    {
      title: "Read the raw boot sector",
      text: "`dd if=/dev/hda bs=512 count=1 | xxd` dumps sector 0 straight from the disk, the way a real tool would. The flag spelling, `dd --if=/dev/hda --bs=512 --count=1`, does the same.",
      focus: { sector: 0 },
    },
    {
      title: "Patch the disk directly",
      text: "`echo 'SHELLDISK  ' | dd --of=/dev/hda --bs=1 --seek=43` overwrites the 11-byte volume label at boot offset 43. The raw write is journaled like any other step, so it rewinds, and the inspector's boot annotation shows the new label.",
      action: (v) => v.writeRaw(LABEL_OFFSET, enc(LABEL)),
      focus: { offset: LABEL_OFFSET },
    },
    {
      title: "Delete and look at what remains",
      text: "`rm /mnt/HELLO.TXT` marks the entry deleted and frees its chain; the bytes stay on disk. `ls -l /mnt` no longer lists it, and the copy in DOCS is untouched.",
      action: (v) => v.deleteFile(FILE),
      focus: (v) => ({ offset: v.geometry().firstRootDirSector * v.geometry().bytesPerSector, path: null, showRemnants: true }),
    },
  ],
};
