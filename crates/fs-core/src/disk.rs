//! The virtual disk: a flat byte buffer addressed by sector. Every write goes
//! through `write`/`fill` so it can be journaled into the open operation.

use crate::trace::{ByteChange, Event, OpRecord};
use crate::{Error, Result};

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
    fn sector_returns_the_right_slice() {
        let mut disk = Disk::new(4, 3);
        disk.write(4, &[1, 2, 3, 4]);
        assert_eq!(disk.sector(1), &[1, 2, 3, 4]);
        assert_eq!(disk.sector(2), &[0, 0, 0, 0]);
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
}
