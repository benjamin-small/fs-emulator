/**
 * Numbers the dump's scroll requests. HexView scrolls when a request's nonce differs from the
 * last one it acted on, and it remembers that nonce for as long as it is mounted: across a
 * format, a load, and a lesson start. So the numbering never restarts. When the selection's
 * reset began again at 1, the first jump after it carried the nonce of the first jump before
 * it, and the dump stayed where it was (close a lesson at its first step, start another).
 */
export class ScrollNonces {
  private last = 0;

  /** One more than every nonce issued so far. */
  next(): number { return ++this.last; }
}
