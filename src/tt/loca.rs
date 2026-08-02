//! 'loca' table — glyph offsets into 'glyf'.
//!
//! Mirrors `tt_face_get_location` in `src/sfnt/ttload.c`.

/// `(offset, length)` of a glyph's data inside the 'glyf' table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphLocation {
    pub offset: u32,
    pub length: u32,
}

/// Resolve a glyph's location from the 'loca' table.
///
/// `index_to_loc_format` is from the 'head' table (0 = short, 1 = long).
/// Returns `Some` with `length == 0` for the empty (space-like) glyph slot.
pub fn get_glyph_location(
    loca: &[u8],
    glyph_index: u16,
    index_to_loc_format: i16,
    glyf_len: usize,
) -> Option<GlyphLocation> {
    let idx = glyph_index as usize;
    let (this, mut next, num_locations) = if index_to_loc_format == 0 {
        let off = idx * 2;
        let record = loca.get(off..off + 4)?;
        let this = u16::from_be_bytes([record[0], record[1]]) as u32 * 2;
        let next = u16::from_be_bytes([record[2], record[3]]) as u32 * 2;
        (this, next, loca.len() / 2)
    } else {
        let off = idx * 4;
        let record = loca.get(off..off + 8)?;
        let this = u32::from_be_bytes([record[0], record[1], record[2], record[3]]);
        let next = u32::from_be_bytes([record[4], record[5], record[6], record[7]]);
        (this, next, loca.len() / 4)
    };

    let glyf_len = u32::try_from(glyf_len).unwrap_or(u32::MAX);
    // FreeType 2.14.3 `tt_face_get_location` treats locations outside the
    // `glyf` table as empty glyphs.  It only truncates an oversized next
    // offset for the final glyph; earlier oversized next offsets are empty.
    if this > glyf_len {
        return Some(GlyphLocation {
            offset: 0,
            length: 0,
        });
    }
    if next > glyf_len {
        if idx == num_locations.saturating_sub(2) {
            next = glyf_len;
        } else {
            return Some(GlyphLocation {
                offset: 0,
                length: 0,
            });
        }
    }
    Some(GlyphLocation {
        offset: this,
        // FreeType deliberately uses the remaining `glyf` bytes as an upper
        // bound for unordered `loca` entries instead of treating them empty.
        length: if next >= this {
            next - this
        } else {
            glyf_len - this
        },
    })
}
