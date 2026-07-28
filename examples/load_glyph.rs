#![allow(
    unused_crate_dependencies,
    reason = "standalone example inherits the package-wide dev dependency graph"
)]
//! Load and render a glyph explicitly through the safe FreeType-shaped API.

use std::error::Error;
use std::io;

use fontdone::{Library, LoadFlags};

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "usage: load_glyph FONT"))?;
    let data = std::fs::read(path)?;
    let mut face = Library::init().new_memory_face(&data, 0, 16.0)?;
    face.set_pixel_sizes(0, 16);
    let glyph_index = face.get_char_index(u32::from('A'));
    let slot = face.load_glyph(glyph_index, LoadFlags::RENDER)?;
    let bitmap = slot
        .bitmap
        .as_ref()
        .ok_or_else(|| io::Error::other("rendered slot has no bitmap"))?;
    println!(
        "glyph={} format={:?} bitmap={}x{} pitch={}",
        slot.glyph_index, slot.format, bitmap.width, bitmap.rows, bitmap.pitch
    );
    Ok(())
}
