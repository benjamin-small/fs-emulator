//! TypeScript declarations for every object the `Volume` methods return or
//! accept. wasm-bindgen appends this block to the generated `.d.ts`.

use wasm_bindgen::prelude::wasm_bindgen;

#[wasm_bindgen(typescript_custom_section)]
const TYPES: &str = r#"
export interface DateTime { year: number; month: number; day: number; hour: number; minute: number; second: number; }
export interface EntryInfo { name: string; isDir: boolean; size: number; created: DateTime | null; modified: DateTime | null; accessed: DateTime | null; }
export interface Range { start: number; end: number; }
export interface ByteChange { offset: number; before: Uint8Array; after: Uint8Array; }
export interface EventRecord { kind: string; text: string; region: Range | null; }
export interface OpRecord { op: string; changes: ByteChange[]; events: EventRecord[]; }
export type RegionKind = "boot" | "metadata" | "allocationTable" | "directory" | "data" | "reserved" | "other";
export interface Region { name: string; sectors: Range; kind: RegionKind; }
export interface Annotation { range: Range; label: string; value: string; }
export interface FormatOptions { bytesPerSector?: number; sectorsPerCluster?: number; totalSectors?: number; fatCount?: number; rootEntries?: number; reservedSectors?: number; volumeLabel?: string; volumeId?: number; enforceFat16Range?: boolean; }
export interface BootSector { oemName: string; bytesPerSector: number; sectorsPerCluster: number; reservedSectors: number; fatCount: number; rootEntries: number; totalSectors: number; media: number; sectorsPerFat: number; sectorsPerTrack: number; heads: number; hiddenSectors: number; driveNumber: number; bootSignature: number; volumeId: number; volumeLabel: string; fsType: string; }
export interface Geometry { variant: "fat16"; bytesPerSector: number; sectorsPerCluster: number; reservedSectors: number; fatCount: number; sectorsPerFat: number; rootEntries: number; rootDirSectors: number; firstRootDirSector: number; firstDataSector: number; totalSectors: number; clusterCount: number; }
export type FatEntry = { kind: "free" } | { kind: "next"; cluster: number } | { kind: "endOfChain" } | { kind: "bad" } | { kind: "reserved" };
export type RawEntry =
  | { kind: "free" }
  | { kind: "deleted"; bytes: Uint8Array }
  | { kind: "short"; name: string; attr: number; firstCluster: number; size: number; created: DateTime | null; modified: DateTime | null; accessed: DateTime | null; isDir: boolean }
  | { kind: "lfn"; order: number; isLast: boolean; checksum: number; text: string };
export interface ClusterOwner { cluster: number; path: string; isDir: boolean; firstCluster: number; }
export interface FsError extends Error { code: string; }
"#;
