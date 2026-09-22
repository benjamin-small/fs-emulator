import { Volume } from "fs-emulator-wasm";
import type { Annotation, ClusterOwner, DateTime, FsError, OpRecord } from "fs-emulator-wasm";

const $ = <T extends HTMLElement>(id: string): T => document.getElementById(id) as T;

let volume = Volume.formatFat16(undefined);
let owners: ClusterOwner[] = volume.clusterOwners();
let exportUrl: string | null = null;

function status(text: string): void {
  $("status").textContent = text;
}

function fail(e: unknown): void {
  const err = e as Partial<FsError>;
  status(`${err.code ?? "Error"}: ${err.message ?? String(e)}`);
}

function esc(s: string): string {
  return s.replace(/[&<>]/g, (c) => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;" })[c] ?? c);
}

function fmtTime(t: DateTime | null): string {
  if (!t) return "";
  const p = (n: number) => String(n).padStart(2, "0");
  return `${t.year}-${p(t.month)}-${p(t.day)} ${p(t.hour)}:${p(t.minute)}:${p(t.second)}`;
}

function renderListing(): void {
  const rows = volume
    .listDir("/")
    .map((e) => `<tr><td>${esc(e.name)}</td><td>${e.isDir ? "dir" : "file"}</td><td>${e.size}</td><td>${fmtTime(e.modified)}</td></tr>`)
    .join("");
  $("listing").innerHTML = `<table><tr><th>Name</th><th>Type</th><th>Size</th><th>Modified</th></tr>${rows}</table>`;
}

function changedSectors(rec: OpRecord): number[] {
  const size = volume.sectorSize();
  const sectors = new Set<number>();
  for (const c of rec.changes) {
    const first = Math.floor(c.offset / size);
    const last = Math.floor((c.offset + Math.max(c.after.length, 1) - 1) / size);
    for (let s = first; s <= last; s++) sectors.add(s);
  }
  return [...sectors].sort((a, b) => a - b);
}

function renderLastOp(): void {
  const n = volume.historyLength();
  if (n === 0) {
    $("lastop").textContent = "no operations yet";
    return;
  }
  const rec = volume.historyAt(n - 1);
  const items = rec.events.map((e) => `<li><code>${esc(e.kind)}</code> ${esc(e.text)}</li>`).join("");
  $("lastop").innerHTML = `<h3>${esc(rec.op)}</h3><p>sectors changed: ${changedSectors(rec).join(", ")}</p><ul>${items}</ul>`;
}

function renderSector(): void {
  const n = Number($<HTMLInputElement>("sector").value) || 0;
  const region = volume.layout().find((r) => n >= r.sectors.start && n < r.sectors.end);
  $("region").textContent = region ? `${region.name} (${region.kind})` : "out of range";
  let bytes: Uint8Array;
  let annotations: Annotation[];
  try {
    bytes = volume.sector(n);
    annotations = volume.annotateSectorWith(n, owners);
  } catch (e) {
    fail(e);
    return;
  }
  const lines: string[] = [];
  for (let row = 0; row < bytes.length; row += 16) {
    const cells: string[] = [];
    for (let i = row; i < row + 16 && i < bytes.length; i++) {
      cells.push(`<span data-i="${i}">${bytes[i].toString(16).padStart(2, "0")}</span>`);
    }
    lines.push(`${row.toString(16).padStart(4, "0")}  ${cells.join(" ")}`);
  }
  $("hex").innerHTML = lines.join("\n");
  $("annotations").innerHTML = annotations
    .map((a, i) => `<li data-a="${i}"><b>${a.range.start}..${a.range.end}</b> ${esc(a.label)}: ${esc(a.value)}</li>`)
    .join("");
  const spans = $("hex").querySelectorAll<HTMLSpanElement>("span");
  $("annotations").querySelectorAll<HTMLLIElement>("li").forEach((li, i) => {
    const { start, end } = annotations[i].range;
    li.addEventListener("mouseenter", () => spans.forEach((s) => s.classList.toggle("hl", Number(s.dataset.i) >= start && Number(s.dataset.i) < end)));
    li.addEventListener("mouseleave", () => spans.forEach((s) => s.classList.remove("hl")));
  });
}

function refresh(): void {
  try {
    owners = volume.clusterOwners();
    renderListing();
    renderLastOp();
    renderSector();
    if (exportUrl) URL.revokeObjectURL(exportUrl);
    const blob = new Blob([new Uint8Array(volume.image())], { type: "application/octet-stream" });
    exportUrl = URL.createObjectURL(blob);
    $<HTMLAnchorElement>("export").href = exportUrl;
  } catch (e) {
    fail(e);
  }
}

function mutate(action: () => void): void {
  try {
    action();
    status("");
  } catch (e) {
    fail(e);
  }
  refresh();
}

const path = () => $<HTMLInputElement>("path").value;
const content = () => new TextEncoder().encode($<HTMLTextAreaElement>("content").value);

$("add").addEventListener("click", () => mutate(() => volume.createFile(path(), content())));
$("overwrite").addEventListener("click", () => mutate(() => volume.writeFile(path(), content())));
$("delete").addEventListener("click", () => mutate(() => volume.deleteFile(path())));
$("mkdir").addEventListener("click", () => mutate(() => volume.createDir(path())));
$("sector").addEventListener("input", renderSector);
$<HTMLInputElement>("load").addEventListener("change", async (ev) => {
  const file = (ev.target as HTMLInputElement).files?.[0];
  if (!file) return;
  let bytes: Uint8Array;
  try {
    bytes = new Uint8Array(await file.arrayBuffer());
  } catch (e) {
    fail(e);
    return;
  }
  mutate(() => {
    volume = Volume.fromImage(bytes);
  });
});

refresh();
