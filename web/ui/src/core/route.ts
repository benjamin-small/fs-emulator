/**
 * The URL hash names a tab: `#fat16`, `#ext`. `parseRoute` reads a hash forgivingly (the `#`
 * is optional, one leading and one trailing `/` are dropped, case is ignored, and `aliases`
 * map other spellings such as `#ext3` onto an id); anything else is null, and the caller
 * falls back to its default. `formatRoute` writes the canonical form back.
 */
export function parseRoute<T extends string>(hash: string, known: readonly T[], aliases: Readonly<Record<string, T>> = {}): T | null {
  let name = hash.startsWith("#") ? hash.slice(1) : hash;
  if (name.startsWith("/")) name = name.slice(1);
  if (name.endsWith("/")) name = name.slice(0, -1);
  name = name.toLowerCase();
  // `Object.hasOwn`, not `in`: `#constructor` must not find Object.prototype's.
  if (Object.hasOwn(aliases, name)) return aliases[name];
  return (known as readonly string[]).includes(name) ? (name as T) : null;
}

export function formatRoute(id: string): string {
  return `#${id}`;
}
