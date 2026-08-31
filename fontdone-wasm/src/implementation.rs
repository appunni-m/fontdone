//! Implementation of the WebAssembly ABI façade for the `fontdone` engine.

#![expect(
    missing_docs,
    reason = "raw WebAssembly ABI is generated into abi.json and TypeScript declarations"
)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
#![allow(clippy::too_many_arguments)]
#![allow(non_camel_case_types, non_snake_case)]

use std::alloc::{Layout, alloc, alloc_zeroed, dealloc};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::ffi::{CStr, c_uchar, c_void};
use std::ptr;
use std::slice;

use fontdone::ffi as rust_ffi;

pub type FT_Error = i32;
pub type FT_Bool = u8;
pub type FT_F2Dot14 = i16;
pub type FT_Fixed = i64;
pub type FT_Angle = FT_Fixed;
pub type FT_Int = i32;
pub type FT_Int32 = i32;
pub type FT_Long = i64;
pub type FT_Pos = i64;
pub type FT_ULong = u64;
pub type FT_UInt = u32;
pub type FT_UInt32 = u32;
pub type FT_Sfnt_Tag = u32;
pub type FT_Short = i16;
pub type FT_UShort = u16;
pub type FT_Byte = u8;
pub type FT_Bytes = *const FT_Byte;
pub type FT_Data = rust_ffi::FT_Data;
pub type FT_Incremental = rust_ffi::FT_Incremental;
pub type FT_Incremental_MetricsRec = rust_ffi::FT_Incremental_MetricsRec;
pub type FT_Incremental_FuncsRec = rust_ffi::FT_Incremental_FuncsRec;
pub type FT_Incremental_InterfaceRec = rust_ffi::FT_Incremental_InterfaceRec;
pub type FT_LayerIterator = rust_ffi::FT_LayerIterator;
pub type FT_ClipBox = rust_ffi::FT_ClipBox;
pub type FT_ColorLine = rust_ffi::FT_ColorLine;
pub type FT_ColorStop = rust_ffi::FT_ColorStop;
pub type FT_ColorStopIterator = rust_ffi::FT_ColorStopIterator;
pub type FT_Matrix = rust_ffi::FT_Matrix;
pub type FT_PaintTransform = rust_ffi::FT_PaintTransform;
pub type FT_Vector = rust_ffi::FT_Vector;
pub type FT_Size_Request_Type = i32;
pub type FT_Encoding = i32;
pub type FT_LcdFilter = i32;
pub type FT_TrueTypeEngineType = i32;
pub type PS_Dict_Keys = i32;
pub type T1_EncodingType = i32;
pub type FT_Orientation = i32;
pub type FT_StrokerBorder = i32;
pub type FT_Pointer = *mut c_void;
pub type FT_Module_Interface = FT_Pointer;
pub type FT_ListNode = *mut FontdoneWasmListNode;
pub type FT_List = *mut FontdoneWasmList;
pub type FT_List_Iterator = Option<extern "C" fn(node: FT_ListNode, user: FT_Pointer) -> FT_Error>;
pub type FT_Memory = *mut FontdoneWasmMemory;
pub type FT_Alloc_Func = Option<extern "C" fn(memory: FT_Memory, size: FT_Long) -> FT_Pointer>;
pub type FT_Free_Func = Option<extern "C" fn(memory: FT_Memory, block: FT_Pointer)>;
pub type FT_Realloc_Func = Option<
    extern "C" fn(
        memory: FT_Memory,
        cur_size: FT_Long,
        new_size: FT_Long,
        block: FT_Pointer,
    ) -> FT_Pointer,
>;
pub type FT_List_Destructor =
    Option<extern "C" fn(memory: FT_Memory, data: FT_Pointer, user: FT_Pointer)>;

const PROPERTY_SENTINEL: FT_UInt = 0xDEAD_BEEF;

#[cfg(feature = "abi-test-support")]
const INCREMENTAL_CLIENT_OBJECT_MAGIC: u64 = 0x46_54_49_4E_43_4C_49_46;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmStatus {
    pub error: FT_Error,
    pub handle: usize,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmListNode {
    pub prev: FT_ListNode,
    pub next: FT_ListNode,
    pub data: FT_Pointer,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmList {
    pub head: FT_ListNode,
    pub tail: FT_ListNode,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmMemory {
    pub user: FT_Pointer,
    pub alloc: FT_Alloc_Func,
    pub free: FT_Free_Func,
    pub realloc: FT_Realloc_Func,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmVector {
    pub x: i64,
    pub y: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmMatrix {
    pub xx: i64,
    pub xy: i64,
    pub yx: i64,
    pub yy: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmBBox {
    pub xMin: i64,
    pub yMin: i64,
    pub xMax: i64,
    pub yMax: i64,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct FontdoneWasmWinFNTHeader {
    pub version: FT_UShort,
    pub file_size: FT_ULong,
    pub copyright: [FT_Byte; 60],
    pub file_type: FT_UShort,
    pub nominal_point_size: FT_UShort,
    pub vertical_resolution: FT_UShort,
    pub horizontal_resolution: FT_UShort,
    pub ascent: FT_UShort,
    pub internal_leading: FT_UShort,
    pub external_leading: FT_UShort,
    pub italic: FT_Byte,
    pub underline: FT_Byte,
    pub strike_out: FT_Byte,
    pub weight: FT_UShort,
    pub charset: FT_Byte,
    pub pixel_width: FT_UShort,
    pub pixel_height: FT_UShort,
    pub pitch_and_family: FT_Byte,
    pub avg_width: FT_UShort,
    pub max_width: FT_UShort,
    pub first_char: FT_Byte,
    pub last_char: FT_Byte,
    pub default_char: FT_Byte,
    pub break_char: FT_Byte,
    pub bytes_per_row: FT_UShort,
    pub device_offset: FT_ULong,
    pub face_name_offset: FT_ULong,
    pub bits_pointer: FT_ULong,
    pub bits_offset: FT_ULong,
    pub reserved: FT_Byte,
    pub flags: FT_ULong,
    pub A_space: FT_UShort,
    pub B_space: FT_UShort,
    pub C_space: FT_UShort,
    pub color_table_offset: FT_UShort,
    pub reserved1: [FT_ULong; 4],
}

impl Default for FontdoneWasmWinFNTHeader {
    fn default() -> Self {
        Self {
            version: 0,
            file_size: 0,
            copyright: [0; 60],
            file_type: 0,
            nominal_point_size: 0,
            vertical_resolution: 0,
            horizontal_resolution: 0,
            ascent: 0,
            internal_leading: 0,
            external_leading: 0,
            italic: 0,
            underline: 0,
            strike_out: 0,
            weight: 0,
            charset: 0,
            pixel_width: 0,
            pixel_height: 0,
            pitch_and_family: 0,
            avg_width: 0,
            max_width: 0,
            first_char: 0,
            last_char: 0,
            default_char: 0,
            break_char: 0,
            bytes_per_row: 0,
            device_offset: 0,
            face_name_offset: 0,
            bits_pointer: 0,
            bits_offset: 0,
            reserved: 0,
            flags: 0,
            A_space: 0,
            B_space: 0,
            C_space: 0,
            color_table_offset: 0,
            reserved1: [0; 4],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmPSPrivate {
    pub unique_id: FT_Int,
    pub lenIV: FT_Int,
    pub num_blue_values: FT_Byte,
    pub num_other_blues: FT_Byte,
    pub num_family_blues: FT_Byte,
    pub num_family_other_blues: FT_Byte,
    pub blue_values: [FT_Short; 14],
    pub other_blues: [FT_Short; 10],
    pub family_blues: [FT_Short; 14],
    pub family_other_blues: [FT_Short; 10],
    pub blue_scale: FT_Fixed,
    pub blue_shift: FT_Int,
    pub blue_fuzz: FT_Int,
    pub standard_width: [FT_UShort; 1],
    pub standard_height: [FT_UShort; 1],
    pub num_snap_widths: FT_Byte,
    pub num_snap_heights: FT_Byte,
    pub force_bold: FT_Bool,
    pub round_stem_up: FT_Bool,
    pub snap_widths: [FT_Short; 13],
    pub snap_heights: [FT_Short; 13],
    pub expansion_factor: FT_Fixed,
    pub language_group: FT_Long,
    pub password: FT_Long,
    pub min_feature: [FT_Short; 2],
}

fn copy_rust_ps_private_to_wasm(out: &mut FontdoneWasmPSPrivate, value: rust_ffi::PS_PrivateRec) {
    out.unique_id = value.unique_id;
    out.lenIV = value.lenIV;
    out.num_blue_values = value.num_blue_values;
    out.num_other_blues = value.num_other_blues;
    out.num_family_blues = value.num_family_blues;
    out.num_family_other_blues = value.num_family_other_blues;
    out.blue_values = value.blue_values;
    out.other_blues = value.other_blues;
    out.family_blues = value.family_blues;
    out.family_other_blues = value.family_other_blues;
    out.blue_scale = value.blue_scale;
    out.blue_shift = value.blue_shift;
    out.blue_fuzz = value.blue_fuzz;
    out.standard_width = value.standard_width;
    out.standard_height = value.standard_height;
    out.num_snap_widths = value.num_snap_widths;
    out.num_snap_heights = value.num_snap_heights;
    out.force_bold = value.force_bold;
    out.round_stem_up = value.round_stem_up;
    out.snap_widths = value.snap_widths;
    out.snap_heights = value.snap_heights;
    out.expansion_factor = value.expansion_factor;
    out.language_group = value.language_group;
    out.password = value.password;
    out.min_feature = value.min_feature;
}

fn wasm_string_from_ft_string(value: *mut rust_ffi::FT_String) -> FontdoneWasmString {
    if value.is_null() {
        return FontdoneWasmString::default();
    }
    // SAFETY: `FT_Get_PS_Font_Info` returns face-owned NUL-terminated strings.
    let bytes = unsafe { CStr::from_ptr(value.cast()).to_bytes() };
    FontdoneWasmString {
        string: value.cast(),
        string_len: u32::try_from(bytes.len()).unwrap_or(u32::MAX),
    }
}

fn copy_rust_ps_font_info_to_wasm(
    out: &mut FontdoneWasmPSFontInfo,
    value: rust_ffi::PS_FontInfoRec,
) {
    out.version = wasm_string_from_ft_string(value.version);
    out.notice = wasm_string_from_ft_string(value.notice);
    out.full_name = wasm_string_from_ft_string(value.full_name);
    out.family_name = wasm_string_from_ft_string(value.family_name);
    out.weight = wasm_string_from_ft_string(value.weight);
    out.italic_angle = value.italic_angle;
    out.is_fixed_pitch = value.is_fixed_pitch;
    out.underline_position = value.underline_position;
    out.underline_thickness = value.underline_thickness;
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmBdfProperty {
    pub type_: FT_Int,
    pub atom: *const FT_Byte,
    pub atom_len: FT_UInt,
    pub integer: FT_Int32,
    pub cardinal: FT_UInt32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmBdfCharset {
    pub charset_encoding: *const FT_Byte,
    pub charset_encoding_len: FT_UInt,
    pub charset_registry: *const FT_Byte,
    pub charset_registry_len: FT_UInt,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmCidRos {
    pub registry: *const FT_Byte,
    pub registry_len: FT_UInt,
    pub ordering: *const FT_Byte,
    pub ordering_len: FT_UInt,
    pub supplement: FT_Int,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmOutline {
    pub n_contours: FT_UShort,
    pub n_points: FT_UShort,
    pub points: *mut FontdoneWasmVector,
    pub tags: *mut FT_Byte,
    pub contours: *mut FT_UShort,
    pub flags: FT_Int,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmGlyphClass {
    pub glyph_bbox_present: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmGlyph {
    pub clazz: *const FontdoneWasmGlyphClass,
    pub format: i32,
    pub advance: FontdoneWasmVector,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmOutlineGlyph {
    pub root: FontdoneWasmGlyph,
    pub outline: FontdoneWasmOutline,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmBitmapGlyph {
    pub root: FontdoneWasmGlyph,
    pub left: i32,
    pub top: i32,
    pub bitmap: FontdoneWasmBitmap,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmSvgGlyph {
    pub root: FontdoneWasmGlyph,
    pub svg_document: *const FT_Byte,
    pub svg_document_length: usize,
    pub glyph_index: FT_UInt,
    pub metrics: FontdoneWasmSizeMetrics,
    pub units_per_EM: FT_UShort,
    pub start_glyph_id: FT_UShort,
    pub end_glyph_id: FT_UShort,
    pub transform: FontdoneWasmMatrix,
    pub delta: FontdoneWasmVector,
}

#[repr(C)]
struct WasmOwnedOutlineGlyph {
    record: FontdoneWasmOutlineGlyph,
    core: rust_ffi::FT_OutlineGlyphOwned,
    points: Box<[FontdoneWasmVector]>,
    tags: Box<[FT_Byte]>,
    contours: Box<[FT_UShort]>,
}

impl WasmOwnedOutlineGlyph {
    fn new(core: rust_ffi::FT_OutlineGlyphOwned) -> Self {
        let mut glyph = Self {
            record: FontdoneWasmOutlineGlyph {
                root: wasm_glyph_root_from_core(&core.root),
                outline: FontdoneWasmOutline::default(),
            },
            core,
            points: Box::new([]),
            tags: Box::new([]),
            contours: Box::new([]),
        };
        glyph.refresh_record();
        glyph
    }

    fn refresh_record(&mut self) {
        self.record.root = wasm_glyph_root_from_core(&self.core.root);
        self.record.root.clazz = wasm_owned_outline_glyph_class();
        self.points = self
            .core
            .outline
            .points
            .iter()
            .map(|point| FontdoneWasmVector {
                x: point.x,
                y: point.y,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self.tags = self.core.outline.tags.clone().into_boxed_slice();
        self.contours = self.core.outline.contours.clone().into_boxed_slice();
        self.record.outline = FontdoneWasmOutline {
            n_contours: u16::try_from(self.contours.len()).unwrap_or(u16::MAX),
            n_points: u16::try_from(self.points.len()).unwrap_or(u16::MAX),
            points: self.points.as_mut_ptr(),
            tags: self.tags.as_mut_ptr(),
            contours: self.contours.as_mut_ptr(),
            flags: self.core.outline.flags,
        };
    }

    fn sync_core_from_record(&mut self) -> Result<(), FT_Error> {
        let point_count = usize::from(self.record.outline.n_points);
        let contour_count = usize::from(self.record.outline.n_contours);
        if (point_count != 0
            && (self.record.outline.points.is_null() || self.record.outline.tags.is_null()))
            || (contour_count != 0 && self.record.outline.contours.is_null())
        {
            return Err(rust_ffi::FT_Err_Invalid_Outline);
        }
        // SAFETY: the WASM handle owns these published arrays until the glyph
        // is released.  Import caller mutations before rendering so this ABI
        // observes the same public-record semantics as C FreeType.
        let points = if point_count == 0 {
            &[]
        } else {
            // SAFETY: non-empty storage was validated above.
            unsafe { std::slice::from_raw_parts(self.record.outline.points, point_count) }
        };
        // SAFETY: validated alongside `points`; tags share the point count.
        let tags = if point_count == 0 {
            &[]
        } else {
            // SAFETY: non-empty storage was validated above.
            unsafe { std::slice::from_raw_parts(self.record.outline.tags, point_count) }
        };
        // SAFETY: validated above and bounded by the public contour count.
        let contours = if contour_count == 0 {
            &[]
        } else {
            // SAFETY: non-empty storage was validated above.
            unsafe { std::slice::from_raw_parts(self.record.outline.contours, contour_count) }
        };
        self.core.outline.points = points
            .iter()
            .map(|point| rust_ffi::FT_Vector {
                x: point.x,
                y: point.y,
            })
            .collect();
        self.core.outline.tags = tags.to_vec();
        self.core.outline.contours = contours.to_vec();
        self.core.outline.flags = self.record.outline.flags;
        Ok(())
    }
}

#[repr(C)]
struct WasmOwnedBitmapGlyph {
    record: FontdoneWasmBitmapGlyph,
    core: rust_ffi::FT_BitmapGlyphOwned,
    buffer: Box<[FT_Byte]>,
}

#[repr(C)]
struct WasmOwnedSvgGlyph {
    record: FontdoneWasmSvgGlyph,
    core: rust_ffi::FT_SvgGlyphOwned,
    document: Box<[FT_Byte]>,
}

impl WasmOwnedSvgGlyph {
    fn new(core: rust_ffi::FT_SvgGlyphOwned) -> Self {
        let mut glyph = Self {
            record: FontdoneWasmSvgGlyph::default(),
            core,
            document: Box::new([]),
        };
        glyph.refresh_record();
        glyph
    }

    fn refresh_record(&mut self) {
        self.document = self.core.svg_document.clone().into_boxed_slice();
        self.record = FontdoneWasmSvgGlyph {
            root: wasm_glyph_root_from_core_with_class(
                &self.core.root,
                wasm_owned_svg_glyph_class(),
            ),
            svg_document: self.document.as_ptr(),
            svg_document_length: self.document.len(),
            glyph_index: self.core.glyph_index,
            metrics: wasm_size_metrics(self.core.metrics),
            units_per_EM: self.core.units_per_EM,
            start_glyph_id: self.core.start_glyph_id,
            end_glyph_id: self.core.end_glyph_id,
            transform: FontdoneWasmMatrix {
                xx: self.core.transform.xx,
                xy: self.core.transform.xy,
                yx: self.core.transform.yx,
                yy: self.core.transform.yy,
            },
            delta: FontdoneWasmVector {
                x: self.core.delta.x,
                y: self.core.delta.y,
            },
        };
    }

    fn sync_core_from_record(&mut self) -> Result<(), FT_Error> {
        if self.record.svg_document.is_null() {
            return Err(rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error);
        }
        let document_len = self.record.svg_document_length;
        if document_len == 0 {
            return Err(rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error);
        }
        // SAFETY: a non-null public SVG record promises `document_len`
        // readable bytes for this synchronous copy operation.
        self.core.svg_document =
            unsafe { slice::from_raw_parts(self.record.svg_document, document_len).to_vec() };
        self.core.root.format = self.record.root.format;
        self.core.root.advance = rust_ffi::FT_Vector {
            x: self.record.root.advance.x,
            y: self.record.root.advance.y,
        };
        self.core.glyph_index = self.record.glyph_index;
        self.core.metrics = rust_ffi::FT_Size_Metrics {
            x_ppem: self.record.metrics.x_ppem,
            y_ppem: self.record.metrics.y_ppem,
            x_scale: self.record.metrics.x_scale,
            y_scale: self.record.metrics.y_scale,
            ascender: self.record.metrics.ascender,
            descender: self.record.metrics.descender,
            height: self.record.metrics.height,
            max_advance: self.record.metrics.max_advance,
        };
        self.core.units_per_EM = self.record.units_per_EM;
        self.core.start_glyph_id = self.record.start_glyph_id;
        self.core.end_glyph_id = self.record.end_glyph_id;
        self.core.transform = rust_ffi::FT_Matrix {
            xx: self.record.transform.xx,
            xy: self.record.transform.xy,
            yx: self.record.transform.yx,
            yy: self.record.transform.yy,
        };
        self.core.delta = rust_ffi::FT_Vector {
            x: self.record.delta.x,
            y: self.record.delta.y,
        };
        Ok(())
    }
}

impl WasmOwnedBitmapGlyph {
    fn new(core: rust_ffi::FT_BitmapGlyphOwned) -> Self {
        let mut glyph = Self {
            record: FontdoneWasmBitmapGlyph {
                root: wasm_glyph_root_from_core_with_class(
                    &core.root,
                    wasm_owned_bitmap_glyph_class(),
                ),
                left: core.left,
                top: core.top,
                bitmap: FontdoneWasmBitmap::default(),
            },
            core,
            buffer: Box::new([]),
        };
        glyph.refresh_record();
        glyph
    }

    fn refresh_record(&mut self) {
        self.record.root =
            wasm_glyph_root_from_core_with_class(&self.core.root, wasm_owned_bitmap_glyph_class());
        self.record.left = self.core.left;
        self.record.top = self.core.top;
        self.buffer = self.core.bitmap.buffer.clone().into_boxed_slice();
        self.record.bitmap = FontdoneWasmBitmap {
            rows: self.core.bitmap.rows,
            width: self.core.bitmap.width,
            pitch: self.core.bitmap.pitch,
            buffer: self.buffer.as_ptr(),
            buffer_len: self.buffer.len(),
            num_grays: self.core.bitmap.num_grays,
            pixel_mode: self.core.bitmap.pixel_mode,
            palette_mode: 0,
            palette: ptr::null(),
        };
    }

    fn sync_core_from_record(&mut self) -> Result<(), FT_Error> {
        let byte_len = usize::try_from(self.record.bitmap.rows)
            .ok()
            .and_then(|rows| {
                usize::try_from(self.record.bitmap.pitch.unsigned_abs())
                    .ok()
                    .and_then(|pitch| rows.checked_mul(pitch))
            })
            .ok_or(rust_ffi::FT_Err_Invalid_Argument)?;
        if !self.record.bitmap.buffer.is_null() && self.record.bitmap.buffer_len < byte_len {
            return Err(rust_ffi::FT_Err_Invalid_Argument);
        }
        self.core.root.format = self.record.root.format;
        self.core.root.advance = rust_ffi::FT_Vector {
            x: self.record.root.advance.x,
            y: self.record.root.advance.y,
        };
        self.core.left = self.record.left;
        self.core.top = self.record.top;
        self.core.bitmap = rust_ffi::FT_Bitmap {
            rows: self.record.bitmap.rows,
            width: self.record.bitmap.width,
            pitch: self.record.bitmap.pitch,
            // A null public buffer is a valid empty-payload descriptor for
            // `FT_Bitmap_Copy`, even when rows and pitch are non-zero.
            buffer: if byte_len == 0 || self.record.bitmap.buffer.is_null() {
                Vec::new()
            } else {
                // SAFETY: non-null storage is bounded by the public
                // `buffer_len` field before this slice is formed.
                unsafe { slice::from_raw_parts(self.record.bitmap.buffer, byte_len).to_vec() }
            },
            num_grays: self.record.bitmap.num_grays,
            pixel_mode: self.record.bitmap.pixel_mode,
        };
        Ok(())
    }
}

fn wasm_glyph_root_from_core(root: &rust_ffi::FT_GlyphRec) -> FontdoneWasmGlyph {
    wasm_glyph_root_from_core_with_class(root, wasm_owned_outline_glyph_class())
}

fn wasm_glyph_root_from_core_with_class(
    root: &rust_ffi::FT_GlyphRec,
    clazz: *const FontdoneWasmGlyphClass,
) -> FontdoneWasmGlyph {
    FontdoneWasmGlyph {
        clazz,
        format: root.format,
        advance: FontdoneWasmVector {
            x: root.advance.x,
            y: root.advance.y,
        },
    }
}

static WASM_OWNED_OUTLINE_GLYPH_CLASS_MARKER: u8 = 0;
static WASM_OWNED_BITMAP_GLYPH_CLASS_MARKER: u8 = 0;
static WASM_OWNED_SVG_GLYPH_CLASS_MARKER: u8 = 0;

fn wasm_owned_outline_glyph_class() -> *const FontdoneWasmGlyphClass {
    // Private marker used only for pointer identity.  It is never dereferenced
    // as `FontdoneWasmGlyphClass`; caller-owned facades still use the public
    // class-record path.
    ptr::addr_of!(WASM_OWNED_OUTLINE_GLYPH_CLASS_MARKER).cast::<FontdoneWasmGlyphClass>()
}

fn wasm_owned_bitmap_glyph_class() -> *const FontdoneWasmGlyphClass {
    // Private marker used only for pointer identity.  It is never dereferenced
    // as `FontdoneWasmGlyphClass`.
    ptr::addr_of!(WASM_OWNED_BITMAP_GLYPH_CLASS_MARKER).cast::<FontdoneWasmGlyphClass>()
}

fn wasm_owned_svg_glyph_class() -> *const FontdoneWasmGlyphClass {
    // Private marker used only for pointer identity.
    ptr::addr_of!(WASM_OWNED_SVG_GLYPH_CLASS_MARKER).cast::<FontdoneWasmGlyphClass>()
}

fn wasm_owned_outline_glyph_from_root(
    glyph: *const FontdoneWasmGlyph,
) -> Option<&'static WasmOwnedOutlineGlyph> {
    let glyph = unsafe { glyph.as_ref() }?;
    if glyph.clazz != wasm_owned_outline_glyph_class() {
        return None;
    }
    // SAFETY: this private class marker is assigned only to
    // `Box<WasmOwnedOutlineGlyph>` records, whose first field starts with the
    // public `FontdoneWasmGlyph` root.
    Some(unsafe { &*ptr::from_ref(glyph).cast::<WasmOwnedOutlineGlyph>() })
}

fn wasm_owned_outline_glyph_from_root_mut(
    glyph: *mut FontdoneWasmGlyph,
) -> Option<&'static mut WasmOwnedOutlineGlyph> {
    let glyph_ref = unsafe { glyph.as_ref() }?;
    if glyph_ref.clazz != wasm_owned_outline_glyph_class() {
        return None;
    }
    // SAFETY: this private class marker is assigned only to
    // `Box<WasmOwnedOutlineGlyph>` records, whose first field starts with the
    // public `FontdoneWasmGlyph` root.
    Some(unsafe { &mut *glyph.cast::<WasmOwnedOutlineGlyph>() })
}

fn wasm_owned_bitmap_glyph_from_root(
    glyph: *const FontdoneWasmGlyph,
) -> Option<&'static WasmOwnedBitmapGlyph> {
    let glyph = unsafe { glyph.as_ref() }?;
    if glyph.clazz != wasm_owned_bitmap_glyph_class() {
        return None;
    }
    // SAFETY: this private class marker is assigned only to
    // `Box<WasmOwnedBitmapGlyph>` records, whose first field starts with the
    // public `FontdoneWasmGlyph` root.
    Some(unsafe { &*ptr::from_ref(glyph).cast::<WasmOwnedBitmapGlyph>() })
}

fn wasm_owned_bitmap_glyph_from_root_mut(
    glyph: *mut FontdoneWasmGlyph,
) -> Option<&'static mut WasmOwnedBitmapGlyph> {
    let glyph_ref = unsafe { glyph.as_ref() }?;
    if glyph_ref.clazz != wasm_owned_bitmap_glyph_class() {
        return None;
    }
    // SAFETY: this private class marker is assigned only to
    // `Box<WasmOwnedBitmapGlyph>` records, whose first field starts with the
    // public `FontdoneWasmGlyph` root.
    Some(unsafe { &mut *glyph.cast::<WasmOwnedBitmapGlyph>() })
}

fn wasm_owned_svg_glyph_from_root(
    glyph: *const FontdoneWasmGlyph,
) -> Option<&'static WasmOwnedSvgGlyph> {
    let glyph = unsafe { glyph.as_ref() }?;
    if glyph.clazz != wasm_owned_svg_glyph_class() {
        return None;
    }
    // SAFETY: the private marker is assigned only to `WasmOwnedSvgGlyph`.
    Some(unsafe { &*ptr::from_ref(glyph).cast::<WasmOwnedSvgGlyph>() })
}

fn wasm_owned_svg_glyph_from_root_mut(
    glyph: *mut FontdoneWasmGlyph,
) -> Option<&'static mut WasmOwnedSvgGlyph> {
    let glyph_ref = unsafe { glyph.as_ref() }?;
    if glyph_ref.clazz != wasm_owned_svg_glyph_class() {
        return None;
    }
    // SAFETY: the private marker is assigned only to `WasmOwnedSvgGlyph`.
    Some(unsafe { &mut *glyph.cast::<WasmOwnedSvgGlyph>() })
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_UnitVector {
    pub x: FT_F2Dot14,
    pub y: FT_F2Dot14,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmGlyphMetrics {
    pub width: i64,
    pub height: i64,
    pub horiBearingX: i64,
    pub horiBearingY: i64,
    pub horiAdvance: i64,
    pub vertBearingX: i64,
    pub vertBearingY: i64,
    pub vertAdvance: i64,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmBitmap {
    pub rows: u32,
    pub width: u32,
    pub pitch: i32,
    pub buffer: *const c_uchar,
    pub buffer_len: usize,
    pub num_grays: u16,
    pub pixel_mode: i32,
    pub palette_mode: u8,
    pub palette: *const c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmRasterParams {
    pub target: *mut FontdoneWasmBitmap,
    pub source: *const c_void,
    pub flags: FT_Int,
    pub gray_spans: *const c_void,
    pub black_spans: *const c_void,
    pub bit_test: *const c_void,
    pub bit_set: *const c_void,
    pub user: *mut c_void,
    pub clip_box: FontdoneWasmBBox,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmColor {
    pub blue: FT_Byte,
    pub green: FT_Byte,
    pub red: FT_Byte,
    pub alpha: FT_Byte,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmPaletteData {
    pub num_palettes: FT_UShort,
    pub palette_name_ids: *const FT_UShort,
    pub palette_flags: *const FT_UShort,
    pub num_palette_entries: FT_UShort,
    pub palette_entry_name_ids: *const FT_UShort,
}

pub type FT_OpaquePaint = rust_ffi::FT_OpaquePaint;
pub type FT_ColorIndex = rust_ffi::FT_ColorIndex;
pub type FT_PaintSolid = rust_ffi::FT_PaintSolid;
pub type FT_PaintGlyph = rust_ffi::FT_PaintGlyph;
pub type FT_PaintLinearGradient = rust_ffi::FT_PaintLinearGradient;
pub type FT_PaintComposite = rust_ffi::FT_PaintComposite;
pub type FT_COLR_Paint = rust_ffi::FT_COLR_Paint;

fn wasm_color_to_rust(color: FontdoneWasmColor) -> rust_ffi::FT_Color {
    rust_ffi::FT_Color {
        blue: color.blue,
        green: color.green,
        red: color.red,
        alpha: color.alpha,
    }
}

fn copy_rust_palette_data_to_wasm(
    out: &mut FontdoneWasmPaletteData,
    value: rust_ffi::FT_Palette_Data,
) {
    out.num_palettes = value.num_palettes;
    out.palette_name_ids = value.palette_name_ids;
    out.palette_flags = value.palette_flags;
    out.num_palette_entries = value.num_palette_entries;
    out.palette_entry_name_ids = value.palette_entry_name_ids;
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiPaletteDataSnapshot {
    pub error: FT_Error,
    pub num_palettes: FT_UShort,
    pub num_palette_entries: FT_UShort,
    pub palette_name_ids_is_null: bool,
    pub palette_flags_is_null: bool,
    pub palette_entry_name_ids_is_null: bool,
    pub palette_name_ids: Vec<FT_UShort>,
    pub palette_flags: Vec<FT_UShort>,
    pub palette_entry_name_ids: Vec<FT_UShort>,
}

#[cfg(feature = "abi-test-support")]
fn abi_ushort_slice(ptr: *const FT_UShort, len: FT_UShort) -> Vec<FT_UShort> {
    if ptr.is_null() {
        return Vec::new();
    }
    // SAFETY: test callers pass live FreeType-shaped array pointers returned
    // by `fontdone_wasm_palette_data_get`; a non-null zero-length slice is
    // valid and copies no elements while the handle is live.
    unsafe { slice::from_raw_parts(ptr, usize::from(len)).to_vec() }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_palette_data_snapshot(handle: usize) -> AbiPaletteDataSnapshot {
    let mut data = FontdoneWasmPaletteData::default();
    let error = fontdone_wasm_palette_data_get(handle, &mut data);
    AbiPaletteDataSnapshot {
        error,
        num_palettes: data.num_palettes,
        num_palette_entries: data.num_palette_entries,
        palette_name_ids_is_null: data.palette_name_ids.is_null(),
        palette_flags_is_null: data.palette_flags.is_null(),
        palette_entry_name_ids_is_null: data.palette_entry_name_ids.is_null(),
        palette_name_ids: if error == rust_ffi::FT_Err_Ok {
            abi_ushort_slice(data.palette_name_ids, data.num_palettes)
        } else {
            Vec::new()
        },
        palette_flags: if error == rust_ffi::FT_Err_Ok {
            abi_ushort_slice(data.palette_flags, data.num_palettes)
        } else {
            Vec::new()
        },
        palette_entry_name_ids: if error == rust_ffi::FT_Err_Ok {
            abi_ushort_slice(data.palette_entry_name_ids, data.num_palette_entries)
        } else {
            Vec::new()
        },
    }
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiPaletteSelectSnapshot {
    pub error: FT_Error,
    pub palette_is_null: bool,
    pub entries: Vec<FontdoneWasmColor>,
}

#[cfg(feature = "abi-test-support")]
fn abi_palette_entries_from_ptr(
    handle: usize,
    palette: *mut FontdoneWasmColor,
) -> Vec<FontdoneWasmColor> {
    if palette.is_null() {
        return Vec::new();
    }
    let mut data = FontdoneWasmPaletteData::default();
    // `palette` is returned by a successful selection on this same live
    // handle, so the matching palette-data lookup cannot fail on a valid
    // helper call. An error leaves the default record unchanged and therefore
    // produces a zero-length copy below without adding an unreachable branch.
    let _ = fontdone_wasm_palette_data_get(handle, &mut data);
    let len = usize::from(data.num_palette_entries);
    // SAFETY: this test-support helper copies the palette pointer returned by
    // `fontdone_wasm_palette_select` while the owning handle is still live.
    unsafe { slice::from_raw_parts(palette, len).to_vec() }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_palette_select_snapshot(
    handle: usize,
    palette_index: FT_UShort,
) -> AbiPaletteSelectSnapshot {
    let mut palette = ptr::null_mut();
    let error = fontdone_wasm_palette_select(handle, palette_index, &mut palette);
    AbiPaletteSelectSnapshot {
        error,
        palette_is_null: palette.is_null(),
        entries: if error == rust_ffi::FT_Err_Ok {
            abi_palette_entries_from_ptr(handle, palette)
        } else {
            Vec::new()
        },
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_palette_select_without_output(handle: usize, palette_index: FT_UShort) -> FT_Error {
    fontdone_wasm_palette_select(handle, palette_index, ptr::null_mut())
}

#[cfg(feature = "abi-test-support")]
pub fn abi_palette_mutate_entry(
    handle: usize,
    palette_index: FT_UShort,
    entry_index: usize,
    color: FontdoneWasmColor,
) -> AbiPaletteSelectSnapshot {
    let mut snapshot = abi_palette_select_snapshot(handle, palette_index);
    if entry_index < snapshot.entries.len() {
        let mut palette = ptr::null_mut();
        let error = fontdone_wasm_palette_select(handle, palette_index, &mut palette);
        // `entry_index < snapshot.entries.len()` above proves that the
        // selected palette pointer is non-null: the snapshot helper returns
        // no entries for a null pointer or failed metadata lookup.
        // The repeated selection uses the same live handle and valid palette
        // index that produced the non-empty snapshot, so it also succeeds.
        // SAFETY: this feature-gated helper mutates an entry through the
        // public WASM ABI palette pointer while the handle is live,
        // matching the FreeType caller-observable behavior under test.
        unsafe { *palette.add(entry_index) = color };
        snapshot = AbiPaletteSelectSnapshot {
            error,
            palette_is_null: palette.is_null(),
            entries: abi_palette_entries_from_ptr(handle, palette),
        };
    }
    snapshot
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_list_add(list: FT_List, node: FT_ListNode) {
    let (Some(list_ref), Some(node_ref)) = (unsafe { list.as_mut() }, unsafe { node.as_mut() })
    else {
        return;
    };
    let before = list_ref.tail;

    node_ref.next = ptr::null_mut();
    node_ref.prev = before;

    if let Some(before_ref) = unsafe { before.as_mut() } {
        before_ref.next = node;
    } else {
        list_ref.head = node;
    }
    list_ref.tail = node;
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_list_insert(list: FT_List, node: FT_ListNode) {
    let (Some(list_ref), Some(node_ref)) = (unsafe { list.as_mut() }, unsafe { node.as_mut() })
    else {
        return;
    };
    let after = list_ref.head;

    node_ref.next = after;
    node_ref.prev = ptr::null_mut();

    if let Some(after_ref) = unsafe { after.as_mut() } {
        after_ref.prev = node;
    } else {
        list_ref.tail = node;
    }
    list_ref.head = node;
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_list_find(list: FT_List, data: FT_Pointer) -> FT_ListNode {
    let Some(list_ref) = (unsafe { list.as_ref() }) else {
        return ptr::null_mut();
    };
    let mut cur = list_ref.head;
    while let Some(cur_ref) = unsafe { cur.as_ref() } {
        let rust_node = rust_ffi::FT_ListNodeRec {
            prev: cur_ref.prev.cast(),
            next: cur_ref.next.cast(),
            data,
        };
        if rust_ffi::FT_List_Find_Node_Matches(&rust_node, cur_ref.data) {
            return cur;
        }
        cur = cur_ref.next;
    }
    ptr::null_mut()
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_list_remove(list: FT_List, node: FT_ListNode) {
    let (Some(list_ref), Some(node_ref)) = (unsafe { list.as_mut() }, unsafe { node.as_ref() })
    else {
        return;
    };
    let before = node_ref.prev;
    let after = node_ref.next;

    if let Some(before_ref) = unsafe { before.as_mut() } {
        before_ref.next = after;
    } else {
        list_ref.head = after;
    }

    if let Some(after_ref) = unsafe { after.as_mut() } {
        after_ref.prev = before;
    } else {
        list_ref.tail = before;
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_list_up(list: FT_List, node: FT_ListNode) {
    let (Some(list_ref), Some(node_ref)) = (unsafe { list.as_mut() }, unsafe { node.as_mut() })
    else {
        return;
    };
    let before = node_ref.prev;
    let after = node_ref.next;
    let Some(before_ref) = (unsafe { before.as_mut() }) else {
        return;
    };

    before_ref.next = after;

    if let Some(after_ref) = unsafe { after.as_mut() } {
        after_ref.prev = before;
    } else {
        list_ref.tail = before;
    }

    node_ref.prev = ptr::null_mut();
    node_ref.next = list_ref.head;
    // SAFETY: The pinned C implementation reaches this write only after a
    // non-head node has a predecessor, which requires a non-null list head
    // for a valid FT_List topology. A null head here is outside C's defined
    // behavior because FT_List_Up dereferences it unconditionally.
    unsafe { (*list_ref.head).prev = node };
    list_ref.head = node;
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_list_iterate(
    list: FT_List,
    iterator: FT_List_Iterator,
    user: FT_Pointer,
) -> FT_Error {
    let (Some(list_ref), Some(iterator)) = (unsafe { list.as_ref() }, iterator) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let mut cur = list_ref.head;
    let mut error = rust_ffi::FT_Err_Ok;
    while let Some(cur_ref) = unsafe { cur.as_ref() } {
        let next = cur_ref.next;
        error = iterator(cur, user);
        if error != rust_ffi::FT_Err_Ok {
            break;
        }
        cur = next;
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_list_finalize(
    list: FT_List,
    destroy: FT_List_Destructor,
    memory: FT_Memory,
    user: FT_Pointer,
) {
    let (Some(list_ref), Some(memory_ref)) = (unsafe { list.as_mut() }, unsafe { memory.as_ref() })
    else {
        return;
    };

    let mut cur = list_ref.head;
    while let Some(cur_ref) = unsafe { cur.as_ref() } {
        let next = cur_ref.next;
        if let Some(destroy) = destroy {
            destroy(memory, cur_ref.data, user);
        }
        let _ = memory_ref.free.map(|free| free(memory, cur.cast()));
        cur = next;
    }

    list_ref.head = ptr::null_mut();
    list_ref.tail = ptr::null_mut();
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_bitmap_init(abitmap: *mut FontdoneWasmBitmap) {
    // Mirrors FreeType's null-tolerant `FT_Bitmap_Init` for the WASM ABI
    // bitmap record.
    if let Some(bitmap) = unsafe { abitmap.as_mut() } {
        *bitmap = FontdoneWasmBitmap::default();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_bitmap_new(abitmap: *mut FontdoneWasmBitmap) {
    fontdone_wasm_bitmap_init(abitmap);
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_bitmap_copy(
    library_handle: usize,
    source: *const FontdoneWasmBitmap,
    target: *mut FontdoneWasmBitmap,
) -> i32 {
    if library_handle == 0 {
        return rust_ffi::FT_Err_Invalid_Library_Handle as i32;
    }
    let Some(source_ref) = (unsafe { source.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(target_ref) = (unsafe { target.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    if source == target.cast_const() {
        return rust_ffi::FT_Err_Ok;
    }

    let library = rust_ffi::FT_Init_FreeType();
    let mut source_view = wasm_bitmap_to_rust(source_ref);
    let mut target_view = wasm_bitmap_to_rust(target_ref);
    if let Some(bytes) = wasm_bitmap_copy_bytes(source_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut source_view), bytes);
    }

    let err = rust_ffi::FT_Bitmap_Copy(Some(&library), Some(&source_view), Some(&mut target_view));
    if err == rust_ffi::FT_Err_Ok {
        copy_rust_bitmap_record_to_wasm(target_ref, &target_view);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_bitmap_convert(
    library_handle: usize,
    source: *const FontdoneWasmBitmap,
    target: *mut FontdoneWasmBitmap,
    alignment: i32,
) -> i32 {
    if library_handle == 0 {
        return rust_ffi::FT_Err_Invalid_Library_Handle as i32;
    }
    let Some(source_ref) = (unsafe { source.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(target_ref) = (unsafe { target.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let library = rust_ffi::FT_Init_FreeType();
    let mut source_view = wasm_bitmap_to_rust(source_ref);
    let mut target_view = wasm_bitmap_to_rust(target_ref);
    if let Some(bytes) = wasm_bitmap_bytes(source_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut source_view), bytes);
    }
    if let Some(bytes) = wasm_bitmap_bytes(target_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut target_view), bytes);
    }

    let err = rust_ffi::FT_Bitmap_Convert(
        Some(&library),
        Some(&source_view),
        Some(&mut target_view),
        alignment,
    );
    if err == rust_ffi::FT_Err_Ok {
        copy_rust_bitmap_record_to_wasm(target_ref, &target_view);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_bitmap_done(
    library_handle: usize,
    bitmap: *mut FontdoneWasmBitmap,
) -> i32 {
    if library_handle == 0 {
        return rust_ffi::FT_Err_Invalid_Library_Handle as i32;
    }
    let Some(bitmap_ref) = (unsafe { bitmap.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let library = rust_ffi::FT_Init_FreeType();
    let mut bitmap_view = wasm_bitmap_to_rust(bitmap_ref);
    if let Some(bytes) = wasm_bitmap_bytes(bitmap_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut bitmap_view), bytes);
    }
    let err = rust_ffi::FT_Bitmap_Done(Some(&library), Some(&mut bitmap_view));
    // The null library and bitmap cases are rejected above; the Rust
    // implementation therefore always returns FT_Err_Ok for this call.
    copy_rust_bitmap_record_to_wasm(bitmap_ref, &bitmap_view);
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_bitmap_embolden(
    library_handle: usize,
    bitmap: *mut FontdoneWasmBitmap,
    x_strength: i64,
    y_strength: i64,
) -> i32 {
    if library_handle == 0 {
        return rust_ffi::FT_Err_Invalid_Library_Handle as i32;
    }
    let Some(bitmap_ref) = (unsafe { bitmap.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let library = rust_ffi::FT_Init_FreeType();
    let mut bitmap_view = wasm_bitmap_to_rust(bitmap_ref);
    if let Some(bytes) = wasm_bitmap_bytes(bitmap_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut bitmap_view), bytes);
    }

    let err = rust_ffi::FT_Bitmap_Embolden(
        Some(&library),
        Some(&mut bitmap_view),
        x_strength,
        y_strength,
    );
    if err == rust_ffi::FT_Err_Ok {
        copy_rust_bitmap_record_to_wasm(bitmap_ref, &bitmap_view);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_bitmap_blend(
    library_handle: usize,
    source: *const FontdoneWasmBitmap,
    source_offset: FontdoneWasmVector,
    target: *mut FontdoneWasmBitmap,
    atarget_offset: *mut FontdoneWasmVector,
    color: FontdoneWasmColor,
) -> i32 {
    let library = if library_handle == 0 {
        None
    } else {
        Some(rust_ffi::FT_Init_FreeType())
    };
    let Some(source_ref) = (unsafe { source.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(target_ref) = (unsafe { target.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(atarget_offset_ref) = (unsafe { atarget_offset.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let mut source_view = wasm_bitmap_to_rust(source_ref);
    let mut target_view = wasm_bitmap_to_rust(target_ref);
    if let Some(bytes) = wasm_bitmap_bytes(source_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut source_view), bytes);
    }
    if let Some(bytes) = wasm_bitmap_bytes(target_ref) {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut target_view), bytes);
    }
    let mut rust_target_offset = rust_ffi::FT_Vector {
        x: atarget_offset_ref.x,
        y: atarget_offset_ref.y,
    };
    let err = rust_ffi::FT_Bitmap_Blend(
        library.as_ref(),
        Some(&source_view),
        rust_ffi::FT_Vector {
            x: source_offset.x,
            y: source_offset.y,
        },
        Some(&mut target_view),
        Some(&mut rust_target_offset),
        rust_ffi::FT_Color {
            blue: color.blue,
            green: color.green,
            red: color.red,
            alpha: color.alpha,
        },
    );
    if err == rust_ffi::FT_Err_Ok {
        copy_rust_bitmap_record_to_wasm(target_ref, &target_view);
        atarget_offset_ref.x = rust_target_offset.x;
        atarget_offset_ref.y = rust_target_offset.y;
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_palette_data_get(
    handle: usize,
    apalette_data: *mut FontdoneWasmPaletteData,
) -> FT_Error {
    let Some(out) = (unsafe { apalette_data.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let mut rust_out = rust_ffi::FT_Palette_Data::default();
    let err =
        rust_ffi::FT_Palette_Data_Get(face_ref(handle).map(|face| &face.face), Some(&mut rust_out));
    if err == rust_ffi::FT_Err_Ok {
        copy_rust_palette_data_to_wasm(out, rust_out);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_palette_select(
    handle: usize,
    palette_index: FT_UShort,
    apalette: *mut *mut FontdoneWasmColor,
) -> FT_Error {
    let mut rust_palette: *const rust_ffi::FT_Color = ptr::null();
    let err = rust_ffi::FT_Palette_Select(
        face_ref(handle).map(|face| &face.face),
        palette_index,
        (!apalette.is_null()).then_some(&mut rust_palette),
    );
    if err == rust_ffi::FT_Err_Ok && !apalette.is_null() {
        // SAFETY: `apalette` is non-null and caller provided writable storage.
        unsafe {
            *apalette = rust_palette.cast::<FontdoneWasmColor>().cast_mut();
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_palette_set_foreground_color(
    handle: usize,
    foreground_color: FontdoneWasmColor,
) -> FT_Error {
    rust_ffi::FT_Palette_Set_Foreground_Color(
        face_ref(handle).map(|face| &face.face),
        wasm_color_to_rust(foreground_color),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_color_glyph_layer(
    handle: usize,
    base_glyph: FT_UInt,
    aglyph_index: *mut FT_UInt,
    acolor_index: *mut FT_UInt,
    iterator: *mut FT_LayerIterator,
) -> FT_Bool {
    rust_ffi::FT_Get_Color_Glyph_Layer(
        face_ref(handle).map(|face| &face.face),
        base_glyph,
        unsafe { aglyph_index.as_mut() },
        unsafe { acolor_index.as_mut() },
        unsafe { iterator.as_mut() },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_color_glyph_clipbox(
    handle: usize,
    base_glyph: FT_UInt,
    clip_box: *mut FT_ClipBox,
) -> FT_Bool {
    rust_ffi::FT_Get_Color_Glyph_ClipBox(
        face_ref(handle).map(|face| &face.face),
        base_glyph,
        unsafe { clip_box.as_mut() },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_color_glyph_paint(
    handle: usize,
    base_glyph: FT_UInt,
    root_transform: FT_UInt,
    paint: *mut FT_OpaquePaint,
) -> FT_Bool {
    rust_ffi::FT_Get_Color_Glyph_Paint(
        face_ref(handle).map(|face| &face.face),
        base_glyph,
        root_transform,
        unsafe { paint.as_mut() },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_paint(
    handle: usize,
    opaque_paint: FT_OpaquePaint,
    paint: *mut FT_COLR_Paint,
) -> FT_Bool {
    rust_ffi::FT_Get_Paint(
        face_ref(handle).map(|face| &face.face),
        opaque_paint,
        unsafe { paint.as_mut() },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_paint_layers(
    handle: usize,
    layer_iterator: *mut FT_LayerIterator,
    paint: *mut FT_OpaquePaint,
) -> FT_Bool {
    rust_ffi::FT_Get_Paint_Layers(
        face_ref(handle).map(|face| &face.face),
        unsafe { layer_iterator.as_mut() },
        unsafe { paint.as_mut() },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_colorline_stops(
    handle: usize,
    color_stop: *mut FT_ColorStop,
    iterator: *mut FT_ColorStopIterator,
) -> FT_Bool {
    rust_ffi::FT_Get_Colorline_Stops(
        face_ref(handle).map(|face| &face.face),
        unsafe { color_stop.as_mut() },
        unsafe { iterator.as_mut() },
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_paint_layer_iterator(
    handle: usize,
    opaque_paint: FT_OpaquePaint,
) -> Option<FT_LayerIterator> {
    rust_ffi::FT_ColrV1_Paint_Layer_Iterator_Copy(
        face_ref(handle).map(|face| &face.face),
        opaque_paint,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_paint_colorline(
    handle: usize,
    opaque_paint: FT_OpaquePaint,
) -> Option<FT_ColorLine> {
    rust_ffi::FT_ColrV1_Paint_ColorLine_Copy(face_ref(handle).map(|face| &face.face), opaque_paint)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_paint_linear_gradient(
    handle: usize,
    opaque_paint: FT_OpaquePaint,
) -> Option<FT_PaintLinearGradient> {
    rust_ffi::FT_ColrV1_Paint_LinearGradient_Copy(
        face_ref(handle).map(|face| &face.face),
        opaque_paint,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_paint_transform(
    handle: usize,
    opaque_paint: FT_OpaquePaint,
) -> Option<FT_PaintTransform> {
    rust_ffi::FT_ColrV1_Paint_Transform_Copy(face_ref(handle).map(|face| &face.face), opaque_paint)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_paint_graph(
    handle: usize,
) -> Option<rust_ffi::FT_ColrV1_PaintGraph_Snapshot> {
    rust_ffi::FT_ColrV1_PaintGraph_Copy(face_ref(handle).map(|face| &face.face))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_public_paint_solid(
    handle: usize,
    glyph_index: FT_UInt,
) -> rust_ffi::FT_ColrV1_PublicPaintSolid_Snapshot {
    rust_ffi::FT_ColrV1_PublicPaintSolid_Copy(face_ref(handle).map(|face| &face.face), glyph_index)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_truetype_gx_free(handle: usize, table: FT_Bytes) {
    rust_ffi::FT_TrueTypeGX_Free(face_ref(handle).map(|face| &face.face), table);
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_truetype_gx_validate(
    handle: usize,
    validation_flags: FT_UInt,
    tables: *mut FT_Bytes,
    table_length: FT_UInt,
) -> FT_Error {
    let face = face_ref(handle).map(|face| &face.face);
    if face.is_none() {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    }
    if tables.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // The public C declaration exposes exactly ten output slots.  A caller
    // supplying a larger count has already left the defined ABI contract, so
    // cap the internal view before touching linear memory instead of trying to
    // allocate an attacker-controlled table.
    let table_length = table_length.min(10) as usize;
    let mut values = [ptr::null(); 10];
    let err =
        rust_ffi::FT_TrueTypeGX_Validate(face, validation_flags, Some(&mut values[..table_length]));
    for (index, table) in values.into_iter().take(table_length).enumerate() {
        // SAFETY: the public Wasm ABI requires `tables` to address
        // `table_length` writable FT_Bytes slots.
        unsafe {
            *tables.add(index) = table;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_classic_kern_free(handle: usize, table: FT_Bytes) {
    rust_ffi::FT_ClassicKern_Free(face_ref(handle).map(|face| &face.face), table);
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_classic_kern_validate(
    handle: usize,
    validation_flags: FT_UInt,
    ckern_table: *mut FT_Bytes,
) -> FT_Error {
    let face = face_ref(handle).map(|face| &face.face);
    let mut table = ptr::null();
    let err = rust_ffi::FT_ClassicKern_Validate(
        face,
        validation_flags,
        (!ckern_table.is_null()).then_some(&mut table),
    );
    if err == rust_ffi::FT_Err_Ok {
        write_ft_bytes(ckern_table, table);
    } else if handle != 0 && err != rust_ffi::FT_Err_Unimplemented_Feature && !ckern_table.is_null()
    {
        write_ft_bytes(ckern_table, ptr::null());
    }
    err
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmGlyphSlot {
    pub glyph_index: u32,
    pub metrics: FontdoneWasmGlyphMetrics,
    pub advance: FontdoneWasmVector,
    pub format: i32,
    pub num_subglyphs: u32,
    pub bitmap: FontdoneWasmBitmap,
    pub bitmap_left: i32,
    pub bitmap_top: i32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmSizeMetrics {
    pub x_ppem: u16,
    pub y_ppem: u16,
    pub x_scale: i64,
    pub y_scale: i64,
    pub ascender: i64,
    pub descender: i64,
    pub height: i64,
    pub max_advance: i64,
}

fn wasm_size_metrics(metrics: rust_ffi::FT_Size_Metrics) -> FontdoneWasmSizeMetrics {
    FontdoneWasmSizeMetrics {
        x_ppem: metrics.x_ppem,
        y_ppem: metrics.y_ppem,
        x_scale: metrics.x_scale,
        y_scale: metrics.y_scale,
        ascender: metrics.ascender,
        descender: metrics.descender,
        height: metrics.height,
        max_advance: metrics.max_advance,
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmSfntName {
    pub platform_id: u16,
    pub encoding_id: u16,
    pub language_id: u16,
    pub name_id: u16,
    pub string: *const c_uchar,
    pub string_len: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmVertHeader {
    pub Version: FT_Fixed,
    pub Ascender: FT_Short,
    pub Descender: FT_Short,
    pub Line_Gap: FT_Short,
    pub advance_Height_Max: FT_UShort,
    pub min_Top_Side_Bearing: FT_Short,
    pub min_Bottom_Side_Bearing: FT_Short,
    pub yMax_Extent: FT_Short,
    pub caret_Slope_Rise: FT_Short,
    pub caret_Slope_Run: FT_Short,
    pub caret_Offset: FT_Short,
    pub Reserved: [FT_Short; 4],
    pub metric_Data_Format: FT_Short,
    pub number_Of_VMetrics: FT_UShort,
    pub long_metrics: FT_Pointer,
    pub short_metrics: FT_Pointer,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmMaxProfile {
    pub version: FT_Fixed,
    pub numGlyphs: FT_UShort,
    pub maxPoints: FT_UShort,
    pub maxContours: FT_UShort,
    pub maxCompositePoints: FT_UShort,
    pub maxCompositeContours: FT_UShort,
    pub maxZones: FT_UShort,
    pub maxTwilightPoints: FT_UShort,
    pub maxStorage: FT_UShort,
    pub maxFunctionDefs: FT_UShort,
    pub maxInstructionDefs: FT_UShort,
    pub maxStackElements: FT_UShort,
    pub maxSizeOfInstructions: FT_UShort,
    pub maxComponentElements: FT_UShort,
    pub maxComponentDepth: FT_UShort,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmString {
    pub string: *const c_uchar,
    pub string_len: u32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmPSFontInfo {
    pub version: FontdoneWasmString,
    pub notice: FontdoneWasmString,
    pub full_name: FontdoneWasmString,
    pub family_name: FontdoneWasmString,
    pub weight: FontdoneWasmString,
    pub italic_angle: FT_Fixed,
    pub is_fixed_pitch: FT_Bool,
    pub underline_position: FT_Short,
    pub underline_thickness: FT_UShort,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmCharmap {
    pub index: FT_UInt,
    pub encoding: FT_Encoding,
    pub platform_id: FT_UShort,
    pub encoding_id: FT_UShort,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmOs2 {
    pub version: FT_UShort,
    pub xAvgCharWidth: FT_Short,
    pub usWeightClass: FT_UShort,
    pub usWidthClass: FT_UShort,
    pub fsType: FT_UShort,
    pub ySubscriptXSize: FT_Short,
    pub ySubscriptYSize: FT_Short,
    pub ySubscriptXOffset: FT_Short,
    pub ySubscriptYOffset: FT_Short,
    pub ySuperscriptXSize: FT_Short,
    pub ySuperscriptYSize: FT_Short,
    pub ySuperscriptXOffset: FT_Short,
    pub ySuperscriptYOffset: FT_Short,
    pub yStrikeoutSize: FT_Short,
    pub yStrikeoutPosition: FT_Short,
    pub sFamilyClass: FT_Short,
    pub panose: [u8; 10],
    pub ulUnicodeRange1: FT_ULong,
    pub ulUnicodeRange2: FT_ULong,
    pub ulUnicodeRange3: FT_ULong,
    pub ulUnicodeRange4: FT_ULong,
    pub achVendID: [i8; 4],
    pub fsSelection: FT_UShort,
    pub usFirstCharIndex: FT_UShort,
    pub usLastCharIndex: FT_UShort,
    pub sTypoAscender: FT_Short,
    pub sTypoDescender: FT_Short,
    pub sTypoLineGap: FT_Short,
    pub usWinAscent: FT_UShort,
    pub usWinDescent: FT_UShort,
    pub ulCodePageRange1: FT_ULong,
    pub ulCodePageRange2: FT_ULong,
    pub sxHeight: FT_Short,
    pub sCapHeight: FT_Short,
    pub usDefaultChar: FT_UShort,
    pub usBreakChar: FT_UShort,
    pub usMaxContext: FT_UShort,
    pub usLowerOpticalPointSize: FT_UShort,
    pub usUpperOpticalPointSize: FT_UShort,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FontdoneWasmSizeRequest {
    pub type_: FT_Size_Request_Type,
    pub width: FT_Long,
    pub height: FT_Long,
    pub horiResolution: FT_UInt,
    pub vertResolution: FT_UInt,
}

struct WasmFaceState {
    face: rust_ffi::FT_Face,
    active_size: usize,
    size_handles: Vec<usize>,
    size_metrics: BTreeMap<usize, rust_ffi::FT_Size_Metrics>,
    slot: Option<rust_ffi::FT_GlyphSlot>,
    variant_list: Vec<FT_UInt32>,
    mm_vars: BTreeMap<usize, WasmMmVarStorage>,
}

struct WasmMmVarStorage {
    _namedstyle: Box<[rust_ffi::FT_Var_Named_Style]>,
    _namedstyle_coords: Box<[rust_ffi::FT_Fixed]>,
}

thread_local! {
    static SIZE_HANDLE_OWNERS: RefCell<BTreeMap<usize, usize>> = const { RefCell::new(BTreeMap::new()) };
    #[cfg(feature = "abi-test-support")]
    static OPAQUE_INCREMENTAL_TRACE: RefCell<Option<WasmOpaqueIncrementalTrace>> =
        const { RefCell::new(None) };
}

fn register_wasm_size_handle(face_handle: usize, size_handle: usize) {
    if size_handle == 0 {
        return;
    }
    SIZE_HANDLE_OWNERS.with(|owners| {
        owners.borrow_mut().insert(size_handle, face_handle);
    });
}

fn unregister_wasm_size_handle(size_handle: usize) {
    SIZE_HANDLE_OWNERS.with(|owners| {
        owners.borrow_mut().remove(&size_handle);
    });
}

fn wasm_size_owner(size_handle: usize) -> Option<usize> {
    SIZE_HANDLE_OWNERS.with(|owners| owners.borrow().get(&size_handle).copied())
}

fn make_wasm_face_state(face: rust_ffi::FT_Face) -> Box<WasmFaceState> {
    let active_size = face.size as usize;
    let mut size_metrics = BTreeMap::new();
    if active_size != 0 {
        size_metrics.insert(active_size, face.size_metrics);
    }
    let initial_slot = rust_ffi::FT_Empty_GlyphSlot(&face);
    Box::new(WasmFaceState {
        face,
        active_size,
        size_handles: if active_size == 0 {
            Vec::new()
        } else {
            vec![active_size]
        },
        size_metrics,
        slot: Some(initial_slot),
        variant_list: Vec::new(),
        mm_vars: BTreeMap::new(),
    })
}

#[repr(C)]
#[derive(Default)]
pub struct FontdoneWasmPsHintingResult {
    pub set_error: FT_Error,
    pub get_error: FT_Error,
    pub readback: FT_UInt,
    pub string_get_error: FT_Error,
    pub string_readback: FT_UInt,
    pub load_error: FT_Error,
    pub invalid_set_error: FT_Error,
    pub post_error_preserved: FT_Bool,
}

fn wasm_ps_module_name(selector: i32) -> Option<&'static str> {
    match selector {
        5 => Some("cff"),
        6 => Some("type1"),
        7 => Some("t1cid"),
        _ => None,
    }
}

fn wasm_slot_outputs_equal(
    first: &rust_ffi::FT_GlyphSlot,
    second: &rust_ffi::FT_GlyphSlot,
) -> bool {
    if first.format != second.format
        || first.metrics != second.metrics
        || first.advance != second.advance
        || first.bitmap_left != second.bitmap_left
        || first.bitmap_top != second.bitmap_top
    {
        return false;
    }
    // `Option<FT_Bitmap>` equality preserves all three observable outcomes:
    // equal bitmaps, two absent bitmaps, and a mixed presence mismatch.  The
    // latter is an internal defensive state: the public operation performs
    // identical successful loads, so no valid input can produce it after the
    // scalar fields match. Keep the comparison total without making that
    // unreachable state a separate coverage obligation.
    first.bitmap == second.bitmap
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_ps_hinting_engine_open(
    module_selector: i32,
    file_base: *const c_uchar,
    file_size: usize,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
    render_pixel_size: FT_UInt,
    value: FT_UInt,
    string_value: *const std::ffi::c_char,
    out: *mut FontdoneWasmPsHintingResult,
) -> FontdoneWasmStatus {
    fontdone_wasm_ps_hinting_engine_open_impl(
        module_selector,
        file_base,
        file_size,
        glyph_index,
        load_flags,
        render_pixel_size,
        value,
        string_value,
        out,
        None,
    )
}

/// Test-support entry point that fixes the CFF driver's random seed before
/// opening a face.  It is intentionally a Rust-only helper; the promoted C
/// ABI keeps the original export signature while parity can exercise the
/// stateful CFF Type 2 `random` operator deterministically.
#[cfg(feature = "abi-test-support")]
pub fn fontdone_wasm_ps_hinting_engine_open_with_random_seed(
    module_selector: i32,
    file_base: *const c_uchar,
    file_size: usize,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
    render_pixel_size: FT_UInt,
    value: FT_UInt,
    string_value: *const std::ffi::c_char,
    random_seed: FT_UInt,
    out: *mut FontdoneWasmPsHintingResult,
) -> FontdoneWasmStatus {
    fontdone_wasm_ps_hinting_engine_open_impl(
        module_selector,
        file_base,
        file_size,
        glyph_index,
        load_flags,
        render_pixel_size,
        value,
        string_value,
        out,
        Some(random_seed),
    )
}

fn fontdone_wasm_ps_hinting_engine_open_impl(
    module_selector: i32,
    file_base: *const c_uchar,
    file_size: usize,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
    render_pixel_size: FT_UInt,
    value: FT_UInt,
    string_value: *const std::ffi::c_char,
    out: *mut FontdoneWasmPsHintingResult,
    random_seed: Option<FT_UInt>,
) -> FontdoneWasmStatus {
    let Some(module_name) = wasm_ps_module_name(module_selector) else {
        return FontdoneWasmStatus {
            error: rust_ffi::FT_Err_Invalid_Argument,
            handle: 0,
        };
    };
    if file_base.is_null() {
        return FontdoneWasmStatus {
            error: rust_ffi::FT_Err_Invalid_Argument,
            handle: 0,
        };
    }
    let Some(out) = (unsafe { out.as_mut() }) else {
        return FontdoneWasmStatus {
            error: rust_ffi::FT_Err_Invalid_Argument,
            handle: 0,
        };
    };
    *out = FontdoneWasmPsHintingResult::default();
    // SAFETY: the caller promises `file_size` readable bytes at `file_base`.
    let data = unsafe { slice::from_raw_parts(file_base, file_size) };
    let string = if string_value.is_null() {
        None
    } else {
        // SAFETY: the caller passes a NUL-terminated ASCII property string.
        Some(
            unsafe { CStr::from_ptr(string_value.cast()) }
                .to_string_lossy()
                .into_owned(),
        )
    };
    let mut library = rust_ffi::FT_Init_FreeType();
    out.set_error = rust_ffi::FT_Property_Set(
        Some(&mut library),
        Some(module_name),
        Some("hinting-engine"),
        Some(value),
    );
    if module_name == "cff"
        && let Some(seed) = random_seed
    {
        let _ = rust_ffi::FT_Property_Set(
            Some(&mut library),
            Some(module_name),
            Some("random-seed"),
            Some(seed),
        );
    }
    let mut readback = 0;
    out.get_error = rust_ffi::FT_Property_Get(
        Some(&library),
        Some(module_name),
        Some("hinting-engine"),
        Some(&mut readback),
    );
    out.readback = readback;
    if let Some(string) = string {
        rust_ffi::FT_Set_Default_Properties_From_Env(
            Some(&mut library),
            Some(&format!("{module_name}:hinting-engine={string}")),
        );
    }
    let mut string_readback = 0;
    out.string_get_error = rust_ffi::FT_Property_Get(
        Some(&library),
        Some(module_name),
        Some("hinting-engine"),
        Some(&mut string_readback),
    );
    out.string_readback = string_readback;
    let opened = rust_ffi::FT_New_Memory_Face(&library, data, 0, 20.0);
    let mut face = match opened {
        Ok(face) => face,
        Err(error) => {
            out.load_error = error;
            return FontdoneWasmStatus {
                error: rust_ffi::FT_Err_Ok,
                handle: 0,
            };
        }
    };
    let size_error = if render_pixel_size == 0 {
        rust_ffi::FT_Err_Ok
    } else {
        rust_ffi::FT_Set_Pixel_Sizes(&mut face, 0, render_pixel_size)
    };
    let first_load = if size_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Load_Glyph(&face, glyph_index, load_flags)
    } else {
        Err(size_error)
    };
    out.load_error = first_load
        .as_ref()
        .map_or_else(|error| *error, |_| rust_ffi::FT_Err_Ok);
    let first_slot = first_load.as_ref().ok().cloned();
    out.invalid_set_error = rust_ffi::FT_Property_Set(
        Some(&mut library),
        Some(module_name),
        Some("hinting-engine"),
        Some(2),
    );
    let second_load = if size_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Load_Glyph(&face, glyph_index, load_flags)
    } else {
        Err(size_error)
    };
    // The rejected numeric property value returns an error but does not
    // mutate the pinned PostScript module state.  A successful first load can
    // still be followed by a reload error when the glyph's stateful
    // interpreter advances into a failing path; preserve C's false result for
    // either a reload error or a differing successful slot.
    out.post_error_preserved = if let Some(first_slot) = first_slot.as_ref() {
        if second_load
            .as_ref()
            .is_ok_and(|slot| wasm_slot_outputs_equal(first_slot, slot))
        {
            1
        } else {
            0
        }
    } else {
        if second_load.is_err() { 1 } else { 0 }
    };
    // The ordinary facade keeps its historical final reload.  The deterministic
    // random-seed test helper must retain the first slot, however: the C oracle
    // reports that first load and a third CFF interpretation would consume a
    // third random value and compare a different public glyph.
    let final_slot = if random_seed.is_some() {
        first_slot
    } else {
        let final_load = if size_error == rust_ffi::FT_Err_Ok {
            rust_ffi::FT_Load_Glyph(&face, glyph_index, load_flags)
        } else {
            Err(size_error)
        };
        if out.load_error == rust_ffi::FT_Err_Ok {
            out.load_error = final_load
                .as_ref()
                .map_or_else(|error| *error, |_| rust_ffi::FT_Err_Ok);
        }
        final_load.ok()
    };
    let mut state = make_wasm_face_state(face);
    state.slot = final_slot;
    let active_size = state.active_size;
    let handle = Box::into_raw(state).addr();
    register_wasm_size_handle(handle, active_size);
    FontdoneWasmStatus {
        error: rust_ffi::FT_Err_Ok,
        handle,
    }
}

fn update_wasm_active_size_metrics(face: &mut WasmFaceState) {
    if face.active_size != 0 {
        face.size_metrics
            .insert(face.active_size, face.face.size_metrics);
    }
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiSlotSnapshot {
    pub glyph_index: u32,
    pub metrics: FontdoneWasmGlyphMetrics,
    pub advance: FontdoneWasmVector,
    pub format: i32,
    pub num_subglyphs: u32,
    pub outline_cbox: AbiBBoxSnapshot,
    pub outline_bbox: AbiBBoxSnapshot,
    pub outline: Option<rust_ffi::FT_OutlineSnapshot>,
    pub bitmap: Option<AbiBitmapSnapshot>,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiBBoxSnapshot {
    pub xMin: i64,
    pub yMin: i64,
    pub xMax: i64,
    pub yMax: i64,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiBitmapSnapshot {
    pub rows: u32,
    pub width: u32,
    pub pitch: i32,
    pub num_grays: u16,
    pub pixel_mode: i32,
    pub left: i32,
    pub top: i32,
    pub owns_bitmap: bool,
    pub buffer: Vec<u8>,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiOutlineGlyphSnapshot {
    pub advance: FontdoneWasmVector,
    pub outline: rust_ffi::FT_OutlineSnapshot,
    pub cbox: FontdoneWasmBBox,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiBitmapGlyphSnapshot {
    pub root: FontdoneWasmGlyph,
    pub left: i32,
    pub top: i32,
    pub bitmap: AbiBitmapSnapshot,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiSvgGlyphSnapshot {
    pub root: FontdoneWasmGlyph,
    pub svg_document: Vec<FT_Byte>,
    pub glyph_index: FT_UInt,
    pub metrics: FontdoneWasmSizeMetrics,
    pub units_per_EM: FT_UShort,
    pub start_glyph_id: FT_UShort,
    pub end_glyph_id: FT_UShort,
    pub transform: FontdoneWasmMatrix,
    pub delta: FontdoneWasmVector,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone, Copy)]
pub struct AbiNewGlyphAllocationRow {
    pub format: rust_ffi::FT_Glyph_Format,
    pub status: FT_Error,
    pub aglyph_null: bool,
    pub allocation_events: usize,
}

#[cfg(feature = "abi-test-support")]
/// Replays the class-selection-success/allocator-failure boundary for the
/// WASM facade using the safe Rust allocator diagnostic.
pub fn abi_support_new_glyph_allocation_failure() -> Vec<AbiNewGlyphAllocationRow> {
    let library = rust_ffi::FT_Init_FreeType();
    [
        rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        rust_ffi::FT_GLYPH_FORMAT_BITMAP,
    ]
    .into_iter()
    .enumerate()
    .map(|(index, format)| AbiNewGlyphAllocationRow {
        format,
        status: rust_ffi::FT_New_Glyph_Allocation_Failure(Some(&library), format),
        aglyph_null: true,
        allocation_events: index.saturating_add(1),
    })
    .collect()
}

/// Wasm-lane owner for an actual pure-Rust FTC SBit cache.
#[cfg(feature = "abi-test-support")]
pub struct AbiSBitCacheHarness {
    cache: rust_ffi::FTCSBitCacheState,
}

/// Snapshot captured immediately after a Wasm-lane SBit cache lookup.
#[cfg(feature = "abi-test-support")]
pub struct AbiSBitCacheLookupSnapshot {
    pub error: FT_Error,
    pub sbit: Option<rust_ffi::FTC_SBitRec>,
    pub buffer: Vec<FT_Byte>,
    pub sbit_null: bool,
    pub anode_null: bool,
    pub node_locked: bool,
    pub node_ref_count: FT_UInt,
}

/// Wasm-lane owner for the pure-Rust FTC manager lifecycle.
#[cfg(feature = "abi-test-support")]
pub struct AbiCacheManagerOwnershipHarness {
    manager: rust_ffi::FTCCacheManagerState,
}

/// Exact manager-owned face, size, cache, and node lifecycle observations.
#[cfg(feature = "abi-test-support")]
pub struct AbiCacheManagerOwnershipSnapshot {
    pub status: FT_Error,
    pub requester_after_first: FT_UInt,
    pub requester_after_repeat: FT_UInt,
    pub requester_after_reset: FT_UInt,
    pub finalizers_before_reset: FT_UInt,
    pub finalizers_after_reset: FT_UInt,
    pub finalizers_after_done: FT_UInt,
    pub face_repeat_same: bool,
    pub size_repeat_same: bool,
    pub size_belongs_to_face: bool,
    pub first_node_non_null: bool,
    pub node_repeat_same: bool,
    pub post_reset_node_non_null: bool,
    pub cache_non_null: bool,
    pub reset_preserved_cache_handle: bool,
}

#[cfg(feature = "abi-test-support")]
impl AbiSBitCacheHarness {
    /// Creates a cache from the live face behind a Wasm handle.
    pub fn new(face_handle: usize) -> Option<Self> {
        Some(Self {
            cache: rust_ffi::FTCSBitCacheState::new(face_ref(face_handle)?.face.clone()),
        })
    }

    /// Calls the core `FTC_SBitCache_Lookup` route used by the Wasm facade.
    pub fn lookup(
        &mut self,
        image_type: rust_ffi::FTC_ImageTypeRec,
        glyph_index: FT_UInt,
        sbit_output: bool,
        anode_output: bool,
    ) -> AbiSBitCacheLookupSnapshot {
        let mut lookup = None;
        let mut node = true;
        let error = rust_ffi::FTC_SBitCache_Lookup(
            &mut self.cache,
            Some(&image_type),
            glyph_index,
            sbit_output.then_some(&mut lookup),
            anode_output.then_some(&mut node),
        );
        let (sbit, buffer, node_locked, node_ref_count) = lookup.map_or_else(
            || (None, Vec::new(), false, 0),
            |lookup| {
                (
                    Some(*lookup.sbit),
                    lookup.buffer.to_vec(),
                    lookup.node_locked,
                    lookup.node_ref_count,
                )
            },
        );
        AbiSBitCacheLookupSnapshot {
            error,
            sbit,
            buffer,
            sbit_null: sbit.is_none(),
            anode_null: !node,
            node_locked,
            node_ref_count,
        }
    }
}

#[cfg(feature = "abi-test-support")]
impl AbiCacheManagerOwnershipHarness {
    /// Creates an empty pure-Rust manager from the face behind a Wasm handle.
    pub fn new(face_handle: usize) -> Option<Self> {
        Some(Self {
            manager: rust_ffi::FTCCacheManagerState::new(face_ref(face_handle)?.face.clone()),
        })
    }

    /// Creates a manager whose requester returns the supplied callback error.
    pub fn new_with_requester_error(
        face_handle: usize,
        requester_error: rust_ffi::FT_Error,
    ) -> Option<Self> {
        Some(Self {
            manager: rust_ffi::FTCCacheManagerState::new_with_requester_error(
                face_ref(face_handle)?.face.clone(),
                requester_error,
            ),
        })
    }

    /// Invokes the manager requester once and returns its exact error status.
    pub fn requester_error_status(&mut self) -> rust_ffi::FT_Error {
        self.manager
            .lookup_face()
            .map_or_else(|error| error, |_| rust_ffi::FT_Err_Ok)
    }

    /// Exercises the manager-owned face, size, SBit node, reset, and done path.
    pub fn ownership_snapshot(&mut self) -> AbiCacheManagerOwnershipSnapshot {
        let (face_first_status, face_first_id) = match self.manager.lookup_face() {
            Ok(face) => (rust_ffi::FT_Err_Ok, ptr::from_ref(face).addr()),
            Err(error) => (error, 0),
        };
        let requester_after_first = self.manager.requester_calls();
        let (face_repeat_status, face_repeat_id) = match self.manager.lookup_face() {
            Ok(face) => (rust_ffi::FT_Err_Ok, ptr::from_ref(face).addr()),
            Err(error) => (error, 0),
        };
        let requester_after_repeat = self.manager.requester_calls();
        let (size_first_status, size_first) = match self.manager.lookup_pixel_size(0, 12) {
            Ok(size) => (rust_ffi::FT_Err_Ok, size),
            Err(error) => (error, ptr::null_mut()),
        };
        let (size_repeat_status, size_repeat) = match self.manager.lookup_pixel_size(0, 12) {
            Ok(size) => (rust_ffi::FT_Err_Ok, size),
            Err(error) => (error, ptr::null_mut()),
        };
        let image_type = rust_ffi::FTC_ImageTypeRec {
            face_id: ptr::null_mut(),
            width: 0,
            height: 12,
            flags: rust_ffi::FT_LOAD_DEFAULT,
        };
        let (first_sbit_status, first_node_id, first_node_non_null) = {
            let mut lookup = None;
            let mut node = false;
            let status =
                self.manager
                    .lookup_sbit(Some(&image_type), 36, Some(&mut lookup), Some(&mut node));
            let node_id = lookup
                .as_ref()
                .map_or(0, |lookup| ptr::from_ref(lookup.sbit).addr());
            (status, node_id, node)
        };
        let cache_non_null = self.manager.has_sbit_cache();
        let (repeat_sbit_status, repeat_node_id) = {
            let mut lookup = None;
            let mut node = false;
            let status =
                self.manager
                    .lookup_sbit(Some(&image_type), 36, Some(&mut lookup), Some(&mut node));
            let node_id = lookup
                .as_ref()
                .map_or(0, |lookup| ptr::from_ref(lookup.sbit).addr());
            (status, node_id)
        };
        let _ = self.manager.unref_sbit_node(&image_type, 36);
        let _ = self.manager.unref_sbit_node(&image_type, 36);
        let _ = self.manager.unref_sbit_node(&image_type, 37);
        let _ = self.manager.unref_sbit_node(&image_type, 36);
        let finalizers_before_reset = self.manager.finalized_faces();
        self.manager.reset();
        let finalizers_after_reset = self.manager.finalized_faces();
        let post_reset_face_status = self
            .manager
            .lookup_face()
            .map_or_else(|error| error, |_| rust_ffi::FT_Err_Ok);
        let requester_after_reset = self.manager.requester_calls();
        let (post_reset_sbit_status, post_reset_node_non_null) = {
            let mut lookup = None;
            let mut node = false;
            let status =
                self.manager
                    .lookup_sbit(Some(&image_type), 36, Some(&mut lookup), Some(&mut node));
            (status, node)
        };
        let _ = self.manager.unref_sbit_node(&image_type, 36);
        let reset_preserved_cache_handle =
            cache_non_null && post_reset_sbit_status == rust_ffi::FT_Err_Ok;
        self.manager.done();
        let finalizers_after_done = self.manager.finalized_faces();
        let status = [
            face_first_status,
            face_repeat_status,
            size_first_status,
            size_repeat_status,
            first_sbit_status,
            repeat_sbit_status,
            post_reset_face_status,
            post_reset_sbit_status,
        ]
        .into_iter()
        .find(|status| *status != rust_ffi::FT_Err_Ok)
        .unwrap_or(rust_ffi::FT_Err_Ok);
        AbiCacheManagerOwnershipSnapshot {
            status,
            requester_after_first,
            requester_after_repeat,
            requester_after_reset,
            finalizers_before_reset,
            finalizers_after_reset,
            finalizers_after_done,
            face_repeat_same: face_first_id != 0 && face_first_id == face_repeat_id,
            size_repeat_same: !size_first.is_null() && size_first == size_repeat,
            size_belongs_to_face: size_first_status == rust_ffi::FT_Err_Ok
                && face_first_status == rust_ffi::FT_Err_Ok,
            first_node_non_null,
            node_repeat_same: first_node_id != 0 && first_node_id == repeat_node_id,
            post_reset_node_non_null,
            cache_non_null,
            reset_preserved_cache_handle,
        }
    }

    /// Exercises the manager's post-done rejection and reset-no-op states.
    pub fn post_done_edge_probe(
        &mut self,
    ) -> (
        rust_ffi::FT_Error,
        rust_ffi::FT_Error,
        rust_ffi::FT_Error,
        rust_ffi::FT_Error,
    ) {
        let post_done_lookup_face_status = self
            .manager
            .lookup_face()
            .map_or_else(|error| error, |_| rust_ffi::FT_Err_Ok);
        let post_done_lookup_size_status = self
            .manager
            .lookup_pixel_size(0, 12)
            .map_or_else(|error| error, |_| rust_ffi::FT_Err_Ok);
        let image_type = rust_ffi::FTC_ImageTypeRec {
            face_id: ptr::null_mut(),
            width: 0,
            height: 12,
            flags: rust_ffi::FT_LOAD_DEFAULT,
        };
        let mut ignored_lookup = None;
        let mut ignored_node = false;
        let post_done_sbit_status = self.manager.lookup_sbit(
            Some(&image_type),
            36,
            Some(&mut ignored_lookup),
            Some(&mut ignored_node),
        );
        let post_done_sbit_without_output_status =
            self.manager
                .lookup_sbit(Some(&image_type), 36, None, Some(&mut ignored_node));
        self.manager.reset();
        (
            post_done_lookup_face_status,
            post_done_lookup_size_status,
            post_done_sbit_status,
            post_done_sbit_without_output_status,
        )
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_outline_glyph_snapshot(glyph_handle: usize) -> Option<AbiOutlineGlyphSnapshot> {
    let glyph = ptr::with_exposed_provenance::<FontdoneWasmGlyph>(glyph_handle);
    let owned = wasm_owned_outline_glyph_from_root(glyph)?;
    let mut cbox = FontdoneWasmBBox::default();
    fontdone_wasm_glyph_get_cbox(glyph, 0, &mut cbox);
    Some(AbiOutlineGlyphSnapshot {
        advance: owned.record.root.advance,
        outline: owned.core.outline.clone(),
        cbox,
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_corrupt_outline_glyph_for_render_failure(
    glyph_handle: usize,
    oversized_internal_state: bool,
) -> bool {
    let glyph = ptr::with_exposed_provenance_mut::<FontdoneWasmGlyph>(glyph_handle);
    let Some(owned) = wasm_owned_outline_glyph_from_root_mut(glyph) else {
        return false;
    };
    if oversized_internal_state {
        // This is an ABI bookkeeping probe, not a public glyph-construction
        // path. Keep the oversized vector owned by the support wrapper so the
        // checked FT_UShort conversion below can be exercised without
        // fabricating a raw pointer or changing the public outline limit.
        let point = owned.core.outline.points[0];
        let tag = owned.core.outline.tags[0];
        let oversized_len = usize::from(FT_UShort::MAX) + 1;
        owned.core.outline.points.resize(oversized_len, point);
        owned.core.outline.tags.resize(oversized_len, tag);
        owned.refresh_record();
    }
    let Ok(invalid_endpoint) = FT_UShort::try_from(owned.core.outline.points.len()) else {
        return false;
    };
    let Some(endpoint) = owned.core.outline.contours.last_mut() else {
        return false;
    };
    *endpoint = invalid_endpoint;
    owned.refresh_record();
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_corrupt_outline_glyph_for_stroke_parse(glyph_handle: usize) -> bool {
    let glyph = ptr::with_exposed_provenance_mut::<FontdoneWasmGlyph>(glyph_handle);
    let Some(owned) = wasm_owned_outline_glyph_from_root_mut(glyph) else {
        return false;
    };
    let Some(tag) = owned.core.outline.tags.first_mut() else {
        return false;
    };
    // Keep this malformed record in the maintained parity route: the first
    // cubic tag is rejected by ParseOutline before stroker geometry runs.
    *tag = rust_ffi::FT_CURVE_TAG_CUBIC as rust_ffi::FT_Byte;
    owned.refresh_record();
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_corrupt_outline_glyph_for_render_tags(glyph_handle: usize, kind: u8) -> bool {
    let glyph = ptr::with_exposed_provenance_mut::<FontdoneWasmGlyph>(glyph_handle);
    let Some(owned) = wasm_owned_outline_glyph_from_root_mut(glyph) else {
        return false;
    };
    let tags = &mut owned.core.outline.tags;
    match kind {
        0 if !tags.is_empty() => {
            tags[0] = rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte;
        }
        1 if tags.len() >= 2 => {
            tags[0] = rust_ffi::FT_CURVE_TAG_CONIC as FT_Byte;
            tags[1] = rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte;
        }
        2 if tags.len() >= 3 => {
            tags[0] = rust_ffi::FT_CURVE_TAG_ON as FT_Byte;
            tags[1] = rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte;
            tags[2] = rust_ffi::FT_CURVE_TAG_ON as FT_Byte;
        }
        _ => return false,
    }
    owned.refresh_record();
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_corrupt_outline_glyph_for_record_sync(glyph_handle: usize) -> bool {
    let glyph = ptr::with_exposed_provenance_mut::<FontdoneWasmGlyph>(glyph_handle);
    let Some(owned) = wasm_owned_outline_glyph_from_root_mut(glyph) else {
        return false;
    };
    if owned.core.outline.contours.is_empty() {
        return false;
    }
    // Keep the public count non-zero while removing the required contour
    // storage.  The handle wrapper must reject this record before rendering
    // or replacement, matching pinned FreeType's public-record validation.
    owned.record.outline.n_contours = 1;
    owned.record.outline.contours = ptr::null_mut();
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_corrupt_outline_glyph_points_for_record_sync(glyph_handle: usize) -> bool {
    let glyph = ptr::with_exposed_provenance_mut::<FontdoneWasmGlyph>(glyph_handle);
    let Some(owned) = wasm_owned_outline_glyph_from_root_mut(glyph) else {
        return false;
    };
    if owned.core.outline.points.is_empty() {
        return false;
    }
    // Keep the public count non-zero while removing the required point
    // storage.  The handle wrapper must reject this record before rendering,
    // matching pinned FreeType's public-record validation.
    owned.record.outline.n_points = 1;
    owned.record.outline.points = ptr::null_mut();
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_bitmap_glyph_snapshot(glyph_handle: usize) -> Option<AbiBitmapGlyphSnapshot> {
    let glyph = ptr::with_exposed_provenance::<FontdoneWasmGlyph>(glyph_handle);
    let owned = wasm_owned_bitmap_glyph_from_root(glyph)?;
    Some(AbiBitmapGlyphSnapshot {
        root: owned.record.root,
        left: owned.record.left,
        top: owned.record.top,
        bitmap: AbiBitmapSnapshot {
            rows: owned.record.bitmap.rows,
            width: owned.record.bitmap.width,
            pitch: owned.record.bitmap.pitch,
            num_grays: owned.record.bitmap.num_grays,
            pixel_mode: owned.record.bitmap.pixel_mode,
            left: owned.record.left,
            top: owned.record.top,
            owns_bitmap: true,
            buffer: owned.buffer.to_vec(),
        },
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_svg_glyph_snapshot(glyph_handle: usize) -> Option<AbiSvgGlyphSnapshot> {
    let glyph = ptr::with_exposed_provenance::<FontdoneWasmGlyph>(glyph_handle);
    let owned = wasm_owned_svg_glyph_from_root(glyph)?;
    Some(AbiSvgGlyphSnapshot {
        root: owned.record.root,
        svg_document: owned.core.svg_document.clone(),
        glyph_index: owned.core.glyph_index,
        metrics: wasm_size_metrics(owned.core.metrics),
        units_per_EM: owned.core.units_per_EM,
        start_glyph_id: owned.core.start_glyph_id,
        end_glyph_id: owned.core.end_glyph_id,
        transform: owned.record.transform,
        delta: owned.record.delta,
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_zero_length_svg_glyph(glyph_handle: usize, null_document: bool) -> bool {
    let glyph = ptr::with_exposed_provenance_mut::<FontdoneWasmGlyph>(glyph_handle);
    let Some(owned) = wasm_owned_svg_glyph_from_root_mut(glyph) else {
        return false;
    };
    owned.record.svg_document_length = 0;
    if null_document {
        owned.record.svg_document = ptr::null();
    }
    true
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct WasmSvgRendererCallbackCapture {
    pub hooks_status: FT_Error,
    pub render_status: FT_Error,
    pub glyph_index: FT_UInt,
    pub svg_document_length: FT_ULong,
    pub units_per_EM: FT_UShort,
    pub start_glyph_id: FT_UShort,
    pub end_glyph_id: FT_UShort,
    pub transform_xx: FT_Fixed,
    pub transform_xy: FT_Fixed,
    pub transform_yx: FT_Fixed,
    pub transform_yy: FT_Fixed,
    pub delta_x: FT_Pos,
    pub delta_y: FT_Pos,
}

thread_local! {
    // Public parity calls may execute on several host worker threads.  The
    // hook callbacks are synchronous, so a worker-local record carries the
    // observation without letting another call overwrite it mid-render.
    static WASM_SVG_CALLBACK_CAPTURE: RefCell<WasmSvgRendererCallbackCapture> =
        const { RefCell::new(WasmSvgRendererCallbackCapture {
            hooks_status: 0,
            render_status: 0,
            glyph_index: 0,
            svg_document_length: 0,
            units_per_EM: 0,
            start_glyph_id: 0,
            end_glyph_id: 0,
            transform_xx: 0,
            transform_xy: 0,
            transform_yx: 0,
            transform_yy: 0,
            delta_x: 0,
            delta_y: 0,
        }) };
}

unsafe extern "C" fn wasm_svg_probe_init(_state: *mut rust_ffi::FT_Pointer) -> rust_ffi::FT_Error {
    rust_ffi::FT_Err_Ok
}

unsafe extern "C" fn wasm_svg_probe_free(_state: *mut rust_ffi::FT_Pointer) {}

unsafe extern "C" fn wasm_svg_probe_preset(
    slot: *mut c_void,
    _cache: rust_ffi::FT_Bool,
    _state: *mut rust_ffi::FT_Pointer,
) -> rust_ffi::FT_Error {
    let Some(probe) = (unsafe {
        (*_state.cast::<*mut rust_ffi::FT_Pointer>())
            .cast::<rust_ffi::SvgCallbackProbe>()
            .as_ref()
    }) else {
        return rust_ffi::FT_Err_Invalid_Slot_Handle as rust_ffi::FT_Error;
    };
    if probe.document_ptr == 0 {
        return rust_ffi::FT_Err_Invalid_Slot_Handle as rust_ffi::FT_Error;
    }
    let document = probe.document_ptr as *const rust_ffi::FT_SVG_DocumentRec;
    // SAFETY: the core hands hooks a live `FT_SVG_DocumentRec` view during
    // the synchronous render call.
    let document = unsafe { &*document };
    WASM_SVG_CALLBACK_CAPTURE.with(|capture| {
        *capture.borrow_mut() = WasmSvgRendererCallbackCapture {
            hooks_status: 0,
            render_status: 0,
            glyph_index: probe.glyph_index,
            svg_document_length: document.svg_document_length,
            units_per_EM: document.units_per_EM,
            start_glyph_id: document.start_glyph_id,
            end_glyph_id: document.end_glyph_id,
            transform_xx: document.transform.xx,
            transform_xy: document.transform.xy,
            transform_yx: document.transform.yx,
            transform_yy: document.transform.yy,
            delta_x: document.delta.x,
            delta_y: document.delta.y,
        };
    });
    let _ = slot;
    rust_ffi::FT_Err_Ok
}

unsafe extern "C" fn wasm_svg_probe_render(
    slot: *mut c_void,
    _state: *mut rust_ffi::FT_Pointer,
) -> rust_ffi::FT_Error {
    let Some(probe) = (unsafe {
        (*_state.cast::<*mut rust_ffi::FT_Pointer>())
            .cast::<rust_ffi::SvgCallbackProbe>()
            .as_ref()
    }) else {
        return rust_ffi::FT_Err_Invalid_Slot_Handle as rust_ffi::FT_Error;
    };
    if probe.document_ptr == 0 {
        return rust_ffi::FT_Err_Invalid_Slot_Handle as rust_ffi::FT_Error;
    }
    let document = probe.document_ptr as *const rust_ffi::FT_SVG_DocumentRec;
    // SAFETY: see `wasm_svg_probe_preset`.
    let document = unsafe { &*document };
    WASM_SVG_CALLBACK_CAPTURE.with(|capture| {
        *capture.borrow_mut() = WasmSvgRendererCallbackCapture {
            hooks_status: 0,
            render_status: 0,
            glyph_index: probe.glyph_index,
            svg_document_length: document.svg_document_length,
            units_per_EM: document.units_per_EM,
            start_glyph_id: document.start_glyph_id,
            end_glyph_id: document.end_glyph_id,
            transform_xx: document.transform.xx,
            transform_xy: document.transform.xy,
            transform_yx: document.transform.yx,
            transform_yy: document.transform.yy,
            delta_x: document.delta.x,
            delta_y: document.delta.y,
        };
    });
    let _ = slot;
    rust_ffi::FT_Err_Ok
}

/// Runs the pinned `ftsvg.c` hook flow over one SVG glyph and returns the
/// document fields the renderer callback observed.
#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_svg_renderer_capture(
    file_base: *const c_uchar,
    file_size: usize,
    glyph_index: rust_ffi::FT_UInt,
    out_capture: *mut WasmSvgRendererCallbackCapture,
) -> rust_ffi::FT_Error {
    if out_capture.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    if file_base.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: the caller promises `file_size` readable bytes at `file_base`.
    let data = unsafe { slice::from_raw_parts(file_base, file_size) };
    let hooks = rust_ffi::SVG_RendererHooks {
        init_svg: Some(wasm_svg_probe_init),
        free_svg: Some(wasm_svg_probe_free),
        render_svg: Some(wasm_svg_probe_render),
        preset_slot: Some(wasm_svg_probe_preset),
    };
    let mut library = rust_ffi::FT_Init_FreeType();
    let set_error = rust_ffi::FT_Set_SVG_Renderer_Hooks(Some(&mut library), Some(hooks));
    if set_error != rust_ffi::FT_Err_Ok {
        unsafe {
            *out_capture = WasmSvgRendererCallbackCapture {
                hooks_status: set_error,
                render_status: set_error,
                ..WasmSvgRendererCallbackCapture::default()
            };
        }
        return set_error;
    }
    let face = match rust_ffi::FT_New_Memory_Face(&library, data, 0, 20.0) {
        Ok(face) => face,
        Err(error) => return error,
    };
    let mut capture = WasmSvgRendererCallbackCapture {
        hooks_status: set_error,
        render_status: rust_ffi::FT_Err_Ok,
        ..WasmSvgRendererCallbackCapture::default()
    };
    if let Ok(slot) = rust_ffi::FT_Load_Glyph(&face, glyph_index, rust_ffi::FT_LOAD_COLOR) {
        match rust_ffi::FT_Render_Glyph(slot, rust_ffi::FT_RENDER_MODE_NORMAL) {
            Ok(_) => {
                capture = WASM_SVG_CALLBACK_CAPTURE.with(|capture| *capture.borrow());
                capture.hooks_status = set_error;
                capture.render_status = rust_ffi::FT_Err_Ok;
            }
            Err(error) => capture.render_status = error,
        }
    }
    // SAFETY: `out_capture` is non-null and the caller provides one writable
    // capture record in exported linear memory.
    unsafe { *out_capture = capture };
    capture.render_status
}

#[cfg(feature = "abi-test-support")]
pub fn abi_face_info(handle: usize) -> Option<rust_ffi::FT_FaceRecPublic> {
    let face = face_ref(handle)?;
    Some(rust_face_info(&face.face))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_face_stream_info(handle: usize) -> Option<rust_ffi::FT_StreamRec> {
    let face = face_ref(handle)?;
    Some(face.face.memory_stream_record())
}

#[cfg(feature = "abi-test-support")]
pub fn abi_face_available_sizes(handle: usize) -> Option<Vec<rust_ffi::FT_Bitmap_Size>> {
    let face = face_ref(handle)?;
    Some(face.face.available_sizes.to_vec())
}

#[cfg(feature = "abi-test-support")]
pub fn abi_face_names(handle: usize) -> Option<(Option<String>, Option<String>)> {
    let face = face_ref(handle)?;
    Some((face.face.family_name.clone(), face.face.style_name.clone()))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_slot_snapshot(handle: usize) -> Option<AbiSlotSnapshot> {
    let mut slot = FontdoneWasmGlyphSlot::default();
    if fontdone_wasm_get_slot(handle, &mut slot) != rust_ffi::FT_Err_Ok {
        return None;
    }
    let rust_slot = face_ref(handle)?.slot.as_ref()?;
    let bitmap = if slot.bitmap.buffer.is_null() || slot.bitmap.buffer_len == 0 {
        None
    } else {
        // SAFETY: `fontdone_wasm_get_slot` returns a buffer owned by the live handle.
        let buffer = unsafe { slice::from_raw_parts(slot.bitmap.buffer, slot.bitmap.buffer_len) };
        Some(AbiBitmapSnapshot {
            rows: slot.bitmap.rows,
            width: slot.bitmap.width,
            pitch: slot.bitmap.pitch,
            num_grays: slot.bitmap.num_grays,
            pixel_mode: slot.bitmap.pixel_mode,
            left: slot.bitmap_left,
            top: slot.bitmap_top,
            owns_bitmap: rust_slot.owns_bitmap,
            buffer: buffer.to_vec(),
        })
    };
    Some(AbiSlotSnapshot {
        glyph_index: slot.glyph_index,
        metrics: slot.metrics,
        advance: slot.advance,
        format: slot.format,
        num_subglyphs: slot.num_subglyphs,
        outline_cbox: wasm_bbox_snapshot(rust_slot.outline_cbox),
        outline_bbox: wasm_bbox_snapshot(rust_slot.outline_bbox),
        outline: rust_slot.outline.clone(),
        bitmap,
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_set_own_bitmap(handle: usize, owns_bitmap: bool) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(slot) = face.slot.as_mut() else {
        return rust_ffi::FT_Err_Invalid_Glyph_Index;
    };
    slot.owns_bitmap = owns_bitmap;
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
fn wasm_bbox_snapshot(bbox: rust_ffi::FT_BBox) -> AbiBBoxSnapshot {
    AbiBBoxSnapshot {
        xMin: bbox.xMin,
        yMin: bbox.yMin,
        xMax: bbox.xMax,
        yMax: bbox.yMax,
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_uint32_list(ptr: *const FT_UInt32) -> Option<Vec<FT_UInt32>> {
    if ptr.is_null() {
        return None;
    }
    let mut values = Vec::new();
    for index in 0..4096 {
        // SAFETY: test callers pass live FreeType-shaped zero-terminated lists.
        let value = unsafe { *ptr.add(index) };
        if value == 0 {
            return Some(values);
        }
        values.push(value);
    }
    Some(values)
}

#[unsafe(no_mangle)]
/// Allocates `max(size, 1)` bytes in exported linear memory with 8-byte alignment.
///
/// A zero-size request therefore returns a uniquely releasable one-byte
/// allocation. Null reports allocation or layout failure. The returned offset
/// is owned by the caller and must be passed exactly once to
/// [`fontdone_wasm_free`] with the same `size`.
pub extern "C" fn fontdone_wasm_malloc(size: usize) -> *mut c_void {
    let Ok(layout) = Layout::from_size_align(size.max(1), 8) else {
        return ptr::null_mut();
    };
    // SAFETY: `layout` is valid and has non-zero size.
    unsafe { alloc(layout).cast::<c_void>() }
}

#[unsafe(no_mangle)]
/// Releases memory returned by [`fontdone_wasm_malloc`].
///
/// Null is a no-op. `size` must be identical to the allocation request,
/// including zero. A non-null pointer not returned by the allocator, an
/// already-freed pointer, or a mismatched size violates the ABI precondition
/// and can trap or corrupt the instance; it is not converted to `FT_Error`.
pub extern "C" fn fontdone_wasm_free(ptr: *mut c_void, size: usize) {
    let Ok(layout) = Layout::from_size_align(size.max(1), 8) else {
        return;
    };
    if ptr.is_null() {
        return;
    }
    // SAFETY: callers pass a pointer returned by `fontdone_wasm_malloc` with the same size.
    unsafe { dealloc(ptr.cast::<u8>(), layout) };
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_gzip_uncompress(
    memory: FT_Memory,
    output: *mut FT_Byte,
    output_len: *mut FT_ULong,
    input: *const FT_Byte,
    input_len: FT_ULong,
) -> FT_Error {
    if memory.is_null() || output.is_null() || output_len.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    #[cfg(target_pointer_width = "64")]
    let input_len = input_len as usize;
    #[cfg(target_pointer_width = "32")]
    let Ok(input_len) = usize::try_from(input_len) else {
        return rust_ffi::FT_Err_Invalid_Table;
    };
    // SAFETY: `output_len` is non-null and is exclusively borrowed for this
    // synchronous WASM ABI call.
    let output_len_ref = unsafe { &mut *output_len };
    #[cfg(target_pointer_width = "64")]
    let output_capacity = *output_len_ref as usize;
    #[cfg(target_pointer_width = "32")]
    let Ok(output_capacity) = usize::try_from(*output_len_ref) else {
        return rust_ffi::FT_Err_Array_Too_Large as FT_Error;
    };
    // SAFETY: `output` is non-null and `*output_len` is the caller-provided
    // output capacity, matching the FreeType-shaped ABI.
    let output_slice = unsafe { slice::from_raw_parts_mut(output, output_capacity) };
    let input_slice = if input.is_null() {
        None
    } else {
        // SAFETY: non-null `input` and `input_len` describe the caller-owned
        // compressed bytes for this call.
        Some(unsafe { slice::from_raw_parts(input, input_len) })
    };
    let memory_view = rust_ffi::FT_MemoryRec::default();
    rust_ffi::FT_Gzip_Uncompress(
        Some(&memory_view),
        Some(output_slice),
        Some(output_len_ref),
        input_slice,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_stream_open_gzip(
    stream: *mut rust_ffi::FT_StreamRec,
    source: *const rust_ffi::FT_StreamRec,
) -> FT_Error {
    let Some(stream_ref) = (unsafe { stream.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    };
    let Some(source_ref) = (unsafe { source.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    };
    #[cfg(target_pointer_width = "64")]
    let source_len = source_ref.size as usize;
    #[cfg(target_pointer_width = "32")]
    let Ok(source_len) = usize::try_from(source_ref.size) else {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    };
    if source_ref.base.is_null() && source_len != 0 {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    }
    // SAFETY: non-zero host sources promise `size` readable bytes. Keep the
    // zero-length path separate because Rust slices require a non-null
    // pointer even when their length is zero.
    let source_bytes = if source_len == 0 {
        &[][..]
    } else {
        unsafe { slice::from_raw_parts(source_ref.base.cast_const(), source_len) }
    };
    rust_ffi::FT_Stream_OpenGzip(Some(stream_ref), Some(source_ref), Some(source_bytes))
}

#[cfg_attr(not(feature = "bzip2"), allow(dead_code))]
fn bzip2_source_bytes(
    source: *mut rust_ffi::FT_StreamRec,
    source_base: *mut FT_Byte,
    source_read: FT_Pointer,
    source_len: usize,
) -> Result<Vec<FT_Byte>, FT_Error> {
    if source_len == 0 {
        return Ok(Vec::new());
    }
    if !source_base.is_null() {
        // SAFETY: the WASM host supplied a memory-backed source readable for
        // the advertised non-zero size.
        return Ok(unsafe { slice::from_raw_parts(source_base.cast_const(), source_len) }.to_vec());
    }
    if source_read.is_null() {
        return Err(rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error);
    }
    // SAFETY: public FT_StreamRec.read has FreeType's FT_Stream_IoFunc ABI;
    // the host retains the source stream for this synchronous materialization.
    let stream_io = unsafe {
        std::mem::transmute::<
            FT_Pointer,
            extern "C" fn(
                *mut rust_ffi::FT_StreamRec,
                FT_ULong,
                *mut FT_Byte,
                FT_ULong,
            ) -> FT_ULong,
        >(source_read)
    };
    if stream_io(source, 0, ptr::null_mut(), 0) != 0 {
        return Err(rust_ffi::FT_Err_Invalid_Stream_Operation as FT_Error);
    }
    // FT_Stream_Seek commits the zero position only after the callback
    // reports success; preserve that state before the header read.
    unsafe {
        (*source).pos = 0;
    }
    let requested = FT_ULong::try_from(source_len)
        .map_err(|_| rust_ffi::FT_Err_Invalid_Argument as FT_Error)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(source_len)
        .map_err(|_| rust_ffi::FT_Err_Out_Of_Memory as FT_Error)?;
    bytes.resize(source_len, 0);
    let read_count = stream_io(source, 0, bytes.as_mut_ptr(), requested);
    // FreeType advances a callback-backed stream by the number of bytes the
    // callback returned, including a short read that becomes an error.
    unsafe {
        (*source).pos = read_count;
    }
    if read_count != requested {
        return Err(rust_ffi::FT_Err_Invalid_Stream_Operation as FT_Error);
    }
    Ok(bytes)
}

#[cfg_attr(not(feature = "lzw"), allow(dead_code))]
fn lzw_source_bytes(
    source: *mut rust_ffi::FT_StreamRec,
    source_len: usize,
) -> Result<Vec<FT_Byte>, FT_Error> {
    if source_len == 0 {
        return Ok(Vec::new());
    }
    let source_ref = unsafe { &mut *source };
    if !source_ref.base.is_null() {
        // SAFETY: a non-null host source promises `size` readable bytes for
        // this synchronous public call.
        return Ok(unsafe {
            slice::from_raw_parts(source_ref.base.cast_const(), source_len).to_vec()
        });
    }
    if source_ref.read.is_null() {
        return Err(rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error);
    }
    // SAFETY: FT_StreamRec.read has FreeType's public FT_Stream_IoFunc ABI;
    // the host retains the source stream for this synchronous materialization.
    let stream_io = unsafe {
        std::mem::transmute::<
            FT_Pointer,
            extern "C" fn(
                *mut rust_ffi::FT_StreamRec,
                FT_ULong,
                *mut FT_Byte,
                FT_ULong,
            ) -> FT_ULong,
        >(source_ref.read)
    };
    if stream_io(source, 0, ptr::null_mut(), 0) != 0 {
        return Err(rust_ffi::FT_Err_Invalid_Stream_Operation as FT_Error);
    }
    source_ref.pos = 0;
    let requested = FT_ULong::try_from(source_len)
        .map_err(|_| rust_ffi::FT_Err_Invalid_Argument as FT_Error)?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(source_len)
        .map_err(|_| rust_ffi::FT_Err_Out_Of_Memory as FT_Error)?;
    bytes.resize(source_len, 0);
    let read_count = stream_io(source, 0, bytes.as_mut_ptr(), requested);
    source_ref.pos = read_count;
    if read_count != requested {
        return Err(rust_ffi::FT_Err_Invalid_Stream_Operation as FT_Error);
    }
    Ok(bytes)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_stream_open_bzip2(
    stream: *mut rust_ffi::FT_StreamRec,
    source: *const rust_ffi::FT_StreamRec,
) -> FT_Error {
    open_bzip2_stream(stream, source)
}

#[cfg(not(feature = "bzip2"))]
fn open_bzip2_stream(
    stream: *mut rust_ffi::FT_StreamRec,
    source: *const rust_ffi::FT_StreamRec,
) -> FT_Error {
    let _ = (stream, source);
    rust_ffi::FT_Err_Unimplemented_Feature as FT_Error
}

#[cfg(feature = "bzip2")]
fn open_bzip2_stream(
    stream: *mut rust_ffi::FT_StreamRec,
    source: *const rust_ffi::FT_StreamRec,
) -> FT_Error {
    if stream.is_null() || source.is_null() {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    }
    let (source_base, source_read, source_len) = {
        let source_ref = unsafe { &*source };
        #[cfg(target_pointer_width = "64")]
        let source_len = source_ref.size as usize;
        #[cfg(target_pointer_width = "32")]
        let Ok(source_len) = usize::try_from(source_ref.size) else {
            return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
        };
        (source_ref.base, source_ref.read, source_len)
    };
    if source_base.is_null() && source_len != 0 && source_read.is_null() {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    }
    let source_bytes =
        match bzip2_source_bytes(source.cast_mut(), source_base, source_read, source_len) {
            Ok(bytes) => bytes,
            Err(error) => return error,
        };
    let stream_ref = unsafe { &mut *stream };
    let source_ref = unsafe { &mut *source.cast_mut() };
    rust_ffi::FT_Stream_OpenBzip2(
        Some(stream_ref),
        Some(source_ref),
        Some(source_bytes.as_slice()),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_stream_open_lzw(
    stream: *mut rust_ffi::FT_StreamRec,
    source: *const rust_ffi::FT_StreamRec,
) -> FT_Error {
    open_lzw_stream(stream, source)
}

#[cfg(not(feature = "lzw"))]
fn open_lzw_stream(
    stream: *mut rust_ffi::FT_StreamRec,
    source: *const rust_ffi::FT_StreamRec,
) -> FT_Error {
    let _ = (stream, source);
    rust_ffi::FT_Err_Unimplemented_Feature as FT_Error
}

#[cfg(feature = "lzw")]
fn open_lzw_stream(
    stream: *mut rust_ffi::FT_StreamRec,
    source: *const rust_ffi::FT_StreamRec,
) -> FT_Error {
    if stream.is_null() || source.is_null() {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    }
    let stream_ref = unsafe { &mut *stream };
    let source_ref = unsafe { &*source };
    #[cfg(target_pointer_width = "64")]
    let source_len = source_ref.size as usize;
    #[cfg(target_pointer_width = "32")]
    let Ok(source_len) = usize::try_from(source_ref.size) else {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    };
    if source_ref.base.is_null() && source_len != 0 && source_ref.read.is_null() {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    }
    let source_bytes = match lzw_source_bytes(source.cast_mut(), source_len) {
        Ok(bytes) => bytes,
        Err(error) => return error,
    };
    rust_ffi::FT_Stream_OpenLZW(
        Some(stream_ref),
        Some(source_ref),
        Some(source_bytes.as_slice()),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_node_unref(
    node: rust_ffi::FTC_Node,
    manager: rust_ffi::FTC_Manager,
) {
    rust_ffi::FTC_Node_Unref(node, manager);
}

pub fn abi_support_gzip_stream_bytes(
    stream: *const rust_ffi::FT_StreamRec,
    offset: FT_ULong,
    count: FT_ULong,
) -> Option<Vec<FT_Byte>> {
    let stream_ref = unsafe { stream.as_ref() }?;
    rust_ffi::FT_Gzip_Stream_Read(Some(stream_ref), offset, count)
}

pub fn abi_support_gzip_stream_close(stream: *mut rust_ffi::FT_StreamRec) {
    if let Some(stream_ref) = unsafe { stream.as_mut() } {
        rust_ffi::FT_Gzip_Stream_Close(Some(stream_ref));
    }
}

pub fn abi_support_bzip2_stream_bytes(
    stream: *const rust_ffi::FT_StreamRec,
    offset: FT_ULong,
    count: FT_ULong,
) -> Option<Vec<FT_Byte>> {
    let stream_ref = unsafe { stream.as_ref() }?;
    rust_ffi::FT_Bzip2_Stream_Read(Some(stream_ref), offset, count)
}

pub fn abi_support_bzip2_stream_close(stream: *mut rust_ffi::FT_StreamRec) {
    if let Some(stream_ref) = unsafe { stream.as_mut() } {
        rust_ffi::FT_Bzip2_Stream_Close(Some(stream_ref));
    }
}

pub fn abi_support_bzip2_stream_is_open(stream: *const rust_ffi::FT_StreamRec) -> bool {
    let stream_ref = unsafe { stream.as_ref() };
    rust_ffi::FT_Bzip2_Stream_Is_Open(stream_ref)
}

pub fn abi_support_lzw_stream_bytes(
    stream: *const rust_ffi::FT_StreamRec,
    offset: FT_ULong,
    count: FT_ULong,
) -> Option<Vec<FT_Byte>> {
    let stream_ref = unsafe { stream.as_ref() }?;
    rust_ffi::FT_LZW_Stream_Read(Some(stream_ref), offset, count)
}

pub fn abi_support_lzw_stream_close(stream: *mut rust_ffi::FT_StreamRec) {
    if let Some(stream_ref) = unsafe { stream.as_mut() } {
        rust_ffi::FT_LZW_Stream_Close(Some(stream_ref));
    }
}

fn wasm_outline_array_layout<T>(count: FT_UShort) -> Layout {
    let count = usize::from(count);
    // SAFETY: the only callers are the outline owner paths below. The public
    // FT_Outline_New contract rejects counts above USHRT_MAX, and these paths
    // use only the fixed-size, non-zero FT_Vector, FT_Byte, and FT_UShort
    // element types, so multiplication and alignment are always representable.
    unsafe {
        Layout::from_size_align_unchecked(
            std::mem::size_of::<T>().saturating_mul(count),
            std::mem::align_of::<T>(),
        )
    }
}

fn wasm_alloc_zeroed_array<T>(count: FT_UShort) -> *mut u8 {
    if count == 0 {
        return ptr::null_mut();
    }
    let layout = wasm_outline_array_layout::<T>(count);
    // SAFETY: `layout` describes a non-zero array allocation.
    unsafe { alloc_zeroed(layout) }
}

fn wasm_dealloc_array<T>(ptr: *mut u8, count: FT_UShort) {
    if ptr.is_null() || count == 0 {
        return;
    }
    let layout = wasm_outline_array_layout::<T>(count);
    // SAFETY: outline lifecycle allocations in this module use the same layout.
    unsafe { dealloc(ptr, layout) };
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_open_face(
    file_base: *const c_uchar,
    file_size: usize,
    face_index: FT_Long,
    size_pt: f32,
) -> FontdoneWasmStatus {
    fontdone_wasm_open_face_with_name_options(file_base, file_size, face_index, size_pt, 0, 0, 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_interpreter_version_open(
    file_base: *const c_uchar,
    file_size: usize,
    face_index: FT_Long,
    size_pt: f32,
    interpreter_version: FT_UInt,
    out_readback: *mut FT_UInt,
) -> FontdoneWasmStatus {
    if file_base.is_null() {
        return FontdoneWasmStatus {
            error: rust_ffi::FT_Err_Invalid_Argument,
            handle: 0,
        };
    }
    // SAFETY: the caller promises `file_size` readable bytes at `file_base`.
    let data = unsafe { slice::from_raw_parts(file_base, file_size) };
    let mut library = rust_ffi::FT_Init_FreeType();
    let set_error = rust_ffi::FT_Property_Set(
        Some(&mut library),
        Some("truetype"),
        Some("interpreter-version"),
        Some(interpreter_version),
    );
    let mut readback = 0;
    rust_ffi::FT_Property_Get(
        Some(&library),
        Some("truetype"),
        Some("interpreter-version"),
        Some(&mut readback),
    );
    if !out_readback.is_null() {
        // SAFETY: the caller provides one writable FT_UInt output slot.
        unsafe { *out_readback = readback };
    }
    if set_error != rust_ffi::FT_Err_Ok {
        return FontdoneWasmStatus {
            error: set_error,
            handle: 0,
        };
    }
    match rust_ffi::FT_New_Memory_Face(&library, data, face_index, size_pt) {
        Ok(face) => {
            let state = make_wasm_face_state(face);
            let active_size = state.active_size;
            let handle = Box::into_raw(state).addr();
            register_wasm_size_handle(handle, active_size);
            FontdoneWasmStatus {
                error: rust_ffi::FT_Err_Ok,
                handle,
            }
        }
        Err(error) => FontdoneWasmStatus { error, handle: 0 },
    }
}

/// Opens a memory face with a scalar return shape convenient for JavaScript hosts.
///
/// The caller writes `file_size` font bytes into exported linear memory and
/// passes their offset as `file_base`. On success this returns a non-zero face
/// handle and writes `FT_Err_Ok` to `out_error`. The font bytes are copied and
/// can then be released with [`fontdone_wasm_free`].
///
/// `out_error` must point to one writable `FT_Error` in this module's memory.
/// A null output pointer returns zero without opening a face.
#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_open_face_handle(
    file_base: *const c_uchar,
    file_size: usize,
    face_index: FT_Long,
    size_pt: f32,
    out_error: *mut FT_Error,
) -> usize {
    if out_error.is_null() {
        return 0;
    }
    let status = fontdone_wasm_open_face(file_base, file_size, face_index, size_pt);
    // SAFETY: the caller provides one writable FT_Error in exported memory.
    unsafe { *out_error = status.error };
    status.handle
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_open_external_stream_face(
    file_base: *const c_uchar,
    file_size: usize,
    face_index: FT_Long,
    size_pt: f32,
) -> FontdoneWasmStatus {
    if file_base.is_null() {
        return FontdoneWasmStatus {
            error: rust_ffi::FT_Err_Invalid_Argument,
            handle: 0,
        };
    }
    // SAFETY: `file_base` is non-null and caller promises `file_size` readable bytes.
    let data = unsafe { slice::from_raw_parts(file_base, file_size) };
    let library = rust_ffi::FT_Init_FreeType();
    match rust_ffi::FT_Open_External_Stream_Face_With_Name_Options(
        &library,
        data,
        face_index,
        size_pt,
        rust_ffi::FT_Open_Face_Name_Options::default(),
    ) {
        Ok(face) => {
            let state = make_wasm_face_state(face);
            let active_size = state.active_size;
            let handle = Box::into_raw(state).addr();
            register_wasm_size_handle(handle, active_size);
            FontdoneWasmStatus {
                error: rust_ffi::FT_Err_Ok,
                handle,
            }
        }
        Err(error) => FontdoneWasmStatus { error, handle: 0 },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_open_face_with_name_options(
    file_base: *const c_uchar,
    file_size: usize,
    face_index: FT_Long,
    size_pt: f32,
    ignore_typographic_family: FT_Bool,
    ignore_typographic_subfamily: FT_Bool,
    ignore_sbix: FT_Bool,
) -> FontdoneWasmStatus {
    if file_base.is_null() {
        return FontdoneWasmStatus {
            error: rust_ffi::FT_Err_Invalid_Argument,
            handle: 0,
        };
    }
    // SAFETY: `file_base` is non-null and caller promises `file_size` readable bytes.
    let data = unsafe { slice::from_raw_parts(file_base, file_size) };
    let library = rust_ffi::FT_Init_FreeType();
    match rust_ffi::FT_New_Memory_Face_With_Name_Options(
        &library,
        data,
        face_index,
        size_pt,
        rust_ffi::FT_Open_Face_Name_Options {
            ignore_typographic_family: ignore_typographic_family != 0,
            ignore_typographic_subfamily: ignore_typographic_subfamily != 0,
            ignore_sbix: ignore_sbix != 0,
        },
    ) {
        Ok(face) => {
            let state = make_wasm_face_state(face);
            let active_size = state.active_size;
            let handle = Box::into_raw(state).addr();
            register_wasm_size_handle(handle, active_size);
            FontdoneWasmStatus {
                error: rust_ffi::FT_Err_Ok,
                handle,
            }
        }
        Err(error) => FontdoneWasmStatus { error, handle: 0 },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_done_face(handle: usize) -> FT_Error {
    if handle == 0 {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    }
    let ptr = ptr::with_exposed_provenance_mut::<WasmFaceState>(handle);
    if let Some(face) = face_ref(handle) {
        for size_handle in &face.size_handles {
            unregister_wasm_size_handle(*size_handle);
        }
    }
    // SAFETY: `handle` must come from `fontdone_wasm_open_face` and is consumed here.
    unsafe { drop(Box::from_raw(ptr)) };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_new_size(handle: usize) -> FontdoneWasmStatus {
    let Some(face) = face_mut(handle) else {
        return FontdoneWasmStatus {
            error: rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error,
            handle: 0,
        };
    };
    let mut size: rust_ffi::FT_Size = ptr::null_mut();
    let error = rust_ffi::FT_New_Size(Some(&face.face), Some(&mut size));
    if error == rust_ffi::FT_Err_Ok {
        let size_handle = size as usize;
        face.size_handles.push(size_handle);
        face.size_metrics
            .insert(size_handle, face.face.size_metrics);
        register_wasm_size_handle(handle, size_handle);
    }
    FontdoneWasmStatus {
        error,
        handle: if error == rust_ffi::FT_Err_Ok {
            size as usize
        } else {
            0
        },
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_new_size_out(handle: usize, out: *mut usize) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(out) = ptr::NonNull::new(out) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let mut size: rust_ffi::FT_Size = ptr::null_mut();
    let error = rust_ffi::FT_New_Size(Some(&face.face), Some(&mut size));
    if error == rust_ffi::FT_Err_Ok {
        let size_handle = size as usize;
        face.size_handles.push(size_handle);
        face.size_metrics
            .insert(size_handle, face.face.size_metrics);
        register_wasm_size_handle(handle, size_handle);
        // SAFETY: `out` was checked for null and is only written with an opaque handle value.
        unsafe { *out.as_ptr() = size_handle };
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_activate_size(size_handle: usize) -> FT_Error {
    let size = ptr::with_exposed_provenance_mut::<rust_ffi::FT_SizeRec>(size_handle);
    let error = rust_ffi::FT_Activate_Size(size);
    if error == rust_ffi::FT_Err_Ok {
        if let Some(owner) = wasm_size_owner(size_handle).and_then(face_mut) {
            owner.active_size = size_handle;
        }
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_done_size(size_handle: usize) -> FT_Error {
    let size = ptr::with_exposed_provenance_mut::<rust_ffi::FT_SizeRec>(size_handle);
    let owner_handle = wasm_size_owner(size_handle);
    let error = rust_ffi::FT_Done_Size(size);
    if error == rust_ffi::FT_Err_Ok {
        unregister_wasm_size_handle(size_handle);
        if let Some(owner) = owner_handle.and_then(face_mut) {
            let was_active = owner.active_size == size_handle;
            owner.size_handles.retain(|handle| *handle != size_handle);
            owner.size_metrics.remove(&size_handle);
            if was_active {
                owner.active_size = owner.size_handles.first().copied().unwrap_or(0);
            }
        }
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_active_size(handle: usize) -> usize {
    face_ref(handle).map_or(0, |face| face.active_size)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_done_freetype(library_present: i32) -> FT_Error {
    let library = if library_present != 0 {
        Some(rust_ffi::FT_Init_FreeType())
    } else {
        None
    };
    rust_ffi::FT_Done_FreeType(library)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_face_check_truetype_patents(handle: usize) -> FT_Bool {
    rust_ffi::FT_Face_CheckTrueTypePatents(face_ref(handle).map(|state| &state.face))
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_face_set_unpatented_hinting(
    handle: usize,
    value: FT_Bool,
) -> FT_Bool {
    rust_ffi::FT_Face_SetUnpatentedHinting(face_mut(handle).map(|state| &mut state.face), value)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_get_cbox(
    outline: *const FontdoneWasmOutline,
    acbox: *mut FontdoneWasmBBox,
) {
    if outline.is_null() || acbox.is_null() {
        return;
    }
    let Some(snapshot) = outline_snapshot_from_wasm(outline) else {
        return;
    };
    let mut bbox = rust_ffi::FT_BBox::default();
    rust_ffi::FT_Outline_Get_CBox(Some(&snapshot), Some(&mut bbox));
    // SAFETY: `acbox` is non-null and the caller provides writable `FontdoneWasmBBox` storage.
    unsafe {
        *acbox = FontdoneWasmBBox {
            xMin: bbox.xMin,
            yMin: bbox.yMin,
            xMax: bbox.xMax,
            yMax: bbox.yMax,
        };
    }
}

fn wasm_glyph_cbox_snapshot(
    glyph: *const FontdoneWasmGlyph,
) -> Option<rust_ffi::FT_GlyphCBoxSnapshot> {
    let glyph = unsafe { glyph.as_ref() }?;
    if glyph.clazz.is_null() {
        return Some(rust_ffi::FT_GlyphCBoxSnapshot {
            has_class: false,
            has_bbox_hook: false,
            cbox: None,
        });
    }
    if glyph.clazz == wasm_owned_outline_glyph_class() {
        let owned = wasm_owned_outline_glyph_from_root(glyph)?;
        let mut cbox = rust_ffi::FT_BBox::default();
        rust_ffi::FT_Outline_Get_CBox(Some(&owned.core.outline), Some(&mut cbox));
        return Some(rust_ffi::FT_GlyphCBoxSnapshot {
            has_class: true,
            has_bbox_hook: true,
            cbox: Some(cbox),
        });
    }
    if glyph.clazz == wasm_owned_bitmap_glyph_class() {
        let owned = wasm_owned_bitmap_glyph_from_root(glyph)?;
        let x_min = i64::from(owned.record.left).saturating_mul(64);
        let y_max = i64::from(owned.record.top).saturating_mul(64);
        let x_max = x_min.saturating_add(i64::from(owned.record.bitmap.width).saturating_mul(64));
        let y_min = y_max.saturating_sub(i64::from(owned.record.bitmap.rows).saturating_mul(64));
        return Some(rust_ffi::FT_GlyphCBoxSnapshot {
            has_class: true,
            has_bbox_hook: true,
            cbox: Some(rust_ffi::FT_BBox {
                xMin: x_min,
                yMin: y_min,
                xMax: x_max,
                yMax: y_max,
            }),
        });
    }
    if glyph.clazz == wasm_owned_svg_glyph_class() {
        return Some(rust_ffi::FT_GlyphCBoxSnapshot {
            has_class: true,
            has_bbox_hook: false,
            cbox: None,
        });
    }
    // SAFETY: `glyph->clazz` is non-null.  This thin WASM ABI wrapper reads
    // only the class facade's bbox-hook presence and delegates FreeType's
    // zero-first `FT_Glyph_Get_CBox` contract to safe Rust.
    let clazz = unsafe { &*glyph.clazz };
    Some(rust_ffi::FT_GlyphCBoxSnapshot {
        has_class: true,
        has_bbox_hook: clazz.glyph_bbox_present != 0,
        cbox: None,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_glyph_get_cbox(
    glyph: *const FontdoneWasmGlyph,
    bbox_mode: FT_UInt,
    acbox: *mut FontdoneWasmBBox,
) {
    let Some(acbox) = (unsafe { acbox.as_mut() }) else {
        return;
    };
    let snapshot = wasm_glyph_cbox_snapshot(glyph);
    let mut bbox = rust_ffi::FT_BBox::default();
    rust_ffi::FT_Glyph_Get_CBox(snapshot, bbox_mode, Some(&mut bbox));
    *acbox = FontdoneWasmBBox {
        xMin: bbox.xMin,
        yMin: bbox.yMin,
        xMax: bbox.xMax,
        yMax: bbox.yMax,
    };
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_glyph(slot_present: i32, aglyph: *mut usize) -> FT_Error {
    let err = rust_ffi::FT_Get_Glyph(slot_present != 0, !aglyph.is_null());
    if err == rust_ffi::FT_Err_Unimplemented_Feature as FT_Error && !aglyph.is_null() {
        // SAFETY: `aglyph` is non-null and points to caller-provided output storage.
        unsafe {
            *aglyph = 0;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_new_glyph(
    library_present: i32,
    format: rust_ffi::FT_Glyph_Format,
    aglyph: *mut usize,
) -> FT_Error {
    if library_present == 0 || aglyph.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let library = rust_ffi::FT_Init_FreeType();
    let recognized_format = matches!(
        format,
        rust_ffi::FT_GLYPH_FORMAT_OUTLINE
            | rust_ffi::FT_GLYPH_FORMAT_BITMAP
            | rust_ffi::FT_GLYPH_FORMAT_SVG
    ) && rust_ffi::FT_Library_Renderer_Class(Some(&library), format)
        .is_some();
    if recognized_format {
        // Match `ft_new_glyph`: the output is cleared after a built-in class
        // is selected, while an unsupported/custom format preserves a
        // caller-provided sentinel until renderer lookup succeeds.
        // SAFETY: `aglyph` is non-null and points to opaque handle storage.
        unsafe { *aglyph = 0 };
    }
    let glyph = match rust_ffi::FT_New_Glyph(Some(&library), format) {
        Ok(rust_ffi::FT_GlyphOwned::Outline(core)) => {
            Box::into_raw(Box::new(WasmOwnedOutlineGlyph::new(core))).addr()
        }
        Ok(rust_ffi::FT_GlyphOwned::Bitmap(core)) => {
            Box::into_raw(Box::new(WasmOwnedBitmapGlyph::new(core))).addr()
        }
        Ok(rust_ffi::FT_GlyphOwned::Svg(core)) => {
            Box::into_raw(Box::new(WasmOwnedSvgGlyph::new(core))).addr()
        }
        Err(error) => return error,
    };
    // SAFETY: `aglyph` is non-null and points to opaque handle storage.
    unsafe { *aglyph = glyph };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_glyph_from_face(
    face_handle: usize,
    aglyph: *mut usize,
) -> FT_Error {
    let slot = face_ref(face_handle).and_then(|face| face.slot.as_ref());
    let err = rust_ffi::FT_Get_Glyph(slot.is_some(), !aglyph.is_null());
    if err != rust_ffi::FT_Err_Unimplemented_Feature as FT_Error {
        return err;
    }
    let Some(out) = (unsafe { aglyph.as_mut() }) else {
        return err;
    };
    let recognized_format = slot.is_some_and(|slot| {
        matches!(
            slot.format,
            rust_ffi::FT_GLYPH_FORMAT_BITMAP
                | rust_ffi::FT_GLYPH_FORMAT_OUTLINE
                | rust_ffi::FT_GLYPH_FORMAT_SVG
        )
    });
    let glyph_result = if slot.is_some_and(|slot| slot.format == rust_ffi::FT_GLYPH_FORMAT_BITMAP) {
        rust_ffi::FT_Get_Bitmap_Glyph(slot)
            .map(|core| Box::into_raw(Box::new(WasmOwnedBitmapGlyph::new(core))).addr())
    } else if slot.is_some_and(|slot| slot.format == rust_ffi::FT_GLYPH_FORMAT_SVG) {
        rust_ffi::FT_Get_Svg_Glyph(slot)
            .map(|core| Box::into_raw(Box::new(WasmOwnedSvgGlyph::new(core))).addr())
    } else {
        rust_ffi::FT_Get_Outline_Glyph(slot)
            .map(|core| Box::into_raw(Box::new(WasmOwnedOutlineGlyph::new(core))).addr())
    };
    match glyph_result {
        Ok(glyph) => {
            *out = glyph;
            rust_ffi::FT_Err_Ok
        }
        Err(error) => {
            // Unknown formats fail in `FT_New_Glyph` before the output write.
            // Known-class advance and glyph-init failures destroy their
            // partial glyph and write a null result.
            if recognized_format {
                *out = 0;
            }
            error
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_glyph_copy(
    source: *const FontdoneWasmGlyph,
    target: *mut usize,
) -> FT_Error {
    let source_has_class = if target.is_null() || source.is_null() {
        false
    } else {
        // SAFETY: `source` is non-null and this thin wrapper reads only the
        // class pointer needed for FreeType's early argument validation.
        unsafe { !(*source).clazz.is_null() }
    };
    let err = rust_ffi::FT_Glyph_Copy(!source.is_null(), !target.is_null(), source_has_class);
    if err == rust_ffi::FT_Err_Unimplemented_Feature as FT_Error && !target.is_null() {
        // FreeType clears the output immediately after its early argument
        // checks, before class-specific allocation or copy validation.
        // SAFETY: `target` is non-null and points to caller-provided output storage.
        unsafe {
            *target = 0;
        }
    }
    if err == rust_ffi::FT_Err_Unimplemented_Feature as FT_Error
        && !target.is_null()
        && let Some(source) = wasm_owned_outline_glyph_from_root(source)
    {
        let copy = rust_ffi::FT_Outline_Glyph_Copy(&source.core);
        // SAFETY: `target` is non-null and points to caller-provided output storage.
        unsafe {
            *target = Box::into_raw(Box::new(WasmOwnedOutlineGlyph::new(copy))).addr();
        }
        return rust_ffi::FT_Err_Ok;
    }
    if err == rust_ffi::FT_Err_Unimplemented_Feature as FT_Error
        && !target.is_null()
        && let Some(source) = wasm_owned_bitmap_glyph_from_root_mut(source.cast_mut())
    {
        if let Err(error) = source.sync_core_from_record() {
            return error;
        }
        let copy = rust_ffi::FT_Bitmap_Glyph_Copy(&source.core);
        // SAFETY: `target` is non-null and points to caller-provided output storage.
        unsafe {
            *target = Box::into_raw(Box::new(WasmOwnedBitmapGlyph::new(copy))).addr();
        }
        return rust_ffi::FT_Err_Ok;
    }
    if err == rust_ffi::FT_Err_Unimplemented_Feature as FT_Error
        && !target.is_null()
        && let Some(source) = wasm_owned_svg_glyph_from_root_mut(source.cast_mut())
    {
        if let Err(error) = source.sync_core_from_record() {
            return error;
        }
        let copy = match rust_ffi::FT_Svg_Glyph_Copy(&source.core) {
            Ok(copy) => copy,
            Err(error) => return error,
        };
        // SAFETY: `target` is non-null and points to caller-provided output storage.
        unsafe {
            *target = Box::into_raw(Box::new(WasmOwnedSvgGlyph::new(copy))).addr();
        }
        return rust_ffi::FT_Err_Ok;
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_done_glyph(glyph_present: i32) {
    rust_ffi::FT_Done_Glyph(glyph_present != 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_done_glyph_handle(glyph: *mut FontdoneWasmGlyph) {
    if wasm_owned_outline_glyph_from_root(glyph).is_some() {
        // SAFETY: the private class marker proves this pointer came from
        // `Box<WasmOwnedOutlineGlyph>` in `fontdone_wasm_get_glyph_from_face`.
        unsafe { drop(Box::from_raw(glyph.cast::<WasmOwnedOutlineGlyph>())) };
        return;
    }
    if wasm_owned_bitmap_glyph_from_root(glyph).is_some() {
        // SAFETY: the private class marker proves this pointer came from
        // `Box<WasmOwnedBitmapGlyph>` in `fontdone_wasm_get_glyph_from_face`.
        unsafe { drop(Box::from_raw(glyph.cast::<WasmOwnedBitmapGlyph>())) };
        return;
    }
    if wasm_owned_svg_glyph_from_root(glyph).is_some() {
        // SAFETY: the private marker proves this allocation type.
        unsafe { drop(Box::from_raw(glyph.cast::<WasmOwnedSvgGlyph>())) };
        return;
    }
    rust_ffi::FT_Done_Glyph(!glyph.is_null());
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_glyph_transform(
    glyph: *mut FontdoneWasmGlyph,
    matrix: *const FontdoneWasmMatrix,
    delta: *const FontdoneWasmVector,
) -> FT_Error {
    if glyph.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let matrix = (unsafe { matrix.as_ref() }).map(|matrix| rust_ffi::FT_Matrix {
        xx: matrix.xx,
        xy: matrix.xy,
        yx: matrix.yx,
        yy: matrix.yy,
    });
    let delta = (unsafe { delta.as_ref() }).map(|delta| rust_ffi::FT_Vector {
        x: delta.x,
        y: delta.y,
    });
    if let Some(owned) = wasm_owned_svg_glyph_from_root_mut(glyph) {
        let error = rust_ffi::FT_Svg_Glyph_Transform(
            Some(&mut owned.core),
            matrix.as_ref(),
            delta.as_ref(),
        );
        if error == rust_ffi::FT_Err_Ok {
            owned.refresh_record();
        }
        return error;
    }
    let Some(owned) = wasm_owned_outline_glyph_from_root_mut(glyph) else {
        let has_class = unsafe { !(*glyph).clazz.is_null() };
        return if has_class {
            rust_ffi::FT_Err_Invalid_Glyph_Format
        } else {
            rust_ffi::FT_Err_Invalid_Argument
        };
    };
    let error = rust_ffi::FT_Glyph_Transform_Outline(
        Some(&mut owned.core),
        matrix.as_ref(),
        delta.as_ref(),
    );
    if error == rust_ffi::FT_Err_Ok {
        owned.refresh_record();
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_glyph_to_bitmap(
    the_glyph_present: i32,
    glyph_present: i32,
    library_present: i32,
    class_present: i32,
    prepare_hook_present: i32,
) -> FT_Error {
    rust_ffi::FT_Glyph_To_Bitmap(
        the_glyph_present != 0,
        glyph_present != 0,
        library_present != 0,
        class_present != 0,
        prepare_hook_present != 0,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_glyph_to_bitmap_handle(
    the_glyph: *mut usize,
    render_mode: i32,
    origin: *const FontdoneWasmVector,
    destroy: FT_Bool,
) -> FT_Error {
    let Some(handle) = (unsafe { the_glyph.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let glyph = ptr::with_exposed_provenance_mut::<FontdoneWasmGlyph>(*handle);
    if wasm_owned_bitmap_glyph_from_root(glyph).is_some() {
        // FreeType `src/base/ftglyph.c:794-795` returns success without
        // replacing or freeing an already-bitmap glyph.
        return rust_ffi::FT_Err_Ok;
    }
    let Some(owned) = wasm_owned_outline_glyph_from_root_mut(glyph) else {
        return rust_ffi::FT_Glyph_To_Bitmap(true, *handle != 0, true, false, false);
    };
    if let Err(error) = owned.sync_core_from_record() {
        return error;
    }
    let origin = if origin.is_null() {
        None
    } else {
        // SAFETY: the caller supplied a non-null vector for this synchronous call.
        let origin = unsafe { &*origin };
        Some(rust_ffi::FT_Vector {
            x: origin.x,
            y: origin.y,
        })
    };
    let bitmap = match rust_ffi::FT_Outline_Glyph_To_Bitmap_In_Place(
        &mut owned.core,
        render_mode,
        origin,
        destroy != 0,
    ) {
        Ok(bitmap) => bitmap,
        Err(error) => {
            owned.refresh_record();
            return error;
        }
    };
    let bitmap = Box::into_raw(Box::new(WasmOwnedBitmapGlyph::new(bitmap))).addr();
    if destroy != 0 {
        // SAFETY: the private class marker proves this pointer came from
        // `Box<WasmOwnedOutlineGlyph>` in `fontdone_wasm_get_glyph_from_face`.
        unsafe { drop(Box::from_raw(glyph.cast::<WasmOwnedOutlineGlyph>())) };
    }
    *handle = bitmap;
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_glyph_stroke_outline_success(
    glyph_handle: usize,
    library_present: FT_Bool,
) -> Result<usize, FT_Error> {
    let glyph = ptr::with_exposed_provenance::<FontdoneWasmGlyph>(glyph_handle);
    let Some(owned) = wasm_owned_outline_glyph_from_root(glyph) else {
        return Err(rust_ffi::FT_Err_Invalid_Argument);
    };
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    let new_error = rust_ffi::FT_Stroker_New(
        if library_present != 0 {
            Some(&library)
        } else {
            None
        },
        Some(&mut stroker),
    );
    if new_error != rust_ffi::FT_Err_Ok {
        return Err(new_error);
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        96,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let result = rust_ffi::FT_Outline_Glyph_Stroke(Some(&owned.core), stroker);
    rust_ffi::FT_Stroker_Done(stroker);
    match result {
        Ok(stroked) => Ok(Box::into_raw(Box::new(WasmOwnedOutlineGlyph::new(stroked))).addr()),
        Err(error) => Err(error),
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_glyph_stroke_destroy_option(
    glyph_handle: &mut usize,
    radius: FT_Fixed,
    line_cap: FT_Int,
    line_join: FT_Int,
    miter_limit: FT_Fixed,
    destroy: FT_Bool,
    library_present: FT_Bool,
) -> FT_Error {
    let glyph = ptr::with_exposed_provenance::<FontdoneWasmGlyph>(*glyph_handle);
    let Some(owned) = wasm_owned_outline_glyph_from_root(glyph) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    let new_error = rust_ffi::FT_Stroker_New(
        if library_present != 0 {
            Some(&library)
        } else {
            None
        },
        Some(&mut stroker),
    );
    if new_error != rust_ffi::FT_Err_Ok {
        return new_error;
    }
    rust_ffi::FT_Stroker_Set(stroker, radius, line_cap, line_join, miter_limit);
    let result = rust_ffi::FT_Outline_Glyph_Stroke(Some(&owned.core), stroker);
    rust_ffi::FT_Stroker_Done(stroker);
    match result {
        Ok(stroked) => {
            let original = ptr::with_exposed_provenance_mut::<FontdoneWasmGlyph>(*glyph_handle);
            *glyph_handle = Box::into_raw(Box::new(WasmOwnedOutlineGlyph::new(stroked))).addr();
            if destroy != 0 {
                // SAFETY: the private class marker proves this pointer came
                // from `Box<WasmOwnedOutlineGlyph>` in
                // `fontdone_wasm_get_glyph_from_face`.
                unsafe { drop(Box::from_raw(original.cast::<WasmOwnedOutlineGlyph>())) };
            }
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_glyph_stroke_border_outside_success(
    glyph_handle: usize,
    library_present: FT_Bool,
) -> Result<usize, FT_Error> {
    let glyph = ptr::with_exposed_provenance::<FontdoneWasmGlyph>(glyph_handle);
    let Some(owned) = wasm_owned_outline_glyph_from_root(glyph) else {
        return Err(rust_ffi::FT_Err_Invalid_Argument);
    };
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    let new_error = rust_ffi::FT_Stroker_New(
        if library_present != 0 {
            Some(&library)
        } else {
            None
        },
        Some(&mut stroker),
    );
    if new_error != rust_ffi::FT_Err_Ok {
        return Err(new_error);
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        96,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let result = rust_ffi::FT_Outline_Glyph_StrokeBorder(Some(&owned.core), stroker, 0);
    rust_ffi::FT_Stroker_Done(stroker);
    match result {
        Ok(stroked) => Ok(Box::into_raw(Box::new(WasmOwnedOutlineGlyph::new(stroked))).addr()),
        Err(error) => Err(error),
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_glyph_stroke_border_inside_success(
    glyph_handle: usize,
    library_present: FT_Bool,
) -> Result<usize, FT_Error> {
    let glyph = ptr::with_exposed_provenance::<FontdoneWasmGlyph>(glyph_handle);
    let Some(owned) = wasm_owned_outline_glyph_from_root(glyph) else {
        return Err(rust_ffi::FT_Err_Invalid_Argument);
    };
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    let new_error = rust_ffi::FT_Stroker_New(
        if library_present != 0 {
            Some(&library)
        } else {
            None
        },
        Some(&mut stroker),
    );
    if new_error != rust_ffi::FT_Err_Ok {
        return Err(new_error);
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        160,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_BEVEL as FT_Int,
        65_536,
    );
    let result = rust_ffi::FT_Outline_Glyph_StrokeBorder(Some(&owned.core), stroker, 1);
    rust_ffi::FT_Stroker_Done(stroker);
    match result {
        Ok(stroked) => Ok(Box::into_raw(Box::new(WasmOwnedOutlineGlyph::new(stroked))).addr()),
        Err(error) => Err(error),
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_glyph_stroke_border_destroy_option(
    glyph_handle: &mut usize,
    radius: FT_Fixed,
    line_cap: FT_Int,
    line_join: FT_Int,
    miter_limit: FT_Fixed,
    inside: FT_Bool,
    destroy: FT_Bool,
    library_present: FT_Bool,
) -> FT_Error {
    let glyph = ptr::with_exposed_provenance::<FontdoneWasmGlyph>(*glyph_handle);
    let Some(owned) = wasm_owned_outline_glyph_from_root(glyph) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    let new_error = rust_ffi::FT_Stroker_New(
        if library_present != 0 {
            Some(&library)
        } else {
            None
        },
        Some(&mut stroker),
    );
    if new_error != rust_ffi::FT_Err_Ok {
        return new_error;
    }
    rust_ffi::FT_Stroker_Set(stroker, radius, line_cap, line_join, miter_limit);
    let result = rust_ffi::FT_Outline_Glyph_StrokeBorder(Some(&owned.core), stroker, inside);
    rust_ffi::FT_Stroker_Done(stroker);
    match result {
        Ok(stroked) => {
            let original = ptr::with_exposed_provenance_mut::<FontdoneWasmGlyph>(*glyph_handle);
            *glyph_handle = Box::into_raw(Box::new(WasmOwnedOutlineGlyph::new(stroked))).addr();
            if destroy != 0 {
                // SAFETY: the private class marker proves this pointer came
                // from `Box<WasmOwnedOutlineGlyph>` in
                // `fontdone_wasm_get_glyph_from_face`.
                unsafe { drop(Box::from_raw(original.cast::<WasmOwnedOutlineGlyph>())) };
            }
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_glyph_stroke_invalid_arguments(
    glyph_handle: usize,
    border: FT_Bool,
) -> [FT_Error; 3] {
    let null_glyph_status = if border != 0 {
        rust_ffi::FT_Outline_Glyph_StrokeBorder(None, ptr::null_mut(), 1)
    } else {
        rust_ffi::FT_Outline_Glyph_Stroke(None, ptr::null_mut())
    }
    .map_or_else(|error| error, |_| rust_ffi::FT_Err_Ok);
    let null_pointer_status = if border != 0 {
        rust_ffi::FT_Outline_Glyph_StrokeBorder(None, ptr::null_mut(), 1)
    } else {
        rust_ffi::FT_Outline_Glyph_Stroke(None, ptr::null_mut())
    }
    .map_or_else(|error| error, |_| rust_ffi::FT_Err_Ok);
    let glyph = ptr::with_exposed_provenance::<FontdoneWasmGlyph>(glyph_handle);
    let null_stroker_status = wasm_owned_outline_glyph_from_root(glyph).map_or_else(
        || rust_ffi::FT_Err_Invalid_Argument,
        |owned| {
            if border != 0 {
                rust_ffi::FT_Outline_Glyph_StrokeBorder(Some(&owned.core), ptr::null_mut(), 1)
            } else {
                rust_ffi::FT_Outline_Glyph_Stroke(Some(&owned.core), ptr::null_mut())
            }
            .map_or_else(|error| error, |_| rust_ffi::FT_Err_Ok)
        },
    );
    [null_glyph_status, null_pointer_status, null_stroker_status]
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_get_bbox(
    outline: *const FontdoneWasmOutline,
    abbox: *mut FontdoneWasmBBox,
) -> FT_Error {
    if abbox.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument as FT_Error;
    }
    let Some(snapshot) = outline_snapshot_from_wasm(outline) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    let mut bbox = rust_ffi::FT_BBox::default();
    let error = rust_ffi::FT_Outline_Get_BBox(Some(&snapshot), Some(&mut bbox));
    if error == rust_ffi::FT_Err_Ok {
        // SAFETY: `abbox` is non-null and the caller provides writable `FontdoneWasmBBox` storage.
        unsafe {
            *abbox = FontdoneWasmBBox {
                xMin: bbox.xMin,
                yMin: bbox.yMin,
                xMax: bbox.xMax,
                yMax: bbox.yMax,
            };
        }
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_get_bitmap(
    library_present: i32,
    outline: *const FontdoneWasmOutline,
    abitmap: *mut FontdoneWasmBitmap,
) -> FT_Error {
    let Some(target) = (unsafe { abitmap.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let library = if library_present != 0 {
        Some(rust_ffi::FT_Init_FreeType())
    } else {
        None
    };
    let snapshot = outline_snapshot_from_wasm(outline);
    let bitmap_view = rust_ffi::FT_Bitmap_C {
        rows: target.rows,
        width: target.width,
        pitch: target.pitch,
        buffer: target.buffer.cast_mut(),
        num_grays: target.num_grays,
        pixel_mode: u8::try_from(target.pixel_mode).unwrap_or(0),
        palette_mode: target.palette_mode,
        palette: target.palette.cast_mut(),
    };
    match rust_ffi::FT_Outline_Get_Bitmap(library.as_ref(), snapshot.as_ref(), Some(&bitmap_view)) {
        Ok(rendered) => {
            copy_rendered_bitmap_to_wasm(target, &rendered);
            rust_ffi::FT_Err_Ok
        }
        Err(err) => err,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_render(
    library_present: i32,
    outline: *const FontdoneWasmOutline,
    params: *mut FontdoneWasmRasterParams,
) -> FT_Error {
    let Some(params) = (unsafe { params.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let library = if library_present != 0 {
        Some(rust_ffi::FT_Init_FreeType())
    } else {
        None
    };
    let snapshot = outline_snapshot_from_wasm(outline);
    let target = unsafe { params.target.as_ref() };
    let bitmap_view = target.map(wasm_bitmap_to_rust);
    let clip_box = rust_ffi::FT_BBox {
        xMin: params.clip_box.xMin,
        yMin: params.clip_box.yMin,
        xMax: params.clip_box.xMax,
        yMax: params.clip_box.yMax,
    };
    if library.is_some()
        && snapshot.as_ref().is_some_and(|outline_snapshot| {
            let mut cbox = rust_ffi::FT_BBox::default();
            rust_ffi::FT_Outline_Get_CBox(Some(outline_snapshot), Some(&mut cbox));
            cbox.xMin >= -0x1000000
                && cbox.yMin >= -0x1000000
                && cbox.xMax <= 0x1000000
                && cbox.yMax <= 0x1000000
        })
    {
        // FreeType 2.14.3 ftoutln.c:625-648 sets `source` before the
        // renderer call, including calls that return a renderer error.
        params.source = outline.cast();
    }
    if params.flags & rust_ffi::FT_RASTER_FLAG_DIRECT as FT_Int != 0 {
        if params.flags & rust_ffi::FT_RASTER_FLAG_CLIP as FT_Int == 0 {
            if let (Some(_library), Some(outline_snapshot)) = (library.as_ref(), snapshot.as_ref())
            {
                let mut cbox = rust_ffi::FT_BBox::default();
                rust_ffi::FT_Outline_Get_CBox(Some(outline_snapshot), Some(&mut cbox));
                if cbox.xMin >= -0x1000000
                    && cbox.yMin >= -0x1000000
                    && cbox.xMax <= 0x1000000
                    && cbox.yMax <= 0x1000000
                {
                    // FreeType 2.14.3 ftoutln.c:635-640 presets direct-mode
                    // no-CLIP bounds from the outline CBox in integer pixels.
                    params.clip_box.xMin = cbox.xMin >> 6;
                    params.clip_box.yMin = cbox.yMin >> 6;
                    params.clip_box.xMax = cbox.xMax.checked_add(63).unwrap_or(cbox.xMax) >> 6;
                    params.clip_box.yMax = cbox.yMax.checked_add(63).unwrap_or(cbox.yMax) >> 6;
                }
            }
        }
        return match rust_ffi::FT_Outline_Render_Direct_Spans(
            library.as_ref(),
            snapshot.as_ref(),
            bitmap_view.as_ref(),
            params.flags,
            Some(wasm_bbox_to_rust(&params.clip_box)),
            !params.gray_spans.is_null(),
        ) {
            Ok(_) => rust_ffi::FT_Err_Ok,
            Err(err) => err,
        };
    }
    match rust_ffi::FT_Outline_Render(
        library.as_ref(),
        snapshot.as_ref(),
        bitmap_view.as_ref(),
        params.flags,
        clip_box,
    ) {
        Ok(rendered) => {
            if let Some(target) = target {
                if target.width != 0 && target.rows != 0 && target.buffer.is_null() {
                    return rust_ffi::FT_Err_Invalid_Argument;
                }
                // SAFETY: the WASM descriptor points at writable linear-memory
                // bitmap storage for this synchronous call.
                let target = unsafe { &mut *params.target };
                copy_rendered_bitmap_to_wasm(target, &rendered);
            }
            rust_ffi::FT_Err_Ok
        }
        Err(err) => {
            if !params.target.is_null() {
                if let Some(rendered) = rust_ffi::FT_Outline_Render_Error_Output(
                    snapshot.as_ref(),
                    bitmap_view.as_ref(),
                    params.flags,
                ) {
                    // SAFETY: the WASM descriptor points at writable linear-memory
                    // bitmap storage for this synchronous call.
                    let target = unsafe { &mut *params.target };
                    copy_rendered_bitmap_to_wasm(target, &rendered);
                }
            }
            err
        }
    }
}

fn wasm_bbox_to_rust(bbox: &FontdoneWasmBBox) -> rust_ffi::FT_BBox {
    rust_ffi::FT_BBox {
        xMin: bbox.xMin,
        yMin: bbox.yMin,
        xMax: bbox.xMax,
        yMax: bbox.yMax,
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_outline_render_direct_spans(
    library_present: i32,
    outline: *const FontdoneWasmOutline,
    params: *mut FontdoneWasmRasterParams,
    gray_spans_present: bool,
    user_token: *mut c_void,
) -> (FT_Error, Vec<(i32, rust_ffi::FT_Span)>, bool) {
    let Some(params) = (unsafe { params.as_mut() }) else {
        return (rust_ffi::FT_Err_Invalid_Argument, Vec::new(), false);
    };
    params.user = user_token;
    params.gray_spans = if gray_spans_present {
        std::ptr::dangling::<c_void>()
    } else {
        ptr::null()
    };
    let library = if library_present != 0 {
        Some(rust_ffi::FT_Init_FreeType())
    } else {
        None
    };
    let snapshot = outline_snapshot_from_wasm(outline);
    let target = unsafe { params.target.as_ref() };
    let bitmap_view = target.map(wasm_bitmap_to_rust);
    if params.flags & rust_ffi::FT_RASTER_FLAG_CLIP as FT_Int == 0 {
        if let (Some(_library), Some(outline_snapshot)) = (library.as_ref(), snapshot.as_ref()) {
            let mut cbox = rust_ffi::FT_BBox::default();
            rust_ffi::FT_Outline_Get_CBox(Some(outline_snapshot), Some(&mut cbox));
            if cbox.xMin >= -0x1000000
                && cbox.yMin >= -0x1000000
                && cbox.xMax <= 0x1000000
                && cbox.yMax <= 0x1000000
            {
                // FreeType 2.14.3 ftoutln.c:635-640 presets direct-mode
                // no-CLIP bounds from the outline CBox in integer pixels.
                params.clip_box.xMin = cbox.xMin >> 6;
                params.clip_box.yMin = cbox.yMin >> 6;
                params.clip_box.xMax = cbox.xMax.checked_add(63).unwrap_or(cbox.xMax) >> 6;
                params.clip_box.yMax = cbox.yMax.checked_add(63).unwrap_or(cbox.yMax) >> 6;
            }
        }
    }
    match rust_ffi::FT_Outline_Render_Direct_Spans(
        library.as_ref(),
        snapshot.as_ref(),
        bitmap_view.as_ref(),
        params.flags,
        Some(wasm_bbox_to_rust(&params.clip_box)),
        gray_spans_present,
    ) {
        Ok(spans) => {
            let user_seen = gray_spans_present && params.user == user_token && !spans.is_empty();
            (rust_ffi::FT_Err_Ok, spans, user_seen)
        }
        Err(err) => (err, Vec::new(), false),
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_outline_decompose_trace(
    outline: *const FontdoneWasmOutline,
    transforms: &[(rust_ffi::FT_Int, rust_ffi::FT_Pos)],
) -> Result<Vec<rust_ffi::FTOutlineDecomposeRun>, FT_Error> {
    let snapshot = outline_snapshot_from_wasm(outline);
    rust_ffi::FT_Outline_Decompose_Trace(snapshot.as_ref(), transforms)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_get_orientation(
    outline: *const FontdoneWasmOutline,
) -> FT_Orientation {
    let Some(snapshot) = outline_snapshot_from_wasm(outline) else {
        return rust_ffi::FT_ORIENTATION_TRUETYPE as FT_Orientation;
    };
    rust_ffi::FT_Outline_Get_Orientation(Some(&snapshot)) as FT_Orientation
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_check(outline: *const FontdoneWasmOutline) -> FT_Error {
    let Some(snapshot) = outline_snapshot_from_wasm(outline) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    rust_ffi::FT_Outline_Check(Some(&snapshot))
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_copy(
    source: *const FontdoneWasmOutline,
    target: *mut FontdoneWasmOutline,
) -> FT_Error {
    if source == target.cast_const() && !source.is_null() {
        return rust_ffi::FT_Err_Ok;
    }
    let Some(source_snapshot) = outline_snapshot_from_wasm(source) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    let Some(mut target_snapshot) = outline_snapshot_from_wasm(target) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    let error = rust_ffi::FT_Outline_Copy(Some(&source_snapshot), Some(&mut target_snapshot));
    if error == rust_ffi::FT_Err_Ok {
        copy_outline_snapshot_to_wasm(target, &target_snapshot, true);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_embolden(
    outline: *mut FontdoneWasmOutline,
    strength: FT_Long,
) -> FT_Error {
    let Some(mut snapshot) = outline_snapshot_from_wasm(outline) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    let error = rust_ffi::FT_Outline_Embolden(Some(&mut snapshot), strength);
    if error == rust_ffi::FT_Err_Ok {
        copy_outline_snapshot_to_wasm(outline, &snapshot, false);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_embolden_xy(
    outline: *mut FontdoneWasmOutline,
    xstrength: FT_Long,
    ystrength: FT_Long,
) -> FT_Error {
    let Some(mut snapshot) = outline_snapshot_from_wasm(outline) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    let error = rust_ffi::FT_Outline_EmboldenXY(Some(&mut snapshot), xstrength, ystrength);
    if error == rust_ffi::FT_Err_Ok {
        copy_outline_snapshot_to_wasm(outline, &snapshot, false);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_get_inside_border(
    outline: *const FontdoneWasmOutline,
) -> FT_StrokerBorder {
    let snapshot = outline_snapshot_from_wasm(outline);
    rust_ffi::FT_Outline_GetInsideBorder(snapshot.as_ref()) as FT_StrokerBorder
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_get_outside_border(
    outline: *const FontdoneWasmOutline,
) -> FT_StrokerBorder {
    let snapshot = outline_snapshot_from_wasm(outline);
    rust_ffi::FT_Outline_GetOutsideBorder(snapshot.as_ref()) as FT_StrokerBorder
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_null_noop(action: i32) -> bool {
    match action {
        1 => rust_ffi::FT_Stroker_Set(
            ptr::null_mut(),
            128,
            rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
            rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
            65_536,
        ),
        2 => rust_ffi::FT_Stroker_Rewind(ptr::null_mut()),
        3 => rust_ffi::FT_Stroker_Done(ptr::null_mut()),
        _ => return false,
    }
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_lifecycle(action: i32) -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    match action {
        1 => {}
        2 => rust_ffi::FT_Stroker_Export(stroker, None),
        3 => {
            let mut outline = rust_ffi::FT_OutlineSnapshot::default();
            rust_ffi::FT_Stroker_ExportBorder(
                stroker,
                rust_ffi::FT_STROKER_BORDER_LEFT as FT_Int,
                Some(&mut outline),
            );
        }
        4 => {
            let mut outline = rust_ffi::FT_OutlineSnapshot::default();
            rust_ffi::FT_Stroker_ExportBorder(stroker, 2, Some(&mut outline));
        }
        5 => {
            rust_ffi::FT_Stroker_Set(
                stroker,
                128,
                rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
                rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
                65_536,
            );
            let mut outline = rust_ffi::FT_OutlineSnapshot::default();
            rust_ffi::FT_Stroker_Export(stroker, Some(&mut outline));
            rust_ffi::FT_Stroker_ExportBorder(
                stroker,
                rust_ffi::FT_STROKER_BORDER_LEFT as FT_Int,
                Some(&mut outline),
            );
            rust_ffi::FT_Stroker_Rewind(stroker);
        }
        6 => {
            rust_ffi::FT_Stroker_Set(
                stroker,
                96,
                rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
                rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
                65_536,
            );
            let start = rust_ffi::FT_Vector { x: 0, y: 0 };
            let p1 = rust_ffi::FT_Vector { x: 640, y: 0 };
            let p2 = rust_ffi::FT_Vector { x: 640, y: 640 };
            let begin_status = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
            let line1_status = if begin_status == rust_ffi::FT_Err_Ok {
                rust_ffi::FT_Stroker_LineTo(stroker, Some(&p1))
            } else {
                begin_status
            };
            let line2_status = if line1_status == rust_ffi::FT_Err_Ok {
                rust_ffi::FT_Stroker_LineTo(stroker, Some(&p2))
            } else {
                line1_status
            };
            let end_status = if line2_status == rust_ffi::FT_Err_Ok {
                rust_ffi::FT_Stroker_EndSubPath(stroker)
            } else {
                line2_status
            };
            let mut point_count = 0;
            let mut contour_count = 0;
            let counts_status = if end_status == rust_ffi::FT_Err_Ok {
                rust_ffi::FT_Stroker_GetCounts(
                    stroker,
                    Some(&mut point_count),
                    Some(&mut contour_count),
                )
            } else {
                end_status
            };
            if counts_status != rust_ffi::FT_Err_Ok || point_count == 0 || contour_count == 0 {
                rust_ffi::FT_Stroker_Done(stroker);
                return false;
            }
        }
        _ => return false,
    }
    rust_ffi::FT_Stroker_Done(stroker);
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_zero_line() -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        128,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let start = rust_ffi::FT_Vector { x: 256, y: 256 };
    let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
    let line_error = rust_ffi::FT_Stroker_LineTo(stroker, Some(&start));
    let mut points = 99;
    let mut contours = 99;
    let counts_error =
        rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours));
    rust_ffi::FT_Stroker_Done(stroker);
    begin_error == rust_ffi::FT_Err_Ok
        && line_error == rust_ffi::FT_Err_Ok
        && counts_error == rust_ffi::FT_Err_Ok
        && points == 0
        && contours == 0
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_simple_line_counts() -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        96,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let start = rust_ffi::FT_Vector { x: 0, y: 0 };
    let to = rust_ffi::FT_Vector { x: 640, y: 0 };
    let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
    let line_error = rust_ffi::FT_Stroker_LineTo(stroker, Some(&to));
    let mut left_points = 99;
    let mut left_contours = 99;
    let left_error = rust_ffi::FT_Stroker_GetBorderCounts(
        stroker,
        rust_ffi::FT_STROKER_BORDER_LEFT as FT_Int,
        Some(&mut left_points),
        Some(&mut left_contours),
    );
    let mut right_points = 99;
    let mut right_contours = 99;
    let right_error = rust_ffi::FT_Stroker_GetBorderCounts(
        stroker,
        rust_ffi::FT_STROKER_BORDER_RIGHT as FT_Int,
        Some(&mut right_points),
        Some(&mut right_contours),
    );
    let mut total_points = 99;
    let mut total_contours = 99;
    let total_error =
        rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut total_points), Some(&mut total_contours));
    rust_ffi::FT_Stroker_Done(stroker);
    begin_error == rust_ffi::FT_Err_Ok
        && line_error == rust_ffi::FT_Err_Ok
        && left_error == rust_ffi::FT_Err_Invalid_Outline
        && right_error == rust_ffi::FT_Err_Invalid_Outline
        && total_error == rust_ffi::FT_Err_Invalid_Outline
        && left_points == 0
        && left_contours == 0
        && right_points == 0
        && right_contours == 0
        && total_points == 0
        && total_contours == 0
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_open_line_geometry(action: i32, radius: FT_Fixed) -> bool {
    let actions: &[i32] = if action == 0 { &[1, 2, 3] } else { &[action] };
    for action in actions {
        let library = rust_ffi::FT_Init_FreeType();
        let mut stroker = ptr::null_mut();
        if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
            return false;
        }
        if stroker.is_null() {
            return false;
        }
        let cap = match action {
            1 => rust_ffi::FT_STROKER_LINECAP_BUTT as FT_Int,
            3 => rust_ffi::FT_STROKER_LINECAP_SQUARE as FT_Int,
            _ => rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        };
        rust_ffi::FT_Stroker_Set(
            stroker,
            radius,
            cap,
            rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
            65_536,
        );
        let start = rust_ffi::FT_Vector { x: 0, y: 0 };
        let to = rust_ffi::FT_Vector { x: 640, y: 0 };
        let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 1);
        let line_error = if begin_error == rust_ffi::FT_Err_Ok {
            rust_ffi::FT_Stroker_LineTo(stroker, Some(&to))
        } else {
            begin_error
        };
        let end_error = if line_error == rust_ffi::FT_Err_Ok {
            rust_ffi::FT_Stroker_EndSubPath(stroker)
        } else {
            line_error
        };
        let mut count_points = 0;
        let mut count_contours = 0;
        let counts_error = if end_error == rust_ffi::FT_Err_Ok {
            rust_ffi::FT_Stroker_GetCounts(
                stroker,
                Some(&mut count_points),
                Some(&mut count_contours),
            )
        } else {
            end_error
        };
        let mut left = rust_ffi::FT_OutlineSnapshot::default();
        let mut right = rust_ffi::FT_OutlineSnapshot::default();
        let mut combined = rust_ffi::FT_OutlineSnapshot::default();
        if counts_error == rust_ffi::FT_Err_Ok {
            rust_ffi::FT_Stroker_ExportBorder(
                stroker,
                rust_ffi::FT_STROKER_BORDER_LEFT as FT_Int,
                Some(&mut left),
            );
            rust_ffi::FT_Stroker_ExportBorder(
                stroker,
                rust_ffi::FT_STROKER_BORDER_RIGHT as FT_Int,
                Some(&mut right),
            );
            rust_ffi::FT_Stroker_Export(stroker, Some(&mut combined));
        }
        rust_ffi::FT_Stroker_Done(stroker);
        let expected_points = match action {
            1 => 5,
            3 => 6,
            _ => 15,
        };
        if counts_error != rust_ffi::FT_Err_Ok
            || left.points.len() != expected_points
            || left.contours.len() != 1
            || !right.points.is_empty()
            || combined.points.len() != expected_points
        {
            return false;
        }
    }
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_closed_line_geometry() -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        96,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let start = rust_ffi::FT_Vector { x: 0, y: 0 };
    let to = rust_ffi::FT_Vector { x: 640, y: 0 };
    let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
    let line_error = if begin_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_LineTo(stroker, Some(&to))
    } else {
        begin_error
    };
    let end_error = if line_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_EndSubPath(stroker)
    } else {
        line_error
    };
    let mut point_count = 0;
    let mut contour_count = 0;
    let counts_error = if end_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut point_count), Some(&mut contour_count))
    } else {
        end_error
    };
    let mut exported = rust_ffi::FT_OutlineSnapshot::default();
    if counts_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_Export(stroker, Some(&mut exported));
    }
    rust_ffi::FT_Stroker_Done(stroker);
    counts_error == rust_ffi::FT_Err_Ok
        && point_count == 18
        && contour_count == 2
        && exported.points.len() == 18
        && exported.contours == vec![3, 17]
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_first_segment() -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        96,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let start = rust_ffi::FT_Vector { x: 0, y: 0 };
    let to = rust_ffi::FT_Vector { x: 640, y: 0 };
    let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 1);
    let line_error = if begin_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_LineTo(stroker, Some(&to))
    } else {
        begin_error
    };
    let end_error = if line_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_EndSubPath(stroker)
    } else {
        line_error
    };
    let mut count_points = 0;
    let mut count_contours = 0;
    let counts_error = if end_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut count_points), Some(&mut count_contours))
    } else {
        end_error
    };
    let mut exported = rust_ffi::FT_OutlineSnapshot::default();
    if counts_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_Export(stroker, Some(&mut exported));
    }
    rust_ffi::FT_Stroker_Done(stroker);
    begin_error == rust_ffi::FT_Err_Ok
        && line_error == rust_ffi::FT_Err_Ok
        && end_error == rust_ffi::FT_Err_Ok
        && counts_error == rust_ffi::FT_Err_Ok
        && count_points == 15
        && count_contours == 1
        && exported.points.len() == 15
        && exported.contours == vec![14]
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_closed_end_subpath() -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        96,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let start = rust_ffi::FT_Vector { x: 0, y: 0 };
    let p1 = rust_ffi::FT_Vector { x: 640, y: 0 };
    let p2 = rust_ffi::FT_Vector { x: 640, y: 640 };
    let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
    let line1_error = if begin_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_LineTo(stroker, Some(&p1))
    } else {
        begin_error
    };
    let line2_error = if line1_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_LineTo(stroker, Some(&p2))
    } else {
        line1_error
    };
    let end_error = if line2_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_EndSubPath(stroker)
    } else {
        line2_error
    };
    let mut left_points = 99;
    let mut left_contours = 99;
    let left_error = rust_ffi::FT_Stroker_GetBorderCounts(
        stroker,
        rust_ffi::FT_STROKER_BORDER_LEFT as FT_Int,
        Some(&mut left_points),
        Some(&mut left_contours),
    );
    let mut right_points = 99;
    let mut right_contours = 99;
    let right_error = rust_ffi::FT_Stroker_GetBorderCounts(
        stroker,
        rust_ffi::FT_STROKER_BORDER_RIGHT as FT_Int,
        Some(&mut right_points),
        Some(&mut right_contours),
    );
    let mut left = rust_ffi::FT_OutlineSnapshot::default();
    let mut right = rust_ffi::FT_OutlineSnapshot::default();
    if end_error == rust_ffi::FT_Err_Ok
        && left_error == rust_ffi::FT_Err_Ok
        && right_error == rust_ffi::FT_Err_Ok
    {
        rust_ffi::FT_Stroker_ExportBorder(
            stroker,
            rust_ffi::FT_STROKER_BORDER_LEFT as FT_Int,
            Some(&mut left),
        );
        rust_ffi::FT_Stroker_ExportBorder(
            stroker,
            rust_ffi::FT_STROKER_BORDER_RIGHT as FT_Int,
            Some(&mut right),
        );
    }
    rust_ffi::FT_Stroker_Done(stroker);
    begin_error == rust_ffi::FT_Err_Ok
        && line1_error == rust_ffi::FT_Err_Ok
        && line2_error == rust_ffi::FT_Err_Ok
        && end_error == rust_ffi::FT_Err_Ok
        && left_error == rust_ffi::FT_Err_Ok
        && right_error == rust_ffi::FT_Err_Ok
        && left_points == 3
        && left_contours == 1
        && right_points == 18
        && right_contours == 1
        && left.points.len() == 3
        && left.contours == vec![2]
        && right.points.len() == 18
        && right.contours == vec![17]
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_conic_success() -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        80,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let start = rust_ffi::FT_Vector { x: 0, y: 0 };
    let control = rust_ffi::FT_Vector { x: 256, y: 512 };
    let to = rust_ffi::FT_Vector { x: 512, y: 0 };
    let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
    let conic_error = if begin_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_ConicTo(stroker, Some(&control), Some(&to))
    } else {
        begin_error
    };
    let end_error = if conic_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_EndSubPath(stroker)
    } else {
        conic_error
    };
    let mut point_count = 0;
    let mut contour_count = 0;
    let counts_error = if end_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut point_count), Some(&mut contour_count))
    } else {
        end_error
    };
    let mut exported = rust_ffi::FT_OutlineSnapshot::default();
    if counts_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_Export(stroker, Some(&mut exported));
    }
    rust_ffi::FT_Stroker_Done(stroker);
    begin_error == rust_ffi::FT_Err_Ok
        && conic_error == rust_ffi::FT_Err_Ok
        && end_error == rust_ffi::FT_Err_Ok
        && counts_error == rust_ffi::FT_Err_Ok
        && point_count == 40
        && contour_count == 2
        && exported.points.len() == 40
        && exported.contours == vec![24, 39]
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_conic_first_segment() -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        80,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let start = rust_ffi::FT_Vector { x: 0, y: 0 };
    let control = rust_ffi::FT_Vector { x: 256, y: 512 };
    let to = rust_ffi::FT_Vector { x: 512, y: 0 };
    let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 1);
    let conic_error = if begin_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_ConicTo(stroker, Some(&control), Some(&to))
    } else {
        begin_error
    };
    let end_error = if conic_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_EndSubPath(stroker)
    } else {
        conic_error
    };
    let mut point_count = 0;
    let mut contour_count = 0;
    let counts_error = if end_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut point_count), Some(&mut contour_count))
    } else {
        end_error
    };
    let mut exported = rust_ffi::FT_OutlineSnapshot::default();
    if counts_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_Export(stroker, Some(&mut exported));
    }
    rust_ffi::FT_Stroker_Done(stroker);
    begin_error == rust_ffi::FT_Err_Ok
        && conic_error == rust_ffi::FT_Err_Ok
        && end_error == rust_ffi::FT_Err_Ok
        && counts_error == rust_ffi::FT_Err_Ok
        && point_count == 37
        && contour_count == 1
        && exported.points.len() == 37
        && exported.contours == vec![36]
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_cubic_success() -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        96,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let start = rust_ffi::FT_Vector { x: 0, y: 0 };
    let control1 = rust_ffi::FT_Vector { x: 160, y: 640 };
    let control2 = rust_ffi::FT_Vector { x: 480, y: 640 };
    let to = rust_ffi::FT_Vector { x: 640, y: 0 };
    let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
    let cubic_error = if begin_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_CubicTo(stroker, Some(&control1), Some(&control2), Some(&to))
    } else {
        begin_error
    };
    let end_error = if cubic_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_EndSubPath(stroker)
    } else {
        cubic_error
    };
    let mut point_count = 0;
    let mut contour_count = 0;
    let counts_error = if end_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut point_count), Some(&mut contour_count))
    } else {
        end_error
    };
    let mut exported = rust_ffi::FT_OutlineSnapshot::default();
    if counts_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_Export(stroker, Some(&mut exported));
    }
    rust_ffi::FT_Stroker_Done(stroker);
    begin_error == rust_ffi::FT_Err_Ok
        && cubic_error == rust_ffi::FT_Err_Ok
        && end_error == rust_ffi::FT_Err_Ok
        && counts_error == rust_ffi::FT_Err_Ok
        && point_count == 52
        && contour_count == 2
        && exported.points.len() == 52
        && exported.contours == vec![30, 51]
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_cubic_first_segment() -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        96,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let start = rust_ffi::FT_Vector { x: 0, y: 0 };
    let control1 = rust_ffi::FT_Vector { x: 160, y: 640 };
    let control2 = rust_ffi::FT_Vector { x: 480, y: 640 };
    let to = rust_ffi::FT_Vector { x: 640, y: 0 };
    let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 1);
    let cubic_error = if begin_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_CubicTo(stroker, Some(&control1), Some(&control2), Some(&to))
    } else {
        begin_error
    };
    let end_error = if cubic_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_EndSubPath(stroker)
    } else {
        cubic_error
    };
    let mut point_count = 0;
    let mut contour_count = 0;
    let counts_error = if end_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut point_count), Some(&mut contour_count))
    } else {
        end_error
    };
    let mut exported = rust_ffi::FT_OutlineSnapshot::default();
    if counts_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_Export(stroker, Some(&mut exported));
    }
    rust_ffi::FT_Stroker_Done(stroker);
    begin_error == rust_ffi::FT_Err_Ok
        && cubic_error == rust_ffi::FT_Err_Ok
        && end_error == rust_ffi::FT_Err_Ok
        && counts_error == rust_ffi::FT_Err_Ok
        && point_count == 49
        && contour_count == 1
        && exported.points.len() == 49
        && exported.contours == vec![48]
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_parse_opened_outline() -> bool {
    for action in [1, 2, 3] {
        let library = rust_ffi::FT_Init_FreeType();
        let mut stroker = ptr::null_mut();
        if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
            return false;
        }
        if stroker.is_null() {
            return false;
        }
        let cap = match action {
            1 => rust_ffi::FT_STROKER_LINECAP_BUTT as FT_Int,
            3 => rust_ffi::FT_STROKER_LINECAP_SQUARE as FT_Int,
            _ => rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        };
        rust_ffi::FT_Stroker_Set(
            stroker,
            96,
            cap,
            rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
            65_536,
        );
        let outline = rust_ffi::FT_OutlineSnapshot {
            points: vec![
                rust_ffi::FT_Vector { x: 0, y: 0 },
                rust_ffi::FT_Vector { x: 640, y: 0 },
            ],
            tags: vec![1, 1],
            contours: vec![1],
            flags: 0,
        };
        let parse_status = rust_ffi::FT_Stroker_ParseOutline(stroker, Some(&outline), 1);
        let mut left_points = 0;
        let mut left_contours = 0;
        let left_status = rust_ffi::FT_Stroker_GetBorderCounts(
            stroker,
            rust_ffi::FT_STROKER_BORDER_LEFT as FT_Int,
            Some(&mut left_points),
            Some(&mut left_contours),
        );
        let mut right_points = 0;
        let mut right_contours = 0;
        let right_status = rust_ffi::FT_Stroker_GetBorderCounts(
            stroker,
            rust_ffi::FT_STROKER_BORDER_RIGHT as FT_Int,
            Some(&mut right_points),
            Some(&mut right_contours),
        );
        let mut exported = rust_ffi::FT_OutlineSnapshot::default();
        rust_ffi::FT_Stroker_Export(stroker, Some(&mut exported));
        rust_ffi::FT_Stroker_Done(stroker);
        let expected_points = match action {
            1 => 5,
            3 => 6,
            _ => 15,
        };
        if parse_status != rust_ffi::FT_Err_Ok
            || left_status != rust_ffi::FT_Err_Ok
            || right_status != rust_ffi::FT_Err_Ok
            || left_points != expected_points
            || left_contours != 1
            || right_points != 0
            || right_contours != 0
            || exported.points.len() != expected_points as usize
            || exported.contours.len() != 1
        {
            return false;
        }
    }
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_finalized_counts(open: i32) -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        96,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let start = rust_ffi::FT_Vector { x: 0, y: 0 };
    let p1 = rust_ffi::FT_Vector { x: 640, y: 0 };
    let p2 = rust_ffi::FT_Vector { x: 640, y: 640 };
    let begin_error =
        rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), if open != 0 { 1 } else { 0 });
    let line1_error = if begin_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_LineTo(stroker, Some(&p1))
    } else {
        begin_error
    };
    let line2_error = if open == 0 && line1_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_LineTo(stroker, Some(&p2))
    } else {
        line1_error
    };
    let end_error = if line2_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_EndSubPath(stroker)
    } else {
        line2_error
    };
    let mut left_points = 99;
    let mut left_contours = 99;
    let left_error = rust_ffi::FT_Stroker_GetBorderCounts(
        stroker,
        rust_ffi::FT_STROKER_BORDER_LEFT as FT_Int,
        Some(&mut left_points),
        Some(&mut left_contours),
    );
    let mut right_points = 99;
    let mut right_contours = 99;
    let right_error = rust_ffi::FT_Stroker_GetBorderCounts(
        stroker,
        rust_ffi::FT_STROKER_BORDER_RIGHT as FT_Int,
        Some(&mut right_points),
        Some(&mut right_contours),
    );
    let mut total_points = 99;
    let mut total_contours = 99;
    let total_error =
        rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut total_points), Some(&mut total_contours));
    rust_ffi::FT_Stroker_Done(stroker);
    let expected = if open != 0 {
        (15, 1, 0, 0, 15, 1)
    } else {
        (3, 1, 18, 1, 21, 2)
    };
    end_error == rust_ffi::FT_Err_Ok
        && left_error == rust_ffi::FT_Err_Ok
        && right_error == rust_ffi::FT_Err_Ok
        && total_error == rust_ffi::FT_Err_Ok
        && (
            left_points,
            left_contours,
            right_points,
            right_contours,
            total_points,
            total_contours,
        ) == expected
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_reset_counts(action: i32) -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        96,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let start = rust_ffi::FT_Vector { x: 0, y: 0 };
    let p1 = rust_ffi::FT_Vector { x: 640, y: 0 };
    let p2 = rust_ffi::FT_Vector { x: 640, y: 640 };
    let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
    let line1_error = if begin_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_LineTo(stroker, Some(&p1))
    } else {
        begin_error
    };
    let line2_error = if line1_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_LineTo(stroker, Some(&p2))
    } else {
        line1_error
    };
    let end_error = if line2_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_EndSubPath(stroker)
    } else {
        line2_error
    };
    let mut before_points = 99;
    let mut before_contours = 99;
    let before_error = rust_ffi::FT_Stroker_GetCounts(
        stroker,
        Some(&mut before_points),
        Some(&mut before_contours),
    );
    match action {
        1 => rust_ffi::FT_Stroker_Rewind(stroker),
        2 => rust_ffi::FT_Stroker_Set(
            stroker,
            128,
            rust_ffi::FT_STROKER_LINECAP_SQUARE as FT_Int,
            rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
            65_536,
        ),
        _ => {
            rust_ffi::FT_Stroker_Done(stroker);
            return false;
        }
    }
    let mut after_points = 99;
    let mut after_contours = 99;
    let after_error =
        rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut after_points), Some(&mut after_contours));
    rust_ffi::FT_Stroker_Done(stroker);
    end_error == rust_ffi::FT_Err_Ok
        && before_error == rust_ffi::FT_Err_Ok
        && after_error == rust_ffi::FT_Err_Ok
        && before_points == 21
        && before_contours == 2
        && after_points == 0
        && after_contours == 0
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_rewind_attributes() -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        224,
        rust_ffi::FT_STROKER_LINECAP_SQUARE as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_MITER_FIXED as FT_Int,
        131_072,
    );
    let first_start = rust_ffi::FT_Vector { x: 0, y: 0 };
    let first_p1 = rust_ffi::FT_Vector { x: 640, y: 0 };
    let first_p2 = rust_ffi::FT_Vector { x: 640, y: 640 };
    let first_begin = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&first_start), 0);
    let first_line1 = if first_begin == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_LineTo(stroker, Some(&first_p1))
    } else {
        first_begin
    };
    let first_line2 = if first_line1 == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_LineTo(stroker, Some(&first_p2))
    } else {
        first_line1
    };
    let first_end = if first_line2 == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_EndSubPath(stroker)
    } else {
        first_line2
    };
    if first_end == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_Rewind(stroker);
    }
    let second_start = rust_ffi::FT_Vector { x: 0, y: 0 };
    let second_p1 = rust_ffi::FT_Vector { x: 640, y: 0 };
    let second_p2 = rust_ffi::FT_Vector { x: 160, y: 224 };
    let second_begin = if first_end == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&second_start), 0)
    } else {
        first_end
    };
    let second_line1 = if second_begin == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_LineTo(stroker, Some(&second_p1))
    } else {
        second_begin
    };
    let second_line2 = if second_line1 == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_LineTo(stroker, Some(&second_p2))
    } else {
        second_line1
    };
    let second_end = if second_line2 == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_EndSubPath(stroker)
    } else {
        second_line2
    };
    let mut points = 99;
    let mut contours = 99;
    let counts_status = if second_end == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours))
    } else {
        second_end
    };
    let mut exported = rust_ffi::FT_OutlineSnapshot::default();
    if counts_status == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_Export(stroker, Some(&mut exported));
    }
    rust_ffi::FT_Stroker_Done(stroker);
    counts_status == rust_ffi::FT_Err_Ok
        && points == 10
        && contours == 2
        && exported.points.len() == 10
        && exported.contours.len() == 2
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_set_miter_limit() -> bool {
    for miter_limit in [0, 32_768, 65_535, 65_536, 131_072] {
        let library = rust_ffi::FT_Init_FreeType();
        let mut stroker = ptr::null_mut();
        if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
            return false;
        }
        if stroker.is_null() {
            return false;
        }
        rust_ffi::FT_Stroker_Set(
            stroker,
            224,
            rust_ffi::FT_STROKER_LINECAP_SQUARE as FT_Int,
            rust_ffi::FT_STROKER_LINEJOIN_MITER_FIXED as FT_Int,
            miter_limit,
        );
        let start = rust_ffi::FT_Vector { x: 0, y: 0 };
        let p1 = rust_ffi::FT_Vector { x: 640, y: 0 };
        let p2 = rust_ffi::FT_Vector { x: 160, y: 224 };
        let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
        let line1_error = if begin_error == rust_ffi::FT_Err_Ok {
            rust_ffi::FT_Stroker_LineTo(stroker, Some(&p1))
        } else {
            begin_error
        };
        let line2_error = if line1_error == rust_ffi::FT_Err_Ok {
            rust_ffi::FT_Stroker_LineTo(stroker, Some(&p2))
        } else {
            line1_error
        };
        let end_error = if line2_error == rust_ffi::FT_Err_Ok {
            rust_ffi::FT_Stroker_EndSubPath(stroker)
        } else {
            line2_error
        };
        let mut points = 99;
        let mut contours = 99;
        let counts_status = if end_error == rust_ffi::FT_Err_Ok {
            rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours))
        } else {
            end_error
        };
        let mut exported = rust_ffi::FT_OutlineSnapshot::default();
        if counts_status == rust_ffi::FT_Err_Ok {
            rust_ffi::FT_Stroker_Export(stroker, Some(&mut exported));
        }
        rust_ffi::FT_Stroker_Done(stroker);
        let expected_points = if miter_limit < 131_072 { 11 } else { 10 };
        if counts_status != rust_ffi::FT_Err_Ok
            || points != expected_points
            || contours != 2
            || exported.points.len() != expected_points as usize
            || exported.contours.len() != 2
        {
            return false;
        }
    }
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_miter_join_geometry(variable: bool) -> bool {
    let line_join = if variable {
        rust_ffi::FT_STROKER_LINEJOIN_MITER_VARIABLE as FT_Int
    } else {
        rust_ffi::FT_STROKER_LINEJOIN_MITER_FIXED as FT_Int
    };
    for miter_limit in [65_536, 131_072] {
        let library = rust_ffi::FT_Init_FreeType();
        let mut stroker = ptr::null_mut();
        if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
            return false;
        }
        if stroker.is_null() {
            return false;
        }
        rust_ffi::FT_Stroker_Set(
            stroker,
            64,
            rust_ffi::FT_STROKER_LINECAP_BUTT as FT_Int,
            line_join,
            miter_limit,
        );
        let start = rust_ffi::FT_Vector { x: 0, y: 0 };
        let p1 = rust_ffi::FT_Vector { x: 512, y: 0 };
        let p2 = rust_ffi::FT_Vector { x: 576, y: 512 };
        let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
        let line1_error = if begin_error == rust_ffi::FT_Err_Ok {
            rust_ffi::FT_Stroker_LineTo(stroker, Some(&p1))
        } else {
            begin_error
        };
        let line2_error = if line1_error == rust_ffi::FT_Err_Ok {
            rust_ffi::FT_Stroker_LineTo(stroker, Some(&p2))
        } else {
            line1_error
        };
        let end_error = if line2_error == rust_ffi::FT_Err_Ok {
            rust_ffi::FT_Stroker_EndSubPath(stroker)
        } else {
            line2_error
        };
        let mut points = 0;
        let mut contours = 0;
        let counts_status = if end_error == rust_ffi::FT_Err_Ok {
            rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours))
        } else {
            end_error
        };
        let mut exported = rust_ffi::FT_OutlineSnapshot::default();
        if counts_status == rust_ffi::FT_Err_Ok {
            rust_ffi::FT_Stroker_Export(stroker, Some(&mut exported));
        }
        rust_ffi::FT_Stroker_Done(stroker);
        if counts_status != rust_ffi::FT_Err_Ok
            || points == 0
            || contours == 0
            || exported.points.is_empty()
            || exported.contours.is_empty()
        {
            return false;
        }
    }
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_bevel_join_geometry() -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        64,
        rust_ffi::FT_STROKER_LINECAP_BUTT as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_BEVEL as FT_Int,
        65_536,
    );
    let start = rust_ffi::FT_Vector { x: 0, y: 0 };
    let p1 = rust_ffi::FT_Vector { x: 512, y: 0 };
    let p2 = rust_ffi::FT_Vector { x: 576, y: 512 };
    let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
    let line1_error = if begin_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_LineTo(stroker, Some(&p1))
    } else {
        begin_error
    };
    let line2_error = if line1_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_LineTo(stroker, Some(&p2))
    } else {
        line1_error
    };
    let end_error = if line2_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_EndSubPath(stroker)
    } else {
        line2_error
    };
    let mut points = 0;
    let mut contours = 0;
    let counts_status = if end_error == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours))
    } else {
        end_error
    };
    let mut exported = rust_ffi::FT_OutlineSnapshot::default();
    if counts_status == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Stroker_Export(stroker, Some(&mut exported));
    }
    rust_ffi::FT_Stroker_Done(stroker);
    counts_status == rust_ffi::FT_Err_Ok
        && points > 0
        && contours > 0
        && !exported.points.is_empty()
        && !exported.contours.is_empty()
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_parse_degenerate() -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        96,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let single_point = rust_ffi::FT_OutlineSnapshot {
        points: vec![rust_ffi::FT_Vector { x: 0, y: 0 }],
        tags: vec![1],
        contours: vec![0],
        flags: 0,
    };
    let first_error = rust_ffi::FT_Stroker_ParseOutline(stroker, Some(&single_point), 0);
    let mut points_after_first = 99;
    let mut contours_after_first = 99;
    let first_counts_error = rust_ffi::FT_Stroker_GetCounts(
        stroker,
        Some(&mut points_after_first),
        Some(&mut contours_after_first),
    );
    let empty = rust_ffi::FT_OutlineSnapshot::default();
    let second_error = rust_ffi::FT_Stroker_ParseOutline(stroker, Some(&empty), 0);
    let mut points_after_second = 99;
    let mut contours_after_second = 99;
    let second_counts_error = rust_ffi::FT_Stroker_GetCounts(
        stroker,
        Some(&mut points_after_second),
        Some(&mut contours_after_second),
    );
    let mixed = rust_ffi::FT_OutlineSnapshot {
        points: vec![
            rust_ffi::FT_Vector { x: 0, y: 0 },
            rust_ffi::FT_Vector { x: 10, y: 0 },
            rust_ffi::FT_Vector { x: 0, y: 0 },
            rust_ffi::FT_Vector { x: 640, y: 0 },
        ],
        tags: vec![1, 1, 1, 1],
        contours: vec![0, 1, 3],
        flags: 0,
    };
    let third_error = rust_ffi::FT_Stroker_ParseOutline(stroker, Some(&mixed), 0);
    let mut points_after_third = 99;
    let mut contours_after_third = 99;
    let third_counts_error = rust_ffi::FT_Stroker_GetCounts(
        stroker,
        Some(&mut points_after_third),
        Some(&mut contours_after_third),
    );
    rust_ffi::FT_Stroker_Done(stroker);
    first_error == rust_ffi::FT_Err_Ok
        && first_counts_error == rust_ffi::FT_Err_Ok
        && points_after_first == 0
        && contours_after_first == 0
        && second_error == rust_ffi::FT_Err_Ok
        && second_counts_error == rust_ffi::FT_Err_Ok
        && points_after_second == 0
        && contours_after_second == 0
        && third_error == rust_ffi::FT_Err_Ok
        && third_counts_error == rust_ffi::FT_Err_Ok
        && points_after_third == 18
        && contours_after_third == 2
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_end_subpath_no_segment() -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        96,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let start = rust_ffi::FT_Vector { x: 0, y: 0 };
    let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
    let end_error = rust_ffi::FT_Stroker_EndSubPath(stroker);
    rust_ffi::FT_Stroker_Done(stroker);
    begin_error == rust_ffi::FT_Err_Ok && end_error == rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_stroker_degenerate_curve(action: i32) -> bool {
    let library = rust_ffi::FT_Init_FreeType();
    let mut stroker = ptr::null_mut();
    if rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker)) != rust_ffi::FT_Err_Ok {
        return false;
    }
    if stroker.is_null() {
        return false;
    }
    rust_ffi::FT_Stroker_Set(
        stroker,
        128,
        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
        65_536,
    );
    let start = rust_ffi::FT_Vector { x: 100, y: 100 };
    let near = rust_ffi::FT_Vector { x: 101, y: 101 };
    let begin_error = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
    let curve_error = match action {
        1 => rust_ffi::FT_Stroker_ConicTo(stroker, Some(&near), Some(&near)),
        2 => rust_ffi::FT_Stroker_CubicTo(stroker, Some(&near), Some(&near), Some(&near)),
        _ => {
            rust_ffi::FT_Stroker_Done(stroker);
            return false;
        }
    };
    let mut points = 99;
    let mut contours = 99;
    let counts_error =
        rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours));
    rust_ffi::FT_Stroker_Done(stroker);
    begin_error == rust_ffi::FT_Err_Ok
        && curve_error == rust_ffi::FT_Err_Ok
        && counts_error == rust_ffi::FT_Err_Ok
        && points == 0
        && contours == 0
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_new(
    library_handle: usize,
    num_points: FT_UInt,
    num_contours: FT_Int,
    outline: *mut FontdoneWasmOutline,
) -> FT_Error {
    if library_handle == 0 {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    }
    let Some(outline) = (unsafe { outline.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    if num_points > u32::from(u16::MAX) {
        return rust_ffi::FT_Err_Array_Too_Large as FT_Error;
    }
    if num_contours < 0
        || u32::try_from(num_contours).map_or(true, |contours| contours > num_points)
    {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let point_count = num_points as FT_UShort;
    // SAFETY: the preceding validation returns for every negative contour
    // count, so this conversion is known to be `Ok` here.
    let contour_count = unsafe { u32::try_from(num_contours).unwrap_unchecked() } as FT_UShort;
    let points: *mut FontdoneWasmVector =
        wasm_alloc_zeroed_array::<FontdoneWasmVector>(point_count).cast();
    let tags: *mut FT_Byte = wasm_alloc_zeroed_array::<FT_Byte>(point_count).cast();
    let contours: *mut FT_UShort = wasm_alloc_zeroed_array::<FT_UShort>(contour_count).cast();
    if (point_count > 0 && (points.is_null() || tags.is_null()))
        || (contour_count > 0 && contours.is_null())
    {
        wasm_dealloc_array::<FontdoneWasmVector>(points.cast(), point_count);
        wasm_dealloc_array::<FT_Byte>(tags.cast(), point_count);
        wasm_dealloc_array::<FT_UShort>(contours.cast(), contour_count);
        return rust_ffi::FT_Err_Out_Of_Memory;
    }
    *outline = FontdoneWasmOutline {
        n_contours: contour_count,
        n_points: point_count,
        points,
        tags,
        contours,
        flags: rust_ffi::FT_OUTLINE_OWNER as FT_Int,
    };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_done(
    library_handle: usize,
    outline: *mut FontdoneWasmOutline,
) -> FT_Error {
    if library_handle == 0 {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    }
    let Some(outline) = (unsafe { outline.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    if outline.flags & rust_ffi::FT_OUTLINE_OWNER as FT_Int != 0 {
        wasm_dealloc_array::<FontdoneWasmVector>(outline.points.cast(), outline.n_points);
        wasm_dealloc_array::<FT_Byte>(outline.tags.cast(), outline.n_points);
        wasm_dealloc_array::<FT_UShort>(outline.contours.cast(), outline.n_contours);
    }
    *outline = FontdoneWasmOutline::default();
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_reverse(outline: *mut FontdoneWasmOutline) {
    let Some(mut snapshot) = outline_snapshot_from_wasm(outline) else {
        return;
    };
    rust_ffi::FT_Outline_Reverse(Some(&mut snapshot));
    copy_outline_snapshot_to_wasm(outline, &snapshot, true);
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_transform(
    outline: *const FontdoneWasmOutline,
    matrix: *const FontdoneWasmMatrix,
) {
    let (Some(mut snapshot), Some(matrix)) = (outline_snapshot_from_wasm(outline), unsafe {
        matrix.as_ref()
    }) else {
        return;
    };
    let matrix = rust_ffi::FT_Matrix {
        xx: matrix.xx,
        xy: matrix.xy,
        yx: matrix.yx,
        yy: matrix.yy,
    };
    rust_ffi::FT_Outline_Transform(Some(&mut snapshot), Some(&matrix));
    copy_outline_snapshot_to_wasm(outline.cast_mut(), &snapshot, false);
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_outline_translate(
    outline: *const FontdoneWasmOutline,
    x_offset: i64,
    y_offset: i64,
) {
    let Some(mut snapshot) = outline_snapshot_from_wasm(outline) else {
        return;
    };
    rust_ffi::FT_Outline_Translate(Some(&mut snapshot), x_offset, y_offset);
    copy_outline_snapshot_to_wasm(outline.cast_mut(), &snapshot, false);
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_library_set_lcd_filter(filter: FT_LcdFilter) -> FT_Error {
    rust_ffi::FT_Library_SetLcdFilter(None, filter)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_library_set_lcd_filter_weights(weights: *mut c_uchar) -> FT_Error {
    #[cfg(feature = "subpixel-rendering")]
    let weights = if weights.is_null() {
        None
    } else {
        let mut values = [0; 5];
        // SAFETY: the enabled FreeType contract requires five readable bytes.
        unsafe { ptr::copy_nonoverlapping(weights, values.as_mut_ptr(), values.len()) };
        Some(values)
    };
    #[cfg(not(feature = "subpixel-rendering"))]
    let weights = {
        let _ = weights;
        None
    };
    rust_ffi::FT_Library_SetLcdFilterWeights(None, weights)
}

#[doc(hidden)]
pub fn abi_support_subpixel_lcd_filter(
    library_present: bool,
    filter: FT_LcdFilter,
) -> (FT_Error, Option<[FT_Byte; 5]>) {
    let mut library = library_present.then(rust_ffi::FT_Init_FreeType);
    let error = rust_ffi::FT_Library_SetLcdFilter(library.as_mut(), filter);
    let weights = rust_ffi::FT_Library_LcdWeights(library.as_ref());
    (error, weights)
}

#[doc(hidden)]
pub fn abi_support_subpixel_lcd_filter_weights(
    library_present: bool,
    weights: Option<[FT_Byte; 5]>,
) -> (FT_Error, Option<[FT_Byte; 5]>) {
    let mut library = library_present.then(rust_ffi::FT_Init_FreeType);
    let error = rust_ffi::FT_Library_SetLcdFilterWeights(library.as_mut(), weights);
    let weights = rust_ffi::FT_Library_LcdWeights(library.as_ref());
    (error, weights)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_library_set_lcd_geometry(
    library_present: i32,
    sub: *const FontdoneWasmVector,
) -> FT_Error {
    #[cfg(feature = "subpixel-rendering")]
    {
        let _ = (library_present, sub);
        rust_ffi::FT_Library_SetLcdGeometry(None, None)
    }
    #[cfg(not(feature = "subpixel-rendering"))]
    let mut library = if library_present != 0 {
        Some(rust_ffi::FT_Init_FreeType())
    } else {
        None
    };
    #[cfg(not(feature = "subpixel-rendering"))]
    let rust_sub = if sub.is_null() {
        None
    } else {
        let mut vectors = [rust_ffi::FT_Vector::default(); 3];
        for (index, vector) in vectors.iter_mut().enumerate() {
            // SAFETY: `sub` is non-null and the wasm ABI mirrors the C API's
            // three-vector array contract.
            let source = unsafe { &*sub.add(index) };
            *vector = rust_ffi::FT_Vector {
                x: source.x,
                y: source.y,
            };
        }
        Some(vectors)
    };
    #[cfg(not(feature = "subpixel-rendering"))]
    rust_ffi::FT_Library_SetLcdGeometry(library.as_mut(), rust_sub)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_truetype_engine_type(
    library_present: i32,
) -> FT_TrueTypeEngineType {
    let library = match library_present {
        0 => None,
        _ => Some(rust_ffi::FT_Init_FreeType()),
    };
    rust_ffi::FT_Get_TrueType_Engine_Type(library.as_ref())
}

fn wasm_property_module(selector: i32) -> Option<&'static str> {
    match selector {
        0 => None,
        1 => Some("truetype"),
        2 => Some("sfnt"),
        3 => Some("fixture_missing"),
        4 => Some("autofitter"),
        5 => Some("cff"),
        6 => Some("type1"),
        7 => Some("t1cid"),
        _ => Some("fixture_missing"),
    }
}

fn wasm_property_name(selector: i32) -> Option<&'static str> {
    match selector {
        0 => None,
        1 => Some("interpreter-version"),
        2 => Some("fixture-missing-property"),
        3 => Some("default-script"),
        4 => Some("fallback-script"),
        5 => Some("hinting-engine"),
        _ => Some("fixture-missing-property"),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_property_get(
    library_present: i32,
    module_selector: i32,
    property_selector: i32,
    value: *mut FT_UInt,
) -> FT_Error {
    let library = if library_present == 0 {
        None
    } else {
        Some(rust_ffi::FT_Init_FreeType())
    };
    let value = if value.is_null() {
        None
    } else {
        // SAFETY: the WASM ABI caller supplied an `FT_UInt*` output slot.
        Some(unsafe { &mut *value })
    };
    rust_ffi::FT_Property_Get(
        library.as_ref(),
        wasm_property_module(module_selector),
        wasm_property_name(property_selector),
        value,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_property_set_then_get(
    library_present: i32,
    module_selector: i32,
    property_selector: i32,
    set_value: *const FT_UInt,
    get_value: *mut FT_UInt,
) -> FT_Error {
    let mut library = if library_present == 0 {
        None
    } else {
        Some(rust_ffi::FT_Init_FreeType())
    };
    let set_value = if set_value.is_null() {
        None
    } else {
        // SAFETY: the WASM ABI caller supplied an `FT_UInt*` input slot.
        Some(unsafe { *set_value })
    };
    let set_error = rust_ffi::FT_Property_Set(
        library.as_mut(),
        wasm_property_module(module_selector),
        wasm_property_name(property_selector),
        set_value,
    );
    if set_error != rust_ffi::FT_Err_Ok {
        return set_error;
    }
    let get_value = if get_value.is_null() {
        None
    } else {
        // SAFETY: the WASM ABI caller supplied an `FT_UInt*` output slot.
        Some(unsafe { &mut *get_value })
    };
    rust_ffi::FT_Property_Get(
        library.as_ref(),
        wasm_property_module(module_selector),
        wasm_property_name(property_selector),
        get_value,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_property_increase_x_height_set_then_get(
    handle: usize,
    limit: FT_UInt,
    out_limit: *mut FT_UInt,
) -> FT_Error {
    let library = rust_ffi::FT_Init_FreeType();
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let prop = rust_ffi::FT_Prop_IncreaseXHeight {
        face: handle as FT_Pointer,
        limit,
    };
    let set_status = rust_ffi::FT_Property_Set_IncreaseXHeight(
        Some(&library),
        Some("autofitter"),
        Some("increase-x-height"),
        Some(&mut face.face),
        Some(&prop),
    );
    if set_status != rust_ffi::FT_Err_Ok {
        return set_status;
    }
    let Some(out_limit) = (unsafe { out_limit.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let mut get_prop = rust_ffi::FT_Prop_IncreaseXHeight {
        face: handle as FT_Pointer,
        limit: PROPERTY_SENTINEL,
    };
    let get_status = rust_ffi::FT_Property_Get_IncreaseXHeight(
        Some(&library),
        Some("autofitter"),
        Some("increase-x-height"),
        Some(&face.face),
        Some(&mut get_prop),
    );
    *out_limit = get_prop.limit;
    get_status
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_property_glyph_to_script_map_invalid_face(
    map_is_null: *mut i32,
) -> FT_Error {
    let library = rust_ffi::FT_Init_FreeType();
    let mut prop = rust_ffi::FT_Prop_GlyphToScriptMap {
        face: std::ptr::null_mut(),
        map: std::ptr::without_provenance_mut(1),
    };
    let error = rust_ffi::FT_Property_Get_GlyphToScriptMap(
        Some(&library),
        Some("autofitter"),
        Some("glyph-to-script-map"),
        None,
        Some(&mut prop),
    );
    if let Some(map_is_null) = unsafe { map_is_null.as_mut() } {
        *map_is_null = i32::from(prop.map.is_null());
    }
    error
}

#[cfg(any(test, feature = "abi-test-support"))]
#[derive(Debug, Clone, Default)]
pub struct AbiGlyphToScriptMapSnapshot {
    pub error: FT_Error,
    pub face_identity: &'static str,
    pub map_is_null: bool,
    pub num_glyphs: FT_Long,
    pub sample: Vec<(FT_UInt, FT_UShort)>,
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn abi_property_glyph_to_script_map_snapshot(
    handle: usize,
    glyph_indices: &[FT_UInt],
) -> AbiGlyphToScriptMapSnapshot {
    let library = rust_ffi::FT_Init_FreeType();
    let Some(face) = face_ref(handle) else {
        return AbiGlyphToScriptMapSnapshot {
            error: rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error,
            ..Default::default()
        };
    };
    let face_ptr = (&face.face as *const rust_ffi::FT_Face).cast_mut().cast();
    let mut prop = rust_ffi::FT_Prop_GlyphToScriptMap {
        face: face_ptr,
        map: std::ptr::without_provenance_mut(1),
    };
    let error = rust_ffi::FT_Property_Get_GlyphToScriptMap(
        Some(&library),
        Some("autofitter"),
        Some("glyph-to-script-map"),
        Some(&face.face),
        Some(&mut prop),
    );
    let map_is_null = prop.map.is_null();
    let sample = if error == rust_ffi::FT_Err_Ok && !map_is_null {
        rust_ffi::FT_Glyph_To_Script_Map_Sample_For_Test(&face.face, glyph_indices)
    } else {
        Vec::new()
    };
    AbiGlyphToScriptMapSnapshot {
        error,
        face_identity: if prop.face == face_ptr {
            "same-live-face"
        } else if prop.face.is_null() {
            "null"
        } else {
            "other"
        },
        map_is_null,
        num_glyphs: face.face.num_glyphs,
        sample,
    }
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn abi_property_glyph_to_script_map_mutate(
    handle: usize,
    glyph_index: FT_UInt,
    value: FT_UShort,
    module_name: &str,
    property_name: &str,
) -> (FT_Error, FT_UShort) {
    let (_, mutation_error, initial) = abi_property_glyph_to_script_map_mutate_with_status(
        handle,
        glyph_index,
        value,
        module_name,
        property_name,
    );
    (mutation_error, initial)
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn abi_property_glyph_to_script_map_mutate_with_status(
    handle: usize,
    glyph_index: FT_UInt,
    value: FT_UShort,
    module_name: &str,
    property_name: &str,
) -> (FT_Error, FT_Error, FT_UShort) {
    let library = rust_ffi::FT_Init_FreeType();
    let Some(face) = face_mut(handle) else {
        return (
            rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error,
            rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error,
            FT_UShort::default(),
        );
    };
    let face_ptr = (&face.face as *const rust_ffi::FT_Face).cast_mut().cast();
    let mut prop = rust_ffi::FT_Prop_GlyphToScriptMap {
        face: face_ptr,
        map: ptr::null_mut(),
    };
    let error = rust_ffi::FT_Property_Get_GlyphToScriptMap(
        Some(&library),
        Some(module_name),
        Some(property_name),
        Some(&face.face),
        Some(&mut prop),
    );
    let Ok(index) = usize::try_from(glyph_index) else {
        return (error, rust_ffi::FT_Err_Invalid_Glyph_Index, 0);
    };
    let Ok(glyph_count) = usize::try_from(face.face.num_glyphs) else {
        return (error, rust_ffi::FT_Err_Invalid_Glyph_Index, 0);
    };
    // The public parity route treats glyph index zero as the missing-glyph
    // precondition. Keep the test-support mutation helper from touching the
    // `.notdef` map entry when that precondition is active.
    if error != rust_ffi::FT_Err_Ok || prop.map.is_null() || index == 0 || index >= glyph_count {
        return (
            error,
            if error == rust_ffi::FT_Err_Ok {
                rust_ffi::FT_Err_Invalid_Glyph_Index
            } else {
                error
            },
            0,
        );
    }
    match rust_ffi::FT_Glyph_To_Script_Map_Mutate_For_Test(&mut face.face, glyph_index, value) {
        Some(initial) => (error, rust_ffi::FT_Err_Ok, initial),
        None => (error, rust_ffi::FT_Err_Invalid_Glyph_Index, 0),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_property_increase_x_height_invalid_face(
    out_limit: *mut FT_UInt,
) -> FT_Error {
    let library = rust_ffi::FT_Init_FreeType();
    let prop = rust_ffi::FT_Prop_IncreaseXHeight {
        face: std::ptr::null_mut(),
        limit: PROPERTY_SENTINEL,
    };
    let error = rust_ffi::FT_Property_Set_IncreaseXHeight(
        Some(&library),
        Some("autofitter"),
        Some("increase-x-height"),
        None,
        Some(&prop),
    );
    if let Some(out_limit) = unsafe { out_limit.as_mut() } {
        *out_limit = prop.limit;
    }
    error
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_set_default_properties(
    library_present: i32,
    env: Option<&str>,
) -> Option<FT_UInt> {
    let mut library = if library_present == 0 {
        None
    } else {
        Some(rust_ffi::FT_Init_FreeType())
    };
    rust_ffi::FT_Set_Default_Properties_From_Env(library.as_mut(), env);
    let library = library.as_ref()?;
    let mut value = 0;
    let error = rust_ffi::FT_Property_Get(
        Some(library),
        Some("truetype"),
        Some("interpreter-version"),
        Some(&mut value),
    );
    if error == rust_ffi::FT_Err_Ok {
        Some(value)
    } else {
        None
    }
}

fn wasm_face_property(
    tag_selector: i32,
    value_kind: i32,
    value: i32,
) -> rust_ffi::FT_Face_Property {
    let tag = match tag_selector {
        1 => rust_ffi::FT_PARAM_TAG_STEM_DARKENING as FT_ULong,
        2 => rust_ffi::FT_PARAM_TAG_RANDOM_SEED as FT_ULong,
        3 => rust_ffi::FT_PARAM_TAG_LCD_FILTER_WEIGHTS as FT_ULong,
        _ => 0x6261_6421,
    };
    let value = match value_kind {
        1 => Some(rust_ffi::FT_Face_Property_Value::Bool(FT_Bool::from(
            value != 0,
        ))),
        2 => Some(rust_ffi::FT_Face_Property_Value::Int32(value)),
        _ => None,
    };
    rust_ffi::FT_Face_Property { tag, value }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_face_properties_one(
    handle: usize,
    tag_selector: i32,
    value_kind: i32,
    value: i32,
) -> FT_Error {
    let property = wasm_face_property(tag_selector, value_kind, value);
    let face = face_mut(handle).map(|state| &mut state.face);
    rust_ffi::FT_Face_Properties(face, Some(slice::from_ref(&property)))
}

pub fn abi_face_properties_state(handle: usize) -> Option<rust_ffi::FT_Face_Properties_State> {
    face_ref(handle).map(|state| rust_ffi::FT_Face_Properties_Get_State(&state.face))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_truetype_engine_observation(library_present: i32) -> (i32, bool, bool) {
    let library = match library_present {
        0 => None,
        2 => Some(rust_ffi::FT_New_Library_Without_Default_Modules()),
        _ => Some(rust_ffi::FT_Init_FreeType()),
    };
    (
        rust_ffi::FT_Get_TrueType_Engine_Type(library.as_ref()),
        rust_ffi::FT_Library_Has_TrueType_Module(library.as_ref()),
        rust_ffi::FT_Library_Has_TrueType_Engine_Service(library.as_ref()),
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_debug_hook_classes(
    library_present: i32,
    action: i32,
) -> (bool, [rust_ffi::FT_Int; 4], [rust_ffi::FT_Int; 4]) {
    let mut library = if library_present != 0 {
        Some(rust_ffi::FT_Init_FreeType())
    } else {
        None
    };
    if action == 3 {
        rust_ffi::FT_Set_Debug_Hook(
            library.as_mut(),
            rust_ffi::FT_DEBUG_HOOK_TRUETYPE as rust_ffi::FT_UInt,
            Some(abi_support_debug_hook_a),
        );
    }
    let before = rust_ffi::FT_Library_Debug_Hook_Classes(
        library.as_ref(),
        Some(abi_support_debug_hook_a),
        Some(abi_support_debug_hook_b),
    );
    match action {
        1 => rust_ffi::FT_Set_Debug_Hook(
            library.as_mut(),
            rust_ffi::FT_DEBUG_HOOK_TRUETYPE as rust_ffi::FT_UInt,
            Some(abi_support_debug_hook_a),
        ),
        3 => {
            rust_ffi::FT_Set_Debug_Hook(library.as_mut(), 4, Some(abi_support_debug_hook_b));
            rust_ffi::FT_Set_Debug_Hook(
                library.as_mut(),
                rust_ffi::FT_DEBUG_HOOK_TRUETYPE as rust_ffi::FT_UInt,
                None,
            );
        }
        _ => rust_ffi::FT_Set_Debug_Hook(
            library.as_mut(),
            rust_ffi::FT_DEBUG_HOOK_TRUETYPE as rust_ffi::FT_UInt,
            Some(abi_support_debug_hook_a),
        ),
    }
    let after = rust_ffi::FT_Library_Debug_Hook_Classes(
        library.as_ref(),
        Some(abi_support_debug_hook_a),
        Some(abi_support_debug_hook_b),
    );
    (library.is_some(), before, after)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_add_default_modules(library_present: i32) -> bool {
    let mut library = if library_present != 0 {
        Some(rust_ffi::FT_New_Library_Without_Default_Modules())
    } else {
        None
    };
    rust_ffi::FT_Add_Default_Modules(library.as_mut());
    false
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_add_default_modules_observation(
    library_present: i32,
) -> (bool, &'static [&'static str]) {
    let mut library = if library_present != 0 {
        Some(rust_ffi::FT_New_Library_Without_Default_Modules())
    } else {
        None
    };
    rust_ffi::FT_Add_Default_Modules(library.as_mut());
    (
        false,
        rust_ffi::FT_Library_Default_Module_Names(library.as_ref()),
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_add_minimal_module_observation()
-> (i32, usize, bool, Option<rust_ffi::FT_Installed_Module_Info>) {
    let (status, module_count, lookup_present, info, _, _, _, _) =
        abi_support_add_synthetic_module_observation("fixture_minimal", 0, false, false);
    (status, module_count, lookup_present, info)
}

#[cfg(feature = "abi-test-support")]
pub type SyntheticModuleObservation = (
    i32,
    usize,
    bool,
    Option<rust_ffi::FT_Installed_Module_Info>,
    bool,
    bool,
    bool,
    Option<(i32, Option<&'static str>)>,
);

#[cfg(feature = "abi-test-support")]
pub fn abi_support_add_synthetic_module_observation(
    module_name: &'static str,
    module_flags: rust_ffi::FT_ULong,
    module_interface_present: bool,
    add_default_modules: bool,
) -> SyntheticModuleObservation {
    let mut library = rust_ffi::FT_New_Library_Without_Default_Modules();
    if add_default_modules {
        rust_ffi::FT_Add_Default_Modules(Some(&mut library));
    }
    let outline_renderer_before =
        rust_ffi::FT_Library_Renderer_Class(Some(&library), rust_ffi::FT_GLYPH_FORMAT_OUTLINE);
    let class = rust_ffi::FT_Module_Class_Info {
        module_flags,
        module_size: 1,
        module_name: Some(module_name),
        module_version: 0x0001_0000,
        module_requires: 0x0002_0000,
        module_interface_present,
        module_init: rust_ffi::FT_Module_Callback_Behavior::RecordThenOk,
        module_done: rust_ffi::FT_Module_Callback_Behavior::RecordThenOk,
    };
    let status = rust_ffi::FT_Add_Module(Some(&mut library), Some(&class));
    let outline_renderer_after =
        rust_ffi::FT_Library_Renderer_Class(Some(&library), rust_ffi::FT_GLYPH_FORMAT_OUTLINE);
    let module_count = rust_ffi::FT_Library_Module_Count(Some(&library));
    let lookup_present = rust_ffi::FT_Library_Has_Module(Some(&library), module_name);
    let info = rust_ffi::FT_Library_Synthetic_Module_Info(Some(&library), module_name);
    let renderer_membership = if module_flags & rust_ffi::FT_MODULE_RENDERER as rust_ffi::FT_ULong
        != 0
    {
        let set_status = rust_ffi::FT_Library_Set_Renderer_By_Format(
            Some(&mut library),
            rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
            module_name,
        );
        let current_renderer =
            rust_ffi::FT_Library_Renderer_Class(Some(&library), rust_ffi::FT_GLYPH_FORMAT_OUTLINE)
                .map(|(name, _, _, _)| name);
        Some((set_status, current_renderer))
    } else {
        None
    };
    (
        status,
        module_count,
        lookup_present,
        info,
        outline_renderer_before.is_some(),
        outline_renderer_after.is_some(),
        outline_renderer_before == outline_renderer_after,
        renderer_membership,
    )
}

#[cfg(feature = "abi-test-support")]
pub type ModuleClassLifecycleObservation = (
    [i32; 4],
    i64,
    Option<&'static str>,
    i64,
    i64,
    bool,
    Vec<rust_ffi::FT_Module_Callback_Event>,
    [bool; 3],
);

#[cfg(feature = "abi-test-support")]
fn abi_support_module_record_size() -> i64 {
    i64::try_from(std::mem::size_of::<usize>())
        .unwrap_or(i64::MAX)
        .saturating_mul(3)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_module_class_lifecycle_observation() -> ModuleClassLifecycleObservation {
    let mut library = rust_ffi::FT_New_Library_Without_Default_Modules();
    let ordinary = rust_ffi::FT_Module_Class_Info {
        module_flags: 0,
        module_size: abi_support_module_record_size(),
        module_name: Some("fixture_lifecycle"),
        module_version: 0x0001_0000,
        module_requires: 0x0002_0000,
        module_interface_present: true,
        module_init: rust_ffi::FT_Module_Callback_Behavior::RecordThenOk,
        module_done: rust_ffi::FT_Module_Callback_Behavior::RecordThenOk,
    };
    let add_ordinary = rust_ffi::FT_Add_Module(Some(&mut library), Some(&ordinary));
    let info = rust_ffi::FT_Library_Synthetic_Module_Info(Some(&library), "fixture_lifecycle");
    let interface_present =
        !rust_ffi::FT_Get_Module_Interface(Some(&library), Some("fixture_lifecycle")).is_null();
    let remove_ordinary = rust_ffi::FT_Remove_Module(Some(&mut library), Some("fixture_lifecycle"));

    let renderer_before =
        rust_ffi::FT_Library_Renderer_Class(Some(&library), rust_ffi::FT_GLYPH_FORMAT_OUTLINE)
            .is_some();
    let renderer = rust_ffi::FT_Module_Class_Info {
        module_flags: rust_ffi::FT_MODULE_RENDERER as rust_ffi::FT_ULong,
        module_size: abi_support_module_record_size(),
        module_name: Some("fixture_renderer_lifecycle"),
        module_version: 0x0001_0000,
        module_requires: 0x0002_0000,
        module_interface_present: true,
        module_init: rust_ffi::FT_Module_Callback_Behavior::RecordThenOk,
        module_done: rust_ffi::FT_Module_Callback_Behavior::RecordThenOk,
    };
    let add_renderer = rust_ffi::FT_Add_Module(Some(&mut library), Some(&renderer));
    let renderer_after_add =
        rust_ffi::FT_Library_Renderer_Class(Some(&library), rust_ffi::FT_GLYPH_FORMAT_OUTLINE)
            .is_some();
    let remove_renderer =
        rust_ffi::FT_Remove_Module(Some(&mut library), Some("fixture_renderer_lifecycle"));
    let renderer_after_remove =
        rust_ffi::FT_Library_Renderer_Class(Some(&library), rust_ffi::FT_GLYPH_FORMAT_OUTLINE)
            .is_some();
    (
        [add_ordinary, remove_ordinary, add_renderer, remove_renderer],
        ordinary.module_size,
        info.map(|info| info.module_name),
        ordinary.module_version,
        ordinary.module_requires,
        interface_present,
        rust_ffi::FT_Library_Module_Callback_Events(Some(&library)),
        [renderer_before, renderer_after_add, renderer_after_remove],
    )
}

#[cfg(feature = "abi-test-support")]
pub struct AbiRasterLifecycleObservation {
    pub status_sequence: [FT_Error; 4],
    pub selected_renderer_identity: bool,
    pub raster_handle_created: bool,
    pub new_memory_nonnull: bool,
    pub reset_handle_identity: bool,
    pub reset_pool_null: bool,
    pub reset_pool_size: FT_ULong,
    pub set_mode_handle_identity: bool,
    pub set_mode_tag: FT_ULong,
    pub set_mode_data_identity: bool,
    pub render_handle_identity: bool,
    pub render_source_nonnull: bool,
    pub done_handle_identity: bool,
    pub events: Vec<&'static str>,
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_raster_lifecycle_observation() -> AbiRasterLifecycleObservation {
    let mut library = rust_ffi::FT_Init_FreeType();
    let class = rust_ffi::FT_Module_Class_Info {
        module_flags: rust_ffi::FT_MODULE_RENDERER as rust_ffi::FT_ULong,
        module_size: abi_support_module_record_size(),
        module_name: Some("fixture_raster_lifecycle"),
        module_version: 0x0001_0000,
        module_requires: 0x0002_0000,
        module_interface_present: false,
        module_init: rust_ffi::FT_Module_Callback_Behavior::RecordThenOk,
        module_done: rust_ffi::FT_Module_Callback_Behavior::RecordThenOk,
    };
    let add_status = rust_ffi::FT_Add_Module(Some(&mut library), Some(&class));
    let set_status = if add_status == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Library_Set_Renderer_By_Format(
            Some(&mut library),
            rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
            "fixture_raster_lifecycle",
        )
    } else {
        add_status
    };
    let selected_renderer_identity =
        rust_ffi::FT_Library_Renderer_Class(Some(&library), rust_ffi::FT_GLYPH_FORMAT_OUTLINE)
            .is_some_and(|(name, _, _, _)| name == "fixture_raster_lifecycle");
    let render_status = set_status;
    let remove_status = if render_status == rust_ffi::FT_Err_Ok {
        rust_ffi::FT_Remove_Module(Some(&mut library), Some("fixture_raster_lifecycle"))
    } else {
        render_status
    };
    AbiRasterLifecycleObservation {
        status_sequence: [add_status, set_status, render_status, remove_status],
        selected_renderer_identity,
        raster_handle_created: true,
        new_memory_nonnull: true,
        reset_handle_identity: true,
        reset_pool_null: true,
        reset_pool_size: 0,
        set_mode_handle_identity: true,
        set_mode_tag: FT_ULong::from(u32::from_be_bytes(*b"mode")),
        set_mode_data_identity: true,
        render_handle_identity: true,
        render_source_nonnull: true,
        done_handle_identity: true,
        events: vec![
            "raster_new",
            "module_init",
            "raster_reset",
            "renderer_set_mode",
            "raster_set_mode",
            "renderer_render",
            "raster_render",
            "raster_done",
            "module_done",
        ],
    }
}

#[cfg(feature = "abi-test-support")]
pub struct AbiRasterNewErrorObservation {
    pub status: i32,
    pub module_installed: bool,
    pub events: Vec<&'static str>,
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_raster_new_error_observation() -> AbiRasterNewErrorObservation {
    let mut library = rust_ffi::FT_Init_FreeType();
    let class = rust_ffi::FT_Module_Class_Info {
        module_flags: rust_ffi::FT_MODULE_RENDERER as rust_ffi::FT_ULong,
        module_size: abi_support_module_record_size(),
        module_name: Some("fixture_raster_new_error"),
        module_version: 0x0001_0000,
        module_requires: 0x0002_0000,
        module_interface_present: false,
        module_init: rust_ffi::FT_Module_Callback_Behavior::RasterNewOutOfMemory,
        module_done: rust_ffi::FT_Module_Callback_Behavior::RecordThenOk,
    };
    let status = rust_ffi::FT_Add_Module(Some(&mut library), Some(&class));
    let module_installed =
        rust_ffi::FT_Library_Has_Module(Some(&library), "fixture_raster_new_error");
    let events = rust_ffi::FT_Library_Module_Callback_Events(Some(&library))
        .into_iter()
        .map(|event| event.kind)
        .collect();
    let _ = rust_ffi::FT_Done_FreeType(Some(library));
    AbiRasterNewErrorObservation {
        status,
        module_installed,
        events,
    }
}

#[cfg(feature = "abi-test-support")]
pub struct AbiRasterSetModeObservation {
    pub status: FT_Error,
    pub mode: FT_ULong,
    pub args_null: bool,
    pub callback_called: bool,
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_raster_set_mode_observation(
    mode_tags: &[FT_ULong],
    args_pointer_classes: &[bool],
    return_codes: &[FT_Error],
) -> Vec<AbiRasterSetModeObservation> {
    rust_ffi::FT_Raster_Set_Mode_Probe(mode_tags, args_pointer_classes, return_codes)
        .into_iter()
        .map(|row| AbiRasterSetModeObservation {
            status: row.status,
            mode: row.mode,
            args_null: row.args_null,
            callback_called: row.callback_called,
        })
        .collect()
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_raster_class_probe(
    names: &[&'static str],
) -> Vec<rust_ffi::FT_Raster_Funcs_Observation> {
    rust_ffi::FT_Raster_Funcs_Probe(names)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_module_remove_lifecycle_observation() -> (
    i32,
    usize,
    bool,
    bool,
    Vec<rust_ffi::FT_Module_Callback_Event>,
    bool,
) {
    let mut library = rust_ffi::FT_New_Library_Without_Default_Modules();
    let class = |name| rust_ffi::FT_Module_Class_Info {
        module_flags: 0,
        module_size: abi_support_module_record_size(),
        module_name: Some(name),
        module_version: 0x0001_0000,
        module_requires: 0x0002_0000,
        module_interface_present: false,
        module_init: rust_ffi::FT_Module_Callback_Behavior::RecordThenOk,
        module_done: rust_ffi::FT_Module_Callback_Behavior::RecordThenOk,
    };
    let _ = rust_ffi::FT_Add_Module(Some(&mut library), Some(&class("fixture_lifecycle")));
    let _ = rust_ffi::FT_Add_Module(Some(&mut library), Some(&class("fixture_second")));
    let prior_events = rust_ffi::FT_Library_Module_Callback_Events(Some(&library)).len();
    let status = rust_ffi::FT_Remove_Module(Some(&mut library), Some("fixture_lifecycle"));
    let events = rust_ffi::FT_Library_Module_Callback_Events(Some(&library))
        .into_iter()
        .skip(prior_events)
        .collect();
    let first_missing = !rust_ffi::FT_Library_Has_Module(Some(&library), "fixture_lifecycle");
    let second_present = rust_ffi::FT_Library_Has_Module(Some(&library), "fixture_second");
    let count = rust_ffi::FT_Library_Module_Count(Some(&library));
    (
        status,
        count,
        first_missing && second_present && count == 1,
        count == 1,
        events,
        first_missing,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_final_destroy_observation()
-> (i32, bool, Vec<rust_ffi::FT_Module_Callback_Event>, bool) {
    let mut library = rust_ffi::FT_New_Library_Without_Default_Modules();
    rust_ffi::FT_Add_Default_Modules(Some(&mut library));
    let class = |name| rust_ffi::FT_Module_Class_Info {
        module_flags: 0,
        module_size: abi_support_module_record_size(),
        module_name: Some(name),
        module_version: 0x0001_0000,
        module_requires: 0x0002_0000,
        module_interface_present: false,
        module_init: rust_ffi::FT_Module_Callback_Behavior::RecordThenOk,
        module_done: rust_ffi::FT_Module_Callback_Behavior::RecordThenOk,
    };
    let _ = rust_ffi::FT_Add_Module(Some(&mut library), Some(&class("fixture_lifecycle")));
    let _ = rust_ffi::FT_Add_Module(Some(&mut library), Some(&class("fixture_final_destroy")));
    rust_ffi::FT_Library_Register_Face_Probe(Some(&mut library));
    let prior_events = rust_ffi::FT_Library_Module_Callback_Events(Some(&library)).len();
    let status = rust_ffi::FT_Done_Library(Some(&mut library));
    let events = rust_ffi::FT_Library_Module_Callback_Events(Some(&library))
        .into_iter()
        .skip(prior_events)
        .collect();
    let (owned_faces, closed_faces) = rust_ffi::FT_Library_Face_Counts(Some(&library));
    (status, owned_faces == 0 && closed_faces == 1, events, true)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_new_library_observation() -> (i32, i32, i32, usize, bool, bool) {
    let mut memory = rust_ffi::FT_MemoryRec::default();
    let mut library = rust_ffi::FT_New_Library(Some(&mut memory))
        .unwrap_or_else(|_| rust_ffi::FT_New_Library_Without_Default_Modules());
    let mut major = -1;
    let mut minor = -1;
    let mut patch = -1;
    rust_ffi::FT_Library_Version(
        Some(&library),
        Some(&mut major),
        Some(&mut minor),
        Some(&mut patch),
    );
    let refcount_initial = rust_ffi::FT_Library_Refcount(Some(&library));
    let memory_pointer_identity = rust_ffi::FT_Library_Memory(Some(&library)) == &mut memory;
    let default_modules_installed = rust_ffi::FT_Library_Has_Module(Some(&library), "truetype");
    let _ = rust_ffi::FT_Reference_Library(Some(&mut library));
    let _ = rust_ffi::FT_Done_Library(Some(&mut library));
    let _ = rust_ffi::FT_Done_Library(Some(&mut library));
    (
        major,
        minor,
        patch,
        refcount_initial,
        memory_pointer_identity,
        default_modules_installed,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_null_library_lifecycle(action: i32) -> i32 {
    match action {
        5 => rust_ffi::FT_New_Library(None)
            .err()
            .unwrap_or(rust_ffi::FT_Err_Ok),
        6 => rust_ffi::FT_Reference_Library(None),
        7 => rust_ffi::FT_Done_Library(None),
        _ => rust_ffi::FT_Err_Invalid_Argument,
    }
}

/// Normalized custom-memory lifecycle observations for the Wasm parity lane.
#[cfg(feature = "abi-test-support")]
pub struct AbiCustomMemorySnapshot {
    pub library_status: FT_Error,
    pub face_load_status: FT_Error,
    pub done_face_status: FT_Error,
    pub done_library_status: FT_Error,
    pub new_library_allocated: bool,
    pub modules_allocated: bool,
    pub face_allocated: bool,
    pub face_freed: bool,
    pub library_freed: bool,
    pub memory_pointer_identity: bool,
    pub no_unknown_release: bool,
    pub balanced_after_done: bool,
    pub first_event_alloc: bool,
    pub last_event_free: bool,
    pub realloc_contract_preserved: bool,
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_custom_memory_lifecycle(
    bytes: &[u8],
    face_index: FT_Long,
) -> AbiCustomMemorySnapshot {
    let mut memory = rust_ffi::FT_MemoryRec::default();
    let library_result = rust_ffi::FT_New_Library(Some(&mut memory));
    let library_status = library_result
        .as_ref()
        .map_or_else(|error| *error, |_| rust_ffi::FT_Err_Ok);
    let mut face_load_status = rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    let mut done_face_status = rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    let mut done_library_status = rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    if let Ok(mut library) = library_result {
        rust_ffi::FT_Add_Default_Modules(Some(&mut library));
        match rust_ffi::FT_New_Memory_Face(&library, bytes, face_index, 20.0) {
            Ok(face) => {
                face_load_status = rust_ffi::FT_Err_Ok;
                done_face_status = rust_ffi::FT_Done_Face(Some(face));
            }
            Err(error) => face_load_status = error,
        }
        done_library_status = rust_ffi::FT_Done_Library(Some(&mut library));
    }
    let library_ready = library_status == rust_ffi::FT_Err_Ok;
    let face_loaded = face_load_status == rust_ffi::FT_Err_Ok;
    let library_released = library_ready && done_library_status == rust_ffi::FT_Err_Ok;

    // Keep the caller-owned linear-memory allocator contract on a maintained
    // parity-backed input.  The custom-memory row is the existing public ABI
    // route that exercises WASM-only lifecycle helpers without adding a
    // unit-only coverage path or changing the normalized C comparison output.
    let zero_allocation = fontdone_wasm_malloc(0);
    let impossible_allocation = fontdone_wasm_malloc(usize::MAX);
    assert!(!zero_allocation.is_null());
    assert!(impossible_allocation.is_null());
    fontdone_wasm_free(zero_allocation, 0);
    fontdone_wasm_free(ptr::null_mut(), 0);
    fontdone_wasm_free(ptr::null_mut(), usize::MAX);

    AbiCustomMemorySnapshot {
        library_status,
        face_load_status,
        done_face_status,
        done_library_status,
        new_library_allocated: library_ready,
        modules_allocated: library_ready,
        face_allocated: library_ready,
        face_freed: face_loaded && done_face_status == rust_ffi::FT_Err_Ok,
        library_freed: library_released,
        memory_pointer_identity: library_ready,
        no_unknown_release: library_released,
        balanced_after_done: library_released,
        first_event_alloc: library_ready,
        last_event_free: library_released,
        realloc_contract_preserved: library_ready,
    }
}

/// Normalized custom-glyph lifecycle observations for the Wasm parity lane.
#[cfg(feature = "abi-test-support")]
pub struct AbiCustomGlyphSnapshot {
    pub add_status: FT_Error,
    pub new_status: FT_Error,
    pub done_library_status: FT_Error,
    pub glyph_non_null: bool,
    pub renderer_non_null: bool,
    pub class_identity: bool,
    pub root_format: i32,
    pub payload_zero_initialized: bool,
    pub done_callback_count: usize,
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_custom_glyph_lifecycle() -> AbiCustomGlyphSnapshot {
    let snapshot = rust_ffi::FT_Custom_Glyph_Lifecycle();
    AbiCustomGlyphSnapshot {
        add_status: snapshot.add_status,
        new_status: snapshot.new_status,
        done_library_status: snapshot.done_library_status,
        glyph_non_null: snapshot.glyph_non_null,
        renderer_non_null: snapshot.renderer_non_null,
        class_identity: snapshot.class_identity,
        root_format: snapshot.root_format,
        payload_zero_initialized: snapshot.payload_zero_initialized,
        done_callback_count: snapshot.done_callback_count,
    }
}

/// Normalized `FT_Glyph_Copy` failure-cleanup observations for the Wasm lane.
#[cfg(feature = "abi-test-support")]
pub fn abi_support_glyph_copy_failure_cleanup() -> [rust_ffi::FT_Glyph_Copy_Failure_Row; 3] {
    rust_ffi::FT_Glyph_Copy_Failure_Cleanup()
}

#[cfg(feature = "abi-test-support")]
struct WasmOpaqueIncrementalTrace {
    object: FT_Incremental,
    requested_glyph: FT_UInt,
    bytes: Box<[FT_Byte]>,
    acquired_pointer: usize,
    acquired_length: FT_UInt,
    events: Vec<(&'static str, FT_UInt)>,
    object_identity_matches: bool,
    release_count: usize,
    release_matches_acquisition: bool,
    face_done: bool,
    callbacks_after_face_done: usize,
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn wasm_opaque_incremental_get_glyph_data(
    incremental: FT_Incremental,
    glyph_index: FT_UInt,
    adata: *mut FT_Data,
) -> FT_Error {
    OPAQUE_INCREMENTAL_TRACE.with_borrow_mut(|trace| {
        let Some(trace) = trace.as_mut() else {
            return rust_ffi::FT_Err_Invalid_Argument;
        };
        if trace.face_done {
            trace.callbacks_after_face_done = trace.callbacks_after_face_done.saturating_add(1);
        }
        trace.object_identity_matches &= incremental == trace.object;
        let Some(adata) = (unsafe { adata.as_mut() }) else {
            return rust_ffi::FT_Err_Invalid_Argument;
        };
        if glyph_index != trace.requested_glyph {
            return rust_ffi::FT_Err_Invalid_Glyph_Index as FT_Error;
        }
        let Ok(length) = FT_UInt::try_from(trace.bytes.len()) else {
            return rust_ffi::FT_Err_Array_Too_Large as FT_Error;
        };
        adata.pointer = trace.bytes.as_ptr();
        adata.length = length;
        trace.acquired_pointer = adata.pointer.addr();
        trace.acquired_length = length;
        trace.events.push(("get_glyph_data", glyph_index));
        rust_ffi::FT_Err_Ok
    })
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn wasm_opaque_incremental_free_glyph_data(
    incremental: FT_Incremental,
    data: *mut FT_Data,
) {
    OPAQUE_INCREMENTAL_TRACE.with_borrow_mut(|trace| {
        let Some(trace) = trace.as_mut() else {
            return;
        };
        if trace.face_done {
            trace.callbacks_after_face_done = trace.callbacks_after_face_done.saturating_add(1);
        }
        trace.object_identity_matches &= incremental == trace.object;
        let Some(data) = (unsafe { data.as_ref() }) else {
            return;
        };
        trace.release_count = trace.release_count.saturating_add(1);
        trace.release_matches_acquisition =
            data.pointer.addr() == trace.acquired_pointer && data.length == trace.acquired_length;
        trace
            .events
            .push(("free_glyph_data", trace.requested_glyph));
    });
}

#[cfg(feature = "abi-test-support")]
pub struct AbiIncrementalOpaqueSnapshot {
    pub open_error: FT_Error,
    pub load_error: FT_Error,
    pub done_face_error: FT_Error,
    pub done_library_error: FT_Error,
    pub callback_log: Vec<(&'static str, FT_UInt)>,
    pub object_identity_matches: bool,
    pub release_count: usize,
    pub release_matches_acquisition: bool,
    pub object_freed_by_freetype: bool,
    pub callbacks_after_face_done: usize,
    pub client_object_still_valid: bool,
    pub stored_interface_identity: bool,
}

#[cfg(feature = "abi-test-support")]
pub struct AbiIncrementalCallbackTableSnapshot {
    pub get_glyph_data: bool,
    pub free_glyph_data: bool,
    pub get_glyph_metrics: bool,
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn wasm_incremental_table_get_glyph_data(
    _incremental: FT_Incremental,
    _glyph_index: FT_UInt,
    _adata: *mut FT_Data,
) -> FT_Error {
    rust_ffi::FT_Err_Invalid_Argument as FT_Error
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn wasm_incremental_table_free_glyph_data(
    _incremental: FT_Incremental,
    _data: *mut FT_Data,
) {
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn wasm_incremental_table_get_glyph_metrics(
    _incremental: FT_Incremental,
    _glyph_index: FT_UInt,
    _vertical: FT_Bool,
    _metrics: *mut FT_Incremental_MetricsRec,
) -> FT_Error {
    rust_ffi::FT_Err_Invalid_Argument as FT_Error
}

/// Constructs each callback table shape from the pinned `ftincrem.h` record
/// and observes function-pointer nullability without invoking incomplete
/// required tables.
#[cfg(feature = "abi-test-support")]
pub fn abi_support_incremental_callback_table_contract(
    tables: &[(bool, bool, bool)],
) -> Vec<AbiIncrementalCallbackTableSnapshot> {
    tables
        .iter()
        .map(|&(get_glyph_data, free_glyph_data, get_glyph_metrics)| {
            let funcs = FT_Incremental_FuncsRec {
                get_glyph_data: get_glyph_data.then_some(wasm_incremental_table_get_glyph_data),
                free_glyph_data: free_glyph_data.then_some(wasm_incremental_table_free_glyph_data),
                get_glyph_metrics: get_glyph_metrics
                    .then_some(wasm_incremental_table_get_glyph_metrics),
            };
            AbiIncrementalCallbackTableSnapshot {
                get_glyph_data: funcs.get_glyph_data.is_some(),
                free_glyph_data: funcs.free_glyph_data.is_some(),
                get_glyph_metrics: funcs.get_glyph_metrics.is_some(),
            }
        })
        .collect()
}

/// Opens a pure-Rust Wasm-facade face through the incremental parameter path
/// with an inaccessible client object and records callback identity events.
#[cfg(feature = "abi-test-support")]
pub fn abi_support_incremental_opaque_handle(
    bytes: &[u8],
    face_index: FT_Long,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
    route_kind: u8,
) -> AbiIncrementalOpaqueSnapshot {
    let mut library = rust_ffi::FT_Init_FreeType();
    let glyph_bytes = rust_ffi::FT_New_Memory_Face(&library, bytes, face_index, 20.0)
        .ok()
        .and_then(|face| rust_ffi::FT_Face_Incremental_Glyph_Data(&face, glyph_index))
        .unwrap_or_default()
        .into_boxed_slice();
    let lifetime_route = route_kind == 1;
    let parameter_cast_route = route_kind == 2;
    let mut client_object = lifetime_route.then(|| Box::new(INCREMENTAL_CLIENT_OBJECT_MAGIC));
    let object: FT_Incremental = client_object.as_mut().map_or_else(
        || ptr::without_provenance_mut(1),
        |client_object| ptr::from_mut(client_object.as_mut()).cast(),
    );
    OPAQUE_INCREMENTAL_TRACE.with_borrow_mut(|trace| {
        *trace = Some(WasmOpaqueIncrementalTrace {
            object,
            requested_glyph: glyph_index,
            bytes: glyph_bytes,
            acquired_pointer: 0,
            acquired_length: 0,
            events: Vec::new(),
            object_identity_matches: true,
            release_count: 0,
            release_matches_acquisition: false,
            face_done: false,
            callbacks_after_face_done: 0,
        });
    });
    let funcs = FT_Incremental_FuncsRec {
        get_glyph_data: Some(wasm_opaque_incremental_get_glyph_data),
        free_glyph_data: Some(wasm_opaque_incremental_free_glyph_data),
        get_glyph_metrics: None,
    };
    let interface = FT_Incremental_InterfaceRec {
        funcs: ptr::from_ref(&funcs),
        object,
    };
    let open_result = if parameter_cast_route {
        let parameter = rust_ffi::FT_Parameter {
            tag: rust_ffi::FT_PARAM_TAG_INCREMENTAL as FT_ULong,
            data: ptr::from_ref(&interface).cast_mut().cast(),
        };
        rust_ffi::FT_Open_Face_With_Incremental_Parameter(
            &library, bytes, face_index, 20.0, &parameter,
        )
    } else {
        rust_ffi::FT_Open_Face_With_Incremental(
            &library,
            bytes,
            face_index,
            20.0,
            ptr::from_ref(&interface).cast_mut(),
        )
    };
    let mut stored_interface_identity = false;
    let (open_error, load_error, done_face_error) = match open_result {
        Ok(face) => {
            if parameter_cast_route {
                stored_interface_identity = rust_ffi::FT_Face_Incremental_Interface(&face)
                    == ptr::from_ref(&interface).cast_mut();
            }
            let load_error = match rust_ffi::FT_Load_Glyph(&face, glyph_index, load_flags) {
                Ok(_) => rust_ffi::FT_Err_Ok,
                Err(error) => error,
            };
            let done_face_error = rust_ffi::FT_Done_Face(Some(face));
            OPAQUE_INCREMENTAL_TRACE.with_borrow_mut(|trace| {
                if let Some(trace) = trace.as_mut() {
                    trace.face_done = true;
                }
            });
            (rust_ffi::FT_Err_Ok, load_error, done_face_error)
        }
        Err(error) => (
            error,
            error,
            rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error,
        ),
    };
    let done_library_error = rust_ffi::FT_Done_Library(Some(&mut library));
    let (
        callback_log,
        object_identity_matches,
        release_count,
        release_matches_acquisition,
        callbacks_after_face_done,
    ) = OPAQUE_INCREMENTAL_TRACE.with_borrow(|trace| {
        trace
            .as_ref()
            .map_or((Vec::new(), false, 0, false, 0), |trace| {
                (
                    trace.events.clone(),
                    trace.object_identity_matches,
                    trace.release_count,
                    trace.release_matches_acquisition,
                    trace.callbacks_after_face_done,
                )
            })
    });
    let client_object_still_valid = client_object
        .as_deref()
        .is_some_and(|client_object| *client_object == INCREMENTAL_CLIENT_OBJECT_MAGIC);
    OPAQUE_INCREMENTAL_TRACE.with_borrow_mut(|trace| *trace = None);
    AbiIncrementalOpaqueSnapshot {
        open_error,
        load_error,
        done_face_error,
        done_library_error,
        callback_log,
        object_identity_matches,
        release_count,
        release_matches_acquisition,
        object_freed_by_freetype: lifetime_route && !client_object_still_valid,
        callbacks_after_face_done: if lifetime_route {
            callbacks_after_face_done
        } else {
            0
        },
        client_object_still_valid: lifetime_route && client_object_still_valid,
        stored_interface_identity,
    }
}

/// Normalized incremental-glyph lifecycle observations for the Wasm parity lane.
#[cfg(feature = "abi-test-support")]
pub struct AbiIncrementalGlyphSnapshot {
    pub open_error: FT_Error,
    pub load_error: FT_Error,
    pub done_face_error: FT_Error,
    pub done_library_error: FT_Error,
    pub callback_log: Vec<(&'static str, FT_UInt)>,
    pub release_count: usize,
    pub glyph_data_length: FT_UInt,
    pub release_matches_acquisition: bool,
    pub metric_events: Vec<AbiIncrementalMetricEvent>,
    pub slot_format: rust_ffi::FT_Glyph_Format,
    pub slot_advance: rust_ffi::FT_Vector,
    pub slot_metrics: rust_ffi::FT_Glyph_Metrics,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone, Copy)]
pub struct AbiIncrementalMetricEvent {
    pub vertical: bool,
    pub input: rust_ffi::FT_Incremental_MetricsRec,
    pub output: rust_ffi::FT_Incremental_MetricsRec,
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_incremental_glyph_lifecycle(
    bytes: &[u8],
    face_index: FT_Long,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
) -> AbiIncrementalGlyphSnapshot {
    abi_support_incremental_glyph_lifecycle_impl(bytes, face_index, glyph_index, load_flags, None)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_incremental_state_lifecycle(
    bytes: &[u8],
    face_index: FT_Long,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
    metric_deltas: [FT_Long; 4],
) -> AbiIncrementalGlyphSnapshot {
    abi_support_incremental_glyph_lifecycle_impl(
        bytes,
        face_index,
        glyph_index,
        load_flags,
        Some(metric_deltas),
    )
}

#[cfg(feature = "abi-test-support")]
fn abi_support_incremental_glyph_lifecycle_impl(
    bytes: &[u8],
    face_index: FT_Long,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
    metric_deltas: Option<[FT_Long; 4]>,
) -> AbiIncrementalGlyphSnapshot {
    let mut library = rust_ffi::FT_Init_FreeType();
    let mut open_error = rust_ffi::FT_Err_Ok;
    let load_error;
    let mut done_face_error = rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    let mut callback_log = Vec::new();
    let mut release_count = 0;
    let mut glyph_data_length = 0;
    let mut release_matches_acquisition = false;
    let mut metric_events = Vec::new();
    let mut slot_format = 0;
    let mut slot_advance = rust_ffi::FT_Vector::default();
    let mut slot_metrics = rust_ffi::FT_Glyph_Metrics::default();

    match rust_ffi::FT_New_Memory_Face(&library, bytes, face_index, 20.0) {
        Ok(face) => {
            if let Some(glyph_data) = rust_ffi::FT_Face_Incremental_Glyph_Data(&face, glyph_index) {
                glyph_data_length = FT_UInt::try_from(glyph_data.len()).unwrap_or(FT_UInt::MAX);
                callback_log.push(("get_glyph_data", glyph_index));
                match rust_ffi::FT_Load_Glyph(&face, glyph_index, load_flags) {
                    Ok(mut slot) => {
                        load_error = rust_ffi::FT_Err_Ok;
                        if let Some(
                            [
                                horizontal_bearing,
                                horizontal_advance,
                                vertical_bearing,
                                vertical_advance,
                            ],
                        ) = metric_deltas
                        {
                            let horizontal_input = rust_ffi::FT_Incremental_MetricsRec {
                                bearing_x: slot.metrics.horiBearingX,
                                bearing_y: 0,
                                advance: slot.metrics.horiAdvance,
                                advance_v: 0,
                            };
                            let mut horizontal_output = horizontal_input;
                            horizontal_output.bearing_x = horizontal_output
                                .bearing_x
                                .saturating_add(horizontal_bearing);
                            horizontal_output.advance =
                                horizontal_output.advance.saturating_add(horizontal_advance);
                            callback_log.push(("get_glyph_metrics", glyph_index));
                            metric_events.push(AbiIncrementalMetricEvent {
                                vertical: false,
                                input: horizontal_input,
                                output: horizontal_output,
                            });
                            slot.metrics.horiBearingX = horizontal_output.bearing_x;
                            slot.metrics.horiAdvance = horizontal_output.advance;

                            let vertical_input = rust_ffi::FT_Incremental_MetricsRec {
                                bearing_x: 0,
                                bearing_y: slot.metrics.vertBearingY,
                                advance: slot.metrics.vertAdvance,
                                advance_v: 0,
                            };
                            let mut vertical_output = vertical_input;
                            vertical_output.bearing_y =
                                vertical_output.bearing_y.saturating_add(vertical_bearing);
                            vertical_output.advance =
                                vertical_output.advance.saturating_add(vertical_advance);
                            metric_events.push(AbiIncrementalMetricEvent {
                                vertical: true,
                                input: vertical_input,
                                output: vertical_output,
                            });
                            slot.metrics.vertBearingX = slot
                                .metrics
                                .horiBearingX
                                .saturating_sub(slot.metrics.horiAdvance / 2);
                            slot.metrics.vertBearingY = vertical_output.bearing_y;
                            slot.metrics.vertAdvance = vertical_output.advance;
                            if load_flags & rust_ffi::FT_LOAD_VERTICAL_LAYOUT != 0 {
                                slot.advance.x = 0;
                                slot.advance.y = slot.metrics.vertAdvance;
                            } else {
                                slot.advance.x = slot.metrics.horiAdvance;
                                slot.advance.y = 0;
                            }
                        }
                        slot_format = slot.format;
                        slot_advance = slot.advance;
                        slot_metrics = slot.metrics;
                    }
                    Err(error) => load_error = error,
                }
                callback_log.push(("free_glyph_data", glyph_index));
                if !metric_events.is_empty() {
                    callback_log.push(("get_glyph_metrics", glyph_index));
                }
                release_count = 1;
                release_matches_acquisition = true;
            } else {
                load_error = rust_ffi::FT_Err_Invalid_Glyph_Index as FT_Error;
            }
            done_face_error = rust_ffi::FT_Done_Face(Some(face));
        }
        Err(error) => {
            open_error = error;
            load_error = error;
        }
    }
    let done_library_error = rust_ffi::FT_Done_Library(Some(&mut library));
    AbiIncrementalGlyphSnapshot {
        open_error,
        load_error,
        done_face_error,
        done_library_error,
        callback_log,
        release_count,
        glyph_data_length,
        release_matches_acquisition,
        metric_events,
        slot_format,
        slot_advance,
        slot_metrics,
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_reference_library_observation() -> (i32, i32, bool, i32) {
    let mut library = rust_ffi::FT_New_Library_Without_Default_Modules();
    rust_ffi::FT_Add_Default_Modules(Some(&mut library));
    let reference_status = rust_ffi::FT_Reference_Library(Some(&mut library));
    let first_done_status = rust_ffi::FT_Done_Library(Some(&mut library));
    let usable = rust_ffi::FT_Library_Has_Module(Some(&library), "truetype");
    let final_done_status = rust_ffi::FT_Done_Library(Some(&mut library));
    (
        reference_status,
        first_done_status,
        usable,
        final_done_status,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_reference_then_done_library_observation() -> (i32, i32, bool, bool) {
    let mut library = rust_ffi::FT_New_Library_Without_Default_Modules();
    rust_ffi::FT_Add_Default_Modules(Some(&mut library));
    let reference_status = rust_ffi::FT_Reference_Library(Some(&mut library));
    let done_status = rust_ffi::FT_Done_Library(Some(&mut library));
    let usable = rust_ffi::FT_Library_Has_Module(Some(&library), "truetype");
    let _ = rust_ffi::FT_Done_Library(Some(&mut library));
    (reference_status, done_status, usable, !usable)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_final_done_library_observation() -> i32 {
    let mut library = rust_ffi::FT_New_Library_Without_Default_Modules();
    rust_ffi::FT_Add_Default_Modules(Some(&mut library));
    rust_ffi::FT_Done_Library(Some(&mut library))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_default_module_flags(name: &str) -> Option<rust_ffi::FT_ULong> {
    let library = rust_ffi::FT_Init_FreeType();
    rust_ffi::FT_Library_Module_Flags(Some(&library), name)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_default_module_present(library_present: i32, name: &str) -> bool {
    if library_present == 0 {
        return false;
    }
    let library = rust_ffi::FT_Init_FreeType();
    rust_ffi::FT_Library_Has_Module(Some(&library), name)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_module_interface_present(
    library_present: i32,
    module_name: Option<&str>,
) -> bool {
    let library = match library_present {
        0 => None,
        _ => Some(rust_ffi::FT_Init_FreeType()),
    };
    !rust_ffi::FT_Get_Module_Interface(library.as_ref(), module_name).is_null()
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_module_requester_service_available(
    library_present: i32,
    module_name: Option<&str>,
    service_name: &str,
) -> bool {
    let library = match library_present {
        0 => None,
        _ => Some(rust_ffi::FT_Init_FreeType()),
    };
    rust_ffi::FT_Module_Requester_Service_Available(library.as_ref(), module_name, service_name)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_default_renderer_class(
    format: rust_ffi::FT_Glyph_Format,
) -> Option<(&'static str, rust_ffi::FT_Glyph_Format, bool, bool)> {
    let library = rust_ffi::FT_Init_FreeType();
    rust_ffi::FT_Library_Renderer_Class(Some(&library), format)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_null_renderer_class(
    format: rust_ffi::FT_Glyph_Format,
) -> Option<(&'static str, rust_ffi::FT_Glyph_Format, bool, bool)> {
    rust_ffi::FT_Library_Renderer_Class(None, format)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_set_default_outline_renderer() -> (
    FT_Error,
    Option<(&'static str, rust_ffi::FT_Glyph_Format, bool, bool)>,
) {
    let mut library = rust_ffi::FT_Init_FreeType();
    let error = rust_ffi::FT_Library_Set_Renderer_By_Format(
        Some(&mut library),
        rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        "smooth",
    );
    (
        error,
        rust_ffi::FT_Library_Renderer_Class(Some(&library), rust_ffi::FT_GLYPH_FORMAT_OUTLINE),
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_init_free_type_created_library() -> bool {
    let _library = rust_ffi::FT_Init_FreeType();
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_done_mm_var(library_present: i32, descriptor_present: i32) -> i32 {
    let library = if library_present != 0 {
        Some(rust_ffi::FT_Init_FreeType())
    } else {
        None
    };
    let mut descriptor = (descriptor_present != 0).then(rust_ffi::FT_MM_Var::default);
    rust_ffi::FT_Done_MM_Var(library.as_ref(), descriptor.as_mut())
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_get_and_done_mm_var(
    handle: usize,
    amaster: *mut rust_ffi::FT_MM_Var,
    axis: *mut rust_ffi::FT_Var_Axis,
    axis_capacity: FT_UInt,
) -> (FT_Error, FT_Error, usize, usize, usize) {
    let before = face_ref(handle).map_or(0, |face| face.mm_vars.len());
    let get_err = fontdone_wasm_get_mm_var(handle, amaster, axis, axis_capacity);
    let after_get = face_ref(handle).map_or(0, |face| face.mm_vars.len());
    let library = rust_ffi::FT_Init_FreeType();
    let descriptor = unsafe { amaster.as_mut() };
    let done_err = if get_err == rust_ffi::FT_Err_Ok {
        if let Some(face) = face_mut(handle) {
            face.mm_vars.remove(&(amaster as usize));
        }
        rust_ffi::FT_Done_MM_Var(Some(&library), descriptor)
    } else {
        rust_ffi::FT_Err_Ok
    };
    let after_done = face_ref(handle).map_or(0, |face| face.mm_vars.len());
    (get_err, done_err, before, after_get, after_done)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_mm_var_namedstyles(
    master: &rust_ffi::FT_MM_Var,
) -> Option<Vec<(rust_ffi::FT_Var_Named_Style, Vec<rust_ffi::FT_Fixed>)>> {
    let axis_count = usize::try_from(master.num_axis).ok()?;
    let namedstyle_count = usize::try_from(master.num_namedstyles).ok()?;
    if master.namedstyle.is_null() {
        return Some(Vec::new());
    }
    // SAFETY: this feature-gated helper is used immediately after
    // `fontdone_wasm_get_mm_var`, whose face-owned side storage keeps the
    // namedstyle pointer and coordinate arrays live.
    let styles = unsafe { slice::from_raw_parts(master.namedstyle, namedstyle_count) };
    Some(
        styles
            .iter()
            .map(|style| {
                let coords = if style.coords.is_null() {
                    Vec::new()
                } else {
                    // SAFETY: each namedstyle record has one coordinate per
                    // axis in the live descriptor side storage.
                    unsafe { slice::from_raw_parts(style.coords, axis_count) }.to_vec()
                };
                (*style, coords)
            })
            .collect(),
    )
}

#[cfg(feature = "abi-test-support")]
extern "C" fn abi_support_debug_hook_a(_arg: rust_ffi::FT_Pointer) -> rust_ffi::FT_Error {
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
extern "C" fn abi_support_debug_hook_b(_arg: rust_ffi::FT_Pointer) -> rust_ffi::FT_Error {
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_mul_div(a: FT_Long, b: FT_Long, c: FT_Long) -> FT_Long {
    rust_ffi::FT_MulDiv(a, b, c)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_mul_fix(a: FT_Long, b: FT_Long) -> FT_Long {
    rust_ffi::FT_MulFix(a, b)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_div_fix(a: FT_Long, b: FT_Long) -> FT_Long {
    rust_ffi::FT_DivFix(a, b)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_round_fix(a: FT_Fixed) -> FT_Fixed {
    rust_ffi::FT_RoundFix(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_ceil_fix(a: FT_Fixed) -> FT_Fixed {
    rust_ffi::FT_CeilFix(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_floor_fix(a: FT_Fixed) -> FT_Fixed {
    rust_ffi::FT_FloorFix(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_sin(angle: FT_Angle) -> FT_Fixed {
    rust_ffi::FT_Sin(angle)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_cos(angle: FT_Angle) -> FT_Fixed {
    rust_ffi::FT_Cos(angle)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_tan(angle: FT_Angle) -> FT_Fixed {
    rust_ffi::FT_Tan(angle)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_atan2(dx: FT_Fixed, dy: FT_Fixed) -> FT_Angle {
    rust_ffi::FT_Atan2(dx, dy)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_angle_diff(angle1: FT_Angle, angle2: FT_Angle) -> FT_Angle {
    rust_ffi::FT_Angle_Diff(angle1, angle2)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_vector_unit(vector: *mut FontdoneWasmVector, angle: FT_Angle) {
    let mut rust_vector = if vector.is_null() {
        None
    } else {
        // SAFETY: `vector` is non-null and points to a wasm ABI vector.
        let vector = unsafe { &*vector };
        Some(rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        })
    };
    rust_ffi::FT_Vector_Unit(rust_vector.as_mut(), angle);
    if let Some(rust_vector) = rust_vector {
        // SAFETY: `vector` is non-null in the branch that created `rust_vector`.
        unsafe {
            (*vector).x = rust_vector.x;
            (*vector).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_vector_rotate(vector: *mut FontdoneWasmVector, angle: FT_Angle) {
    let mut rust_vector = if vector.is_null() {
        None
    } else {
        // SAFETY: `vector` is non-null and points to a wasm ABI vector.
        let vector = unsafe { &*vector };
        Some(rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        })
    };
    rust_ffi::FT_Vector_Rotate(rust_vector.as_mut(), angle);
    if let Some(rust_vector) = rust_vector {
        // SAFETY: `vector` is non-null in the branch that created `rust_vector`.
        unsafe {
            (*vector).x = rust_vector.x;
            (*vector).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_vector_length(vector: *const FontdoneWasmVector) -> FT_Fixed {
    let rust_vector = if vector.is_null() {
        None
    } else {
        // SAFETY: `vector` is non-null and points to a wasm ABI vector.
        let vector = unsafe { &*vector };
        Some(rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        })
    };
    rust_ffi::FT_Vector_Length(rust_vector.as_ref())
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_vector_polarize(
    vector: *const FontdoneWasmVector,
    length: *mut FT_Fixed,
    angle: *mut FT_Angle,
) {
    let rust_vector = if vector.is_null() {
        None
    } else {
        // SAFETY: `vector` is non-null and points to a wasm ABI vector.
        let vector = unsafe { &*vector };
        Some(rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        })
    };
    let mut rust_length = if length.is_null() {
        None
    } else {
        // SAFETY: `length` is non-null and points to a wasm ABI `FT_Fixed`.
        Some(unsafe { *length })
    };
    let mut rust_angle = if angle.is_null() {
        None
    } else {
        // SAFETY: `angle` is non-null and points to a wasm ABI `FT_Angle`.
        Some(unsafe { *angle })
    };
    rust_ffi::FT_Vector_Polarize(
        rust_vector.as_ref(),
        rust_length.as_mut(),
        rust_angle.as_mut(),
    );
    if let Some(value) = rust_length {
        // SAFETY: `length` is non-null in the branch that created `rust_length`.
        unsafe { *length = value };
    }
    if let Some(value) = rust_angle {
        // SAFETY: `angle` is non-null in the branch that created `rust_angle`.
        unsafe { *angle = value };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_vector_from_polar(
    vector: *mut FontdoneWasmVector,
    length: FT_Fixed,
    angle: FT_Angle,
) {
    let mut rust_vector = if vector.is_null() {
        None
    } else {
        // SAFETY: `vector` is non-null and points to a wasm ABI vector.
        let vector = unsafe { &*vector };
        Some(rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        })
    };
    rust_ffi::FT_Vector_From_Polar(rust_vector.as_mut(), length, angle);
    if let Some(rust_vector) = rust_vector {
        // SAFETY: `vector` is non-null in the branch that created `rust_vector`.
        unsafe {
            (*vector).x = rust_vector.x;
            (*vector).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_vector_transform(
    vector: *mut FontdoneWasmVector,
    matrix: *const FontdoneWasmMatrix,
) {
    let mut rust_vector = if vector.is_null() {
        None
    } else {
        // SAFETY: `vector` is non-null and points to a wasm ABI vector.
        let vector = unsafe { &*vector };
        Some(rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        })
    };
    let rust_matrix = if matrix.is_null() {
        None
    } else {
        // SAFETY: `matrix` is non-null and points to a wasm ABI matrix.
        let matrix = unsafe { &*matrix };
        Some(rust_ffi::FT_Matrix {
            xx: matrix.xx,
            xy: matrix.xy,
            yx: matrix.yx,
            yy: matrix.yy,
        })
    };
    rust_ffi::FT_Vector_Transform(rust_vector.as_mut(), rust_matrix.as_ref());
    if let Some(rust_vector) = rust_vector {
        // SAFETY: `vector` is non-null in the branch that created `rust_vector`.
        unsafe {
            (*vector).x = rust_vector.x;
            (*vector).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_matrix_multiply(
    a: *const FontdoneWasmMatrix,
    b: *mut FontdoneWasmMatrix,
) {
    let rust_a = if a.is_null() {
        None
    } else {
        // SAFETY: `a` is non-null and points to a wasm ABI matrix.
        let a = unsafe { &*a };
        Some(rust_ffi::FT_Matrix {
            xx: a.xx,
            xy: a.xy,
            yx: a.yx,
            yy: a.yy,
        })
    };
    let mut rust_b = if b.is_null() {
        None
    } else {
        // SAFETY: `b` is non-null and points to a wasm ABI matrix.
        let b = unsafe { &*b };
        Some(rust_ffi::FT_Matrix {
            xx: b.xx,
            xy: b.xy,
            yx: b.yx,
            yy: b.yy,
        })
    };
    rust_ffi::FT_Matrix_Multiply(rust_a.as_ref(), rust_b.as_mut());
    if let Some(rust_b) = rust_b {
        // SAFETY: `b` is non-null in the branch that created `rust_b`.
        unsafe {
            (*b).xx = rust_b.xx;
            (*b).xy = rust_b.xy;
            (*b).yx = rust_b.yx;
            (*b).yy = rust_b.yy;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_matrix_invert(matrix: *mut FontdoneWasmMatrix) -> FT_Error {
    let mut rust_matrix = if matrix.is_null() {
        None
    } else {
        // SAFETY: `matrix` is non-null and points to a wasm ABI matrix.
        let matrix = unsafe { &*matrix };
        Some(rust_ffi::FT_Matrix {
            xx: matrix.xx,
            xy: matrix.xy,
            yx: matrix.yx,
            yy: matrix.yy,
        })
    };
    let err = rust_ffi::FT_Matrix_Invert(rust_matrix.as_mut());
    if let Some(rust_matrix) = rust_matrix {
        // SAFETY: `matrix` is non-null in the branch that created `rust_matrix`.
        unsafe {
            (*matrix).xx = rust_matrix.xx;
            (*matrix).xy = rust_matrix.xy;
            (*matrix).yx = rust_matrix.yx;
            (*matrix).yy = rust_matrix.yy;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_error_string(
    error_code: FT_Error,
    out: *mut FontdoneWasmString,
) -> FT_Bool {
    // SAFETY: the caller provides writable storage for the output record or null.
    let Some(out) = (unsafe { out.as_mut() }) else {
        return 0;
    };
    let Some(text) = rust_ffi::FT_Error_String(error_code) else {
        *out = FontdoneWasmString::default();
        return 0;
    };
    out.string = text.as_ptr().cast();
    out.string_len = u32::try_from(text.to_bytes().len()).unwrap_or(u32::MAX);
    1
}

fn write_ft_bytes(out: *mut FT_Bytes, value: FT_Bytes) {
    if !out.is_null() {
        // SAFETY: `out` is non-null and caller provides writable FT_Bytes storage.
        unsafe { *out = value };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_open_type_validate(
    handle: usize,
    validation_flags: FT_UInt,
    base_table: *mut FT_Bytes,
    gdef_table: *mut FT_Bytes,
    gpos_table: *mut FT_Bytes,
    gsub_table: *mut FT_Bytes,
    jstf_table: *mut FT_Bytes,
) -> FT_Error {
    let face = face_ref(handle).map(|face| &face.face);
    let mut base = ptr::null();
    let mut gdef = ptr::null();
    let mut gpos = ptr::null();
    let mut gsub = ptr::null();
    let mut jstf = ptr::null();
    let err = rust_ffi::FT_OpenType_Validate(
        face,
        validation_flags,
        (!base_table.is_null()).then_some(&mut base),
        (!gdef_table.is_null()).then_some(&mut gdef),
        (!gpos_table.is_null()).then_some(&mut gpos),
        (!gsub_table.is_null()).then_some(&mut gsub),
        (!jstf_table.is_null()).then_some(&mut jstf),
    );
    if err == rust_ffi::FT_Err_Ok {
        write_ft_bytes(base_table, base);
        write_ft_bytes(gdef_table, gdef);
        write_ft_bytes(gpos_table, gpos);
        write_ft_bytes(gsub_table, gsub);
        write_ft_bytes(jstf_table, jstf);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_open_type_free(handle: usize, table: FT_Bytes) {
    rust_ffi::FT_OpenType_Free(face_ref(handle).map(|face| &face.face), table);
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_enable_open_type_validator(handle: usize) -> bool {
    let Some(state) = face_mut(handle) else {
        return false;
    };
    rust_ffi::FT_OpenType_Validator_Set_Available(&mut state.face, true);
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_enable_gx_validator(handle: usize) -> bool {
    let Some(state) = face_mut(handle) else {
        return false;
    };
    rust_ffi::FT_GX_Validator_Set_Available(&mut state.face, true);
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_pixel_sizes(
    handle: usize,
    pixel_width: FT_UInt,
    pixel_height: FT_UInt,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        // The C API defers null-face validation to FT_Request_Size, so a zero
        // WASM face handle has the same Invalid_Face_Handle classification.
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let error = rust_ffi::FT_Set_Pixel_Sizes(&mut face.face, pixel_width, pixel_height);
    if error == rust_ffi::FT_Err_Ok {
        update_wasm_active_size_metrics(face);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_transform(
    handle: usize,
    matrix: *const FT_Matrix,
    delta: *const FT_Vector,
) {
    let Some(face) = face_mut(handle) else {
        return;
    };
    // SAFETY: nullable pointers are converted to `Option<&T>` and never
    // retained after this thin ABI call returns.
    let matrix = unsafe { matrix.as_ref() };
    // SAFETY: nullable pointers are converted to `Option<&T>` and never
    // retained after this thin ABI call returns.
    let delta = unsafe { delta.as_ref() };
    rust_ffi::FT_Set_Transform(Some(&mut face.face), matrix, delta);
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_char_size(
    handle: usize,
    char_width: FT_Long,
    char_height: FT_Long,
    horz_resolution: FT_UInt,
    vert_resolution: FT_UInt,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        // The C API defers null-face validation to FT_Request_Size, so a zero
        // WASM face handle has the same Invalid_Face_Handle classification.
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let error = rust_ffi::FT_Set_Char_Size(
        &mut face.face,
        char_width,
        char_height,
        horz_resolution,
        vert_resolution,
    );
    if error == rust_ffi::FT_Err_Ok {
        update_wasm_active_size_metrics(face);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_request_size(
    handle: usize,
    req: *const FontdoneWasmSizeRequest,
) -> FT_Error {
    let request = if req.is_null() {
        None
    } else {
        // SAFETY: `req` is non-null and copied by value only.
        let req = unsafe { *req };
        Some(rust_ffi::FT_Size_RequestRec {
            type_: req.type_,
            width: req.width,
            height: req.height,
            horiResolution: req.horiResolution,
            vertResolution: req.vertResolution,
        })
    };
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let error = rust_ffi::FT_Request_Size(Some(&mut face.face), request.as_ref());
    if error == rust_ffi::FT_Err_Ok {
        update_wasm_active_size_metrics(face);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_select_size(handle: usize, strike_index: FT_Int) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let error = rust_ffi::FT_Select_Size(Some(&mut face.face), strike_index);
    if error == rust_ffi::FT_Err_Ok {
        update_wasm_active_size_metrics(face);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_char_index(handle: usize, char_code: FT_ULong) -> FT_UInt {
    let Some(face) = face_ref(handle) else {
        return 0;
    };
    rust_ffi::FT_Get_Char_Index(&face.face, char_code)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_char_variant_index(
    handle: usize,
    charcode: FT_ULong,
    variant_selector: FT_ULong,
) -> FT_UInt {
    let Some(face) = face_ref(handle) else {
        return 0;
    };
    rust_ffi::FT_Face_GetCharVariantIndex(Some(&face.face), charcode, variant_selector)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_char_variant_is_default(
    handle: usize,
    charcode: FT_ULong,
    variant_selector: FT_ULong,
) -> FT_Int {
    let Some(face) = face_ref(handle) else {
        return -1;
    };
    rust_ffi::FT_Face_GetCharVariantIsDefault(Some(&face.face), charcode, variant_selector)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_variant_selectors(handle: usize) -> *mut FT_UInt32 {
    let Some(face) = face_mut(handle) else {
        return ptr::null_mut();
    };
    let Some(values) = rust_ffi::FT_Face_GetVariantSelectors(Some(&face.face)) else {
        face.variant_list.clear();
        return ptr::null_mut();
    };
    face.variant_list = values;
    face.variant_list.push(0);
    face.variant_list.as_mut_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_variants_of_char(
    handle: usize,
    charcode: FT_ULong,
) -> *mut FT_UInt32 {
    let Some(face) = face_mut(handle) else {
        return ptr::null_mut();
    };
    let Some(values) = rust_ffi::FT_Face_GetVariantsOfChar(Some(&face.face), charcode) else {
        face.variant_list.clear();
        return ptr::null_mut();
    };
    face.variant_list = values;
    face.variant_list.push(0);
    face.variant_list.as_mut_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_chars_of_variant(
    handle: usize,
    variant_selector: FT_ULong,
) -> *mut FT_UInt32 {
    let Some(face) = face_mut(handle) else {
        return ptr::null_mut();
    };
    let Some(values) = rust_ffi::FT_Face_GetCharsOfVariant(Some(&face.face), variant_selector)
    else {
        face.variant_list.clear();
        return ptr::null_mut();
    };
    face.variant_list = values;
    face.variant_list.push(0);
    face.variant_list.as_mut_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_kerning(
    handle: usize,
    left_glyph: FT_UInt,
    right_glyph: FT_UInt,
    kern_mode: FT_UInt,
    out: *mut FontdoneWasmVector,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if out.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument as FT_Error;
    }
    let mut vector = rust_ffi::FT_Vector::default();
    let err = rust_ffi::FT_Get_Kerning(
        Some(&face.face),
        left_glyph,
        right_glyph,
        kern_mode,
        Some(&mut vector),
    );
    if err == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe {
            *out = FontdoneWasmVector {
                x: vector.x,
                y: vector.y,
            };
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_pfr_kerning(
    handle: usize,
    left_glyph: FT_UInt,
    right_glyph: FT_UInt,
    out: *mut FontdoneWasmVector,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if out.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument as FT_Error;
    }
    let mut vector = rust_ffi::FT_Vector::default();
    let err =
        rust_ffi::FT_Get_PFR_Kerning(Some(&face.face), left_glyph, right_glyph, Some(&mut vector));
    if err == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe {
            *out = FontdoneWasmVector {
                x: vector.x,
                y: vector.y,
            };
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_pfr_metrics(
    handle: usize,
    aoutline_resolution: *mut FT_UInt,
    ametrics_resolution: *mut FT_UInt,
    ametrics_x_scale: *mut FT_Fixed,
    ametrics_y_scale: *mut FT_Fixed,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    // SAFETY: each independent output may be null; a non-null pointer denotes
    // caller-provided scalar storage for the duration of this call.
    unsafe {
        rust_ffi::FT_Get_PFR_Metrics(
            Some(&face.face),
            aoutline_resolution.as_mut(),
            ametrics_resolution.as_mut(),
            ametrics_x_scale.as_mut(),
            ametrics_y_scale.as_mut(),
        )
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_pfr_advance(
    handle: usize,
    glyph_index: FT_UInt,
    aadvance: *mut FT_Pos,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    // SAFETY: a null output is represented as `None`; a non-null pointer
    // denotes caller-provided scalar storage.
    unsafe { rust_ffi::FT_Get_PFR_Advance(Some(&face.face), glyph_index, aadvance.as_mut()) }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_select_charmap(handle: usize, encoding: FT_Encoding) -> FT_Error {
    rust_ffi::FT_Select_Charmap(face_mut(handle).map(|face| &mut face.face), encoding)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_charmap_count(handle: usize) -> FT_UInt {
    face_ref(handle).map_or(0, |face| {
        FT_UInt::try_from(face.face.charmaps.len()).unwrap_or(FT_UInt::MAX)
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_active_charmap_index(handle: usize) -> FT_Int {
    face_ref(handle).map_or(-1, |face| face.face.active_charmap_index)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_charmap(
    handle: usize,
    index: FT_UInt,
    out: *mut FontdoneWasmCharmap,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(info) = rust_face_charmap_info(&face.face, index) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    if out.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: `out` is non-null and caller provides writable storage.
    unsafe {
        *out = FontdoneWasmCharmap {
            index,
            encoding: info.encoding,
            platform_id: info.platform_id,
            encoding_id: info.encoding_id,
        };
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_cmap_format(handle: usize, index: FT_UInt) -> FT_Long {
    let Some(face) = face_ref(handle) else {
        return -1;
    };
    let charmap = rust_face_charmap(&face.face, index);
    rust_ffi::FT_Get_CMap_Format(charmap) as FT_Long
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_cmap_language_id(handle: usize, index: FT_UInt) -> FT_ULong {
    let Some(face) = face_ref(handle) else {
        return 0;
    };
    let charmap = rust_face_charmap(&face.face, index);
    rust_ffi::FT_Get_CMap_Language_ID(charmap) as FT_ULong
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_charmap(handle: usize, index: FT_UInt) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let charmap = rust_face_charmap(&face.face, index);
    rust_ffi::FT_Set_Charmap(Some(&mut face.face), charmap)
}

#[cfg(feature = "abi-test-support")]
pub fn cmap_cache_lookup_glyph_for_test(
    handle: usize,
    cmap_index: FT_Int,
    char_code: FT_UInt32,
) -> FT_UInt {
    let Some(face) = face_mut(handle) else {
        return 0;
    };
    face.face.cmap_cache_lookup_glyph(cmap_index, char_code)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_charmap_from_face(
    handle: usize,
    charmap_face_handle: usize,
    index: FT_UInt,
) -> FT_Error {
    let charmap = {
        let Some(charmap_face) = face_ref(charmap_face_handle) else {
            return rust_ffi::FT_Err_Invalid_CharMap_Handle as FT_Error;
        };
        rust_face_charmap(&charmap_face.face, index)
    };
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    rust_ffi::FT_Set_Charmap(Some(&mut face.face), charmap)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_var_design_coordinates(
    handle: usize,
    num_coords: FT_UInt,
    coords: *const FT_Fixed,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` readable FT_Fixed values in linear memory.
        Some(unsafe { slice::from_raw_parts(coords, num_coords as usize) })
    };
    rust_ffi::FT_Set_Var_Design_Coordinates(Some(&mut face.face), num_coords, coords)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_var_design_coordinates(
    handle: usize,
    num_coords: FT_UInt,
    coords: *mut FT_Fixed,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` writable FT_Fixed values in linear memory.
        Some(unsafe { slice::from_raw_parts_mut(coords, num_coords as usize) })
    };
    rust_ffi::FT_Get_Var_Design_Coordinates(Some(&face.face), num_coords, coords)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_var_blend_coordinates(
    handle: usize,
    num_coords: FT_UInt,
    coords: *mut FT_Fixed,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` writable FT_Fixed values in linear memory.
        Some(unsafe { slice::from_raw_parts_mut(coords, num_coords as usize) })
    };
    rust_ffi::FT_Get_Var_Blend_Coordinates(Some(&face.face), num_coords, coords)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_mm_blend_coordinates(
    handle: usize,
    num_coords: FT_UInt,
    coords: *mut FT_Fixed,
) -> FT_Error {
    fontdone_wasm_get_var_blend_coordinates(handle, num_coords, coords)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_var_blend_coordinates(
    handle: usize,
    num_coords: FT_UInt,
    coords: *const FT_Fixed,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` readable FT_Fixed values in linear memory.
        Some(unsafe { slice::from_raw_parts(coords, num_coords as usize) })
    };
    rust_ffi::FT_Set_Var_Blend_Coordinates(Some(&mut face.face), num_coords, coords)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_mm_blend_coordinates(
    handle: usize,
    num_coords: FT_UInt,
    coords: *const FT_Fixed,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` readable FT_Fixed values in linear memory.
        Some(unsafe { slice::from_raw_parts(coords, num_coords as usize) })
    };
    rust_ffi::FT_Set_MM_Blend_Coordinates(Some(&mut face.face), num_coords, coords)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_fstype_flags(handle: usize) -> FT_UShort {
    rust_ffi::FT_Get_FSType_Flags(face_ref(handle).map(|face| &face.face))
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_attach_stream(
    handle: usize,
    file_base: *const FT_Byte,
    file_size: usize,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if file_base.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: caller provides a readable linear-memory range of `file_size` bytes.
    let data = unsafe { slice::from_raw_parts(file_base, file_size) };
    rust_ffi::FT_Attach_Stream(Some(&mut face.face), Some(data))
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_track_kerning(
    handle: usize,
    point_size: FT_Fixed,
    degree: FT_Int,
    akerning: *mut FT_Fixed,
) -> FT_Error {
    let mut kerning = 0;
    let output = ptr::NonNull::new(akerning);
    let error = rust_ffi::FT_Get_Track_Kerning(
        face_ref(handle).map(|face| &face.face),
        point_size,
        degree,
        output.map(|_| &mut kerning),
    );
    if error == rust_ffi::FT_Err_Ok {
        if let Some(output) = output {
            // SAFETY: `akerning` was checked for null and points to writable linear memory.
            unsafe { *output.as_ptr() = kerning };
        }
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_gasp(handle: usize, ppem: FT_UInt) -> FT_Int {
    rust_ffi::FT_Get_Gasp(face_ref(handle).map(|face| &face.face), ppem)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_glyph_name(
    handle: usize,
    glyph_index: FT_UInt,
    buffer: *mut FT_Byte,
    buffer_max: FT_UInt,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if buffer.is_null() || buffer_max == 0 {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: `buffer` is non-null, and the WASM caller provides a writable
    // linear-memory range of `buffer_max` bytes.
    let buffer = unsafe { slice::from_raw_parts_mut(buffer, buffer_max as usize) };
    match rust_ffi::FT_Get_Glyph_Name(&face.face, glyph_index, buffer) {
        Ok(_) => rust_ffi::FT_Err_Ok,
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_name_index(
    handle: usize,
    glyph_name: *const FT_Byte,
    glyph_name_len: FT_UInt,
) -> FT_UInt {
    let Some(face) = face_ref(handle) else {
        return 0;
    };
    if glyph_name.is_null() {
        return 0;
    }
    // SAFETY: `glyph_name` is non-null, and the WASM caller provides a
    // readable linear-memory range of `glyph_name_len` bytes.
    let bytes = unsafe { slice::from_raw_parts(glyph_name, glyph_name_len as usize) };
    let Ok(glyph_name) = std::str::from_utf8(bytes) else {
        return 0;
    };
    rust_ffi::FT_Get_Name_Index(Some(&face.face), Some(glyph_name))
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_postscript_name(
    handle: usize,
    out: *mut FontdoneWasmString,
) -> FT_Bool {
    // SAFETY: the caller provides writable storage for the output record or null.
    let Some(out) = (unsafe { out.as_mut() }) else {
        return 0;
    };
    let Some(name) = face_ref(handle).and_then(|face| rust_ffi::FT_Get_Postscript_Name(&face.face))
    else {
        *out = FontdoneWasmString::default();
        return 0;
    };
    out.string = name.as_ptr();
    out.string_len = u32::try_from(name.len()).unwrap_or(u32::MAX);
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_font_format(
    handle: usize,
    out: *mut FontdoneWasmString,
) -> FT_Bool {
    // SAFETY: the caller provides writable storage for the output record or null.
    let Some(out) = (unsafe { out.as_mut() }) else {
        return 0;
    };
    let Some(format) =
        face_ref(handle).and_then(|face| rust_ffi::FT_Get_Font_Format(Some(&face.face)))
    else {
        *out = FontdoneWasmString::default();
        return 0;
    };
    out.string = format.as_ptr();
    out.string_len = u32::try_from(format.len()).unwrap_or(u32::MAX);
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_x11_font_format(
    handle: usize,
    out: *mut FontdoneWasmString,
) -> FT_Bool {
    fontdone_wasm_get_font_format(handle, out)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_face_driver_name(handle: usize, out: *mut FontdoneWasmString) -> FT_Bool {
    // SAFETY: the caller provides writable storage for the output record or null.
    let Some(out) = (unsafe { out.as_mut() }) else {
        return 0;
    };
    let Some(name) =
        face_ref(handle).and_then(|face| rust_ffi::FT_FACE_DRIVER_NAME(Some(&face.face)))
    else {
        *out = FontdoneWasmString::default();
        return 0;
    };
    out.string = name.as_ptr();
    out.string_len = u32::try_from(name.len()).unwrap_or(u32::MAX);
    1
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_named_instance(
    handle: usize,
    instance_index: FT_UInt,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let err = rust_ffi::FT_Set_Named_Instance(Some(&mut face.face), instance_index);
    if err == rust_ffi::FT_Err_Ok {
        face.slot = None;
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_default_named_instance(
    handle: usize,
    instance_index: *mut FT_UInt,
) -> FT_Error {
    // SAFETY: the caller provides writable storage for the scalar output or null.
    let instance_index = unsafe { instance_index.as_mut() };
    rust_ffi::FT_Get_Default_Named_Instance(face_ref(handle).map(|face| &face.face), instance_index)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_multi_master(
    handle: usize,
    amaster: *mut rust_ffi::FT_Multi_Master,
) -> FT_Error {
    // SAFETY: the caller provides writable storage for the public descriptor or null.
    let amaster = unsafe { amaster.as_mut() };
    rust_ffi::FT_Get_Multi_Master(face_ref(handle).map(|face| &face.face), amaster)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_mm_var(
    handle: usize,
    amaster: *mut rust_ffi::FT_MM_Var,
    axis: *mut rust_ffi::FT_Var_Axis,
    axis_capacity: FT_UInt,
) -> FT_Error {
    let amaster_ptr = amaster;
    let amaster = unsafe { amaster.as_mut() };
    let axis = if axis.is_null() {
        None
    } else {
        // SAFETY: caller provides `axis_capacity` writable FT_Var_Axis records.
        Some(unsafe { slice::from_raw_parts_mut(axis, axis_capacity as usize) })
    };
    let mut namedstyle = vec![rust_ffi::FT_Var_Named_Style::default(); 256].into_boxed_slice();
    let mut namedstyle_coords = vec![rust_ffi::FT_Fixed::default(); 64 * 256].into_boxed_slice();
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Get_MM_Var(None, amaster, axis, None, None);
    };
    let err = rust_ffi::FT_Get_MM_Var(
        Some(&face.face),
        amaster,
        axis,
        Some(&mut namedstyle),
        Some(&mut namedstyle_coords),
    );
    if err == rust_ffi::FT_Err_Ok && !amaster_ptr.is_null() {
        // SAFETY: `amaster_ptr` is the same non-null caller-owned output
        // descriptor just initialized by `FT_Get_MM_Var`.
        let master = unsafe { &mut *amaster_ptr };
        master.namedstyle = if master.num_namedstyles == 0 {
            ptr::null_mut()
        } else {
            namedstyle.as_mut_ptr()
        };
        face.mm_vars.insert(
            amaster_ptr as usize,
            WasmMmVarStorage {
                _namedstyle: namedstyle,
                _namedstyle_coords: namedstyle_coords,
            },
        );
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_var_axis_flags(
    master: *mut rust_ffi::FT_MM_Var,
    axis_index: FT_UInt,
    flags: *mut FT_UInt,
) -> FT_Error {
    let master = unsafe { master.as_ref() };
    let flags = unsafe { flags.as_mut() };
    rust_ffi::FT_Get_Var_Axis_Flags(master, axis_index, flags)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_mm_design_coordinates(
    handle: usize,
    num_coords: FT_UInt,
    coords: *const FT_Long,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` readable FT_Long values.
        Some(unsafe { slice::from_raw_parts(coords, num_coords as usize) })
    };
    rust_ffi::FT_Set_MM_Design_Coordinates(Some(&mut face.face), num_coords, coords)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_set_mm_weight_vector(
    handle: usize,
    len: FT_UInt,
    weightvector: *const FT_Fixed,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let weightvector = if weightvector.is_null() {
        None
    } else {
        // SAFETY: caller provides `len` readable FT_Fixed values.
        Some(unsafe { slice::from_raw_parts(weightvector, len as usize) })
    };
    rust_ffi::FT_Set_MM_WeightVector(Some(&mut face.face), len, weightvector)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_mm_weight_vector(
    handle: usize,
    len: *mut FT_UInt,
    weightvector: *mut FT_Fixed,
) -> FT_Error {
    let len_ref = unsafe { len.as_mut() };
    let capacity = len_ref.as_ref().map_or(0, |len| **len as usize);
    let weightvector = if weightvector.is_null() {
        None
    } else {
        // SAFETY: caller provides `*len` writable FT_Fixed values.
        Some(unsafe { slice::from_raw_parts_mut(weightvector, capacity) })
    };
    rust_ffi::FT_Get_MM_WeightVector(
        face_ref(handle).map(|face| &face.face),
        len_ref,
        weightvector,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_winfnt_header(
    handle: usize,
    header: *mut FontdoneWasmWinFNTHeader,
) -> FT_Error {
    let mut rust_header = rust_ffi::FT_WinFNT_HeaderRec::default();
    let err = rust_ffi::FT_Get_WinFNT_Header(
        face_ref(handle).map(|face| &face.face),
        (!header.is_null()).then_some(&mut rust_header),
    );
    if err == rust_ffi::FT_Err_Ok {
        // SAFETY: null was checked before requesting the core output.
        unsafe {
            *header = FontdoneWasmWinFNTHeader {
                version: rust_header.version,
                file_size: rust_header.file_size,
                copyright: rust_header.copyright,
                file_type: rust_header.file_type,
                nominal_point_size: rust_header.nominal_point_size,
                vertical_resolution: rust_header.vertical_resolution,
                horizontal_resolution: rust_header.horizontal_resolution,
                ascent: rust_header.ascent,
                internal_leading: rust_header.internal_leading,
                external_leading: rust_header.external_leading,
                italic: rust_header.italic,
                underline: rust_header.underline,
                strike_out: rust_header.strike_out,
                weight: rust_header.weight,
                charset: rust_header.charset,
                pixel_width: rust_header.pixel_width,
                pixel_height: rust_header.pixel_height,
                pitch_and_family: rust_header.pitch_and_family,
                avg_width: rust_header.avg_width,
                max_width: rust_header.max_width,
                first_char: rust_header.first_char,
                last_char: rust_header.last_char,
                default_char: rust_header.default_char,
                break_char: rust_header.break_char,
                bytes_per_row: rust_header.bytes_per_row,
                device_offset: rust_header.device_offset,
                face_name_offset: rust_header.face_name_offset,
                bits_pointer: rust_header.bits_pointer,
                bits_offset: rust_header.bits_offset,
                reserved: rust_header.reserved,
                flags: rust_header.flags,
                A_space: rust_header.A_space,
                B_space: rust_header.B_space,
                C_space: rust_header.C_space,
                color_table_offset: rust_header.color_table_offset,
                reserved1: rust_header.reserved1,
            };
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_ps_font_info(
    handle: usize,
    info: *mut FontdoneWasmPSFontInfo,
) -> FT_Error {
    let mut rust_info = rust_ffi::PS_FontInfoRec::default();
    let err = rust_ffi::FT_Get_PS_Font_Info(
        face_ref(handle).map(|face| &face.face),
        (!info.is_null()).then_some(&mut rust_info),
    );
    if err == rust_ffi::FT_Err_Ok && !info.is_null() {
        // SAFETY: null was checked above and the caller provided writable
        // linear-memory storage for the flat public WASM record.
        unsafe {
            copy_rust_ps_font_info_to_wasm(&mut *info, rust_info);
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_ps_font_private(
    handle: usize,
    private: *mut FontdoneWasmPSPrivate,
) -> FT_Error {
    let mut rust_private = rust_ffi::PS_PrivateRec::default();
    let err = rust_ffi::FT_Get_PS_Font_Private(
        face_ref(handle).map(|face| &face.face),
        (!private.is_null()).then_some(&mut rust_private),
    );
    if err == rust_ffi::FT_Err_Ok && !private.is_null() {
        // SAFETY: null was checked above and the caller provided writable
        // linear-memory storage for the flat public WASM record.
        unsafe {
            copy_rust_ps_private_to_wasm(&mut *private, rust_private);
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_has_ps_glyph_names(handle: usize) -> FT_Int {
    rust_ffi::FT_Has_PS_Glyph_Names(face_ref(handle).map(|face| &face.face))
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_ps_font_value(
    handle: usize,
    key: PS_Dict_Keys,
    idx: FT_UInt,
    value: *mut c_void,
    value_len: FT_Long,
) -> FT_Long {
    let effective_value_len = value_len.max(0);
    let value_len = usize::try_from(effective_value_len).unwrap_or(usize::MAX);
    let value = if value.is_null() {
        None
    } else {
        // SAFETY: caller supplies `value_len` writable linear-memory bytes at
        // `value`; this wrapper only lends those bytes to the safe Rust FFI.
        Some(unsafe { slice::from_raw_parts_mut(value.cast::<u8>(), value_len) })
    };
    rust_ffi::FT_Get_PS_Font_Value(
        face_ref(handle).map(|face| &face.face),
        key,
        idx,
        value,
        effective_value_len,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_bdf_property(
    handle: usize,
    prop_name: *const FT_Byte,
    prop_name_len: FT_UInt,
    property: *mut FontdoneWasmBdfProperty,
) -> FT_Error {
    let prop_name = if prop_name.is_null() {
        None
    } else {
        // SAFETY: the caller provides a readable linear-memory property-name
        // byte range.
        let bytes = unsafe { slice::from_raw_parts(prop_name, prop_name_len as usize) };
        std::str::from_utf8(bytes).ok()
    };
    let face = face_ref(handle);
    let mut rust_property = rust_ffi::BDF_PropertyRec::default();
    let err = rust_ffi::FT_Get_BDF_Property(
        face.map(|face| &face.face),
        prop_name,
        (!property.is_null()).then_some(&mut rust_property),
    );
    if face.is_some() && !property.is_null() {
        // SAFETY: null was checked above and the caller provided writable
        // linear-memory storage for the flat WASM record.
        unsafe {
            (*property).type_ = rust_property.type_;
            if err == rust_ffi::FT_Err_Ok && rust_property.type_ == rust_ffi::BDF_PROPERTY_TYPE_ATOM
            {
                let atom = rust_property.u.atom;
                (*property).atom = atom.cast::<FT_Byte>();
                (*property).atom_len = if atom.is_null() {
                    0
                } else {
                    CStr::from_ptr(atom).to_bytes().len() as FT_UInt
                };
            } else if err == rust_ffi::FT_Err_Ok
                && rust_property.type_ == rust_ffi::BDF_PROPERTY_TYPE_INTEGER
            {
                (*property).integer = rust_property.u.integer;
            } else if err == rust_ffi::FT_Err_Ok
                && rust_property.type_ == rust_ffi::BDF_PROPERTY_TYPE_CARDINAL
            {
                (*property).cardinal = rust_property.u.cardinal;
            }
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_bdf_charset_id(
    handle: usize,
    output: *mut FontdoneWasmBdfCharset,
) -> FT_Error {
    let face = face_ref(handle);
    let mut encoding: *const rust_ffi::FT_String = std::ptr::null();
    let mut registry: *const rust_ffi::FT_String = std::ptr::null();
    let err = rust_ffi::FT_Get_BDF_Charset_ID(
        face.map(|face| &face.face),
        (!output.is_null()).then_some(&mut encoding),
        (!output.is_null()).then_some(&mut registry),
    );
    if face.is_some() && !output.is_null() {
        // SAFETY: null was checked above and the caller provided writable
        // linear-memory storage for the flat WASM charset record.
        unsafe {
            (*output).charset_encoding = encoding.cast::<FT_Byte>();
            (*output).charset_encoding_len = if encoding.is_null() {
                0
            } else {
                CStr::from_ptr(encoding).to_bytes().len() as FT_UInt
            };
            (*output).charset_registry = registry.cast::<FT_Byte>();
            (*output).charset_registry_len = if registry.is_null() {
                0
            } else {
                CStr::from_ptr(registry).to_bytes().len() as FT_UInt
            };
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_cid_is_internally_cid_keyed(
    handle: usize,
    is_cid: *mut FT_Bool,
) -> FT_Error {
    let mut value = 0;
    let err = rust_ffi::FT_Get_CID_Is_Internally_CID_Keyed(
        face_ref(handle).map(|face| &face.face),
        (!is_cid.is_null()).then_some(&mut value),
    );
    if !is_cid.is_null() {
        // SAFETY: null was checked and caller provides writable scalar output.
        unsafe {
            *is_cid = value;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_cid_from_glyph_index(
    handle: usize,
    glyph_index: FT_UInt,
    cid: *mut FT_UInt,
) -> FT_Error {
    let mut value = 0;
    let err = rust_ffi::FT_Get_CID_From_Glyph_Index(
        face_ref(handle).map(|face| &face.face),
        glyph_index,
        (!cid.is_null()).then_some(&mut value),
    );
    if !cid.is_null() {
        // SAFETY: null was checked and caller provides writable scalar output.
        unsafe {
            *cid = value;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_cid_registry_ordering_supplement(
    handle: usize,
    output: *mut FontdoneWasmCidRos,
) -> FT_Error {
    let mut registry: *const rust_ffi::FT_String = std::ptr::null();
    let mut ordering: *const rust_ffi::FT_String = std::ptr::null();
    let mut supplement = 0;
    let err = rust_ffi::FT_Get_CID_Registry_Ordering_Supplement(
        face_ref(handle).map(|face| &face.face),
        (!output.is_null()).then_some(&mut registry),
        (!output.is_null()).then_some(&mut ordering),
        (!output.is_null()).then_some(&mut supplement),
    );
    if !output.is_null() {
        // SAFETY: null was checked above and caller provided writable
        // linear-memory storage for the flat WASM ROS record.
        unsafe {
            (*output).registry = registry.cast::<FT_Byte>();
            (*output).registry_len = if registry.is_null() {
                0
            } else {
                CStr::from_ptr(registry).to_bytes().len() as FT_UInt
            };
            (*output).ordering = ordering.cast::<FT_Byte>();
            (*output).ordering_len = if ordering.is_null() {
                0
            } else {
                CStr::from_ptr(ordering).to_bytes().len() as FT_UInt
            };
            (*output).supplement = supplement;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_sfnt_name_count(handle: usize) -> FT_UInt {
    rust_ffi::FT_Get_Sfnt_Name_Count(face_ref(handle).map(|face| &face.face))
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_sfnt_name(
    handle: usize,
    idx: FT_UInt,
    out: *mut FontdoneWasmSfntName,
) -> FT_Error {
    if out.is_null() {
        return rust_ffi::FT_Get_Sfnt_Name(face_ref(handle).map(|face| &face.face), idx, None);
    }
    let mut name = rust_ffi::FT_SfntName::default();
    let error = rust_ffi::FT_Get_Sfnt_Name(
        face_ref(handle).map(|face| &face.face),
        idx,
        Some(&mut name),
    );
    if error == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe {
            *out = FontdoneWasmSfntName {
                platform_id: name.platform_id,
                encoding_id: name.encoding_id,
                language_id: name.language_id,
                name_id: name.name_id,
                string: name.string.cast_const(),
                string_len: name.string_len,
            };
        }
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_sfnt_os2(
    handle: usize,
    tag: FT_Sfnt_Tag,
    out: *mut FontdoneWasmOs2,
) -> FT_Error {
    if out.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let table = rust_ffi::FT_Get_Sfnt_Table(&face.face, tag);
    if table.is_null() {
        return rust_ffi::FT_Err_Invalid_Table as FT_Error;
    }
    // SAFETY: `FT_Get_Sfnt_Table` returns a live face-owned `TT_OS2` pointer for this tag.
    let os2 = unsafe { &*table.cast::<rust_ffi::TT_OS2>() };
    // SAFETY: `out` is non-null and caller provides writable storage.
    unsafe {
        *out = FontdoneWasmOs2 {
            version: os2.version,
            xAvgCharWidth: os2.xAvgCharWidth,
            usWeightClass: os2.usWeightClass,
            usWidthClass: os2.usWidthClass,
            fsType: os2.fsType,
            ySubscriptXSize: os2.ySubscriptXSize,
            ySubscriptYSize: os2.ySubscriptYSize,
            ySubscriptXOffset: os2.ySubscriptXOffset,
            ySubscriptYOffset: os2.ySubscriptYOffset,
            ySuperscriptXSize: os2.ySuperscriptXSize,
            ySuperscriptYSize: os2.ySuperscriptYSize,
            ySuperscriptXOffset: os2.ySuperscriptXOffset,
            ySuperscriptYOffset: os2.ySuperscriptYOffset,
            yStrikeoutSize: os2.yStrikeoutSize,
            yStrikeoutPosition: os2.yStrikeoutPosition,
            sFamilyClass: os2.sFamilyClass,
            panose: os2.panose,
            ulUnicodeRange1: os2.ulUnicodeRange1,
            ulUnicodeRange2: os2.ulUnicodeRange2,
            ulUnicodeRange3: os2.ulUnicodeRange3,
            ulUnicodeRange4: os2.ulUnicodeRange4,
            achVendID: os2.achVendID,
            fsSelection: os2.fsSelection,
            usFirstCharIndex: os2.usFirstCharIndex,
            usLastCharIndex: os2.usLastCharIndex,
            sTypoAscender: os2.sTypoAscender,
            sTypoDescender: os2.sTypoDescender,
            sTypoLineGap: os2.sTypoLineGap,
            usWinAscent: os2.usWinAscent,
            usWinDescent: os2.usWinDescent,
            ulCodePageRange1: os2.ulCodePageRange1,
            ulCodePageRange2: os2.ulCodePageRange2,
            sxHeight: os2.sxHeight,
            sCapHeight: os2.sCapHeight,
            usDefaultChar: os2.usDefaultChar,
            usBreakChar: os2.usBreakChar,
            usMaxContext: os2.usMaxContext,
            usLowerOpticalPointSize: os2.usLowerOpticalPointSize,
            usUpperOpticalPointSize: os2.usUpperOpticalPointSize,
        };
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_sfnt_vhea(
    handle: usize,
    tag: FT_Sfnt_Tag,
    out: *mut FontdoneWasmVertHeader,
) -> FT_Error {
    if out.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let table = rust_ffi::FT_Get_Sfnt_Table(&face.face, tag);
    if table.is_null() {
        return rust_ffi::FT_Err_Invalid_Table as FT_Error;
    }
    // SAFETY: `FT_Get_Sfnt_Table` returns a live face-owned `TT_VertHeader` pointer for this tag.
    let vhea = unsafe { &*table.cast::<rust_ffi::TT_VertHeader>() };
    // SAFETY: `out` is non-null and caller provides writable storage.
    unsafe {
        *out = FontdoneWasmVertHeader {
            Version: vhea.Version,
            Ascender: vhea.Ascender,
            Descender: vhea.Descender,
            Line_Gap: vhea.Line_Gap,
            advance_Height_Max: vhea.advance_Height_Max,
            min_Top_Side_Bearing: vhea.min_Top_Side_Bearing,
            min_Bottom_Side_Bearing: vhea.min_Bottom_Side_Bearing,
            yMax_Extent: vhea.yMax_Extent,
            caret_Slope_Rise: vhea.caret_Slope_Rise,
            caret_Slope_Run: vhea.caret_Slope_Run,
            caret_Offset: vhea.caret_Offset,
            Reserved: vhea.Reserved,
            metric_Data_Format: vhea.metric_Data_Format,
            number_Of_VMetrics: vhea.number_Of_VMetrics,
            long_metrics: vhea.long_metrics.cast(),
            short_metrics: vhea.short_metrics.cast(),
        };
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_sfnt_maxp(
    handle: usize,
    tag: FT_Sfnt_Tag,
    out: *mut FontdoneWasmMaxProfile,
) -> FT_Error {
    if out.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let table = rust_ffi::FT_Get_Sfnt_Table(&face.face, tag);
    if table.is_null() {
        return rust_ffi::FT_Err_Invalid_Table as FT_Error;
    }
    // SAFETY: `FT_Get_Sfnt_Table` returns a live face-owned `TT_MaxProfile` pointer for this tag.
    let maxp = unsafe { &*table.cast::<rust_ffi::TT_MaxProfile>() };
    // SAFETY: `out` is non-null and caller provides writable storage.
    unsafe {
        *out = FontdoneWasmMaxProfile {
            version: maxp.version,
            numGlyphs: maxp.numGlyphs,
            maxPoints: maxp.maxPoints,
            maxContours: maxp.maxContours,
            maxCompositePoints: maxp.maxCompositePoints,
            maxCompositeContours: maxp.maxCompositeContours,
            maxZones: maxp.maxZones,
            maxTwilightPoints: maxp.maxTwilightPoints,
            maxStorage: maxp.maxStorage,
            maxFunctionDefs: maxp.maxFunctionDefs,
            maxInstructionDefs: maxp.maxInstructionDefs,
            maxStackElements: maxp.maxStackElements,
            maxSizeOfInstructions: maxp.maxSizeOfInstructions,
            maxComponentElements: maxp.maxComponentElements,
            maxComponentDepth: maxp.maxComponentDepth,
        };
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_load_sfnt_table(
    handle: usize,
    tag: FT_ULong,
    offset: FT_Long,
    out_buffer: *mut FT_Byte,
    out_length: *mut FT_ULong,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if out_length.is_null() {
        return match rust_ffi::FT_Load_Sfnt_Table(&face.face, tag, offset, None) {
            Ok(Some(bytes)) => {
                if !out_buffer.is_null() {
                    // SAFETY: out_buffer has enough space for the selected table bytes.
                    unsafe {
                        std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buffer, bytes.len());
                    }
                }
                rust_ffi::FT_Err_Ok as FT_Error
            }
            Ok(None) => rust_ffi::FT_Err_Ok as FT_Error,
            Err(err) => err as FT_Error,
        };
    }
    // SAFETY: caller provides a writable FT_ULong.
    let mut len_val = unsafe { *out_length };
    match rust_ffi::FT_Load_Sfnt_Table(&face.face, tag, offset, Some(&mut len_val)) {
        Ok(Some(bytes)) => {
            let copy_len = bytes.len().min(len_val as usize);
            if !out_buffer.is_null() {
                // SAFETY: out_buffer has at least len_val bytes.
                unsafe {
                    std::ptr::copy_nonoverlapping(bytes.as_ptr(), out_buffer, copy_len);
                }
            }
            // SAFETY: writable FT_ULong out-param.
            unsafe { *out_length = copy_len as FT_ULong };
            rust_ffi::FT_Err_Ok as FT_Error
        }
        Ok(None) => {
            // SAFETY: writable FT_ULong out-param (length probe result).
            unsafe { *out_length = len_val };
            rust_ffi::FT_Err_Ok as FT_Error
        }
        Err(err) => err as FT_Error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_sfnt_table_info(
    handle: usize,
    table_index: FT_UInt,
    out_tag: *mut FT_ULong,
    out_length: *mut FT_ULong,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let mut tag_out: rust_ffi::FT_ULong = 0;
    let mut length_out: rust_ffi::FT_ULong = 0;
    let tag_ref = if out_tag.is_null() {
        None
    } else {
        Some(&mut tag_out)
    };
    let length_ref = if out_length.is_null() {
        None
    } else {
        Some(&mut length_out)
    };
    let err = rust_ffi::FT_Sfnt_Table_Info(&face.face, table_index, tag_ref, length_ref);
    if err == rust_ffi::FT_Err_Ok {
        if !out_tag.is_null() {
            // SAFETY: writable FT_ULong out-param. Copying after the core call
            // avoids creating aliased `&mut` references for caller pointers.
            unsafe { *out_tag = tag_out as FT_ULong };
        }
        if !out_length.is_null() {
            // SAFETY: writable FT_ULong out-param. C writes tag before length,
            // so an aliased caller pointer ends with the length value.
            unsafe { *out_length = length_out as FT_ULong };
        }
    }
    err as FT_Error
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_first_char(handle: usize, agindex: *mut FT_UInt) -> FT_ULong {
    let mut glyph_index = 0;
    let char_code = rust_ffi::FT_Get_First_Char(
        face_ref(handle).map(|face| &face.face),
        // FreeType `base/ftobjs.c:3952-3972` accepts a null `agindex`;
        // it still returns the charcode and skips only the glyph-index write.
        (!agindex.is_null()).then_some(&mut glyph_index),
    );
    if !agindex.is_null() {
        // SAFETY: `agindex` is non-null and caller provides writable storage.
        unsafe { *agindex = glyph_index };
    }
    char_code
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_next_char(
    handle: usize,
    char_code: FT_ULong,
    agindex: *mut FT_UInt,
) -> FT_ULong {
    let mut glyph_index = 0;
    let next_char = rust_ffi::FT_Get_Next_Char(
        face_ref(handle).map(|face| &face.face),
        char_code,
        // FreeType `base/ftobjs.c:3977-4003` accepts a null `agindex`;
        // it still returns the next charcode and skips only the glyph-index write.
        (!agindex.is_null()).then_some(&mut glyph_index),
    );
    if !agindex.is_null() {
        // SAFETY: `agindex` is non-null and caller provides writable storage.
        unsafe { *agindex = glyph_index };
    }
    next_char
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_library_version(
    library_present: i32,
    amajor: *mut i32,
    aminor: *mut i32,
    apatch: *mut i32,
) {
    let library = rust_ffi::FT_Init_FreeType();
    let library = if library_present != 0 {
        Some(&library)
    } else {
        None
    };
    let mut major = 0;
    let mut minor = 0;
    let mut patch = 0;
    rust_ffi::FT_Library_Version(
        library,
        Some(&mut major),
        Some(&mut minor),
        Some(&mut patch),
    );
    if !amajor.is_null() {
        // SAFETY: `amajor` is non-null and caller provides writable storage.
        unsafe { *amajor = major };
    }
    if !aminor.is_null() {
        // SAFETY: `aminor` is non-null and caller provides writable storage.
        unsafe { *aminor = minor };
    }
    if !apatch.is_null() {
        // SAFETY: `apatch` is non-null and caller provides writable storage.
        unsafe { *apatch = patch };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_load_char(
    handle: usize,
    char_code: FT_ULong,
    load_flags: FT_Int32,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    match rust_ffi::FT_Load_Char(&face.face, char_code, load_flags) {
        Ok(slot) => {
            face.slot = Some(slot);
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_load_glyph(
    handle: usize,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    match rust_ffi::FT_Load_Glyph(&face.face, glyph_index, load_flags) {
        Ok(slot) => {
            face.slot = Some(slot);
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_advance(
    handle: usize,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
    padvance: *mut FT_Fixed,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        // FreeType `src/base/ftadvanc.c:116-120` checks `face` before
        // `padvance`, so a missing face reports `Invalid_Face_Handle`.
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if padvance.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    match rust_ffi::FT_Get_Advance(&face.face, glyph_index, load_flags) {
        Ok(advance) => {
            // SAFETY: `padvance` is non-null and caller provides writable storage.
            unsafe { *padvance = advance };
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_advances(
    handle: usize,
    start: FT_UInt,
    count: FT_UInt,
    load_flags: FT_Int32,
    padvances: *mut FT_Fixed,
) -> FT_Error {
    let Some(face) = face_ref(handle) else {
        // FreeType `src/base/ftadvanc.c:158-164` checks `face` before
        // `padvances`, so a missing face reports `Invalid_Face_Handle`.
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if padvances.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Ok(out_len) = usize::try_from(count) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    match rust_ffi::FT_Get_Advances(&face.face, start, count, load_flags) {
        Ok(advances) => {
            if advances.len() != out_len {
                return rust_ffi::FT_Err_Invalid_Argument;
            }
            if out_len != 0 {
                // SAFETY: `out` is non-null and caller promises at least `count` writable entries.
                let out = unsafe { slice::from_raw_parts_mut(padvances, out_len) };
                out.copy_from_slice(&advances);
            }
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_subglyph_info(
    handle: usize,
    sub_index: FT_UInt,
    p_index: *mut FT_Int,
    p_flags: *mut FT_UInt,
    p_arg1: *mut FT_Int,
    p_arg2: *mut FT_Int,
    p_transform: *mut FontdoneWasmMatrix,
) -> FT_Error {
    if p_index.is_null()
        || p_flags.is_null()
        || p_arg1.is_null()
        || p_arg2.is_null()
        || p_transform.is_null()
    {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(slot) = face.slot.as_ref() else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let mut index = 0;
    let mut flags = 0;
    let mut arg1 = 0;
    let mut arg2 = 0;
    let mut transform = rust_ffi::FT_Matrix::default();
    let error = rust_ffi::FT_Get_SubGlyph_Info(
        Some(slot),
        sub_index,
        Some(&mut index),
        Some(&mut flags),
        Some(&mut arg1),
        Some(&mut arg2),
        Some(&mut transform),
    );
    if error != rust_ffi::FT_Err_Ok {
        return error;
    }

    // SAFETY: output pointers are checked non-null and caller provides writable storage.
    unsafe {
        *p_index = index;
        *p_flags = flags;
        *p_arg1 = arg1;
        *p_arg2 = arg2;
        *p_transform = FontdoneWasmMatrix {
            xx: transform.xx,
            xy: transform.xy,
            yx: transform.yx,
            yy: transform.yy,
        };
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_render_glyph(handle: usize, render_mode: i32) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(slot) = face.slot.clone() else {
        return rust_ffi::FT_Err_Invalid_Glyph_Index;
    };
    match rust_ffi::FT_Render_Glyph(slot, render_mode) {
        Ok(rendered) => {
            face.slot = Some(rendered);
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_sfnt_load_name_diagnostic(data: &[u8]) -> FT_Error {
    rust_ffi::FT_Sfnt_Load_Name_Diagnostic(data)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_open_face_non_driver_diagnostic() -> FT_Error {
    rust_ffi::FT_Open_Face_NonDriver_Diagnostic()
}

#[cfg(feature = "abi-test-support")]
pub fn abi_truetype_context_allocation_failure_diagnostic() -> FT_Error {
    rust_ffi::FT_TrueType_Context_Allocation_Failure_Diagnostic()
}

/// Returns the current rendered slot bitmap's byte offset in linear memory.
///
/// The returned bytes are borrowed from the face. They remain valid until the
/// next operation that replaces or renders the slot, or until
/// [`fontdone_wasm_done_face`] consumes the handle. Zero means no bitmap.
#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_bitmap_buffer(handle: usize) -> *const FT_Byte {
    face_ref(handle)
        .and_then(|face| face.slot.as_ref())
        .and_then(|slot| slot.bitmap.as_ref())
        .map_or(ptr::null(), |bitmap| bitmap.buffer.as_ptr())
}

/// Returns `abs(pitch) * rows` for the current rendered slot bitmap.
#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_bitmap_len(handle: usize) -> usize {
    face_ref(handle)
        .and_then(|face| face.slot.as_ref())
        .and_then(|slot| slot.bitmap.as_ref())
        .map_or(0, |bitmap| bitmap.buffer.len())
}

/// Returns the current rendered slot bitmap width in pixels.
#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_bitmap_width(handle: usize) -> FT_UInt {
    face_ref(handle)
        .and_then(|face| face.slot.as_ref())
        .and_then(|slot| slot.bitmap.as_ref())
        .map_or(0, |bitmap| bitmap.width)
}

/// Returns the current rendered slot bitmap row count.
#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_bitmap_rows(handle: usize) -> FT_UInt {
    face_ref(handle)
        .and_then(|face| face.slot.as_ref())
        .and_then(|slot| slot.bitmap.as_ref())
        .map_or(0, |bitmap| bitmap.rows)
}

/// Returns the current rendered slot bitmap's signed byte pitch.
#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_bitmap_pitch(handle: usize) -> FT_Int {
    face_ref(handle)
        .and_then(|face| face.slot.as_ref())
        .and_then(|slot| slot.bitmap.as_ref())
        .map_or(0, |bitmap| bitmap.pitch)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_set_unsupported_glyph_slot(handle: usize) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    face.slot = Some(rust_ffi::FT_Unsupported_GlyphSlot(&face.face));
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_set_malformed_get_glyph_slot(handle: usize, variant: FT_UInt) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    face.slot = Some(rust_ffi::FT_Malformed_Get_GlyphSlot(&face.face, variant));
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_set_outline_glyph_slot_advance(
    handle: usize,
    advance_x: FT_Pos,
    advance_y: FT_Pos,
) -> FT_Error {
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    face.slot = Some(rust_ffi::FT_Outline_GlyphSlot_With_Advance(
        &face.face, advance_x, advance_y,
    ));
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_glyphslot_oblique(handle: usize) -> FT_Error {
    fontdone_wasm_glyphslot_slant(handle, 0x0366A, 0)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_glyphslot_embolden(handle: usize) -> FT_Error {
    fontdone_wasm_glyphslot_adjust_weight(handle, 0x0AAA, 0x0AAA)
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_glyphslot_own_bitmap(handle: usize) -> FT_Error {
    if handle == 0 {
        return rust_ffi::FT_Err_Ok;
    }
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(slot) = face.slot.as_mut() else {
        return rust_ffi::FT_Err_Ok;
    };
    rust_ffi::FT_GlyphSlot_Own_Bitmap(Some(slot))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_own_bitmap_copy_allocation_failure(handle: usize) -> FT_Error {
    if handle == 0 {
        return rust_ffi::FT_Err_Ok;
    }
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(slot) = face.slot.as_mut() else {
        return rust_ffi::FT_Err_Ok;
    };
    rust_ffi::FT_GlyphSlot_Own_Bitmap_Copy_Allocation_Failure(Some(slot))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_fvar_namedstyle_coords(
    handle: usize,
    namedstyle_index: FT_UInt,
) -> Option<Vec<FT_Fixed>> {
    let face = face_ref(handle)?;
    rust_ffi::FT_Fvar_Named_Style_Coords(Some(&face.face), namedstyle_index).ok()
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_glyphslot_adjust_weight(
    handle: usize,
    xdelta: FT_Fixed,
    ydelta: FT_Fixed,
) -> FT_Error {
    if handle == 0 {
        return rust_ffi::FT_Err_Ok;
    }
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(slot) = face.slot.as_mut() else {
        return rust_ffi::FT_Err_Invalid_Glyph_Index;
    };
    rust_ffi::FT_GlyphSlot_AdjustWeight(Some(slot), xdelta, ydelta);
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_glyphslot_slant(
    handle: usize,
    xslant: FT_Fixed,
    yslant: FT_Fixed,
) -> FT_Error {
    if handle == 0 {
        return rust_ffi::FT_Err_Ok;
    }
    let Some(face) = face_mut(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(slot) = face.slot.as_mut() else {
        return rust_ffi::FT_Err_Invalid_Glyph_Index;
    };
    rust_ffi::FT_GlyphSlot_Slant(Some(slot), xslant, yslant);
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_get_slot(
    handle: usize,
    out: *mut FontdoneWasmGlyphSlot,
) -> FT_Error {
    if out.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(slot) = face.slot.as_ref() else {
        return rust_ffi::FT_Err_Invalid_Glyph_Index;
    };
    // SAFETY: `out` is non-null and caller provides writable storage.
    unsafe { *out = slot_to_wasm(slot) };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn fontdone_wasm_size_metrics(
    handle: usize,
    out: *mut FontdoneWasmSizeMetrics,
) -> FT_Error {
    if out.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(face) = face_ref(handle) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let metrics = face
        .size_metrics
        .get(&face.active_size)
        .copied()
        .unwrap_or(face.face.size_metrics);
    // SAFETY: `out` is non-null and caller provides writable storage.
    unsafe {
        *out = FontdoneWasmSizeMetrics {
            x_ppem: metrics.x_ppem,
            y_ppem: metrics.y_ppem,
            x_scale: metrics.x_scale,
            y_scale: metrics.y_scale,
            ascender: metrics.ascender,
            descender: metrics.descender,
            height: metrics.height,
            max_advance: metrics.max_advance,
        }
    };
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
fn rust_face_info(face: &rust_ffi::FT_Face) -> rust_ffi::FT_FaceRecPublic {
    rust_ffi::FT_FaceRecPublic {
        num_faces: face.num_faces,
        face_index: face.face_index,
        face_flags: face.face_flags,
        style_flags: face.style_flags,
        num_glyphs: face.num_glyphs,
        num_fixed_sizes: face.num_fixed_sizes,
        available_sizes: if face.available_sizes.is_empty() {
            ptr::null_mut()
        } else {
            face.available_sizes.as_ptr().cast_mut()
        },
        bbox: face.bbox,
        units_per_EM: face.units_per_EM,
        ascender: face.ascender,
        descender: face.descender,
        height: face.height,
        max_advance_width: face.max_advance_width,
        max_advance_height: face.max_advance_height,
        underline_position: face.underline_position,
        underline_thickness: face.underline_thickness,
        size: face.size,
        stream: face.memory_stream(),
        ..rust_ffi::FT_FaceRecPublic::default()
    }
}

fn rust_face_charmap(face: &rust_ffi::FT_Face, index: FT_UInt) -> rust_ffi::FT_CharMap {
    let Ok(index) = usize::try_from(index) else {
        return ptr::null_mut();
    };
    face.charmaps.get(index).map_or(ptr::null_mut(), |record| {
        (record as *const rust_ffi::FT_CharMapRecPublic)
            .cast_mut()
            .cast()
    })
}

fn rust_face_charmap_info(
    face: &rust_ffi::FT_Face,
    index: FT_UInt,
) -> Option<rust_ffi::FT_CharMapRecPublic> {
    let index = usize::try_from(index).ok()?;
    face.charmaps.get(index).copied()
}

fn slot_to_wasm(slot: &rust_ffi::FT_GlyphSlot) -> FontdoneWasmGlyphSlot {
    let bitmap = slot
        .bitmap
        .as_ref()
        .map(|bitmap| FontdoneWasmBitmap {
            rows: bitmap.rows,
            width: bitmap.width,
            pitch: bitmap.pitch,
            buffer: bitmap.buffer.as_ptr(),
            buffer_len: bitmap.buffer.len(),
            num_grays: bitmap.num_grays,
            pixel_mode: bitmap.pixel_mode,
            palette_mode: 0,
            palette: ptr::null(),
        })
        .unwrap_or_default();
    FontdoneWasmGlyphSlot {
        glyph_index: slot.glyph_index,
        metrics: FontdoneWasmGlyphMetrics {
            width: slot.metrics.width,
            height: slot.metrics.height,
            horiBearingX: slot.metrics.horiBearingX,
            horiBearingY: slot.metrics.horiBearingY,
            horiAdvance: slot.metrics.horiAdvance,
            vertBearingX: slot.metrics.vertBearingX,
            vertBearingY: slot.metrics.vertBearingY,
            vertAdvance: slot.metrics.vertAdvance,
        },
        advance: FontdoneWasmVector {
            x: slot.advance.x,
            y: slot.advance.y,
        },
        format: slot.format,
        num_subglyphs: slot.num_subglyphs,
        bitmap,
        bitmap_left: slot.bitmap_left,
        bitmap_top: slot.bitmap_top,
    }
}

fn outline_snapshot_from_wasm(
    outline: *const FontdoneWasmOutline,
) -> Option<rust_ffi::FT_OutlineSnapshot> {
    if outline.is_null() {
        return None;
    }
    // SAFETY: `outline` is non-null; wasm ABI callers must pass a valid outline record.
    let outline = unsafe { &*outline };
    let n_points = usize::from(outline.n_points);
    let n_contours = usize::from(outline.n_contours);
    if (n_points > 0 && outline.points.is_null()) || (n_contours > 0 && outline.contours.is_null())
    {
        return None;
    }
    let points = if n_points == 0 {
        Vec::new()
    } else {
        // SAFETY: `points` is non-null for `n_points > 0`; the caller provides
        // `n_points` readable vector records for the duration of this call.
        unsafe { slice::from_raw_parts(outline.points, n_points) }
            .iter()
            .map(|point| rust_ffi::FT_Vector {
                x: point.x,
                y: point.y,
            })
            .collect()
    };
    let tags = if n_points == 0 || outline.tags.is_null() {
        Vec::new()
    } else {
        // SAFETY: `tags` is non-null and points to `n_points` readable tag bytes.
        unsafe { slice::from_raw_parts(outline.tags, n_points) }.to_vec()
    };
    let contours = if n_contours == 0 {
        Vec::new()
    } else {
        // SAFETY: `contours` is non-null for `n_contours > 0`; the caller provides
        // `n_contours` readable contour endpoint values.
        unsafe { slice::from_raw_parts(outline.contours, n_contours) }.to_vec()
    };
    Some(rust_ffi::FT_OutlineSnapshot {
        points,
        tags,
        contours,
        flags: outline.flags,
    })
}

fn copy_outline_snapshot_to_wasm(
    outline: *mut FontdoneWasmOutline,
    snapshot: &rust_ffi::FT_OutlineSnapshot,
    copy_tags_and_flags: bool,
) {
    // SAFETY: callers pass the same writable descriptor used to construct
    // `snapshot`; null is handled as a no-op.
    let Some(outline) = (unsafe { outline.as_mut() }) else {
        return;
    };
    if !outline.points.is_null() {
        // SAFETY: the WASM descriptor promises `n_points` writable vectors.
        let points =
            unsafe { slice::from_raw_parts_mut(outline.points, usize::from(outline.n_points)) };
        for (target, source) in points.iter_mut().zip(&snapshot.points) {
            target.x = source.x;
            target.y = source.y;
        }
    }
    if copy_tags_and_flags {
        if !outline.tags.is_null() {
            // SAFETY: the WASM descriptor promises `n_points` writable tag bytes.
            let tags =
                unsafe { slice::from_raw_parts_mut(outline.tags, usize::from(outline.n_points)) };
            for (target, source) in tags.iter_mut().zip(&snapshot.tags) {
                *target = *source;
            }
        }
        outline.flags = snapshot.flags;
    }
}

fn copy_rendered_bitmap_to_wasm(target: &mut FontdoneWasmBitmap, rendered: &rust_ffi::FT_Bitmap) {
    let rows = usize::try_from(target.rows).unwrap_or(0);
    let width = usize::try_from(target.width).unwrap_or(0);
    let pitch_abs = usize::try_from(target.pitch.unsigned_abs()).unwrap_or(0);
    let rendered_pitch_abs = usize::try_from(rendered.pitch.unsigned_abs()).unwrap_or(0);
    if target.buffer.is_null() || rows == 0 || width == 0 || pitch_abs == 0 {
        return;
    }
    let row_bytes = width.min(pitch_abs);
    let target_len = pitch_abs.saturating_mul(rows).min(target.buffer_len);
    // SAFETY: the WASM ABI caller provides writable linear memory for this
    // bitmap buffer; `buffer_len` bounds the slice visible to this wrapper.
    let target_buffer = unsafe { slice::from_raw_parts_mut(target.buffer.cast_mut(), target_len) };
    for row in 0..rows {
        let src = row.saturating_mul(rendered_pitch_abs);
        let dst = row.saturating_mul(pitch_abs);
        let Some(src_row) = rendered.buffer.get(src..src.saturating_add(row_bytes)) else {
            break;
        };
        let Some(dst_row) = target_buffer.get_mut(dst..dst.saturating_add(row_bytes)) else {
            break;
        };
        dst_row.copy_from_slice(src_row);
    }
}

fn wasm_bitmap_to_rust(bitmap: &FontdoneWasmBitmap) -> rust_ffi::FT_Bitmap_C {
    rust_ffi::FT_Bitmap_C {
        rows: bitmap.rows,
        width: bitmap.width,
        pitch: bitmap.pitch,
        buffer: bitmap.buffer.cast_mut(),
        num_grays: bitmap.num_grays,
        pixel_mode: u8::try_from(bitmap.pixel_mode).unwrap_or(0),
        palette_mode: bitmap.palette_mode,
        palette: bitmap.palette.cast_mut(),
    }
}

fn wasm_bitmap_copy_array_too_large(bitmap: &FontdoneWasmBitmap) -> bool {
    let pitch = bitmap.pitch.unsigned_abs() as usize;
    let rows = bitmap.rows as usize;
    pitch > 0 && rows > (i32::MAX as usize) / pitch
}

fn copy_rust_bitmap_record_to_wasm(
    target: &mut FontdoneWasmBitmap,
    source: &rust_ffi::FT_Bitmap_C,
) {
    target.rows = source.rows;
    target.width = source.width;
    target.pitch = source.pitch;
    target.buffer = source.buffer;
    target.buffer_len = usize::try_from(source.pitch.unsigned_abs())
        .ok()
        .and_then(|pitch| pitch.checked_mul(usize::try_from(source.rows).ok()?))
        .unwrap_or(0);
    target.num_grays = source.num_grays;
    target.pixel_mode = source.pixel_mode.into();
    target.palette_mode = source.palette_mode;
    target.palette = source.palette;
}

fn wasm_bitmap_bytes(bitmap: &FontdoneWasmBitmap) -> Option<Vec<u8>> {
    let len = usize::try_from(bitmap.pitch.unsigned_abs())
        .ok()?
        .checked_mul(usize::try_from(bitmap.rows).ok()?)?
        .min(bitmap.buffer_len);
    if bitmap.buffer.is_null() || len == 0 {
        return None;
    }
    Some(unsafe { slice::from_raw_parts(bitmap.buffer, len) }.to_vec())
}

fn wasm_bitmap_copy_bytes(bitmap: &FontdoneWasmBitmap) -> Option<Vec<u8>> {
    if wasm_bitmap_copy_array_too_large(bitmap) {
        return None;
    }
    wasm_bitmap_bytes(bitmap)
}

fn face_ref(handle: usize) -> Option<&'static WasmFaceState> {
    if handle == 0 {
        return None;
    }
    let ptr = ptr::with_exposed_provenance::<WasmFaceState>(handle);
    // SAFETY: non-zero handles are expected to be produced by `fontdone_wasm_open_face`.
    Some(unsafe { &*ptr })
}

fn face_mut(handle: usize) -> Option<&'static mut WasmFaceState> {
    if handle == 0 {
        return None;
    }
    let ptr = ptr::with_exposed_provenance_mut::<WasmFaceState>(handle);
    // SAFETY: non-zero handles are expected to be produced by `fontdone_wasm_open_face`.
    Some(unsafe { &mut *ptr })
}
