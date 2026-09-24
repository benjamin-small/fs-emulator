//! The one class a UI talks to. Generic operations go through the
//! `FileSystem` trait; family-specific inspection is gated on the inner variant.

use crate::dto;
use crate::error::{js_error, to_js, BAD_ARGUMENT, NOT_EXT, NOT_FAT};
use ext::ExtFs;
use fat::FatFs;
use fs_core::FileSystem;
use js_sys::Object;
use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;

enum Inner {
    Fat(Box<FatFs>),
    Ext(Box<ExtFs>),
}

/// The family an image's signature names; `from_image` picks the parser from it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Detected {
    Ext,
    Fat,
    Unknown,
}

/// Byte offset of `s_magic` (superblock offset 56) in an ext image: 1080.
const EXT_MAGIC_OFFSET: usize = ext::SUPERBLOCK_OFFSET + 56;

/// Detection rule, in order: ext when the image reaches byte 1082 and the
/// `u16le` at 1080 is `0xEF53`; then FAT when the image is at least one
/// 512-byte sector long and bytes 510..512 are `55 AA`. ext goes first
/// because a bootable ext image can also carry `55 AA` at 510. Whether the
/// metadata behind the signature parses is the family parser's job, not
/// this function's.
fn detect(bytes: &[u8]) -> Detected {
    let ext_magic = bytes
        .get(EXT_MAGIC_OFFSET..EXT_MAGIC_OFFSET + 2)
        .map(|m| u16::from_le_bytes([m[0], m[1]]));
    if ext_magic == Some(ext::EXT2_MAGIC) {
        Detected::Ext
    } else if has_fat_signature(bytes) {
        Detected::Fat
    } else {
        Detected::Unknown
    }
}

/// Whether bytes 510..512 are FAT's `55 AA` boot signature.
fn has_fat_signature(bytes: &[u8]) -> bool {
    bytes.len() >= 512 && bytes[510..512] == [0x55, 0xAA]
}

/// Parse `bytes` as the family `detect` names. An image that carries the
/// ext magic but does not parse as ext, and also carries `55 AA` at 510,
/// is handed to the FAT parser, whose result (error included) is returned:
/// a FAT volume can hold `53 EF` at 1080 in its FAT or root directory.
fn load(bytes: Vec<u8>) -> fs_core::Result<Inner> {
    match detect(&bytes) {
        Detected::Ext if has_fat_signature(&bytes) => match ExtFs::from_image(bytes.clone()) {
            Ok(fs) => Ok(Inner::Ext(Box::new(fs))),
            Err(_) => FatFs::from_image(bytes).map(|fs| Inner::Fat(Box::new(fs))),
        },
        Detected::Ext => ExtFs::from_image(bytes).map(|fs| Inner::Ext(Box::new(fs))),
        Detected::Fat => FatFs::from_image(bytes).map(|fs| Inner::Fat(Box::new(fs))),
        Detected::Unknown => Err(fs_core::Error::Unsupported(
            "no recognisable filesystem signature".into(),
        )),
    }
}

/// Largest volume the wrapper will allocate in browser memory.
pub const MAX_VOLUME_BYTES: u64 = 256 * 1024 * 1024;

/// `BadArgument` when a volume of `bytes` would pass `MAX_VOLUME_BYTES`.
fn check_volume_size(bytes: u64) -> Result<(), JsValue> {
    if bytes > MAX_VOLUME_BYTES {
        return Err(js_error(
            BAD_ARGUMENT,
            &format!("volume of {bytes} bytes exceeds the {MAX_VOLUME_BYTES}-byte limit"),
        ));
    }
    Ok(())
}

/// The ext volume `formatExt2` and `formatExt3` return: the size check,
/// then `ExtFs::format` with the converted options.
fn ext_volume(opts: ext::ExtFormatOptions) -> Result<Volume, JsValue> {
    check_volume_size(u64::from(opts.total_blocks) * u64::from(ext::BLOCK_SIZE))?;
    let fs = ExtFs::format(opts).map_err(to_js)?;
    Ok(Volume {
        inner: Inner::Ext(Box::new(fs)),
    })
}

#[wasm_bindgen]
pub struct Volume {
    inner: Inner,
}

// `Result::unwrap_err` requires the `Ok` type to implement `Debug`. Neither
// filesystem type nor `fs_core::Disk` derives it, so this is written by hand
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
        .map_err(|e| js_error(BAD_ARGUMENT, &e.to_string()))
}

fn from_value<T: serde::de::DeserializeOwned>(value: JsValue) -> Result<T, JsValue> {
    serde_wasm_bindgen::from_value(value).map_err(|e| js_error(BAD_ARGUMENT, &e.to_string()))
}

/// Reject any own key of `value` that is not in `allowed`. `#[serde(deny_unknown_fields)]`
/// can't do this itself at the JS boundary: `serde-wasm-bindgen` deserializes a struct by
/// looking up each known field by name on the object, never by iterating the object's own
/// keys, so a typo'd key is simply never observed rather than rejected.
fn reject_unknown_keys(value: &JsValue, allowed: &[&str]) -> Result<(), JsValue> {
    for key in Object::keys(value.unchecked_ref::<Object>()).iter() {
        let key = key.as_string().unwrap_or_default();
        if !allowed.contains(&key.as_str()) {
            return Err(js_error(BAD_ARGUMENT, &format!("unknown option \"{key}\"")));
        }
    }
    Ok(())
}

impl Volume {
    fn fs(&self) -> &dyn FileSystem {
        match &self.inner {
            Inner::Fat(f) => f.as_ref(),
            Inner::Ext(e) => e.as_ref(),
        }
    }

    fn fs_mut(&mut self) -> &mut dyn FileSystem {
        match &mut self.inner {
            Inner::Fat(f) => f.as_mut(),
            Inner::Ext(e) => e.as_mut(),
        }
    }

    /// The FAT volume, or `NotFat` on any other family.
    fn fat(&self) -> Result<&FatFs, JsValue> {
        match &self.inner {
            Inner::Fat(f) => Ok(f.as_ref()),
            Inner::Ext(_) => Err(js_error(NOT_FAT, "not a FAT volume")),
        }
    }

    /// The ext volume, or `NotExt` on any other family.
    fn ext(&self) -> Result<&ExtFs, JsValue> {
        match &self.inner {
            Inner::Ext(e) => Ok(e),
            Inner::Fat(_) => Err(js_error(NOT_EXT, "not an ext volume")),
        }
    }

    /// The ext volume for a mutation, or `NotExt` on any other family.
    fn ext_mut(&mut self) -> Result<&mut ExtFs, JsValue> {
        match &mut self.inner {
            Inner::Ext(e) => Ok(e),
            Inner::Fat(_) => Err(js_error(NOT_EXT, "not an ext volume")),
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
        check_volume_size(opts.total_sectors as u64 * opts.bytes_per_sector as u64)?;
        let fs = FatFs::format(opts).map_err(to_js)?;
        Ok(Volume {
            inner: Inner::Fat(Box::new(fs)),
        })
    }

    /// Format a fresh ext2 volume with 1 KiB blocks. `options` may be `undefined`.
    #[wasm_bindgen(js_name = formatExt2)]
    pub fn format_ext2(
        #[wasm_bindgen(unchecked_param_type = "ExtFormatOptions | undefined")] options: JsValue,
    ) -> Result<Volume, JsValue> {
        let opts: dto::ExtFormatOptions = if options.is_undefined() || options.is_null() {
            dto::ExtFormatOptions::default()
        } else {
            reject_unknown_keys(&options, dto::ExtFormatOptions::FIELDS)?;
            from_value(options)?
        };
        let opts =
            ext::ExtFormatOptions::try_from(opts).map_err(|msg| js_error(BAD_ARGUMENT, &msg))?;
        ext_volume(opts)
    }

    /// Format a fresh ext3 volume: the ext2 layout plus a journal on inode 8.
    /// `options` may be `undefined`.
    #[wasm_bindgen(js_name = formatExt3)]
    pub fn format_ext3(
        #[wasm_bindgen(unchecked_param_type = "Ext3FormatOptions | undefined")] options: JsValue,
    ) -> Result<Volume, JsValue> {
        let opts: dto::Ext3FormatOptions = if options.is_undefined() || options.is_null() {
            dto::Ext3FormatOptions::default()
        } else {
            reject_unknown_keys(&options, dto::Ext3FormatOptions::FIELDS)?;
            from_value(options)?
        };
        let opts =
            ext::ExtFormatOptions::try_from(opts).map_err(|msg| js_error(BAD_ARGUMENT, &msg))?;
        ext_volume(opts)
    }

    /// Load an image of any family `detect` recognises; `Unsupported` otherwise.
    #[wasm_bindgen(js_name = fromImage)]
    pub fn from_image(bytes: Vec<u8>) -> Result<Volume, JsValue> {
        check_volume_size(bytes.len() as u64)?;
        let inner = load(bytes).map_err(to_js)?;
        Ok(Volume { inner })
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
                BAD_ARGUMENT,
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
                BAD_ARGUMENT,
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
                BAD_ARGUMENT,
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

/// ext-specific methods. Each throws `code === "NotExt"` on a non-ext volume.
/// The superblock, inode, and block-ownership DTOs arrive with the explorer's
/// ext panels; for now there are the group count (slice 2) and the ext3
/// journal's crash, recovery, and inspection methods (slice 3, section 8).
/// The journal methods answer on ext2 too: no journal, no armed crash,
/// nothing to recover.
#[wasm_bindgen]
impl Volume {
    /// How many block groups the volume has (2 on the default 16 MiB disk).
    #[wasm_bindgen(js_name = blockGroupCount)]
    pub fn block_group_count(&self) -> Result<u32, JsValue> {
        Ok(self.ext()?.geometry().groups)
    }

    /// Arm a crash for the next journaled mutation: `"before_commit"`,
    /// `"after_commit"`, or `"during_checkpoint"` (`BadArgument` otherwise).
    /// Throws `Unsupported` on ext2, which has no journal.
    #[wasm_bindgen(js_name = armCrash)]
    pub fn arm_crash(&mut self, phase: &str) -> Result<(), JsValue> {
        let fs = self.ext_mut()?;
        let parsed = ext::CrashPhase::parse(phase).ok_or_else(|| {
            js_error(
                BAD_ARGUMENT,
                &format!(
                    "crash phase \"{phase}\" is not \"before_commit\", \"after_commit\", or \"during_checkpoint\""
                ),
            )
        })?;
        fs.arm_crash(parsed).map_err(to_js)
    }

    /// Clear an armed crash; a no-op when none is armed.
    #[wasm_bindgen(js_name = disarmCrash)]
    pub fn disarm_crash(&mut self) -> Result<(), JsValue> {
        self.ext_mut()?.disarm_crash();
        Ok(())
    }

    /// The armed phase, as `armCrash` takes it, or `undefined`.
    #[wasm_bindgen(js_name = crashPhase)]
    pub fn crash_phase(&self) -> Result<Option<String>, JsValue> {
        Ok(self.ext()?.crash_phase().map(|p| p.as_str().to_string()))
    }

    /// Whether a crash left a transaction for `recover()`; path mutations
    /// throw `NeedsRecovery` until then. Always `false` on ext2.
    #[wasm_bindgen(js_name = needsRecovery)]
    pub fn needs_recovery(&self) -> Result<bool, JsValue> {
        Ok(self.ext()?.needs_recovery())
    }

    /// Replay or discard what the journal holds, as a mount does, recorded
    /// as the operation `recover`. On a clean journal (and on ext2) the
    /// record has no changes and one `recovery_scanned` event.
    #[wasm_bindgen(unchecked_return_type = "OpRecord")]
    pub fn recover(&mut self) -> Result<JsValue, JsValue> {
        let r = self.ext_mut()?.recover();
        self.op(r)
    }

    /// The journal's shape and state, or `undefined` on ext2.
    #[wasm_bindgen(js_name = journalInfo, unchecked_return_type = "JournalInfo | undefined")]
    pub fn journal_info(&self) -> Result<JsValue, JsValue> {
        match self.ext()?.journal_info() {
            Some(info) => to_value(&dto::JournalInfo::from(info)),
            None => Ok(JsValue::UNDEFINED),
        }
    }

    /// Every journal block in index order with what it holds; empty on ext2.
    #[wasm_bindgen(js_name = journalBlocks, unchecked_return_type = "JournalBlock[]")]
    pub fn journal_blocks(&self) -> Result<JsValue, JsValue> {
        let list: Vec<dto::JournalBlock> = self
            .ext()?
            .journal_blocks()
            .into_iter()
            .map(Into::into)
            .collect();
        to_value(&list)
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::{detect, load, Detected, Inner, EXT_MAGIC_OFFSET};

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

    #[test]
    fn detect_names_ext_by_the_superblock_magic_before_fat() {
        assert_eq!(EXT_MAGIC_OFFSET, 1080);
        let mut img = vec![0u8; 2048];
        assert_eq!(detect(&img), Detected::Unknown);
        img[1080] = 0x53;
        img[1081] = 0xEF;
        assert_eq!(detect(&img), Detected::Ext);
        assert_eq!(detect(&img[..1082]), Detected::Ext);
        assert_eq!(detect(&img[..1081]), Detected::Unknown);
        // A boot signature does not outrank the magic: ext is checked first.
        img[510] = 0x55;
        img[511] = 0xAA;
        assert_eq!(detect(&img), Detected::Ext);
        // Too short to hold the magic, so FAT's rule decides.
        assert_eq!(detect(&img[..1081]), Detected::Fat);
        // The magic is little-endian; the byte-swapped value is not ext.
        img[1080] = 0xEF;
        img[1081] = 0x53;
        assert_eq!(detect(&img), Detected::Fat);
        let real = ext::ExtFs::format(ext::ExtFormatOptions::default()).unwrap();
        assert_eq!(
            detect(fs_core::FileSystem::disk(&real).as_bytes()),
            Detected::Ext
        );
    }

    #[test]
    fn a_fat_image_carrying_the_ext_magic_falls_back_to_fat() {
        let fat = fat::FatFs::format(fat::FormatOptions::default()).unwrap();
        let mut bytes = fat.disk().as_bytes().to_vec();
        // Offset 1080 sits in the first FAT (sectors 1..), in entry 284.
        bytes[1080] = 0x53;
        bytes[1081] = 0xEF;
        assert_eq!(detect(&bytes), Detected::Ext);
        match load(bytes) {
            Ok(Inner::Fat(fs)) => assert_eq!(fs.fs_type(), "FAT16"),
            Ok(Inner::Ext(_)) => panic!("loaded as ext"),
            Err(e) => panic!("did not load: {e}"),
        }
    }

    #[test]
    fn an_ext_magic_without_the_fat_signature_keeps_the_ext_error() {
        let mut img = vec![0u8; 4096];
        img[1080] = 0x53;
        img[1081] = 0xEF;
        assert!(matches!(
            load(img.clone()),
            Err(fs_core::Error::Unsupported(_))
        ));
        // With `55 AA` too, the FAT parser's own error comes back instead.
        img[510] = 0x55;
        img[511] = 0xAA;
        let fat_error = fat::FatFs::from_image(img.clone()).err().unwrap();
        assert_eq!(load(img).err(), Some(fat_error));
        // A real ext image with `55 AA` still loads as ext.
        let ext = ext::ExtFs::format(ext::ExtFormatOptions::default()).unwrap();
        let mut bytes = fs_core::FileSystem::disk(&ext).as_bytes().to_vec();
        bytes[510] = 0x55;
        bytes[511] = 0xAA;
        assert!(matches!(load(bytes), Ok(Inner::Ext(_))));
    }

    /// Writes a formatted ext2 image holding `/hello.txt` to the path in
    /// `FS_EMULATOR_EXT2_IMAGE` when it is set, for loading into the explorer
    /// by hand; without it the test only checks that the image detects as ext.
    #[test]
    fn writes_an_ext2_image_for_the_explorer_when_asked() {
        let mut fs = ext::ExtFs::format(ext::ExtFormatOptions::default()).unwrap();
        fs_core::FileSystem::create_file(&mut fs, "/hello.txt", b"hello from ext2\n").unwrap();
        let bytes = fs_core::FileSystem::disk(&fs).as_bytes().to_vec();
        assert_eq!(detect(&bytes), Detected::Ext);
        if let Some(path) = std::env::var_os("FS_EMULATOR_EXT2_IMAGE") {
            std::fs::write(&path, &bytes).unwrap();
        }
    }
}
