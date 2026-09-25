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
export type RegionKind = "boot" | "metadata" | "allocationTable" | "directory" | "data" | "journal" | "reserved" | "other";
export interface Region { name: string; sectors: Range; kind: RegionKind; }
export interface Annotation { range: Range; label: string; value: string; }
export interface FormatOptions { bytesPerSector?: number; sectorsPerCluster?: number; totalSectors?: number; fatCount?: number; rootEntries?: number; reservedSectors?: number; volumeLabel?: string; volumeId?: number; enforceFat16Range?: boolean; }
export interface ExtFormatOptions { totalBlocks?: number; inodesPerGroup?: number; label?: string; uuid?: string; }
export type JournalMode = "ordered" | "data";
export interface Ext3FormatOptions extends ExtFormatOptions { journalBlocks?: number; journalMode?: JournalMode; }
export interface JournalInfo { inode: number; maxlen: number; firstBlock: number; sequence: number; start: number; head: number; mode: JournalMode; needsRecovery: boolean; maxTransaction: number; }
export type JournalBlockKind = "superblock" | "descriptor" | "copy" | "commit" | "revoke" | "unused";
export interface JournalBlock { index: number; block: number; kind: JournalBlockKind; tid: number | null; home: number | null; escaped: boolean | null; stale: boolean; }
export interface ExtGroup { index: number; firstBlock: number; blockCount: number; superblockBlock: number | null; descriptorsBlock: number | null; blockBitmap: number; inodeBitmap: number; inodeTable: number; firstData: number; freeBlocks: number; freeInodes: number; usedDirs: number; }
export interface ExtGeometry { blockSize: number; totalBlocks: number; firstDataBlock: number; blocksPerGroup: number; inodesPerGroup: number; inodesCount: number; inodeSize: number; inodeTableBlocks: number; descriptorBlocks: number; groups: ExtGroup[]; }
export interface ExtSuperblock { inodesCount: number; blocksCount: number; reservedBlocks: number; freeBlocks: number; freeInodes: number; firstDataBlock: number; logBlockSize: number; blocksPerGroup: number; inodesPerGroup: number; magic: number; state: number; revLevel: number; firstIno: number; inodeSize: number; featureCompat: number; featureIncompat: number; featureRoCompat: number; uuid: string; label: string; journalInum: number; defaultMountOpts: number; mtime: number; wtime: number; mntCount: number; }
export type ExtBlockOwnerRole = "data" | "directory" | "indirect" | "journal";
export interface ExtBlockOwner { block: number; inode: number; path: string; role: ExtBlockOwnerRole; }
export interface ExtInodeSlot { block: number; offset: number; }
export interface ExtInode { ino: number; mode: number; uid: number; gid: number; size: number; links: number; blocks: number; flags: number; atime: number; ctime: number; mtime: number; dtime: number; block: number[]; slot: ExtInodeSlot; }
export interface ExtDirEntry { block: number; offset: number; inode: number; recLen: number; nameLen: number; fileType: number; name: string; }
export interface ExtIndirectBlock { block: number; level: number; }
export interface ExtFileBlocks { data: number[]; indirect: ExtIndirectBlock[]; }
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
