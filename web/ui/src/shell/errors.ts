/**
 * Errors thrown by shell commands. browser-terminal reads `message` and `help` off a thrown
 * object (js_command.rs `js_error_to_shell`) and prefixes the command name itself, so no
 * message here ever starts with a command name.
 */
export class ShellError extends Error {
  // `declare`: with ES2022 class fields a plain `help?: string;` would define an own
  // `undefined` property; these stay absent until set, so `"help" in e` mirrors what was given.
  declare help?: string;
  declare code?: string;

  constructor(message: string, opts: { help?: string; code?: string } = {}) {
    super(message);
    this.name = "ShellError";
    if (opts.help !== undefined) this.help = opts.help;
    if (opts.code !== undefined) this.code = opts.code;
  }
}

/**
 * The way back from an unparsable boot sector. `wrapFs` attaches it to every `CorruptImage`
 * error, whichever call produced it, and `assertMounted` uses the same text for the gate it
 * raises itself.
 */
export const CORRUPT_HELP =
  "the boot sector no longer parses; rewind on the timeline, or write the saved sector back with: <the saved bytes> | dd --of=/dev/hda";

/** Coreutils phrasing for the wasm error codes a shell user meets most; other codes keep the wasm text. */
const PHRASES: Record<string, string> = {
  NotFound: "No such file or directory",
  AlreadyExists: "File exists",
  IsADirectory: "Is a directory",
  NotADirectory: "Not a directory",
  DirectoryNotEmpty: "Directory not empty",
  DiskFull: "No space left on device",
  OutOfBounds: "Range runs past the end of the disk",
};

export function fsPhrase(code: string | undefined, raw: string): string {
  // `Object.hasOwn`, not `PHRASES[code]`: an inherited key like `constructor` or `toString`
  // would otherwise hit and phrase a wasm error as a function.
  return code !== undefined && Object.hasOwn(PHRASES, code) ? PHRASES[code] : raw;
}

/** `${display}: ${phrase}` for anything a Volume call threw. A ShellError is already phrased and passes through. */
export function wrapFs(display: string, e: unknown): ShellError {
  if (e instanceof ShellError) return e;
  const err = typeof e === "object" && e !== null ? (e as { message?: unknown; code?: unknown }) : null;
  const raw = typeof err?.message === "string" ? err.message : String(e);
  const code = typeof err?.code === "string" ? err.code : undefined;
  // Every CorruptImage error carries the recovery hint, wherever in the core it came from.
  return new ShellError(`${display}: ${fsPhrase(code, raw)}`, { code, help: code === "CorruptImage" ? CORRUPT_HELP : undefined });
}

/** Run `fn`; a thrown `ShellError` passes through unchanged, anything else becomes `wrapFs(display, e)`. */
export function fsCall<T>(display: string, fn: () => T): T {
  try {
    return fn();
  } catch (e) {
    throw wrapFs(display, e);
  }
}
