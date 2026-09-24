//! Byte-accurate ext2 (revision 1, 1 KiB blocks) on an in-memory disk. The
//! on-disk codecs live here; `ExtFs`, the filesystem itself, joins them in
//! `fs.rs`.

mod alloc;
pub mod bitmap;
pub mod blockmap;
pub mod dir;
pub mod events;
pub mod fs;
pub mod group;
pub mod inode;
pub mod journal;
pub mod superblock;

pub use dir::DirEntry;
pub use events::{BitmapKind, ExtEvent};
pub use fs::{BlockOwner, BlockRole, ExtFs};
pub use group::{Geometry, GroupDescriptor, GroupLayout};
pub use inode::Inode;
pub use journal::state::{JournalInfo, JournalState};
pub use journal::{
    decode_commit, decode_descriptor, default_journal_blocks, descriptor_blocks_needed,
    encode_commit, encode_descriptor, escape, journal_feature_names, needs_escape, read_header,
    unescape, wrap, write_header, BlockHeader, CrashPhase, JournalMode, JournalOptions,
    JournalSuperblock, Tag, BLOCKTYPE_COMMIT, BLOCKTYPE_DESCRIPTOR, BLOCKTYPE_REVOKE,
    BLOCKTYPE_SUPERBLOCK_V1, BLOCKTYPE_SUPERBLOCK_V2, COMMIT_NSEC_OFFSET, COMMIT_SEC_OFFSET,
    DEFM_JMODE_DATA, DEFM_JMODE_MASK, DEFM_JMODE_ORDERED, DEFM_JMODE_WBACK,
    FEATURE_COMPAT_HAS_JOURNAL, FEATURE_INCOMPAT_RECOVER, HEADER_LEN, JBD2_MAGIC, JOURNAL_INO,
    MIN_JOURNAL_BLOCKS, MIN_VOLUME_BLOCKS_FOR_JOURNAL, TAGS_PER_DESCRIPTOR, TAG_BYTES, TAG_DELETED,
    TAG_ESCAPE, TAG_LAST, TAG_SAME_UUID,
};
pub use superblock::{
    ExtFormatOptions, Superblock, BLOCKS_PER_GROUP, BLOCK_SIZE, DEFAULT_UUID, EXT2_MAGIC,
    FEATURE_INCOMPAT_FILETYPE, FEATURE_RO_COMPAT_SPARSE_SUPER, FIRST_INO, INODE_SIZE,
    LOST_FOUND_INO, MAX_BLOCKS, MIN_BLOCKS, RESERVED_INODES, ROOT_INO, SUPERBLOCK_OFFSET,
};
