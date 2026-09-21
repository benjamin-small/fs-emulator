//! The file allocation table: one entry per cluster saying whether it is free,
//! which cluster follows it, or that it ends a chain.

use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FatEntry {
    Free,
    Next(u32),
    EndOfChain,
    Bad,
    Reserved,
}

impl fmt::Display for FatEntry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FatEntry::Free => write!(f, "free"),
            FatEntry::Next(n) => write!(f, "next -> {n}"),
            FatEntry::EndOfChain => write!(f, "end of chain"),
            FatEntry::Bad => write!(f, "bad"),
            FatEntry::Reserved => write!(f, "reserved"),
        }
    }
}

pub fn decode16(raw: u16) -> FatEntry {
    match raw {
        0x0000 => FatEntry::Free,
        0x0002..=0xFFEF => FatEntry::Next(raw as u32),
        0xFFF7 => FatEntry::Bad,
        0xFFF8..=0xFFFF => FatEntry::EndOfChain,
        _ => FatEntry::Reserved,
    }
}

pub fn encode16(entry: FatEntry) -> u16 {
    match entry {
        FatEntry::Free => 0x0000,
        FatEntry::Next(n) => n as u16,
        FatEntry::EndOfChain => 0xFFFF,
        FatEntry::Bad => 0xFFF7,
        FatEntry::Reserved => 0x0001,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_covers_every_range() {
        assert_eq!(decode16(0x0000), FatEntry::Free);
        assert_eq!(decode16(0x0001), FatEntry::Reserved);
        assert_eq!(decode16(0x0002), FatEntry::Next(2));
        assert_eq!(decode16(0xFFEF), FatEntry::Next(0xFFEF));
        assert_eq!(decode16(0xFFF0), FatEntry::Reserved);
        assert_eq!(decode16(0xFFF7), FatEntry::Bad);
        assert_eq!(decode16(0xFFF8), FatEntry::EndOfChain);
        assert_eq!(decode16(0xFFFF), FatEntry::EndOfChain);
    }

    #[test]
    fn encode_round_trips() {
        for e in [
            FatEntry::Free,
            FatEntry::Next(77),
            FatEntry::EndOfChain,
            FatEntry::Bad,
            FatEntry::Reserved,
        ] {
            assert_eq!(decode16(encode16(e)), e);
        }
        assert_eq!(encode16(FatEntry::EndOfChain), 0xFFFF);
    }

    #[test]
    fn display() {
        assert_eq!(FatEntry::Next(9).to_string(), "next -> 9");
        assert_eq!(FatEntry::EndOfChain.to_string(), "end of chain");
    }
}
