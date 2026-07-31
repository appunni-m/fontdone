//! Auto-hinter: snaps glyph edges to pixel grid for readability at small sizes.
//!
//! # Pipeline (per dimension, HORZ then VERT)
//!
//! 1. `reload` → coords + direction chain + WEAK/STRONG classify
//! 2. `compute_segments` → horizontal/vertical runs
//! 3. `compute_edges` → merge overlapping segments
//! 4. `compute_blue_edges` → assign to baseline/cap-height zones
//! 5. `hint_edges` → 4-phase snap: stems → serifs → blues → anchors
//! 6. `align_edge_points` → snap contour points to hinted edges
//! 7. `align_strong_points` → grid-fit corners (skips WEAK)
//! 8. `align_weak_points` (IUP) → interpolate smooth runs
//! 9. phantom adjust → pixel-grid shift via pp1.x
//!
//! # WEAK/STRONG classification
//!
//! See `reload` and `build_direction_chain` in `loader.rs`. Wrong flag here
//! cascades: skipped point → wrong IUP ref → 1-2 unit drift → pixel mismatch.
//!
//! # Font categories
//!
//! | Category | `near_limit` | Behavior |
//! |----------|-------------|----------|
//! | UPEM=2048 | 20 FU | Sparse chain, classification clear |
//! | UPEM=1000 | 9 FU | Dense chain, more merges, fragile |
//! | Italic | 20 FU | NO_HORIZONTAL (skips X-axis) |
//!
//! Reference: `freetype/src/autofit/` (VER-2-14-1).

pub mod blue_strings;
pub mod cjk;
pub mod coverage;
pub mod globals;
pub mod globals_data;
pub mod latin;
pub mod loader;
pub mod types;

pub use globals::FaceGlobals;
pub use globals_data::{STYLE_FALLBACK, STYLE_TABLE, STYLE_UNASSIGNED, StyleClass, UniRange};
pub use latin::apply_hints;
pub use latin::{metrics_init_blues_impl, metrics_init_widths};
pub use types::{
    AF_LATIN_MAX_WIDTHS, AFEdge, AFPoint, AFSegment, AfLatinAxisMetrics, AfLatinBlue,
    AfLatinMetrics, AfWidth, AxisHints, Dimension, Direction, GlyphHints,
};

#[cfg(test)]
mod tests {
    const FIXTURES: &[&str] = &[
        "DejaVuSans.ttf",
        "autohint/basic-latin.ttf",
        "autohint/latin-blue-delta.ttf",
        "autohint/latin-blue-edge-cases.ttf",
        "autohint/latin-blue-overlap.ttf",
        "autohint/latin-empty-standard.ttf",
        "autohint/latin-greek-cyrillic.ttf",
        "autohint/latin-low-upem.ttf",
        "autohint/latin-many-widths.ttf",
        "autohint/latin-missing-standard.ttf",
        "autohint/latin-remaining-topology.ttf",
        "autohint/latin-small-ignore.ttf",
        "autohint/latin-width-clusters.ttf",
        "autohint/latin-x-height-rejection.ttf",
        "autohint/cjk-coverage.ttf",
        "autohint/cjk-blue-edge-cases.ttf",
        "autohint/cjk-duplicate-edge.ttf",
        "autohint/cjk-empty-standard.ttf",
        "autohint/cjk-many-widths.ttf",
        "autohint/cjk-multi-width-snap.ttf",
        "autohint/cjk-quantized-widths.ttf",
        "autohint/cjk-remaining-branches.ttf",
        "autohint/cjk-round-stem-light.ttf",
        "autohint/cjk-snap-below-standard.ttf",
        "autohint/cjk-tiny-stem.ttf",
        "autohint/cjk-wide-stem-snap.ttf",
        "autohint/cjk-width-order.ttf",
        "autohint/arabic-neutral-first.ttf",
        "autohint/arabic-neutral-round-skip.ttf",
        "autohint/arabic-standard-fallback.ttf",
        "autohint/indic-coverage.ttf",
        "autohint/khmer-sub-top-overlap.ttf",
        "autohint/mixed-script-map.ttf",
        "autohint/script-coverage.ttf",
    ];

    fn scale_all_mapped_glyphs(data: &[u8], size_pt: f32) {
        let font = match crate::Font::truetype(data, size_pt) {
            Ok(font) => font,
            Err(err) => panic!("fixture should load at {size_pt}pt: {err}"),
        };
        let mut current = font.first_char();
        let mut scaled_count = 0usize;
        while let Some((char_code, _)) = current {
            let glyph = font.char_index(char_code);
            let metrics = font.face_globals.get_metrics(glyph);
            let scaled = crate::scaler::scale_glyph_for_metrics_with_autohint(
                &font.data,
                glyph,
                metrics.as_deref(),
                font.is_italic,
            );
            match scaled {
                Ok(scaled) => {
                    assert!(
                        scaled.advance_width >= 0 && scaled.slot_advance_width >= 0,
                        "advance must stay non-negative at {size_pt}pt"
                    );
                    scaled_count += 1;
                }
                Err(err) => {
                    panic!("glyph {glyph} (U+{char_code:04X}) should scale at {size_pt}pt: {err}")
                }
            }
            current = font.next_char(char_code);
        }
        assert!(
            scaled_count > 0,
            "fixture must expose at least one cmap-mapped glyph"
        );
    }

    #[test]
    fn autohint_fixtures_scale_every_mapped_glyph_at_two_sizes() {
        for name in FIXTURES {
            let data = match std::fs::read(
                std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                    .join(format!("tests/fixtures/input/fonts/{name}")),
            ) {
                Ok(data) => data,
                Err(err) => panic!("fixture {name} should be readable: {err}"),
            };
            scale_all_mapped_glyphs(&data, 12.0);
            scale_all_mapped_glyphs(&data, 16.0);
        }
    }
}
