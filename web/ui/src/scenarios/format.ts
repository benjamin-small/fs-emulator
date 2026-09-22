import type { Scenario } from "../state/scenarios.svelte";

export const scenario: Scenario = {
  id: "format",
  title: "Format an empty disk",
  summary: "Walk the regions of a freshly formatted volume before any file exists.",
  steps: [
    {
      title: "The boot sector",
      text: "Sector 0 holds the BIOS parameter block: sector size, cluster size, and where every other region starts. The inspector decodes each field when you point at this sector.",
      focus: { sector: 0 },
    },
    {
      title: "Two copies of the FAT",
      text: "The file allocation table starts at sector 1. Entries 0 and 1 are reserved housekeeping slots; every other entry is free until a file claims it.",
      focus: { sector: 1 },
    },
    {
      title: "The root directory",
      text: "After both FAT copies comes the root directory: 512 fixed 32-byte slots, laid out before any data cluster exists.",
      focus: (v) => ({ sector: v.geometry().firstRootDirSector }),
    },
    {
      title: "Free space collapses",
      text: "The data region starts here and it is all free. The dump folds thousands of identical empty sectors into a single collapsed row so you can skip past them.",
      focus: (v) => ({ sector: v.geometry().firstDataSector }),
    },
  ],
};
