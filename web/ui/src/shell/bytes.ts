import type { Value } from "./types";
import { ShellError } from "./errors";

/**
 * The pipe convention. browser-terminal's `Value` has no bytes type, so a string is UTF-8
 * text and a `BytesBlob` record carries raw bytes losslessly as lowercase hex. A blob that
 * reaches the terminal renders as a key/value record; users pipe it into `xxd` or `write`.
 */
// A `type`, not an `interface`: only object type literals get the implicit index signature
// that makes a blob assignable to browser-terminal's `Value` record type.
export type BytesBlob = { bytes: string; length: number };

export const BYTES_HELP = "pipe text (echo hi), a blob from `cat --bytes` or `dd`, or a list of words";

const HEX_RE = /^([0-9a-f]{2})*$/;

export function isBlob(v: unknown): v is BytesBlob {
  if (typeof v !== "object" || v === null || Array.isArray(v)) return false;
  const { bytes, length } = v as { bytes?: unknown; length?: unknown };
  return typeof bytes === "string" && typeof length === "number" && HEX_RE.test(bytes) && length === bytes.length / 2;
}

export function hex(b: Uint8Array): string {
  let s = "";
  for (let i = 0; i < b.length; i++) s += b[i].toString(16).padStart(2, "0");
  return s;
}

export function unhex(s: string): Uint8Array {
  const out = new Uint8Array(s.length / 2);
  for (let i = 0; i < out.length; i++) out[i] = parseInt(s.slice(2 * i, 2 * i + 2), 16);
  return out;
}

export function fromBytes(b: Uint8Array): BytesBlob {
  return { bytes: hex(b), length: b.length };
}

/**
 * True when `v` carries real piped input. browser-terminal's stream collector turns a
 * pipe with nothing written to it into `Value::List([])`, not `null` — only the
 * terminal-boundary path (not a mid-pipeline command) ever hands a command `null`
 * (`crates/bterm-core/src/stream.rs`, `crates/bterm-wasm/src/js_command.rs`). So a
 * command deciding "was anything piped in?" must treat both `null` and `[]` as "no
 * input", not just `null`.
 */
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
 * string → UTF-8; number or boolean → `String(v)`; list of scalars → items joined by one
 * space (what `echo a b` yields); blob → its bytes; null → empty; anything else throws.
 */
export function toBytes(v: Value): Uint8Array {
  if (v === null) return new Uint8Array(0);
  if (typeof v === "string") return new TextEncoder().encode(v);
  if (typeof v === "number" || typeof v === "boolean") return new TextEncoder().encode(String(v));
  if (isBlob(v)) return unhex(v.bytes);
  if (Array.isArray(v) && v.every(isScalar)) return new TextEncoder().encode(v.map(scalarText).join(" "));
  throw new ShellError(`expected text or bytes, found ${describe(v)}`, { help: BYTES_HELP });
}
