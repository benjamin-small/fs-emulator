/** The indefinite article for `word`, by its first letter: "an ext3", "a FAT16". */
export function articleFor(word: string): "a" | "an" {
  return /^[aeiou]/i.test(word) ? "an" : "a";
}

/** The status line's notice after Load image opened `fileName` in another family's tab:
 *  where it went, what it is, and that the tab it was loaded from is untouched. */
export function loadNotice(fileName: string, targetName: string, fsType: string, fromName: string): string {
  return `Opened ${fileName} in the ${targetName} tab: it is ${articleFor(fsType)} ${fsType} image. The ${fromName} tab is as you left it.`;
}

/** What Load image does besides mounting, for an image of `targetId`'s family loaded from the
 *  `fromId` tab: a routed load (another tab) closes a lesson running in the target tab, whose
 *  disk is about to go, and leaves a notice there; a same-tab load does neither. */
export function loadPlan(fromId: string, targetId: string, lessonRunning: boolean): { stopLesson: boolean; notice: boolean } {
  const routed = fromId !== targetId;
  return { stopLesson: routed && lessonRunning, notice: routed };
}
