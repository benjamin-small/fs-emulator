//! Byte-accurate ext2 (revision 1, 1 KiB blocks) on an in-memory disk. The
//! on-disk codecs live here; `ExtFs`, the filesystem itself, joins them in
//! `fs.rs`.

pub mod bitmap;
pub mod blockmap;
pub mod dir;
pub mod events;
pub mod group;
pub mod inode;
pub mod superblock;

pub use dir::DirEntry;
pub use events::{BitmapKind, ExtEvent};
pub use group::{Geometry, GroupDescriptor, GroupLayout};
pub use inode::Inode;
pub use superblock::{
    ExtFormatOptions, Superblock, BLOCKS_PER_GROUP, BLOCK_SIZE, DEFAULT_UUID, EXT2_MAGIC,
    FEATURE_INCOMPAT_FILETYPE, FEATURE_RO_COMPAT_SPARSE_SUPER, FIRST_INO, INODE_SIZE,
    LOST_FOUND_INO, MAX_BLOCKS, MIN_BLOCKS, RESERVED_INODES, ROOT_INO, SUPERBLOCK_OFFSET,
};
