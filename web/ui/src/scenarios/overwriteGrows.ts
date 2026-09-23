import type { Scenario } from "../state/scenarios.svelte";

const GROWING = "/GROWING.TXT";
const OTHER = "/OTHER.TXT";

export const scenario: Scenario = {
  id: "overwrite-grows",
  title: "Overwrite with a bigger file",
  summary: "Grow a file past its first cluster and watch its chain fragment around a neighbour.",
  family: "fat16",
  steps: [
    {
      title: "One cluster to start",
      text: "GROWING.TXT starts small enough to fit in a single cluster, so its chain is just one entry long.",
      action: (v, fs) => v.createFile(GROWING, new Uint8Array(fs.unitSize).fill(0x41)),
      focus: { path: GROWING },
    },
    {
      title: "Three clusters now",
      text: "Overwriting it with more data grows the chain to three clusters. The FAT map now draws that chain as a short polyline instead of a single dot.",
      action: (v, fs) => v.writeFile(GROWING, new Uint8Array(3 * fs.unitSize).fill(0x42)),
      focus: { path: GROWING },
    },
    {
      title: "Following the chain in the FAT",
      text: "Each cluster's FAT entry points to the next one in the chain, ending in an end-of-chain marker at the last cluster.",
      focus: { sector: 1 },
    },
    {
      title: "A second file arrives",
      text: "OTHER.TXT is created next and claims the first free cluster right after GROWING.TXT's chain.",
      action: (v) => v.createFile(OTHER, new TextEncoder().encode("A neighbour.")),
      focus: { path: OTHER },
    },
    {
      title: "Growing around a neighbour",
      text: "Overwriting GROWING.TXT again with five clusters' worth of data can't use the cluster OTHER.TXT now owns, so the chain skips over it. That skip is fragmentation.",
      action: (v, fs) => v.writeFile(GROWING, new Uint8Array(5 * fs.unitSize).fill(0x43)),
      focus: { path: GROWING },
    },
  ],
};
