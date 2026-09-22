import { findEntrySlots } from "../core/direntry";
import type { Scenario } from "../state/scenarios.svelte";

export const scenario: Scenario = {
  id: "small-file",
  title: "Add a small file",
  summary: "Create one small file and see exactly which bytes on disk moved.",
  steps: [
    {
      title: "Three things changed",
      text: "Creating HELLO.TXT touches three places: a directory entry that names it, a FAT entry that allocates its cluster, and the data cluster that holds its bytes.",
      action: (v) => v.createFile("/HELLO.TXT", new TextEncoder().encode("Hello, FAT16!")),
      focus: { path: "/HELLO.TXT" },
    },
    {
      title: "The directory entry",
      text: "This 32-byte slot in the root directory stores the file's name, its size, and the cluster its data starts at.",
      focus: (v) => {
        const slots = findEntrySlots(v, v.geometry(), v.fatEntries(0), v.clusterOwners(), "/HELLO.TXT");
        return { offset: slots?.start };
      },
    },
    {
      title: "The FAT entry",
      text: "Back in the allocation table, the entry for cluster 2 now reads end-of-chain: the file uses exactly one cluster and nothing follows it.",
      focus: { sector: 1 },
    },
    {
      title: "The data cluster",
      text: "Cluster 2 holds the file's actual bytes. Only the first 13 bytes were written; the rest of the cluster is still zero.",
      focus: { cluster: 2 },
    },
  ],
};
