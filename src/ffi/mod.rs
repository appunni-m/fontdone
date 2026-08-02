//! FreeType-shaped safe Rust compatibility facade.
//!
//! Choose this module when porting code that uses FreeType concepts but can
//! replace raw C ownership with Rust values and references. It intentionally
//! retains `FT_*`/`FTC_*` names, numeric constants, units, and record concepts;
//! it is not the raw native ABI exported by the `fontdone-c-abi` package.
//!
//! # Shared contract
//!
//! - `FT_Long`, `FT_Pos`, and related aliases follow the active target's C data
//!   model. Scaled positions and advances normally use signed 26.6 units;
//!   `FT_Fixed` and transform coefficients use signed 16.16 units.
//! - C input pointers become references, slices, or `Option`; output pointers
//!   become returned values or mutable references. An owned Rust result remains
//!   valid independently unless its type documents face-owned state.
//! - Functions returning [`FT_Error`] use the pinned FreeType numeric error
//!   space. Functions returning [`Result`] expose the same error as `Err`.
//! - `FT_Face` and its glyph/size state are mutable, single-thread-oriented
//!   owners. A load or render operation can replace the current slot snapshot.
//! - File and environment I/O occurs only in the explicitly named
//!   `FT_New_Face`, `FT_Attach_File`, and `FT_Set_Default_Properties` routes.
//!   Memory-face operations copy their input and perform no external I/O.
//! - Availability is determined by the generated function adoption map and
//!   exact parity evidence; the presence of a Rust item alone is not a claim
//!   that every FreeType success path is complete.
//!
//! No function in this module builds, links, loads, or calls native FreeType.

mod constants;
mod convert;
mod handles;
mod types;

pub use constants::*;
/// Extracts the FreeType render-target selector encoded in load flags.
pub use convert::FT_LOAD_TARGET_MODE;
/// Converts the core glyph-format discriminator to its FreeType numeric value.
pub use convert::glyph_format_from_core;
/// Converts FreeType load-flag bits into the core loader policy.
pub use convert::load_flags_to_core;
/// Converts the core bitmap pixel mode to its FreeType numeric value.
pub use convert::pixel_mode_from_core;
/// Converts a FreeType render-mode value into the core rasterization mode.
pub use convert::render_mode_to_core;
pub use handles::{
    FT_Face, FT_Face_Properties_State, FT_Face_Property, FT_Face_Property_Value, FT_GlyphOwned,
    FT_GlyphSlot, FT_Installed_Module_Info, FT_Library, FT_Module_Callback_Behavior,
    FT_Module_Class_Info, FT_Open_Face_Name_Options, FT_Stroker, FTCCacheManagerState,
    FTCSBitCacheLookup, FTCSBitCacheState, FTOutlineDecomposeEvent, FTOutlineDecomposeRun,
};

macro_rules! export_freetype_routes {
    ($($name:ident),+ $(,)?) => {
        $(
            #[doc = concat!(
                "Executes the safe Rust compatibility operation `",
                stringify!($name),
                "`."
            )]
            ///
            /// Its signature is the authoritative Rust ownership and nullability
            /// mapping. Shared units, errors, mutation rules, I/O boundaries, and
            /// compatibility limits are defined by the [`crate::ffi`] module
            /// contract.
            pub use handles::$name;
        )+
    };
}

export_freetype_routes!(
    FT_Activate_Size,
    FT_Add_Default_Modules,
    FT_Add_Module,
    FT_Angle_Diff,
    FT_Atan2,
    FT_Attach_File,
    FT_Attach_Stream,
    FT_Bitmap_Blend,
    FT_Bitmap_Convert,
    FT_Bitmap_Copy,
    FT_Bitmap_Done,
    FT_Bitmap_Embolden,
    FT_Bitmap_Glyph_Copy,
    FT_Bitmap_Init,
    FT_Bitmap_New,
    FT_Bitmap_Owned_Buffer_Bytes,
    FT_Bitmap_Set_Owned_Buffer,
    FT_Bzip2_Stream_Close,
    FT_Bzip2_Stream_Is_Open,
    FT_Bzip2_Stream_Read,
    FT_CeilFix,
    FT_ClassicKern_Free,
    FT_ClassicKern_Validate,
    FT_Cos,
    FT_DivFix,
    FT_Done_Face,
    FT_Done_FreeType,
    FT_Done_Glyph,
    FT_Done_MM_Var,
    FT_Done_Size,
    FT_Error_String,
    FT_FACE_DRIVER_NAME,
    FT_Face_CheckTrueTypePatents,
    FT_Face_GetCharVariantIndex,
    FT_Face_GetCharVariantIsDefault,
    FT_Face_GetCharsOfVariant,
    FT_Face_GetVariantSelectors,
    FT_Face_GetVariantsOfChar,
    FT_Face_Incremental_Glyph_Data,
    FT_Face_Properties,
    FT_Face_Properties_Get_State,
    FT_Face_SetUnpatentedHinting,
    FT_FloorFix,
    FT_GX_Validator_Set_Available,
    FT_Get_Advance,
    FT_Get_Advances,
    FT_Get_BDF_Charset_ID,
    FT_Get_BDF_Property,
    FT_Get_Bitmap_Glyph,
    FT_Get_CID_From_Glyph_Index,
    FT_Get_CID_Is_Internally_CID_Keyed,
    FT_Get_CID_Registry_Ordering_Supplement,
    FT_Get_CMap_Format,
    FT_Get_CMap_Language_ID,
    FT_Get_Char_Index,
    FT_Get_Charmap_Index,
    FT_Get_Color_Glyph_ClipBox,
    FT_Get_Color_Glyph_Layer,
    FT_Get_Color_Glyph_Paint,
    FT_Get_Colorline_Stops,
    FT_Get_Default_Named_Instance,
    FT_Get_FSType_Flags,
    FT_Get_First_Char,
    FT_Get_Font_Format,
    FT_Get_Gasp,
    FT_Get_Glyph,
    FT_Get_Glyph_Name,
    FT_Get_Kerning,
    FT_Get_MM_Blend_Coordinates,
    FT_Get_MM_Var,
    FT_Get_MM_WeightVector,
    FT_Get_Module_Interface,
    FT_Get_Multi_Master,
    FT_Get_Name_Index,
    FT_Get_Next_Char,
    FT_New_Glyph,
    FT_Get_Outline_Glyph,
    FT_Get_PFR_Advance,
    FT_Get_PFR_Kerning,
    FT_Get_PFR_Metrics,
    FT_Get_PS_Font_Info,
    FT_Get_PS_Font_Private,
    FT_Get_PS_Font_Value,
    FT_Get_Paint,
    FT_Get_Paint_Layers,
    FT_Get_Postscript_Name,
    FT_Get_Sfnt_LangTag,
    FT_Get_Sfnt_Name,
    FT_Get_Sfnt_Name_Count,
    FT_Get_Sfnt_Table,
    FT_Get_SubGlyph_Info,
    FT_Get_Svg_Glyph,
    FT_Get_Track_Kerning,
    FT_Get_Transform,
    FT_Get_TrueType_Engine_Type,
    FT_Get_Var_Axis_Flags,
    FT_Get_Var_Blend_Coordinates,
    FT_Get_Var_Design_Coordinates,
    FT_Get_WinFNT_Header,
    FT_Get_X11_Font_Format,
    FT_Glyph_Copy,
    FT_Glyph_Get_CBox,
    FT_Glyph_To_Bitmap,
    FT_Glyph_Transform_Outline,
    FT_GlyphSlot_AdjustWeight,
    FT_GlyphSlot_Embolden,
    FT_GlyphSlot_Oblique,
    FT_GlyphSlot_Own_Bitmap,
    FT_GlyphSlot_Slant,
    FT_Gzip_Stream_Close,
    FT_Gzip_Stream_Read,
    FT_Gzip_Uncompress,
    FT_Has_PS_Glyph_Names,
    FT_LZW_Stream_Close,
    FT_LZW_Stream_Read,
    FT_Library_LcdWeights,
    FT_Library_SetLcdFilter,
    FT_Library_SetLcdFilterWeights,
    FT_Library_SetLcdGeometry,
    FT_Library_Version,
    FT_List_Add,
    FT_List_Finalize_Clear,
    FT_List_Finalize_Node,
    FT_List_Find_Node_Matches,
    FT_List_Insert,
    FT_List_Iterate_Next,
    FT_List_Remove,
    FT_List_Up,
    FT_Load_Char,
    FT_Load_Glyph,
    FT_Load_Sfnt_Table,
    FT_Matrix_Invert,
    FT_Matrix_Multiply,
    FT_MulDiv,
    FT_MulFix,
    FT_New_Face,
    FT_New_Memory_Face,
    FT_New_Memory_Face_With_Name_Options,
    FT_New_Size,
    FT_Open_External_Stream_Face_With_Name_Options,
    FT_OpenType_Free,
    FT_OpenType_Table_Copy,
    FT_OpenType_Validate,
    FT_OpenType_Validator_Set_Available,
    FT_Outline_Check,
    FT_Outline_Copy,
    FT_Outline_Decompose_Trace,
    FT_Outline_Embolden,
    FT_Outline_EmboldenXY,
    FT_Outline_Get_BBox,
    FT_Outline_Get_Bitmap,
    FT_Outline_Get_CBox,
    FT_Outline_Get_Orientation,
    FT_Outline_GetInsideBorder,
    FT_Outline_GetOutsideBorder,
    FT_Outline_Glyph_CBox,
    FT_Outline_Glyph_Copy,
    FT_Outline_Glyph_Stroke,
    FT_Outline_Glyph_StrokeBorder,
    FT_Outline_Glyph_To_Bitmap,
    FT_Outline_Glyph_To_Bitmap_In_Place,
    FT_Outline_Glyph_To_Bitmap_With_Origin,
    FT_Outline_Render,
    FT_Outline_Render_Direct_Spans,
    FT_Outline_Render_Error_Output,
    FT_Outline_Reverse,
    FT_Outline_Transform,
    FT_Outline_Translate,
    FT_Palette_Data_Get,
    FT_Palette_Select,
    FT_Palette_Set_Foreground_Color,
    FT_Property_Get,
    FT_Property_Get_GlyphToScriptMap,
    FT_Property_Get_IncreaseXHeight,
    FT_Property_Set,
    FT_Property_Set_IncreaseXHeight,
    FT_Reference_Face,
    FT_Reference_Library,
    FT_Remove_Module,
    FT_Render_Glyph,
    FT_Request_Size,
    FT_RoundFix,
    FT_Select_Charmap,
    FT_Select_Size,
    FT_Set_Char_Size,
    FT_Set_Charmap,
    FT_Set_Debug_Hook,
    FT_Set_Default_Properties,
    FT_Set_Default_Properties_From_Env,
    FT_Set_MM_Blend_Coordinates,
    FT_Set_MM_Design_Coordinates,
    FT_Set_MM_WeightVector,
    FT_Set_Named_Instance,
    FT_Set_Pixel_Sizes,
    FT_Set_Transform,
    FT_Set_Var_Blend_Coordinates,
    FT_Set_Var_Design_Coordinates,
    FT_Sfnt_Table_Info,
    FT_Sin,
    FT_Stream_OpenBzip2,
    FT_Stream_OpenGzip,
    FT_Stream_OpenLZW,
    FT_Stroker_BeginSubPath,
    FT_Stroker_ConicTo,
    FT_Stroker_CubicTo,
    FT_Stroker_Done,
    FT_Stroker_EndSubPath,
    FT_Stroker_Export,
    FT_Stroker_ExportBorder,
    FT_Stroker_GetBorderCounts,
    FT_Stroker_GetCounts,
    FT_Stroker_LineTo,
    FT_Stroker_New,
    FT_Stroker_ParseOutline,
    FT_Stroker_Rewind,
    FT_Stroker_Set,
    FT_Svg_Glyph_Copy,
    FT_Svg_Glyph_Transform,
    FT_Tan,
    FT_TrueTypeGX_Free,
    FT_TrueTypeGX_Validate,
    FT_Vector_From_Polar,
    FT_Vector_Length,
    FT_Vector_Polarize,
    FT_Vector_Rotate,
    FT_Vector_Transform,
    FT_Vector_Unit,
    FTC_Node_Unref,
    FTC_SBitCache_Lookup,
);

#[cfg(any(test, feature = "abi-test-support"))]
macro_rules! export_parity_helpers {
    ($($name:ident),+ $(,)?) => {
        $(
            #[doc = concat!(
                "Exposes cross-facade verification state for `",
                stringify!($name),
                "`."
            )]
            ///
            /// This operation is public only for maintained parity and ABI
            /// integration tests. Application code must use the corresponding
            /// non-diagnostic operation documented by [`crate::ffi`].
            pub use handles::$name;
        )+
    };
}

/// Creates a pure-Rust library handle with the maintained default modules.
///
/// The returned owner is the safe Rust counterpart of FreeType's output
/// `FT_Library` handle. It performs no native call and is consumed by
/// [`FT_Done_FreeType`].
pub use handles::FT_Init_FreeType;

#[cfg(any(test, feature = "abi-test-support"))]
export_parity_helpers!(
    FT_ColrV1_Paint_Layer_Iterator_Copy,
    FT_New_Glyph_Allocation_Failure,
    FT_New_Glyph_Validate,
);
#[cfg(feature = "abi-test-support")]
export_parity_helpers!(FT_Outline_GlyphSlot_With_Advance);

/// Installs the four public OT-SVG renderer hooks on a library, mirroring
/// FreeType's `ot-svg:svg-hooks` driver property.
pub use handles::FT_Set_SVG_Renderer_Hooks;
#[cfg(any(test, feature = "abi-test-support"))]
pub use handles::{
    FT_ColrV1_PaintGraph_Snapshot, FT_ColrV1_PaintNode_Snapshot, FT_ColrV1_PaintRecord_Snapshot,
    FT_ColrV1_PublicPaintSolid_Snapshot, FT_Custom_Glyph_Lifecycle_Snapshot,
    FT_Glyph_Copy_Failure_Row, FT_Module_Callback_Event, FT_Palette_Data_Snapshot,
    FT_Palette_Select_Snapshot, FT_Raster_Funcs_Observation, FT_Raster_Set_Mode_Observation,
};
#[cfg(any(test, feature = "abi-test-support"))]
export_parity_helpers!(
    FT_ColrV1_Paint_ColorLine_Copy,
    FT_ColrV1_Paint_LinearGradient_Copy,
    FT_ColrV1_Paint_Transform_Copy,
    FT_ColrV1_PaintGraph_Copy,
    FT_ColrV1_PublicPaintSolid_Copy,
    FT_Custom_Glyph_Lifecycle,
    FT_Face_Incremental_Interface,
    FT_Fvar_Named_Style_Coords,
    FT_Get_Sfnt_MaxProfile_Copy,
    FT_Get_Sfnt_VertHeader_Copy,
    FT_Glyph_Copy_Failure_Cleanup,
    FT_Glyph_To_Script_Map_Mutate_For_Test,
    FT_Glyph_To_Script_Map_Sample_For_Test,
    FT_GlyphSlot_Own_Bitmap_Copy_Allocation_Failure,
    FT_Library_Debug_Hook_Classes,
    FT_Library_Default_Module_Names,
    FT_Library_Face_Counts,
    FT_Library_Has_TrueType_Engine_Service,
    FT_Library_Has_TrueType_Module,
    FT_Library_Module_Callback_Events,
    FT_Library_Module_Count,
    FT_Library_Register_Face_Probe,
    FT_Library_Synthetic_Module_Info,
    FT_Library_Unregister_Face_Probe,
    FT_Malformed_Get_GlyphSlot,
    FT_Module_Requester_Service_Available,
    FT_New_Library_Without_Default_Modules,
    FT_Open_Face_With_Incremental,
    FT_Open_Face_With_Incremental_Parameter,
    FT_Open_Face_NonDriver_Diagnostic,
    FT_Palette_Active_Entries_Copy,
    FT_Palette_Data_Copy,
    FT_Palette_Foreground_Copy,
    FT_Palette_Select_Copy,
    FT_Palette_Set_Active_Entry_For_Test,
    FT_Sfnt_Load_Name_Diagnostic,
    FT_Raster_Set_Mode_Probe,
    FT_Raster_Funcs_Probe,
    FT_TrueType_Context_Allocation_Failure_Diagnostic,
    FT_Unsupported_GlyphSlot,
);
export_freetype_routes!(
    FT_Done_Library,
    FT_Library_Memory,
    FT_Library_Refcount,
    FT_New_Library,
    FT_Empty_GlyphSlot,
    FT_Library_Has_Module,
    FT_Library_Module_Flags,
    FT_Library_Renderer_Class,
    FT_Library_Set_Renderer_By_Format,
);
pub use types::*;
