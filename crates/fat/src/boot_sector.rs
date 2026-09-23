//! The boot sector (BPB + EBPB) and the geometry derived from it.

use fs_core::{Error, Result};
use std::ops::Range;

pub const SIGNATURE: [u8; 2] = [0x55, 0xAA];
/// How many bytes `BootSector::parse` and `decode` read: the BPB, EBPB and
/// signature all lie inside the first 512 bytes whatever the sector size.
pub const BOOT_SECTOR_LEN: usize = 512;
pub const OEM_NAME: [u8; 8] = *b"FAT16EMU";
/// Fewer clusters than this is FAT12 to a real driver.
pub const FAT16_MIN_CLUSTERS: u32 = 4085;
/// More clusters than this is FAT32.
pub const FAT16_MAX_CLUSTERS: u32 = 65524;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormatOptions {
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub total_sectors: u32,
    pub fat_count: u8,
    pub root_entries: u16,
    pub reserved_sectors: u16,
    pub volume_label: [u8; 11],
    pub volume_id: u32,
    /// Reject geometries whose cluster count is not in the FAT16 range.
    pub enforce_fat16_range: bool,
}

impl Default for FormatOptions {
    fn default() -> Self {
        Self {
            bytes_per_sector: 512,
            sectors_per_cluster: 4,
            total_sectors: 32768,
            fat_count: 2,
            root_entries: 512,
            reserved_sectors: 1,
            volume_label: *b"NO NAME    ",
            volume_id: 0x1234_5678,
            enforce_fat16_range: true,
        }
    }
}

/// Which FAT flavour a volume uses. Only FAT16 is implemented; FAT32 will be
/// added here and dispatched on wherever the two differ.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatVariant {
    Fat16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BootSector {
    pub oem_name: [u8; 8],
    pub bytes_per_sector: u16,
    pub sectors_per_cluster: u8,
    pub reserved_sectors: u16,
    pub fat_count: u8,
    pub root_entries: u16,
    pub total_sectors_16: u16,
    pub media: u8,
    pub sectors_per_fat: u16,
    pub sectors_per_track: u16,
    pub heads: u16,
    pub hidden_sectors: u32,
    pub total_sectors_32: u32,
    pub drive_number: u8,
    pub boot_signature: u8,
    pub volume_id: u32,
    pub volume_label: [u8; 11],
    pub fs_type: [u8; 8],
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

fn u32_at(b: &[u8], i: usize) -> u32 {
    u32::from_le_bytes([b[i], b[i + 1], b[i + 2], b[i + 3]])
}

fn root_dir_sectors(root_entries: u64, bytes_per_sector: u64) -> u64 {
    (root_entries * 32).div_ceil(bytes_per_sector)
}

/// Smallest FAT size (in sectors) that has an entry for every cluster plus
/// the two reserved entries. Grows one sector at a time; each extra FAT
/// sector removes clusters, so this converges quickly.
fn sectors_per_fat(opts: &FormatOptions) -> Result<u16> {
    let bps = opts.bytes_per_sector as u64;
    let spc = opts.sectors_per_cluster as u64;
    let total = opts.total_sectors as u64;
    let fixed = opts.reserved_sectors as u64 + root_dir_sectors(opts.root_entries as u64, bps);
    let mut spf: u64 = 1;
    loop {
        let data_sectors = total.saturating_sub(fixed + opts.fat_count as u64 * spf);
        let entries_needed = data_sectors / spc + 2;
        if spf * bps / 2 >= entries_needed {
            return u16::try_from(spf).map_err(|_| {
                Error::InvalidGeometry(format!(
                    "volume needs {spf} sectors per FAT, more than FAT16 allows"
                ))
            });
        }
        spf += 1;
    }
}

impl BootSector {
    pub fn from_options(opts: &FormatOptions) -> Result<BootSector> {
        let geo_err = Error::InvalidGeometry;
        if ![512, 1024, 2048, 4096].contains(&opts.bytes_per_sector) {
            return Err(geo_err(format!(
                "bytes_per_sector {} must be 512, 1024, 2048 or 4096",
                opts.bytes_per_sector
            )));
        }
        if !opts.sectors_per_cluster.is_power_of_two() || opts.sectors_per_cluster > 128 {
            return Err(geo_err(format!(
                "sectors_per_cluster {} must be a power of two up to 128",
                opts.sectors_per_cluster
            )));
        }
        if opts.fat_count == 0 {
            return Err(geo_err("fat_count must be at least 1".into()));
        }
        if opts.root_entries == 0
            || !(opts.root_entries as u32 * 32).is_multiple_of(opts.bytes_per_sector as u32)
        {
            return Err(geo_err(format!(
                "root_entries {} must be non-zero and fill whole sectors",
                opts.root_entries
            )));
        }
        if opts.reserved_sectors == 0 {
            return Err(geo_err("reserved_sectors must be at least 1".into()));
        }
        let spf = sectors_per_fat(opts)?;
        let (total_16, total_32) = if opts.total_sectors < 0x1_0000 {
            (opts.total_sectors as u16, 0)
        } else {
            (0, opts.total_sectors)
        };
        let bs = BootSector {
            oem_name: OEM_NAME,
            bytes_per_sector: opts.bytes_per_sector,
            sectors_per_cluster: opts.sectors_per_cluster,
            reserved_sectors: opts.reserved_sectors,
            fat_count: opts.fat_count,
            root_entries: opts.root_entries,
            total_sectors_16: total_16,
            media: 0xF8,
            sectors_per_fat: spf,
            sectors_per_track: 63,
            heads: 16,
            hidden_sectors: 0,
            total_sectors_32: total_32,
            drive_number: 0x80,
            boot_signature: 0x29,
            volume_id: opts.volume_id,
            volume_label: opts.volume_label,
            fs_type: *b"FAT16   ",
        };
        let geo = bs.geometry()?;
        if opts.enforce_fat16_range
            && !(FAT16_MIN_CLUSTERS..=FAT16_MAX_CLUSTERS).contains(&geo.cluster_count)
        {
            return Err(geo_err(format!(
                "{} clusters is outside the FAT16 range {}..={}; adjust total_sectors or sectors_per_cluster, or set enforce_fat16_range = false",
                geo.cluster_count, FAT16_MIN_CLUSTERS, FAT16_MAX_CLUSTERS
            )));
        }
        Ok(bs)
    }

    /// Read the BPB/EBPB fields of the first `BOOT_SECTOR_LEN` bytes without
    /// validating any of them, so callers can describe whatever is on disk.
    /// Panics if `bytes` is shorter than `BOOT_SECTOR_LEN`.
    pub fn decode(bytes: &[u8]) -> BootSector {
        let mut oem_name = [0u8; 8];
        oem_name.copy_from_slice(&bytes[3..11]);
        let mut volume_label = [0u8; 11];
        volume_label.copy_from_slice(&bytes[43..54]);
        let mut fs_type = [0u8; 8];
        fs_type.copy_from_slice(&bytes[54..62]);
        BootSector {
            oem_name,
            bytes_per_sector: u16_at(bytes, 11),
            sectors_per_cluster: bytes[13],
            reserved_sectors: u16_at(bytes, 14),
            fat_count: bytes[16],
            root_entries: u16_at(bytes, 17),
            total_sectors_16: u16_at(bytes, 19),
            media: bytes[21],
            sectors_per_fat: u16_at(bytes, 22),
            sectors_per_track: u16_at(bytes, 24),
            heads: u16_at(bytes, 26),
            hidden_sectors: u32_at(bytes, 28),
            total_sectors_32: u32_at(bytes, 32),
            drive_number: bytes[36],
            boot_signature: bytes[38],
            volume_id: u32_at(bytes, 39),
            volume_label,
            fs_type,
        }
    }

    /// Parse and validate the first `BOOT_SECTOR_LEN` bytes of a volume.
    pub fn parse(bytes: &[u8]) -> Result<BootSector> {
        if bytes.len() < BOOT_SECTOR_LEN {
            return Err(Error::CorruptImage(
                "boot sector is shorter than 512 bytes".into(),
            ));
        }
        if bytes[510..512] != SIGNATURE {
            return Err(Error::CorruptImage(
                "boot sector signature is not 55 AA".into(),
            ));
        }
        let bs = Self::decode(bytes);
        if ![512, 1024, 2048, 4096].contains(&bs.bytes_per_sector) {
            return Err(Error::CorruptImage(format!(
                "bytes per sector is {}",
                bs.bytes_per_sector
            )));
        }
        if bs.sectors_per_cluster == 0 || !bs.sectors_per_cluster.is_power_of_two() {
            return Err(Error::CorruptImage(format!(
                "sectors per cluster is {}",
                bs.sectors_per_cluster
            )));
        }
        if bs.fat_count == 0 || bs.reserved_sectors == 0 || bs.total_sectors() == 0 {
            return Err(Error::CorruptImage(
                "zero FAT count, reserved sectors, or total sectors".into(),
            ));
        }
        Ok(bs)
    }

    /// Serialize to one full sector (`bytes_per_sector` bytes).
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = vec![0u8; self.bytes_per_sector as usize];
        b[0..3].copy_from_slice(&[0xEB, 0x3C, 0x90]);
        b[3..11].copy_from_slice(&self.oem_name);
        b[11..13].copy_from_slice(&self.bytes_per_sector.to_le_bytes());
        b[13] = self.sectors_per_cluster;
        b[14..16].copy_from_slice(&self.reserved_sectors.to_le_bytes());
        b[16] = self.fat_count;
        b[17..19].copy_from_slice(&self.root_entries.to_le_bytes());
        b[19..21].copy_from_slice(&self.total_sectors_16.to_le_bytes());
        b[21] = self.media;
        b[22..24].copy_from_slice(&self.sectors_per_fat.to_le_bytes());
        b[24..26].copy_from_slice(&self.sectors_per_track.to_le_bytes());
        b[26..28].copy_from_slice(&self.heads.to_le_bytes());
        b[28..32].copy_from_slice(&self.hidden_sectors.to_le_bytes());
        b[32..36].copy_from_slice(&self.total_sectors_32.to_le_bytes());
        b[36] = self.drive_number;
        b[37] = 0;
        b[38] = self.boot_signature;
        b[39..43].copy_from_slice(&self.volume_id.to_le_bytes());
        b[43..54].copy_from_slice(&self.volume_label);
        b[54..62].copy_from_slice(&self.fs_type);
        b[510..512].copy_from_slice(&SIGNATURE);
        b
    }

    pub fn total_sectors(&self) -> u32 {
        if self.total_sectors_16 != 0 {
            self.total_sectors_16 as u32
        } else {
            self.total_sectors_32
        }
    }

    pub fn geometry(&self) -> Result<Geometry> {
        if self.sectors_per_fat == 0 {
            return Err(Error::Unsupported(
                "FAT32 volumes (sectors_per_fat = 0) are not supported yet".into(),
            ));
        }
        let bps = self.bytes_per_sector as u64;
        let root_dir_sectors = root_dir_sectors(self.root_entries as u64, bps);
        let first_root_dir_sector =
            self.reserved_sectors as u64 + self.fat_count as u64 * self.sectors_per_fat as u64;
        let first_data_sector = first_root_dir_sector + root_dir_sectors;
        let total_sectors = self.total_sectors() as u64;
        if usize::try_from(total_sectors * bps).is_err() {
            return Err(Error::InvalidGeometry(format!(
                "volume of {total_sectors} sectors x {bps} bytes is too large for this platform"
            )));
        }
        if first_data_sector >= total_sectors {
            return Err(Error::InvalidGeometry(format!(
                "metadata needs {first_data_sector} sectors but the volume has only {total_sectors}"
            )));
        }
        let cluster_count =
            ((total_sectors - first_data_sector) / self.sectors_per_cluster as u64) as u32;
        if cluster_count == 0 {
            return Err(Error::InvalidGeometry("volume has no data clusters".into()));
        }
        if cluster_count > FAT16_MAX_CLUSTERS {
            return Err(Error::Unsupported(format!(
                "{cluster_count} clusters means FAT32, which is not supported yet"
            )));
        }
        if cluster_count < FAT16_MIN_CLUSTERS && &self.fs_type != b"FAT16   " {
            return Err(Error::Unsupported(format!(
                "{cluster_count} clusters means FAT12, which is not supported"
            )));
        }
        let fat_bytes = self.sectors_per_fat as u64 * bps;
        if fat_bytes < (cluster_count as u64 + 2) * 2 {
            return Err(Error::CorruptImage(format!(
                "FAT of {fat_bytes} bytes cannot hold {} entries",
                cluster_count + 2
            )));
        }
        Ok(Geometry {
            variant: FatVariant::Fat16,
            bytes_per_sector: bps as usize,
            sectors_per_cluster: self.sectors_per_cluster as usize,
            reserved_sectors: self.reserved_sectors as u64,
            fat_count: self.fat_count,
            sectors_per_fat: self.sectors_per_fat as u64,
            root_entries: self.root_entries as usize,
            root_dir_sectors,
            first_root_dir_sector,
            first_data_sector,
            total_sectors,
            cluster_count,
        })
    }
}

/// Everything derived from the BPB that the other layers need to find bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Geometry {
    pub variant: FatVariant,
    pub bytes_per_sector: usize,
    pub sectors_per_cluster: usize,
    pub reserved_sectors: u64,
    pub fat_count: u8,
    pub sectors_per_fat: u64,
    pub root_entries: usize,
    pub root_dir_sectors: u64,
    pub first_root_dir_sector: u64,
    pub first_data_sector: u64,
    pub total_sectors: u64,
    /// Number of data clusters. Valid cluster numbers are `2..=cluster_count + 1`.
    pub cluster_count: u32,
}

impl Geometry {
    pub fn cluster_size(&self) -> usize {
        self.bytes_per_sector * self.sectors_per_cluster
    }

    pub fn fat_offset(&self, fat: u8) -> usize {
        (self.reserved_sectors + fat as u64 * self.sectors_per_fat) as usize * self.bytes_per_sector
    }

    pub fn fat_len(&self) -> usize {
        self.sectors_per_fat as usize * self.bytes_per_sector
    }

    pub fn fat_entry_offset(&self, fat: u8, cluster: u32) -> usize {
        match self.variant {
            FatVariant::Fat16 => self.fat_offset(fat) + cluster as usize * 2,
        }
    }

    pub fn root_dir_offset(&self) -> usize {
        self.first_root_dir_sector as usize * self.bytes_per_sector
    }

    pub fn root_dir_len(&self) -> usize {
        self.root_entries * 32
    }

    /// Absolute byte offset of a data cluster. Panics for clusters below 2.
    pub fn cluster_offset(&self, cluster: u32) -> usize {
        assert!(cluster >= 2, "cluster {cluster} has no data");
        self.first_data_sector as usize * self.bytes_per_sector
            + (cluster as usize - 2) * self.cluster_size()
    }

    pub fn cluster_range(&self, cluster: u32) -> Range<usize> {
        let start = self.cluster_offset(cluster);
        start..start + self.cluster_size()
    }

    pub fn max_cluster(&self) -> u32 {
        self.cluster_count + 1
    }

    pub fn is_valid_cluster(&self, cluster: u32) -> bool {
        (2..=self.max_cluster()).contains(&cluster)
    }

    /// Which cluster a data sector belongs to, if it is inside a cluster.
    pub fn cluster_of_sector(&self, sector: u64) -> Option<u32> {
        if sector < self.first_data_sector {
            return None;
        }
        let cluster =
            2 + ((sector - self.first_data_sector) / self.sectors_per_cluster as u64) as u32;
        self.is_valid_cluster(cluster).then_some(cluster)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fs_core::Error;

    #[test]
    fn defaults_produce_a_fat16_geometry() {
        let bs = BootSector::from_options(&FormatOptions::default()).unwrap();
        assert_eq!(bs.sectors_per_fat, 32);
        assert_eq!(bs.total_sectors_16, 32768);
        assert_eq!(bs.total_sectors_32, 0);
        assert_eq!(&bs.fs_type, b"FAT16   ");
        let g = bs.geometry().unwrap();
        assert_eq!(g.variant, FatVariant::Fat16);
        assert_eq!(g.cluster_count, 8167);
        assert_eq!(g.root_dir_sectors, 32);
        assert_eq!(g.first_root_dir_sector, 65);
        assert_eq!(g.first_data_sector, 97);
        assert_eq!(g.cluster_size(), 2048);
        assert_eq!(g.fat_offset(0), 512);
        assert_eq!(g.fat_offset(1), 512 + 32 * 512);
        assert_eq!(g.fat_entry_offset(1, 5), 512 + 32 * 512 + 10);
        assert_eq!(g.root_dir_offset(), 65 * 512);
        assert_eq!(g.root_dir_len(), 512 * 32);
        assert_eq!(g.cluster_offset(2), 97 * 512);
        assert_eq!(g.cluster_offset(3), 97 * 512 + 2048);
        assert_eq!(g.max_cluster(), 8168);
        assert!(g.is_valid_cluster(2) && g.is_valid_cluster(8168));
        assert!(!g.is_valid_cluster(1) && !g.is_valid_cluster(8169));
        assert_eq!(g.cluster_of_sector(97), Some(2));
        assert_eq!(g.cluster_of_sector(101), Some(3));
        assert_eq!(g.cluster_of_sector(96), None);
    }

    #[test]
    fn to_bytes_and_parse_round_trip() {
        let bs = BootSector::from_options(&FormatOptions::default()).unwrap();
        let bytes = bs.to_bytes();
        assert_eq!(bytes.len(), 512);
        assert_eq!(&bytes[0..3], &[0xEB, 0x3C, 0x90]);
        assert_eq!(&bytes[3..11], b"FAT16EMU");
        assert_eq!(&bytes[510..512], &[0x55, 0xAA]);
        assert_eq!(BootSector::parse(&bytes).unwrap(), bs);
    }

    #[test]
    fn decode_reads_fields_without_validating() {
        let zeros = BootSector::decode(&[0u8; BOOT_SECTOR_LEN]);
        assert_eq!(zeros.bytes_per_sector, 0);
        assert_eq!(zeros.sectors_per_cluster, 0);
        assert_eq!(zeros.volume_label, [0u8; 11]);
        let bs = BootSector::from_options(&FormatOptions::default()).unwrap();
        let bytes = bs.to_bytes();
        assert_eq!(BootSector::decode(&bytes), bs);
        assert_eq!(
            BootSector::decode(&bytes),
            BootSector::parse(&bytes).unwrap()
        );
        let mut unsigned = bytes.clone();
        unsigned[510] = 0;
        assert_eq!(BootSector::decode(&unsigned), bs);
        assert!(BootSector::parse(&unsigned).is_err());
    }

    #[test]
    fn parse_rejects_bad_signature() {
        let mut bytes = BootSector::from_options(&FormatOptions::default())
            .unwrap()
            .to_bytes();
        bytes[511] = 0;
        assert!(matches!(
            BootSector::parse(&bytes),
            Err(Error::CorruptImage(_))
        ));
    }

    #[test]
    fn small_volume_is_rejected_unless_range_check_is_off() {
        let opts = FormatOptions {
            total_sectors: 2048,
            ..Default::default()
        };
        assert!(matches!(
            BootSector::from_options(&opts),
            Err(Error::InvalidGeometry(_))
        ));
        let opts = FormatOptions {
            enforce_fat16_range: false,
            ..opts
        };
        let g = BootSector::from_options(&opts).unwrap().geometry().unwrap();
        assert!(g.cluster_count < FAT16_MIN_CLUSTERS);
        assert_eq!(g.variant, FatVariant::Fat16);
    }

    #[test]
    fn geometry_refuses_fat12_and_fat32_images() {
        let mut bs = BootSector::from_options(&FormatOptions {
            total_sectors: 2048,
            enforce_fat16_range: false,
            ..Default::default()
        })
        .unwrap();
        bs.fs_type = *b"FAT12   ";
        assert!(matches!(bs.geometry(), Err(Error::Unsupported(_))));
        let mut bs = BootSector::from_options(&FormatOptions::default()).unwrap();
        bs.sectors_per_fat = 0;
        assert!(matches!(bs.geometry(), Err(Error::Unsupported(_))));
    }

    #[test]
    fn invalid_options_are_rejected() {
        let bad = |o: FormatOptions| {
            matches!(BootSector::from_options(&o), Err(Error::InvalidGeometry(_)))
        };
        assert!(bad(FormatOptions {
            bytes_per_sector: 500,
            ..Default::default()
        }));
        assert!(bad(FormatOptions {
            sectors_per_cluster: 3,
            ..Default::default()
        }));
        assert!(bad(FormatOptions {
            fat_count: 0,
            ..Default::default()
        }));
        assert!(bad(FormatOptions {
            root_entries: 0,
            ..Default::default()
        }));
        assert!(bad(FormatOptions {
            reserved_sectors: 0,
            ..Default::default()
        }));
    }

    #[test]
    fn huge_total_sectors_exceeds_fat_size() {
        let opts = FormatOptions {
            total_sectors: u32::MAX,
            sectors_per_cluster: 1,
            enforce_fat16_range: false,
            ..Default::default()
        };
        assert!(matches!(
            BootSector::from_options(&opts),
            Err(Error::InvalidGeometry(_))
        ));
    }

    #[test]
    fn geometry_rejects_a_fat_too_small_for_the_cluster_count() {
        let mut bs = BootSector::from_options(&FormatOptions::default()).unwrap();
        bs.sectors_per_fat = 1;
        assert!(matches!(bs.geometry(), Err(Error::CorruptImage(_))));
    }

    #[test]
    fn oversized_volumes_are_rejected() {
        let mut bs = BootSector::from_options(&FormatOptions::default()).unwrap();
        bs.total_sectors_16 = 0;
        bs.total_sectors_32 = u32::MAX;
        assert!(matches!(
            bs.geometry(),
            Err(Error::InvalidGeometry(_)) | Err(Error::Unsupported(_))
        ));
    }
}
