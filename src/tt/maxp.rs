//! 'maxp' table — maximum profile. Mirrors `tt_load_maxp`.
//!
//! Reference: `src/sfnt/ttload.c`, `TT_MaxProfile` in `tttables.h`.

use crate::error::FontError;

/// Parsed 'maxp' table.
#[derive(Debug, Clone, Default)]
pub struct MaxpTable {
    /// Total number of glyphs (including glyph 0 / .notdef).
    pub num_glyphs: u16,
    /// Maximum points in a simple glyph (used for buffer sizing).
    pub max_points: u16,
    /// Maximum contours in a simple glyph.
    pub max_contours: u16,
    /// Number of twilight-zone points available to TrueType bytecode.
    pub max_twilight_points: u16,
    /// Number of storage area locations available to TrueType bytecode.
    pub max_storage: u16,
    /// Maximum number of function definitions (`FDEF`) the bytecode may define.
    pub max_function_defs: u16,
    /// Maximum number of instruction definitions (`IDEF`) the bytecode may define.
    pub max_instruction_defs: u16,
    /// Declared maximum TrueType interpreter operand-stack depth.
    pub max_stack_elements: u16,
    /// Maximum component depth for composite glyphs.
    pub max_component_depth: u16,
}

/// Parse the `maxp` table.
pub fn parse_maxp(data: &[u8]) -> Result<MaxpTable, FontError> {
    if data.len() < 6 {
        // sfnt_load_face ignores tt_face_load_maxp errors and continues with
        // its zero-initialized profile.
        return Ok(MaxpTable::default());
    }
    let version = u32::from_be_bytes([data[0], data[1], data[2], data[3]]);
    let num_glyphs = u16::from_be_bytes([data[4], data[5]]);

    // FreeType's tt_face_load_maxp reads only the six-byte header below
    // version 1.0 and a complete 26-byte extra frame otherwise.
    let (
        max_points,
        max_contours,
        max_twilight_points,
        max_storage,
        mut max_function_defs,
        max_instruction_defs,
        max_stack_elements,
        max_component_depth,
    ) = if version >= 0x0001_0000 {
        if data.len() < 32 {
            return Err(FontError::InvalidFont(
                "maxp version 1 table too short (need 32 bytes)".into(),
            ));
        }
        (
            u16::from_be_bytes([data[6], data[7]]),
            u16::from_be_bytes([data[8], data[9]]),
            u16::from_be_bytes([data[16], data[17]]),
            u16::from_be_bytes([data[18], data[19]]),
            u16::from_be_bytes([data[20], data[21]]),
            u16::from_be_bytes([data[22], data[23]]),
            u16::from_be_bytes([data[24], data[25]]),
            u16::from_be_bytes([data[30], data[31]]),
        )
    } else {
        (0, 0, 0, 0, 0, 0, 0, 0)
    };
    // FreeType `tt_face_load_maxp` (`src/sfnt/ttload.c`) allocates at least
    // 64 FDEF entries for broken fonts whose `maxFunctionDefs` is smaller.
    if version >= 0x0001_0000 && max_function_defs < 64 {
        max_function_defs = 64;
    }
    Ok(MaxpTable {
        num_glyphs,
        max_points,
        max_contours,
        max_twilight_points,
        max_storage,
        max_function_defs,
        max_instruction_defs,
        max_stack_elements,
        max_component_depth,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_version_one_profile() -> Result<(), FontError> {
        let mut data = vec![0u8; 32];
        data[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
        data[4..6].copy_from_slice(&50u16.to_be_bytes()); // num_glyphs
        data[6..8].copy_from_slice(&100u16.to_be_bytes()); // max_points
        data[8..10].copy_from_slice(&10u16.to_be_bytes()); // max_contours
        data[16..18].copy_from_slice(&20u16.to_be_bytes()); // twilight points
        data[18..20].copy_from_slice(&30u16.to_be_bytes()); // storage
        data[20..22].copy_from_slice(&12u16.to_be_bytes()); // function defs
        data[22..24].copy_from_slice(&8u16.to_be_bytes()); // instruction defs
        data[24..26].copy_from_slice(&64u16.to_be_bytes()); // stack
        data[30..32].copy_from_slice(&4u16.to_be_bytes()); // component depth
        let table = parse_maxp(&data)?;
        assert_eq!(table.num_glyphs, 50);
        assert_eq!(table.max_points, 100);
        assert_eq!(table.max_contours, 10);
        assert_eq!(table.max_twilight_points, 20);
        assert_eq!(table.max_storage, 30);
        assert_eq!(table.max_function_defs, 64); // floored at 64
        assert_eq!(table.max_instruction_defs, 8);
        assert_eq!(table.max_stack_elements, 64);
        assert_eq!(table.max_component_depth, 4);
        Ok(())
    }

    #[test]
    fn version_zero_returns_zero_profile() -> Result<(), FontError> {
        let mut data = vec![0u8; 6];
        data[0..4].copy_from_slice(&0x0000_5000u32.to_be_bytes());
        data[4..6].copy_from_slice(&42u16.to_be_bytes());
        let table = parse_maxp(&data)?;
        assert_eq!(table.num_glyphs, 42);
        assert_eq!(table.max_points, 0);
        assert_eq!(table.max_stack_elements, 0);
        Ok(())
    }

    #[test]
    fn short_table_defaults_and_v1_shortness() {
        // < 6 bytes returns the default profile without an error.
        let table = match parse_maxp(&[0u8; 5]) {
            Ok(table) => table,
            Err(error) => panic!("short maxp should default, got {error}"),
        };
        assert_eq!(table.num_glyphs, 0);

        // Version 1 with fewer than 32 bytes is an error.
        let mut data = vec![0u8; 20];
        data[0..4].copy_from_slice(&0x0001_0000u32.to_be_bytes());
        let error = match parse_maxp(&data) {
            Err(error) => error,
            Ok(_) => panic!("short v1 maxp should fail"),
        };
        assert!(error.to_string().contains("maxp version 1"));
    }
}
