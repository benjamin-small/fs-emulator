import type { CommandDef } from "./types";
import { FAMILIES } from "../fs";
import type { FsFamily, MkfsFlag } from "../fs/adapter";
import { flagGiven } from "./bytes";
import { ShellError, fsCall } from "./errors";
import type { ShellHost } from "./host";
import { Vfs, promptFor } from "./vfs";

/**
 * The `mkfs` command and the helpers that build its one spec from every family's `mkfs`
 * flags: `--type` picks the family (and the variant, for a family with several types), so
 * the spec does not depend on the family mounted at registration.
 */

/** `a, b, or c` (`a or b` for two): the way a summary lists choices. */
export function orList(items: readonly string[]): string {
  return items.length <= 2 ? items.join(" or ") : `${items.slice(0, -1).join(", ")}, or ${items.at(-1)}`;
}

/** `mkfs --type`'s values: every family's `fsTypes`, lower-cased, in registry order
 *  (`fat16`, `ext2`, `ext3`). */
export function mkfsTypes(families: Readonly<Record<string, FsFamily>>): string[] {
  return Object.values(families).flatMap((f) => f.fsTypes.map((t) => t.toLowerCase()));
}

/** `mkfs`'s default `--type`: the mounted volume's own type, lower-cased (`FAT16` is `fat16`). */
export function mkfsTypeOf(host: ShellHost): string {
  return host.vol.fsType().toLowerCase();
}

/**
 * Every family's `mkfs.flags` as one list, in registry order and deduplicated by `long`. A flag
 * two families share keeps the first one's kind and merges the descriptions, each split at its
 * first comma, as `${base} (fat16: ${restA}; ext: ${restB})`: `--label` reads
 * `volume label (fat16: up to 11 characters; ext: up to 16 bytes)`.
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

/** `mkfs`: format `/dev/hda` as `--type` (default: the mounted volume's type) with the chosen
 *  family's flags; the done line names the type the new volume reports. */
export function mkfsCommand(host: ShellHost, vfs: Vfs): CommandDef {
  // One `mkfs` for every family: `--type` picks the family (and the variant, for a family with
  // several types), and the flag list is every family's, so the spec does not depend on the
  // family mounted at registration. The option keys come from the chosen family's own flags.
  // The done line names the type the new volume reports.
  const types = mkfsTypes(FAMILIES);
  const union = mkfsFlagsFor(FAMILIES);
  return {
    spec: {
      name: "mkfs",
      summary: `Format /dev/hda (clears the timeline); --type picks ${orList(types)}`,
      flags: [
        { long: "type", desc: `${orList(types)} (default: the mounted volume's type)`, shape: "str" },
        ...union.map((f) => ({ long: f.long, desc: f.desc, shape: f.kind })),
      ],
    },
    fn: (args, _input, ctx) => {
      const typeFlag = args.flags.type;
      const type = flagGiven(typeFlag) ? String(typeFlag).toLowerCase() : mkfsTypeOf(host);
      const family = Object.values(FAMILIES).find((f) => f.fsTypes.some((t) => t.toLowerCase() === type));
      if (!family) throw new ShellError(`unknown type '${type}'`, { help: `types: ${types.join(", ")}` });
      const { flags } = family.mkfs;
      for (const f of union) {
        if (flagGiven(args.flags[f.long]) && !flags.some((g) => g.long === f.long)) {
          throw new ShellError(`--${f.long} is not ${/^[aeiou]/i.test(type) ? "an" : "a"} ${type} option`);
        }
      }
      // A family with several types (ext: ext2, ext3) takes the one asked for as its `variant`.
      const options: Record<string, number | string> = family.fsTypes.length > 1 ? { variant: type } : {};
      for (const f of flags) {
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
