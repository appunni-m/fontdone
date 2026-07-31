//! 'hdmx' table — horizontal device metrics.
//!
//! FreeType uses this optional table as a pixel-size-specific horizontal
//! advance override for hinted TrueType loads (`ttgload.c:2299-2313`,
//! `ttgload.c:1974-1977`).

use crate::error::FontError;

#[derive(Debug, Clone)]
pub struct HdmxTable {
    records: Vec<HdmxRecord>,
}

#[derive(Debug, Clone)]
struct HdmxRecord {
    ppem: u8,
    widths: Vec<u8>,
}

impl HdmxTable {
    pub fn width_for_ppem(&self, ppem: i32, glyph_index: u16) -> Option<u8> {
        let ppem = u8::try_from(ppem).ok()?;
        let index = self
            .records
            .binary_search_by_key(&ppem, |record| record.ppem)
            .ok()?;
        let record = &self.records[index];
        record.widths.get(glyph_index as usize).copied()
    }
}

pub fn parse_hdmx(data: &[u8], num_glyphs: u16) -> Result<HdmxTable, FontError> {
    if data.len() < 8 {
        return Err(FontError::InvalidFont("hdmx table too short".into()));
    }

    let num_records = u16::from_be_bytes([data[2], data[3]]) as usize;
    let mut record_size = u32::from_be_bytes([data[4], data[5], data[6], data[7]]);
    if record_size >= 0xFFFF_0000 {
        record_size &= 0xFFFF;
    }

    if num_records == 0 || num_records > 255 {
        return Err(FontError::InvalidFont(
            "hdmx record count out of range".into(),
        ));
    }

    let expected_record_size = (u32::from(num_glyphs) + 2 + 3) & !3;
    if record_size != expected_record_size {
        return Err(FontError::InvalidFont("hdmx record size mismatch".into()));
    }

    let record_size = record_size as usize;
    let num_glyphs = num_glyphs as usize;
    let mut records = Vec::with_capacity(num_records);
    let mut offset = 8usize;
    for _ in 0..num_records {
        let Some(record) = data.get(offset..offset + record_size) else {
            break;
        };
        records.push(HdmxRecord {
            ppem: record[0],
            widths: record[2..2 + num_glyphs].to_vec(),
        });
        offset += record_size;
    }

    records.sort_by_key(|record| record.ppem);
    Ok(HdmxTable { records })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_ok(data: &[u8], num_glyphs: u16, label: &str) -> HdmxTable {
        match parse_hdmx(data, num_glyphs) {
            Ok(table) => table,
            Err(error) => panic!("{label}: {error}"),
        }
    }

    #[test]
    fn parses_and_finds_width_record() {
        let data = [
            0, 0, // version
            0, 1, // one record
            0, 0, 0, 8, // record size = (3 glyphs + 2 + padding)
            10, 7, // ppem, max width
            4, 5, 6, // glyph widths
            0, 0, 0, // padding
        ];
        let table = parse_ok(&data, 3, "valid hdmx parses");
        assert_eq!(table.width_for_ppem(10, 1), Some(5));
        assert_eq!(table.width_for_ppem(9, 1), None);
    }

    #[test]
    fn rejects_bad_record_counts_and_sizes() {
        assert!(parse_hdmx(&[0; 7], 3).is_err());
        // Record count 0.
        assert!(parse_hdmx(&[0, 0, 0, 0, 0, 0, 0, 8], 3).is_err());
        // Record count above 255.
        let mut data = vec![0u8; 8];
        data[2..4].copy_from_slice(&256u16.to_be_bytes());
        assert!(parse_hdmx(&data, 3).is_err());
        // Record-size mismatch.
        let mut data = vec![0u8; 8];
        data[2..4].copy_from_slice(&1u16.to_be_bytes());
        data[4..8].copy_from_slice(&4u32.to_be_bytes());
        assert!(parse_hdmx(&data, 3).is_err());
    }

    #[test]
    fn accepts_large_record_size_normalization() {
        let mut data = vec![0u8; 8];
        data[2..4].copy_from_slice(&1u16.to_be_bytes());
        // record size with high bits set is masked to 8 for 3 glyphs.
        data[4..8].copy_from_slice(&0xFFFF_0008u32.to_be_bytes());
        data.extend_from_slice(&[10, 7, 4, 5, 6, 0, 0, 0]);
        let table = parse_ok(&data, 3, "normalized record size parses");
        assert_eq!(table.width_for_ppem(10, 2), Some(6));
    }

    #[test]
    fn truncation_stops_cleanly() {
        let mut data = vec![0u8; 8];
        data[2..4].copy_from_slice(&2u16.to_be_bytes());
        data[4..8].copy_from_slice(&8u32.to_be_bytes());
        data.extend_from_slice(&[10, 7, 4, 5, 6, 0, 0, 0]); // record 1
        data.extend_from_slice(&[20, 8]); // truncated record 2
        let table = parse_ok(&data, 3, "truncated records parse");
        assert_eq!(table.records.len(), 1);
    }
}
