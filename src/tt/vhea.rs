//! 'vhea' table — Vertical Header.
//!
//! Reference: `TT_VertHeader` in FreeType's `tttables.h`.

use crate::error::FontError;

/// Parsed 'vhea' table.
#[derive(Debug, Clone)]
pub struct VheaTable {
    /// Maximum vertical advance height in font units.
    pub advance_height_max: u16,
    /// Number of vmtx entries with explicit advance heights.
    pub num_vmetrics: u16,
}

/// Parse the 'vhea' table (36 bytes).
pub fn parse_vhea(data: &[u8]) -> Result<VheaTable, FontError> {
    if data.len() < 36 {
        return Err(FontError::InvalidFont(
            "vhea table too short (need 36 bytes)".into(),
        ));
    }
    Ok(VheaTable {
        advance_height_max: u16::from_be_bytes([data[10], data[11]]),
        num_vmetrics: u16::from_be_bytes([data[34], data[35]]),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_valid_vhea() -> Result<(), FontError> {
        let mut data = vec![0u8; 36];
        data[10..12].copy_from_slice(&1_800u16.to_be_bytes());
        data[34..36].copy_from_slice(&12u16.to_be_bytes());
        let table = parse_vhea(&data)?;
        assert_eq!(table.advance_height_max, 1_800);
        assert_eq!(table.num_vmetrics, 12);
        Ok(())
    }

    #[test]
    fn rejects_short_vhea() {
        let error = match parse_vhea(&[0u8; 35]) {
            Err(error) => error,
            Ok(_) => panic!("short vhea should be rejected"),
        };
        assert!(error.to_string().contains("vhea table too short"));
    }
}
