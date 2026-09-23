import type { Scenario } from "../state/scenarios.svelte";

const PATH = "/NOTE.TXT";
const SENTENCE = "Meet at noon on Friday.";

export const scenario: Scenario = {
  id: "delete-remnants",
  title: "Delete and see what remains",
  summary: "Delete a file and find out how little actually disappears from the disk.",
  steps: [
    {
      title: "A file with a memorable sentence",
      text: "NOTE.TXT holds one sentence, written into its single data cluster.",
      action: (v) => v.createFile(PATH, new TextEncoder().encode(SENTENCE)),
      focus: { path: PATH },
    },
    {
      title: "Deleting only marks the entry",
      text: "Deleting the file only rewrites the first byte of its directory entry to 0xE5. The rest of the name is still sitting right there, readable.",
      action: (v) => v.deleteFile(PATH),
      focus: (v) => ({ offset: v.geometry().firstRootDirSector * v.geometry().bytesPerSector, showRemnants: true }),
    },
    {
      title: "The data never moved",
      text: "Cluster 2's bytes are untouched, and its FAT entry now reads free. That gap between 'marked deleted' and 'actually erased' is exactly what undelete tools rely on.",
      focus: { unit: 2, showRemnants: true },
    },
    {
      title: "A new file overwrites the remnant",
      text: "Creating another file reuses the first free directory slot and the first free cluster, so it lands right on top of NOTE.TXT's remnant and erases it for good.",
      action: (v) => v.createFile("/AGAIN.TXT", new TextEncoder().encode("A fresh file.")),
      focus: { path: "/AGAIN.TXT", showRemnants: true },
    },
  ],
};
