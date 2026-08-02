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
        let version = read_u16(data, 0);
        // Every supported fontdone target has a 32- or 64-bit `usize`, so an
        // OpenType u32 offset is lossless.  Keeping this as a fallible
        // conversion would create a target-independent dead error path.
        let document_list_offset = read_u32(data, 2) as usize;
        if document_list_offset < SVG_TABLE_HEADER_SIZE
            || document_list_offset > data.len().saturating_sub(SVG_DOCUMENT_LIST_MINIMUM_SIZE)
        {
            return Err(invalid_svg_table());
        }
        // The range check above proves this start offset is in bounds.
        let document_list = &data[document_list_offset..];
        let document_count = usize::from(read_u16(document_list, 0));
        // `document_count` is a u16, so this cannot overflow a supported
        // 32-bit (or wider) `usize`.
        let records_end = 2 + document_count * SVG_DOCUMENT_RECORD_SIZE;
        if records_end > document_list.len() {
            return Err(invalid_svg_table());
        }

        let mut documents = Vec::with_capacity(document_count);
        for index in 0..document_count {
            let record_offset = 2 + index * SVG_DOCUMENT_RECORD_SIZE;
            let start_glyph_id = read_u16(document_list, record_offset);
            let end_glyph_id = read_u16(document_list, record_offset + 2);
            let document_offset = read_u32(document_list, record_offset + 4);
            let document_length = read_u32(document_list, record_offset + 8);
            // Widen before addition so malformed u32 offset/length pairs have
            // one platform-independent bounds result on both 32- and 64-bit
            // targets.
            let document_end = u64::from(document_offset) + u64::from(document_length);
            if document_end > document_list.len() as u64 {
                return Err(invalid_svg_table());
            }
            let document_offset = document_offset as usize;
            let document_end = document_end as usize;
            // The widened bounds check proves both indices are representable
            // and that this range is valid on every supported usize width.
            let document = &document_list[document_offset..document_end];
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

fn read_u16(data: &[u8], offset: usize) -> u16 {
    // Callers establish the table/list/record bounds before reading.
    let bytes = &data[offset..offset + 2];
    u16::from_be_bytes([bytes[0], bytes[1]])
}

fn read_u32(data: &[u8], offset: usize) -> u32 {
    // Callers establish the table/list/record bounds before reading.
    let bytes = &data[offset..offset + 4];
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}
