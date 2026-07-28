use std::error::Error;
use std::io;

use fontdone::{Font, FontError, Library, LoadFlags};

fn main() -> Result<(), Box<dyn Error>> {
    let path = std::env::args()
        .nth(1)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "missing font path"))?;
    let bytes = std::fs::read(path)?;

    let font = Font::truetype(&bytes, 16.0)?;
    let mask = font.getmask("A")?;
    if mask.pixels.len() != (mask.width as usize) * (mask.height as usize) {
        return Err(io::Error::other("compact mask layout is inconsistent").into());
    }

    let mut face = Library::init().new_memory_face(&bytes, 0, 16.0)?;
    face.set_pixel_sizes(0, 16);
    let glyph = face.get_char_index(u32::from('A'));
    let slot = face.load_glyph(glyph, LoadFlags::RENDER)?;
    if slot.bitmap.is_none() {
        return Err(io::Error::other("explicit glyph load produced no bitmap").into());
    }

    match Font::truetype(b"invalid", 16.0) {
        Err(FontError::InvalidFont(_)) => {}
        Err(other) => return Err(other.into()),
        Ok(_) => return Err(io::Error::other("invalid bytes opened").into()),
    }
    println!(
        "external Rust consumer: mask={}x{}, glyph={glyph}",
        mask.width, mask.height
    );
    Ok(())
}
