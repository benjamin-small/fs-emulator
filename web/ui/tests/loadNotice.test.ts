import { describe, expect, it } from "vitest";
import { articleFor, loadNotice, loadPlan } from "../src/core/loadNotice";
import { FAMILIES, detectFamily } from "../src/fs";
import { Volume } from "../src/lib/wasm";

describe("articleFor", () => {
  it("answers an before a vowel and a before anything else, in either case", () => {
    expect(articleFor("ext3")).toBe("an");
    expect(articleFor("ext2")).toBe("an");
    expect(articleFor("Image")).toBe("an");
    expect(articleFor("FAT16")).toBe("a");
    expect(articleFor("fat16")).toBe("a");
    expect(articleFor("")).toBe("a");
  });
});

describe("loadNotice", () => {
  it("names the file, the tab it opened in, its type, and the tab left behind", () => {
    expect(loadNotice("disk.img", "ext", "ext3", "FAT16")).toBe(
      "Opened disk.img in the ext tab: it is an ext3 image. The FAT16 tab is as you left it.",
    );
    expect(loadNotice("old.img", "FAT16", "FAT16", "ext")).toBe(
      "Opened old.img in the FAT16 tab: it is a FAT16 image. The ext tab is as you left it.",
    );
  });
});

describe("loadPlan", () => {
  it("leaves a same-tab load alone: no notice, and a running lesson keeps its card", () => {
    expect(loadPlan("fat16", "fat16", true)).toEqual({ stopLesson: false, notice: false });
    expect(loadPlan("ext", "ext", false)).toEqual({ stopLesson: false, notice: false });
  });

  it("gives a routed load a notice, and closes a lesson running in the target tab", () => {
    expect(loadPlan("fat16", "ext", true)).toEqual({ stopLesson: true, notice: true });
    expect(loadPlan("ext", "fat16", false)).toEqual({ stopLesson: false, notice: true });
  });
});

describe("detectFamily", () => {
  it("mounts a FAT16 image and names the fat16 family", () => {
    const { vol, id } = detectFamily(FAMILIES.fat16.format().image());
    expect(id).toBe("fat16");
    expect(vol.fsType()).toBe("FAT16");
  });

  it("mounts an ext3 or ext2 image and names the one ext family", () => {
    const ext3 = detectFamily(Volume.formatExt3(undefined).image());
    expect(ext3.id).toBe("ext");
    expect(ext3.vol.fsType()).toBe("ext3");
    const ext2 = detectFamily(Volume.formatExt2(undefined).image());
    expect(ext2.id).toBe("ext");
    expect(ext2.vol.fsType()).toBe("ext2");
  });

  it("throws today's load error for bytes no family recognises", () => {
    const garbage = new TextEncoder().encode("not a disk image ".repeat(64));
    expect(() => detectFamily(garbage)).toThrow("unsupported: no recognisable filesystem signature");
    expect(() => detectFamily(new Uint8Array(0))).toThrow("unsupported: no recognisable filesystem signature");
  });
});
