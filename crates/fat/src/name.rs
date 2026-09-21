//! Name rules: which names fit a short (8.3) entry, how to derive a short
//! alias for a long name, and UCS-2 chunking for long-name entries.

use fs_core::{Error, Result};

pub const MAX_LONG_NAME: usize = 255;
pub const LFN_CHARS_PER_ENTRY: usize = 13;

const SHORT_INVALID: &[u8] = b"\"*+,/:;<=>?\\[]|";
const LONG_INVALID: &[char] = &['"', '*', '/', ':', '<', '>', '?', '\\', '|'];

fn is_short_char(c: u8) -> bool {
    c > 0x20 && c < 0x7F && c != b'.' && !SHORT_INVALID.contains(&c)
}

/// True when the name, uppercased, fits a short entry without an LFN.
pub fn is_valid_short_name(name: &str) -> bool {
    to_short_name_bytes(name).is_some()
}

/// The padded, uppercased 11-byte form, or `None` if the name needs an LFN.
pub fn to_short_name_bytes(name: &str) -> Option<[u8; 11]> {
    if name.is_empty() || !name.is_ascii() || name.matches('.').count() > 1 {
        return None;
    }
    let (base, ext) = match name.rfind('.') {
        Some(i) => (&name[..i], &name[i + 1..]),
        None => (name, ""),
    };
    if base.is_empty() || base.len() > 8 || ext.len() > 3 || (name.contains('.') && ext.is_empty())
    {
        return None;
    }
    if !base.bytes().chain(ext.bytes()).all(is_short_char) {
        return None;
    }
    let mut out = [b' '; 11];
    for (i, b) in base.bytes().enumerate() {
        out[i] = b.to_ascii_uppercase();
    }
    for (i, b) in ext.bytes().enumerate() {
        out[8 + i] = b.to_ascii_uppercase();
    }
    Some(out)
}

/// Rules for any name the caller passes in: non-empty, at most 255 UCS-2
/// units, no reserved characters, no leading/trailing spaces, not all dots.
pub fn validate_long_name(name: &str) -> Result<()> {
    if name.is_empty() || name.encode_utf16().count() > MAX_LONG_NAME {
        return Err(Error::InvalidName);
    }
    if name
        .chars()
        .any(|c| LONG_INVALID.contains(&c) || (c as u32) < 0x20)
    {
        return Err(Error::InvalidName);
    }
    if name.trim() != name || name.chars().all(|c| c == '.') {
        return Err(Error::InvalidName);
    }
    Ok(())
}

/// Windows-style alias: cleaned, uppercased base truncated to make room for
/// `~N`, where N is the smallest number whose result `taken` does not report.
pub fn generate_short_name(long: &str, taken: &dyn Fn(&[u8; 11]) -> bool) -> Result<[u8; 11]> {
    let (base_src, ext_src) = match long.rfind('.') {
        Some(i) if i > 0 => (&long[..i], &long[i + 1..]),
        _ => (long, ""),
    };
    let clean = |s: &str| -> Vec<u8> {
        s.chars()
            .filter(|&c| c != ' ' && c != '.')
            .map(|c| {
                if c.is_ascii() && is_short_char(c as u8) {
                    (c as u8).to_ascii_uppercase()
                } else {
                    b'_'
                }
            })
            .collect()
    };
    let base = clean(base_src);
    let ext: Vec<u8> = clean(ext_src).into_iter().take(3).collect();
    for n in 1..=999_999u32 {
        let tail = format!("~{n}");
        let keep = 8 - tail.len();
        let mut out = [b' '; 11];
        let mut i = 0;
        for &b in base.iter().take(keep) {
            out[i] = b;
            i += 1;
        }
        for &b in tail.as_bytes() {
            out[i] = b;
            i += 1;
        }
        for (j, &b) in ext.iter().enumerate() {
            out[8 + j] = b;
        }
        if !taken(&out) {
            return Ok(out);
        }
    }
    Err(Error::DirectoryFull)
}

pub fn to_ucs2(name: &str) -> Vec<u16> {
    name.encode_utf16().collect()
}

/// Decode up to the first `0x0000`, ignoring `0xFFFF` padding.
pub fn from_ucs2(units: &[u16]) -> String {
    let end = units.iter().position(|&u| u == 0).unwrap_or(units.len());
    let kept: Vec<u16> = units[..end]
        .iter()
        .copied()
        .filter(|&u| u != 0xFFFF)
        .collect();
    String::from_utf16_lossy(&kept)
}

/// Split a long name into 13-unit chunks: a `0x0000` terminator follows the
/// name unless it exactly fills the last chunk, and `0xFFFF` pads the rest.
pub fn lfn_chunks(name: &str) -> Vec<[u16; 13]> {
    let mut units = to_ucs2(name);
    if !units.len().is_multiple_of(LFN_CHARS_PER_ENTRY) {
        units.push(0);
    }
    while !units.len().is_multiple_of(LFN_CHARS_PER_ENTRY) {
        units.push(0xFFFF);
    }
    units
        .chunks(LFN_CHARS_PER_ENTRY)
        .map(|c| {
            let mut a = [0u16; 13];
            a.copy_from_slice(c);
            a
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use fs_core::Error;

    #[test]
    fn valid_short_names() {
        for n in [
            "README.TXT",
            "readme.txt",
            "FOO",
            "A",
            "FILE~1.TXT",
            "12345678.123",
        ] {
            assert!(is_valid_short_name(n), "{n} should be valid");
        }
    }

    #[test]
    fn invalid_short_names_need_lfn() {
        for n in [
            "My File.txt",
            "toolongname.txt",
            "a.b.c",
            "FOO.",
            "",
            ".HIDDEN",
            "FILE.TOOLONG",
            "über.txt",
            "a+b",
        ] {
            assert!(!is_valid_short_name(n), "{n} should be invalid");
        }
    }

    #[test]
    fn short_name_bytes_are_uppercased_and_padded() {
        assert_eq!(to_short_name_bytes("readme.txt"), Some(*b"README  TXT"));
        assert_eq!(to_short_name_bytes("foo"), Some(*b"FOO        "));
    }

    #[test]
    fn generated_short_names() {
        let none = |_: &[u8; 11]| false;
        assert_eq!(
            generate_short_name("My File.txt", &none).unwrap(),
            *b"MYFILE~1TXT"
        );
        assert_eq!(
            generate_short_name("a.b.c.txt", &none).unwrap(),
            *b"ABC~1   TXT"
        );
        assert_eq!(
            generate_short_name(".bashrc", &none).unwrap(),
            *b"BASHRC~1   "
        );
        assert_eq!(
            generate_short_name("verylongfilename.document", &none).unwrap(),
            *b"VERYLO~1DOC"
        );
        assert_eq!(generate_short_name("x", &none).unwrap(), *b"X~1        ");
        assert_eq!(
            generate_short_name("über.txt", &none).unwrap(),
            *b"_BER~1  TXT"
        );
    }

    #[test]
    fn tilde_number_skips_taken_names() {
        let taken = |n: &[u8; 11]| n == b"MYFILE~1TXT" || n == b"MYFILE~2TXT";
        assert_eq!(
            generate_short_name("My File.txt", &taken).unwrap(),
            *b"MYFILE~3TXT"
        );
        let many = |n: &[u8; 11]| n[6] == b'~'; // every single-digit tail is taken
        assert_eq!(
            generate_short_name("My File.txt", &many).unwrap(),
            *b"MYFIL~10TXT"
        );
    }

    #[test]
    fn long_name_validation() {
        assert_eq!(validate_long_name("My File.txt"), Ok(()));
        assert_eq!(validate_long_name(""), Err(Error::InvalidName));
        assert_eq!(
            validate_long_name(&"a".repeat(256)),
            Err(Error::InvalidName)
        );
        assert_eq!(validate_long_name("bad*name"), Err(Error::InvalidName));
        assert_eq!(validate_long_name(" lead"), Err(Error::InvalidName));
        assert_eq!(validate_long_name("."), Err(Error::InvalidName));
    }

    #[test]
    fn lfn_chunks_terminate_and_pad() {
        let chunks = lfn_chunks("My File.txt");
        assert_eq!(chunks.len(), 1);
        assert_eq!(&chunks[0][..11], &to_ucs2("My File.txt")[..]);
        assert_eq!(chunks[0][11], 0);
        assert_eq!(chunks[0][12], 0xFFFF);
        assert_eq!(lfn_chunks(&"A".repeat(13)).len(), 1);
        assert_eq!(lfn_chunks(&"A".repeat(13))[0][12], b'A' as u16);
        assert_eq!(lfn_chunks(&"A".repeat(14)).len(), 2);
        assert_eq!(lfn_chunks(&"A".repeat(14))[1][1], 0);
    }

    #[test]
    fn ucs2_round_trip_strips_padding() {
        let mut units = to_ucs2("Héllo");
        units.extend([0, 0xFFFF, 0xFFFF]);
        assert_eq!(from_ucs2(&units), "Héllo");
    }
}
