import { describe, expect, it } from "vitest";
import { findStrings, scanChunked, type StringHit } from "../src/core/strings";

const bytes = (s: string) => new TextEncoder().encode(s);

/** Runs a `scanChunked` generator to completion and returns its final hits. */
function drain(gen: Generator<unknown, StringHit[], void>): StringHit[] {
  let r = gen.next();
  while (!r.done) r = gen.next();
  return r.value;
}

/** Deterministic pseudo-random byte for a given index — no `Math.random()`,
 * so the test is reproducible, but the bit-mixing still produces a varied
 * spread of printable/non-printable runs of different lengths. */
function detByte(i: number): number {
  let x = Math.imul(i ^ 0x9e3779b9, 2654435761) >>> 0;
  x ^= x >>> 15;
  x = Math.imul(x, 2246822519) >>> 0;
  x ^= x >>> 13;
  return x & 0xff;
}

describe("findStrings", () => {
  it("finds printable runs of at least minRun", () => {
    const buf = new Uint8Array([...bytes("ab"), 0, ...bytes("hello"), 0, 0, ...bytes("wor"), 1, ...bytes("READMETXT")]);
    expect(findStrings(buf, 0, buf.length)).toEqual([
      { offset: 3, length: 5, text: "hello" },
      { offset: 14, length: 9, text: "READMETXT" },
    ]);
  });
  it("respects the range, minRun, and limit", () => {
    const buf = bytes("aaaa bbbb cccc");
    expect(findStrings(buf, 5, 9)).toEqual([{ offset: 5, length: 4, text: "bbbb" }]);
    expect(findStrings(buf, 0, buf.length, 15)).toEqual([]);
    expect(findStrings(buf, 0, buf.length, 4, 1).length).toBe(1);
  });
  it("a run touching the range end is included", () => {
    const buf = bytes("xxxxyyyy");
    expect(findStrings(buf, 0, 8, 4)).toEqual([{ offset: 0, length: 8, text: "xxxxyyyy" }]);
  });
  it("handles a run longer than the spread argument limit", () => {
    const buf = new Uint8Array(300_000).fill(0x41);
    const hits = findStrings(buf, 0, buf.length);
    expect(hits.length).toBe(1);
    expect(hits[0].length).toBe(300_000);
    expect(hits[0].text.length).toBe(300_000);
  });
});

describe("scanChunked", () => {
  it("carries a run straddling a chunk boundary into one hit instead of splitting it", () => {
    const buf = new Uint8Array(4096); // 4 KiB, all zero
    const runText = "Boundary10"; // 10 printable bytes
    const runStart = 1020; // chunk 0 is [0, 1024): this run spans 1020..1029, crossing it
    buf.set(bytes(runText), runStart);
    const hits = drain(scanChunked(buf, 1024, 4, 2000));
    expect(hits).toEqual([{ offset: runStart, length: runText.length, text: runText }]);
  });

  it("matches a single findStrings call over the whole buffer for arbitrary data", () => {
    const n = 5000;
    const buf = new Uint8Array(n);
    for (let i = 0; i < n; i++) buf[i] = detByte(i);
    const whole = findStrings(buf, 0, buf.length, 4, 2000);
    // A chunk size that doesn't evenly divide the buffer or align with any
    // particular run, so boundaries land mid-run throughout the scan.
    const chunked = drain(scanChunked(buf, 97, 4, 2000));
    expect(chunked).toEqual(whole);
  });
});
