import type { Fat16FormatOptions } from "../fs/fat16";
import type { Scenario } from "../state/scenarios.svelte";

// Typed with the family's options so the `format` step is checked against FormatOptions;
// `Scenario<Fat16FormatOptions>` still fits `all: Scenario[]` because `O` only appears in
// `Step.format`.
export const scenario: Scenario<Fat16FormatOptions> = {
  id: "fill-disk",
  title: "Fill the disk",
  summary: "Run a tiny volume all the way to full and watch the next write fail cleanly.",
  family: "fat16",
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
      action: (v, fs) => v.createFile("/BIG.BIN", new Uint8Array((fs.unitCount - 1) * fs.unitSize)),
      focus: { path: "/BIG.BIN" },
    },
    {
      title: "Expect: disk full",
      text: "Creating a file that needs two clusters fails with the DiskFull error, since only one cluster is left. The operation is rolled back, so the timeline gains no new step and the disk is unchanged.",
      action: (v, fs) => v.createFile("/TOOBIG.BIN", new Uint8Array(fs.unitSize + 1)),
    },
  ],
};
