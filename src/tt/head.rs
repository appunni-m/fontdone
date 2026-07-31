//! 'head' table — font header. Mirrors `tt_load_head` field offsets.
//!
//! Reference: `src/sfnt/ttload.c`, `TT_Header` in `include/freetype/tttables.h`.

use crate::error::FontError;

/// Parsed 'head' table (the fields reachable from the rendering path).
#[derive(Debug, Clone)]
pub struct HeadTable {
    /// Font design units per em-square (typically 1000 or 2048).
    pub units_per_em: u16,
    /// Font bounding box minimum x in design units.
    pub x_min: i16,
    /// Font bounding box minimum y in design units.
    pub y_min: i16,
    /// Font bounding box maximum x in design units.
    pub x_max: i16,
    /// Font bounding box maximum y in design units.
    pub y_max: i16,
    /// Format of the 'loca' table: 0 = short, 1 = long.
    pub index_to_loc_format: i16,
    /// Font flags (bit 0 baseline-at-y0, etc.).
    pub flags: u16,
    /// Macintosh style flags (bit 0=bold, bit 1=italic).
    pub mac_style: u16,
    /// Lowest recPPEM (smallest size the font is designed for).
    pub lowest_rec_ppem: u16,
}

/// Parse the 'head' table from raw bytes (54 bytes minimum).
pub fn parse_head(data: &[u8]) -> Result<HeadTable, FontError> {
    if data.len() < 54 {
        return Err(FontError::InvalidFont(
            "head table too short (need 54 bytes)".into(),
        ));
    }
    let units_per_em = u16::from_be_bytes([data[18], data[19]]);
    let flags = u16::from_be_bytes([data[16], data[17]]);
    let x_min = i16::from_be_bytes([data[36], data[37]]);
    let y_min = i16::from_be_bytes([data[38], data[39]]);
    let x_max = i16::from_be_bytes([data[40], data[41]]);
    let y_max = i16::from_be_bytes([data[42], data[43]]);
    let mac_style = u16::from_be_bytes([data[44], data[45]]);
    let lowest_rec_ppem = u16::from_be_bytes([data[46], data[47]]);
    let index_to_loc_format = i16::from_be_bytes([data[50], data[51]]);

    // FreeType 2.14.3 applies the OpenType Units_Per_EM limits while
    // initializing an SFNT face (`sfobjs.c`), before glyph loading or
    // auto-hinting can observe the value.
    if !(16..=16_384).contains(&units_per_em) {
        return Err(FontError::InvalidTable(format!(
            "head: units_per_em {units_per_em} is outside 16..=16384"
        )));
    }

    Ok(HeadTable {
        units_per_em,
        x_min,
        y_min,
        x_max,
        y_max,
        index_to_loc_format,
        flags,
        mac_style,
        lowest_rec_ppem,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn head_with_units_per_em(units_per_em: u16) -> [u8; 54] {
        let mut head = [0_u8; 54];
        head[18..20].copy_from_slice(&units_per_em.to_be_bytes());
        head
    }

    #[test]
    fn rejects_units_per_em_outside_pinned_freetype_limits_as_invalid_table() {
        for units_per_em in [0, 15, 16_385, u16::MAX] {
            assert!(matches!(
                parse_head(&head_with_units_per_em(units_per_em)),
                Err(FontError::InvalidTable(_))
            ));
        }
    }

    #[test]
    fn accepts_units_per_em_at_pinned_freetype_limits() {
        for units_per_em in [16, 16_384] {
            assert!(matches!(
                parse_head(&head_with_units_per_em(units_per_em)),
                Ok(HeadTable {
                    units_per_em: parsed_units_per_em,
                    ..
                }) if parsed_units_per_em == units_per_em
            ));
        }
    }

    #[test]
    fn parses_all_public_fields() -> Result<(), FontError> {
        let mut head = [0_u8; 54];
        head[16..18].copy_from_slice(&0x0003u16.to_be_bytes()); // flags
        head[18..20].copy_from_slice(&1000u16.to_be_bytes()); // units_per_em
        head[36..38].copy_from_slice(&(-100i16).to_be_bytes());
        head[38..40].copy_from_slice(&(-200i16).to_be_bytes());
        head[40..42].copy_from_slice(&1500i16.to_be_bytes());
        head[42..44].copy_from_slice(&1700i16.to_be_bytes());
        head[44..46].copy_from_slice(&1u16.to_be_bytes()); // mac_style
        head[46..48].copy_from_slice(&8u16.to_be_bytes()); // lowest_rec_ppem
        head[50..52].copy_from_slice(&1i16.to_be_bytes()); // long loca
        let table = parse_head(&head)?;
        assert_eq!(table.units_per_em, 1000);
        assert_eq!(table.flags, 3);
        assert_eq!(table.x_min, -100);
        assert_eq!(table.y_min, -200);
        assert_eq!(table.x_max, 1500);
        assert_eq!(table.y_max, 1700);
        assert_eq!(table.mac_style, 1);
        assert_eq!(table.lowest_rec_ppem, 8);
        assert_eq!(table.index_to_loc_format, 1);
        Ok(())
    }

    #[test]
    fn rejects_short_head() {
        let error = match parse_head(&[0u8; 53]) {
            Err(error) => error,
            Ok(_) => panic!("short head should be rejected"),
        };
        assert!(error.to_string().contains("head table too short"));
    }
}
