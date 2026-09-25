import type { CommandDef } from "./types";
import { FAMILIES } from "../fs";
import type { FsFamily, MkfsFlag } from "../fs/adapter";
import { flagGiven } from "./bytes";
import { ShellError, fsCall } from "./errors";
import type { ShellHost } from "./host";
import { Vfs, promptFor } from "./vfs";

/**
 * The `mkfs` command and the helpers that build its spec from a family's `mkfs` flags. Each tab
 * formats its own family only, so the spec is built from the mounted family alone: `--type`
 * picks the variant (ext2 or ext3 on the ext tab), and another family's type names the tab
 * that formats it.
 */

/** `a, b, or c` (`a or b` for two): the way a summary lists choices. */
export function orList(items: readonly string[]): string {
  return items.length <= 2 ? items.join(" or ") : `${items.slice(0, -1).join(", ")}, or ${items.at(-1)}`;
}

/** `mkfs --type`'s values: the given families' `fsTypes`, lower-cased, in registry order
 *  (`fat16` on the FAT16 tab; `ext2`, `ext3` on the ext tab). */
export function mkfsTypes(families: Readonly<Record<string, FsFamily>>): string[] {
  return Object.values(families).flatMap((f) => f.fsTypes.map((t) => t.toLowerCase()));
}

/** The registered family one of whose `fsTypes` is `type`, compared lower-cased (`ext3` is
 *  ext's, `FAT16` is fat16's); `undefined` for a type no family formats. */
export function familyOfType(type: string): FsFamily | undefined {
  const wanted = type.toLowerCase();
  return Object.values(FAMILIES).find((f) => f.fsTypes.some((t) => t.toLowerCase() === wanted));
}

/** `mkfs`'s default `--type`: the mounted volume's own type, lower-cased (`FAT16` is `fat16`). */
export function mkfsTypeOf(host: ShellHost): string {
  return host.vol.fsType().toLowerCase();
}

/**
 * The given families' `mkfs.flags` as one list, in registry order and deduplicated by `long`.
 * `mkfs` passes its one family, whose flags come back in their own words. A flag two families
 * share keeps the first one's kind and merges the descriptions, each split at its first comma,
 * as `${base} (fat16: ${restA}; ext: ${restB})`.
 */
export function mkfsFlagsFor(families: Readonly<Record<string, FsFamily>>): Omit<MkfsFlag, "option">[] {
  const split = (desc: string): [string, string] => {
    const i = desc.indexOf(",");
    return i < 0 ? [desc, ""] : [desc.slice(0, i), desc.slice(i + 1).trim()];
  };
  const byLong = new Map<string, { kind: MkfsFlag["kind"]; descs: { id: string; desc: string }[] }>();
  for (const family of Object.values(families)) {
    for (const f of family.mkfs.flags) {
      const seen = byLong.get(f.long);
      if (seen) seen.descs.push({ id: family.id, desc: f.desc });
      else byLong.set(f.long, { kind: f.kind, descs: [{ id: family.id, desc: f.desc }] });
    }
  }
  return [...byLong].map(([long, { kind, descs }]) => ({
    long,
    kind,
    desc: descs.length === 1 ? descs[0].desc : `${split(descs[0].desc)[0]} (${descs.map((d) => `${d.id}: ${split(d.desc)[1]}`).join("; ")})`,
  }));
}

/** `a` or `an` before `word`, by its first letter (`an ext3`, `a FAT16`). */
const article = (word: string): string => (/^[aeiou]/i.test(word) ? "an" : "a");

/** `mkfs`: format `/dev/hda` as `--type` (default: the mounted volume's type), one of the tab's
 *  family's types, with that family's flags; the done line names the type the new volume reports. */
export function mkfsCommand(host: ShellHost, vfs: Vfs): CommandDef {
  // The spec is the mounted family's alone: a tab formats its own family, so the summary, the
  // `--type` values, and the flags (each in the family's own words) are that family's. A
  // registration never outlives its family: the terminal re-registers on a tab switch.
  const family = FAMILIES[host.adapter.id];
  const own = { [family.id]: family };
  const types = mkfsTypes(own);
  const flags = mkfsFlagsFor(own);
  const typesHelp = `types: ${types.join(", ")}`;
  return {
    spec: {
      name: "mkfs",
      summary: `Format /dev/hda (clears the timeline); --type picks ${orList(types)}`,
      flags: [
        { long: "type", desc: `${orList(types)} (default: the mounted volume's type)`, shape: "str" },
        ...flags.map((f) => ({ long: f.long, desc: f.desc, shape: f.kind })),
      ],
    },
    fn: (args, _input, ctx) => {
      const typeFlag = args.flags.type;
      const type = flagGiven(typeFlag) ? String(typeFlag).toLowerCase() : mkfsTypeOf(host);
      const owner = familyOfType(type);
      if (!owner) throw new ShellError(`unknown type '${type}'`, { help: typesHelp });
      if (owner.id !== family.id) {
        throw new ShellError(`'${type}' is ${article(owner.name)} ${owner.name} type: switch to the ${owner.name} tab to format one`, { help: typesHelp });
      }
      // browser-terminal refuses a flag the spec does not list before this runs; a caller that
      // bypasses the parser (the tests) still gets a refusal naming the type.
      for (const [long, v] of Object.entries(args.flags)) {
        if (long !== "type" && flagGiven(v) && !flags.some((f) => f.long === long)) {
          throw new ShellError(`--${long} is not ${article(type)} ${type} option`);
        }
      }
      // A family with several types (ext: ext2, ext3) takes the one asked for as its `variant`.
      const options: Record<string, number | string> = family.fsTypes.length > 1 ? { variant: type } : {};
      for (const f of family.mkfs.flags) {
        const v = args.flags[f.long];
        if (!flagGiven(v)) continue;
        if (f.kind === "int") {
          if (typeof v !== "number" || !Number.isInteger(v) || v < 0) throw new ShellError(`--${f.long} must be a non-negative integer`);
          options[f.option] = v;
        } else {
          options[f.option] = String(v);
        }
      }
      fsCall("/dev/hda", () => host.format(family.id, options));
      vfs.cwd = "/mnt";
      host.setPrompt(promptFor(vfs.cwd));
      ctx.log(`formatted /dev/hda as ${host.vol.fsType()}; the timeline was cleared`);
    },
  };
}
