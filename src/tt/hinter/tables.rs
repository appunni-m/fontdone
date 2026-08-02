//! Table parsing for the bytecode hinter: fpgm, prep, cvt.
//!
//! C reference: `tt_face_load_cvt`, `tt_face_load_fpgm`, `tt_face_load_prep`
//! in `ttpload.c:295-505`.
//!
//! These tables are required for TrueType bytecode hinting:
//! - `cvt` (Control Value Table): array of FWORD values, scaled to 26.6
//! - `fpgm` (Font Program): bytecode executed once at face load
//! - `prep` (CVT Program): bytecode executed when ppem changes

use crate::error::FontError;

/// Parsed 'cvt ' table — array of control values in 26.6 format.
///
/// Each entry is a 16-bit signed FWORD from the font file, multiplied by 64
/// to convert from font units to 26.6 fixed-point. FreeType stores these as
/// `FT_Int32` values in 26.6.
pub fn parse_cvt(data: &[u8]) -> Result<Vec<i32>, FontError> {
    if data.is_empty() {
        return Ok(Vec::new());
    }
    if !data.len().is_multiple_of(2) {
        return Err(FontError::InvalidOutline(
            "cvt: table length must be even".into(),
        ));
    }

    let count = data.len() / 2;
    let mut cvt = Vec::with_capacity(count);

    for i in 0..count {
        let off = i * 2;
        let val = i16::from_be_bytes([data[off], data[off + 1]]) as i32;
        // Scale to 26.6: multiply by 64 (FT_GET_SHORT() * 64 in C)
        cvt.push(val * 64);
    }

    Ok(cvt)
}

/// Return the raw font-program bytecode.
pub fn parse_fpgm(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}

/// Return the raw control-value program bytecode.
pub fn parse_prep(data: &[u8]) -> Vec<u8> {
    data.to_vec()
}
