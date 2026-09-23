import type { RedirectContext, RedirectHandler, Value } from "./types";
import { decodeText, toBytes } from "./bytes";
import { RAW_DEVICE_HELP, readVolumeFile, writeVolumeFile } from "./commands";
import { ShellError } from "./errors";
import type { ShellHost } from "./host";
import { resolved, type Vfs } from "./vfs";

/** Where `>` sends someone who aimed at `/dev/hda`: the tool that can place bytes by address. */
export const RAW_WRITE_HELP = "place the bytes at an address with: dd --of=/dev/hda --bs=512 --seek=<blocks>";

/**
 * browser-terminal 0.3.0 hands `>`, `>>` and `<` to the host rather than inventing a
 * filesystem, so this is the explorer's: the same `/mnt` and `/dev` tree the commands
 * resolve, the same journaled `host.run` every other write goes through, and the same
 * errors — `echo hi > /mnt/A.TXT` lands in the timeline exactly like
 * `echo hi | write /mnt/A.TXT`.
 *
 * Neither hook gets a `CommandCtx`, so there is nowhere to log or warn. That costs the
 * `write` command's byte counts and the rewound-timeline warning; the behaviour itself is
 * unchanged, including reading the latest state while the timeline is rewound and the
 * `CorruptImage` gate, which reaches the terminal because `fsCall`/`wrapFs` attach the
 * recovery hint to the thrown error and browser-terminal keeps a thrown `help`.
 */
export function createRedirectHandler(host: ShellHost, vfs: Vfs): RedirectHandler {
  return {
    /** `< target`: the value handed to the first command of the pipeline. */
    read(target: string): Value {
      const { r, display } = resolved(vfs, target);
      switch (r.kind) {
        case "root":
        case "dev":
          throw new ShellError(`${display}: Is a directory`, { code: "IsADirectory", help: "read a file instead, e.g. cat /mnt/A.TXT" });
        case "raw":
        case "zero":
          throw new ShellError(`${display}: is a raw device`, { help: RAW_DEVICE_HELP });
        case "null":
          return "";
        case "volume":
          // Text, like `cat` without `--bytes`: `<` feeds a command's arguments, and the
          // commands that read a redirect (`str`, `filter`, `from json`) want text.
          // `dd --if=` and `xxd` are the way in for bytes.
          return decodeText(readVolumeFile(host, vfs, r.path));
      }
    },

    /** `> target` / `>> target`: the pipeline's collected value, already complete. */
    write(target: string, value: Value, ctx: RedirectContext & { append: boolean }): void {
      const { r, display } = resolved(vfs, target);
      switch (r.kind) {
        case "root":
        case "dev":
          throw new ShellError(`${display}: Is a directory`, { code: "IsADirectory", help: "redirect into a file under /mnt, e.g. > /mnt/OUT.TXT" });
        case "raw":
        case "zero":
          throw new ShellError(`${display}: cannot redirect into a raw device`, { help: RAW_WRITE_HELP });
        case "null":
          // Converted first, so a record is still refused: `> /dev/null` must not be the
          // one target that accepts a value nothing else would.
          toBytes(value);
          return;
        case "volume":
          writeVolumeFile(host, vfs, r.path, toBytes(value), { append: ctx.append });
          return;
      }
    },
  };
}
