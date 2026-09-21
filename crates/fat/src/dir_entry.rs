//! The 32-byte directory entry in its two forms: the classic short (8.3)
//! entry and the VFAT long-file-name entry that precedes it.

use fs_core::DateTime;

pub const ENTRY_SIZE: usize = 32;
/// First byte of a never-used slot; also terminates directory scans.
pub const FREE: u8 = 0x00;
/// First byte of a deleted slot.
pub const DELETED: u8 = 0xE5;

pub mod attr {
    pub const READ_ONLY: u8 = 0x01;
    pub const HIDDEN: u8 = 0x02;
    pub const SYSTEM: u8 = 0x04;
    pub const VOLUME_ID: u8 = 0x08;
    pub const DIRECTORY: u8 = 0x10;
    pub const ARCHIVE: u8 = 0x20;
    /// READ_ONLY | HIDDEN | SYSTEM | VOLUME_ID marks a long-name entry.
    pub const LFN: u8 = 0x0F;

    pub fn is_lfn(attributes: u8) -> bool {
        attributes & 0x3F == LFN
    }
}

/// FAT date: bits 15..9 year since 1980, 8..5 month, 4..0 day.
pub fn pack_date(dt: &DateTime) -> u16 {
    let year = dt.year.saturating_sub(1980).min(127);
    (year << 9) | ((dt.month as u16 & 0x0F) << 5) | (dt.day as u16 & 0x1F)
}

/// FAT time: bits 15..11 hour, 10..5 minute, 4..0 seconds / 2.
pub fn pack_time(dt: &DateTime) -> u16 {
    ((dt.hour as u16 & 0x1F) << 11)
        | ((dt.minute as u16 & 0x3F) << 5)
        | ((dt.second as u16 / 2) & 0x1F)
}

/// `None` when the date is zero, which FAT uses for "not set".
pub fn unpack(date: u16, time: u16) -> Option<DateTime> {
    if date == 0 {
        return None;
    }
    Some(DateTime::new(
        1980 + (date >> 9),
        ((date >> 5) & 0x0F) as u8,
        (date & 0x1F) as u8,
        (time >> 11) as u8,
        ((time >> 5) & 0x3F) as u8,
        ((time & 0x1F) * 2) as u8,
    ))
}

/// The checksum of a short name that every LFN entry for it carries.
pub fn lfn_checksum(name: &[u8; 11]) -> u8 {
    name.iter().fold(0u8, |sum, &b| {
        ((sum & 1) << 7).wrapping_add(sum >> 1).wrapping_add(b)
    })
}

fn u16_at(b: &[u8], i: usize) -> u16 {
    u16::from_le_bytes([b[i], b[i + 1]])
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortEntry {
    pub name: [u8; 11],
    pub attr: u8,
    pub nt_reserved: u8,
    pub create_time_tenths: u8,
    pub create_time: u16,
    pub create_date: u16,
    pub access_date: u16,
    pub first_cluster_hi: u16,
    pub write_time: u16,
    pub write_date: u16,
    pub first_cluster_lo: u16,
    pub size: u32,
}

impl ShortEntry {
    /// A fresh entry with all three timestamps set from `now`, no cluster, size 0.
    pub fn new(name: [u8; 11], attributes: u8, now: &DateTime) -> ShortEntry {
        ShortEntry {
            name,
            attr: attributes,
            nt_reserved: 0,
            create_time_tenths: 0,
            create_time: pack_time(now),
            create_date: pack_date(now),
            access_date: pack_date(now),
            first_cluster_hi: 0,
            write_time: pack_time(now),
            write_date: pack_date(now),
            first_cluster_lo: 0,
            size: 0,
        }
    }

    pub fn parse(b: &[u8]) -> ShortEntry {
        let mut name = [0u8; 11];
        name.copy_from_slice(&b[0..11]);
        ShortEntry {
            name,
            attr: b[11],
            nt_reserved: b[12],
            create_time_tenths: b[13],
            create_time: u16_at(b, 14),
            create_date: u16_at(b, 16),
            access_date: u16_at(b, 18),
            first_cluster_hi: u16_at(b, 20),
            write_time: u16_at(b, 22),
            write_date: u16_at(b, 24),
            first_cluster_lo: u16_at(b, 26),
            size: u32::from_le_bytes([b[28], b[29], b[30], b[31]]),
        }
    }

    pub fn to_bytes(&self) -> [u8; ENTRY_SIZE] {
        let mut b = [0u8; ENTRY_SIZE];
        b[0..11].copy_from_slice(&self.name);
        b[11] = self.attr;
        b[12] = self.nt_reserved;
        b[13] = self.create_time_tenths;
        b[14..16].copy_from_slice(&self.create_time.to_le_bytes());
        b[16..18].copy_from_slice(&self.create_date.to_le_bytes());
        b[18..20].copy_from_slice(&self.access_date.to_le_bytes());
        b[20..22].copy_from_slice(&self.first_cluster_hi.to_le_bytes());
        b[22..24].copy_from_slice(&self.write_time.to_le_bytes());
        b[24..26].copy_from_slice(&self.write_date.to_le_bytes());
        b[26..28].copy_from_slice(&self.first_cluster_lo.to_le_bytes());
        b[28..32].copy_from_slice(&self.size.to_le_bytes());
        b
    }

    /// The high half is always zero on FAT16 but is kept for FAT32.
    pub fn first_cluster(&self) -> u32 {
        ((self.first_cluster_hi as u32) << 16) | self.first_cluster_lo as u32
    }

    pub fn set_first_cluster(&mut self, cluster: u32) {
        self.first_cluster_hi = (cluster >> 16) as u16;
        self.first_cluster_lo = cluster as u16;
    }

    pub fn is_dir(&self) -> bool {
        self.attr & attr::DIRECTORY != 0
    }

    pub fn is_volume_label(&self) -> bool {
        self.attr & attr::VOLUME_ID != 0 && !attr::is_lfn(self.attr)
    }

    pub fn is_dot_entry(&self) -> bool {
        self.name == *b".          " || self.name == *b"..         "
    }

    /// `NAME.EXT`, or `NAME` when the extension is blank.
    pub fn display_name(&self) -> String {
        let base = String::from_utf8_lossy(&self.name[..8])
            .trim_end()
            .to_string();
        let ext = String::from_utf8_lossy(&self.name[8..])
            .trim_end()
            .to_string();
        if ext.is_empty() {
            base
        } else {
            format!("{base}.{ext}")
        }
    }
}

/// One VFAT long-name entry holding 13 UCS-2 characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LfnEntry {
    /// Order (1-based, bits 0..5) with bit 6 set on the last (first on disk) entry.
    pub sequence: u8,
    pub name1: [u16; 5],
    pub attr: u8,
    pub entry_type: u8,
    pub checksum: u8,
    pub name2: [u16; 6],
    pub first_cluster_lo: u16,
    pub name3: [u16; 2],
}

pub const LAST_LFN_FLAG: u8 = 0x40;

impl LfnEntry {
    pub fn new(order: u8, last: bool, checksum: u8, chars: &[u16; 13]) -> LfnEntry {
        let mut name1 = [0u16; 5];
        name1.copy_from_slice(&chars[0..5]);
        let mut name2 = [0u16; 6];
        name2.copy_from_slice(&chars[5..11]);
        let mut name3 = [0u16; 2];
        name3.copy_from_slice(&chars[11..13]);
        LfnEntry {
            sequence: order | if last { LAST_LFN_FLAG } else { 0 },
            name1,
            attr: attr::LFN,
            entry_type: 0,
            checksum,
            name2,
            first_cluster_lo: 0,
            name3,
        }
    }

    pub fn parse(b: &[u8]) -> LfnEntry {
        let read = |start: usize, out: &mut [u16]| {
            for (i, slot) in out.iter_mut().enumerate() {
                *slot = u16_at(b, start + i * 2);
            }
        };
        let mut name1 = [0u16; 5];
        read(1, &mut name1);
        let mut name2 = [0u16; 6];
        read(14, &mut name2);
        let mut name3 = [0u16; 2];
        read(28, &mut name3);
        LfnEntry {
            sequence: b[0],
            name1,
            attr: b[11],
            entry_type: b[12],
            checksum: b[13],
            name2,
            first_cluster_lo: u16_at(b, 26),
            name3,
        }
    }

    pub fn to_bytes(&self) -> [u8; ENTRY_SIZE] {
        let mut b = [0u8; ENTRY_SIZE];
        b[0] = self.sequence;
        let write = |start: usize, b: &mut [u8], chars: &[u16]| {
            for (i, c) in chars.iter().enumerate() {
                b[start + i * 2..start + i * 2 + 2].copy_from_slice(&c.to_le_bytes());
            }
        };
        write(1, &mut b, &self.name1);
        b[11] = self.attr;
        b[12] = self.entry_type;
        b[13] = self.checksum;
        write(14, &mut b, &self.name2);
        b[26..28].copy_from_slice(&self.first_cluster_lo.to_le_bytes());
        write(28, &mut b, &self.name3);
        b
    }

    /// All 13 characters in name order.
    pub fn chars(&self) -> [u16; 13] {
        let mut out = [0u16; 13];
        out[0..5].copy_from_slice(&self.name1);
        out[5..11].copy_from_slice(&self.name2);
        out[11..13].copy_from_slice(&self.name3);
        out
    }

    pub fn order(&self) -> u8 {
        self.sequence & 0x1F
    }

    pub fn is_last(&self) -> bool {
        self.sequence & LAST_LFN_FLAG != 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use fs_core::DateTime;

    #[test]
    fn date_and_time_pack_to_known_values() {
        let dt = DateTime::new(2026, 9, 21, 12, 34, 56);
        assert_eq!(pack_date(&dt), 0x5D35);
        assert_eq!(pack_time(&dt), 0x645C);
        assert_eq!(unpack(0x5D35, 0x645C), Some(dt));
    }

    #[test]
    fn odd_seconds_round_down_and_zero_date_is_none() {
        let dt = DateTime::new(2026, 9, 21, 12, 34, 57);
        assert_eq!(unpack(pack_date(&dt), pack_time(&dt)).unwrap().second, 56);
        assert_eq!(unpack(0, 0), None);
    }

    #[test]
    fn out_of_range_time_fields_stay_in_their_bits() {
        // minute = 0 so an unmasked seconds overflow (99 / 2 = 0b110001, bit
        // 5 set) would visibly set the minute field's low bit if it leaked
        // across, instead of landing on a bit that's already 1.
        let dt = DateTime::new(2000, 1, 1, 99, 0, 99);
        assert_eq!(pack_time(&dt) >> 11, 99 & 0x1F);
        assert_eq!((pack_time(&dt) >> 5) & 0x3F, 0);
        assert_eq!(pack_time(&dt) & 0x1F, 17);
    }

    #[test]
    fn checksum_matches_hand_computed_value() {
        assert_eq!(lfn_checksum(b"A          "), 0x80);
        assert_eq!(lfn_checksum(&[0; 11]), 0);
    }

    #[test]
    fn short_entry_round_trips_and_exposes_fields() {
        let now = DateTime::new(2026, 9, 21, 12, 34, 56);
        let mut e = ShortEntry::new(*b"README  TXT", attr::ARCHIVE, &now);
        e.set_first_cluster(0x0001_0005);
        e.size = 1234;
        let bytes = e.to_bytes();
        assert_eq!(&bytes[0..11], b"README  TXT");
        assert_eq!(bytes[11], attr::ARCHIVE);
        assert_eq!(&bytes[20..22], &0x0001u16.to_le_bytes());
        assert_eq!(&bytes[26..28], &0x0005u16.to_le_bytes());
        assert_eq!(&bytes[28..32], &1234u32.to_le_bytes());
        let parsed = ShortEntry::parse(&bytes);
        assert_eq!(parsed, e);
        assert_eq!(parsed.first_cluster(), 0x0001_0005);
        assert_eq!(parsed.display_name(), "README.TXT");
        assert!(!parsed.is_dir());
        assert_eq!(unpack(parsed.create_date, parsed.create_time), Some(now));
        assert_eq!(
            unpack(parsed.access_date, 0),
            Some(DateTime::new(2026, 9, 21, 0, 0, 0))
        );
    }

    #[test]
    fn display_name_without_extension_and_dot_entries() {
        let e = ShortEntry::new(*b"FOO        ", attr::DIRECTORY, &DateTime::default());
        assert_eq!(e.display_name(), "FOO");
        assert!(e.is_dir());
        assert!(
            ShortEntry::new(*b".          ", attr::DIRECTORY, &DateTime::default()).is_dot_entry()
        );
        assert!(
            ShortEntry::new(*b"..         ", attr::DIRECTORY, &DateTime::default()).is_dot_entry()
        );
        assert!(!e.is_dot_entry());
        assert!(
            ShortEntry::new(*b"LABEL      ", attr::VOLUME_ID, &DateTime::default())
                .is_volume_label()
        );
    }

    #[test]
    fn lfn_entry_round_trips() {
        let chars: [u16; 13] = [
            b'M' as u16,
            b'y' as u16,
            0,
            0xFFFF,
            0xFFFF,
            0xFFFF,
            0xFFFF,
            0xFFFF,
            0xFFFF,
            0xFFFF,
            0xFFFF,
            0xFFFF,
            0xFFFF,
        ];
        let e = LfnEntry::new(1, true, 0x80, &chars);
        assert_eq!(e.sequence, 0x41);
        assert!(e.is_last());
        assert_eq!(e.order(), 1);
        let bytes = e.to_bytes();
        assert_eq!(bytes[0], 0x41);
        assert_eq!(bytes[11], attr::LFN);
        assert_eq!(bytes[13], 0x80);
        assert_eq!(&bytes[1..3], &[b'M', 0]);
        assert_eq!(&bytes[26..28], &[0, 0]);
        assert!(attr::is_lfn(bytes[11]));
        let parsed = LfnEntry::parse(&bytes);
        assert_eq!(parsed, e);
        assert_eq!(parsed.chars(), chars);
    }
}
