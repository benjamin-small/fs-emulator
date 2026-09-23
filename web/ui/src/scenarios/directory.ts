import { findEntrySlots } from "../fs/fat16/direntry";
import type { Scenario } from "../state/scenarios.svelte";

const DIR = "/DOCS";
const FILE = "/DOCS/NOTE.TXT";

export const scenario: Scenario = {
  id: "directory",
  title: "Directories are files too",
  summary: "Create a directory, put a file inside it, then try to remove it before and after emptying it.",
  steps: [
    {
      title: "A directory is a cluster",
      text: "Creating DOCS allocates a cluster like any file would. That cluster is pre-filled with two entries: a dot entry pointing at itself, and a dot-dot entry pointing at its parent.",
      action: (v) => v.createDir(DIR),
      focus: { path: DIR },
    },
    {
      title: "The dot entries",
      text: "Jump into the directory's own cluster and the two dot entries are the first thing you find, before any file it holds.",
      focus: (v) => {
        const owner = v.clusterOwners().find((o) => o.path === DIR);
        return { unit: owner?.firstCluster };
      },
    },
    {
      title: "A file inside the directory",
      text: "NOTE.TXT's directory entry is written inside DOCS's own cluster, right after the dot entries, not in the root directory.",
      action: (v) => v.createFile(FILE, new TextEncoder().encode("Draft notes.")),
      focus: (v) => {
        const slots = findEntrySlots(v, v.geometry(), v.fatEntries(0), v.clusterOwners(), FILE);
        return { offset: slots?.start };
      },
    },
    {
      title: "Expect: not empty",
      text: "Removing DOCS while NOTE.TXT still lives inside it fails with DirectoryNotEmpty. FAT16 refuses to free a directory's cluster while it still holds entries.",
      action: (v) => v.removeDir(DIR),
    },
    {
      title: "Empty it first",
      text: "Deleting the one file inside leaves DOCS holding nothing but its dot entries again.",
      action: (v) => v.deleteFile(FILE),
      focus: { path: DIR },
    },
    {
      title: "Now it can go",
      text: "With no files left inside, removeDir succeeds: the directory's cluster is freed and its own entry in the root directory is marked deleted.",
      action: (v) => v.removeDir(DIR),
      focus: { path: null },
    },
  ],
};
