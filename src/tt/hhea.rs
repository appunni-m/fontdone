//! 'hhea' table — Horizontal Header. Mirrors `tt_load_hhea`.
//!
//! Reference: `src/sfnt/ttload.c`, `TT_HoriHeader` in `tttables.h`.

use crate::error::FontError;

/// Parsed 'hhea' table.
#[derive(Debug, Clone)]
pub struct HheaTable {
    /// Typographic ascent (font units, positive up).
    pub ascent: i16,
    /// Typographic descent (font units, negative down).
    pub descent: i16,
    /// Typographic line gap.
    pub line_gap: i16,
    /// Maximum horizontal advance width in font units.
    pub advance_width_max: u16,
    /// Number of hmtx entries with explicit advance widths.
    pub num_hmetrics: u16,
}

/// Parse the 'hhea' table (36 bytes).
pub fn parse_hhea(data: &[u8]) -> Result<HheaTable, FontError> {
    if data.len() < 36 {
        return Err(FontError::InvalidFont(
            "hhea table too short (need 36 bytes)".into(),
        ));
    }
    Ok(HheaTable {
        ascent: i16::from_be_bytes([data[4], data[5]]),
        descent: i16::from_be_bytes([data[6], data[7]]),
        line_gap: i16::from_be_bytes([data[8], data[9]]),
        advance_width_max: u16::from_be_bytes([data[10], data[11]]),
        num_hmetrics: u16::from_be_bytes([data[34], data[35]]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_hhea() -> Result<(), FontError> {
        let mut data = vec![0u8; 36];
        data[4..6].copy_from_slice(&800i16.to_be_bytes());
        data[6..8].copy_from_slice(&(-200i16).to_be_bytes());
        data[8..10].copy_from_slice(&0i16.to_be_bytes());
        data[10..12].copy_from_slice(&1500u16.to_be_bytes());
        data[34..36].copy_from_slice(&18u16.to_be_bytes());
        let table = parse_hhea(&data)?;
        assert_eq!(table.ascent, 800);
        assert_eq!(table.descent, -200);
        assert_eq!(table.line_gap, 0);
        assert_eq!(table.advance_width_max, 1500);
        assert_eq!(table.num_hmetrics, 18);
        Ok(())
    }

    #[test]
    fn rejects_short_hhea() {
        let error = match parse_hhea(&[0u8; 35]) {
            Err(error) => error,
            Ok(_) => panic!("short hhea should be rejected"),
        };
        assert!(error.to_string().contains("hhea table too short"));
    }
}
