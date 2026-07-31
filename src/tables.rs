//! Parsed font tables: holds all TrueType table data for glyph rendering.
//!
//! [`FontData`] is constructed by [`crate::font::Font::truetype`] and
//! holds the parsed results of all required TrueType tables.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;
use std::sync::{Arc, OnceLock};

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
    pub gvar: Option<GvarTable>,
    pub design_variation_coords: Vec<i32>,
    pub normalized_variation_coords: Vec<i16>,
    pub blend_variation_coords_16_16: Vec<i32>,
    pub variation_coordinates_set: bool,
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
    /// Load a glyph outline, returning a shared reference on cache hit.
    /// Uses Rc to avoid cloning the entire outline Vec on every access.
    pub fn load_glyph_outline(
        &self,
        glyph_index: u16,
    ) -> Result<Rc<crate::tt::glyf::GlyphOutline>, crate::error::FontError> {
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
        let outline = crate::tt::glyf::load_glyph(
            &self.glyf_data,
            &self.loca_data,
            self.head.index_to_loc_format,
            glyph_index,
            &self.hmtx,
        )?;
        let outline = Rc::new(self.apply_gvar_deltas(glyph_index, &outline)?);
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
        if let Some(cff) = &self.cff {
            return Ok(Rc::new(cff.load_glyph(glyph_index)?));
        }
        if let Some(cff2) = &self.cff2 {
            return Ok(Rc::new(cff2.load_glyph(glyph_index)?));
        }
        let outline = crate::tt::glyf::load_glyph_no_hinting(
            &self.glyf_data,
            &self.loca_data,
            self.head.index_to_loc_format,
            glyph_index,
            &self.hmtx,
        )?;
        Ok(Rc::new(self.apply_gvar_deltas(glyph_index, &outline)?))
    }

    fn apply_gvar_deltas(
        &self,
        glyph_index: u16,
        outline: &crate::tt::glyf::GlyphOutline,
    ) -> Result<crate::tt::glyf::GlyphOutline, crate::error::FontError> {
        let Some(gvar) = &self.gvar else {
            return Ok(outline.clone());
        };
        if self.normalized_variation_coords.is_empty() {
            return Ok(outline.clone());
        }
        let point_count_with_phantoms = outline.points.len() + 4;
        let Some(deltas) = gvar.glyph_deltas_fixed(
            glyph_index,
            point_count_with_phantoms,
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
        if let Some(hvar) = &self.hvar {
            return Ok(advance + hvar.advance_delta(glyph_index, &self.normalized_variation_coords));
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
        if self.normalized_variation_coords.is_empty() {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::font::Font;
    use crate::tt::glyf::GlyphOutline;

    const CFF1_FONT: &[u8] = include_bytes!("../tests/fixtures/input/fonts/cff/pure-cff-cubic.otf");
    const CFF2_FONT: &[u8] =
        include_bytes!("../tests/fixtures/input/fonts/cff2/fontinfo-invalid-argument.otf");
    const GLYF_FONT: &[u8] = include_bytes!("../tests/fixtures/input/fonts/DejaVuSans.ttf");
    const GVAR_FONT: &[u8] =
        include_bytes!("../tests/fixtures/input/fonts/variable/gvar-hvar-wght.ttf");

    #[test]
    fn cff_outline_loaders_cover_cache_and_no_hinting_routes() -> Result<(), crate::FontError> {
        let font = Font::truetype(CFF1_FONT, 16.0)?;
        assert!(font.data.has_cff_outlines());
        assert!(font.data.cff.is_some());
        assert!(font.data.cff2.is_none());
        font.data.glyph_cache.borrow_mut().clear();

        let first = font.data.load_glyph_outline(1)?;
        let cached = font.data.load_glyph_outline(1)?;
        assert!(Rc::ptr_eq(&first, &cached));

        let no_hinting = font.data.load_glyph_outline_no_hinting(1)?;
        assert!(!Rc::ptr_eq(&first, &no_hinting));
        assert_eq!(first.points.len(), no_hinting.points.len());
        Ok(())
    }

    #[test]
    fn cff2_outline_loaders_cover_cache_and_no_hinting_routes() -> Result<(), crate::FontError> {
        let font = Font::truetype(CFF2_FONT, 16.0)?;
        assert!(font.data.has_cff_outlines());
        assert!(font.data.cff.is_none());
        assert!(font.data.cff2.is_some());
        font.data.glyph_cache.borrow_mut().clear();

        let first = font.data.load_glyph_outline(1)?;
        let cached = font.data.load_glyph_outline(1)?;
        assert!(Rc::ptr_eq(&first, &cached));

        let no_hinting = font.data.load_glyph_outline_no_hinting(1)?;
        assert!(!Rc::ptr_eq(&first, &no_hinting));
        assert_eq!(first.points.len(), no_hinting.points.len());
        Ok(())
    }

    #[test]
    fn glyf_font_reports_non_cff_and_caches_loaded_outline() -> Result<(), crate::FontError> {
        let font = Font::truetype(GLYF_FONT, 16.0)?;
        assert!(!font.data.has_cff_outlines());
        font.data.glyph_cache.borrow_mut().clear();

        let first = font.data.load_glyph_outline(36)?;
        let cached = font.data.load_glyph_outline(36)?;
        assert!(Rc::ptr_eq(&first, &cached));

        let no_hinting = font.data.load_glyph_outline_no_hinting(36)?;
        assert!(!Rc::ptr_eq(&first, &no_hinting));
        Ok(())
    }

    #[test]
    fn gvar_helpers_cover_inactive_missing_and_active_glyph_routes() -> Result<(), crate::FontError>
    {
        let font = Font::truetype(GVAR_FONT, 16.0)?;
        assert!(font.data.gvar.is_some());

        let mut inactive = font.data.as_ref().clone();
        inactive.normalized_variation_coords.clear();
        let unchanged = inactive.apply_gvar_deltas(10, &GlyphOutline::default())?;
        assert!(unchanged.points.is_empty());
        assert_eq!(inactive.gvar_hori_advance_delta(10, 0)?, 0);

        let mut active = font.data.as_ref().clone();
        active.normalized_variation_coords = vec![0x2000];
        let missing = active.apply_gvar_deltas(u16::MAX, &GlyphOutline::default())?;
        assert!(missing.points.is_empty());
        assert_eq!(active.gvar_hori_advance_delta(u16::MAX, 0)?, 0);

        let base = crate::tt::glyf::load_glyph_no_hinting(
            &active.glyf_data,
            &active.loca_data,
            active.head.index_to_loc_format,
            10,
            &active.hmtx,
        )?;
        let varied = active.apply_gvar_deltas(10, &base)?;
        assert_eq!(varied.points.len(), base.points.len());
        let _ = active.gvar_hori_advance_delta(10, base.points.len())?;
        let _ = active.hmtx_hori_advance_with_gvar_delta(10, base.points.len())?;

        let cff = Font::truetype(CFF1_FONT, 16.0)?;
        assert_eq!(cff.data.gvar_hori_advance_delta(1, 0)?, 0);
        let unchanged = cff.data.apply_gvar_deltas(1, &GlyphOutline::default())?;
        assert!(unchanged.points.is_empty());
        Ok(())
    }
}
