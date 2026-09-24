//! Browser bindings for the emulator: one `Volume` class over the
//! `FileSystem` trait (FAT16, ext2, and ext3 today) plus family-specific
//! inspection and the ext3 journal's crash and recovery methods, with plain
//! JS objects crossing the boundary.

pub mod dto;
pub mod error;
pub mod types;
pub mod volume;

pub use volume::Volume;

use wasm_bindgen::prelude::wasm_bindgen;

/// Runs once when the module loads so any panic reaches the console with a message.
#[wasm_bindgen(start)]
fn init() {
    console_error_panic_hook::set_once();
}
