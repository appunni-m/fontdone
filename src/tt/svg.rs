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
        let document_list_offset =
            usize::try_from(read_u32(data, 2)?).map_err(|_| invalid_svg_table())?;
        if document_list_offset < SVG_TABLE_HEADER_SIZE
            || document_list_offset > data.len().saturating_sub(SVG_DOCUMENT_LIST_MINIMUM_SIZE)
        {
            return Err(invalid_svg_table());
        }
        let document_list = data
            .get(document_list_offset..)
            .ok_or_else(invalid_svg_table)?;
        let document_count = usize::from(read_u16(document_list, 0)?);
        let records_end = 2usize
            .checked_add(
                document_count
                    .checked_mul(SVG_DOCUMENT_RECORD_SIZE)
                    .ok_or_else(invalid_svg_table)?,
            )
            .ok_or_else(invalid_svg_table)?;
        if records_end > document_list.len() {
            return Err(invalid_svg_table());
        }

        let mut documents = Vec::with_capacity(document_count);
        for index in 0..document_count {
            let record_offset = 2 + index * SVG_DOCUMENT_RECORD_SIZE;
            let start_glyph_id = read_u16(document_list, record_offset)?;
            let end_glyph_id = read_u16(document_list, record_offset + 2)?;
            let document_offset = usize::try_from(read_u32(document_list, record_offset + 4)?)
                .map_err(|_| invalid_svg_table())?;
            let document_length = usize::try_from(read_u32(document_list, record_offset + 8)?)
                .map_err(|_| invalid_svg_table())?;
            let document_end = document_offset
                .checked_add(document_length)
                .ok_or_else(invalid_svg_table)?;
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

    #[test]
    fn parses_document_range_and_bytes() -> Result<(), FontError> {
        let document = b"<svg/>";
        let mut table = Vec::new();
        table.extend_from_slice(&0u16.to_be_bytes());
        table.extend_from_slice(&10u32.to_be_bytes());
        table.extend_from_slice(&0u32.to_be_bytes());
        table.extend_from_slice(&1u16.to_be_bytes());
        table.extend_from_slice(&1u16.to_be_bytes());
        table.extend_from_slice(&2u16.to_be_bytes());
        table.extend_from_slice(&14u32.to_be_bytes());
        table.extend_from_slice(&(document.len() as u32).to_be_bytes());
        table.extend_from_slice(document);

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
}
