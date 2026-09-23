// Type-only re-exports of browser-terminal's public command types. `verbatimModuleSyntax`
// erases all of this at runtime, so nothing under src/shell/ loads the terminal package;
// only TerminalPanel.svelte (via storeHost.svelte.ts) ever imports it for real.
import type { CommandFn, CommandSpec } from "@benjamin-small/browser-terminal";

export type {
  CommandArgs,
  CommandCtx,
  CommandFn,
  CommandSpec,
  FlagSpec,
  PosArg,
  RedirectContext,
  RedirectHandler,
  Value,
} from "@benjamin-small/browser-terminal";

/** One registered command. The drawer wires it with `bt.registerCommand(def.spec, def.fn)`. */
export interface CommandDef {
  spec: CommandSpec;
  fn: CommandFn;
}
