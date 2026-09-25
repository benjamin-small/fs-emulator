import type { CommandDef, CommandSpec } from "./types";
import type { FsAdapter } from "../fs/adapter";
import { ddXxdCommands, mutationCommands, readCommands } from "./commands";
import type { ShellHost } from "./host";
import { journalCommands } from "./journalCommands";
import { Vfs } from "./vfs";

/**
 * Every command the terminal registers. `echo` is a browser-terminal builtin
 * and is not registered here. `vfs` holds the one working directory per page.
 * `crash` and `recover` come last, and only while the mounted adapter has a journal (ext3);
 * the terminal calls this again whenever that or the family changes.
 */
export function createCommands(host: ShellHost, vfs: Vfs = new Vfs()): CommandDef[] {
  const journal = host.adapter.journal ? journalCommands(host) : [];
  return [...readCommands(host, vfs), ...mutationCommands(host, vfs), ...ddXxdCommands(host, vfs), ...journal];
}

/**
 * Which command set `createCommands` builds for this adapter: the family, plus `+journal` when
 * it has a journal capability (`fat16`, `ext`, `ext+journal`). The terminal re-registers when
 * this changes, because the summaries and flag descriptions that name the family's nouns, and
 * the presence of `crash` and `recover`, are fixed when a command is registered.
 */
export function commandSetOf(adapter: FsAdapter): string {
  return adapter.journal ? `${adapter.id}+journal` : adapter.id;
}

/** browser-terminal's two registration calls, all `registerCommands` needs of the terminal. */
export interface CommandRegistry {
  registerCommand(spec: CommandSpec, fn: CommandDef["fn"]): void;
  unregisterCommand(name: string): void;
}

/**
 * Unregister the `previous` command names, register `createCommands(host, vfs)` in their place,
 * and return the names now registered for the next call. The same `vfs` keeps the working
 * directory across a re-registration.
 */
export function registerCommands(term: CommandRegistry, host: ShellHost, vfs: Vfs, previous: readonly string[] = []): string[] {
  for (const name of previous) term.unregisterCommand(name);
  const defs = createCommands(host, vfs);
  for (const { spec, fn } of defs) term.registerCommand(spec, fn);
  return defs.map((d) => d.spec.name);
}
