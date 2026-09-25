import type { Value } from "./types";
import { ShellError } from "./errors";

/**
 * The pipe convention. browser-terminal 0.3.0 carries `Uint8Array` as a first-class
 * `Value`, so a string is UTF-8 text and raw bytes are simply bytes: they survive pipes,
 * variables and `run().value` untouched, `length` counts them, and the terminal renders
 * them as `<N bytes>` instead of interpreting them as text.
 */
export const BYTES_HELP = "pipe text (echo hi), bytes from `cat --bytes` or `dd`, or a list of words";

/**
 * True when `v` carries real piped input. browser-terminal's stream collector turns a
 * pipe with nothing written to it into `Value::List([])`, not `null` — only the
 * terminal-boundary path (not a mid-pipeline command) ever hands a command `null`
 * (`crates/bterm-core/src/stream.rs`, `crates/bterm-wasm/src/js_command.rs`). So a
 * command deciding "was anything piped in?" must treat both `null` and `[]` as "no
 * input", not just `null`.
 *
 * An empty `Uint8Array` is input: the pipe was written to, with a value that happens to
 * be zero bytes long (`cat --bytes /dev/null`), so `> f` makes an empty file rather than
 * failing with "nothing to write".
 */
/** True when a flag was given at all: browser-terminal hands an absent flag as `undefined`
 *  (or `null`), and `0` and `""` are values. */
export function flagGiven(v: unknown): boolean {
  return v !== undefined && v !== null;
}

export function hasInput(v: Value | undefined): boolean {
  return !(v === undefined || v === null || (Array.isArray(v) && v.length === 0));
}

export function decodeText(b: Uint8Array): string {
  return new TextDecoder("utf-8", { fatal: false }).decode(b);
}

type Scalar = string | number | boolean | null;
const isScalar = (v: Value): v is Scalar => v === null || typeof v !== "object";
const scalarText = (v: Scalar): string => (v === null ? "" : String(v));

function describe(v: Value): string {
  if (v === null) return "null";
  if (Array.isArray(v)) return "a list with nested values";
  if (typeof v === "object") return "a record";
  return typeof v;
}

/**
 * bytes → themselves; string → UTF-8; number or boolean → `String(v)`; list of scalars →
 * items joined by one space (what `echo a b` yields); null → empty; anything else throws.
 *
 * A `Uint8Array` is returned as it arrived, view and all: the engine already copies a byte
 * buffer when it crosses the host boundary, and no caller here mutates what it is handed —
 * they slice it, concatenate it into a fresh array, or pass it to wasm, which copies again.
 */
export function toBytes(v: Value): Uint8Array {
  if (v === null) return new Uint8Array(0);
  if (v instanceof Uint8Array) return v;
  if (typeof v === "string") return new TextEncoder().encode(v);
  if (typeof v === "number" || typeof v === "boolean") return new TextEncoder().encode(String(v));
  if (Array.isArray(v) && v.every(isScalar)) return new TextEncoder().encode(v.map(scalarText).join(" "));
  throw new ShellError(`expected text or bytes, found ${describe(v)}`, { help: BYTES_HELP });
}
