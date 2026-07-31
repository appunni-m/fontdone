//! 'kern' table — legacy horizontal kerning pairs.
//!
//! This implements the format-0 horizontal pair data used by FreeType's SFNT
//! kerning service for fonts such as DejaVu Sans. GPOS kerning is intentionally
//! separate; this table is enough for the `_imagingft` AV fixture and is a
//! standard TrueType/OpenType table, not an oracle shortcut.

use crate::error::FontError;

/// Parsed legacy kerning table.
#[derive(Debug, Clone)]
pub struct KernTable {
    pairs: Vec<KernPair>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct KernPair {
    left: u16,
    right: u16,
    value: i16,
}

impl KernTable {
    /// Return whether the table contains no usable horizontal pairs.
    pub fn is_empty(&self) -> bool {
        self.pairs.is_empty()
    }

    /// Return the unscaled kerning value in font units.
    pub fn get(&self, left: u16, right: u16) -> i16 {
        self.pairs
            .binary_search_by_key(&(left, right), |pair| (pair.left, pair.right))
            .map_or(0, |index| self.pairs[index].value)
    }
}

/// Parse a classic TrueType `kern` table.
pub fn parse_kern(data: &[u8]) -> Result<KernTable, FontError> {
    if data.len() < 4 {
        return Err(FontError::InvalidFont("kern table too short".into()));
    }
    // ttkern.c ignores the top-level version and caps work at 32 subtables.
    let n_tables = usize::from(u16::from_be_bytes([data[2], data[3]])).min(32);
    let mut offset = 4usize;
    let mut pairs = Vec::new();

    for _ in 0..n_tables {
        let Some(header) = data.get(offset..offset + 6) else {
            break;
        };
        let length = u16::from_be_bytes([header[2], header[3]]) as usize;
        let coverage = u16::from_be_bytes([header[4], header[5]]);
        if length <= 14 {
            break;
        }
        let subtable_end = offset.saturating_add(length).min(data.len());
        let subtable = &data[offset..subtable_end];
        let format = coverage >> 8;
        if format == 0 && (coverage & 3) == 1 && subtable.len() >= 14 {
            parse_format0(subtable, &mut pairs);
        }
        offset = offset.saturating_add(length);
    }

    pairs.sort_unstable_by_key(|pair| (pair.left, pair.right));
    pairs.dedup_by_key(|pair| (pair.left, pair.right));
    Ok(KernTable { pairs })
}

fn parse_format0(subtable: &[u8], pairs: &mut Vec<KernPair>) {
    let n_pairs = u16::from_be_bytes([subtable[6], subtable[7]]) as usize;
    let records = &subtable[14..];
    for chunk in records.chunks_exact(6).take(n_pairs) {
        pairs.push(KernPair {
            left: u16::from_be_bytes([chunk[0], chunk[1]]),
            right: u16::from_be_bytes([chunk[2], chunk[3]]),
            value: i16::from_be_bytes([chunk[4], chunk[5]]),
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(data: &[u8], label: &str) -> KernTable {
        match parse_kern(data) {
            Ok(table) => table,
            Err(error) => panic!("{label}: {error}"),
        }
    }

    fn format0_subtable(pairs: &[(u16, u16, i16)]) -> Vec<u8> {
        let mut bytes = vec![0, 0, 0, 0, 0, 1]; // version 0, length, coverage=1 (horizontal)
        bytes.extend_from_slice(&(pairs.len() as u16).to_be_bytes());
        bytes.extend_from_slice(&[0; 6]); // searchRange, entrySelector, rangeShift
        for (left, right, value) in pairs {
            bytes.extend_from_slice(&left.to_be_bytes());
            bytes.extend_from_slice(&right.to_be_bytes());
            bytes.extend_from_slice(&value.to_be_bytes());
        }
        let length = bytes.len() as u16;
        bytes[2..4].copy_from_slice(&length.to_be_bytes());
        bytes
    }

    #[test]
    fn parses_horizontal_format0_pairs() {
        let mut data = vec![0, 0, 0, 1]; // version 0, one subtable
        let subtable = format0_subtable(&[(65, 66, -40), (66, 65, 10)]);
        data.extend_from_slice(&subtable);
        let table = parse_ok(&data, "valid kern parses");
        assert!(!table.is_empty());
        assert_eq!(table.get(65, 66), -40);
        assert_eq!(table.get(66, 65), 10);
        assert_eq!(table.get(65, 67), 0);
    }

    #[test]
    fn rejects_short_and_ignores_non_horizontal_tables() {
        assert!(parse_kern(&[0, 0]).is_err());
        assert!(parse_kern(&[0, 0, 0, 0]).is_ok());
        // Coverage bits 0-1 select vertical/horizontal; bit 0 clear means
        // the format-0 horizontal branch is skipped.
        let mut data = vec![0, 0, 0, 1];
        let mut subtable = vec![0u8; 20];
        subtable[4] = 0; // coverage = 0 -> not horizontal
        data.extend_from_slice(&(subtable.len() as u16).to_be_bytes());
        data.extend_from_slice(&subtable);
        let table = parse_ok(&data, "non-horizontal kern parses empty");
        assert!(table.is_empty());
    }

    #[test]
    fn stops_on_malformed_subtable() {
        let mut data = vec![0, 0, 0, 1];
        // Header with length 10 (< 14) causes parsing to stop.
        data.extend_from_slice(&10u16.to_be_bytes());
        data.extend_from_slice(&[0; 6]);
        let table = parse_ok(&data, "malformed subtable stops cleanly");
        assert!(table.is_empty());
    }
}
