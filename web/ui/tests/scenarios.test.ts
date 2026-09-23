import { describe, expect, it } from "vitest";
import { adapterFor, FAMILIES } from "../src/fs";
import { ScenarioCursor } from "../src/core/scenarioCursor";
import { all } from "../src/scenarios";
import { scenario as formatScenario } from "../src/scenarios/format";
import { scenario as shell } from "../src/scenarios/shell";

// Steps titled "Expect: ..." are the scenario's deliberate failure demonstrations; each
// one names the error code its action must throw.
const EXPECTED_ERROR_CODE: Record<string, string> = {
  "Expect: disk full": "DiskFull",
  "Expect: not empty": "DirectoryNotEmpty",
};

describe("scenario scripts", () => {
  it("every scenario has at least 3 steps, each with non-empty text", () => {
    for (const s of all) {
      expect(s.steps.length).toBeGreaterThanOrEqual(3);
      for (const step of s.steps) expect(step.text.trim().length).toBeGreaterThan(0);
    }
  });

  // browser-terminal 0.3.0 delivered redirection and `key=value` barewords, so step text
  // teaching the 0.2.0 workarounds is now wrong. A learner reads this copy and types what
  // it says; nothing else in the suite would catch a stale sentence.
  it("no step text still teaches a workaround 0.3.0 removed", () => {
    const stale = ["no `>`", "There is no `>`", "must be quoted"];
    for (const s of all) {
      for (const step of s.steps) {
        for (const phrase of stale) {
          expect(step.text, `${s.id} — "${step.title}"`).not.toContain(phrase);
        }
      }
    }
  });

  // `start()` formats the scenario's family before its first step, so an id the registry
  // does not know would only surface as a throw inside the runner. Pin it here instead.
  it("every scenario names a registered family", () => {
    for (const s of all) {
      expect(FAMILIES[s.family], `${s.id} names family "${s.family}"`).toBeDefined();
      expect(FAMILIES[s.family].id).toBe(s.family);
    }
  });

  // The runner itself needs runes, so the step bookkeeping it drives lives in a plain
  // class (src/core/scenarioCursor.ts) that can be exercised here.
  describe("step cursor", () => {
    it("runs each step once; going back and forward again only seeks", () => {
      const c = new ScenarioCursor(3);
      expect(c.canRunNext()).toBe(true);
      expect(c.next()).toEqual({ run: true });   // step 0
      c.advance(0);                              // volume.cursor after step 0
      expect(c.next()).toEqual({ run: true });   // step 1
      c.advance(1);                              // volume.cursor after step 1
      expect(c.index).toBe(1);

      expect(c.back()).toBe(0);                  // prev -> step 0's disk
      expect(c.index).toBe(0);
      expect(c.canRunNext()).toBe(false);
      expect(c.next()).toEqual({ seekTo: 1 });   // next -> step 1's disk, not a third run
      expect(c.index).toBe(1);
    });

    it("clamps at both ends", () => {
      const c = new ScenarioCursor(1);
      expect(c.back()).toBeNull();
      expect(c.next()).toEqual({ run: true });
      c.advance(7);
      expect(c.next()).toBeNull();               // past the last step
      expect(c.index).toBe(0);
      expect(c.back()).toBeNull();               // already at the first step
      expect(c.index).toBe(0);
    });
  });

  for (const s of all) {
    it(`runs "${s.title}" end to end against a fresh Volume`, () => {
      // The runner's shape: format the scenario's family, bind an adapter to the result, and
      // hand every action and function focus that adapter. A format is a new Volume (bind
      // again); an action is the same Volume with new contents (refresh).
      let vol = FAMILIES[s.family].format();
      let fs = adapterFor(vol);
      for (const step of s.steps) {
        const run = () => {
          if (step.format) {
            vol = FAMILIES[s.family].format(step.format);
            fs = adapterFor(vol);
          }
          if (step.action) {
            step.action(vol, fs);
            fs.refresh();
          }
          if (typeof step.focus === "function") step.focus(fs);
        };
        const expectedCode = EXPECTED_ERROR_CODE[step.title];
        if (step.title.startsWith("Expect:")) {
          expect(expectedCode, `unrecognized "Expect:" step title: ${step.title}`).toBeDefined();
          expect(run).toThrow(expect.objectContaining({ code: expectedCode }));
        } else {
          expect(run).not.toThrow();
        }
      }
    });
  }

  // The shell scenario's step text shows a terminal command and its action performs the
  // equivalent Volume call. Pin the outcomes the commands would leave behind.
  describe("work from the shell", () => {
    it("is registered last, after the eight explorer-driven scenarios", () => {
      expect(all.length).toBe(9);
      expect(all[all.length - 1]).toBe(shell);
      expect(shell.id).toBe("shell");
      expect(shell.steps.length).toBe(8);
      // Every step's text names the command it stands for.
      for (const step of shell.steps) expect(step.text).toMatch(/`[a-z]+[^`]*`/);
    });

    it("leaves the volume the way the equivalent commands would", () => {
      const vol = FAMILIES[shell.family].format();
      const fs = adapterFor(vol);
      const run = (i: number) => {
        const step = shell.steps[i];
        expect(step.action, `step ${i} "${step.title}" has no action`).toBeDefined();
        const rec = step.action!(vol, fs);
        fs.refresh();
        return rec;
      };
      const text = (b: Uint8Array) => new TextDecoder().decode(b);

      // echo 'Hello from the shell' | write /mnt/HELLO.TXT
      expect(run(1).op).toBe("create_file /HELLO.TXT");
      expect(text(vol.readFile("/HELLO.TXT"))).toBe("Hello from the shell");
      expect(vol.clusterOwners().find((o) => o.path === "/HELLO.TXT")?.firstCluster).toBe(2);

      // mkdir /mnt/DOCS
      expect(run(3).op).toBe("create_dir /DOCS");
      expect(vol.stat("/DOCS").isDir).toBe(true);

      // cp /mnt/HELLO.TXT /mnt/DOCS/COPY.TXT
      run(4);
      expect(vol.readFile("/DOCS/COPY.TXT")).toEqual(vol.readFile("/HELLO.TXT"));
      expect(vol.clusterOwners().find((o) => o.path === "/DOCS/COPY.TXT")?.firstCluster).toBe(4);

      // echo 'SHELLDISK  ' | dd --of=/dev/hda --bs=1 --seek=43
      // The BootSector DTO trims trailing spaces from the 11-byte label (dto.rs `text`).
      const patch = run(6);
      expect(patch.op).toBe("write_raw 0x2b +11");
      expect(patch.changes).toHaveLength(1);
      expect(patch.changes[0].offset).toBe(43);
      expect(patch.changes[0].after).toEqual(new TextEncoder().encode("SHELLDISK  "));
      expect(vol.bootSector().volumeLabel).toBe("SHELLDISK");
      expect(vol.corruption()).toBeNull();

      // rm /mnt/HELLO.TXT: the entry is marked deleted, DOCS (slot 1) is all that lists.
      expect(run(7).op).toBe("delete_file /HELLO.TXT");
      expect(vol.listDir("/").map((e) => e.name)).toEqual(["DOCS"]);
      expect(text(vol.readFile("/DOCS/COPY.TXT"))).toBe("Hello from the shell");
    });
  });

  // "Format an empty disk" quotes no numbers in its own copy, but its function-form
  // focuses resolve region starts from the adapter; pin them to the default disk's sectors.
  describe("format an empty disk", () => {
    it("points the root directory and free space steps at sectors 65 and 97 on the default disk", () => {
      const vol = FAMILIES.fat16.format();
      const fs = adapterFor(vol);
      const focusOf = (title: string) => {
        const step = formatScenario.steps.find((s) => s.title === title)!;
        return typeof step.focus === "function" ? step.focus(fs) : step.focus;
      };
      expect(focusOf("The root directory")).toEqual({ sector: 65 });
      expect(focusOf("Free space collapses")).toEqual({ sector: 97 });
    });
  });
});
