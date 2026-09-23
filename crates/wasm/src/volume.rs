//! The one class a UI talks to. Generic operations go through the
//! `FileSystem` trait; FAT-specific inspection is gated on the inner variant.

use crate::dto;
use crate::error::{js_error, to_js};
use fat::FatFs;
use fs_core::FileSystem;
use js_sys::Object;
use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;

enum Inner {
    Fat(FatFs),
}

/// The family an image's signature names; `from_image` picks the parser from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Detected {
    Fat,
    Unknown,
}

/// Detection rule: FAT when the image is at least one 512-byte sector long
/// and bytes 510..512 are `55 AA`. Whether the BPB inside parses is
/// `FatFs::from_image`'s job, not this function's.
///
/// Reserved for the ext slice: an ext superblock is recognised by the `u16le`
/// at offset 1080 equal to `0xEF53`, and that check must run before the FAT
/// check because a bootable ext image can also carry `55 AA` at 510. Once
/// `Inner::Ext` exists, `Unknown` becomes
/// `Unsupported("no recognisable filesystem signature")` in `from_image`.
fn detect(bytes: &[u8]) -> Detected {
    if bytes.len() >= 512 && bytes[510..512] == [0x55, 0xAA] {
        Detected::Fat
    } else {
        Detected::Unknown
    }
}

/// Largest volume the wrapper will allocate in browser memory.
pub const MAX_VOLUME_BYTES: u64 = 256 * 1024 * 1024;

#[wasm_bindgen]
pub struct Volume {
    inner: Inner,
}

// `Result::unwrap_err` requires the `Ok` type to implement `Debug`. Neither
// `fat::FatFs` nor `fs_core::Disk` derive it, so this is written by hand
// rather than derived; it is never shown to JS, only used by `.unwrap_err()`
// in tests and any other Rust-side debug formatting.
impl std::fmt::Debug for Volume {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Volume")
            .field("fs_type", &self.fs().fs_type())
            .finish()
    }
}

fn to_value<T: Serialize>(value: &T) -> Result<JsValue, JsValue> {
    let serializer = Serializer::new()
        .serialize_maps_as_objects(true)
        .serialize_missing_as_null(true);
    value
        .serialize(&serializer)
        .map_err(|e| js_error("BadArgument", &e.to_string()))
}

fn from_value<T: serde::de::DeserializeOwned>(value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value).map_err(|e| js_error("BadArgument", &e.to_string()))
}

/// Reject any own key of `value` that is not in `allowed`. `#[serde(deny_unknown_fields)]`
/// can't do this itself at the JS boundary: `serde-wasm-bindgen` deserializes a struct by
/// looking up each known field by name on the object, never by iterating the object's own
/// keys, so a typo'd key is simply never observed rather than rejected.
fn reject_unknown_keys(value: &JsValue, allowed: &[&str]) -> Result<(), JsValue> {
    for key in Object::keys(value.unchecked_ref::<Object>()).iter() {
        let key = key.as_string().unwrap_or_default();
        if !allowed.contains(&key.as_str()) {
            return Err(js_error(
                "BadArgument",
                &format!("unknown option \"{key}\""),
            ));
        }
    }
    Ok(())
}

impl Volume {
    fn fs(&self) -> &dyn FileSystem {
        match &self.inner {
            Inner::Fat(f) => f,
        }
    }

    fn fs_mut(&mut self) -> &mut dyn FileSystem {
        match &mut self.inner {
            Inner::Fat(f) => f,
        }
    }

    /// The FAT volume: `Ok` for the only variant today; the `NotFat` arm
    /// arrives with `Inner::Ext`.
    fn fat(&self) -> Result<&FatFs, JsValue> {
        match &self.inner {
            Inner::Fat(f) => Ok(f),
        }
    }

    fn op(&mut self, result: fs_core::Result<fs_core::OpRecord>) -> Result<JsValue, JsValue> {
        let record = result.map_err(to_js)?;
        to_value(&dto::OpRecord::from(&record))
    }
}

#[wasm_bindgen]
impl Volume {
    /// Format a fresh FAT16 volume. `options` may be `undefined`.
    #[wasm_bindgen(js_name = formatFat16)]
    pub fn format_fat16(
        #[wasm_bindgen(unchecked_param_type = "FormatOptions | undefined")] options: JsValue,
    ) -> Result<Volume, JsValue> {
        let opts: dto::FormatOptions = if options.is_undefined() || options.is_null() {
            dto::FormatOptions::default()
        } else {
            reject_unknown_keys(&options, dto::FormatOptions::FIELDS)?;
            from_value(options)?
        };
        let opts: fat::FormatOptions = opts.into();
        let bytes = opts.total_sectors as u64 * opts.bytes_per_sector as u64;
        if bytes > MAX_VOLUME_BYTES {
            return Err(js_error(
                "BadArgument",
                &format!("volume of {bytes} bytes exceeds the {MAX_VOLUME_BYTES}-byte limit"),
            ));
        }
        let fs = FatFs::format(opts).map_err(to_js)?;
        Ok(Volume {
            inner: Inner::Fat(fs),
        })
    }

    #[wasm_bindgen(js_name = fromImage)]
    pub fn from_image(bytes: Vec<u8>) -> Result<Volume, JsValue> {
        let len = bytes.len() as u64;
        if len > MAX_VOLUME_BYTES {
            return Err(js_error(
                "BadArgument",
                &format!("volume of {len} bytes exceeds the {MAX_VOLUME_BYTES}-byte limit"),
            ));
        }
        let fs = match detect(&bytes) {
            // Both arms parse as FAT today, so an image without the signature
            // still fails inside `FatFs::from_image` with the same
            // `CorruptImage` text and code as before. The ext slice adds an
            // `Ext` arm and turns `Unknown` into `Unsupported`.
            Detected::Fat | Detected::Unknown => FatFs::from_image(bytes).map_err(to_js)?,
        };
        Ok(Volume {
            inner: Inner::Fat(fs),
        })
    }

    #[wasm_bindgen(js_name = fsType)]
    pub fn fs_type(&self) -> String {
        self.fs().fs_type().to_string()
    }

    #[wasm_bindgen(js_name = setNow)]
    pub fn set_now(
        &mut self,
        #[wasm_bindgen(unchecked_param_type = "DateTime")] now: JsValue,
    ) -> Result<(), JsValue> {
        let now: dto::DateTime = from_value(now)?;
        self.fs_mut().set_now(now.into());
        Ok(())
    }

    #[wasm_bindgen(js_name = createFile, unchecked_return_type = "OpRecord")]
    pub fn create_file(&mut self, path: &str, data: &[u8]) -> Result<JsValue, JsValue> {
        let r = self.fs_mut().create_file(path, data);
        self.op(r)
    }

    #[wasm_bindgen(js_name = writeFile, unchecked_return_type = "OpRecord")]
    pub fn write_file(&mut self, path: &str, data: &[u8]) -> Result<JsValue, JsValue> {
        let r = self.fs_mut().write_file(path, data);
        self.op(r)
    }

    #[wasm_bindgen(js_name = readFile)]
    pub fn read_file(&self, path: &str) -> Result<Vec<u8>, JsValue> {
        self.fs().read_file(path).map_err(to_js)
    }

    #[wasm_bindgen(js_name = deleteFile, unchecked_return_type = "OpRecord")]
    pub fn delete_file(&mut self, path: &str) -> Result<JsValue, JsValue> {
        let r = self.fs_mut().delete_file(path);
        self.op(r)
    }

    #[wasm_bindgen(js_name = createDir, unchecked_return_type = "OpRecord")]
    pub fn create_dir(&mut self, path: &str) -> Result<JsValue, JsValue> {
        let r = self.fs_mut().create_dir(path);
        self.op(r)
    }

    #[wasm_bindgen(js_name = removeDir, unchecked_return_type = "OpRecord")]
    pub fn remove_dir(&mut self, path: &str) -> Result<JsValue, JsValue> {
        let r = self.fs_mut().remove_dir(path);
        self.op(r)
    }

    #[wasm_bindgen(js_name = listDir, unchecked_return_type = "EntryInfo[]")]
    pub fn list_dir(&self, path: &str) -> Result<JsValue, JsValue> {
        let entries: Vec<dto::EntryInfo> = self
            .fs()
            .list_dir(path)
            .map_err(to_js)?
            .into_iter()
            .map(Into::into)
            .collect();
        to_value(&entries)
    }

    #[wasm_bindgen(unchecked_return_type = "EntryInfo")]
    pub fn stat(&self, path: &str) -> Result<JsValue, JsValue> {
        to_value(&dto::EntryInfo::from(self.fs().stat(path).map_err(to_js)?))
    }

    #[wasm_bindgen(unchecked_return_type = "Region[]")]
    pub fn layout(&self) -> Result<JsValue, JsValue> {
        let regions: Vec<dto::Region> = self.fs().layout().into_iter().map(Into::into).collect();
        to_value(&regions)
    }

    #[wasm_bindgen(js_name = annotateSector, unchecked_return_type = "Annotation[]")]
    pub fn annotate_sector(&self, sector: u32) -> Result<JsValue, JsValue> {
        let list: Vec<dto::Annotation> = self
            .fs()
            .annotate_sector(sector as u64)
            .into_iter()
            .map(Into::into)
            .collect();
        to_value(&list)
    }

    #[wasm_bindgen(js_name = historyLength)]
    pub fn history_length(&self) -> usize {
        self.fs().history().len()
    }

    #[wasm_bindgen(js_name = historyAt, unchecked_return_type = "OpRecord")]
    pub fn history_at(&self, index: usize) -> Result<JsValue, JsValue> {
        let history = self.fs().history();
        let record = history.get(index).ok_or_else(|| {
            js_error(
                "BadArgument",
                &format!(
                    "history index {index} is out of range (length {})",
                    history.len()
                ),
            )
        })?;
        to_value(&dto::OpRecord::from(record))
    }

    #[wasm_bindgen(js_name = sectorSize)]
    pub fn sector_size(&self) -> usize {
        self.fs().disk().sector_size()
    }

    #[wasm_bindgen(js_name = sectorCount)]
    pub fn sector_count(&self) -> u32 {
        self.fs().disk().sector_count() as u32
    }

    /// A copy of one sector's bytes.
    pub fn sector(&self, n: u32) -> Result<Vec<u8>, JsValue> {
        let disk = self.fs().disk();
        if n as u64 >= disk.sector_count() {
            return Err(js_error(
                "BadArgument",
                &format!("sector {n} is out of range (count {})", disk.sector_count()),
            ));
        }
        Ok(disk.sector(n as u64).to_vec())
    }

    /// Write bytes at an absolute byte offset, journaled like every other
    /// operation. Throws `OutOfBounds` if the range runs past the disk.
    #[wasm_bindgen(js_name = writeRaw, unchecked_return_type = "OpRecord")]
    pub fn write_raw(&mut self, offset: u32, bytes: &[u8]) -> Result<JsValue, JsValue> {
        let r = self.fs_mut().write_raw(offset as u64, bytes);
        self.op(r)
    }

    /// A copy of `len` bytes starting at an absolute byte offset.
    #[wasm_bindgen(js_name = readRaw)]
    pub fn read_raw(&self, offset: u32, len: u32) -> Result<Vec<u8>, JsValue> {
        let disk = self.fs().disk();
        let disk_len = disk.len() as u64;
        let end = (offset as u64)
            .checked_add(len as u64)
            .filter(|&end| end <= disk_len);
        let Some(end) = end else {
            return Err(js_error(
                "BadArgument",
                &format!(
                    "range {offset}..{} is out of range (disk is {disk_len} bytes)",
                    offset as u64 + len as u64
                ),
            ));
        };
        Ok(disk
            .read(offset as usize, (end - offset as u64) as usize)
            .to_vec())
    }

    /// The `CorruptImage` message a raw write left behind (the on-disk
    /// metadata no longer parses), or `null` while the volume is mounted.
    /// Families without a gate answer `null` always.
    #[wasm_bindgen(unchecked_return_type = "string | null")]
    pub fn corruption(&self) -> Result<JsValue, JsValue> {
        Ok(match self.fs().corruption() {
            Some(err) => JsValue::from_str(&err.to_string()),
            None => JsValue::NULL,
        })
    }

    /// A copy of the whole disk image.
    pub fn image(&self) -> Vec<u8> {
        self.fs().disk().as_bytes().to_vec()
    }
}

/// FAT-specific inspection. Each throws `code === "NotFat"` on a non-FAT volume.
#[wasm_bindgen]
impl Volume {
    #[wasm_bindgen(js_name = bootSector, unchecked_return_type = "BootSector")]
    pub fn boot_sector(&self) -> Result<JsValue, JsValue> {
        to_value(&dto::BootSector::from(self.fat()?.boot_sector()))
    }

    #[wasm_bindgen(unchecked_return_type = "Geometry")]
    pub fn geometry(&self) -> Result<JsValue, JsValue> {
        to_value(&dto::Geometry::from(self.fat()?.geometry()))
    }

    /// Every entry of one FAT copy, indexed by cluster; empty for a copy that does not exist.
    #[wasm_bindgen(js_name = fatEntries, unchecked_return_type = "FatEntry[]")]
    pub fn fat_entries(&self, fat: u8) -> Result<JsValue, JsValue> {
        let list: Vec<dto::FatEntry> = self
            .fat()?
            .fat_entries(fat)
            .into_iter()
            .map(Into::into)
            .collect();
        to_value(&list)
    }

    #[wasm_bindgen(js_name = clusterChain, unchecked_return_type = "number[]")]
    pub fn cluster_chain(&self, start: u32) -> Result<JsValue, JsValue> {
        let chain = self.fat()?.cluster_chain(start).map_err(to_js)?;
        to_value(&chain)
    }

    #[wasm_bindgen(js_name = rawDirEntries, unchecked_return_type = "RawEntry[]")]
    pub fn raw_dir_entries(&self, path: &str) -> Result<JsValue, JsValue> {
        let list: Vec<dto::RawEntry> = self
            .fat()?
            .raw_dir_entries(path)
            .map_err(to_js)?
            .into_iter()
            .map(Into::into)
            .collect();
        to_value(&list)
    }

    /// One full directory-tree walk; reuse the result with `annotateSectorWith`.
    #[wasm_bindgen(js_name = clusterOwners, unchecked_return_type = "ClusterOwner[]")]
    pub fn cluster_owners(&self) -> Result<JsValue, JsValue> {
        to_value(&dto::owners_to_list(&self.fat()?.cluster_owners()))
    }

    #[wasm_bindgen(js_name = annotateSectorWith, unchecked_return_type = "Annotation[]")]
    pub fn annotate_sector_with(
        &self,
        sector: u32,
        #[wasm_bindgen(unchecked_param_type = "ClusterOwner[]")] owners: JsValue,
    ) -> Result<JsValue, JsValue> {
        let owners: Vec<dto::ClusterOwner> = from_value(owners)?;
        let map = dto::owners_from_list(owners);
        let list: Vec<dto::Annotation> = self
            .fat()?
            .annotate_sector_with(sector as u64, &map)
            .into_iter()
            .map(Into::into)
            .collect();
        to_value(&list)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{detect, Detected};

    #[test]
    fn detect_names_fat_by_the_55_aa_signature_in_a_full_first_sector() {
        let mut img = vec![0u8; 512];
        assert_eq!(detect(&img), Detected::Unknown);
        img[510] = 0x55;
        img[511] = 0xAA;
        assert_eq!(detect(&img), Detected::Fat);
        assert_eq!(detect(&img[..511]), Detected::Unknown);
        assert_eq!(detect(&[]), Detected::Unknown);
        img[510] = 0xAA;
        img[511] = 0x55;
        assert_eq!(detect(&img), Detected::Unknown);
        let mut long = vec![0xFFu8; 4096];
        long[510] = 0x55;
        long[511] = 0xAA;
        assert_eq!(detect(&long), Detected::Fat);
        let real = fat::FatFs::format(fat::FormatOptions::default()).unwrap();
        assert_eq!(detect(real.disk().as_bytes()), Detected::Fat);
    }
}
