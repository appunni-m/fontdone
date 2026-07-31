//! SFNT / TrueType table parsing.
//!
//! Faithful ports of the table-loading paths in FreeType's
//! `src/sfnt/ttload.c`, `src/sfnt/ttcmap.c`, and `src/truetype/ttgload.c`
//! for the subset needed by the FreeType-compatible renderer (head, maxp, hhea,
//! hmtx, kern, cmap, name, loca, glyf, OS/2).

pub mod cff;
pub mod cmap;
pub mod fvar;
pub mod gasp;
pub mod glyf;
pub mod gvar;
pub mod hdmx;
pub mod head;
pub mod hhea;
pub mod hinter;
pub mod hmtx;
pub mod hvar;
pub mod kern;
pub mod loca;
pub mod maxp;
pub mod mvar;
pub mod name;
pub mod os2;
pub mod post;
pub mod sbit;
pub mod sbix;
pub mod svg;
pub mod varstore;
pub mod vhea;
pub mod vmtx;

use crate::error::FontError;

/// Magic bytes identifying an OpenType font with CFF outlines.
pub const OTTO_MAGIC: u32 = 0x4F54_544F; // "OTTO"
/// Magic bytes identifying a TrueType font.
pub const TRUE_MAGIC: u32 = 0x0001_0000;

/// A reference to a single font table within the raw data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TableRecord {
    /// 4-byte table tag (e.g. `b"cmap"`).
    pub tag: u32,
    /// Byte offset from start of font data.
    pub offset: u32,
    /// Length in bytes.
    pub length: u32,
}

/// Parsed table directory — maps table tags to their data slices.
#[derive(Debug, Clone)]
pub struct TableDirectory {
    /// Individual table records, in order of appearance.
    pub records: Vec<TableRecord>,
}

impl TableDirectory {
    /// Look up a table by its 4-byte tag, returning a slice into the font data.
    pub fn find<'a>(&self, data: &'a [u8], tag: u32) -> Option<&'a [u8]> {
        for record in &self.records {
            if record.tag == tag {
                let start = record.offset as usize;
                let end = start + record.length as usize;
                return data.get(start..end);
            }
        }
        None
    }

    /// Look up a table record by its 4-byte tag.
    pub fn record(&self, tag: u32) -> Option<TableRecord> {
        self.records
            .iter()
            .copied()
            .find(|record| record.tag == tag)
    }
}

/// Parse the TrueType/OpenType offset table + table directory.
///
/// Mirrors `tt_face_load_font_dir` in `src/sfnt/ttload.c`: read the sfVersion,
/// then `numTables` directory records of 16 bytes each.
pub fn parse_table_directory(data: &[u8]) -> Result<TableDirectory, FontError> {
    parse_table_directory_at(data, 0)
}

/// Parse a TrueType/OpenType table directory at an absolute byte offset.
pub fn parse_table_directory_at(data: &[u8], base: usize) -> Result<TableDirectory, FontError> {
    let font = data
        .get(base..)
        .ok_or_else(|| FontError::InvalidFont("font offset out of range".into()))?;
    if font.len() < 12 {
        return Err(FontError::InvalidFont(
            "data too short for offset table (need 12 bytes)".into(),
        ));
    }

    let sf_version = read_u32(font, 0);
    if sf_version != TRUE_MAGIC && sf_version != OTTO_MAGIC {
        return Err(FontError::InvalidFont(format!(
            "unknown sfVersion: 0x{:08X}",
            sf_version
        )));
    }

    let num_tables = read_u16(font, 4) as usize;
    if num_tables == 0 {
        // FreeType 2.14.3 reads the SFNT offset table, then rejects both
        // TrueType and OTTO directories with no valid entries during face
        // open (`sfnt/ttload.c`: `tt_face_load_font_dir`/`check_table_dir`).
        // The public status is `FT_Err_Invalid_Stream_Operation`.
        return Err(FontError::SfntZeroTablesStreamOperation);
    }
    let dir_start = 12usize;
    let dir_end = dir_start + num_tables * 16;
    if font.len() < dir_end {
        return Err(FontError::InvalidFont(format!(
            "data too short for {num_tables} table records"
        )));
    }

    let mut records = Vec::with_capacity(num_tables);
    for i in 0..num_tables {
        let off = dir_start + i * 16;
        records.push(TableRecord {
            tag: read_u32(font, off),
            // In TTC files, table offsets remain absolute from the collection
            // start (ttload.c:tt_face_load_font_dir); don't add the face base.
            offset: read_u32(font, off + 8),
            length: read_u32(font, off + 12),
        });
    }

    Ok(TableDirectory { records })
}

/// Face offsets for either a single SFNT face or a TrueType collection.
pub fn face_offsets(data: &[u8]) -> Result<Vec<usize>, FontError> {
    if data.len() < 4 {
        return Err(FontError::InvalidFont(
            "data too short for SFNT header".into(),
        ));
    }
    if &data[0..4] != b"ttcf" {
        parse_table_directory(data)?;
        return Ok(vec![0]);
    }
    if data.len() < 12 {
        return Err(FontError::InvalidFont(
            "TTC header too short (need 12 bytes)".into(),
        ));
    }
    let num_faces = read_u32(data, 8) as usize;
    // FreeType 2.14.3 `sfnt_open_font` (`src/sfnt/sfobjs.c`) routes a TTC
    // face-count whose offset array cannot fit in the stream through
    // `FT_Err_Array_Too_Large` before attempting allocation.
    let Some(offset_bytes) = num_faces.checked_mul(4) else {
        return Err(FontError::ArrayTooLarge);
    };
    let Some(offset_table_end) = 12usize.checked_add(offset_bytes) else {
        return Err(FontError::ArrayTooLarge);
    };
    if data.len() < offset_table_end {
        return Err(FontError::ArrayTooLarge);
    }
    let mut offsets = Vec::with_capacity(num_faces);
    for i in 0..num_faces {
        offsets.push(read_u32(data, 12 + i * 4) as usize);
    }
    Ok(offsets)
}

/// Return `(face_count, selected_face_offset)` for FreeType-like face index handling.
pub fn resolve_face_index(data: &[u8], face_index: usize) -> Result<(usize, usize), FontError> {
    let offsets = face_offsets(data)?;
    let count = offsets.len();
    let offset = offsets.get(face_index).copied().ok_or_else(|| {
        FontError::InvalidFont(format!(
            "face index {face_index} out of range for {count} face(s)"
        ))
    })?;
    Ok((count, offset))
}

/// Build a u32 tag from 4 ASCII bytes. E.g. `tag(b"cmap")` = `0x636D6170`.
#[inline]
pub const fn tag(bytes: &[u8; 4]) -> u32 {
    u32::from_be_bytes(*bytes)
}

// ── big-endian primitives ──────────────────────────────────────────────────

#[inline]
pub(crate) fn read_u16(data: &[u8], offset: usize) -> u16 {
    u16::from_be_bytes([data[offset], data[offset + 1]])
}

#[inline]
pub(crate) fn read_i16(data: &[u8], offset: usize) -> i16 {
    i16::from_be_bytes([data[offset], data[offset + 1]])
}

#[inline]
pub(crate) fn read_u32(data: &[u8], offset: usize) -> u32 {
    u32::from_be_bytes([
        data[offset],
        data[offset + 1],
        data[offset + 2],
        data[offset + 3],
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ttf_with_tables(tables: &[(&[u8; 4], &[u8])]) -> Vec<u8> {
        let mut data = Vec::new();
        data.extend_from_slice(&TRUE_MAGIC.to_be_bytes());
        data.extend_from_slice(&(tables.len() as u16).to_be_bytes());
        data.extend_from_slice(&[0; 6]); // search fields
        let mut offset = 12 + tables.len() * 16;
        for (tag_bytes, bytes) in tables {
            data.extend_from_slice(*tag_bytes);
            data.extend_from_slice(&0u32.to_be_bytes()); // checksum
            data.extend_from_slice(&(offset as u32).to_be_bytes());
            data.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
            offset += bytes.len();
        }
        for (_, bytes) in tables {
            data.extend_from_slice(bytes);
        }
        data
    }

    #[test]
    fn parses_table_directory_and_lookup() -> Result<(), FontError> {
        let data = ttf_with_tables(&[(b"cmap", &[1, 2, 3]), (b"glyf", &[4, 5, 6, 7])]);
        let directory = parse_table_directory(&data)?;
        assert_eq!(directory.records.len(), 2);
        assert_eq!(directory.find(&data, tag(b"cmap")), Some(&[1, 2, 3][..]));
        assert_eq!(directory.find(&data, tag(b"glyf")), Some(&[4, 5, 6, 7][..]));
        assert!(directory.find(&data, tag(b"head")).is_none());
        assert_eq!(
            directory.record(tag(b"cmap")).map(|r| r.tag),
            Some(tag(b"cmap"))
        );
        assert!(directory.record(tag(b"head")).is_none());
        Ok(())
    }

    #[test]
    fn rejects_bad_directories() {
        assert!(parse_table_directory(&[0u8; 11]).is_err());
        let mut data = ttf_with_tables(&[(b"cmap", &[1])]);
        data[0..4].copy_from_slice(&0xDEAD_BEEFu32.to_be_bytes());
        assert!(parse_table_directory(&data).is_err());
        let mut data = ttf_with_tables(&[(b"cmap", &[1])]);
        data[4..6].copy_from_slice(&0u16.to_be_bytes());
        assert!(parse_table_directory(&data).is_err());
    }

    #[test]
    fn face_offsets_handles_single_and_collection() -> Result<(), FontError> {
        let single = ttf_with_tables(&[(b"cmap", &[1])]);
        assert_eq!(face_offsets(&single)?, vec![0]);

        let mut ttc = b"ttcf".to_vec();
        ttc.extend_from_slice(&0x0001_0000u32.to_be_bytes());
        ttc.extend_from_slice(&2u32.to_be_bytes());
        ttc.extend_from_slice(&0x100u32.to_be_bytes());
        ttc.extend_from_slice(&0x200u32.to_be_bytes());
        assert_eq!(face_offsets(&ttc)?, vec![0x100, 0x200]);
        Ok(())
    }

    #[test]
    fn resolve_face_index_and_errors() -> Result<(), FontError> {
        let single = ttf_with_tables(&[(b"cmap", &[1])]);
        assert_eq!(resolve_face_index(&single, 0)?, (1, 0));
        let error = match resolve_face_index(&single, 1) {
            Err(error) => error,
            Ok(_) => panic!("out-of-range face index should fail"),
        };
        assert!(error.to_string().contains("out of range"));
        Ok(())
    }

    #[test]
    fn primitives_read_big_endian() {
        assert_eq!(read_u16(&[0x12, 0x34], 0), 0x1234);
        assert_eq!(read_i16(&[0xFF, 0x9C], 0), -100);
        assert_eq!(read_u32(&[0xDE, 0xAD, 0xBE, 0xEF], 0), 0xDEAD_BEEF);
    }
}
