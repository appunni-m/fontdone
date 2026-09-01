//! 'glyf' table — glyph outline data (simple + composite).
//!
//! Faithful port of the decode paths in `src/truetype/ttgload.c`:
//! `TT_Load_Simple_Glyph` (flag/delta coordinate decoding) and the composite
//! component loop (`TT_Load_Composite_Glyph`). Output is a flattened list of
//! contours with on/off-curve tags, in font design units.

use crate::casts::{u16_from_i16, u16_from_u32, u32_from_usize};

use crate::error::FontError;
use crate::fixed::{ft_div_fix, ft_mul_fix};
use crate::outline::OUTLINE_OVERLAP;
use crate::tt::loca::get_glyph_location;

// Simple glyph flag bits (TrueType spec, ttgload.c:53).
const ON_CURVE: u8 = 0x01;
const X_SHORT_VECTOR: u8 = 0x02;
const Y_SHORT_VECTOR: u8 = 0x04;
const REPEAT_FLAG: u8 = 0x08;
const X_IS_SAME_OR_POSITIVE_SHORT: u8 = 0x10;
const Y_IS_SAME_OR_POSITIVE_SHORT: u8 = 0x20;
const OVERLAP_SIMPLE: u8 = 0x40;

// Composite glyph flag bits (ttgload.c:69).
const ARG_1_AND_2_ARE_WORDS: u16 = 0x0001;
const ARGS_ARE_XY_VALUES: u16 = 0x0002;
const ROUND_XY_TO_GRID: u16 = 0x0004;
const WE_HAVE_A_SCALE: u16 = 0x0008;
const MORE_COMPONENTS: u16 = 0x0020;
const WE_HAVE_AN_X_Y_SCALE: u16 = 0x0040;
const WE_HAVE_A_TWO_BY_TWO: u16 = 0x0080;
const WE_HAVE_INSTRUCTIONS: u16 = 0x0100;
const USE_MY_METRICS: u16 = 0x0200;
const OVERLAP_COMPOUND: u16 = 0x0400;

/// A single decoded outline point in font design units.
#[derive(Debug, Clone, Copy)]
pub struct OutlinePoint {
    pub x: i32,
    pub y: i32,
    /// `true` for on-curve points, `false` for off-curve (control) points.
    pub on_curve: bool,
    /// Original TrueType point flag byte reused by FreeType as outline tag.
    pub tag: u8,
}

/// A TrueType point coordinate in unscaled 26.6 font units.
#[derive(Debug, Clone, Copy)]
pub struct UnroundedPoint {
    pub x: i32,
    pub y: i32,
}

/// A CFF/Type 2 outline coordinate in Adobe interpreter 16.16 units.
#[derive(Debug, Clone, Copy)]
pub struct CffFixedPoint {
    pub x: i32,
    pub y: i32,
}

/// A glyph outline as a flattened list of contours.
#[derive(Debug, Clone, Default)]
pub struct GlyphOutline {
    pub num_contours: u16,
    pub end_pts_of_contours: Vec<u16>,
    pub points: Vec<OutlinePoint>,
    /// FreeType's variable-glyph sidecar coordinates after `gvar` deltas.
    ///
    /// `TT_Vary_Apply_Glyph_Deltas` keeps integer `outline.points` and a
    /// parallel unrounded 26.6 array. Scaled variable glyph loads use this
    /// sidecar before hinting; public no-scale outline state uses `points`.
    pub unrounded_points: Option<Vec<UnroundedPoint>>,
    /// CFF Adobe-interpreter coordinates before the builder's 16.16-to-26.6
    /// truncation.  Hinted CFF loads need these fractions because the Adobe
    /// scale is applied before that truncation; unhinted loads use `points`.
    pub cff_fixed_points: Option<Vec<CffFixedPoint>>,
    pub xmin: i32,
    pub ymin: i32,
    pub xmax: i32,
    pub ymax: i32,
    /// Original glyf header bbox xMin for this glyph. Composite loading keeps
    /// `xmin` as the final subglyph cache used for pp1.x compatibility.
    pub bbox_xmin: i32,
    /// Original glyf header bbox yMax used to seed the vertical phantom
    /// point. Variation deltas change the outline CBox but do not rewrite the
    /// loader's header bbox (`ttgload.c:324,1339`).
    pub bbox_ymax: i32,
    /// Whether composite. If true, xmin tracks last sub-glyph's glyf header
    /// (matching C's loader->bbox ttgload.c:324) and lsb tracks last sub's.
    pub is_composite: bool,
    pub sub_lsb: i32,
    /// TrueType bytecode instructions for this glyph (from glyf table).
    /// Empty for glyphs with no instructions.
    pub instructions: Vec<u8>,
    /// Component records for composite glyphs. Empty for simple glyphs.
    pub components: Vec<CompositeComponent>,
    /// FreeType `FT_Outline.flags` bits carried by the glyph loader.
    pub outline_flags: u32,
    /// True for Type2/CFF outlines whose off-curve points can be cubic.
    pub has_cubic_tags: bool,
}

/// A 2×2 fixed-point transform for a composite component (16.16).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Affine {
    pub xx: i32,
    pub xy: i32,
    pub yx: i32,
    pub yy: i32,
}

impl Affine {
    const IDENTITY: Self = Self {
        xx: 0x1_0000,
        xy: 0,
        yx: 0,
        yy: 0x1_0000,
    };
}

/// One component of a composite glyph.
#[derive(Debug, Clone)]
pub struct CompositeComponent {
    pub glyph_index: u16,
    /// Raw TrueType component flags as stored in the glyf table.
    pub flags: u16,
    /// Translation args (font units when ARGS_ARE_XY_VALUES).
    pub arg1: i32,
    pub arg2: i32,
    pub args_are_xy: bool,
    pub transform: Affine,
    pub round_xy_to_grid: bool,
    pub use_my_metrics: bool,
}

struct CompositeGlyph {
    components: Vec<CompositeComponent>,
    /// Offset immediately after the component records. FreeType records this
    /// position in `TT_Load_Composite_Glyph` and does not read it until
    /// `TT_Process_Composite_Glyph` has confirmed that the assembled glyph has
    /// points to hint (`ttgload.c:1902-1913`).
    instruction_pos: Option<usize>,
}

/// Load a glyph outline from 'glyf'/'loca', resolving composite glyphs recursively.
///
/// `depth` guards against malformed recursive composites.
pub fn load_glyph(
    glyf: &[u8],
    loca: &[u8],
    index_to_loc_format: i16,
    glyph_index: u16,
    hmtx: &crate::tt::hmtx::HmtxTable,
) -> Result<GlyphOutline, FontError> {
    load_glyph_inner(
        glyf,
        loca,
        index_to_loc_format,
        glyph_index,
        hmtx,
        GlyphLoadOptions {
            load_composite_instructions: true,
            ..GlyphLoadOptions::default()
        },
    )
}

/// Load a glyph while applying the active `gvar` records at each recursion
/// level.  Simple glyph records describe contour points; composite records
/// describe synthetic component points, so variation must be applied while
/// the component list is still available instead of after flattening.
pub(crate) fn load_glyph_with_variations(
    glyf: &[u8],
    loca: &[u8],
    index_to_loc_format: i16,
    glyph_index: u16,
    hmtx: &crate::tt::hmtx::HmtxTable,
    gvar: &crate::tt::gvar::GvarTable,
    normalized_coords: &[i32],
) -> Result<GlyphOutline, FontError> {
    load_glyph_inner(
        glyf,
        loca,
        index_to_loc_format,
        glyph_index,
        hmtx,
        GlyphLoadOptions {
            load_composite_instructions: true,
            variation: Some(VariationContext {
                gvar,
                normalized_coords,
            }),
            ..GlyphLoadOptions::default()
        },
    )
}

/// Load an unhinted glyph without reading trailing composite instructions.
///
/// FreeType records the composite instruction position while decoding the
/// component list, but only reads the length and bytecode when the load is
/// hinted (`ttgload.c`).  This distinction is observable for malformed
/// trailing instruction data under `FT_LOAD_NO_HINTING` and
/// `FT_LOAD_NO_SCALE`.
pub fn load_glyph_no_hinting(
    glyf: &[u8],
    loca: &[u8],
    index_to_loc_format: i16,
    glyph_index: u16,
    hmtx: &crate::tt::hmtx::HmtxTable,
) -> Result<GlyphOutline, FontError> {
    load_glyph_inner(
        glyf,
        loca,
        index_to_loc_format,
        glyph_index,
        hmtx,
        GlyphLoadOptions::default(),
    )
}

pub(crate) fn load_glyph_no_hinting_with_variations(
    glyf: &[u8],
    loca: &[u8],
    index_to_loc_format: i16,
    glyph_index: u16,
    hmtx: &crate::tt::hmtx::HmtxTable,
    gvar: &crate::tt::gvar::GvarTable,
    normalized_coords: &[i32],
) -> Result<GlyphOutline, FontError> {
    load_glyph_inner(
        glyf,
        loca,
        index_to_loc_format,
        glyph_index,
        hmtx,
        GlyphLoadOptions {
            variation: Some(VariationContext {
                gvar,
                normalized_coords,
            }),
            ..GlyphLoadOptions::default()
        },
    )
}

pub fn load_glyph_with_scaled_component_offsets(
    glyf: &[u8],
    loca: &[u8],
    index_to_loc_format: i16,
    glyph_index: u16,
    hmtx: &crate::tt::hmtx::HmtxTable,
    x_scale: i32,
    y_scale: i32,
) -> Result<GlyphOutline, FontError> {
    load_glyph_inner(
        glyf,
        loca,
        index_to_loc_format,
        glyph_index,
        hmtx,
        GlyphLoadOptions {
            component_offset_scale: Some((x_scale, y_scale)),
            load_composite_instructions: true,
            ..GlyphLoadOptions::default()
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn load_glyph_with_scaled_component_offsets_and_variations(
    glyf: &[u8],
    loca: &[u8],
    index_to_loc_format: i16,
    glyph_index: u16,
    hmtx: &crate::tt::hmtx::HmtxTable,
    x_scale: i32,
    y_scale: i32,
    gvar: &crate::tt::gvar::GvarTable,
    normalized_coords: &[i32],
) -> Result<GlyphOutline, FontError> {
    load_glyph_inner(
        glyf,
        loca,
        index_to_loc_format,
        glyph_index,
        hmtx,
        GlyphLoadOptions {
            component_offset_scale: Some((x_scale, y_scale)),
            load_composite_instructions: true,
            variation: Some(VariationContext {
                gvar,
                normalized_coords,
            }),
            ..GlyphLoadOptions::default()
        },
    )
}

/// Load a glyph outline scaled to 26.6 for the TrueType no-hinting path.
///
/// FreeType scales simple subglyphs and component offsets independently while
/// resolving composites.  This preserves that rounding instead of flattening
/// in font units and scaling the summed coordinates later. The scaler calls
/// this only after `load_glyph` validates the same complete component tree.
pub fn load_glyph_scaled_no_hinting(
    glyf: &[u8],
    loca: &[u8],
    index_to_loc_format: i16,
    glyph_index: u16,
    hmtx: &crate::tt::hmtx::HmtxTable,
    x_scale: i32,
    y_scale: i32,
) -> Result<GlyphOutline, FontError> {
    load_glyph_scaled_no_hinting_inner(
        glyf,
        loca,
        index_to_loc_format,
        glyph_index,
        hmtx,
        x_scale,
        y_scale,
        false,
    )
}

/// Load a no-hinting outline for an active variation instance without a
/// `gvar` table. FreeType keeps an unrounded 26.6 copy of simple points for
/// every non-default TrueType variation instance, so even a font whose
/// variation is expressed only through `fvar` must use the final rounded
/// `FT_MulFix` conversion rather than scaling integer font-unit points first.
pub(crate) fn load_glyph_scaled_no_hinting_with_active_variation(
    glyf: &[u8],
    loca: &[u8],
    index_to_loc_format: i16,
    glyph_index: u16,
    hmtx: &crate::tt::hmtx::HmtxTable,
    x_scale: i32,
    y_scale: i32,
) -> Result<GlyphOutline, FontError> {
    load_glyph_scaled_no_hinting_inner(
        glyf,
        loca,
        index_to_loc_format,
        glyph_index,
        hmtx,
        x_scale,
        y_scale,
        true,
    )
}

#[allow(clippy::too_many_arguments)]
fn load_glyph_scaled_no_hinting_inner(
    glyf: &[u8],
    loca: &[u8],
    index_to_loc_format: i16,
    glyph_index: u16,
    hmtx: &crate::tt::hmtx::HmtxTable,
    x_scale: i32,
    y_scale: i32,
    unrounded_variation: bool,
) -> Result<GlyphOutline, FontError> {
    load_glyph_inner(
        glyf,
        loca,
        index_to_loc_format,
        glyph_index,
        hmtx,
        GlyphLoadOptions {
            point_scale: Some((x_scale, y_scale)),
            unrounded_variation,
            ..GlyphLoadOptions::default()
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn load_glyph_scaled_no_hinting_with_variations(
    glyf: &[u8],
    loca: &[u8],
    index_to_loc_format: i16,
    glyph_index: u16,
    hmtx: &crate::tt::hmtx::HmtxTable,
    x_scale: i32,
    y_scale: i32,
    gvar: &crate::tt::gvar::GvarTable,
    normalized_coords: &[i32],
) -> Result<GlyphOutline, FontError> {
    load_glyph_inner(
        glyf,
        loca,
        index_to_loc_format,
        glyph_index,
        hmtx,
        GlyphLoadOptions {
            point_scale: Some((x_scale, y_scale)),
            unrounded_variation: true,
            variation: Some(VariationContext {
                gvar,
                normalized_coords,
            }),
            ..GlyphLoadOptions::default()
        },
    )
}

#[derive(Clone, Copy)]
struct VariationContext<'a> {
    gvar: &'a crate::tt::gvar::GvarTable,
    normalized_coords: &'a [i32],
}

#[derive(Clone, Copy, Default)]
struct GlyphLoadOptions<'a> {
    depth: u8,
    component_offset_scale: Option<(i32, i32)>,
    point_scale: Option<(i32, i32)>,
    load_composite_instructions: bool,
    unrounded_variation: bool,
    variation: Option<VariationContext<'a>>,
}

fn load_glyph_inner<'a>(
    glyf: &[u8],
    loca: &[u8],
    index_to_loc_format: i16,
    glyph_index: u16,
    hmtx: &crate::tt::hmtx::HmtxTable,
    options: GlyphLoadOptions<'a>,
) -> Result<GlyphOutline, FontError> {
    let GlyphLoadOptions {
        depth,
        component_offset_scale,
        point_scale,
        load_composite_instructions,
        unrounded_variation,
        variation,
    } = options;
    if depth > 100 {
        return Err(FontError::InvalidOutline(
            "glyf: composite recursion too deep".into(),
        ));
    }

    let loc = get_glyph_location(loca, glyph_index, index_to_loc_format, glyf.len()).ok_or_else(
        || {
            // FreeType 2.14.3 `tt_glyph_load` rejects an out-of-range public
            // request as Invalid_Argument before `load_truetype_glyph`; a bad
            // component index is Invalid_Composite (`ttdriver.c`, `ttgload.c`).
            if depth == 0 {
                FontError::InvalidArgument("loca: glyph index out of range".into())
            } else {
                FontError::InvalidComposite
            }
        },
    )?;
    if loc.length == 0 {
        return Ok(GlyphOutline::default());
    }

    let bytes = glyf
        .get(loc.offset as usize..loc.offset as usize + loc.length as usize)
        .ok_or_else(|| FontError::InvalidOutline("glyf: data out of range".into()))?;
    if bytes.len() < 10 {
        return Err(FontError::InvalidOutline("glyf: glyph too short".into()));
    }

    let num_contours = i16::from_be_bytes([bytes[0], bytes[1]]);
    let xmin = i16::from_be_bytes([bytes[2], bytes[3]]) as i32;
    let ymin = i16::from_be_bytes([bytes[4], bytes[5]]) as i32;
    let xmax = i16::from_be_bytes([bytes[6], bytes[7]]) as i32;
    let ymax = i16::from_be_bytes([bytes[8], bytes[9]]) as i32;

    if num_contours >= 0 {
        let mut outline = parse_simple_glyph(bytes, u16_from_i16(num_contours))?;
        outline.xmin = xmin;
        outline.ymin = ymin;
        outline.xmax = xmax;
        outline.ymax = ymax;
        outline.bbox_xmin = xmin;
        outline.bbox_ymax = ymax;
        outline.is_composite = false;
        outline.sub_lsb = hmtx.get(glyph_index).lsb as i32;
        if let Some(variation) = variation {
            outline = apply_simple_variation(glyph_index, outline, variation)?;
        }
        if let Some((x_scale, y_scale)) = point_scale {
            for point in &mut outline.points {
                if unrounded_variation {
                    // `TT_Process_Simple_Glyph` retains the design-unit
                    // coordinate in 26.6 before `FT_MulFix` rounds it. This
                    // branch also covers active `fvar` instances with no
                    // `gvar` tuple/table, where there is no sidecar to carry
                    // that precision through a composite load.
                    point.x = scale_unrounded_fdot6(point.x << 6, x_scale);
                    point.y = scale_unrounded_fdot6(point.y << 6, y_scale);
                } else {
                    point.x = crate::fixed::ft_mul_fix(point.x, x_scale);
                    point.y = crate::fixed::ft_mul_fix(point.y, y_scale);
                }
            }
        }
        Ok(outline)
    } else {
        // ── Composite glyph: decode components, recurse, flatten ──
        //
        // C's load_truetype_glyph recursively loads each sub-glyph.
        // Each recursive call hits TT_Load_Glyph_Header (ttgload.c:324)
        // which overwrites `loader->bbox` with the glyf header values.
        // tt_get_metrics does the same for `loader->left_bearing`.
        // The LAST sub-glyph "wins" — shared mutable state written N times.
        //
        // What looks like a bug is actually a deliberate choice. After
        // loading, compute_glyph_metrics (ttgload.c:1962-1968) checks:
        //
        //   if (glyph->format != FT_GLYPH_FORMAT_COMPOSITE)
        //       FT_Outline_Get_CBox(&glyph->outline, &bbox);  // O(n)
        //   else
        //       bbox = loader->bbox;  // O(1) — reuse cached value
        //
        // For composites it SKIPS calling FT_Outline_Get_CBox and reuses
        // whatever is already cached — the last sub-glyph's header. This
        // saves walking every point of every component again.
        //
        // The cost: pp1.x (computed from bbox.xMin - left_bearing)
        // can differ from the actual outline minimum by ±1-2 font units.
        // That's 1/64 of a pixel — invisible at any screen resolution.
        // The tradeoff has been there since FreeType 2.0 (1996).
        //
        // To achieve pixel-identical output we track BOTH last_sub_xmin
        // and last_sub_lsb from the final recursive sub-glyph, then
        // compute pp1.x = xmin - sub_lsb in scaler.rs — exactly
        // matching C's accidental-but-intentional behavior.
        let mut composite = parse_composite_components(bytes, 10)?;
        if let Some(variation) = variation {
            apply_composite_variation(glyph_index, &mut composite.components, variation)?;
        }
        let outline_flags = outline_flags_from_components(&composite.components);
        let mut points: Vec<OutlinePoint> = Vec::new();
        let mut end_pts: Vec<u16> = Vec::new();
        let mut num_contours_total = 0u16;
        let mut last_sub_xmin = xmin;
        let mut last_sub_lsb = hmtx.get(glyph_index).lsb as i32;
        for comp in &composite.components {
            let sub = load_glyph_inner(
                glyf,
                loca,
                index_to_loc_format,
                comp.glyph_index,
                hmtx,
                GlyphLoadOptions {
                    depth: depth + 1,
                    component_offset_scale,
                    point_scale,
                    load_composite_instructions,
                    unrounded_variation,
                    variation,
                },
            )?;
            last_sub_xmin = sub.xmin;
            last_sub_lsb = sub.sub_lsb;
            let base = points.len();
            let mut transformed = Vec::with_capacity(sub.points.len());
            for (index, pt) in sub.points.iter().enumerate() {
                let point = point_scale
                    .and_then(|(x_scale, y_scale)| {
                        sub.unrounded_points
                            .as_ref()
                            .and_then(|points| points.get(index))
                            .map(|unrounded| OutlinePoint {
                                x: scale_unrounded_fdot6(unrounded.x, x_scale),
                                y: scale_unrounded_fdot6(unrounded.y, y_scale),
                                on_curve: pt.on_curve,
                                tag: pt.tag,
                            })
                    })
                    .unwrap_or(*pt);
                transformed.push(transform_point(point, comp, 0, 0));
            }
            let (dx, dy) = if comp.args_are_xy {
                if let Some((x_scale, y_scale)) = point_scale {
                    (
                        crate::fixed::ft_mul_fix(comp.arg1, x_scale),
                        crate::fixed::ft_mul_fix(comp.arg2, y_scale),
                    )
                } else {
                    component_xy_offset(comp, component_offset_scale)
                }
            } else {
                let parent_point = comp.arg1 as usize;
                let component_point = comp.arg2 as usize;
                match (points.get(parent_point), transformed.get(component_point)) {
                    (Some(parent), Some(component)) => {
                        (parent.x - component.x, parent.y - component.y)
                    }
                    _ => {
                        // C's TT_Process_Composite_Component in
                        // ttgload.c:1059-1071 rejects invalid attachment
                        // point indices as Invalid_Composite instead of
                        // applying a zero offset.
                        return Err(FontError::InvalidComposite);
                    }
                }
            };
            // FreeType's `FT_GlyphLoader_CheckPoints` rejects a composite
            // before copying a component when the accumulated outline would
            // exceed `FT_OUTLINE_POINTS_MAX` or
            // `FT_OUTLINE_CONTOURS_MAX` (`base/ftgloadr.c:222-291`).  Keep
            // the public loader error as `Array_Too_Large` instead of letting
            // the endpoint conversion below reach a debug assertion.
            let Some(new_point_count) = points.len().checked_add(transformed.len()) else {
                return Err(FontError::ArrayTooLarge);
            };
            if new_point_count > usize::from(u16::MAX) {
                return Err(FontError::ArrayTooLarge);
            }
            let Some(new_contour_count) = num_contours_total.checked_add(sub.num_contours) else {
                return Err(FontError::ArrayTooLarge);
            };
            for pt in transformed {
                points.push(OutlinePoint {
                    x: pt.x + dx,
                    y: pt.y + dy,
                    on_curve: pt.on_curve,
                    tag: pt.tag,
                });
            }
            for &ep in &sub.end_pts_of_contours {
                end_pts.push(u16_from_u32(u32_from_usize(base) + ep as u32));
            }
            num_contours_total = new_contour_count;
        }

        // `TT_Process_Composite_Glyph` is skipped when every component is
        // empty (`num_points == start_point`). In that case even a truncated
        // trailing instruction block is accepted by FreeType. Defer this
        // read until after recursive component loading so malformed public
        // inputs follow the same control-flow boundary.
        let instructions = if load_composite_instructions && !points.is_empty() {
            if let Some(pos) = composite.instruction_pos {
                if pos + 2 > bytes.len() {
                    return Err(FontError::InvalidOutline(
                        "glyf: composite instruction length overflow".into(),
                    ));
                }
                let instruction_length = u16::from_be_bytes([bytes[pos], bytes[pos + 1]]) as usize;
                let instruction_start = pos + 2;
                if instruction_start + instruction_length > bytes.len() {
                    return Err(FontError::InvalidOutline(
                        "glyf: composite instructions overflow".into(),
                    ));
                }
                bytes[instruction_start..instruction_start + instruction_length].to_vec()
            } else {
                Vec::new()
            }
        } else {
            Vec::new()
        };

        Ok(GlyphOutline {
            num_contours: num_contours_total,
            end_pts_of_contours: end_pts,
            points,
            unrounded_points: None,
            cff_fixed_points: None,
            xmin: last_sub_xmin,
            ymin,
            xmax,
            ymax,
            bbox_xmin: xmin,
            bbox_ymax: ymax,
            is_composite: true,
            sub_lsb: last_sub_lsb,
            // C: TT_Process_Composite_Glyph in ttgload.c:1208-1234 reads
            // only this composite glyph's instruction block.  Component
            // glyph instructions run during recursive component loading and
            // must not be inherited for a second hint pass.
            instructions,
            components: composite.components,
            // C: TT_Load_Glyph keeps OVERLAP_COMPOUND only from the first
            // subglyph flags (`ttgload.c:1917-1920`).
            outline_flags,
            has_cubic_tags: false,
        })
    }
}

#[inline]
fn scale_unrounded_fdot6(value: i32, scale: i32) -> i32 {
    ft_mul_fix(value, scale).wrapping_add(32) >> 6
}

fn apply_simple_variation(
    glyph_index: u16,
    outline: GlyphOutline,
    variation: VariationContext<'_>,
) -> Result<GlyphOutline, FontError> {
    let Some(deltas) = variation.gvar.glyph_deltas_fixed_for_outline(
        glyph_index,
        &outline,
        variation.normalized_coords,
    )?
    else {
        return Ok(outline);
    };
    let mut varied = outline;
    crate::tt::gvar::apply_fixed_deltas_to_outline(&mut varied, &deltas);
    Ok(varied)
}

fn apply_composite_variation(
    glyph_index: u16,
    components: &mut [CompositeComponent],
    variation: VariationContext<'_>,
) -> Result<(), FontError> {
    if components.is_empty() {
        return Ok(());
    }
    let component_points = components
        .iter()
        .map(|component| {
            (
                component.arg1.wrapping_shl(16),
                component.arg2.wrapping_shl(16),
            )
        })
        .collect::<Vec<_>>();
    let contour_ends = (0..components.len())
        .map(|index| u16::try_from(index).unwrap_or(u16::MAX))
        .collect::<Vec<_>>();
    let Some(deltas) = variation.gvar.glyph_deltas_fixed_for_points(
        glyph_index,
        &component_points,
        &contour_ends,
        variation.normalized_coords,
    )?
    else {
        return Ok(());
    };
    // FreeType's composite variation path applies the synthetic component
    // point deltas only to XY arguments.  Anchor arguments are point indices,
    // not coordinates, and remain untouched; child glyphs receive their own
    // gvar records during recursive loading.
    for (component, (dx, dy)) in components.iter_mut().zip(deltas) {
        if component.args_are_xy {
            component.arg1 = component
                .arg1
                .wrapping_add(crate::tt::gvar::fixed_to_int(dx));
            component.arg2 = component
                .arg2
                .wrapping_add(crate::tt::gvar::fixed_to_int(dy));
        }
    }
    Ok(())
}

/// Apply a composite component's transform + translation to a point.
fn transform_point(pt: OutlinePoint, comp: &CompositeComponent, dx: i32, dy: i32) -> OutlinePoint {
    // FreeType applies the 2×2 in 16.16 (FT_MulFix) then adds the XY args.
    let x = crate::fixed::ft_mul_fix(pt.x, comp.transform.xx)
        + crate::fixed::ft_mul_fix(pt.y, comp.transform.xy);
    let y = crate::fixed::ft_mul_fix(pt.x, comp.transform.yx)
        + crate::fixed::ft_mul_fix(pt.y, comp.transform.yy);
    OutlinePoint {
        x: x + dx,
        y: y + dy,
        on_curve: pt.on_curve,
        tag: pt.tag,
    }
}

fn parse_simple_glyph(data: &[u8], num_contours: u16) -> Result<GlyphOutline, FontError> {
    let nc = num_contours as usize;
    if data.len() < 10 + nc * 2 + 2 {
        return Err(FontError::InvalidOutline("glyf: end_pts overflow".into()));
    }
    let end_off = 10usize;
    let mut end_pts = Vec::with_capacity(nc);
    let mut last_end = None;
    for i in 0..nc {
        let end = u16::from_be_bytes([data[end_off + i * 2], data[end_off + i * 2 + 1]]);
        // C `TT_Load_Simple_Glyph` rejects duplicate or decreasing contour
        // endpoints before allocating point/tag arrays (`ttgload.c:371-381`).
        if last_end.is_some_and(|last| end <= last) {
            return Err(FontError::InvalidOutline(
                "glyf: contour endpoints are not strictly increasing".into(),
            ));
        }
        end_pts.push(end);
        last_end = Some(end);
    }
    // C `TT_Load_Simple_Glyph` starts `last` at -1, so a valid zero-contour
    // record has zero points while still being allowed to carry instructions.
    let num_points = end_pts
        .last()
        .map_or(0, |&end_point| end_point as usize + 1);

    let inst_off = end_off + nc * 2;
    let instruction_length = u16::from_be_bytes([data[inst_off], data[inst_off + 1]]) as usize;
    if inst_off + 2 + instruction_length > data.len() {
        return Err(FontError::InvalidOutline(
            "glyf: simple instructions overflow".into(),
        ));
    }
    let instructions = if instruction_length > 0 {
        data[inst_off + 2..inst_off + 2 + instruction_length].to_vec()
    } else {
        Vec::new()
    };
    let mut pos = inst_off + 2 + instruction_length;

    // Decode flags with repeat compaction.
    let mut flags = Vec::with_capacity(num_points);
    while flags.len() < num_points {
        if pos >= data.len() {
            return Err(FontError::InvalidOutline("glyf: flags overflow".into()));
        }
        let flag = data[pos];
        pos += 1;
        flags.push(flag);
        if flag & REPEAT_FLAG != 0 {
            if pos >= data.len() {
                return Err(FontError::InvalidOutline(
                    "glyf: repeat count overflow".into(),
                ));
            }
            let repeat = data[pos] as usize;
            pos += 1;
            // C's TT_Load_Simple_Glyph in ttgload.c:445-455 rejects a
            // repeat that extends beyond the remaining point-tag slots.
            if flags.len() + repeat > num_points {
                return Err(FontError::InvalidOutline(
                    "glyf: repeat count exceeds point count".into(),
                ));
            }
            for _ in 0..repeat {
                flags.push(flag);
            }
        }
    }

    // Decode X coordinates.
    let mut points = Vec::with_capacity(num_points);
    let mut x: i32 = 0;
    let mut y: i32 = 0;
    for &flag in &flags {
        if flag & X_SHORT_VECTOR != 0 {
            if pos >= data.len() {
                return Err(FontError::InvalidOutline("glyf: x short overflow".into()));
            }
            let dx = data[pos] as i32;
            pos += 1;
            // X_SHORT only → negative; X_SHORT | X_IS_SAME → positive.
            x += if flag & X_IS_SAME_OR_POSITIVE_SHORT != 0 {
                dx
            } else {
                -dx
            };
        } else if flag & X_IS_SAME_OR_POSITIVE_SHORT == 0 {
            if pos + 2 > data.len() {
                return Err(FontError::InvalidOutline("glyf: x long overflow".into()));
            }
            x += i16::from_be_bytes([data[pos], data[pos + 1]]) as i32;
            pos += 2;
        }
        points.push(OutlinePoint {
            x,
            y: 0,
            on_curve: false,
            tag: flag,
        });
    }
    // Decode Y coordinates.
    for (i, &flag) in flags.iter().enumerate() {
        if flag & Y_SHORT_VECTOR != 0 {
            if pos >= data.len() {
                return Err(FontError::InvalidOutline("glyf: y short overflow".into()));
            }
            let dy = data[pos] as i32;
            pos += 1;
            y += if flag & Y_IS_SAME_OR_POSITIVE_SHORT != 0 {
                dy
            } else {
                -dy
            };
        } else if flag & Y_IS_SAME_OR_POSITIVE_SHORT == 0 {
            if pos + 2 > data.len() {
                return Err(FontError::InvalidOutline("glyf: y long overflow".into()));
            }
            y += i16::from_be_bytes([data[pos], data[pos + 1]]) as i32;
            pos += 2;
        }
        points[i].y = y;
        points[i].on_curve = flag & ON_CURVE != 0;
    }

    let outline_flags = outline_flags_from_simple_tags(&flags);

    Ok(GlyphOutline {
        num_contours,
        end_pts_of_contours: end_pts,
        points,
        unrounded_points: None,
        cff_fixed_points: None,
        xmin: 0,
        ymin: 0,
        xmax: 0,
        ymax: 0,
        bbox_xmin: 0,
        bbox_ymax: 0,
        is_composite: false,
        sub_lsb: 0,
        instructions,
        components: Vec::new(),
        // C: TT_Load_Simple_Glyph retains OVERLAP_SIMPLE from the first point
        // tag in `FT_Outline.flags`, then masks public point tags back to the
        // curve bit (`ttgload.c:459-461, 530-532`).
        outline_flags,
        has_cubic_tags: false,
    })
}

fn outline_flags_from_components(components: &[CompositeComponent]) -> u32 {
    if components
        .first()
        .is_some_and(|component| component.flags & OVERLAP_COMPOUND != 0)
    {
        OUTLINE_OVERLAP
    } else {
        0
    }
}

fn outline_flags_from_simple_tags(flags: &[u8]) -> u32 {
    if flags.first().is_some_and(|flag| flag & OVERLAP_SIMPLE != 0) {
        OUTLINE_OVERLAP
    } else {
        0
    }
}

fn parse_composite_components(data: &[u8], mut pos: usize) -> Result<CompositeGlyph, FontError> {
    let mut components = Vec::new();
    loop {
        if pos + 4 > data.len() {
            return Err(FontError::InvalidOutline(
                "glyf: composite header overflow".into(),
            ));
        }
        let flags = u16::from_be_bytes([data[pos], data[pos + 1]]);
        let glyph_index = u16::from_be_bytes([data[pos + 2], data[pos + 3]]);
        pos += 4;
        let mut count = 2usize;
        if flags & ARG_1_AND_2_ARE_WORDS != 0 {
            count += 2;
        }
        if flags & WE_HAVE_A_SCALE != 0 {
            count += 2;
        } else if flags & WE_HAVE_AN_X_Y_SCALE != 0 {
            count += 4;
        } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
            count += 8;
        }
        if pos + count > data.len() {
            return Err(FontError::InvalidOutline(
                "glyf: composite component overflow".into(),
            ));
        }

        let args_are_xy = flags & ARGS_ARE_XY_VALUES != 0;
        let (arg1, arg2) = if flags & ARG_1_AND_2_ARE_WORDS != 0 {
            let a = i16::from_be_bytes([data[pos], data[pos + 1]]) as i32;
            let b = i16::from_be_bytes([data[pos + 2], data[pos + 3]]) as i32;
            pos += 4;
            (a, b)
        } else if args_are_xy {
            let a = data[pos] as i8 as i32;
            let b = data[pos + 1] as i8 as i32;
            pos += 2;
            (a, b)
        } else {
            let a = data[pos] as i32;
            let b = data[pos + 1] as i32;
            pos += 2;
            (a, b)
        };

        let mut transform = Affine::IDENTITY;
        if flags & WE_HAVE_A_SCALE != 0 {
            let s = i16::from_be_bytes([data[pos], data[pos + 1]]) as i32 * 4;
            pos += 2;
            transform.xx = s;
            transform.yy = s;
        } else if flags & WE_HAVE_AN_X_Y_SCALE != 0 {
            transform.xx = i16::from_be_bytes([data[pos], data[pos + 1]]) as i32 * 4;
            transform.yy = i16::from_be_bytes([data[pos + 2], data[pos + 3]]) as i32 * 4;
            pos += 4;
        } else if flags & WE_HAVE_A_TWO_BY_TWO != 0 {
            transform.xx = i16::from_be_bytes([data[pos], data[pos + 1]]) as i32 * 4;
            transform.yx = i16::from_be_bytes([data[pos + 2], data[pos + 3]]) as i32 * 4;
            transform.xy = i16::from_be_bytes([data[pos + 4], data[pos + 5]]) as i32 * 4;
            transform.yy = i16::from_be_bytes([data[pos + 6], data[pos + 7]]) as i32 * 4;
            pos += 8;
        }

        components.push(CompositeComponent {
            glyph_index,
            flags,
            arg1,
            arg2,
            args_are_xy,
            transform,
            round_xy_to_grid: flags & ROUND_XY_TO_GRID != 0,
            use_my_metrics: flags & USE_MY_METRICS != 0,
        });

        if flags & MORE_COMPONENTS == 0 {
            break;
        }
    }

    // The C loader tests the final component's flag when deciding whether to
    // process the parent instruction stream. Preserve that distinction
    // instead of eagerly treating any intermediate record as a program.
    let last_has_instructions = components
        .last()
        .is_some_and(|component| component.flags & WE_HAVE_INSTRUCTIONS != 0);

    Ok(CompositeGlyph {
        components,
        instruction_pos: last_has_instructions.then_some(pos),
    })
}

fn component_xy_offset(comp: &CompositeComponent, scale: Option<(i32, i32)>) -> (i32, i32) {
    if !comp.round_xy_to_grid {
        return (comp.arg1, comp.arg2);
    }

    let Some((x_scale, y_scale)) = scale else {
        return (comp.arg1, comp.arg2);
    };

    (
        rounded_offset_font_units(comp.arg1, x_scale),
        rounded_offset_font_units(comp.arg2, y_scale),
    )
}

fn rounded_offset_font_units(value: i32, scale: i32) -> i32 {
    let scaled = ft_mul_fix(value, scale);
    let rounded = (scaled + 32) & !63;
    ft_div_fix(rounded, scale)
}
