#![allow(
    unused_crate_dependencies,
    reason = "standalone example inherits the package-wide dev dependency graph"
)]
//! Exercise the safe Rust facade with an explicit FreeType-style lifecycle.

use std::error::Error;
use std::io;

use fontdone::ffi::*;

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "usage: ffi_migration FONT"))?;
    let bytes = std::fs::read(path)?;

    let library = FT_Init_FreeType();
    let mut face = FT_New_Memory_Face(&library, &bytes, 0, 16.0)
        .map_err(|error| io::Error::other(format!("FT_New_Memory_Face: {error}")))?;
    let error = FT_Set_Pixel_Sizes(&mut face, 0, 16);
    if error != FT_Err_Ok as FT_Error {
        return Err(io::Error::other(format!("FT_Set_Pixel_Sizes: {error}")).into());
    }

    let glyph_index = FT_Get_Char_Index(&face, FT_ULong::from('A' as u32));
    let slot = FT_Load_Glyph(&face, glyph_index, FT_LOAD_DEFAULT)
        .map_err(|error| io::Error::other(format!("FT_Load_Glyph: {error}")))?;
    let slot = FT_Render_Glyph(slot, FT_RENDER_MODE_NORMAL)
        .map_err(|error| io::Error::other(format!("FT_Render_Glyph: {error}")))?;
    let bitmap = slot
        .bitmap
        .as_ref()
        .ok_or_else(|| io::Error::other("rendered slot has no bitmap"))?;
    println!(
        "glyph={} bitmap={}x{} pitch={} advance={}",
        slot.glyph_index, bitmap.width, bitmap.rows, bitmap.pitch, slot.advance.x
    );

    if FT_Done_Face(Some(face)) != FT_Err_Ok as FT_Error {
        return Err(io::Error::other("FT_Done_Face failed").into());
    }
    if FT_Done_FreeType(Some(library)) != FT_Err_Ok as FT_Error {
        return Err(io::Error::other("FT_Done_FreeType failed").into());
    }
    Ok(())
}
