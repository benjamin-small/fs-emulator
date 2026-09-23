import { Volume, type FormatOptions, type OpRecord } from "../../src/lib/wasm";
import type { ShellHost } from "../../src/shell/host";
import type { CommandDef } from "../../src/shell/commands";
import type { CommandCtx, Value } from "@benjamin-small/browser-terminal";

/**
 * A ShellHost over a plain Volume. `run` mirrors VolumeStore.run: it snaps
 * back to the latest step, applies the op, appends the record, and leaves the
 * cursor at the end. `rewind` only moves the cursor (the volume itself always
 * holds the latest state, exactly as in the app).
 */
export class TestHost implements ShellHost {
  vol: Volume;
  history: OpRecord[] = [];
  cursor = -1;
  selected: (string | null)[] = [];
  jumps: number[] = [];
  prompts: string[] = [];
  formats: FormatOptions[] = [];
  closed = false;

  constructor(vol: Volume) {
    this.vol = vol;
  }

  get historyLength(): number {
    return this.history.length;
  }

  run(fn: (v: Volume) => OpRecord): OpRecord {
    this.cursor = this.history.length - 1;
    const rec = fn(this.vol); // a wasm FsError ({ message, code }) propagates as-is
    this.history.push(rec);
    this.cursor = this.history.length - 1;
    return rec;
  }

  format(options: FormatOptions): void {
    this.vol = Volume.formatFat16(options);
    this.history = [];
    this.cursor = -1;
    this.formats.push(options);
  }

  select(path: string | null): void {
    this.selected.push(path);
  }

  jumpTo(offset: number): void {
    this.jumps.push(offset);
  }

  setPrompt(prefix: string): void {
    this.prompts.push(prefix);
  }

  closeTerminal(): void {
    this.closed = true;
  }

  /** View step `step` (-1 = before any op) without touching the volume. */
  rewind(step: number): void {
    this.cursor = Math.max(-1, Math.min(step, this.history.length - 1));
  }
}

export function makeHost(vol: Volume = Volume.formatFat16(undefined)): TestHost {
  return new TestHost(vol);
}

export type CallArgs = Value[] | { positionals?: Value[]; flags?: Record<string, Value> };
export interface CallResult { value: unknown; log: string[]; err: string[] }
export interface ShellFailure { message: string; help?: string; code?: string }

/** A ChannelWriter whose cooked calls and raw writes both land in `sink`. */
function writer(sink: string[]): CommandCtx["log"] {
  return Object.assign(
    (line: string) => { sink.push(line); },
    {
      write(s: string) { sink.push(s); },
      flush() {},
      mode() {},
    },
  );
}

function bindArgs(args: CallArgs): { positionals: Value[]; flags: Record<string, Value> } {
  if (Array.isArray(args)) return { positionals: args, flags: {} };
  return { positionals: args.positionals ?? [], flags: args.flags ?? {} };
}

/**
 * Run one registered command the way the engine would: bound args, piped input, a fake ctx.
 * `input` defaults to `[]`, not `null` — browser-terminal's stream collector turns a pipe with
 * nothing written to it into an empty list, and only ever hands a command `null` at the terminal
 * boundary (never mid-pipeline), so `[]` is what a no-pipe command line actually receives.
 */
export async function call(defs: CommandDef[], name: string, args: CallArgs = [], input: Value = []): Promise<CallResult> {
  const def = defs.find((d) => d.spec.name === name);
  if (!def) throw new Error(`no command named "${name}" is registered`);
  const log: string[] = [];
  const err: string[] = [];
  const controller = new AbortController();
  // `session`/`pane` are the ids browser-terminal 0.3.0 puts on every ctx. No command reads
  // them yet (the working directory is one per page), so any stable pair will do.
  const ctx: CommandCtx = { session: 1, pane: 1, signal: controller.signal, log: writer(log), err: writer(err), emit: writer(log) };
  const value = await def.fn(bindArgs(args), input, ctx);
  return { value, log, err };
}

/** Like `call`, but the command must throw; returns what the engine would render. */
export async function callErr(defs: CommandDef[], name: string, args: CallArgs = [], input: Value = []): Promise<ShellFailure> {
  try {
    await call(defs, name, args, input);
  } catch (e) {
    const f = e as Partial<ShellFailure>;
    return { message: String(f.message ?? e), help: f.help, code: f.code };
  }
  throw new Error(`${name} did not throw`);
}
