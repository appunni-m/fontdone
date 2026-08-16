//! Parsed font tables: holds all TrueType table data for glyph rendering.
//!
//! [`FontData`] is constructed by [`crate::font::Font::truetype`] and
//! holds the parsed results of all required TrueType tables.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

use crate::tt::avar::AvarTable;
use crate::tt::cff::{Cff2Table, CffTable};
use crate::tt::cmap::CmapTable;
use crate::tt::fvar::FvarTable;
use crate::tt::gasp::GaspTable;
use crate::tt::gvar::GvarTable;
use crate::tt::hdmx::HdmxTable;
use crate::tt::head::HeadTable;
use crate::tt::hhea::HheaTable;
use crate::tt::hmtx::HmtxTable;
use crate::tt::hvar::HvarTable;
use crate::tt::kern::KernTable;
use crate::tt::maxp::MaxpTable;
use crate::tt::mvar::{MvarTable, VerticalHeaderDeltas};
use crate::tt::name::NameTable;
use crate::tt::os2::Os2Table;
use crate::tt::post::PostTable;
use crate::tt::sbit::SbitTable;
use crate::tt::sbix::SbixTable;
use crate::tt::svg::SvgTable;
use crate::tt::vhea::VheaTable;
use crate::tt::vmtx::VmtxTable;

/// All parsed font tables for one face, plus the requested point size.
#[derive(Debug, Clone)]
pub struct FontData {
    pub raw_data: Vec<u8>,
    pub face_offset: usize,
    pub face_index: usize,
    pub num_faces: usize,
    pub table_directory: crate::tt::TableDirectory,
    pub cmap: CmapTable,
    pub fvar: Option<FvarTable>,
    pub avar: Option<AvarTable>,
    pub gvar: Option<GvarTable>,
    pub gvar_error: Option<crate::error::FontError>,
    pub design_variation_coords: Vec<i32>,
    pub normalized_variation_coords: Vec<i16>,
    pub blend_variation_coords_16_16: Vec<i32>,
    pub variation_coordinates_set: bool,
    /// True after the public design-coordinate setter has rebuilt this face,
    /// even when the supplied coordinates resolve to the default instance.
    pub variation_coordinates_explicitly_set: bool,
    pub gasp: Option<GaspTable>,
    pub head: HeadTable,
    pub hhea: HheaTable,
    pub hvar: Option<HvarTable>,
    pub mvar: Option<MvarTable>,
    pub hmtx: HmtxTable,
    pub maxp: MaxpTable,
    pub name: NameTable,
    pub os2: Option<Os2Table>,
    pub post: Option<PostTable>,
    pub vhea: Option<VheaTable>,
    pub vmtx: Option<VmtxTable>,
    pub hdmx: Option<HdmxTable>,
    pub kern: Option<KernTable>,
    pub sbit: Option<SbitTable>,
    pub sbix: Option<SbixTable>,
    pub svg: Option<SvgTable>,
    /// TrueType interpreter version selected through the `truetype`
    /// `interpreter-version` driver property.
    pub interpreter_version: i32,
    pub cff: Option<CffTable>,
    pub cff2: Option<Cff2Table>,
    pub loca_data: Vec<u8>,
    pub glyf_data: Vec<u8>,
    pub size_pt: Cell<f32>,
    pub size_public_x_scale: Cell<i32>,
    pub size_public_y_scale: Cell<i32>,
    pub size_x_scale: Cell<i32>,
    pub size_y_scale: Cell<i32>,
    pub size_tt_scale: Cell<i32>,
    pub size_tt_ppem: Cell<i32>,
    pub size_tt_x_ratio: Cell<i32>,
    pub size_tt_y_ratio: Cell<i32>,
    pub size_tt_point_size: Cell<i32>,
    /// Active 2×2 transform set via FT_Set_Transform.  The scaler reads these
    /// before the auto-hinter runs so hinting decisions match the transformed
    /// geometry.  Identity is (0x10000, 0, 0, 0x10000, 0, 0).
    pub transform_xx: Cell<i32>,
    pub transform_xy: Cell<i32>,
    pub transform_yx: Cell<i32>,
    pub transform_yy: Cell<i32>,
    pub transform_dx: Cell<i32>,
    pub transform_dy: Cell<i32>,
    /// Font program bytecode (fpgm table). Optional — not all fonts have bytecode.
    pub fpgm: Option<Vec<u8>>,
    /// CVT program bytecode (prep table). Optional.
    pub prep: Option<Vec<u8>>,
    /// Control Value Table (cvt table) in 26.6 format. Optional.
    pub cvt: Option<Vec<i32>>,
    /// Cached parsed glyph outlines.  Populated lazily during glyph loads
    /// to avoid re-parsing the glyf/loca table on every call.
    pub glyph_cache: RefCell<HashMap<u16, Rc<crate::tt::glyf::GlyphOutline>>>,
    /// Back-pointer to the `Arc<FontData>` that owns this instance.
    /// Set once during font construction; used to avoid expensive clones.
    #[doc(hidden)]
    pub self_arc: OnceLock<Arc<FontData>>,
}

impl FontData {
    /// True when the selected variation coordinates leave the default
    /// instance. FreeType skips `gvar`/HVAR delta application when all
    /// normalized coordinates are zero, even if the public setter was called
    /// with an explicit default tuple.
    pub(crate) fn has_active_variation(&self) -> bool {
        self.normalized_variation_coords
            .iter()
            .any(|coordinate| *coordinate != 0)
    }

    /// Load a glyph outline, returning a shared reference on cache hit.
    /// Uses Rc to avoid cloning the entire outline Vec on every access.
    pub fn load_glyph_outline(
        &self,
        glyph_index: u16,
    ) -> Result<Rc<crate::tt::glyf::GlyphOutline>, crate::error::FontError> {
        if self.variation_coordinates_explicitly_set {
            if let Some(error) = &self.gvar_error {
                return Err(error.clone());
            }
        }
        {
            let cache = self.glyph_cache.borrow();
            if let Some(outline) = cache.get(&glyph_index) {
                return Ok(Rc::clone(outline));
            }
        }
        if let Some(cff) = &self.cff {
            let outline = Rc::new(cff.load_glyph(glyph_index)?);
            self.glyph_cache
                .borrow_mut()
                .insert(glyph_index, Rc::clone(&outline));
            return Ok(outline);
        }
        if let Some(cff2) = &self.cff2 {
            let outline = Rc::new(cff2.load_glyph(glyph_index)?);
            self.glyph_cache
                .borrow_mut()
                .insert(glyph_index, Rc::clone(&outline));
            return Ok(outline);
        }
        let outline = if self.has_active_variation() {
            if let Some(gvar) = &self.gvar {
                crate::tt::glyf::load_glyph_with_variations(
                    &self.glyf_data,
                    &self.loca_data,
                    self.head.index_to_loc_format,
                    glyph_index,
                    &self.hmtx,
                    gvar,
                    &self.normalized_variation_coords,
                )?
            } else {
                crate::tt::glyf::load_glyph(
                    &self.glyf_data,
                    &self.loca_data,
                    self.head.index_to_loc_format,
                    glyph_index,
                    &self.hmtx,
                )?
            }
        } else {
            crate::tt::glyf::load_glyph(
                &self.glyf_data,
                &self.loca_data,
                self.head.index_to_loc_format,
                glyph_index,
                &self.hmtx,
            )?
        };
        let outline = Rc::new(outline);
        self.glyph_cache
            .borrow_mut()
            .insert(glyph_index, Rc::clone(&outline));
        Ok(outline)
    }

    /// Load an outline for a no-hinting path.
    ///
    /// Composite instruction bytes are deliberately not read because pinned
    /// FreeType defers that read until it executes native hinting.  These
    /// outlines are not stored in `glyph_cache`: a later hinted load must
    /// still parse and retain the composite program.
    pub fn load_glyph_outline_no_hinting(
        &self,
        glyph_index: u16,
    ) -> Result<Rc<crate::tt::glyf::GlyphOutline>, crate::error::FontError> {
        if self.variation_coordinates_explicitly_set {
            if let Some(error) = &self.gvar_error {
                return Err(error.clone());
            }
        }
        if let Some(cff) = &self.cff {
            return Ok(Rc::new(cff.load_glyph(glyph_index)?));
        }
        if let Some(cff2) = &self.cff2 {
            return Ok(Rc::new(cff2.load_glyph(glyph_index)?));
        }
        let outline = if self.has_active_variation() {
            if let Some(gvar) = &self.gvar {
                crate::tt::glyf::load_glyph_no_hinting_with_variations(
                    &self.glyf_data,
                    &self.loca_data,
                    self.head.index_to_loc_format,
                    glyph_index,
                    &self.hmtx,
                    gvar,
                    &self.normalized_variation_coords,
                )?
            } else {
                crate::tt::glyf::load_glyph_no_hinting(
                    &self.glyf_data,
                    &self.loca_data,
                    self.head.index_to_loc_format,
                    glyph_index,
                    &self.hmtx,
                )?
            }
        } else {
            crate::tt::glyf::load_glyph_no_hinting(
                &self.glyf_data,
                &self.loca_data,
                self.head.index_to_loc_format,
                glyph_index,
                &self.hmtx,
            )?
        };
        Ok(Rc::new(outline))
    }

    /// Load a composite with component offsets rounded in the scaled path,
    /// retaining the same per-glyph variation ordering as the unscaled
    /// loaders.
    pub(crate) fn load_glyph_outline_with_scaled_component_offsets(
        &self,
        glyph_index: u16,
        x_scale: i32,
        y_scale: i32,
    ) -> Result<Rc<crate::tt::glyf::GlyphOutline>, crate::error::FontError> {
        let outline = if self.has_active_variation() {
            if let Some(gvar) = &self.gvar {
                crate::tt::glyf::load_glyph_with_scaled_component_offsets_and_variations(
                    &self.glyf_data,
                    &self.loca_data,
                    self.head.index_to_loc_format,
                    glyph_index,
                    &self.hmtx,
                    x_scale,
                    y_scale,
                    gvar,
                    &self.normalized_variation_coords,
                )?
            } else {
                crate::tt::glyf::load_glyph_with_scaled_component_offsets(
                    &self.glyf_data,
                    &self.loca_data,
                    self.head.index_to_loc_format,
                    glyph_index,
                    &self.hmtx,
                    x_scale,
                    y_scale,
                )?
            }
        } else {
            crate::tt::glyf::load_glyph_with_scaled_component_offsets(
                &self.glyf_data,
                &self.loca_data,
                self.head.index_to_loc_format,
                glyph_index,
                &self.hmtx,
                x_scale,
                y_scale,
            )?
        };
        Ok(Rc::new(outline))
    }

    /// Load a composite with independently scaled component offsets for the
    /// no-hinting path, including recursive `gvar` application.
    pub(crate) fn load_glyph_scaled_no_hinting(
        &self,
        glyph_index: u16,
        x_scale: i32,
        y_scale: i32,
    ) -> Result<crate::tt::glyf::GlyphOutline, crate::error::FontError> {
        if self.has_active_variation() {
            if let Some(gvar) = &self.gvar {
                return crate::tt::glyf::load_glyph_scaled_no_hinting_with_variations(
                    &self.glyf_data,
                    &self.loca_data,
                    self.head.index_to_loc_format,
                    glyph_index,
                    &self.hmtx,
                    x_scale,
                    y_scale,
                    gvar,
                    &self.normalized_variation_coords,
                );
            } else {
                return crate::tt::glyf::load_glyph_scaled_no_hinting_with_active_variation(
                    &self.glyf_data,
                    &self.loca_data,
                    self.head.index_to_loc_format,
                    glyph_index,
                    &self.hmtx,
                    x_scale,
                    y_scale,
                );
            }
        }
        crate::tt::glyf::load_glyph_scaled_no_hinting(
            &self.glyf_data,
            &self.loca_data,
            self.head.index_to_loc_format,
            glyph_index,
            &self.hmtx,
            x_scale,
            y_scale,
        )
    }

    fn apply_gvar_deltas(
        &self,
        glyph_index: u16,
        outline: &crate::tt::glyf::GlyphOutline,
    ) -> Result<crate::tt::glyf::GlyphOutline, crate::error::FontError> {
        let Some(gvar) = &self.gvar else {
            return Ok(outline.clone());
        };
        if !self.has_active_variation() {
            return Ok(outline.clone());
        }
        let Some(deltas) = gvar.glyph_deltas_fixed_for_outline(
            glyph_index,
            outline,
            &self.normalized_variation_coords,
        )?
        else {
            return Ok(outline.clone());
        };
        let mut varied = outline.clone();
        crate::tt::gvar::apply_fixed_deltas_to_outline(&mut varied, &deltas);
        Ok(varied)
    }

    pub(crate) fn has_cff_outlines(&self) -> bool {
        self.cff.is_some() || self.cff2.is_some()
    }

    /// Return horizontal advance in font units after `gvar` phantom deltas.
    ///
    /// FreeType applies `gvar` deltas to the four phantom points as part of
    /// `TT_Vary_Apply_Glyph_Deltas` before `compute_glyph_metrics`
    /// (`truetype/ttgxvar.c`, `truetype/ttgload.c`).  The public horizontal
    /// advance is therefore `pp2.x - pp1.x`, not the static `hmtx` width, for
    /// active variable-font instances.
    pub(crate) fn hmtx_hori_advance_with_gvar_delta(
        &self,
        glyph_index: u16,
        outline_point_count: usize,
    ) -> Result<i32, crate::error::FontError> {
        let advance = self.hmtx.get(glyph_index).advance_width as i32;
        if self.has_active_variation() {
            if let Some(hvar) = &self.hvar {
                return Ok(
                    advance + hvar.advance_delta(glyph_index, &self.normalized_variation_coords)
                );
            }
        }
        Ok(advance + self.gvar_hori_advance_delta(glyph_index, outline_point_count)?)
    }

    pub(crate) fn hmtx_hori_advance_with_gvar_delta_or_hmtx(
        &self,
        glyph_index: u16,
        outline_point_count: usize,
    ) -> i32 {
        self.hmtx_hori_advance_with_gvar_delta(glyph_index, outline_point_count)
            .unwrap_or_else(|_| self.hmtx.get(glyph_index).advance_width as i32)
    }

    pub(crate) fn gvar_hori_advance_delta(
        &self,
        glyph_index: u16,
        outline_point_count: usize,
    ) -> Result<i32, crate::error::FontError> {
        let Some(gvar) = &self.gvar else {
            return Ok(0);
        };
        if !self.has_active_variation() {
            return Ok(0);
        }
        let Some(deltas) = gvar.glyph_deltas(
            glyph_index,
            outline_point_count + 4,
            &self.normalized_variation_coords,
        )?
        else {
            return Ok(0);
        };
        let pp1_delta = deltas.get(outline_point_count).copied().unwrap_or_default();
        let pp2_delta = deltas
            .get(outline_point_count + 1)
            .copied()
            .unwrap_or_default();
        Ok(pp2_delta.0 - pp1_delta.0)
    }

    pub(crate) fn mvar_vertical_header_deltas(&self) -> Option<VerticalHeaderDeltas> {
        self.mvar
            .as_ref()
            .map(|mvar| mvar.vertical_header_deltas(&self.normalized_variation_coords))
    }
}
