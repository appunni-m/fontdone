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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn short_format_locations() {
        // Short format stores offsets in 2-byte units.
        let loca = [0u16, 10, 20, 30];
        let bytes = loca
            .iter()
            .flat_map(|value| value.to_be_bytes())
            .collect::<Vec<_>>();
        let loc = match get_glyph_location(&bytes, 0, 0, 100) {
            Some(loc) => loc,
            None => panic!("glyph 0 location missing"),
        };
        assert_eq!(
            loc,
            GlyphLocation {
                offset: 0,
                length: 20
            }
        );
        let loc = match get_glyph_location(&bytes, 1, 0, 100) {
            Some(loc) => loc,
            None => panic!("glyph 1 location missing"),
        };
        assert_eq!(
            loc,
            GlyphLocation {
                offset: 20,
                length: 20
            }
        );
        // Missing record -> None.
        assert!(get_glyph_location(&bytes, 5, 0, 100).is_none());
    }

    #[test]
    fn long_format_locations() {
        let offsets = [0u32, 100, 250, 400];
        let bytes = offsets
            .iter()
            .flat_map(|value| value.to_be_bytes())
            .collect::<Vec<_>>();
        let loc = match get_glyph_location(&bytes, 0, 1, 500) {
            Some(loc) => loc,
            None => panic!("glyph 0 location missing"),
        };
        assert_eq!(
            loc,
            GlyphLocation {
                offset: 0,
                length: 100
            }
        );
        let loc = match get_glyph_location(&bytes, 2, 1, 500) {
            Some(loc) => loc,
            None => panic!("glyph 2 location missing"),
        };
        assert_eq!(
            loc,
            GlyphLocation {
                offset: 250,
                length: 150
            }
        );
        assert!(get_glyph_location(&bytes, 4, 1, 500).is_none());
    }

    #[test]
    fn out_of_range_and_truncation() {
        // This offset exceeds the glyf length -> empty glyph.
        let offsets = [600u32, 600];
        let bytes = offsets
            .iter()
            .flat_map(|value| value.to_be_bytes())
            .collect::<Vec<_>>();
        let loc = match get_glyph_location(&bytes, 0, 1, 500) {
            Some(loc) => loc,
            None => panic!("glyph 0 location missing"),
        };
        assert_eq!(
            loc,
            GlyphLocation {
                offset: 0,
                length: 0
            }
        );

        // Earlier oversized next offset -> empty glyph.
        let earlier = [0u8, 1, 0, 20, 0, 21];
        let loc = get_glyph_location(&earlier, 0, 0, 16);
        assert_eq!(
            loc,
            Some(GlyphLocation {
                offset: 0,
                length: 0
            })
        );

        // Final glyph's next offset is truncated to the glyf length.
        let offsets = [0u32, 600];
        let bytes = offsets
            .iter()
            .flat_map(|value| value.to_be_bytes())
            .collect::<Vec<_>>();
        let loc = match get_glyph_location(&bytes, 0, 1, 500) {
            Some(loc) => loc,
            None => panic!("final glyph location missing"),
        };
        assert_eq!(
            loc,
            GlyphLocation {
                offset: 0,
                length: 500
            }
        );

        // Unordered entries use the remaining glyf bytes as length.
        let offsets = [100u32, 50, 50];
        let bytes = offsets
            .iter()
            .flat_map(|value| value.to_be_bytes())
            .collect::<Vec<_>>();
        let loc = match get_glyph_location(&bytes, 0, 1, 500) {
            Some(loc) => loc,
            None => panic!("unordered glyph location missing"),
        };
        assert_eq!(
            loc,
            GlyphLocation {
                offset: 100,
                length: 400
            }
        );
    }
}
