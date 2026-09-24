//! The virtual disk: a flat byte buffer addressed by sector. Every write goes
//! through `write`/`fill` so it can be journaled into the open operation.

use crate::trace::{ByteChange, Event, OpRecord, RawWrite};
use crate::{Error, Result};
use std::ops::Range;

pub struct Disk {
    bytes: Vec<u8>,
    sector_size: usize,
    open_op: Option<OpRecord>,
}

impl Disk {
    /// A zero-filled disk.
    pub fn new(sector_size: usize, sector_count: u64) -> Disk {
        assert!(sector_size > 0, "sector size must be non-zero");
        Disk {
            bytes: vec![0; sector_size * sector_count as usize],
            sector_size,
            open_op: None,
        }
    }

    /// Wrap an existing image. The length must be a whole number of sectors.
    pub fn from_bytes(sector_size: usize, bytes: Vec<u8>) -> Result<Disk> {
        if sector_size == 0 || !bytes.len().is_multiple_of(sector_size) {
            return Err(Error::InvalidGeometry(format!(
                "image length {} is not a multiple of sector size {}",
                bytes.len(),
                sector_size
            )));
        }
        Ok(Disk {
            bytes,
            sector_size,
            open_op: None,
        })
    }

    pub fn sector_size(&self) -> usize {
        self.sector_size
    }

    pub fn sector_count(&self) -> u64 {
        (self.bytes.len() / self.sector_size) as u64
    }

    pub fn len(&self) -> usize {
        self.bytes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// The whole image.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn sector(&self, n: u64) -> &[u8] {
        let start = n as usize * self.sector_size;
        &self.bytes[start..start + self.sector_size]
    }

    pub fn read(&self, offset: usize, len: usize) -> &[u8] {
        &self.bytes[offset..offset + len]
    }

    /// Write bytes at an absolute offset. Recorded if an operation is open.
    /// Writing past the end of the disk is a programmer error and panics.
    pub fn write(&mut self, offset: usize, data: &[u8]) {
        assert!(
            offset + data.len() <= self.bytes.len(),
            "write of {} bytes at offset {offset} runs past the end of the {}-byte disk",
            data.len(),
            self.bytes.len()
        );
        let end = offset + data.len();
        if let Some(op) = self.open_op.as_mut() {
            op.changes.push(ByteChange {
                offset,
                before: self.bytes[offset..end].to_vec(),
                after: data.to_vec(),
            });
        }
        self.bytes[offset..end].copy_from_slice(data);
    }

    pub fn fill(&mut self, offset: usize, len: usize, byte: u8) {
        let data = vec![byte; len];
        self.write(offset, &data);
    }

    /// Start recording. Panics if an operation is already open.
    pub fn begin_op(&mut self, op: impl Into<String>) {
        assert!(self.open_op.is_none(), "an operation is already open");
        self.open_op = Some(OpRecord::new(op));
    }

    /// Attach an event to the open operation. Dropped if none is open.
    pub fn event(&mut self, event: Box<dyn Event>) {
        if let Some(op) = self.open_op.as_mut() {
            op.events.push(event);
        }
    }

    /// Stop recording and return the record. Panics if no operation is open.
    pub fn end_op(&mut self) -> OpRecord {
        self.open_op.take().expect("no operation is open")
    }

    pub fn op_open(&self) -> bool {
        self.open_op.is_some()
    }
}

/// The op name every filesystem uses for a raw write, e.g. `write_raw 0x200 +512`.
pub fn raw_write_op(offset: u64, len: usize) -> String {
    format!("write_raw 0x{offset:x} +{len}")
}

/// Bounds-check, then write `bytes` at `offset` and report one `RawWrite`
/// event. Journaled only if the caller has an operation open; the caller
/// (a filesystem's `run_op`) owns begin/end and history. Nothing is written
/// on `OutOfBounds`, so a rollback has nothing to undo. Empty `bytes` is
/// bounds-checked but records no `ByteChange` and no event. Returns the
/// absolute byte range written.
pub fn raw_write(disk: &mut Disk, offset: u64, bytes: &[u8]) -> Result<Range<usize>> {
    let disk_len = disk.len() as u64;
    let len = bytes.len() as u64;
    let Some(end) = offset.checked_add(len).filter(|&e| e <= disk_len) else {
        return Err(Error::OutOfBounds {
            offset,
            len,
            disk_len,
        });
    };
    // Both are <= disk.len(), which fits in usize by construction.
    let (start, end) = (offset as usize, end as usize);
    if bytes.is_empty() {
        return Ok(start..start);
    }
    disk.write(start, bytes);
    disk.event(Box::new(RawWrite {
        offset: start,
        len: bytes.len(),
    }));
    Ok(start..end)
}

/// Settle the operation the caller opened with `disk.begin_op`. On `Ok` a
/// clone of the record is appended to `history` and the record returned. On
/// `Err` every recorded change is restored in reverse order (the op is
/// already closed, so the restores are not journaled), `history` is left
/// alone, and the error is returned. Panics if no operation is open.
pub fn finish_op(
    disk: &mut Disk,
    history: &mut Vec<OpRecord>,
    result: Result<()>,
) -> Result<OpRecord> {
    let record = disk.end_op();
    if let Err(e) = result {
        for change in record.changes.iter().rev() {
            disk.write(change.offset, &change.before);
        }
        return Err(e);
    }
    history.push(record.clone());
    Ok(record)
}

/// Run `body` as one recorded operation named `op`: `begin_op`, the body,
/// then `finish_op`. For bodies that need only the disk; a filesystem whose
/// body needs more of itself calls `begin_op` and `finish_op` directly.
pub fn run_op(
    disk: &mut Disk,
    history: &mut Vec<OpRecord>,
    op: String,
    body: impl FnOnce(&mut Disk) -> Result<()>,
) -> Result<OpRecord> {
    disk.begin_op(op);
    let result = body(disk);
    finish_op(disk, history, result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::trace::{ByteChange, Event};
    use std::fmt;
    use std::ops::Range;

    #[derive(Debug, Clone)]
    struct Dummy;
    impl fmt::Display for Dummy {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            write!(f, "dummy")
        }
    }
    impl Event for Dummy {
        fn kind(&self) -> &'static str {
            "dummy"
        }
        fn region(&self) -> Option<Range<usize>> {
            Some(0..1)
        }
        fn clone_box(&self) -> Box<dyn Event> {
            Box::new(self.clone())
        }
    }

    #[test]
    fn new_disk_is_zeroed_with_expected_size() {
        let disk = Disk::new(512, 4);
        assert_eq!(disk.len(), 2048);
        assert_eq!(disk.sector_count(), 4);
        assert!(disk.as_bytes().iter().all(|&b| b == 0));
    }

    #[test]
    fn from_bytes_rejects_partial_sector() {
        assert!(matches!(
            Disk::from_bytes(512, vec![0; 513]),
            Err(crate::Error::InvalidGeometry(_))
        ));
        assert_eq!(
            Disk::from_bytes(512, vec![0; 1024]).unwrap().sector_count(),
            2
        );
    }

    #[test]
    fn write_outside_an_op_changes_bytes_without_recording() {
        let mut disk = Disk::new(512, 1);
        disk.write(3, &[7, 8]);
        assert_eq!(disk.read(3, 2), &[7, 8]);
        assert!(!disk.op_open());
    }

    #[test]
    fn write_inside_an_op_records_before_and_after() {
        let mut disk = Disk::new(512, 2);
        disk.begin_op("test");
        disk.write(3, &[7, 8]);
        disk.fill(600, 3, 0xAA);
        disk.event(Box::new(Dummy));
        let record = disk.end_op();
        assert_eq!(record.op, "test");
        assert_eq!(
            record.changes,
            vec![
                ByteChange {
                    offset: 3,
                    before: vec![0, 0],
                    after: vec![7, 8]
                },
                ByteChange {
                    offset: 600,
                    before: vec![0, 0, 0],
                    after: vec![0xAA; 3]
                },
            ]
        );
        assert_eq!(record.event_kinds(), vec!["dummy"]);
        assert_eq!(record.changed_sectors(512), vec![0, 1]);
        assert!(!disk.op_open());
    }

    #[test]
    fn events_outside_an_op_are_dropped() {
        let mut disk = Disk::new(512, 1);
        disk.event(Box::new(Dummy)); // must not panic
        disk.begin_op("x");
        assert!(disk.end_op().events.is_empty());
    }

    #[test]
    #[should_panic(expected = "runs past the end")]
    fn write_past_the_end_panics_with_a_clear_message() {
        let mut disk = Disk::new(4, 1);
        let len = disk.len();
        disk.write(len - 1, &[1, 2]);
    }

    #[test]
    fn sector_returns_the_right_slice() {
        let mut disk = Disk::new(4, 3);
        disk.write(4, &[1, 2, 3, 4]);
        assert_eq!(disk.sector(1), &[1, 2, 3, 4]);
        assert_eq!(disk.sector(2), &[0, 0, 0, 0]);
    }

    #[test]
    fn raw_write_op_formats_lowercase_hex_and_length() {
        assert_eq!(raw_write_op(512, 3), "write_raw 0x200 +3");
        assert_eq!(raw_write_op(0, 0), "write_raw 0x0 +0");
        assert_eq!(raw_write_op(0xABCD, 16), "write_raw 0xabcd +16");
    }

    #[test]
    fn raw_write_inside_an_op_records_change_and_event() {
        let mut disk = Disk::new(512, 2);
        disk.begin_op("t");
        assert_eq!(raw_write(&mut disk, 512, &[1, 2, 3]), Ok(512..515));
        let record = disk.end_op();
        assert_eq!(
            record.changes,
            vec![ByteChange {
                offset: 512,
                before: vec![0, 0, 0],
                after: vec![1, 2, 3]
            }]
        );
        assert_eq!(record.event_kinds(), vec!["raw_write"]);
        assert_eq!(record.events[0].region(), Some(512..515));
        assert_eq!(record.events[0].to_string(), "wrote 3 raw bytes at 0x200");
        assert_eq!(record.changed_sectors(512), vec![1]);
        assert_eq!(disk.read(512, 3), &[1, 2, 3]);
    }

    #[test]
    fn raw_write_outside_an_op_changes_bytes_without_recording() {
        let mut disk = Disk::new(512, 1);
        assert_eq!(raw_write(&mut disk, 5, &[9, 9]), Ok(5..7));
        assert_eq!(disk.read(5, 2), &[9, 9]);
        assert!(!disk.op_open());
    }

    #[test]
    fn raw_write_rejects_ranges_past_the_end_without_touching_the_disk() {
        let mut disk = Disk::new(4, 1);
        assert_eq!(
            raw_write(&mut disk, 3, &[1, 2]),
            Err(Error::OutOfBounds {
                offset: 3,
                len: 2,
                disk_len: 4
            })
        );
        assert_eq!(
            raw_write(&mut disk, 4, &[1]),
            Err(Error::OutOfBounds {
                offset: 4,
                len: 1,
                disk_len: 4
            })
        );
        assert_eq!(
            raw_write(&mut disk, u64::MAX, &[1]),
            Err(Error::OutOfBounds {
                offset: u64::MAX,
                len: 1,
                disk_len: 4
            })
        );
        assert_eq!(
            raw_write(&mut disk, u64::MAX, &[]),
            Err(Error::OutOfBounds {
                offset: u64::MAX,
                len: 0,
                disk_len: 4
            })
        );
        assert!(disk.as_bytes().iter().all(|&b| b == 0));
        disk.begin_op("t");
        assert!(raw_write(&mut disk, 3, &[1, 2]).is_err());
        let record = disk.end_op();
        assert!(record.changes.is_empty());
        assert!(record.events.is_empty());
    }

    #[test]
    fn raw_write_of_zero_bytes_is_bounds_checked_but_records_nothing() {
        let mut disk = Disk::new(4, 1);
        assert_eq!(raw_write(&mut disk, 4, &[]), Ok(4..4));
        assert_eq!(raw_write(&mut disk, 1, &[]), Ok(1..1));
        assert_eq!(
            raw_write(&mut disk, 5, &[]),
            Err(Error::OutOfBounds {
                offset: 5,
                len: 0,
                disk_len: 4
            })
        );
        disk.begin_op("t");
        assert_eq!(raw_write(&mut disk, 2, &[]), Ok(2..2));
        let record = disk.end_op();
        assert!(record.changes.is_empty());
        assert!(record.events.is_empty());
    }

    #[test]
    fn op_record_is_cloneable() {
        let mut disk = Disk::new(4, 1);
        disk.begin_op("clone");
        disk.event(Box::new(Dummy));
        let record = disk.end_op();
        let copy = record.clone();
        assert_eq!(copy.events[0].to_string(), "dummy");
    }

    #[test]
    fn run_op_on_success_pushes_the_record_and_returns_it() {
        let mut disk = Disk::new(4, 2);
        let mut history = Vec::new();
        let record = run_op(&mut disk, &mut history, "ok".to_string(), |d| {
            d.write(1, &[5, 6]);
            d.event(Box::new(Dummy));
            Ok(())
        })
        .unwrap();
        assert_eq!(record.op, "ok");
        assert_eq!(
            record.changes,
            vec![ByteChange {
                offset: 1,
                before: vec![0, 0],
                after: vec![5, 6]
            }]
        );
        assert_eq!(record.event_kinds(), vec!["dummy"]);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].op, "ok");
        assert_eq!(history[0].changes, record.changes);
        assert_eq!(disk.read(1, 2), &[5, 6]);
        assert!(!disk.op_open());
    }

    #[test]
    fn run_op_on_error_restores_every_before_in_reverse_order() {
        let mut disk = Disk::new(4, 1);
        disk.write(0, &[9, 9, 9, 9]);
        let mut history = Vec::new();
        run_op(&mut disk, &mut history, "first".to_string(), |_| Ok(())).unwrap();
        // Overlapping writes: only a reverse-order restore gets back to 9 9 9 9.
        let err = run_op(&mut disk, &mut history, "fails".to_string(), |d| {
            d.write(0, &[1, 1]);
            d.write(1, &[2, 2]);
            Err(Error::DiskFull)
        })
        .unwrap_err();
        assert_eq!(err, Error::DiskFull);
        assert_eq!(disk.as_bytes(), &[9, 9, 9, 9]);
        assert_eq!(history.len(), 1);
        assert_eq!(history[0].op, "first");
        assert!(!disk.op_open());
    }

    #[test]
    fn finish_op_settles_an_op_the_caller_opened() {
        let mut disk = Disk::new(4, 1);
        let mut history = Vec::new();
        disk.begin_op("kept");
        disk.write(0, &[7]);
        let record = finish_op(&mut disk, &mut history, Ok(())).unwrap();
        assert_eq!(record.op, "kept");
        assert_eq!(history.len(), 1);
        disk.begin_op("dropped");
        disk.write(0, &[8]);
        disk.write(3, &[8]);
        assert_eq!(
            finish_op(&mut disk, &mut history, Err(Error::InvalidName)).unwrap_err(),
            Error::InvalidName
        );
        assert_eq!(disk.as_bytes(), &[7, 0, 0, 0]);
        assert_eq!(history.len(), 1);
        assert!(!disk.op_open());
    }

    #[test]
    #[should_panic(expected = "no operation is open")]
    fn finish_op_without_an_open_op_panics() {
        let mut disk = Disk::new(4, 1);
        let mut history = Vec::new();
        let _ = finish_op(&mut disk, &mut history, Ok(()));
    }
}
