//! Minimal OpenType `sbix` table support for public face flags, fixed-size
//! strikes, and glyph-load error parity.

use crate::casts::i16_from_i32;
use crate::error::FontError;
use crate::fixed::{ft_div_fix, ft_mul_fix};
use crate::tt::{TableDirectory, tag};

#[derive(Debug, Clone)]
pub struct SbixTable {
    /// `sbix` header flags; bit 1 selects outline overlay.
    pub flags: u16,
    raw: Vec<u8>,
    strikes: Vec<SbixStrike>,
}

#[derive(Debug, Clone, Copy)]
struct SbixStrike {
    /// Offset of the strike record relative to the start of the `sbix` table.
    offset: u32,
    ppem: u16,
    ppi: u16,
}

#[derive(Debug, Clone, Copy)]
pub struct SbixStrikeMetrics {
    pub x_ppem: u16,
    pub y_ppem: u16,
    pub height: i16,
}

pub fn parse_sbix(directory: &TableDirectory, data: &[u8]) -> Option<SbixTable> {
    let table = directory.find(data, tag(b"sbix"))?;
    if table.len() < 8 {
        return None;
    }
    let version = read_u16(table, 0)?;
    let flags = read_u16(table, 2)?;
    let declared_strikes = read_u32(table, 4)?;
    // Pinned `sfnt/ttsbit.c` requires version >= 1 and flags 1 or 3; all
    // other bit combinations make the table invalid.
    if version < 1 || !matches!(flags, 1 | 3) || declared_strikes >= 0x1_0000 {
        return None;
    }
    let physical_strikes = (table.len().saturating_sub(8) / 4) as u32;
    let strike_count = declared_strikes.min(physical_strikes);
    let mut strikes = Vec::with_capacity(strike_count as usize);
    for index in 0..strike_count as usize {
        let offset = read_u32(table, 8 + index * 4)? as usize;
        let record = table.get(offset..)?;
        let (ppem, ppi) = (read_u16(record, 0)?, read_u16(record, 2)?);
        strikes.push(SbixStrike {
            offset: offset as u32,
            ppem,
            ppi,
        });
    }
    Some(SbixTable {
        flags,
        raw: table.to_vec(),
        strikes,
    })
}

impl SbixTable {
    pub fn flags(&self) -> u16 {
        self.flags
    }

    pub fn strike_count(&self) -> usize {
        self.strikes.len()
    }

    pub fn has_strike(&self, ppem: u16) -> bool {
        self.strikes.iter().any(|strike| strike.ppem == ppem)
    }

    /// Build the `FT_Bitmap_Size` height used by `sfnt/sfobjs.c` from the
    /// strike ppem and the face horizontal metrics.
    pub fn strike_metrics(
        &self,
        index: usize,
        hhea_ascent: i16,
        hhea_descent: i16,
        hhea_line_gap: i16,
        units_per_em: u16,
    ) -> Option<SbixStrikeMetrics> {
        let strike = *self.strikes.get(index)?;
        let scale = ft_div_fix(i32::from(strike.ppem) << 6, i32::from(units_per_em));
        let height = ft_mul_fix(
            i32::from(hhea_ascent) - i32::from(hhea_descent) + i32::from(hhea_line_gap),
            scale,
        );
        Some(SbixStrikeMetrics {
            x_ppem: strike.ppem,
            y_ppem: strike.ppem,
            height: i16_from_i32(height >> 6),
        })
    }

    /// Replicate pinned `sfnt/ttsbit.c:tt_face_load_sbix_image` public error
    /// behavior for the selected strike.  The pinned build has PNG decoding
    /// disabled, so a `png ` record is reported as `Unimplemented_Feature`;
    /// missing records report `Missing_Bitmap`.
    pub fn load_glyph_error(
        &self,
        glyph_index: u16,
        num_glyphs: u16,
        ppem: u16,
        recurse_count: u32,
    ) -> Result<(), FontError> {
        if recurse_count > 4 {
            return Err(FontError::InvalidFileFormat(
                "sbix duplicate/flip recursion too deep".into(),
            ));
        }
        let Some(strike) = self.strikes.iter().find(|strike| strike.ppem == ppem) else {
            return Err(FontError::InvalidArgument(
                "embedded bitmap strike not selected".into(),
            ));
        };
        if glyph_index > num_glyphs {
            return Err(FontError::InvalidArgument(
                "glyph index out of range".into(),
            ));
        }
        let strike_offset = usize::try_from(strike.offset).unwrap_or(usize::MAX);
        // The glyph-data-offset array immediately follows the ppem/ppi pair
        // in the strike record; each value is relative to the strike start.
        let glyph_data_base = strike_offset
            .checked_add(4)
            .ok_or_else(|| FontError::InvalidFileFormat("sbix offset overflow".into()))?;
        let glyph_data_end = glyph_data_base
            .checked_add(4)
            .and_then(|end| end.checked_add(4 * usize::from(glyph_index)))
            .and_then(|end| end.checked_add(8))
            .ok_or_else(|| FontError::InvalidFileFormat("sbix glyph range overflow".into()))?;
        if glyph_data_end > self.raw.len() {
            return Err(FontError::InvalidFileFormat(
                "sbix glyph range too short".into(),
            ));
        }
        let start = read_u32(&self.raw, glyph_data_base + 4 * usize::from(glyph_index))
            .ok_or_else(|| FontError::InvalidFileFormat("sbix glyph offset missing".into()))?
            as usize;
        let end = read_u32(
            &self.raw,
            glyph_data_base + 4 * usize::from(glyph_index) + 4,
        )
        .ok_or_else(|| FontError::InvalidFileFormat("sbix glyph end offset missing".into()))?
            as usize;
        if start == end {
            return Err(FontError::MissingBitmap);
        }
        if start > end
            || end - start < 8
            || strike_offset
                .checked_add(end)
                .is_none_or(|x| x > self.raw.len())
        {
            return Err(FontError::InvalidFileFormat(
                "sbix image range invalid".into(),
            ));
        }
        let record_base = strike_offset
            .checked_add(start)
            .ok_or_else(|| FontError::InvalidFileFormat("sbix image offset overflow".into()))?;
        let record = self
            .raw
            .get(record_base..record_base + 8)
            .ok_or_else(|| FontError::InvalidFileFormat("sbix image header missing".into()))?;
        let graphic_type = [record[4], record[5], record[6], record[7]];
        match &graphic_type {
            b"flip" | b"dupe" => {
                let payload = self.raw.get(record_base + 8..).ok_or_else(|| {
                    FontError::InvalidFileFormat("sbix flip/dupe payload missing".into())
                })?;
                let target = read_u16(payload, 0).ok_or_else(|| {
                    FontError::InvalidFileFormat("sbix flip/dupe glyph missing".into())
                })?;
                self.load_glyph_error(target, num_glyphs, ppem, recurse_count + 1)
            }
            b"png " => Err(FontError::UnimplementedFeature(
                "sbix PNG decoding is disabled in the pinned build".into(),
            )),
            b"jpg " | b"tiff" | b"rgbl" => Err(FontError::UnknownFileFormat(
                "sbix graphic type requires an unavailable decoder".into(),
            )),
            _ => Err(FontError::UnimplementedFeature(
                "unsupported sbix graphic type".into(),
            )),
        }
    }
}

fn read_u16(data: &[u8], offset: usize) -> Option<u16> {
    let bytes = data.get(offset..offset + 2)?;
    Some(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], offset: usize) -> Option<u32> {
    let bytes = data.get(offset..offset + 4)?;
    Some(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tt::TableRecord;

    fn directory_for(table: &[u8]) -> (Vec<u8>, TableDirectory) {
        // Lay the sbix table at offset 12 and build a one-record directory.
        let mut font = vec![0u8; 12];
        font.extend_from_slice(table);
        let directory = TableDirectory {
            records: vec![TableRecord {
                tag: tag(b"sbix"),
                offset: 12,
                length: table.len() as u32,
            }],
        };
        (font, directory)
    }

    /// One-glyph strike: ppem/ppi header, glyph offset pair, then a payload
    /// of the given graphic type at strike-relative offset 16.
    fn strike_with_glyph(ppem: u16, ppi: u16, graphic_type: &[u8; 4]) -> Vec<u8> {
        let mut bytes = ppem.to_be_bytes().to_vec();
        bytes.extend_from_slice(&ppi.to_be_bytes());
        bytes.extend_from_slice(&16u32.to_be_bytes()); // glyph start
        bytes.extend_from_slice(&32u32.to_be_bytes()); // glyph end
        bytes.extend_from_slice(&[0; 4]); // glyph record: size
        bytes.extend_from_slice(&[0; 4]); // origin x/y
        bytes.extend_from_slice(graphic_type);
        bytes.extend_from_slice(&[0; 12]); // remainder, keeps end offset in range
        bytes
    }

    fn sbix_table(strikes: &[Vec<u8>]) -> Vec<u8> {
        let mut table = Vec::new();
        table.extend_from_slice(&1u16.to_be_bytes()); // version
        table.extend_from_slice(&1u16.to_be_bytes()); // flags
        table.extend_from_slice(&(strikes.len() as u32).to_be_bytes());
        let mut offset = 8u32 + 4 * strikes.len() as u32;
        for _ in strikes {
            table.extend_from_slice(&offset.to_be_bytes());
            offset += 8 + 4 + 4 + 8;
        }
        for strike in strikes {
            table.extend_from_slice(strike);
        }
        table
    }

    fn parse_ok(font: &[u8], directory: &TableDirectory, label: &str) -> SbixTable {
        match parse_sbix(directory, font) {
            Some(table) => table,
            None => panic!("{label}: sbix table rejected"),
        }
    }

    #[test]
    fn parses_strikes_and_metrics() {
        let table = sbix_table(&[strike_with_glyph(24, 72, b"png ")]);
        let (font, directory) = directory_for(&table);
        let sbix = parse_ok(&font, &directory, "valid sbix parses");
        assert_eq!(sbix.flags(), 1);
        assert_eq!(sbix.strike_count(), 1);
        assert!(sbix.has_strike(24));
        assert!(!sbix.has_strike(16));
        let metrics = match sbix.strike_metrics(0, 800, -200, 0, 1000) {
            Some(metrics) => metrics,
            None => panic!("strike metrics missing"),
        };
        assert_eq!(metrics.x_ppem, 24);
        assert_eq!(metrics.y_ppem, 24);
        assert!(metrics.height > 0);
        assert!(sbix.strike_metrics(3, 800, -200, 0, 1000).is_none());
    }

    #[test]
    fn rejects_invalid_headers() {
        let mut table = sbix_table(&[strike_with_glyph(24, 72, b"png ")]);
        table[0..2].copy_from_slice(&0u16.to_be_bytes());
        let (font, directory) = directory_for(&table);
        assert!(parse_sbix(&directory, &font).is_none());

        let mut table = sbix_table(&[strike_with_glyph(24, 72, b"png ")]);
        table[2..4].copy_from_slice(&2u16.to_be_bytes());
        let (font, directory) = directory_for(&table);
        assert!(parse_sbix(&directory, &font).is_none());

        let (font, directory) = directory_for(&[0u8; 4]);
        assert!(parse_sbix(&directory, &font).is_none());
    }

    #[test]
    fn load_glyph_error_paths() {
        let table = sbix_table(&[strike_with_glyph(24, 72, b"jpg ")]);
        let (font, directory) = directory_for(&table);
        let sbix = parse_ok(&font, &directory, "valid sbix parses");

        let error = match sbix.load_glyph_error(0, 10, 16, 0) {
            Err(error) => error,
            Ok(()) => panic!("missing strike should fail"),
        };
        assert!(error.to_string().contains("strike not selected"));
        let error = match sbix.load_glyph_error(11, 10, 24, 0) {
            Err(error) => error,
            Ok(()) => panic!("out-of-range glyph should fail"),
        };
        assert!(error.to_string().contains("glyph index out of range"));
        let error = match sbix.load_glyph_error(0, 10, 24, 0) {
            Err(error) => error,
            Ok(()) => panic!("jpg should fail"),
        };
        assert!(error.to_string().contains("unavailable decoder"));
        let error = match sbix.load_glyph_error(0, 10, 24, 5) {
            Err(error) => error,
            Ok(()) => panic!("deep recursion should fail"),
        };
        assert!(error.to_string().contains("recursion too deep"));

        // Zero-length glyph range reports Missing_Bitmap.
        let mut table = sbix_table(&[strike_with_glyph(24, 72, b"png ")]);
        table[16..20].copy_from_slice(&0u32.to_be_bytes());
        table[20..24].copy_from_slice(&0u32.to_be_bytes());
        let (font, directory) = directory_for(&table);
        let sbix = parse_ok(&font, &directory, "valid sbix parses");
        let error = match sbix.load_glyph_error(0, 10, 24, 0) {
            Err(error) => error,
            Ok(()) => panic!("empty image should fail"),
        };
        assert!(error.to_string().contains("Missing"));
    }

    #[test]
    fn graphic_type_dispatch() {
        for (graphic, expected) in [
            (b"png ", "PNG decoding is disabled"),
            (b"rgbl", "unavailable decoder"),
        ] {
            let table = sbix_table(&[strike_with_glyph(24, 72, graphic)]);
            let (font, directory) = directory_for(&table);
            let sbix = parse_ok(&font, &directory, "valid sbix parses");
            let error = match sbix.load_glyph_error(0, 10, 24, 0) {
                Err(error) => error,
                Ok(()) => panic!("graphic type should fail"),
            };
            assert!(
                error.to_string().contains(expected),
                "graphic {graphic:?} expected {expected:?}, got {error}"
            );
        }

        // A flip record whose payload glyph is out of range reports the
        // missing-glyph error; a flip to a valid glyph recurses to the depth
        // limit instead.
        let mut flip_strike = strike_with_glyph(24, 72, b"flip");
        // Overwrite the payload's glyph index (strike-relative 24) with an
        // out-of-range value.
        flip_strike[24..26].copy_from_slice(&99u16.to_be_bytes());
        let table = sbix_table(&[flip_strike]);
        let (font, directory) = directory_for(&table);
        let sbix = parse_ok(&font, &directory, "valid sbix parses");
        let error = match sbix.load_glyph_error(0, 10, 24, 0) {
            Err(error) => error,
            Ok(()) => panic!("flip should fail"),
        };
        assert!(error.to_string().contains("glyph index out of range"));
    }
}
