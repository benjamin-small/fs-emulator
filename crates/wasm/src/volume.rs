//! The one class a UI talks to. Generic operations go through the
//! `FileSystem` trait; FAT-specific inspection is gated on the inner variant.

use crate::dto;
use crate::error::{js_error, to_js};
use fat::FatFs;
use fs_core::FileSystem;
use serde::Serialize;
use serde_wasm_bindgen::Serializer;
use wasm_bindgen::prelude::*;

enum Inner {
    Fat(FatFs),
}

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

    /// The FAT volume, or a `NotFat` error once other filesystems exist.
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
            from_value(options)?
        };
        let fs = FatFs::format(opts.into()).map_err(to_js)?;
        Ok(Volume {
            inner: Inner::Fat(fs),
        })
    }

    #[wasm_bindgen(js_name = fromImage)]
    pub fn from_image(bytes: &[u8]) -> Result<Volume, JsValue> {
        let fs = FatFs::from_image(bytes.to_vec()).map_err(to_js)?;
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
