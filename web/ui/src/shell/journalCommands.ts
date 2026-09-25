import type { CommandDef } from "./types";
import { CRASH_PHASES, CRASH_PHASE_LABELS, type CrashPhase, type JournalCapability } from "../fs/adapter";
import { ShellError, fsCall } from "./errors";
import { warnIfRewound, type ShellHost } from "./host";

/** The shell's name for a crash phase: wasm's snake_case with hyphens (`after-commit`). */
const phaseName = (p: CrashPhase): string => p.replaceAll("_", "-");

const PHASE_HELP = `phases: ${CRASH_PHASES.map(phaseName).join(", ")}`;

/**
 * The mounted volume's journal, read on every call: the definitions outlive a format, and the
 * terminal re-registers only when the family or the journal's presence changes, so a stale
 * definition says what is missing instead of failing on `undefined`.
 */
function journalOf(host: ShellHost): JournalCapability {
  const journal = host.adapter.journal;
  if (!journal) {
    throw new ShellError(`/dev/hda has no journal (${host.vol.fsType()})`, { help: "crash and recover need an ext3 volume: mkfs --type ext3" });
  }
  return journal;
}

/**
 * `crash` and `recover`, the terminal's half of the Journal panel. `createCommands` includes
 * them only when the mounted adapter has a journal capability (ext3). Arming is not a recorded
 * operation, so `crash` works while the timeline is rewound (with the read commands' warning);
 * it arms through `host.setArmedPhase` (in the app, the store, so the Journal panel shows the phase
 * at once), inside `fsCall` like every wasm call. `recover` is the volume op and goes through
 * `host.run` like every other change.
 */
export function journalCommands(host: ShellHost): CommandDef[] {
  const crash: CommandDef = {
    spec: {
      name: "crash",
      summary: "Arm a crash in the journal: the next change to /dev/hda stops mid-transaction",
      flags: [
        {
          long: "at",
          shape: "str",
          desc: `where the change stops: ${CRASH_PHASES.map((p) => (p === "after_commit" ? `${phaseName(p)} (default)` : phaseName(p))).join(", ")}`,
        },
        { long: "off", desc: "disarm the armed crash" },
      ],
    },
    fn: (args, _input, ctx) => {
      journalOf(host); // a definition that outlived its journal says so
      const at = args.flags.at;
      const given = at !== undefined && at !== null;
      if (args.flags.off === true) {
        if (given) throw new ShellError("give --at or --off, not both");
        warnIfRewound(host, ctx);
        fsCall("/dev/hda", () => host.setArmedPhase(null));
        ctx.log("disarmed");
        return;
      }
      const phase = given ? CRASH_PHASES.find((p) => phaseName(p) === String(at)) : "after_commit";
      if (phase === undefined) throw new ShellError(`unknown crash phase '${String(at)}'`, { help: PHASE_HELP });
      warnIfRewound(host, ctx);
      fsCall("/dev/hda", () => host.setArmedPhase(phase));
      ctx.log(`armed: the next change to /dev/hda stops ${CRASH_PHASE_LABELS[phase]}`);
    },
  };

  const recover: CommandDef = {
    spec: { name: "recover", summary: "Replay or discard the journal's unfinished transaction" },
    fn: (_args, _input, ctx) => {
      const journal = journalOf(host);
      const rec = fsCall("/dev/hda", () => host.run(() => journal.recover()));
      for (const e of rec.events) ctx.log(e.text);
    },
  };

  return [crash, recover];
}
