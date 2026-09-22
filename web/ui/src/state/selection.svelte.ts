export class SelectionStore {
  path = $state<string | null>(null);
  cursorOffset = $state<number | null>(null);
  hoverOffset = $state<number | null>(null);
  stringsOn = $state(false);
  showRemnants = $state(false);
  expandedGaps = $state(new Set<number>());
  scrollTarget = $state<{ offset: number; nonce: number } | null>(null);

  jumpTo(offset: number) { this.cursorOffset = offset; this.scrollTarget = { offset, nonce: (this.scrollTarget?.nonce ?? 0) + 1 }; }
  select(path: string | null) { this.path = path; }
  expandGap(startSector: number) { const s = new Set(this.expandedGaps); s.add(startSector); this.expandedGaps = s; }
}

export const selection = new SelectionStore();
