export { Volume } from "fs-emulator-wasm";
export type {
  Annotation, BootSector, ByteChange, ClusterOwner, DateTime, EntryInfo, EventRecord, FatEntry,
  FormatOptions, FsError, Geometry, OpRecord, RawEntry, Range, Region, RegionKind,
} from "fs-emulator-wasm";

// The ext family's DTOs: only src/fs/ext/ may import them (tests/adapterBoundary.test.ts).
export type {
  Ext3FormatOptions, ExtBlockOwner, ExtBlockOwnerRole, ExtDirEntry, ExtFileBlocks, ExtFormatOptions,
  ExtGeometry, ExtGroup, ExtIndirectBlock, ExtInode, ExtInodeSlot, ExtSuperblock, JournalBlock,
  JournalBlockKind, JournalInfo, JournalMode,
} from "fs-emulator-wasm";
