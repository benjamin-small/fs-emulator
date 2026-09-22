import type { Scenario } from "../state/scenarios.svelte";

export const scenario: Scenario = {
  id: "fill-disk",
  title: "Fill the disk",
  summary: "Run a tiny volume all the way to full and watch the next write fail cleanly.",
  steps: [
    {
      title: "A deliberately tiny disk",
      text: "This volume is formatted much smaller than usual, with one sector per cluster, so it only takes one file to run it out of room.",
      format: { totalSectors: 8192, sectorsPerCluster: 1, enforceFat16Range: true },
      focus: { sector: 0 },
    },
    {
      title: "Nearly every cluster claimed",
      text: "BIG.BIN is sized to use every free cluster but one, leaving exactly one cluster of room on the whole disk.",
      action: (v) => v.createFile("/BIG.BIN", new Uint8Array((v.geometry().clusterCount - 1) * v.geometry().bytesPerSector)),
      focus: { path: "/BIG.BIN" },
    },
    {
      title: "Expect: disk full",
      text: "Creating a file that needs two clusters fails with the DiskFull error, since only one cluster is left. The operation is rolled back, so the timeline gains no new step and the disk is unchanged.",
      action: (v) => v.createFile("/TOOBIG.BIN", new Uint8Array(v.geometry().bytesPerSector + 1)),
    },
  ],
};
