import { findEntrySlots } from "../fs/fat16/direntry";
import type { Scenario } from "../state/scenarios.svelte";

const PATH = "/Quarterly report (draft).txt";

export const scenario: Scenario = {
  id: "long-name",
  title: "A long file name",
  summary: "See how a name that does not fit 8.3 gets spread across several directory entries.",
  steps: [
    {
      title: "A name too long for one slot",
      text: "This file's name is too long for a plain 8.3 directory entry, so the driver needs extra entries just to store it.",
      action: (v) => v.createFile(PATH, new TextEncoder().encode("Numbers for the quarter.")),
      focus: { path: PATH },
    },
    {
      title: "Long-name entries come first",
      text: "The long-name entries are written backwards, last fragment first, each holding up to 13 characters. They are followed by a short entry, QUARTE~1.TXT, which is what an old DOS driver would see.",
      focus: (v) => {
        const slots = findEntrySlots(v, v.geometry(), v.fatEntries(0), v.clusterOwners(), PATH);
        return { offset: slots?.start };
      },
    },
    {
      title: "The checksum that ties them together",
      text: "Each long-name entry carries a checksum of the short name. The inspector's annotation for this slot shows that byte, which is how a reader confirms the fragments and the short entry belong together.",
      focus: (v) => {
        const slots = findEntrySlots(v, v.geometry(), v.fatEntries(0), v.clusterOwners(), PATH);
        return { offset: slots?.start };
      },
    },
  ],
};
