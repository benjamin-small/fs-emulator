import type { CommandDef, CommandSpec } from "./types";
import { FAMILIES } from "../fs";
import type { FsAdapter, FsFamilyId } from "../fs/adapter";
import { ddXxdCommands, mutationCommands, readCommands } from "./commands";
import type { ShellHost } from "./host";
import { journalCommands } from "./journalCommands";
import { Vfs } from "./vfs";

/**
 * Every command the terminal registers. `echo` is a browser-terminal builtin
 * and is not registered here. `vfs` holds the working directory of the tab `host` belongs to.
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
 * directory across a re-registration; each tab passes its own, so a switch keeps each tab's.
 */
export function registerCommands(term: CommandRegistry, host: ShellHost, vfs: Vfs, previous: readonly string[] = []): string[] {
  for (const name of previous) term.unregisterCommand(name);
  const defs = createCommands(host, vfs);
  for (const { spec, fn } of defs) term.registerCommand(spec, fn);
  return defs.map((d) => d.spec.name);
}

/** What the page's one terminal has registered: whose tab (`ws`, compared by identity) and
 *  which command set (`commandSetOf`). */
export interface Registration<W> {
  ws: W;
  set: string;
}

/**
 * What the terminal does to follow the active tab from `prev` (`null` before its first
 * registration) to `next`: `register` the tab's commands again when the tab or its command set
 * changed, and print the tab's `banner` when the tab changed, never on the first registration.
 * Anything else (a format of the same type, a load) only re-sets the prompt.
 */
export function registrationChange<W>(prev: Registration<W> | null, next: Registration<W>): { register: boolean; banner: boolean } {
  if (prev === null) return { register: true, banner: false };
  const tabChanged = prev.ws !== next.ws;
  return { register: tabChanged || prev.set !== next.set, banner: tabChanged };
}

/** The line the terminal prints when it follows a switch: `-- ext tab: /dev/hda is ext3 --`. The
 *  scrollback is shared by every tab, so this marks where one tab's output ends. */
export function tabBanner(id: FsFamilyId, fsType: string): string {
  return `-- ${FAMILIES[id].name} tab: /dev/hda is ${fsType} --`;
}
