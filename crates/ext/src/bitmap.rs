//! Bit operations over a block or inode bitmap. Bit `i` is bit `i % 8`
//! (least significant first) of byte `i / 8`, as ext2 lays them out.

pub fn is_set(bytes: &[u8], bit: usize) -> bool {
    bytes[bit / 8] & (1 << (bit % 8)) != 0
}

pub fn set(bytes: &mut [u8], bit: usize) {
    bytes[bit / 8] |= 1 << (bit % 8);
}

pub fn clear(bytes: &mut [u8], bit: usize) {
    bytes[bit / 8] &= !(1 << (bit % 8));
}

/// The first clear bit in `[from, upto)`, if any.
pub fn find_clear(bytes: &[u8], from: usize, upto: usize) -> Option<usize> {
    (from..upto).find(|&bit| !is_set(bytes, bit))
}

/// Set every bit in `[from, upto)`; used for metadata blocks and for the
/// padding bits past a short group's last block or past `inodes_per_group`.
pub fn set_range(bytes: &mut [u8], from: usize, upto: usize) {
    for bit in from..upto {
        set(bytes, bit);
    }
}

/// Maximal runs of equal bits in `[0, upto)` as `(value, start, end)`, `end`
/// exclusive, in order.
pub fn runs(bytes: &[u8], upto: usize) -> Vec<(bool, usize, usize)> {
    let mut out: Vec<(bool, usize, usize)> = Vec::new();
    for bit in 0..upto {
        let value = is_set(bytes, bit);
        match out.last_mut() {
            Some((v, _, end)) if *v == value => *end = bit + 1,
            _ => out.push((value, bit, bit + 1)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn set_clear_and_is_set_use_lsb_first_order() {
        let mut b = [0u8; 4];
        set(&mut b, 0);
        set(&mut b, 9);
        set(&mut b, 31);
        assert_eq!(b, [0x01, 0x02, 0x00, 0x80]);
        assert!(is_set(&b, 0) && is_set(&b, 9) && is_set(&b, 31));
        assert!(!is_set(&b, 1) && !is_set(&b, 8));
        clear(&mut b, 9);
        assert_eq!(b, [0x01, 0x00, 0x00, 0x80]);
        clear(&mut b, 9);
        assert!(!is_set(&b, 9));
    }

    #[test]
    fn find_clear_searches_the_half_open_range() {
        let mut b = [0u8; 1024];
        set_range(&mut b, 0, 68);
        assert_eq!(find_clear(&b, 0, 8192), Some(68));
        assert_eq!(find_clear(&b, 10, 8192), Some(68));
        assert_eq!(find_clear(&b, 70, 8192), Some(70));
        assert_eq!(find_clear(&b, 0, 68), None);
        set_range(&mut b, 0, 8192);
        assert_eq!(find_clear(&b, 0, 8192), None);
        clear(&mut b, 8191);
        assert_eq!(find_clear(&b, 0, 8192), Some(8191));
    }

    #[test]
    fn set_range_covers_padding_bits_of_a_short_group() {
        // The default disk's group 1 has 8,191 blocks: bit 8,191 is padding.
        let mut b = [0u8; 1024];
        set_range(&mut b, 8191, 8192);
        assert_eq!(b[1023], 0x80);
        assert_eq!(b[..1023], [0u8; 1023]);
        // A 16-inode group pads bits 16.. of its inode bitmap.
        let mut b = [0u8; 1024];
        set_range(&mut b, 16, 8192);
        assert_eq!(b[..2], [0, 0]);
        assert!(b[2..].iter().all(|&x| x == 0xFF));
        set_range(&mut b, 5, 5);
        assert!(!is_set(&b, 5));
    }

    #[test]
    fn runs_reports_maximal_runs() {
        let mut b = [0u8; 1024];
        set_range(&mut b, 0, 69);
        set(&mut b, 80);
        set_range(&mut b, 8191, 8192);
        assert_eq!(
            runs(&b, 8192),
            vec![
                (true, 0, 69),
                (false, 69, 80),
                (true, 80, 81),
                (false, 81, 8191),
                (true, 8191, 8192)
            ]
        );
        assert_eq!(runs(&b, 0), vec![]);
        assert_eq!(runs(&b, 3), vec![(true, 0, 3)]);
    }
}
