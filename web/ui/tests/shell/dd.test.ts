import { describe, expect, it } from "vitest";
import { SIZE_HELP } from "../../src/shell/addr";
import { DD_MAX_BYTES, OPERAND_HELP, formatRecords, parseDd, planWindow } from "../../src/shell/dd";
import { ShellError } from "../../src/shell/errors";

function thrown(fn: () => unknown): ShellError {
  try { fn(); } catch (e) { return e as ShellError; }
  throw new Error("expected a throw");
}

describe("parseDd", () => {
  it("defaults: bs 512, skip 0, seek 0, no if/of/count", () => {
    expect(parseDd({}, [])).toEqual({ bs: 512, skip: 0, seek: 0 });
  });
  it("reads --if/--of/--bs/--count/--skip/--seek flags (strings from the 'str' shape, or ints)", () => {
    expect(parseDd({ if: "/dev/hda", of: "/mnt/boot.bin", bs: "512", count: "1", skip: "65", seek: 2 }, [])).toEqual({
      if: "/dev/hda", of: "/mnt/boot.bin", bs: 512, count: 1, skip: 65, seek: 2,
    });
    expect(parseDd({ bs: "1k", count: "2" }, [])).toEqual({ bs: 1024, count: 2, skip: 0, seek: 0 });
  });
  it("accepts quoted classic key=value operands and merges them with flags", () => {
    expect(parseDd({}, ["if=/dev/zero", "of=/dev/hda", "bs=512", "seek=0", "count=1"])).toEqual({
      if: "/dev/zero", of: "/dev/hda", bs: 512, count: 1, skip: 0, seek: 0,
    });
    expect(parseDd({ if: "/dev/hda" }, ["count=1"])).toEqual({ if: "/dev/hda", bs: 512, count: 1, skip: 0, seek: 0 });
  });
  it("rejects a key given twice, across flags and operands", () => {
    const e = thrown(() => parseDd({ if: "/dev/hda" }, ["if=/dev/zero"]));
    expect(e).toBeInstanceOf(ShellError);
    expect(e.message).toBe("'if' given twice");
    expect(thrown(() => parseDd({}, ["bs=1", "bs=2"])).message).toBe("'bs' given twice");
  });
  it("rejects unknown or malformed operands with help", () => {
    for (const op of ["foo", "conv=sync", "if", "=x"]) {
      const e = thrown(() => parseDd({}, [op]));
      expect(e.message).toBe(`unrecognized operand '${op}'`);
      expect(e.help).toBe(OPERAND_HELP);
    }
    expect(thrown(() => parseDd({}, [7])).message).toBe("unrecognized operand '7'");
  });
  it("validates sizes and paths", () => {
    const bs = thrown(() => parseDd({ bs: "x" }, []));
    expect(bs.message).toBe("invalid block size 'x'");
    expect(bs.help).toBe(SIZE_HELP);
    expect(thrown(() => parseDd({ bs: "0" }, [])).message).toBe("invalid block size '0'");
    expect(thrown(() => parseDd({}, ["count=-1"])).message).toBe("invalid count '-1'");
    expect(thrown(() => parseDd({ skip: "1.5" }, [])).message).toBe("invalid skip '1.5'");
    expect(thrown(() => parseDd({ seek: "abc" }, [])).message).toBe("invalid seek 'abc'");
    expect(thrown(() => parseDd({ if: "" }, [])).message).toBe("'if' needs a path");
    expect(thrown(() => parseDd({}, ["of="])).message).toBe("'of' needs a path");
    expect(thrown(() => parseDd({ if: null }, [])).message).toBe("'if' needs a path");
  });
});

describe("planWindow and the 1 MiB cap", () => {
  it("DD_MAX_BYTES is 1 MiB", () => {
    expect(DD_MAX_BYTES).toBe(1048576);
  });
  it("computes skip*bs and count*bs clipped to what is available", () => {
    expect(planWindow({ bs: 512, count: 1, skip: 65 }, 16 * 1048576)).toEqual({ start: 65 * 512, len: 512 });
    expect(planWindow({ bs: 512, count: undefined, skip: 0 }, 1000)).toEqual({ start: 0, len: 1000 });
    expect(planWindow({ bs: 512, count: 4, skip: 1 }, 1000)).toEqual({ start: 512, len: 488 });
    expect(planWindow({ bs: 512, count: 1, skip: 10 }, 1000)).toEqual({ start: 5120, len: 0 }); // skip past the end
    expect(planWindow({ bs: 512, count: 3, skip: 0 }, Infinity)).toEqual({ start: 0, len: 1536 }); // /dev/zero
  });
  it("refuses a read window over DD_MAX_BYTES before anything is written", () => {
    expect(planWindow({ bs: 1048576, count: 1, skip: 0 }, Infinity).len).toBe(DD_MAX_BYTES);
    const e = thrown(() => planWindow({ bs: 512, count: 2049, skip: 0 }, Infinity));
    expect(e).toBeInstanceOf(ShellError);
    expect(e.message).toBe("refusing to copy 1049088 bytes in one dd; the limit is 1048576 (1 MiB)");
    expect(e.help).toBe("lower --count or --bs, or copy in several runs with --skip and --seek");
    expect(thrown(() => planWindow({ bs: 512, count: undefined, skip: 0 }, 16 * 1048576)).message).toMatch(/^refusing to copy 16777216 bytes/);
  });
});

describe("formatRecords", () => {
  it("counts full and partial blocks like dd", () => {
    expect(formatRecords(512, 512)).toBe("1+0 records");
    expect(formatRecords(1000, 512)).toBe("1+1 records");
    expect(formatRecords(0, 512)).toBe("0+0 records");
    expect(formatRecords(3, 512)).toBe("0+1 records");
  });
});
