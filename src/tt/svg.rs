//! OpenType `SVG ` table parsing.
//!
//! This follows FreeType 2.14.3 `src/sfnt/ttsvg.c` for the table header,
//! document-list records, and glyph-range lookup.  SVG rendering is a separate
//! renderer concern; this module only owns the document bytes needed by
//! `FT_Load_Glyph`, `FT_Get_Glyph`, and the detached SVG glyph class.

use crate::error::FontError;

const SVG_TABLE_HEADER_SIZE: usize = 10;
const SVG_DOCUMENT_RECORD_SIZE: usize = 12;
const SVG_DOCUMENT_LIST_MINIMUM_SIZE: usize = 2 + SVG_DOCUMENT_RECORD_SIZE;
const SVG_MINIMUM_SIZE: usize = SVG_TABLE_HEADER_SIZE + SVG_DOCUMENT_LIST_MINIMUM_SIZE;

/// One document record from an OpenType `SVG ` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgDocumentRecord {
    /// First glyph covered by the document.
    pub start_glyph_id: u16,
    /// Last glyph covered by the document.
    pub end_glyph_id: u16,
    /// Uncompressed SVG document bytes.
    pub document: Vec<u8>,
}

/// Parsed OpenType `SVG ` table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SvgTable {
    /// Table version.
    pub version: u16,
    documents: Vec<SvgDocumentRecord>,
}

impl SvgTable {
    /// Parse an OpenType `SVG ` table.
    ///
    /// Gzip-compressed documents are deliberately rejected here.  The
    /// maintained parity fixture is plain XML; compressed document support is
    /// tracked independently from the SVG glyph-object contract.
    pub fn parse(data: &[u8]) -> Result<Self, FontError> {
        if data.len() < SVG_MINIMUM_SIZE {
            return Err(invalid_svg_table());
        }
        let version = read_u16(data, 0)?;
        // Every supported fontdone target has a 32- or 64-bit `usize`, so an
        // OpenType u32 offset is lossless.  Keeping this as a fallible
        // conversion would create a target-independent dead error path.
        let document_list_offset = read_u32(data, 2)? as usize;
        if document_list_offset < SVG_TABLE_HEADER_SIZE
            || document_list_offset > data.len().saturating_sub(SVG_DOCUMENT_LIST_MINIMUM_SIZE)
        {
            return Err(invalid_svg_table());
        }
        // The range check above proves this start offset is in bounds.
        let document_list = &data[document_list_offset..];
        let document_count = usize::from(read_u16(document_list, 0)?);
        // `document_count` is a u16, so this cannot overflow a supported
        // 32-bit (or wider) `usize`.
        let records_end = 2 + document_count * SVG_DOCUMENT_RECORD_SIZE;
        if records_end > document_list.len() {
            return Err(invalid_svg_table());
        }

        let mut documents = Vec::with_capacity(document_count);
        for index in 0..document_count {
            let record_offset = 2 + index * SVG_DOCUMENT_RECORD_SIZE;
            let start_glyph_id = read_u16(document_list, record_offset)?;
            let end_glyph_id = read_u16(document_list, record_offset + 2)?;
            let document_offset = read_u32(document_list, record_offset + 4)?;
            let document_length = read_u32(document_list, record_offset + 8)?;
            // Widen before addition so malformed u32 offset/length pairs have
            // one platform-independent bounds result on both 32- and 64-bit
            // targets.
            let document_end = u64::from(document_offset) + u64::from(document_length);
            if document_end > document_list.len() as u64 {
                return Err(invalid_svg_table());
            }
            let document_offset = document_offset as usize;
            let document_end = document_end as usize;
            let document = document_list
                .get(document_offset..document_end)
                .ok_or_else(invalid_svg_table)?;
            if document.starts_with(&[0x1f, 0x8b, 0x08]) {
                return Err(FontError::InvalidTable(
                    "SVG: gzip-compressed document unsupported".into(),
                ));
            }
            documents.push(SvgDocumentRecord {
                start_glyph_id,
                end_glyph_id,
                document: document.to_vec(),
            });
        }

        Ok(Self { version, documents })
    }

    /// Return the document whose inclusive range covers `glyph_index`.
    pub fn document_for_glyph(&self, glyph_index: u16) -> Option<&SvgDocumentRecord> {
        self.documents.iter().find(|document| {
            document.start_glyph_id <= glyph_index && glyph_index <= document.end_glyph_id
        })
    }
}

fn invalid_svg_table() -> FontError {
    FontError::InvalidTable("SVG: invalid table".into())
}

fn read_u16(data: &[u8], offset: usize) -> Result<u16, FontError> {
    let bytes = data
        .get(offset..offset.saturating_add(2))
        .ok_or_else(invalid_svg_table)?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Result<u32, FontError> {
    let bytes = data
        .get(offset..offset.saturating_add(4))
        .ok_or_else(invalid_svg_table)?;
    Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn one_document_table(start: u16, end: u16, document: &[u8]) -> Vec<u8> {
        let mut table = Vec::new();
        table.extend_from_slice(&0u16.to_be_bytes());
        table.extend_from_slice(&10u32.to_be_bytes());
        table.extend_from_slice(&0u32.to_be_bytes());
        table.extend_from_slice(&1u16.to_be_bytes());
        table.extend_from_slice(&start.to_be_bytes());
        table.extend_from_slice(&end.to_be_bytes());
        table.extend_from_slice(&14u32.to_be_bytes());
        table.extend_from_slice(&(document.len() as u32).to_be_bytes());
        table.extend_from_slice(document);
        table
    }

    #[test]
    fn parses_document_range_and_bytes() -> Result<(), FontError> {
        let document = b"<svg/>";
        let table = one_document_table(1, 2, document);

        let parsed = SvgTable::parse(&table)?;
        assert_eq!(parsed.version, 0);
        assert!(parsed.document_for_glyph(0).is_none());
        assert_eq!(
            parsed
                .document_for_glyph(1)
                .map(|record| record.document.as_slice()),
            Some(document.as_slice())
        );
        assert!(parsed.document_for_glyph(3).is_none());
        Ok(())
    }

    #[test]
    fn rejects_short_headers_and_invalid_document_list_ranges() {
        for len in 0..SVG_MINIMUM_SIZE {
            assert!(SvgTable::parse(&vec![0; len]).is_err(), "length {len}");
        }

        let mut before_header = one_document_table(1, 2, b"<svg/>");
        before_header[2..6].copy_from_slice(&9u32.to_be_bytes());
        assert!(SvgTable::parse(&before_header).is_err());

        let mut after_last_complete_list = one_document_table(1, 2, b"<svg/>");
        after_last_complete_list[2..6].copy_from_slice(&11u32.to_be_bytes());
        assert!(SvgTable::parse(&after_last_complete_list).is_err());

        let mut truncated_records = one_document_table(1, 2, b"<svg/>");
        truncated_records[10..12].copy_from_slice(&2u16.to_be_bytes());
        assert!(SvgTable::parse(&truncated_records).is_err());
    }

    #[test]
    fn rejects_out_of_range_and_gzip_documents() {
        let mut out_of_range = one_document_table(1, 2, b"<svg/>");
        out_of_range[16..20].copy_from_slice(&u32::MAX.to_be_bytes());
        out_of_range[20..24].copy_from_slice(&u32::MAX.to_be_bytes());
        assert!(SvgTable::parse(&out_of_range).is_err());

        let gzip = one_document_table(1, 2, &[0x1f, 0x8b, 0x08, 0]);
        assert_eq!(
            SvgTable::parse(&gzip),
            Err(FontError::InvalidTable(
                "SVG: gzip-compressed document unsupported".into()
            ))
        );
    }

    #[test]
    fn primitive_readers_reject_missing_bytes_at_extreme_offsets() {
        assert!(read_u16(&[], 0).is_err());
        assert!(read_u16(&[0, 1], usize::MAX).is_err());
        assert!(read_u32(&[], 0).is_err());
        assert!(read_u32(&[0, 1, 2, 3], usize::MAX).is_err());
    }
}
