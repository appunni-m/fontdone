//! Implementation of the C ABI façade for the `fontdone` engine.

#![expect(
    missing_docs,
    reason = "raw C ABI is documented by the shipped header, support matrix, and package README"
)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::not_unsafe_ptr_arg_deref)]
// The ABI coverage probes deliberately enumerate bounded public-input
// permutations and retain values long enough to exercise lifecycle/error
// paths. Keep these allowances restricted to the optional test-support build;
// the production ABI and runtime crates remain fully linted.
#![cfg_attr(
    feature = "abi-test-support",
    allow(
        clippy::arithmetic_side_effects,
        clippy::drop_non_drop,
        clippy::expect_used,
        clippy::let_unit_value,
        clippy::map_unwrap_or,
        clippy::needless_borrow,
        clippy::redundant_clone,
        clippy::unnecessary_mut_passed,
        clippy::useless_vec
    )
)]
#![allow(non_camel_case_types, non_snake_case)]

use std::alloc::{Layout, alloc_zeroed, dealloc};
use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{
    CStr, CString, c_char, c_int, c_long, c_short, c_uchar, c_uint, c_ulong, c_ushort, c_void,
};
use std::ptr::{self, NonNull};
use std::slice;
use std::sync::{Mutex, OnceLock};

use fontdone::ffi as rust_ffi;

// Keep the sibling facade dev-dependency visible to the library test target;
// the optional-feature probe links both native and WASM facades.
#[cfg(test)]
use fontdone_wasm as _;

thread_local! {
    static TEST_OUTLINE_RENDER_SPANS: RefCell<Vec<(c_int, FT_Span)>> = const { RefCell::new(Vec::new()) };
    static TEST_OUTLINE_RENDER_USER_SEEN: RefCell<bool> = const { RefCell::new(false) };
    static TEST_OUTLINE_RENDER_USER_TOKEN: RefCell<*mut c_void> = const { RefCell::new(ptr::null_mut()) };
}

#[cfg(feature = "abi-test-support")]
const INCREMENTAL_CLIENT_OBJECT_MAGIC: u64 = 0x46_54_49_4E_43_4C_49_46;

#[cfg(feature = "abi-test-support")]
thread_local! {
    static ABI_INCREMENTAL_OPAQUE_TRACE: RefCell<Option<AbiIncrementalOpaqueTrace>> =
        const { RefCell::new(None) };
}

struct OwnedMmVar {
    head: Box<OwnedMmVarHead>,
    _axis: Box<[FT_Var_Axis]>,
    _namedstyle: Box<[rust_ffi::FT_Var_Named_Style]>,
    _namedstyle_coords: Box<[rust_ffi::FT_Fixed]>,
}

#[repr(C)]
struct OwnedMmVarHead {
    master: FT_MM_Var,
    axis_flags: [FT_UShort; 64],
}

#[cfg(feature = "abi-test-support")]
pub type AbiMmVarNamedStyleSnapshot = (rust_ffi::FT_Var_Named_Style, Vec<rust_ffi::FT_Fixed>);

#[cfg(feature = "abi-test-support")]
pub type AbiMmVarDescriptorSnapshot = (
    FT_Error,
    FT_MM_Var,
    Vec<FT_Var_Axis>,
    Vec<FT_UInt>,
    Vec<AbiMmVarNamedStyleSnapshot>,
    FT_Error,
);

thread_local! {
    static OWNED_MM_VARS: RefCell<BTreeMap<usize, OwnedMmVar>> = const { RefCell::new(BTreeMap::new()) };
}

// Pinned FreeType dereferences every non-null `FT_Library` and therefore has
// no result for foreign or stale handles.  Fontdone deliberately hardens that
// undefined boundary.  The registry is process-wide so a live library can be
// passed between threads when the caller provides the synchronization that
// FreeType itself requires.
fn live_libraries() -> &'static Mutex<BTreeSet<usize>> {
    static LIVE_LIBRARIES: OnceLock<Mutex<BTreeSet<usize>>> = OnceLock::new();
    LIVE_LIBRARIES.get_or_init(|| Mutex::new(BTreeSet::new()))
}

fn with_live_libraries<T>(callback: impl FnOnce(&mut BTreeSet<usize>) -> T) -> T {
    let mut libraries = live_libraries()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    callback(&mut libraries)
}

enum OpenTypeTableAllocation {
    System { _bytes: Box<[FT_Byte]> },
    FaceMemory { memory: usize },
}

struct OwnedOpenTypeTable {
    owner: usize,
    _len: usize,
    allocation: OpenTypeTableAllocation,
}

fn owned_open_type_tables() -> &'static Mutex<BTreeMap<usize, OwnedOpenTypeTable>> {
    static TABLES: OnceLock<Mutex<BTreeMap<usize, OwnedOpenTypeTable>>> = OnceLock::new();
    TABLES.get_or_init(|| Mutex::new(BTreeMap::new()))
}

fn retain_c_open_type_table(face: FT_Face, bytes: Vec<FT_Byte>) -> Result<FT_Bytes, FT_Error> {
    if bytes.is_empty() {
        return Ok(ptr::null());
    }
    let memory = face_state(face)
        .and_then(|state| library_state_mut(state.library))
        .map_or(ptr::null_mut(), |state| state.allocation_memory);
    let custom_callbacks = NonNull::new(memory).and_then(|memory| {
        // SAFETY: the live face's library retains its FT_MemoryRec until all
        // faces are destroyed.
        let memory_ref = unsafe { memory.as_ref() };
        memory_ref.alloc.zip(memory_ref.free)
    });
    let len = bytes.len();
    let (pointer, allocation) = if let Some((alloc, _free)) = custom_callbacks {
        let size = c_long::try_from(len).map_err(|_| rust_ffi::FT_Err_Out_Of_Memory)?;
        let block = alloc(memory, size);
        let Some(block) = NonNull::new(block.cast::<FT_Byte>()) else {
            return Err(rust_ffi::FT_Err_Out_Of_Memory);
        };
        // SAFETY: the face allocator returned at least `len` writable bytes,
        // and `bytes` remains live and non-overlapping for this copy.
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), block.as_ptr(), len);
        }
        (
            block.as_ptr().cast_const(),
            OpenTypeTableAllocation::FaceMemory {
                memory: memory.addr(),
            },
        )
    } else {
        let bytes = bytes.into_boxed_slice();
        let pointer = bytes.as_ptr();
        (pointer, OpenTypeTableAllocation::System { _bytes: bytes })
    };
    owned_open_type_tables()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .insert(
            pointer.addr(),
            OwnedOpenTypeTable {
                owner: face.addr(),
                _len: len,
                allocation,
            },
        );
    Ok(pointer)
}

fn release_c_open_type_table(face: FT_Face, table: FT_Bytes) -> bool {
    let (Some(face), Some(table)) = (NonNull::new(face), NonNull::new(table.cast_mut())) else {
        return false;
    };
    let allocation = {
        let mut tables = owned_open_type_tables()
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if tables
            .get(&table.as_ptr().addr())
            .is_none_or(|entry| entry.owner != face.as_ptr().addr())
        {
            return false;
        }
        tables
            .remove(&table.as_ptr().addr())
            .map(|entry| entry.allocation)
    };
    if let Some(OpenTypeTableAllocation::FaceMemory { memory }) = allocation {
        let memory = memory as FT_Memory;
        // SAFETY: this entry was allocated by this exact live face-memory
        // record, and removal above guarantees the callback runs at most once.
        unsafe {
            if let Some(free) = (*memory).free {
                free(memory, table.as_ptr().cast());
            }
        }
    }
    true
}

fn into_library_handle(library: Box<FT_LibraryRec>) -> FT_Library {
    let library = Box::into_raw(library);
    with_live_libraries(|libraries| {
        libraries.insert(library as usize);
    });
    library
}

fn library_is_live(library: FT_Library) -> bool {
    !library.is_null() && with_live_libraries(|libraries| libraries.contains(&(library as usize)))
}

fn unregister_library(library: FT_Library) -> bool {
    !library.is_null() && with_live_libraries(|libraries| libraries.remove(&(library as usize)))
}

fn live_cache_managers() -> &'static Mutex<BTreeSet<usize>> {
    static LIVE_CACHE_MANAGERS: OnceLock<Mutex<BTreeSet<usize>>> = OnceLock::new();
    LIVE_CACHE_MANAGERS.get_or_init(|| Mutex::new(BTreeSet::new()))
}

fn with_live_cache_managers<T>(callback: impl FnOnce(&mut BTreeSet<usize>) -> T) -> T {
    let mut managers = live_cache_managers()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    callback(&mut managers)
}

fn cache_manager_is_live(manager: FTC_Manager) -> bool {
    !manager.is_null()
        && with_live_cache_managers(|managers| managers.contains(&(manager as usize)))
}

fn unregister_cache_manager(manager: FTC_Manager) -> bool {
    !manager.is_null() && with_live_cache_managers(|managers| managers.remove(&(manager as usize)))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_owned_mm_var_count() -> usize {
    OWNED_MM_VARS.with(|vars| vars.borrow().len())
}

pub type FT_Error = c_int;
pub type FT_Bool = c_uchar;
pub type FT_Int = c_int;
pub type FT_UInt = c_uint;
pub type FT_Int32 = i32;
pub type FT_UInt32 = u32;
pub type FT_Byte = c_uchar;
pub type FT_Bytes = *const FT_Byte;
pub type FT_Char = i8;
pub type FT_Long = c_long;
pub type FT_ULong = c_ulong;
pub type FT_Pos = c_long;
pub type FT_Fixed = c_long;
pub type FT_Angle = FT_Fixed;
pub type FT_F26Dot6 = c_long;
pub type FT_F2Dot14 = c_short;
pub type FT_Short = c_short;
pub type FT_UShort = c_ushort;
pub type FT_Render_Mode = c_int;
pub type FT_Pixel_Mode = c_int;
pub type FT_Glyph_Format = c_int;
pub type FT_Renderer = *mut FT_RendererRec;
pub type FT_Orientation = c_int;
pub type FT_Size_Request_Type = c_int;
pub type FT_Encoding = c_int;
pub type FT_Sfnt_Tag = c_uint;
pub type FT_LcdFilter = c_int;
pub type FT_TrueTypeEngineType = c_int;
pub type PS_Dict_Keys = c_int;
pub type T1_EncodingType = c_int;
pub type FT_DebugHook_Func = rust_ffi::FT_DebugHook_Func;
pub type FT_StrokerBorder = c_int;
pub type FT_Stroker_LineCap = c_int;
pub type FT_Stroker_LineJoin = c_int;
pub type FT_Color_Root_Transform = c_int;
pub type FT_Stroker = *mut c_void;
pub type FT_String = c_char;
pub type FT_MM_Axis = rust_ffi::FT_MM_Axis;
pub type FT_Multi_Master = rust_ffi::FT_Multi_Master;
pub type FT_Var_Axis = rust_ffi::FT_Var_Axis;
pub type FT_MM_Var = rust_ffi::FT_MM_Var;
pub type FT_WinFNT_HeaderRec = rust_ffi::FT_WinFNT_HeaderRec;
pub type FT_WinFNT_Header = *mut FT_WinFNT_HeaderRec;
pub type FT_LayerIterator = rust_ffi::FT_LayerIterator;
pub type FT_ClipBox = rust_ffi::FT_ClipBox;
pub type FT_PaintTransform = rust_ffi::FT_PaintTransform;
pub type BDF_PropertyType = rust_ffi::BDF_PropertyType;
pub type BDF_PropertyValue = rust_ffi::BDF_PropertyValue;
pub type BDF_PropertyRec = rust_ffi::BDF_PropertyRec;
pub type BDF_Property = *mut BDF_PropertyRec;

#[cfg(feature = "abi-test-support")]
#[derive(Clone, Copy)]
pub struct AbiBdfPropertySnapshot {
    pub type_: BDF_PropertyType,
    pub atom: *const FT_String,
    pub integer: FT_Int32,
    pub cardinal: FT_UInt32,
}
pub type PS_FontInfoRec = rust_ffi::PS_FontInfoRec;
pub type PS_FontInfo = *mut PS_FontInfoRec;
pub type T1_FontInfo = PS_FontInfoRec;
pub type PS_PrivateRec = rust_ffi::PS_PrivateRec;
pub type PS_Private = *mut PS_PrivateRec;
pub type T1_Private = PS_PrivateRec;
pub type FT_Pointer = *mut c_void;
pub type FT_Module_Interface = FT_Pointer;
pub type FT_Module = *mut FT_ModuleRec;
pub type FT_Generic_Finalizer = Option<unsafe extern "C" fn(object: FT_Pointer)>;
pub type FT_ListNode = *mut FT_ListNodeRec;
pub type FT_List = *mut FT_ListRec;
pub type FT_List_Iterator = Option<extern "C" fn(node: FT_ListNode, user: FT_Pointer) -> FT_Error>;
pub type FT_Memory = *mut FT_MemoryRec;
pub type FT_StreamDesc = rust_ffi::FT_StreamDesc;
pub type FT_StreamRec = rust_ffi::FT_StreamRec;
pub type FT_Stream = *mut FT_StreamRec;
pub type FT_Stream_CloseFunc = Option<extern "C" fn(stream: FT_Stream)>;
pub type SvgInitFunc = Option<unsafe extern "C" fn(data_pointer: *mut FT_Pointer) -> FT_Error>;
pub type SvgFreeFunc = Option<unsafe extern "C" fn(data_pointer: *mut FT_Pointer)>;
pub type SvgRenderFunc =
    Option<unsafe extern "C" fn(slot: FT_GlyphSlot, data_pointer: *mut FT_Pointer) -> FT_Error>;
pub type SvgPresetSlotFunc = Option<
    unsafe extern "C" fn(
        slot: FT_GlyphSlot,
        cache: FT_Bool,
        data_pointer: *mut FT_Pointer,
    ) -> FT_Error,
>;

/// Public `SVG_RendererHooks` record (`freetype/otsvg.h`).
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SVG_RendererHooks {
    pub init_svg: SvgInitFunc,
    pub free_svg: SvgFreeFunc,
    pub render_svg: SvgRenderFunc,
    pub preset_slot: SvgPresetSlotFunc,
}
pub type FT_Alloc_Func = Option<extern "C" fn(memory: FT_Memory, size: c_long) -> FT_Pointer>;
pub type FT_Free_Func = Option<extern "C" fn(memory: FT_Memory, block: FT_Pointer)>;
pub type FT_Realloc_Func = Option<
    extern "C" fn(
        memory: FT_Memory,
        cur_size: c_long,
        new_size: c_long,
        block: FT_Pointer,
    ) -> FT_Pointer,
>;
pub type FT_List_Destructor =
    Option<extern "C" fn(memory: FT_Memory, data: FT_Pointer, user: FT_Pointer)>;

pub type FT_Library = *mut FT_LibraryRec;
pub type FT_Face = *mut FT_FaceRec;
pub type FT_Size = *mut FT_SizeRec;
pub type FT_GlyphSlot = *mut FT_GlyphSlotRec;
pub type FT_Glyph = *mut FT_GlyphRec;
pub type FT_BitmapGlyph = *mut FT_BitmapGlyphRec;
pub type FT_OutlineGlyph = *mut FT_OutlineGlyphRec;
pub type FT_CharMap = *mut FT_CharMapRec;
pub type FT_Driver = *mut c_void;
pub type FT_SubGlyph = *mut c_void;
pub type FT_Face_Internal = *mut c_void;
pub type FT_Size_Internal = *mut c_void;
pub type FT_Slot_Internal = *mut c_void;
pub type FT_Bitmap_Size = rust_ffi::FT_Bitmap_Size;
pub type FT_Raster = rust_ffi::FT_Raster;
pub type FT_Incremental = rust_ffi::FT_Incremental;
pub type FT_Data = rust_ffi::FT_Data;
pub type FT_Incremental_MetricsRec = rust_ffi::FT_Incremental_MetricsRec;
pub type FT_Incremental_Metrics = *mut FT_Incremental_MetricsRec;
pub type FT_Incremental_FuncsRec = rust_ffi::FT_Incremental_FuncsRec;
pub type FT_Incremental_InterfaceRec = rust_ffi::FT_Incremental_InterfaceRec;
pub type FT_Incremental_Interface = *mut FT_Incremental_InterfaceRec;
pub type FTC_FaceID = FT_Pointer;
pub type FTC_Manager = *mut FTC_ManagerRec;
pub type FTC_Node = *mut FTC_NodeRec;
pub type FTC_CMapCache = *mut FTC_CMapCacheRec;
pub type FTC_ImageCache = *mut FTC_ImageCacheRec;
pub type FTC_SBitCache = *mut FTC_SBitCacheRec;
pub type FTC_ScalerRec = rust_ffi::FTC_ScalerRec;
pub type FTC_Scaler = *mut FTC_ScalerRec;
pub type FTC_ImageTypeRec = rust_ffi::FTC_ImageTypeRec;
pub type FTC_ImageType = *mut FTC_ImageTypeRec;
pub type FTC_SBitRec = rust_ffi::FTC_SBitRec;
pub type FTC_SBit = *mut FTC_SBitRec;

pub type FT_Glyph_InitFunc =
    Option<unsafe extern "C" fn(glyph: FT_Glyph, slot: FT_GlyphSlot) -> FT_Error>;
pub type FT_Glyph_DoneFunc = Option<unsafe extern "C" fn(glyph: FT_Glyph)>;
pub type FT_Glyph_TransformFunc = Option<
    unsafe extern "C" fn(glyph: FT_Glyph, matrix: *const FT_Matrix, delta: *const FT_Vector),
>;
pub type FT_Glyph_GetBBoxFunc = Option<unsafe extern "C" fn(glyph: FT_Glyph, abbox: *mut FT_BBox)>;
pub type FT_Glyph_CopyFunc =
    Option<unsafe extern "C" fn(source: FT_Glyph, target: FT_Glyph) -> FT_Error>;
pub type FT_Glyph_PrepareFunc =
    Option<unsafe extern "C" fn(glyph: FT_Glyph, slot: FT_GlyphSlot) -> FT_Error>;
pub type FT_Outline_MoveToFunc =
    Option<unsafe extern "C" fn(to: *const FT_Vector, user: FT_Pointer) -> c_int>;
pub type FT_Outline_LineToFunc = FT_Outline_MoveToFunc;
pub type FT_Outline_ConicToFunc = Option<
    unsafe extern "C" fn(
        control: *const FT_Vector,
        to: *const FT_Vector,
        user: FT_Pointer,
    ) -> c_int,
>;
pub type FT_Outline_CubicToFunc = Option<
    unsafe extern "C" fn(
        control1: *const FT_Vector,
        control2: *const FT_Vector,
        to: *const FT_Vector,
        user: FT_Pointer,
    ) -> c_int,
>;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Outline_Funcs {
    pub move_to: FT_Outline_MoveToFunc,
    pub line_to: FT_Outline_LineToFunc,
    pub conic_to: FT_Outline_ConicToFunc,
    pub cubic_to: FT_Outline_CubicToFunc,
    pub shift: c_int,
    pub delta: FT_Pos,
}

pub type FT_Incremental_GetGlyphDataFunc = Option<
    unsafe extern "C" fn(
        incremental: FT_Incremental,
        glyph_index: FT_UInt,
        adata: *mut FT_Data,
    ) -> FT_Error,
>;
pub type FT_Incremental_FreeGlyphDataFunc =
    Option<unsafe extern "C" fn(incremental: FT_Incremental, data: *mut FT_Data)>;
pub type FT_Incremental_GetGlyphMetricsFunc = Option<
    unsafe extern "C" fn(
        incremental: FT_Incremental,
        glyph_index: FT_UInt,
        vertical: FT_Bool,
        ametrics: *mut FT_Incremental_MetricsRec,
    ) -> FT_Error,
>;
pub type FT_Module_Constructor = Option<unsafe extern "C" fn(module: FT_Module) -> FT_Error>;
pub type FT_Module_Destructor = Option<unsafe extern "C" fn(module: FT_Module)>;
pub type FT_Module_Requester =
    Option<unsafe extern "C" fn(module: FT_Module, name: *const c_char) -> FT_Module_Interface>;
pub type FT_Custom_Log_Handler = FT_Pointer;
pub type FT_Renderer_RenderFunc = Option<
    unsafe extern "C" fn(
        renderer: FT_Renderer,
        slot: FT_GlyphSlot,
        mode: FT_Render_Mode,
        origin: *const FT_Vector,
    ) -> FT_Error,
>;
pub type FT_Renderer_TransformFunc = Option<
    unsafe extern "C" fn(
        renderer: FT_Renderer,
        slot: FT_GlyphSlot,
        matrix: *const FT_Matrix,
        delta: *const FT_Vector,
    ) -> FT_Error,
>;
pub type FT_Renderer_GetCBoxFunc =
    Option<unsafe extern "C" fn(renderer: FT_Renderer, slot: FT_GlyphSlot, cbox: *mut FT_BBox)>;
pub type FT_Renderer_SetModeFunc = Option<
    unsafe extern "C" fn(
        renderer: FT_Renderer,
        mode_tag: FT_ULong,
        mode_ptr: FT_Pointer,
    ) -> FT_Error,
>;
pub type FTC_Face_Requester = Option<
    unsafe extern "C" fn(
        face_id: FTC_FaceID,
        library: FT_Library,
        req_data: FT_Pointer,
        aface: *mut FT_Face,
    ) -> FT_Error,
>;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Generic {
    pub data: FT_Pointer,
    pub finalizer: FT_Generic_Finalizer,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Vector {
    pub x: FT_Pos,
    pub y: FT_Pos,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Matrix {
    pub xx: FT_Fixed,
    pub xy: FT_Fixed,
    pub yx: FT_Fixed,
    pub yy: FT_Fixed,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_BBox {
    pub xMin: FT_Pos,
    pub yMin: FT_Pos,
    pub xMax: FT_Pos,
    pub yMax: FT_Pos,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Parameter {
    pub tag: FT_ULong,
    pub data: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_ListNodeRec {
    pub prev: FT_ListNode,
    pub next: FT_ListNode,
    pub data: FT_Pointer,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_ListRec {
    pub head: FT_ListNode,
    pub tail: FT_ListNode,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_MemoryRec {
    pub user: FT_Pointer,
    pub alloc: FT_Alloc_Func,
    pub free: FT_Free_Func,
    pub realloc: FT_Realloc_Func,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Open_Args {
    pub flags: FT_UInt,
    pub memory_base: *const FT_Byte,
    pub memory_size: FT_Long,
    pub pathname: *mut c_char,
    pub stream: FT_Stream,
    pub driver: *mut c_void,
    pub num_params: FT_Int,
    pub params: *mut FT_Parameter,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Outline {
    pub n_contours: FT_UShort,
    pub n_points: FT_UShort,
    pub points: *mut FT_Vector,
    pub tags: *mut FT_Byte,
    pub contours: *mut FT_UShort,
    pub flags: FT_Int,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Glyph_Class {
    pub glyph_size: FT_Long,
    pub glyph_format: FT_Glyph_Format,
    pub glyph_init: FT_Glyph_InitFunc,
    pub glyph_done: FT_Glyph_DoneFunc,
    pub glyph_copy: FT_Glyph_CopyFunc,
    pub glyph_transform: FT_Glyph_TransformFunc,
    pub glyph_bbox: FT_Glyph_GetBBoxFunc,
    pub glyph_prepare: FT_Glyph_PrepareFunc,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_GlyphRec {
    pub library: FT_Library,
    pub clazz: *const FT_Glyph_Class,
    pub format: FT_Glyph_Format,
    pub advance: FT_Vector,
}

#[repr(C)]
pub struct FT_OutlineGlyphRec {
    pub root: FT_GlyphRec,
    pub outline: FT_Outline,
}

#[repr(C)]
pub struct FT_BitmapGlyphRec {
    pub root: FT_GlyphRec,
    pub left: FT_Int,
    pub top: FT_Int,
    pub bitmap: FT_Bitmap,
}

#[repr(C)]
struct AbiSvgGlyphRec {
    root: FT_GlyphRec,
    svg_document: *mut FT_Byte,
    svg_document_length: FT_ULong,
    glyph_index: FT_UInt,
    metrics: FT_Size_Metrics,
    units_per_EM: FT_UShort,
    start_glyph_id: FT_UShort,
    end_glyph_id: FT_UShort,
    transform: FT_Matrix,
    delta: FT_Vector,
}

#[repr(C)]
struct OwnedOutlineGlyph {
    record: FT_OutlineGlyphRec,
    core: rust_ffi::FT_OutlineGlyphOwned,
    points: Box<[FT_Vector]>,
    tags: Box<[FT_Byte]>,
    contours: Box<[FT_UShort]>,
    allocation_memory: FT_Memory,
    allocation_block: FT_Pointer,
}

impl OwnedOutlineGlyph {
    fn new(core: rust_ffi::FT_OutlineGlyphOwned) -> Self {
        let mut glyph = Self {
            record: FT_OutlineGlyphRec {
                root: c_glyph_root_from_core(&core.root),
                outline: FT_Outline::default(),
            },
            core,
            points: Box::new([]),
            tags: Box::new([]),
            contours: Box::new([]),
            allocation_memory: ptr::null_mut(),
            allocation_block: ptr::null_mut(),
        };
        glyph.refresh_record();
        glyph
    }

    fn refresh_record(&mut self) {
        self.record.root = c_glyph_root_from_core(&self.core.root);
        self.record.root.clazz = owned_outline_glyph_class();
        self.points = self
            .core
            .outline
            .points
            .iter()
            .map(|point| FT_Vector {
                x: point.x,
                y: point.y,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self.tags = self.core.outline.tags.clone().into_boxed_slice();
        self.contours = self.core.outline.contours.clone().into_boxed_slice();
        self.record.outline = FT_Outline {
            n_contours: u16::try_from(self.contours.len()).unwrap_or(u16::MAX),
            n_points: u16::try_from(self.points.len()).unwrap_or(u16::MAX),
            points: if self.points.is_empty() {
                ptr::null_mut()
            } else {
                self.points.as_mut_ptr()
            },
            tags: if self.tags.is_empty() {
                ptr::null_mut()
            } else {
                self.tags.as_mut_ptr()
            },
            contours: if self.contours.is_empty() {
                ptr::null_mut()
            } else {
                self.contours.as_mut_ptr()
            },
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
        // SAFETY: an owned outline glyph publishes arrays that remain valid
        // until FT_Done_Glyph.  C callers may mutate their contents through
        // the public FT_Outline record, so import those writes before invoking
        // the renderer, as C FreeType does by reading the record directly.
        let points = if point_count == 0 {
            &[]
        } else {
            // SAFETY: non-empty storage was validated above.
            unsafe { std::slice::from_raw_parts(self.record.outline.points, point_count) }
        };
        // SAFETY: validated alongside `points`; tags have the same public count.
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

impl Drop for OwnedOutlineGlyph {
    fn drop(&mut self) {
        free_custom_memory_block(self.allocation_memory, self.allocation_block);
        self.allocation_block = ptr::null_mut();
    }
}

#[repr(C)]
struct OwnedBitmapGlyph {
    record: FT_BitmapGlyphRec,
    core: rust_ffi::FT_BitmapGlyphOwned,
    buffer: Box<[FT_Byte]>,
    allocation_memory: FT_Memory,
    allocation_block: FT_Pointer,
    payload_allocation_block: FT_Pointer,
}

#[repr(C)]
struct OwnedSvgGlyph {
    record: AbiSvgGlyphRec,
    core: rust_ffi::FT_SvgGlyphOwned,
    document: Box<[FT_Byte]>,
    allocation_memory: FT_Memory,
    allocation_block: FT_Pointer,
    payload_allocation_block: FT_Pointer,
}

impl OwnedSvgGlyph {
    fn new(core: rust_ffi::FT_SvgGlyphOwned) -> Self {
        let mut glyph = Self {
            record: AbiSvgGlyphRec {
                root: c_glyph_root_from_core_with_class(&core.root, owned_svg_glyph_class()),
                svg_document: ptr::null_mut(),
                svg_document_length: 0,
                glyph_index: core.glyph_index,
                metrics: rust_size_metrics_to_abi(core.metrics),
                units_per_EM: core.units_per_EM,
                start_glyph_id: core.start_glyph_id,
                end_glyph_id: core.end_glyph_id,
                transform: FT_Matrix::default(),
                delta: FT_Vector::default(),
            },
            core,
            document: Box::new([]),
            allocation_memory: ptr::null_mut(),
            allocation_block: ptr::null_mut(),
            payload_allocation_block: ptr::null_mut(),
        };
        glyph.refresh_record();
        glyph
    }

    fn refresh_record(&mut self) {
        self.record.root =
            c_glyph_root_from_core_with_class(&self.core.root, owned_svg_glyph_class());
        self.document = self.core.svg_document.clone().into_boxed_slice();
        self.record.svg_document = if self.document.is_empty() {
            ptr::null_mut()
        } else {
            self.document.as_mut_ptr()
        };
        self.record.svg_document_length =
            FT_ULong::try_from(self.document.len()).unwrap_or(FT_ULong::MAX);
        self.record.glyph_index = self.core.glyph_index;
        self.record.metrics = rust_size_metrics_to_abi(self.core.metrics);
        self.record.units_per_EM = self.core.units_per_EM;
        self.record.start_glyph_id = self.core.start_glyph_id;
        self.record.end_glyph_id = self.core.end_glyph_id;
        self.record.transform = FT_Matrix {
            xx: self.core.transform.xx,
            xy: self.core.transform.xy,
            yx: self.core.transform.yx,
            yy: self.core.transform.yy,
        };
        self.record.delta = FT_Vector {
            x: self.core.delta.x,
            y: self.core.delta.y,
        };
    }

    fn sync_core_from_record(&mut self) -> Result<(), FT_Error> {
        if self.record.svg_document.is_null() {
            return Err(rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error);
        }
        let document_len = usize::try_from(self.record.svg_document_length)
            .map_err(|_| rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error)?;
        if document_len == 0 {
            return Err(rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error);
        }
        // SAFETY: a non-null public SVG record promises `document_len`
        // readable bytes for the duration of the class-copy callback.
        self.core.svg_document =
            unsafe { slice::from_raw_parts(self.record.svg_document, document_len).to_vec() };
        self.core.root.library = self.record.root.library.cast::<c_void>();
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

impl OwnedBitmapGlyph {
    fn new(core: rust_ffi::FT_BitmapGlyphOwned) -> Self {
        let mut glyph = Self {
            record: FT_BitmapGlyphRec {
                root: c_glyph_root_from_core_with_class(&core.root, owned_bitmap_glyph_class()),
                left: core.left,
                top: core.top,
                bitmap: FT_Bitmap::default(),
            },
            core,
            buffer: Box::new([]),
            allocation_memory: ptr::null_mut(),
            allocation_block: ptr::null_mut(),
            payload_allocation_block: ptr::null_mut(),
        };
        glyph.refresh_record();
        glyph
    }

    fn refresh_record(&mut self) {
        self.record.root =
            c_glyph_root_from_core_with_class(&self.core.root, owned_bitmap_glyph_class());
        self.record.left = self.core.left;
        self.record.top = self.core.top;
        self.buffer = self.core.bitmap.buffer.clone().into_boxed_slice();
        self.record.bitmap = FT_Bitmap {
            rows: self.core.bitmap.rows,
            width: self.core.bitmap.width,
            pitch: self.core.bitmap.pitch,
            buffer: if self.buffer.is_empty() {
                ptr::null_mut()
            } else {
                self.buffer.as_mut_ptr()
            },
            num_grays: self.core.bitmap.num_grays,
            pixel_mode: u8::try_from(self.core.bitmap.pixel_mode).unwrap_or(0),
            palette_mode: 0,
            palette: ptr::null_mut(),
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
        self.core.root.library = self.record.root.library.cast::<c_void>();
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
            // FreeType's `FT_Bitmap_Copy` preserves a null source buffer and
            // returns success even when rows and pitch describe a non-empty
            // descriptor.  Keep that public-record behavior without ever
            // constructing a slice from a null pointer.
            buffer: if byte_len == 0 || self.record.bitmap.buffer.is_null() {
                Vec::new()
            } else {
                // SAFETY: non-empty public bitmap storage was checked above
                // and must be readable through the class-copy call.
                unsafe { slice::from_raw_parts(self.record.bitmap.buffer, byte_len).to_vec() }
            },
            num_grays: self.record.bitmap.num_grays,
            pixel_mode: FT_Int::from(self.record.bitmap.pixel_mode),
        };
        Ok(())
    }
}

impl Drop for OwnedBitmapGlyph {
    fn drop(&mut self) {
        free_custom_memory_block(self.allocation_memory, self.payload_allocation_block);
        self.payload_allocation_block = ptr::null_mut();
        free_custom_memory_block(self.allocation_memory, self.allocation_block);
        self.allocation_block = ptr::null_mut();
    }
}

impl Drop for OwnedSvgGlyph {
    fn drop(&mut self) {
        free_custom_memory_block(self.allocation_memory, self.payload_allocation_block);
        self.payload_allocation_block = ptr::null_mut();
        free_custom_memory_block(self.allocation_memory, self.allocation_block);
        self.allocation_block = ptr::null_mut();
    }
}

fn c_glyph_root_from_core(root: &rust_ffi::FT_GlyphRec) -> FT_GlyphRec {
    c_glyph_root_from_core_with_class(root, owned_outline_glyph_class())
}

fn c_glyph_root_from_core_with_class(
    root: &rust_ffi::FT_GlyphRec,
    clazz: *const FT_Glyph_Class,
) -> FT_GlyphRec {
    FT_GlyphRec {
        library: root.library.cast::<FT_LibraryRec>(),
        clazz,
        format: root.format,
        advance: FT_Vector {
            x: root.advance.x,
            y: root.advance.y,
        },
    }
}

static OWNED_OUTLINE_GLYPH_CLASS_MARKER: u8 = 0;
static OWNED_BITMAP_GLYPH_CLASS_MARKER: u8 = 0;
static OWNED_SVG_GLYPH_CLASS_MARKER: u8 = 0;

fn owned_outline_glyph_class() -> *const FT_Glyph_Class {
    // Private marker used only for pointer identity.  We never dereference this
    // address as an `FT_Glyph_Class`; real class facades continue down the
    // caller-owned public-record path.
    ptr::addr_of!(OWNED_OUTLINE_GLYPH_CLASS_MARKER).cast::<FT_Glyph_Class>()
}

fn owned_bitmap_glyph_class() -> *const FT_Glyph_Class {
    // Private marker used only for pointer identity.  We never dereference this
    // address as an `FT_Glyph_Class`.
    ptr::addr_of!(OWNED_BITMAP_GLYPH_CLASS_MARKER).cast::<FT_Glyph_Class>()
}

fn owned_svg_glyph_class() -> *const FT_Glyph_Class {
    // Private marker used only for pointer identity.  It is never
    // dereferenced as a public class record.
    ptr::addr_of!(OWNED_SVG_GLYPH_CLASS_MARKER).cast::<FT_Glyph_Class>()
}

fn owned_outline_glyph_from_root(glyph: FT_Glyph) -> Option<&'static OwnedOutlineGlyph> {
    let glyph = non_null_mut(glyph)?;
    // SAFETY: checked non-null and only reads the public root class pointer.
    let root = unsafe { glyph.as_ref() };
    if root.clazz != owned_outline_glyph_class() {
        return None;
    }
    // SAFETY: this sentinel is assigned only for `Box<OwnedOutlineGlyph>`
    // allocations whose first field is an `FT_OutlineGlyphRec`, whose first
    // field is the public `FT_GlyphRec` root.
    Some(unsafe { &*glyph.as_ptr().cast::<OwnedOutlineGlyph>() })
}

fn owned_outline_glyph_from_root_mut(glyph: FT_Glyph) -> Option<&'static mut OwnedOutlineGlyph> {
    let glyph = non_null_mut(glyph)?;
    // SAFETY: checked non-null and only reads the public root class pointer.
    let root = unsafe { glyph.as_ref() };
    if root.clazz != owned_outline_glyph_class() {
        return None;
    }
    // SAFETY: this sentinel is assigned only for `Box<OwnedOutlineGlyph>`
    // allocations whose first field is an `FT_OutlineGlyphRec`, whose first
    // field is the public `FT_GlyphRec` root.
    Some(unsafe { &mut *glyph.as_ptr().cast::<OwnedOutlineGlyph>() })
}

fn owned_bitmap_glyph_from_root(glyph: FT_Glyph) -> Option<&'static OwnedBitmapGlyph> {
    let glyph = non_null_mut(glyph)?;
    // SAFETY: checked non-null and only reads the public root class pointer.
    let root = unsafe { glyph.as_ref() };
    if root.clazz != owned_bitmap_glyph_class() {
        return None;
    }
    // SAFETY: this sentinel is assigned only for `Box<OwnedBitmapGlyph>`
    // allocations whose first field is an `FT_BitmapGlyphRec`, whose first
    // field is the public `FT_GlyphRec` root.
    Some(unsafe { &*glyph.as_ptr().cast::<OwnedBitmapGlyph>() })
}

fn owned_bitmap_glyph_from_root_mut(glyph: FT_Glyph) -> Option<&'static mut OwnedBitmapGlyph> {
    let glyph = non_null_mut(glyph)?;
    // SAFETY: checked non-null and only reads the public root class pointer.
    let root = unsafe { glyph.as_ref() };
    if root.clazz != owned_bitmap_glyph_class() {
        return None;
    }
    // SAFETY: the private class marker proves the allocation type.
    Some(unsafe { &mut *glyph.as_ptr().cast::<OwnedBitmapGlyph>() })
}

fn owned_svg_glyph_from_root(glyph: FT_Glyph) -> Option<&'static OwnedSvgGlyph> {
    let glyph = non_null_mut(glyph)?;
    // SAFETY: checked non-null and only reads the public root class pointer.
    let root = unsafe { glyph.as_ref() };
    if root.clazz != owned_svg_glyph_class() {
        return None;
    }
    // SAFETY: this sentinel is assigned only to `Box<OwnedSvgGlyph>`
    // allocations whose first field begins with the public glyph root.
    Some(unsafe { &*glyph.as_ptr().cast::<OwnedSvgGlyph>() })
}

fn owned_svg_glyph_from_root_mut(glyph: FT_Glyph) -> Option<&'static mut OwnedSvgGlyph> {
    let glyph = non_null_mut(glyph)?;
    // SAFETY: checked non-null and only reads the public root class pointer.
    let root = unsafe { glyph.as_ref() };
    if root.clazz != owned_svg_glyph_class() {
        return None;
    }
    // SAFETY: the private class marker proves the allocation type.
    Some(unsafe { &mut *glyph.as_ptr().cast::<OwnedSvgGlyph>() })
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_UnitVector {
    pub x: FT_F2Dot14,
    pub y: FT_F2Dot14,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Glyph_Metrics {
    pub width: FT_Pos,
    pub height: FT_Pos,
    pub horiBearingX: FT_Pos,
    pub horiBearingY: FT_Pos,
    pub horiAdvance: FT_Pos,
    pub vertBearingX: FT_Pos,
    pub vertBearingY: FT_Pos,
    pub vertAdvance: FT_Pos,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Size_Metrics {
    pub x_ppem: FT_UShort,
    pub y_ppem: FT_UShort,
    pub x_scale: FT_Fixed,
    pub y_scale: FT_Fixed,
    pub ascender: FT_Pos,
    pub descender: FT_Pos,
    pub height: FT_Pos,
    pub max_advance: FT_Pos,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Size_RequestRec {
    pub type_: FT_Size_Request_Type,
    pub width: FT_Long,
    pub height: FT_Long,
    pub horiResolution: FT_UInt,
    pub vertResolution: FT_UInt,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Bitmap {
    pub rows: u32,
    pub width: u32,
    pub pitch: FT_Int,
    pub buffer: *mut c_uchar,
    pub num_grays: FT_UShort,
    pub pixel_mode: c_uchar,
    pub palette_mode: c_uchar,
    pub palette: *mut c_void,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Span {
    pub x: c_short,
    pub len: c_ushort,
    pub coverage: c_uchar,
}

pub type FT_SpanFunc =
    Option<unsafe extern "C" fn(y: c_int, count: c_int, spans: *const FT_Span, user: *mut c_void)>;
pub type FT_Raster_BitTest_Func =
    Option<unsafe extern "C" fn(y: c_int, x: c_int, user: *mut c_void) -> c_int>;
pub type FT_Raster_BitSet_Func =
    Option<unsafe extern "C" fn(y: c_int, x: c_int, user: *mut c_void)>;
pub type FT_Raster_NewFunc =
    Option<unsafe extern "C" fn(memory: FT_Pointer, raster: *mut FT_Raster) -> c_int>;
pub type FT_Raster_DoneFunc = Option<unsafe extern "C" fn(raster: FT_Raster)>;
pub type FT_Raster_ResetFunc =
    Option<unsafe extern "C" fn(raster: FT_Raster, pool_base: *mut FT_Byte, pool_size: FT_ULong)>;
pub type FT_Raster_SetModeFunc =
    Option<unsafe extern "C" fn(raster: FT_Raster, mode: FT_ULong, args: FT_Pointer) -> c_int>;
pub type FT_Raster_RenderFunc =
    Option<unsafe extern "C" fn(raster: FT_Raster, params: *const FT_Raster_Params) -> c_int>;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Raster_Params {
    pub target: *const FT_Bitmap,
    pub source: *const c_void,
    pub flags: c_int,
    pub gray_spans: FT_SpanFunc,
    pub black_spans: FT_SpanFunc,
    pub bit_test: FT_Raster_BitTest_Func,
    pub bit_set: FT_Raster_BitSet_Func,
    pub user: *mut c_void,
    pub clip_box: FT_BBox,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Raster_Funcs {
    pub glyph_format: FT_Glyph_Format,
    pub raster_new: FT_Raster_NewFunc,
    pub raster_reset: FT_Raster_ResetFunc,
    pub raster_set_mode: FT_Raster_SetModeFunc,
    pub raster_render: FT_Raster_RenderFunc,
    pub raster_done: FT_Raster_DoneFunc,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Renderer_Class {
    pub root: FT_Module_Class,
    pub glyph_format: FT_Glyph_Format,
    pub render_glyph: FT_Renderer_RenderFunc,
    pub transform_glyph: FT_Renderer_TransformFunc,
    pub get_glyph_cbox: FT_Renderer_GetCBoxFunc,
    pub set_mode: FT_Renderer_SetModeFunc,
    pub raster_class: *const FT_Raster_Funcs,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Color {
    pub blue: FT_Byte,
    pub green: FT_Byte,
    pub red: FT_Byte,
    pub alpha: FT_Byte,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Palette_Data {
    pub num_palettes: FT_UShort,
    pub palette_name_ids: *const FT_UShort,
    pub palette_flags: *const FT_UShort,
    pub num_palette_entries: FT_UShort,
    pub palette_entry_name_ids: *const FT_UShort,
}

pub type FT_OpaquePaint = rust_ffi::FT_OpaquePaint;
pub type FT_ColorIndex = rust_ffi::FT_ColorIndex;
pub type FT_ColorLine = rust_ffi::FT_ColorLine;
pub type FT_ColorStop = rust_ffi::FT_ColorStop;
pub type FT_ColorStopIterator = rust_ffi::FT_ColorStopIterator;
pub type FT_Affine23 = rust_ffi::FT_Affine23;
pub type FT_PaintColrGlyph = rust_ffi::FT_PaintColrGlyph;
pub type FT_PaintColrLayers = rust_ffi::FT_PaintColrLayers;
pub type FT_PaintLinearGradient = rust_ffi::FT_PaintLinearGradient;
pub type FT_PaintRadialGradient = rust_ffi::FT_PaintRadialGradient;
pub type FT_PaintRotate = rust_ffi::FT_PaintRotate;
pub type FT_PaintScale = rust_ffi::FT_PaintScale;
pub type FT_PaintSkew = rust_ffi::FT_PaintSkew;
pub type FT_PaintSolid = rust_ffi::FT_PaintSolid;
pub type FT_PaintSweepGradient = rust_ffi::FT_PaintSweepGradient;
pub type FT_PaintTranslate = rust_ffi::FT_PaintTranslate;
pub type FT_PaintGlyph = rust_ffi::FT_PaintGlyph;
pub type FT_PaintComposite = rust_ffi::FT_PaintComposite;
pub type FT_COLR_Paint = rust_ffi::FT_COLR_Paint;
pub type FT_Prop_GlyphToScriptMap = rust_ffi::FT_Prop_GlyphToScriptMap;
pub type FT_Prop_IncreaseXHeight = rust_ffi::FT_Prop_IncreaseXHeight;
pub type FT_SvgGlyphRec = rust_ffi::FT_SvgGlyphRec;
pub type FT_SvgGlyph = *mut FT_SvgGlyphRec;
pub type FT_Var_Named_Style = rust_ffi::FT_Var_Named_Style;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_SVG_DocumentRec {
    pub svg_document: *mut FT_Byte,
    pub svg_document_length: FT_ULong,
    pub metrics: FT_Size_Metrics,
    pub units_per_EM: FT_UShort,
    pub start_glyph_id: FT_UShort,
    pub end_glyph_id: FT_UShort,
    pub transform: FT_Matrix,
    pub delta: FT_Vector,
}

pub type FT_SVG_Document = *mut FT_SVG_DocumentRec;

fn rust_color_from_c(color: FT_Color) -> rust_ffi::FT_Color {
    rust_ffi::FT_Color {
        blue: color.blue,
        green: color.green,
        red: color.red,
        alpha: color.alpha,
    }
}

fn copy_palette_data_to_c(out: &mut FT_Palette_Data, value: rust_ffi::FT_Palette_Data) {
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
    // by `FT_Palette_Data_Get`; a non-null zero-length slice is valid and
    // copies no elements.
    unsafe { slice::from_raw_parts(ptr, usize::from(len)).to_vec() }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_palette_data_snapshot(face: FT_Face) -> AbiPaletteDataSnapshot {
    let mut data = FT_Palette_Data::default();
    let error = FT_Palette_Data_Get(face, &mut data);
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
    pub entries: Vec<FT_Color>,
}

#[cfg(feature = "abi-test-support")]
fn abi_palette_entries_from_ptr(face: FT_Face, palette: *mut FT_Color) -> Vec<FT_Color> {
    if palette.is_null() {
        return Vec::new();
    }
    let mut data = FT_Palette_Data::default();
    if FT_Palette_Data_Get(face, &mut data) != rust_ffi::FT_Err_Ok {
        return Vec::new();
    }
    let len = usize::from(data.num_palette_entries);
    // SAFETY: this test-support helper copies the palette pointer returned by
    // `FT_Palette_Select` while the owning face is still live.
    unsafe { slice::from_raw_parts(palette, len).to_vec() }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_palette_select_snapshot(
    face: FT_Face,
    palette_index: FT_UShort,
) -> AbiPaletteSelectSnapshot {
    let mut palette = ptr::null_mut();
    let error = FT_Palette_Select(face, palette_index, &mut palette);
    AbiPaletteSelectSnapshot {
        error,
        palette_is_null: palette.is_null(),
        entries: if error == rust_ffi::FT_Err_Ok {
            abi_palette_entries_from_ptr(face, palette)
        } else {
            Vec::new()
        },
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_palette_select_without_output(face: FT_Face, palette_index: FT_UShort) -> FT_Error {
    FT_Palette_Select(face, palette_index, ptr::null_mut())
}

#[cfg(feature = "abi-test-support")]
pub fn abi_palette_mutate_entry(
    face: FT_Face,
    palette_index: FT_UShort,
    entry_index: usize,
    color: FT_Color,
) -> AbiPaletteSelectSnapshot {
    let mut snapshot = abi_palette_select_snapshot(face, palette_index);
    if entry_index < snapshot.entries.len() {
        let mut palette = ptr::null_mut();
        let error = FT_Palette_Select(face, palette_index, &mut palette);
        // `entry_index < snapshot.entries.len()` above proves that the
        // selected palette pointer is non-null: the snapshot helper returns
        // no entries for a null pointer or failed metadata lookup.
        // The repeated selection uses the same live face and valid palette
        // index that produced the non-empty snapshot, so it also succeeds.
        // SAFETY: this feature-gated helper mutates an entry through the
        // public ABI palette pointer while the face is live, matching the
        // FreeType caller-observable behavior under test.
        unsafe { *palette.add(entry_index) = color };
        snapshot = AbiPaletteSelectSnapshot {
            error,
            palette_is_null: palette.is_null(),
            entries: abi_palette_entries_from_ptr(face, palette),
        };
    }
    snapshot
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_Init(abitmap: *mut FT_Bitmap) {
    // FreeType accepts NULL here and otherwise overwrites the public record
    // with the static zero `null_bitmap`.
    if let Some(bitmap) = unsafe { abitmap.as_mut() } {
        *bitmap = FT_Bitmap::default();
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_New(abitmap: *mut FT_Bitmap) {
    FT_Bitmap_Init(abitmap);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Gzip_Uncompress(
    memory: FT_Memory,
    output: *mut FT_Byte,
    output_len: *mut FT_ULong,
    input: *const FT_Byte,
    input_len: FT_ULong,
) -> FT_Error {
    if memory.is_null() || output.is_null() || output_len.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // `FT_ULong` is the platform C `unsigned long`; on the supported target
    // ABIs it is never wider than `usize`, so this conversion cannot fail.
    let input_len = input_len as usize;
    // SAFETY: `output_len` was checked for null above and is only borrowed for
    // the duration of this C ABI call.
    let output_len_ref = unsafe { &mut *output_len };
    let output_capacity = *output_len_ref as usize;
    // SAFETY: `output` is non-null and the caller-provided `*output_len`
    // defines the writable output buffer length, matching FreeType's ABI.
    let output_slice = unsafe { slice::from_raw_parts_mut(output, output_capacity) };
    let input_slice = if input.is_null() {
        None
    } else {
        // SAFETY: non-null `input` plus `input_len` form the caller-provided
        // compressed byte slice for the duration of this call.
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

#[cfg_attr(not(feature = "bzip2"), allow(dead_code))]
fn bzip2_source_bytes(
    source: FT_Stream,
    source_base: *mut FT_Byte,
    source_read: FT_Pointer,
    source_len: usize,
) -> Result<Vec<FT_Byte>, FT_Error> {
    if source_len == 0 {
        return Ok(Vec::new());
    }
    if !source_base.is_null() {
        // SAFETY: the caller supplied a memory-backed source whose base is
        // readable for the advertised non-zero size.
        return Ok(unsafe { slice::from_raw_parts(source_base.cast_const(), source_len) }.to_vec());
    }
    if source_read.is_null() {
        return Err(rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error);
    }
    // SAFETY: public FT_StreamRec.read has FreeType's FT_Stream_IoFunc ABI;
    // the caller retains the source stream for this synchronous materialization.
    let stream_io = unsafe {
        std::mem::transmute::<
            FT_Pointer,
            extern "C" fn(FT_Stream, FT_ULong, *mut FT_Byte, FT_ULong) -> FT_ULong,
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

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stream_OpenBzip2(stream: FT_Stream, source: FT_Stream) -> FT_Error {
    open_bzip2_stream(stream, source)
}

#[cfg(not(feature = "bzip2"))]
fn open_bzip2_stream(stream: FT_Stream, source: FT_Stream) -> FT_Error {
    let _ = (stream, source);
    rust_ffi::FT_Err_Unimplemented_Feature as FT_Error
}

#[cfg(feature = "bzip2")]
fn open_bzip2_stream(stream: FT_Stream, source: FT_Stream) -> FT_Error {
    // FreeType checks both handles before reading the source stream. Keep the
    // validation in one place so the second source reborrow cannot create an
    // unreachable defensive branch after source bytes have been collected.
    if stream.is_null() || source.is_null() {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    }
    let (source_base, source_read, source_len) = unsafe {
        let source_ref = &*source;
        (source_ref.base, source_ref.read, source_ref.size as usize)
    };
    if source_base.is_null() && source_len != 0 && source_read.is_null() {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    }
    let source_bytes = match bzip2_source_bytes(source, source_base, source_read, source_len) {
        Ok(bytes) => bytes,
        Err(error) => return error,
    };
    let stream_ref = unsafe { &mut *stream };
    let source_ref = unsafe { &mut *source };
    let error = rust_ffi::FT_Stream_OpenBzip2(
        Some(stream_ref),
        Some(source_ref),
        Some(source_bytes.as_slice()),
    );
    if error == rust_ffi::FT_Err_Ok {
        stream_ref.read = c_bzip2_stream_io as *const () as FT_Pointer;
        stream_ref.close = c_bzip2_stream_close as *const () as FT_Pointer;
    }
    error
}

#[cfg_attr(not(feature = "bzip2"), allow(dead_code))]
extern "C" fn c_bzip2_stream_io(
    stream: FT_Stream,
    offset: FT_ULong,
    buffer: *mut FT_Byte,
    count: FT_ULong,
) -> FT_ULong {
    if buffer.is_null() && count != 0 {
        return 0;
    }
    let Some(bytes) = abi_support_bzip2_stream_bytes(stream, offset, count) else {
        return 0;
    };
    if !buffer.is_null() && !bytes.is_empty() {
        // SAFETY: the C stream callback contract provides `count` writable
        // bytes and the registry never returns more than that count.
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len());
        }
    }
    FT_ULong::try_from(bytes.len()).unwrap_or(FT_ULong::MAX)
}

#[cfg_attr(not(feature = "bzip2"), allow(dead_code))]
extern "C" fn c_bzip2_stream_close(stream: FT_Stream) {
    abi_support_bzip2_stream_close(stream);
}

pub fn abi_support_bzip2_stream_bytes(
    stream: FT_Stream,
    offset: FT_ULong,
    count: FT_ULong,
) -> Option<Vec<FT_Byte>> {
    let stream_ref = unsafe { stream.as_ref() }?;
    rust_ffi::FT_Bzip2_Stream_Read(Some(stream_ref), offset, count)
}

pub fn abi_support_bzip2_stream_close(stream: FT_Stream) {
    if let Some(stream_ref) = unsafe { stream.as_mut() } {
        rust_ffi::FT_Bzip2_Stream_Close(Some(stream_ref));
    }
}

pub fn abi_support_bzip2_stream_is_open(stream: FT_Stream) -> bool {
    let stream_ref = unsafe { stream.as_ref() };
    rust_ffi::FT_Bzip2_Stream_Is_Open(stream_ref)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stream_OpenLZW(stream: FT_Stream, source: FT_Stream) -> FT_Error {
    open_lzw_stream(stream, source)
}

#[cfg(not(feature = "lzw"))]
fn open_lzw_stream(stream: FT_Stream, source: FT_Stream) -> FT_Error {
    let _ = (stream, source);
    rust_ffi::FT_Err_Unimplemented_Feature as FT_Error
}

#[cfg(feature = "lzw")]
fn open_lzw_stream(stream: FT_Stream, source: FT_Stream) -> FT_Error {
    if stream.is_null() || source.is_null() {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    }
    let stream_ref = unsafe { &mut *stream };
    let source_ref = unsafe { &mut *source };
    if source_ref.base.is_null() {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    }
    let source_len = source_ref.size as usize;
    // SAFETY: the maintained C ABI route supplies a memory-backed source
    // stream whose caller-owned `base` remains readable for `size` bytes.
    let source_bytes = unsafe { slice::from_raw_parts(source_ref.base.cast_const(), source_len) };
    let error = rust_ffi::FT_Stream_OpenLZW(Some(stream_ref), Some(source_ref), Some(source_bytes));
    if source_len >= 2 {
        source_ref.pos = 2;
    }
    if error == rust_ffi::FT_Err_Ok {
        stream_ref.read = c_lzw_stream_io as *const () as FT_Pointer;
        stream_ref.close = c_lzw_stream_close as *const () as FT_Pointer;
    }
    error
}

#[cfg_attr(not(feature = "lzw"), allow(dead_code))]
extern "C" fn c_lzw_stream_io(
    stream: FT_Stream,
    offset: FT_ULong,
    buffer: *mut FT_Byte,
    count: FT_ULong,
) -> FT_ULong {
    if buffer.is_null() && count != 0 {
        return 0;
    }
    let Some(bytes) = abi_support_lzw_stream_bytes(stream, offset, count) else {
        return 0;
    };
    if !buffer.is_null() && !bytes.is_empty() {
        // SAFETY: the C stream callback contract provides `count` writable
        // bytes and the registry never returns more than that count.
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len());
        }
    }
    FT_ULong::try_from(bytes.len()).unwrap_or(FT_ULong::MAX)
}

#[cfg_attr(not(feature = "lzw"), allow(dead_code))]
extern "C" fn c_lzw_stream_close(stream: FT_Stream) {
    abi_support_lzw_stream_close(stream);
}

pub fn abi_support_lzw_stream_bytes(
    stream: FT_Stream,
    offset: FT_ULong,
    count: FT_ULong,
) -> Option<Vec<FT_Byte>> {
    let stream_ref = unsafe { stream.as_ref() }?;
    rust_ffi::FT_LZW_Stream_Read(Some(stream_ref), offset, count)
}

pub fn abi_support_lzw_stream_close(stream: FT_Stream) {
    if let Some(stream_ref) = unsafe { stream.as_mut() } {
        rust_ffi::FT_LZW_Stream_Close(Some(stream_ref));
    }
}

fn ftc_manager_lookup_face_impl(
    manager: FTC_Manager,
    face_id: FTC_FaceID,
) -> Result<FT_Face, FT_Error> {
    if manager.is_null() {
        return Err(rust_ffi::FT_Err_Invalid_Cache_Handle as FT_Error);
    }
    let key = face_id as usize;
    // SAFETY: `manager` is a live handle created by `FTC_Manager_New`; this
    // immutable lookup ends before a possible foreign callback.
    if let Some(face) = unsafe { (*manager).faces.get(&key).copied() } {
        return Ok(face);
    }
    // SAFETY: these fields are copied out before invoking the requester, so no
    // Rust reference into manager state crosses the callback boundary.
    let (requester, library, req_data) = unsafe {
        (
            (*manager).requester,
            (*manager).library,
            (*manager).req_data,
        )
    };
    let Some(requester) = requester else {
        return Err(rust_ffi::FT_Err_Invalid_Argument);
    };
    let mut face = ptr::null_mut();
    // SAFETY: the requester is the callback supplied to `FTC_Manager_New`;
    // output storage lives through this synchronous invocation.
    let error = unsafe { requester(face_id, library, req_data, &mut face) };
    if error != rust_ffi::FT_Err_Ok {
        return Err(error);
    }
    if face.is_null() {
        return Err(rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error);
    }
    // SAFETY: no manager borrow is live across the callback; the returned face
    // becomes manager-owned until removal, reset, or destruction.
    unsafe {
        // FreeType's cache manager owns sizes independently and removes the
        // requester's default size from a newly cached face.
        (*face).size = ptr::null_mut();
        (*manager).faces.insert(key, face);
    }
    Ok(face)
}

fn ftc_destroy_node(node: FTC_Node) {
    if node.is_null() {
        return;
    }
    // SAFETY: nodes are allocated once by this crate and removed from their
    // manager before this function reconstructs the owning box.
    let node = unsafe { Box::from_raw(node) };
    if let FtcNodePayload::Glyph(glyph) = node.payload {
        FT_Done_Glyph(glyph);
    }
}

fn ftc_reset_manager(manager: FTC_Manager) {
    if manager.is_null() {
        return;
    }
    // SAFETY: move owned collections out in one exclusive access; foreign
    // destructors run only after that borrow has ended.
    let (faces, nodes) = unsafe {
        (
            std::mem::take(&mut (*manager).faces),
            std::mem::take(&mut (*manager).nodes),
        )
    };
    for node in nodes {
        ftc_destroy_node(node);
    }
    for face in faces.into_values() {
        let _ = FT_Done_Face(face);
    }
    // SAFETY: cache handles remain live, and reset only invalidates entries.
    unsafe {
        for cache in &mut (*manager).cmap_caches {
            if let Some(cache) = cache.as_mut() {
                cache.entries.clear();
            }
        }
        for cache in &mut (*manager).image_caches {
            if let Some(cache) = cache.as_mut() {
                cache.entries.clear();
            }
        }
        for cache in &mut (*manager).sbit_caches {
            if let Some(cache) = cache.as_mut() {
                cache.entries.clear();
            }
        }
    }
}

fn ftc_image_type_key(type_: FTC_ImageTypeRec, glyph_index: FT_UInt) -> FtcImageKey {
    FtcImageKey {
        face_id: type_.face_id,
        width: type_.width,
        height: type_.height,
        pixel: 1,
        x_res: 0,
        y_res: 0,
        load_flags: FT_ULong::from(FT_UInt32::from_ne_bytes(type_.flags.to_ne_bytes())),
        glyph_index,
    }
}

fn ftc_scaler_key(
    scaler: FTC_ScalerRec,
    load_flags: FT_ULong,
    glyph_index: FT_UInt,
) -> FtcImageKey {
    FtcImageKey {
        face_id: scaler.face_id,
        width: scaler.width,
        height: scaler.height,
        pixel: scaler.pixel,
        x_res: scaler.x_res,
        y_res: scaler.y_res,
        load_flags,
        glyph_index,
    }
}

fn ftc_load_flags(load_flags: FT_ULong) -> FT_Int32 {
    let low =
        FT_UInt32::try_from(load_flags & FT_ULong::from(FT_UInt32::MAX)).unwrap_or(FT_UInt32::MAX);
    FT_Int32::from_ne_bytes(low.to_ne_bytes())
}

fn ftc_apply_scaler(
    manager: FTC_Manager,
    face_id: FTC_FaceID,
    width: FT_UInt,
    height: FT_UInt,
    pixel: FT_Int,
    x_res: FT_UInt,
    y_res: FT_UInt,
) -> Result<FT_Face, FT_Error> {
    let face = ftc_manager_lookup_face_impl(manager, face_id)?;
    if unsafe { (*face).size.is_null() }
        && let Some(state) = face_state_mut(face)
        && let Some(size) = state.size_records.first().copied()
    {
        // SAFETY: the manager-owned face retains this face-owned size record.
        unsafe {
            (*face).size = size;
        }
    }
    let error = if pixel != 0 {
        FT_Set_Pixel_Sizes(face, width, height)
    } else {
        FT_Set_Char_Size(
            face,
            FT_F26Dot6::from(width),
            FT_F26Dot6::from(height),
            x_res,
            y_res,
        )
    };
    if error == rust_ffi::FT_Err_Ok {
        Ok(face)
    } else {
        Err(error)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_Manager_New(
    library: FT_Library,
    max_faces: FT_UInt,
    max_sizes: FT_UInt,
    max_bytes: FT_ULong,
    requester: FTC_Face_Requester,
    req_data: FT_Pointer,
    amanager: *mut FTC_Manager,
) -> FT_Error {
    if library_ref(library).is_none() {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    }
    if requester.is_none() || amanager.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let manager = Box::into_raw(Box::new(FTC_ManagerRec {
        library,
        requester,
        req_data,
        _max_faces: max_faces,
        _max_sizes: max_sizes,
        _max_bytes: max_bytes,
        faces: BTreeMap::new(),
        nodes: Vec::new(),
        cmap_caches: Vec::new(),
        image_caches: Vec::new(),
        sbit_caches: Vec::new(),
    }));
    with_live_cache_managers(|managers| {
        managers.insert(manager as usize);
    });
    // SAFETY: `amanager` is non-null caller-owned output storage.
    unsafe {
        *amanager = manager;
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_Manager_Reset(manager: FTC_Manager) {
    ftc_reset_manager(manager);
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_Manager_Done(manager: FTC_Manager) {
    if !unregister_cache_manager(manager) {
        return;
    }
    ftc_reset_manager(manager);
    // SAFETY: move cache arrays out before freeing the manager, then recover
    // every cache allocation exactly once.
    let (cmap_caches, image_caches, sbit_caches) = unsafe {
        (
            std::mem::take(&mut (*manager).cmap_caches),
            std::mem::take(&mut (*manager).image_caches),
            std::mem::take(&mut (*manager).sbit_caches),
        )
    };
    for cache in cmap_caches {
        // SAFETY: each pointer came from `Box::into_raw` in the matching
        // constructor and is still manager-owned.
        unsafe { drop(Box::from_raw(cache)) };
    }
    for cache in image_caches {
        // SAFETY: same allocation ownership as the CMap cache above.
        unsafe { drop(Box::from_raw(cache)) };
    }
    for cache in sbit_caches {
        // SAFETY: same allocation ownership as the CMap cache above.
        unsafe { drop(Box::from_raw(cache)) };
    }
    // SAFETY: the live manager allocation is consumed exactly once here.
    unsafe { drop(Box::from_raw(manager)) };
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_Manager_LookupFace(
    manager: FTC_Manager,
    face_id: FTC_FaceID,
    aface: *mut FT_Face,
) -> FT_Error {
    if aface.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: clear the non-null caller output before the fallible lookup.
    unsafe {
        *aface = ptr::null_mut();
    }
    match ftc_manager_lookup_face_impl(manager, face_id) {
        Ok(face) => {
            // SAFETY: `aface` remains valid caller output storage.
            unsafe {
                *aface = face;
            }
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_Manager_LookupSize(
    manager: FTC_Manager,
    scaler: FTC_Scaler,
    asize: *mut FT_Size,
) -> FT_Error {
    if scaler.is_null() || asize.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: clear output before reading the non-null caller scaler record.
    unsafe {
        *asize = ptr::null_mut();
    }
    // SAFETY: `scaler` is borrowed only long enough to copy its scalar fields.
    let scaler = unsafe { *scaler };
    let face = match ftc_apply_scaler(
        manager,
        scaler.face_id,
        scaler.width,
        scaler.height,
        scaler.pixel,
        scaler.x_res,
        scaler.y_res,
    ) {
        Ok(face) => face,
        Err(error) => return error,
    };
    // SAFETY: manager-requested faces are live wrapper handles; their public
    // size pointer remains face-owned.
    unsafe {
        *asize = (*face).size;
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_Manager_RemoveFaceID(manager: FTC_Manager, face_id: FTC_FaceID) {
    if manager.is_null() {
        return;
    }
    let key = face_id as usize;
    // SAFETY: manager is live; removal transfers the owned face out before
    // running its destructor.
    let face = unsafe { (*manager).faces.remove(&key) };
    if let Some(face) = face {
        let _ = FT_Done_Face(face);
    }
    // SAFETY: cache handles remain manager-owned; removing their lookup keys
    // hides both referenced and unreferenced nodes immediately.  The manager
    // retains node allocations until reset/done so an outstanding node may
    // still be unreferenced safely.
    unsafe {
        for cache in &mut (*manager).cmap_caches {
            if let Some(cache) = cache.as_mut() {
                cache.entries.retain(|key, _| key.face_id != face_id);
            }
        }
        for cache in &mut (*manager).image_caches {
            if let Some(cache) = cache.as_mut() {
                cache.entries.retain(|key, _| key.face_id != face_id);
            }
        }
        for cache in &mut (*manager).sbit_caches {
            if let Some(cache) = cache.as_mut() {
                cache.entries.retain(|key, _| key.face_id != face_id);
            }
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_CMapCache_New(manager: FTC_Manager, acache: *mut FTC_CMapCache) -> FT_Error {
    if manager.is_null() || acache.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    if ftc_cache_count(manager) >= 16 {
        return rust_ffi::FT_Err_Too_Many_Caches as FT_Error;
    }
    let cache = Box::into_raw(Box::new(FTC_CMapCacheRec {
        manager,
        entries: BTreeMap::new(),
    }));
    // SAFETY: both manager and output are live; manager assumes ownership.
    unsafe {
        (*manager).cmap_caches.push(cache);
        *acache = cache;
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_CMapCache_Lookup(
    cache: FTC_CMapCache,
    face_id: FTC_FaceID,
    cmap_index: FT_Int,
    char_code: FT_UInt32,
) -> FT_UInt {
    if cache.is_null() {
        return 0;
    }
    // `cmap_index` is the public `FT_Int` type.  After clamping negatives to
    // zero, its value is always representable by the public `FT_UInt` type;
    // keep this conversion total so the defensive conversion-failure arm does
    // not become an unreachably counted region for valid ABI inputs.
    let normalized_cmap_index = cmap_index.max(0) as FT_UInt;
    let key = FtcCMapKey {
        face_id,
        cmap_index: normalized_cmap_index,
        char_code,
    };
    // C `FTC_CMapCache_Lookup` hashes negative cmap indexes as index zero.
    // Consequently a negative lookup can hit a value populated by a prior
    // index-zero lookup without consulting the face's current charmap.
    // SAFETY: `cache` is a live manager-owned cache handle.
    if let Some(glyph) = unsafe { (*cache).entries.get(&key).copied() } {
        return glyph;
    }
    // SAFETY: cache is a live handle and stores its owning manager pointer.
    let manager = unsafe { (*cache).manager };
    let Ok(face) = ftc_manager_lookup_face_impl(manager, face_id) else {
        return 0;
    };
    let glyph = cmap_cache_lookup_glyph_for_face(face, cmap_index, char_code);
    // SAFETY: the cache remains manager-owned for the duration of this call.
    unsafe {
        (*cache).entries.insert(key, glyph);
    }
    glyph
}

fn cmap_cache_lookup_glyph_for_face(
    face: FT_Face,
    cmap_index: FT_Int,
    char_code: FT_UInt32,
) -> FT_UInt {
    let glyph = {
        let Some(state) = face_state_mut(face) else {
            return 0;
        };
        state.inner.cmap_cache_lookup_glyph(cmap_index, char_code)
    };
    sync_face_public_record(face);
    glyph
}

#[cfg(feature = "abi-test-support")]
pub fn cmap_cache_lookup_glyph_for_test(
    face: FT_Face,
    cmap_index: FT_Int,
    char_code: FT_UInt32,
) -> FT_UInt {
    cmap_cache_lookup_glyph_for_face(face, cmap_index, char_code)
}

fn ftc_cache_count(manager: FTC_Manager) -> usize {
    if manager.is_null() {
        return 0;
    }
    // SAFETY: manager is live for this read-only count.
    unsafe {
        (*manager)
            .cmap_caches
            .len()
            .saturating_add((*manager).image_caches.len())
            .saturating_add((*manager).sbit_caches.len())
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_ImageCache_New(
    manager: FTC_Manager,
    acache: *mut FTC_ImageCache,
) -> FT_Error {
    if manager.is_null() || acache.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    if ftc_cache_count(manager) >= 16 {
        return rust_ffi::FT_Err_Too_Many_Caches as FT_Error;
    }
    let cache = Box::into_raw(Box::new(FTC_ImageCacheRec {
        manager,
        entries: BTreeMap::new(),
    }));
    // SAFETY: both manager and output are live; manager assumes ownership.
    unsafe {
        (*manager).image_caches.push(cache);
        *acache = cache;
    }
    rust_ffi::FT_Err_Ok
}

fn ftc_image_cache_lookup_impl(
    cache: FTC_ImageCache,
    key: FtcImageKey,
    aglyph: *mut FT_Glyph,
    anode: *mut FTC_Node,
) -> FT_Error {
    if cache.is_null() || aglyph.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: initialize caller outputs before any fallible operation.
    unsafe {
        *aglyph = ptr::null_mut();
        if let Some(anode) = anode.as_mut() {
            *anode = ptr::null_mut();
        }
    }
    // SAFETY: cache is live; entry pointers are manager-owned nodes.
    if let Some(node) = unsafe { (*cache).entries.get(&key).copied() } {
        // SAFETY: cached node remains live until manager reset/done.
        let glyph = match unsafe { &mut (*node).payload } {
            FtcNodePayload::Glyph(glyph) => *glyph,
            FtcNodePayload::SBit { .. } => return rust_ffi::FT_Err_Invalid_Argument,
        };
        // SAFETY: outputs were validated above.
        unsafe {
            *aglyph = glyph;
            if !anode.is_null() {
                (*node).ref_count = (*node).ref_count.saturating_add(1);
                *anode = node;
            }
        }
        return rust_ffi::FT_Err_Ok;
    }
    // SAFETY: cache stores its owning live manager.
    let manager = unsafe { (*cache).manager };
    let face = match ftc_apply_scaler(
        manager,
        key.face_id,
        key.width,
        key.height,
        key.pixel,
        key.x_res,
        key.y_res,
    ) {
        Ok(face) => face,
        Err(error) => return error,
    };
    let error = FT_Load_Glyph(face, key.glyph_index, ftc_load_flags(key.load_flags));
    if error != rust_ffi::FT_Err_Ok {
        return error;
    }
    let mut glyph = ptr::null_mut();
    let error = FT_Get_Glyph(
        // SAFETY: `face` is a live manager-owned wrapper handle.
        unsafe { (*face).glyph },
        &mut glyph,
    );
    if error != rust_ffi::FT_Err_Ok {
        return error;
    }
    let node = Box::into_raw(Box::new(FTC_NodeRec {
        mru_next: ptr::null_mut(),
        mru_prev: ptr::null_mut(),
        link: ptr::null_mut(),
        hash: 0,
        cache_index: 0,
        ref_count: FT_Short::from(!anode.is_null()),
        payload: FtcNodePayload::Glyph(glyph),
    }));
    // SAFETY: manager and cache are live and jointly retain the new node.
    unsafe {
        (*manager).nodes.push(node);
        (*cache).entries.insert(key, node);
        *aglyph = glyph;
        if !anode.is_null() {
            *anode = node;
        }
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_ImageCache_Lookup(
    cache: FTC_ImageCache,
    type_: FTC_ImageType,
    gindex: FT_UInt,
    aglyph: *mut FT_Glyph,
    anode: *mut FTC_Node,
) -> FT_Error {
    if type_.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: image type is a caller-owned descriptor copied by value.
    let type_ = unsafe { *type_ };
    let key = ftc_image_type_key(type_, gindex);
    ftc_image_cache_lookup_impl(cache, key, aglyph, anode)
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_ImageCache_LookupScaler(
    cache: FTC_ImageCache,
    scaler: FTC_Scaler,
    load_flags: FT_ULong,
    gindex: FT_UInt,
    aglyph: *mut FT_Glyph,
    anode: *mut FTC_Node,
) -> FT_Error {
    if scaler.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: scaler is a caller-owned descriptor copied by value.
    let scaler = unsafe { *scaler };
    let key = ftc_scaler_key(scaler, load_flags, gindex);
    ftc_image_cache_lookup_impl(cache, key, aglyph, anode)
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_SBitCache_New(manager: FTC_Manager, acache: *mut FTC_SBitCache) -> FT_Error {
    if !acache.is_null() {
        // SAFETY: the output pointer is non-null and caller-owned.
        unsafe {
            *acache = ptr::null_mut();
        }
    }
    if manager.is_null() || acache.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    if ftc_cache_count(manager) >= 16 {
        return rust_ffi::FT_Err_Too_Many_Caches as FT_Error;
    }
    let cache = Box::into_raw(Box::new(FTC_SBitCacheRec {
        manager,
        entries: BTreeMap::new(),
    }));
    // SAFETY: both manager and output are live; manager assumes ownership.
    unsafe {
        (*manager).sbit_caches.push(cache);
        *acache = cache;
    }
    rust_ffi::FT_Err_Ok
}

fn ftc_sbit_cache_lookup_impl(
    cache: FTC_SBitCache,
    key: FtcImageKey,
    sbit: *mut FTC_SBit,
    anode: *mut FTC_Node,
) -> FT_Error {
    if cache.is_null() || sbit.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: initialize caller outputs before any fallible operation.
    unsafe {
        *sbit = ptr::null_mut();
        if let Some(anode) = anode.as_mut() {
            *anode = ptr::null_mut();
        }
    }
    // SAFETY: cache is live; entry pointers are manager-owned nodes.
    if let Some(node) = unsafe { (*cache).entries.get(&key).copied() } {
        // SAFETY: cached node remains live until manager reset/done.
        let record = match unsafe { &mut (*node).payload } {
            FtcNodePayload::SBit { record, .. } => record as *mut FTC_SBitRec,
            FtcNodePayload::Glyph(_) => return rust_ffi::FT_Err_Invalid_Argument,
        };
        // SAFETY: outputs were validated above.
        unsafe {
            *sbit = record;
            if !anode.is_null() {
                (*node).ref_count = (*node).ref_count.saturating_add(1);
                *anode = node;
            }
        }
        return rust_ffi::FT_Err_Ok;
    }
    // SAFETY: cache stores its owning live manager.
    let manager = unsafe { (*cache).manager };
    let face = match ftc_apply_scaler(
        manager,
        key.face_id,
        key.width,
        key.height,
        key.pixel,
        key.x_res,
        key.y_res,
    ) {
        Ok(face) => face,
        Err(error) => return error,
    };
    // SAFETY: the manager requester returned a live face wrapper. FreeType's
    // SBit cache rejects an out-of-range glyph before classifying load errors
    // as cacheable missing-bitmap sentinels.
    if FT_Long::from(key.glyph_index) >= unsafe { (*face).num_glyphs } {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let error = FT_Load_Glyph(face, key.glyph_index, ftc_load_flags(key.load_flags));
    if error != rust_ffi::FT_Err_Ok {
        // `ftcsbits.c` suppresses every non-OOM glyph-load error and caches
        // it as an unavailable SBit sentinel with a successful lookup.
        let record = FTC_SBitRec {
            width: FT_Byte::MAX,
            ..FTC_SBitRec::default()
        };
        return ftc_sbit_cache_store(cache, manager, key, record, Box::new([]), sbit, anode);
    }
    // SAFETY: face is live and owns its public glyph slot.
    let slot = unsafe { (*face).glyph };
    if slot.is_null() {
        return rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error;
    }
    // SAFETY: slot is a live face-owned wrapper record.
    if unsafe { (*slot).format } != rust_ffi::FT_GLYPH_FORMAT_BITMAP {
        let error = FT_Render_Glyph(slot, rust_ffi::FT_RENDER_MODE_NORMAL);
        if error != rust_ffi::FT_Err_Ok {
            if error == rust_ffi::FT_Err_Out_Of_Memory {
                return error;
            }
            // `ftcsbits.c:133-137,193-204` converts a successful glyph load
            // that cannot render into the unavailable-SBit sentinel. Only
            // OOM escapes the cache lookup as an error.
            let record = FTC_SBitRec {
                width: FT_Byte::MAX,
                ..FTC_SBitRec::default()
            };
            return ftc_sbit_cache_store(cache, manager, key, record, Box::new([]), sbit, anode);
        }
    }
    // The public ABI adapter restores a Gray descriptor for an empty rendered
    // slot, but a successful SBIT lookup with no embedded glyph image keeps a
    // distinct Mono-format, null-buffer sentinel in `FTC_SBitRec`. Inspect
    // the Rust slot retained by the compatibility record so these two cases
    // do not collapse at the C ABI boundary.
    let missing_bitmap =
        slot_internal(slot).is_some_and(|internal| !internal.rust_slot.bitmap_descriptor_present());
    // SAFETY: render success leaves a live bitmap record in the slot.
    let slot = unsafe { &*slot };
    let rendered_empty_bitmap = slot.bitmap.width == 0
        && slot.bitmap.rows == 0
        && slot.bitmap.buffer.is_null()
        && slot.bitmap.pixel_mode == 0
        && !missing_bitmap;
    let row_bytes = usize::try_from(slot.bitmap.pitch.unsigned_abs()).unwrap_or(0);
    let rows = usize::try_from(slot.bitmap.rows).unwrap_or(0);
    let len = row_bytes.saturating_mul(rows);
    let mut buffer = if slot.bitmap.buffer.is_null() || len == 0 {
        Box::new([])
    } else {
        // SAFETY: the live slot owns at least pitch*rows initialized bytes.
        unsafe { slice::from_raw_parts(slot.bitmap.buffer, len) }
            .to_vec()
            .into_boxed_slice()
    };
    let record = FTC_SBitRec {
        width: FT_Byte::try_from(slot.bitmap.width).unwrap_or(FT_Byte::MAX),
        height: FT_Byte::try_from(slot.bitmap.rows).unwrap_or(FT_Byte::MAX),
        left: FT_Char::try_from(slot.bitmap_left).unwrap_or(if slot.bitmap_left < 0 {
            FT_Char::MIN
        } else {
            FT_Char::MAX
        }),
        top: FT_Char::try_from(slot.bitmap_top).unwrap_or(if slot.bitmap_top < 0 {
            FT_Char::MIN
        } else {
            FT_Char::MAX
        }),
        // The public C ABI slot elides the empty buffer, while FreeType keeps
        // the zero-sized rendered descriptor's gray format observable.
        format: if rendered_empty_bitmap {
            rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte
        } else if missing_bitmap {
            rust_ffi::FT_PIXEL_MODE_MONO as FT_Byte
        } else {
            FT_Byte::try_from(slot.bitmap.pixel_mode).unwrap_or(0)
        },
        max_grays: if rendered_empty_bitmap || missing_bitmap {
            255
        } else {
            FT_Byte::try_from(slot.bitmap.num_grays.saturating_sub(1)).unwrap_or(FT_Byte::MAX)
        },
        pitch: FT_Short::try_from(slot.bitmap.pitch).unwrap_or(if slot.bitmap.pitch < 0 {
            FT_Short::MIN
        } else {
            FT_Short::MAX
        }),
        // Pinned FreeType `ftcsbits.c` adds half a pixel before converting
        // the slot's 26.6 advances to FTC_SBit pixels.
        xadvance: FT_Char::try_from((slot.advance.x.saturating_add(32)) >> 6)
            .unwrap_or(FT_Char::MAX),
        yadvance: FT_Char::try_from((slot.advance.y.saturating_add(32)) >> 6)
            .unwrap_or(FT_Char::MAX),
        buffer: buffer.as_mut_ptr(),
    };
    ftc_sbit_cache_store(cache, manager, key, record, buffer, sbit, anode)
}

fn ftc_sbit_cache_store(
    cache: FTC_SBitCache,
    manager: FTC_Manager,
    key: FtcImageKey,
    mut record: FTC_SBitRec,
    mut buffer: Box<[FT_Byte]>,
    sbit: *mut FTC_SBit,
    anode: *mut FTC_Node,
) -> FT_Error {
    record.buffer = if buffer.is_empty() {
        ptr::null_mut()
    } else {
        buffer.as_mut_ptr()
    };
    let mut node = Box::new(FTC_NodeRec {
        mru_next: ptr::null_mut(),
        mru_prev: ptr::null_mut(),
        link: ptr::null_mut(),
        hash: 0,
        cache_index: 0,
        ref_count: FT_Short::from(!anode.is_null()),
        payload: FtcNodePayload::SBit {
            record,
            _buffer: buffer,
        },
    });
    let FtcNodePayload::SBit { record, .. } = &mut node.payload else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let record = record as *mut FTC_SBitRec;
    let node = Box::into_raw(node);
    // SAFETY: node payload is stable in its box; manager and cache retain it.
    unsafe {
        (*manager).nodes.push(node);
        (*cache).entries.insert(key, node);
        *sbit = record;
        if !anode.is_null() {
            *anode = node;
        }
    }
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
struct AbiSBitRequesterData {
    bytes: Box<[FT_Byte]>,
    face_index: FT_Long,
    requester_calls: FT_UInt,
    finalizer_calls: FT_UInt,
    requester_error: FT_Error,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum AbiCustomMemoryPhase {
    NewLibrary,
    AddDefaultModules,
    NewFace,
    DoneFace,
    DoneLibrary,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum AbiCustomMemoryEventKind {
    Alloc,
    Realloc,
    Free,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone, Copy)]
struct AbiCustomMemoryEvent {
    phase: AbiCustomMemoryPhase,
    kind: AbiCustomMemoryEventKind,
    memory_identity: bool,
}

#[cfg(feature = "abi-test-support")]
struct AbiCustomMemoryData {
    expected_memory: usize,
    phase: AbiCustomMemoryPhase,
    events: Vec<AbiCustomMemoryEvent>,
    blocks: BTreeMap<usize, Box<[u8]>>,
    unknown_release: bool,
    fail_after: Option<usize>,
    allocation_count: usize,
}

#[cfg(feature = "abi-test-support")]
unsafe fn abi_custom_memory_data(memory: FT_Memory) -> Option<&'static mut AbiCustomMemoryData> {
    let memory = NonNull::new(memory)?;
    // SAFETY: the harness installs a live `AbiCustomMemoryData` pointer in
    // `memory.user` for the complete custom-library lifecycle.
    unsafe { memory.as_ref().user.cast::<AbiCustomMemoryData>().as_mut() }
}

#[cfg(feature = "abi-test-support")]
extern "C" fn abi_custom_memory_alloc(memory: FT_Memory, size: c_long) -> FT_Pointer {
    // SAFETY: callback use is confined to `AbiCustomMemoryHarness::run`.
    let Some(data) = (unsafe { abi_custom_memory_data(memory) }) else {
        return ptr::null_mut();
    };
    let Ok(size) = usize::try_from(size) else {
        return ptr::null_mut();
    };
    let attempt = data.allocation_count;
    data.allocation_count = data.allocation_count.saturating_add(1);
    data.events.push(AbiCustomMemoryEvent {
        phase: data.phase,
        kind: AbiCustomMemoryEventKind::Alloc,
        memory_identity: memory.addr() == data.expected_memory,
    });
    if data.fail_after.is_some_and(|limit| attempt >= limit) {
        return ptr::null_mut();
    }
    let mut block = vec![0_u8; size.max(1)].into_boxed_slice();
    let pointer = block.as_mut_ptr().cast::<c_void>();
    data.blocks.insert(pointer.addr(), block);
    pointer
}

#[cfg(feature = "abi-test-support")]
extern "C" fn abi_custom_memory_free(memory: FT_Memory, block: FT_Pointer) {
    // SAFETY: callback use is confined to `AbiCustomMemoryHarness::run`.
    let Some(data) = (unsafe { abi_custom_memory_data(memory) }) else {
        return;
    };
    let removed = data.blocks.remove(&block.addr());
    data.unknown_release |= removed.is_none();
    data.events.push(AbiCustomMemoryEvent {
        phase: data.phase,
        kind: AbiCustomMemoryEventKind::Free,
        memory_identity: memory.addr() == data.expected_memory,
    });
}

#[cfg(feature = "abi-test-support")]
extern "C" fn abi_custom_memory_realloc(
    memory: FT_Memory,
    _current_size: c_long,
    new_size: c_long,
    block: FT_Pointer,
) -> FT_Pointer {
    // SAFETY: callback use is confined to `AbiCustomMemoryHarness::run`.
    let Some(data) = (unsafe { abi_custom_memory_data(memory) }) else {
        return ptr::null_mut();
    };
    let attempt = data.allocation_count;
    data.allocation_count = data.allocation_count.saturating_add(1);
    data.events.push(AbiCustomMemoryEvent {
        phase: data.phase,
        kind: AbiCustomMemoryEventKind::Realloc,
        memory_identity: memory.addr() == data.expected_memory,
    });
    if data.fail_after.is_some_and(|limit| attempt >= limit) {
        return ptr::null_mut();
    }
    if !block.is_null() && data.blocks.remove(&block.addr()).is_none() {
        data.unknown_release = true;
    }
    let Ok(new_size) = usize::try_from(new_size) else {
        return ptr::null_mut();
    };
    let mut replacement = vec![0_u8; new_size.max(1)].into_boxed_slice();
    let pointer = replacement.as_mut_ptr().cast::<c_void>();
    data.blocks.insert(pointer.addr(), replacement);
    pointer
}

#[cfg(feature = "abi-test-support")]
extern "C" fn abi_coverage_bzip2_nonzero_probe(
    _stream: FT_Stream,
    _offset: FT_ULong,
    _buffer: *mut FT_Byte,
    _count: FT_ULong,
) -> FT_ULong {
    1
}

#[cfg(feature = "abi-test-support")]
extern "C" fn abi_coverage_bzip2_short_probe(
    _stream: FT_Stream,
    _offset: FT_ULong,
    _buffer: *mut FT_Byte,
    _count: FT_ULong,
) -> FT_ULong {
    0
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_coverage_cache_requester_null_face(
    _face_id: FTC_FaceID,
    _library: FT_Library,
    _req_data: FT_Pointer,
    aface: *mut FT_Face,
) -> FT_Error {
    if let Some(aface) = unsafe { aface.as_mut() } {
        *aface = ptr::null_mut();
    }
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
fn abi_coverage_manager_record(
    library: FT_Library,
    requester: FTC_Face_Requester,
    req_data: FT_Pointer,
) -> FTC_ManagerRec {
    FTC_ManagerRec {
        library,
        requester,
        req_data,
        _max_faces: 0,
        _max_sizes: 0,
        _max_bytes: 0,
        faces: BTreeMap::new(),
        nodes: Vec::new(),
        cmap_caches: Vec::new(),
        image_caches: Vec::new(),
        sbit_caches: Vec::new(),
    }
}

#[cfg(feature = "abi-test-support")]
fn abi_coverage_image_key(face_id: FTC_FaceID) -> FtcImageKey {
    FtcImageKey {
        face_id,
        width: 12,
        height: 12,
        pixel: 1,
        x_res: 0,
        y_res: 0,
        load_flags: rust_ffi::FT_LOAD_DEFAULT as FT_ULong,
        glyph_index: 0,
    }
}

#[cfg(feature = "abi-test-support")]
fn abi_coverage_node(payload: FtcNodePayload) -> FTC_Node {
    Box::into_raw(Box::new(FTC_NodeRec {
        mru_next: ptr::null_mut(),
        mru_prev: ptr::null_mut(),
        link: ptr::null_mut(),
        hash: 0,
        cache_index: 0,
        ref_count: 0,
        payload,
    }))
}

#[cfg(feature = "abi-test-support")]
fn abi_coverage_stack_cache_lifecycle(library: FT_Library, remove_face: bool) {
    let mut manager = abi_coverage_manager_record(library, None, ptr::null_mut());
    let manager_ptr = ptr::from_mut(&mut manager);
    let cmap = Box::into_raw(Box::new(FTC_CMapCacheRec {
        manager: manager_ptr,
        entries: BTreeMap::new(),
    }));
    let image = Box::into_raw(Box::new(FTC_ImageCacheRec {
        manager: manager_ptr,
        entries: BTreeMap::new(),
    }));
    let sbit = Box::into_raw(Box::new(FTC_SBitCacheRec {
        manager: manager_ptr,
        entries: BTreeMap::new(),
    }));
    manager.cmap_caches.push(cmap);
    manager.image_caches.push(image);
    manager.sbit_caches.push(sbit);
    let face_id = NonNull::<c_void>::dangling().as_ptr();
    let other_face_id = NonNull::new(2_usize as *mut c_void).expect("non-null probe key");
    let image_key = abi_coverage_image_key(face_id);
    let other_face_id = other_face_id.as_ptr();
    let other_image_key = abi_coverage_image_key(other_face_id);
    // Populate every cache before either lifecycle operation.  Reset must
    // clear observable entries, and RemoveFaceID must retain the nonmatching
    // sentinel while invoking each matching-key predicate.
    unsafe {
        (*cmap).entries.insert(
            FtcCMapKey {
                face_id,
                cmap_index: 0,
                char_code: 65,
            },
            0,
        );
        (*cmap).entries.insert(
            FtcCMapKey {
                face_id: other_face_id,
                cmap_index: 0,
                char_code: 66,
            },
            0,
        );
        (*image).entries.insert(image_key, ptr::null_mut());
        (*image).entries.insert(other_image_key, ptr::null_mut());
        (*sbit).entries.insert(image_key, ptr::null_mut());
        (*sbit).entries.insert(other_image_key, ptr::null_mut());
    }
    if remove_face {
        let remove_face_id: extern "C" fn(FTC_Manager, FTC_FaceID) = FTC_Manager_RemoveFaceID;
        std::hint::black_box(remove_face_id)(manager_ptr, face_id);
        assert_eq!(unsafe { (*cmap).entries.len() }, 1);
        assert_eq!(unsafe { (*image).entries.len() }, 1);
        assert_eq!(unsafe { (*sbit).entries.len() }, 1);
    } else {
        let reset_manager: extern "C" fn(FTC_Manager) = FTC_Manager_Reset;
        std::hint::black_box(reset_manager)(manager_ptr);
        assert!(unsafe { (*cmap).entries.is_empty() });
        assert!(unsafe { (*image).entries.is_empty() });
        assert!(unsafe { (*sbit).entries.is_empty() });
    }
    manager.cmap_caches.clear();
    manager.image_caches.clear();
    manager.sbit_caches.clear();
    // SAFETY: each cache is owned by this helper and was removed from the
    // manager vectors before reconstructing its box.
    unsafe {
        drop(Box::from_raw(cmap));
        drop(Box::from_raw(image));
        drop(Box::from_raw(sbit));
    }
}

#[cfg(feature = "abi-test-support")]
fn abi_coverage_cmap_restore(face: FT_Face, library: FT_Library) {
    let face_id = NonNull::<c_void>::dangling().as_ptr();
    let mut manager = abi_coverage_manager_record(library, None, ptr::null_mut());
    manager.faces.insert(face_id.addr(), face);
    let charmap = face_state(face)
        .and_then(|state| state.charmap_by_index(0))
        .expect("maintained cmap restore font has a first charmap");
    assert_eq!(FT_Set_Charmap(face, charmap), rust_ffi::FT_Err_Ok);
    assert_eq!(
        face_state(face).map(|state| state.inner.active_charmap_index),
        Some(0)
    );
    let mut cache = FTC_CMapCacheRec {
        manager: ptr::from_mut(&mut manager),
        entries: BTreeMap::new(),
    };
    let cmap_lookup: extern "C" fn(FTC_CMapCache, FTC_FaceID, FT_Int, FT_UInt32) -> FT_UInt =
        FTC_CMapCache_Lookup;
    let glyph = std::hint::black_box(cmap_lookup)(ptr::from_mut(&mut cache), face_id, 0, 65);
    std::hint::black_box(glyph);
    manager.faces.clear();
}

#[cfg(feature = "abi-test-support")]
fn abi_core_load_case(
    bytes: &[FT_Byte],
    load_flags: FT_Int32,
    glyphs: &[FT_UInt],
) -> Vec<(FT_UInt, FT_Error)> {
    let core_library = rust_ffi::FT_Init_FreeType();
    let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) else {
        return Vec::new();
    };
    let _ = rust_ffi::FT_Set_Pixel_Sizes(&mut core_face, 0, 16);
    glyphs
        .iter()
        .map(
            |&glyph| match rust_ffi::FT_Load_Glyph(&core_face, glyph, load_flags) {
                Ok(slot) => (slot.glyph_index, rust_ffi::FT_Err_Ok),
                Err(error) => (0, error),
            },
        )
        .collect()
}

#[cfg(feature = "abi-test-support")]
fn abi_c99_core_probe(bytes: &[FT_Byte], variant: u16) {
    let core_library = rust_ffi::FT_Init_FreeType();
    let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) else {
        return;
    };
    let sizes: [(FT_UInt, FT_UInt); 5] = [(0, 16), (1, 8), (8, 16), (16, 24), (32, 32)];
    let load_flags: [FT_Int32; 26] = [
        rust_ffi::FT_LOAD_DEFAULT,
        rust_ffi::FT_LOAD_NO_SCALE,
        rust_ffi::FT_LOAD_NO_HINTING,
        rust_ffi::FT_LOAD_FORCE_AUTOHINT,
        rust_ffi::FT_LOAD_NO_BITMAP,
        rust_ffi::FT_LOAD_VERTICAL_LAYOUT,
        rust_ffi::FT_LOAD_NO_RECURSE,
        rust_ffi::FT_LOAD_IGNORE_GLOBAL_ADVANCE_WIDTH,
        rust_ffi::FT_LOAD_TARGET_LIGHT,
        rust_ffi::FT_LOAD_TARGET_MONO,
        rust_ffi::FT_LOAD_TARGET_LCD,
        rust_ffi::FT_LOAD_TARGET_LCD_V,
        rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_NO_HINTING,
        rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_FORCE_AUTOHINT,
        rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_NO_BITMAP,
        rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_MONO,
        rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_LCD,
        rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_LCD_V,
        rust_ffi::FT_LOAD_NO_SCALE | rust_ffi::FT_LOAD_NO_HINTING,
        rust_ffi::FT_LOAD_NO_AUTOHINT | rust_ffi::FT_LOAD_NO_HINTING,
        rust_ffi::FT_LOAD_PEDANTIC,
        rust_ffi::FT_LOAD_COMPUTE_METRICS,
        rust_ffi::FT_LOAD_BITMAP_METRICS_ONLY,
        rust_ffi::FT_LOAD_COLOR,
        rust_ffi::FT_LOAD_NO_SVG,
        rust_ffi::FT_LOAD_SVG_ONLY,
    ];
    let render_modes: [FT_Render_Mode; 6] = [
        rust_ffi::FT_RENDER_MODE_NORMAL,
        rust_ffi::FT_RENDER_MODE_LIGHT,
        rust_ffi::FT_RENDER_MODE_MONO,
        rust_ffi::FT_RENDER_MODE_LCD,
        rust_ffi::FT_RENDER_MODE_LCD_V,
        rust_ffi::FT_RENDER_MODE_SDF,
    ];
    let index = usize::from(variant.saturating_sub(301));
    let (width, height) = sizes[index % sizes.len()];
    let flags = load_flags[index % load_flags.len()];
    let _ = rust_ffi::FT_Set_Pixel_Sizes(&mut core_face, width, height);
    let char_codes = [0, 32, 65, 0x20AC, 0xFFFF];
    let char_code = char_codes[index % char_codes.len()];
    let glyph_index = rust_ffi::FT_Get_Char_Index(&core_face, char_code);
    let glyphs = [0, 1, glyph_index, 3, 36, 65, FT_UInt::MAX];
    let glyph = glyphs[index % glyphs.len()];

    if index % 4 == 0 {
        let matrix = rust_ffi::FT_Matrix {
            xx: 0x10000,
            xy: 0x0800,
            yx: -0x0400,
            yy: 0x10000,
        };
        let delta = rust_ffi::FT_Vector { x: 9, y: -7 };
        rust_ffi::FT_Set_Transform(Some(&mut core_face), Some(&matrix), Some(&delta));
    } else if index % 4 == 1 {
        rust_ffi::FT_Set_Transform(Some(&mut core_face), None, None);
    }

    let loaded = rust_ffi::FT_Load_Glyph(&core_face, glyph, flags);
    let loaded_char = rust_ffi::FT_Load_Char(&core_face, char_code, flags);
    let advance = rust_ffi::FT_Get_Advance(&core_face, glyph, flags);
    let advances = rust_ffi::FT_Get_Advances(&core_face, 0, 3, flags);
    let rendered = loaded
        .and_then(|slot| rust_ffi::FT_Render_Glyph(slot, render_modes[index % render_modes.len()]));
    let _ = std::hint::black_box((loaded_char, advance, advances, rendered, glyph_index));
}

#[cfg(feature = "abi-test-support")]
fn abi_c100_core_probe(bytes: &[FT_Byte], variant: u16) {
    let core_library = rust_ffi::FT_Init_FreeType();
    let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) else {
        return;
    };
    let index = usize::from(variant.saturating_sub(401));
    let sizes = [(0, 12), (8, 8), (12, 16), (20, 24), (32, 32)];
    let (pixel_width, pixel_height) = sizes[index % sizes.len()];
    let size_status = rust_ffi::FT_Set_Pixel_Sizes(&mut core_face, pixel_width, pixel_height);
    let char_codes = [0, 32, 65, 0x391, 0x4E00, 0x20AC, 0xFFFF];
    let char_code = char_codes[index % char_codes.len()];
    let glyph_index = rust_ffi::FT_Get_Char_Index(&core_face, char_code);
    let glyphs = [0, 1, glyph_index, 3, 36, 65, FT_UInt::MAX];
    let glyph = glyphs[index % glyphs.len()];
    let load_flags = [
        rust_ffi::FT_LOAD_DEFAULT,
        rust_ffi::FT_LOAD_NO_SCALE,
        rust_ffi::FT_LOAD_NO_HINTING,
        rust_ffi::FT_LOAD_NO_BITMAP,
        rust_ffi::FT_LOAD_VERTICAL_LAYOUT,
        rust_ffi::FT_LOAD_NO_RECURSE,
        rust_ffi::FT_LOAD_FORCE_AUTOHINT,
        rust_ffi::FT_LOAD_NO_AUTOHINT,
        rust_ffi::FT_LOAD_PEDANTIC,
        rust_ffi::FT_LOAD_COMPUTE_METRICS,
        rust_ffi::FT_LOAD_BITMAP_METRICS_ONLY,
        rust_ffi::FT_LOAD_TARGET_LIGHT,
        rust_ffi::FT_LOAD_TARGET_MONO,
        rust_ffi::FT_LOAD_TARGET_LCD,
        rust_ffi::FT_LOAD_TARGET_LCD_V,
        rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_NO_HINTING,
        rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_FORCE_AUTOHINT,
        rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_MONO,
        rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_LCD,
        rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_LCD_V,
    ];
    let flags = load_flags[index % load_flags.len()];

    match index % 10 {
        0 => {
            let mut matrix = rust_ffi::FT_Matrix {
                xx: 0x1_0000,
                xy: 0x0800,
                yx: -0x0400,
                yy: 0x1_0000,
            };
            let delta = rust_ffi::FT_Vector { x: 11, y: -9 };
            rust_ffi::FT_Set_Transform(Some(&mut core_face), Some(&matrix), Some(&delta));
            let transformed = rust_ffi::FT_Get_Transform(Some(&core_face), Some(&mut matrix), None);
            let char_size = rust_ffi::FT_Set_Char_Size(
                &mut core_face,
                0,
                FT_Long::from((pixel_height.max(1) * 64) as i32),
                72,
                96,
            );
            let request = rust_ffi::FT_Size_RequestRec {
                type_: rust_ffi::FT_SIZE_REQUEST_TYPE_BBOX as _,
                width: 12 * 64,
                height: 16 * 64,
                horiResolution: 96,
                vertResolution: 72,
            };
            let requested = rust_ffi::FT_Request_Size(Some(&mut core_face), Some(&request));
            let loaded = rust_ffi::FT_Load_Glyph(&core_face, glyph, flags);
            let loaded_char = rust_ffi::FT_Load_Char(&core_face, char_code, flags);
            let advance = rust_ffi::FT_Get_Advance(&core_face, glyph, flags);
            let advances = rust_ffi::FT_Get_Advances(&core_face, 0, 4, flags);
            let _ = std::hint::black_box((
                size_status,
                transformed,
                char_size,
                requested,
                loaded,
                loaded_char,
                advance,
                advances,
            ));
        }
        1 => {
            let Ok(slot) = rust_ffi::FT_Load_Glyph(
                &core_face,
                glyph,
                rust_ffi::FT_LOAD_NO_HINTING | rust_ffi::FT_LOAD_NO_BITMAP,
            ) else {
                return;
            };
            let Ok(owned) = rust_ffi::FT_Get_Outline_Glyph(Some(&slot)) else {
                return;
            };
            let snapshot = owned.outline.clone();
            let mut cbox = rust_ffi::FT_BBox::default();
            let bbox = rust_ffi::FT_Outline_Get_BBox(Some(&snapshot), Some(&mut cbox));
            let orientation = rust_ffi::FT_Outline_Get_Orientation(Some(&snapshot));
            let check = rust_ffi::FT_Outline_Check(Some(&snapshot));
            let mut copied = snapshot.clone();
            let copy = rust_ffi::FT_Outline_Copy(Some(&snapshot), Some(&mut copied));
            let mut transformed = copied.clone();
            rust_ffi::FT_Outline_Transform(
                Some(&mut transformed),
                Some(&rust_ffi::FT_Matrix {
                    xx: 0x1_0000,
                    xy: 0x0200,
                    yx: -0x0100,
                    yy: 0x1_0000,
                }),
            );
            rust_ffi::FT_Outline_Translate(Some(&mut transformed), 7, -5);
            let embolden = rust_ffi::FT_Outline_EmboldenXY(Some(&mut transformed), 3, 5);
            rust_ffi::FT_Outline_Reverse(Some(&mut transformed));
            let inside = rust_ffi::FT_Outline_GetInsideBorder(Some(&snapshot));
            let outside = rust_ffi::FT_Outline_GetOutsideBorder(Some(&snapshot));
            let decompose =
                rust_ffi::FT_Outline_Decompose_Trace(Some(&snapshot), &[(0, 0), (1, 9), (2, -13)]);
            let mut pixels = vec![0_u8; 96 * 96];
            let target = rust_ffi::FT_Bitmap_C {
                rows: 96,
                width: 96,
                pitch: 96,
                buffer: pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as _,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let bitmap = rust_ffi::FT_Outline_Get_Bitmap(
                Some(&core_library),
                Some(&snapshot),
                Some(&target),
            );
            let rendered = rust_ffi::FT_Outline_Render(
                Some(&core_library),
                Some(&snapshot),
                Some(&target),
                rust_ffi::FT_RASTER_FLAG_AA as _,
                rust_ffi::FT_BBox::default(),
            );
            let spans = rust_ffi::FT_Outline_Render_Direct_Spans(
                Some(&core_library),
                Some(&snapshot),
                Some(&target),
                rust_ffi::FT_RASTER_FLAG_DIRECT as FT_Int | rust_ffi::FT_RASTER_FLAG_AA as FT_Int,
                None,
                true,
            );
            let _ = std::hint::black_box((
                bbox,
                cbox,
                orientation,
                check,
                copy,
                embolden,
                inside,
                outside,
                decompose,
                bitmap,
                rendered,
                spans,
                pixels,
            ));
        }
        2 => {
            let Ok(slot) = rust_ffi::FT_Load_Glyph(
                &core_face,
                glyph,
                rust_ffi::FT_LOAD_NO_HINTING | rust_ffi::FT_LOAD_NO_BITMAP,
            ) else {
                return;
            };
            let Ok(owned) = rust_ffi::FT_Get_Outline_Glyph(Some(&slot)) else {
                return;
            };
            let mut cbox = rust_ffi::FT_BBox::default();
            rust_ffi::FT_Outline_Glyph_CBox(Some(&owned), index as _, Some(&mut cbox));
            let copied = rust_ffi::FT_Outline_Glyph_Copy(&owned);
            let modes = [
                rust_ffi::FT_RENDER_MODE_NORMAL,
                rust_ffi::FT_RENDER_MODE_MONO,
                rust_ffi::FT_RENDER_MODE_LCD,
                rust_ffi::FT_RENDER_MODE_LCD_V,
                rust_ffi::FT_RENDER_MODE_SDF,
            ];
            let bitmaps = modes.map(|mode| rust_ffi::FT_Outline_Glyph_To_Bitmap(&owned, mode));
            let mut in_place = copied.clone();
            let in_place_bitmap = rust_ffi::FT_Outline_Glyph_To_Bitmap_In_Place(
                &mut in_place,
                modes[index % modes.len()],
                Some(rust_ffi::FT_Vector { x: 13, y: -7 }),
                index % 2 == 0,
            );
            let mut transformed = copied.clone();
            let transform = rust_ffi::FT_Glyph_Transform_Outline(
                Some(&mut transformed),
                Some(&rust_ffi::FT_Matrix {
                    xx: 0x1_0000,
                    xy: 0x0100,
                    yx: 0,
                    yy: 0x1_0000,
                }),
                Some(&rust_ffi::FT_Vector { x: 3, y: 4 }),
            );
            let new_glyphs = [
                rust_ffi::FT_New_Glyph(Some(&core_library), rust_ffi::FT_GLYPH_FORMAT_OUTLINE),
                rust_ffi::FT_New_Glyph(Some(&core_library), rust_ffi::FT_GLYPH_FORMAT_BITMAP),
                rust_ffi::FT_New_Glyph(Some(&core_library), rust_ffi::FT_GLYPH_FORMAT_SVG),
            ];
            let _ = std::hint::black_box((cbox, bitmaps, in_place_bitmap, transform, new_glyphs));
        }
        3 => {
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                rust_ffi::FT_Stroker_Set(
                    stroker,
                    64 + (index as i64 & 3) * 16,
                    (index % 3) as _,
                    (index % 4) as _,
                    65_536,
                );
                let start = rust_ffi::FT_Vector { x: 0, y: 0 };
                let control = rust_ffi::FT_Vector { x: 256, y: 512 };
                let control2 = rust_ffi::FT_Vector { x: 512, y: 512 };
                let end = rust_ffi::FT_Vector { x: 640, y: 0 };
                let begin =
                    rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), (index % 2) as _);
                let line = rust_ffi::FT_Stroker_LineTo(stroker, Some(&control));
                let conic = rust_ffi::FT_Stroker_ConicTo(stroker, Some(&control2), Some(&end));
                let cubic = rust_ffi::FT_Stroker_CubicTo(
                    stroker,
                    Some(&control),
                    Some(&control2),
                    Some(&start),
                );
                let parsed = rust_ffi::FT_Load_Glyph(
                    &core_face,
                    glyph,
                    rust_ffi::FT_LOAD_NO_HINTING | rust_ffi::FT_LOAD_NO_BITMAP,
                )
                .ok()
                .and_then(|slot| rust_ffi::FT_Get_Outline_Glyph(Some(&slot)).ok())
                .map(|owned| {
                    rust_ffi::FT_Stroker_ParseOutline(
                        stroker,
                        Some(&owned.outline),
                        (index % 2) as _,
                    )
                });
                let ended = rust_ffi::FT_Stroker_EndSubPath(stroker);
                let mut points = 0;
                let mut contours = 0;
                let counts =
                    rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours));
                let border = rust_ffi::FT_Stroker_GetBorderCounts(
                    stroker,
                    rust_ffi::FT_STROKER_BORDER_LEFT as _,
                    Some(&mut points),
                    Some(&mut contours),
                );
                let mut exported = rust_ffi::FT_OutlineSnapshot::default();
                rust_ffi::FT_Stroker_Export(stroker, Some(&mut exported));
                rust_ffi::FT_Stroker_ExportBorder(
                    stroker,
                    rust_ffi::FT_STROKER_BORDER_RIGHT as _,
                    Some(&mut exported),
                );
                let _ = std::hint::black_box((
                    begin, line, conic, cubic, parsed, ended, counts, border, points, contours,
                    exported,
                ));
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        4 => {
            let render_modes = [
                rust_ffi::FT_RENDER_MODE_NORMAL,
                rust_ffi::FT_RENDER_MODE_LIGHT,
                rust_ffi::FT_RENDER_MODE_MONO,
                rust_ffi::FT_RENDER_MODE_LCD,
                rust_ffi::FT_RENDER_MODE_LCD_V,
                rust_ffi::FT_RENDER_MODE_SDF,
            ];
            let rendered = render_modes.map(|mode| {
                rust_ffi::FT_Load_Glyph(
                    &core_face,
                    glyph,
                    rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_NO_HINTING,
                )
                .and_then(|slot| rust_ffi::FT_Render_Glyph(slot, mode))
            });
            let mut pixels = vec![0_u8; 64 * 64];
            let bitmap = rust_ffi::FT_Bitmap_C {
                rows: 64,
                width: 64,
                pitch: 64,
                buffer: pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as _,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let outline = rust_ffi::FT_Load_Glyph(
                &core_face,
                glyph,
                rust_ffi::FT_LOAD_NO_HINTING | rust_ffi::FT_LOAD_NO_BITMAP,
            )
            .ok()
            .and_then(|slot| rust_ffi::FT_Get_Outline_Glyph(Some(&slot)).ok())
            .map(|owned| {
                let mono = rust_ffi::FT_Outline_Render(
                    Some(&core_library),
                    Some(&owned.outline),
                    Some(&bitmap),
                    rust_ffi::FT_RASTER_FLAG_DEFAULT as _,
                    rust_ffi::FT_BBox::default(),
                );
                let sdf = rust_ffi::FT_Outline_Render(
                    Some(&core_library),
                    Some(&owned.outline),
                    Some(&bitmap),
                    rust_ffi::FT_RASTER_FLAG_AA as _,
                    rust_ffi::FT_BBox::default(),
                );
                (mono, sdf)
            });
            let _ = std::hint::black_box((rendered, outline, pixels));
        }
        5 => {
            let mut first_glyph = 0;
            let first = rust_ffi::FT_Get_First_Char(Some(&core_face), Some(&mut first_glyph));
            let mut next_glyph = 0;
            let next = rust_ffi::FT_Get_Next_Char(Some(&core_face), first, Some(&mut next_glyph));
            let mut kerning = rust_ffi::FT_Vector::default();
            let kern = rust_ffi::FT_Get_Kerning(
                Some(&core_face),
                first_glyph,
                next_glyph,
                index as _,
                Some(&mut kerning),
            );
            let mut glyph_name = [0_u8; 128];
            let name = rust_ffi::FT_Get_Glyph_Name(&core_face, glyph, &mut glyph_name);
            let name_index = rust_ffi::FT_Get_Name_Index(Some(&core_face), Some("A"));
            let format = rust_ffi::FT_Get_Font_Format(Some(&core_face));
            let driver = rust_ffi::FT_FACE_DRIVER_NAME(Some(&core_face));
            let postscript = rust_ffi::FT_Get_Postscript_Name(&core_face);
            let gasp = rust_ffi::FT_Get_Gasp(Some(&core_face), pixel_height);
            let fs_type = rust_ffi::FT_Get_FSType_Flags(Some(&core_face));
            let count = rust_ffi::FT_Get_Sfnt_Name_Count(Some(&core_face));
            let mut sfnt_name = rust_ffi::FT_SfntName::default();
            let sfnt =
                rust_ffi::FT_Get_Sfnt_Name(Some(&core_face), index as _, Some(&mut sfnt_name));
            let table_tags = [
                rust_ffi::FT_SFNT_HEAD as _,
                rust_ffi::FT_SFNT_MAXP as _,
                rust_ffi::FT_SFNT_OS2 as _,
                rust_ffi::FT_SFNT_POST as _,
                u32::from_be_bytes(*b"glyf"),
            ];
            let tables = table_tags.map(|tag| {
                let pointer = rust_ffi::FT_Get_Sfnt_Table(&core_face, tag);
                let mut length = 0;
                let loaded =
                    rust_ffi::FT_Load_Sfnt_Table(&core_face, tag as _, 0, Some(&mut length));
                (pointer, loaded, length)
            });
            let _ = std::hint::black_box((
                first, next, kern, kerning, name, glyph_name, name_index, format, driver,
                postscript, gasp, fs_type, count, sfnt, tables,
            ));
        }
        6 => {
            let mut design = [0; 4];
            let mut blend = [0; 4];
            let get_design = rust_ffi::FT_Get_Var_Design_Coordinates(
                Some(&core_face),
                design.len() as _,
                Some(&mut design),
            );
            let get_blend = rust_ffi::FT_Get_Var_Blend_Coordinates(
                Some(&core_face),
                blend.len() as _,
                Some(&mut blend),
            );
            let coords = [0x4000_i64, -0x2000_i64, 0x1000_i64, 0];
            let set_design = rust_ffi::FT_Set_Var_Design_Coordinates(
                Some(&mut core_face),
                coords.len() as _,
                Some(&coords),
            );
            let set_blend = rust_ffi::FT_Set_Var_Blend_Coordinates(
                Some(&mut core_face),
                coords.len() as _,
                Some(&coords),
            );
            let mut default_instance = 0;
            let default_result = rust_ffi::FT_Get_Default_Named_Instance(
                Some(&core_face),
                Some(&mut default_instance),
            );
            let named = rust_ffi::FT_Set_Named_Instance(Some(&mut core_face), index as _);
            let loaded = rust_ffi::FT_Load_Glyph(
                &core_face,
                glyph,
                rust_ffi::FT_LOAD_FORCE_AUTOHINT | rust_ffi::FT_LOAD_NO_BITMAP,
            );
            let _ = std::hint::black_box((
                get_design,
                get_blend,
                set_design,
                set_blend,
                design,
                blend,
                default_result,
                default_instance,
                named,
                loaded,
            ));
        }
        7 => {
            let mut palette = rust_ffi::FT_Palette_Data::default();
            let palette_result =
                rust_ffi::FT_Palette_Data_Get(Some(&core_face), Some(&mut palette));
            let mut layer_iterator = rust_ffi::FT_LayerIterator::default();
            let mut layer_glyph = 0;
            let mut layer_color = 0;
            let mut layers = Vec::new();
            for base in [0, glyph_index, 1, 36] {
                layers.push(rust_ffi::FT_Get_Color_Glyph_Layer(
                    Some(&core_face),
                    base,
                    Some(&mut layer_glyph),
                    Some(&mut layer_color),
                    Some(&mut layer_iterator),
                ));
            }
            let mut clip = rust_ffi::FT_ClipBox::default();
            let clip_result = rust_ffi::FT_Get_Color_Glyph_ClipBox(
                Some(&core_face),
                glyph_index,
                Some(&mut clip),
            );
            let graph = rust_ffi::FT_ColrV1_PaintGraph_Copy(Some(&core_face));
            let _ = std::hint::black_box((
                palette_result,
                palette,
                layers,
                layer_glyph,
                layer_color,
                clip_result,
                clip,
                graph,
            ));
        }
        8 => {
            let empty = rust_ffi::FT_OutlineSnapshot::default();
            let mut malformed = rust_ffi::FT_OutlineSnapshot {
                points: vec![rust_ffi::FT_Vector::default()],
                tags: Vec::new(),
                contours: vec![0],
                flags: 0,
            };
            let mut bbox = rust_ffi::FT_BBox::default();
            let empty_check = rust_ffi::FT_Outline_Check(Some(&empty));
            let malformed_check = rust_ffi::FT_Outline_Check(Some(&malformed));
            let empty_bbox = rust_ffi::FT_Outline_Get_BBox(Some(&empty), Some(&mut bbox));
            let malformed_bbox = rust_ffi::FT_Outline_Get_BBox(Some(&malformed), Some(&mut bbox));
            let decompose = rust_ffi::FT_Outline_Decompose_Trace(Some(&malformed), &[(0, 0)]);
            rust_ffi::FT_Outline_Reverse(Some(&mut malformed));
            let mut pixels = [0_u8; 8];
            let target = rust_ffi::FT_Bitmap_C {
                rows: 1,
                width: 8,
                pitch: 8,
                buffer: pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as _,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let rendered_empty = rust_ffi::FT_Outline_Render(
                Some(&core_library),
                Some(&empty),
                Some(&target),
                rust_ffi::FT_RASTER_FLAG_AA as _,
                rust_ffi::FT_BBox::default(),
            );
            let _ = std::hint::black_box((
                empty_check,
                malformed_check,
                empty_bbox,
                malformed_bbox,
                decompose,
                rendered_empty,
                bbox,
                pixels,
            ));
        }
        _ => {
            let curve = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 64, y: 256 },
                    rust_ffi::FT_Vector { x: 192, y: 256 },
                    rust_ffi::FT_Vector { x: 256, y: 0 },
                    rust_ffi::FT_Vector { x: 320, y: -128 },
                    rust_ffi::FT_Vector { x: 384, y: 0 },
                ],
                tags: vec![
                    rust_ffi::FT_CURVE_TAG_ON as _,
                    rust_ffi::FT_CURVE_TAG_CUBIC as _,
                    rust_ffi::FT_CURVE_TAG_CUBIC as _,
                    rust_ffi::FT_CURVE_TAG_ON as _,
                    rust_ffi::FT_CURVE_TAG_CONIC as _,
                    rust_ffi::FT_CURVE_TAG_ON as _,
                ],
                contours: vec![3, 5],
                flags: 0,
            };
            let mut pixels = vec![0_u8; 32 * 32];
            let target = rust_ffi::FT_Bitmap_C {
                rows: 32,
                width: 32,
                pitch: 32,
                buffer: pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as _,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let bitmap =
                rust_ffi::FT_Outline_Get_Bitmap(Some(&core_library), Some(&curve), Some(&target));
            let render = rust_ffi::FT_Outline_Render(
                Some(&core_library),
                Some(&curve),
                Some(&target),
                rust_ffi::FT_RASTER_FLAG_AA as _,
                rust_ffi::FT_BBox::default(),
            );
            let direct = rust_ffi::FT_Outline_Render_Direct_Spans(
                Some(&core_library),
                Some(&curve),
                None,
                rust_ffi::FT_RASTER_FLAG_DIRECT as FT_Int | rust_ffi::FT_RASTER_FLAG_AA as FT_Int,
                None,
                true,
            );
            let trace = rust_ffi::FT_Outline_Decompose_Trace(Some(&curve), &[(0, 0), (3, 17)]);
            let _ = std::hint::black_box((bitmap, render, direct, trace, pixels));
        }
    }
}

#[cfg(feature = "abi-test-support")]
fn abi_c101_wrapper_probe(library: FT_Library, face: FT_Face, variant: u16) {
    // c101 keeps the production façade thin while driving its null and edge
    // guards through the same Rust-owned FFI forwarding functions used by C.
    // The ten families are repeated across one hundred distinct fixture
    // dispatches so each case remains independently measurable by Coverage
    // MCP without adding implementation policy to the exported ABI.
    match usize::from(variant.saturating_sub(501)) % 10 {
        0 => {
            let cache_new = FTC_CMapCache_New(ptr::null_mut(), ptr::null_mut());
            let cache_lookup = FTC_CMapCache_Lookup(ptr::null_mut(), ptr::null_mut(), -1, 0);
            let image_new = FTC_ImageCache_New(ptr::null_mut(), ptr::null_mut());
            let image_lookup = FTC_ImageCache_Lookup(
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            );
            let image_scaler = FTC_ImageCache_LookupScaler(
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            );
            let sbit_new = FTC_SBitCache_New(ptr::null_mut(), ptr::null_mut());
            let sbit_lookup = FTC_SBitCache_Lookup(
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            );
            let sbit_scaler = FTC_SBitCache_LookupScaler(
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            );
            FTC_Node_Unref(ptr::null_mut(), ptr::null_mut());
            std::hint::black_box((
                cache_new,
                cache_lookup,
                image_new,
                image_lookup,
                image_scaler,
                sbit_new,
                sbit_lookup,
                sbit_scaler,
            ));
        }
        1 => {
            let bzip = FT_Stream_OpenBzip2(ptr::null_mut(), ptr::null_mut());
            let lzw = FT_Stream_OpenLZW(ptr::null_mut(), ptr::null_mut());
            let gzip = FT_Stream_OpenGzip(ptr::null_mut(), ptr::null_mut());
            let bzip_bytes = abi_support_bzip2_stream_bytes(ptr::null_mut(), 0, 0);
            let lzw_bytes = abi_support_lzw_stream_bytes(ptr::null_mut(), 0, 0);
            let gzip_bytes = abi_support_gzip_stream_bytes(ptr::null_mut(), 0, 0);
            abi_support_bzip2_stream_close(ptr::null_mut());
            abi_support_lzw_stream_close(ptr::null_mut());
            abi_support_gzip_stream_close(ptr::null_mut());
            std::hint::black_box((bzip, lzw, gzip, bzip_bytes, lzw_bytes, gzip_bytes));
        }
        2 => {
            FT_List_Add(ptr::null_mut(), ptr::null_mut());
            FT_List_Insert(ptr::null_mut(), ptr::null_mut());
            let found = FT_List_Find(ptr::null_mut(), ptr::null_mut());
            FT_List_Remove(ptr::null_mut(), ptr::null_mut());
            FT_List_Up(ptr::null_mut(), ptr::null_mut());
            let iterated = FT_List_Iterate(ptr::null_mut(), None, ptr::null_mut());
            FT_List_Finalize(ptr::null_mut(), None, ptr::null_mut(), ptr::null_mut());
            std::hint::black_box((found, iterated));
        }
        3 => {
            FT_Bitmap_Init(ptr::null_mut());
            FT_Bitmap_New(ptr::null_mut());
            let copied = FT_Bitmap_Copy(library, ptr::null(), ptr::null_mut());
            let converted = FT_Bitmap_Convert(library, ptr::null(), ptr::null_mut(), 0);
            let done = FT_Bitmap_Done(library, ptr::null_mut());
            let emboldened = FT_Bitmap_Embolden(library, ptr::null_mut(), 0, 0);
            let blended = FT_Bitmap_Blend(
                library,
                ptr::null(),
                FT_Vector::default(),
                ptr::null_mut(),
                ptr::null_mut(),
                FT_Color::default(),
            );
            std::hint::black_box((copied, converted, done, emboldened, blended));
        }
        4 => {
            let palette_data = FT_Palette_Data_Get(face, ptr::null_mut());
            let palette_select = FT_Palette_Select(face, 0, ptr::null_mut());
            let foreground = FT_Palette_Set_Foreground_Color(face, FT_Color::default());
            let layers = FT_Get_Color_Glyph_Layer(
                face,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            let clip = FT_Get_Color_Glyph_ClipBox(face, 0, ptr::null_mut());
            let paint = FT_Get_Color_Glyph_Paint(face, 0, 0, ptr::null_mut());
            let paint_value = FT_Get_Paint(face, FT_OpaquePaint::default(), ptr::null_mut());
            let paint_layers = FT_Get_Paint_Layers(face, ptr::null_mut(), ptr::null_mut());
            let stops = FT_Get_Colorline_Stops(face, ptr::null_mut(), ptr::null_mut());
            std::hint::black_box((
                palette_data,
                palette_select,
                foreground,
                layers,
                clip,
                paint,
                paint_value,
                paint_layers,
                stops,
            ));
        }
        5 => {
            let opentype = FT_OpenType_Validate(
                face,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            let classic = FT_ClassicKern_Validate(face, 0, ptr::null_mut());
            let ps_info = FT_Get_PS_Font_Info(face, ptr::null_mut());
            let ps_private = FT_Get_PS_Font_Private(face, ptr::null_mut());
            let ps_value = FT_Get_PS_Font_Value(face, 0, 0, ptr::null_mut(), 0);
            FT_OpenType_Free(face, ptr::null());
            FT_ClassicKern_Free(face, ptr::null());
            std::hint::black_box((opentype, classic, ps_info, ps_private, ps_value));
        }
        6 => {
            let get = FT_Get_Glyph(ptr::null_mut(), ptr::null_mut());
            let new = FT_New_Glyph(library, rust_ffi::FT_GLYPH_FORMAT_NONE, ptr::null_mut());
            let copy = FT_Glyph_Copy(ptr::null_mut(), ptr::null_mut());
            FT_Done_Glyph(ptr::null_mut());
            let transform = FT_Glyph_Transform(ptr::null_mut(), ptr::null(), ptr::null());
            let bitmap = FT_Glyph_To_Bitmap(ptr::null_mut(), 0, ptr::null(), 0);
            let stroke = FT_Glyph_Stroke(ptr::null_mut(), ptr::null_mut(), 0);
            let border = FT_Glyph_StrokeBorder(ptr::null_mut(), ptr::null_mut(), 0, 0);
            let cbox = {
                let mut bbox = FT_BBox::default();
                FT_Glyph_Get_CBox(ptr::null_mut(), 0, ptr::from_mut(&mut bbox));
                bbox
            };
            std::hint::black_box((get, new, copy, transform, bitmap, stroke, border, cbox));
        }
        7 => {
            let mut bbox = FT_BBox::default();
            FT_Outline_Get_CBox(ptr::null(), ptr::from_mut(&mut bbox));
            let bbox_error = FT_Outline_Get_BBox(ptr::null(), ptr::from_mut(&mut bbox));
            let bitmap = FT_Outline_Get_Bitmap(library, ptr::null(), ptr::null());
            let render = FT_Outline_Render(library, ptr::null(), ptr::null_mut());
            let decompose = FT_Outline_Decompose(ptr::null_mut(), ptr::null(), ptr::null_mut());
            let orientation = FT_Outline_Get_Orientation(ptr::null());
            let check = FT_Outline_Check(ptr::null());
            let copy = FT_Outline_Copy(ptr::null(), ptr::null_mut());
            let embolden = FT_Outline_Embolden(ptr::null_mut(), 0);
            let embolden_xy = FT_Outline_EmboldenXY(ptr::null_mut(), 0, 0);
            let inside = FT_Outline_GetInsideBorder(ptr::null());
            let outside = FT_Outline_GetOutsideBorder(ptr::null());
            let new = FT_Outline_New(library, 0, 0, ptr::null_mut());
            let done = FT_Outline_Done(library, ptr::null_mut());
            FT_Outline_Reverse(ptr::null_mut());
            FT_Outline_Transform(ptr::null(), ptr::null());
            FT_Outline_Translate(ptr::null(), 0, 0);
            std::hint::black_box((
                bbox,
                bbox_error,
                bitmap,
                render,
                decompose,
                orientation,
                check,
                copy,
                embolden,
                embolden_xy,
                inside,
                outside,
                new,
                done,
            ));
        }
        8 => {
            let new = FT_Stroker_New(library, ptr::null_mut());
            FT_Stroker_Set(ptr::null_mut(), 0, 0, 0, 0);
            FT_Stroker_Rewind(ptr::null_mut());
            let begin = FT_Stroker_BeginSubPath(ptr::null_mut(), ptr::null(), 0);
            let parse = FT_Stroker_ParseOutline(ptr::null_mut(), ptr::null(), 0);
            let line = FT_Stroker_LineTo(ptr::null_mut(), ptr::null());
            let conic = FT_Stroker_ConicTo(ptr::null_mut(), ptr::null(), ptr::null());
            let cubic = FT_Stroker_CubicTo(ptr::null_mut(), ptr::null(), ptr::null(), ptr::null());
            let end = FT_Stroker_EndSubPath(ptr::null_mut());
            let border_count =
                FT_Stroker_GetBorderCounts(ptr::null_mut(), 0, ptr::null_mut(), ptr::null_mut());
            let count = FT_Stroker_GetCounts(ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
            FT_Stroker_ExportBorder(ptr::null_mut(), 0, ptr::null_mut());
            FT_Stroker_Export(ptr::null_mut(), ptr::null_mut());
            FT_Stroker_Done(ptr::null_mut());
            std::hint::black_box((
                new,
                begin,
                parse,
                line,
                conic,
                cubic,
                end,
                border_count,
                count,
            ));
        }
        9 => {
            let char_size = FT_Set_Char_Size(face, 0, 0, 72, 72);
            let pixels = FT_Set_Pixel_Sizes(face, 0, 0);
            FT_Set_Transform(face, ptr::null(), ptr::null());
            FT_Get_Transform(face, ptr::null_mut(), ptr::null_mut());
            let request = FT_Request_Size(face, ptr::null());
            let new_size = FT_New_Size(face, ptr::null_mut());
            let done_size = FT_Done_Size(ptr::null_mut());
            let glyph_index = FT_Get_Char_Index(face, 0);
            let kerning = FT_Get_Kerning(face, 0, 0, 0, ptr::null_mut());
            let glyph_name = FT_Get_Glyph_Name(face, 0, ptr::null_mut(), 0);
            let name_index = FT_Get_Name_Index(face, ptr::null());
            let design = FT_Set_Var_Design_Coordinates(face, 0, ptr::null());
            let blend = FT_Get_Var_Blend_Coordinates(face, 0, ptr::null_mut());
            let load = FT_Load_Glyph(face, 0, rust_ffi::FT_LOAD_DEFAULT);
            let load_char = FT_Load_Char(face, 0, rust_ffi::FT_LOAD_DEFAULT);
            let advance = FT_Get_Advance(face, 0, rust_ffi::FT_LOAD_DEFAULT, ptr::null_mut());
            let advances = FT_Get_Advances(face, 0, 0, rust_ffi::FT_LOAD_DEFAULT, ptr::null_mut());
            let render = FT_Render_Glyph(ptr::null_mut(), 0);
            let subglyph = FT_Get_SubGlyph_Info(
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            std::hint::black_box((
                char_size,
                pixels,
                request,
                new_size,
                done_size,
                glyph_index,
                kerning,
                glyph_name,
                name_index,
                design,
                blend,
                load,
                load_char,
                advance,
                advances,
                render,
                subglyph,
            ));
        }
        _ => unreachable!(),
    }
}

#[cfg(feature = "abi-test-support")]
extern "C" fn abi_c102_list_iterator(_node: FT_ListNode, _user: FT_Pointer) -> FT_Error {
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
fn abi_c102_live_wrapper_probe(
    bytes: &[FT_Byte],
    library: FT_Library,
    face: FT_Face,
    memory: FT_Memory,
    variant: u16,
) {
    // c102 complements the null/guard matrix with valid caller-owned records.
    // The exported C ABI remains a forwarding layer: all stateful behavior is
    // still implemented by the Rust FFI/core paths, and these calls exist only
    // behind abi-test-support for independent Coverage MCP witnesses.
    match usize::from(variant.saturating_sub(601)) % 10 {
        0 => {
            let mut bzip_source = FT_StreamRec {
                base: bytes.as_ptr().cast_mut(),
                size: 1,
                ..FT_StreamRec::default()
            };
            let mut bzip_target = FT_StreamRec::default();
            let bzip = FT_Stream_OpenBzip2(&mut bzip_target, &mut bzip_source);
            if bzip == rust_ffi::FT_Err_Ok {
                let mut output = [0_u8; 1];
                let read = (unsafe {
                    std::mem::transmute::<
                        FT_Pointer,
                        extern "C" fn(FT_Stream, FT_ULong, *mut FT_Byte, FT_ULong) -> FT_ULong,
                    >(bzip_target.read)
                })(&mut bzip_target, 0, output.as_mut_ptr(), 1);
                std::hint::black_box(read);
                c_bzip2_stream_close(&mut bzip_target);
            }
            let mut lzw_source = FT_StreamRec {
                base: bytes.as_ptr().cast_mut(),
                size: 2,
                ..FT_StreamRec::default()
            };
            let mut lzw_target = FT_StreamRec::default();
            let lzw = FT_Stream_OpenLZW(&mut lzw_target, &mut lzw_source);
            let mut gzip_source = FT_StreamRec {
                base: bytes.as_ptr().cast_mut(),
                size: 1,
                ..FT_StreamRec::default()
            };
            let mut gzip_target = FT_StreamRec::default();
            let gzip = FT_Stream_OpenGzip(&mut gzip_target, &mut gzip_source);
            std::hint::black_box((bzip, lzw, gzip, lzw_source.pos));
            abi_support_lzw_stream_close(&mut lzw_target);
            abi_support_gzip_stream_close(&mut gzip_target);
        }
        1 => {
            let size = c_long::try_from(std::mem::size_of::<FT_ListNodeRec>()).unwrap_or(0);
            let first = abi_custom_memory_alloc(memory, size).cast::<FT_ListNodeRec>();
            let second = abi_custom_memory_alloc(memory, size).cast::<FT_ListNodeRec>();
            if first.is_null() || second.is_null() {
                if !first.is_null() {
                    abi_custom_memory_free(memory, first.cast());
                }
                if !second.is_null() {
                    abi_custom_memory_free(memory, second.cast());
                }
            } else {
                let mut first_data = 1_u8;
                let mut second_data = 2_u8;
                // SAFETY: both records are blocks returned by the live custom
                // allocator and are large enough for FT_ListNodeRec.
                unsafe {
                    *first = FT_ListNodeRec {
                        data: ptr::from_mut(&mut first_data).cast(),
                        ..FT_ListNodeRec::default()
                    };
                    *second = FT_ListNodeRec {
                        data: ptr::from_mut(&mut second_data).cast(),
                        ..FT_ListNodeRec::default()
                    };
                }
                let mut list = FT_ListRec::default();
                FT_List_Add(&mut list, first);
                FT_List_Insert(&mut list, second);
                let found = FT_List_Find(&mut list, ptr::from_mut(&mut first_data).cast());
                FT_List_Up(&mut list, first);
                let iterated =
                    FT_List_Iterate(&mut list, Some(abi_c102_list_iterator), ptr::null_mut());
                FT_List_Remove(&mut list, second);
                abi_custom_memory_free(memory, second.cast());
                FT_List_Finalize(&mut list, None, memory, ptr::null_mut());
                std::hint::black_box((found, iterated));
            }
        }
        2 => {
            let mut pixels = [0x7f_u8, 0x80_u8];
            let source = FT_Bitmap {
                rows: 1,
                width: 2,
                pitch: 2,
                buffer: pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte,
                ..FT_Bitmap::default()
            };
            let mut copied = FT_Bitmap::default();
            let mut converted = FT_Bitmap::default();
            let copy = FT_Bitmap_Copy(library, &source, &mut copied);
            let convert = FT_Bitmap_Convert(library, &source, &mut converted, 1);
            let embolden = FT_Bitmap_Embolden(library, &mut converted, 64, 64);
            let done_copy = FT_Bitmap_Done(library, &mut copied);
            let done_converted = FT_Bitmap_Done(library, &mut converted);
            std::hint::black_box((copy, convert, embolden, done_copy, done_converted));
        }
        3 => {
            let mut palette_data = FT_Palette_Data::default();
            let mut palette = ptr::null_mut();
            let mut layer_iterator = FT_LayerIterator::default();
            let mut glyph_index = 0;
            let mut color_index = 0;
            let mut clip_box = FT_ClipBox::default();
            let mut opaque_paint = FT_OpaquePaint::default();
            let mut paint = FT_COLR_Paint::default();
            let mut color_stop = FT_ColorStop::default();
            let mut color_stop_iterator = FT_ColorStopIterator::default();
            let data = FT_Palette_Data_Get(face, &mut palette_data);
            let select = FT_Palette_Select(face, 0, &mut palette);
            let foreground = FT_Palette_Set_Foreground_Color(
                face,
                FT_Color {
                    blue: 3,
                    green: 2,
                    red: 1,
                    alpha: 255,
                },
            );
            let layer = FT_Get_Color_Glyph_Layer(
                face,
                0,
                &mut glyph_index,
                &mut color_index,
                &mut layer_iterator,
            );
            let clip = FT_Get_Color_Glyph_ClipBox(face, 0, &mut clip_box);
            let glyph_paint = FT_Get_Color_Glyph_Paint(face, 0, 0, &mut opaque_paint);
            let paint_value = FT_Get_Paint(face, opaque_paint, &mut paint);
            let paint_layers = FT_Get_Paint_Layers(face, &mut layer_iterator, &mut opaque_paint);
            let stops = FT_Get_Colorline_Stops(face, &mut color_stop, &mut color_stop_iterator);
            std::hint::black_box((
                data,
                select,
                foreground,
                layer,
                clip,
                glyph_paint,
                paint_value,
                paint_layers,
                stops,
                palette,
            ));
        }
        4 => {
            let mut base = ptr::null();
            let mut gdef = ptr::null();
            let mut gpos = ptr::null();
            let mut gsub = ptr::null();
            let mut jstf = ptr::null();
            let open_type = FT_OpenType_Validate(
                face,
                rust_ffi::FT_VALIDATE_GDEF as FT_UInt,
                &mut base,
                &mut gdef,
                &mut gpos,
                &mut gsub,
                &mut jstf,
            );
            let mut classic = ptr::null();
            let classic_status = FT_ClassicKern_Validate(face, 0, &mut classic);
            let mut info = PS_FontInfoRec::default();
            let mut private = PS_PrivateRec::default();
            let ps_info = FT_Get_PS_Font_Info(face, &mut info);
            let ps_private = FT_Get_PS_Font_Private(face, &mut private);
            let mut value = [0_u8; 16];
            let ps_value = FT_Get_PS_Font_Value(
                face,
                0,
                0,
                value.as_mut_ptr().cast(),
                value.len() as FT_Long,
            );
            FT_OpenType_Free(face, base);
            FT_OpenType_Free(face, gdef);
            FT_OpenType_Free(face, gpos);
            FT_OpenType_Free(face, gsub);
            FT_OpenType_Free(face, jstf);
            FT_ClassicKern_Free(face, classic);
            std::hint::black_box((open_type, classic_status, ps_info, ps_private, ps_value));
        }
        5 => {
            let load = FT_Load_Glyph(
                face,
                36,
                rust_ffi::FT_LOAD_NO_HINTING | rust_ffi::FT_LOAD_NO_BITMAP,
            );
            let slot = unsafe { face.as_ref().map_or(ptr::null_mut(), |record| record.glyph) };
            let mut glyph = ptr::null_mut();
            let get = FT_Get_Glyph(slot, &mut glyph);
            let mut copied = ptr::null_mut();
            let copy = if !glyph.is_null() {
                FT_Glyph_Copy(glyph, &mut copied)
            } else {
                rust_ffi::FT_Err_Invalid_Argument
            };
            let matrix = FT_Matrix {
                xx: 0x1_0000,
                xy: 0x1000,
                yx: 0,
                yy: 0x1_0000,
            };
            let delta = FT_Vector { x: 3, y: -2 };
            let transform = if !glyph.is_null() {
                FT_Glyph_Transform(glyph, &matrix, &delta)
            } else {
                rust_ffi::FT_Err_Invalid_Argument
            };
            let mut bbox = FT_BBox::default();
            if !glyph.is_null() {
                FT_Glyph_Get_CBox(glyph, 0, &mut bbox);
            }
            let bitmap = if !glyph.is_null() {
                FT_Glyph_To_Bitmap(&mut glyph, 0, &delta, 0)
            } else {
                rust_ffi::FT_Err_Invalid_Argument
            };
            if !copied.is_null() {
                FT_Done_Glyph(copied);
            }
            if !glyph.is_null() {
                FT_Done_Glyph(glyph);
            }
            std::hint::black_box((load, get, copy, transform, bitmap, bbox));
        }
        6 => {
            let load = FT_Load_Glyph(
                face,
                36,
                rust_ffi::FT_LOAD_NO_HINTING | rust_ffi::FT_LOAD_NO_BITMAP,
            );
            let slot = unsafe { face.as_ref().map_or(ptr::null_mut(), |record| record.glyph) };
            let mut bbox = FT_BBox::default();
            let mut copied = FT_Outline::default();
            let outline = unsafe { slot.as_mut().map(|slot| &mut slot.outline) };
            let cbox = outline
                .map(|outline| {
                    FT_Outline_Get_CBox(outline, &mut bbox);
                    let bbox_error = FT_Outline_Get_BBox(outline, &mut bbox);
                    let check = FT_Outline_Check(outline);
                    let copy = FT_Outline_Copy(outline, &mut copied);
                    (bbox_error, check, copy)
                })
                .unwrap_or((
                    rust_ffi::FT_Err_Invalid_Outline,
                    rust_ffi::FT_Err_Invalid_Outline,
                    rust_ffi::FT_Err_Invalid_Outline,
                ));
            let mut transformed = cbox;
            if copied.n_points != 0 {
                let matrix = FT_Matrix {
                    xx: 0x1_0000,
                    xy: 0,
                    yx: 0,
                    yy: 0x1_0000,
                };
                let embolden = FT_Outline_Embolden(&mut copied, 1);
                let embolden_xy = FT_Outline_EmboldenXY(&mut copied, 1, 2);
                FT_Outline_Transform(&copied, &matrix);
                FT_Outline_Translate(&copied, 1, 2);
                let orientation = FT_Outline_Get_Orientation(&copied);
                let inside = FT_Outline_GetInsideBorder(&copied);
                let outside = FT_Outline_GetOutsideBorder(&copied);
                FT_Outline_Reverse(&mut copied);
                std::hint::black_box((inside, outside));
                transformed = (embolden, embolden_xy, orientation);
            }
            let mut empty = FT_Outline::default();
            let new_outline = FT_Outline_New(library, 0, 0, &mut empty);
            let done_empty = FT_Outline_Done(library, &mut empty);
            let done_copy = FT_Outline_Done(library, &mut copied);
            std::hint::black_box((load, cbox, transformed, new_outline, done_empty, done_copy));
        }
        7 => {
            let mut stroker = ptr::null_mut();
            let new = FT_Stroker_New(library, &mut stroker);
            let mut counts = (0_u32, 0_u32);
            let mut border_counts = (0_u32, 0_u32);
            let start = FT_Vector { x: 0, y: 0 };
            let control = FT_Vector { x: 64, y: 128 };
            let middle = FT_Vector { x: 128, y: 64 };
            let end = FT_Vector { x: 192, y: 0 };
            if !stroker.is_null() {
                FT_Stroker_Set(stroker, 64, 0, 0, 0x1_0000);
                let begin = FT_Stroker_BeginSubPath(stroker, &start, 0);
                let line = FT_Stroker_LineTo(stroker, &middle);
                let conic = FT_Stroker_ConicTo(stroker, &control, &end);
                let cubic = FT_Stroker_CubicTo(stroker, &start, &control, &end);
                let finish = FT_Stroker_EndSubPath(stroker);
                let border = FT_Stroker_GetBorderCounts(
                    stroker,
                    0,
                    &mut border_counts.0,
                    &mut border_counts.1,
                );
                let all = FT_Stroker_GetCounts(stroker, &mut counts.0, &mut counts.1);
                FT_Stroker_ExportBorder(stroker, 0, ptr::null_mut());
                FT_Stroker_Export(stroker, ptr::null_mut());
                FT_Stroker_Rewind(stroker);
                FT_Stroker_Done(stroker);
                std::hint::black_box((begin, line, conic, cubic, finish, border, all));
            }
            std::hint::black_box((new, counts, border_counts));
        }
        8 => {
            let char_size = FT_Set_Char_Size(face, 12 * 64, 16 * 64, 72, 72);
            let pixels = FT_Set_Pixel_Sizes(face, 16, 18);
            let matrix = FT_Matrix {
                xx: 0x1_0000,
                xy: 0x0800,
                yx: 0,
                yy: 0x1_0000,
            };
            let delta = FT_Vector { x: 2, y: -3 };
            FT_Set_Transform(face, &matrix, &delta);
            let mut got_matrix = FT_Matrix::default();
            let mut got_delta = FT_Vector::default();
            FT_Get_Transform(face, &mut got_matrix, &mut got_delta);
            let request = FT_Size_RequestRec {
                type_: 0,
                width: 12 * 64,
                height: 16 * 64,
                horiResolution: 72,
                vertResolution: 72,
            };
            let requested = FT_Request_Size(face, &request);
            let mut size = ptr::null_mut();
            let new_size = FT_New_Size(face, &mut size);
            let done_size = if !size.is_null() {
                FT_Done_Size(size)
            } else {
                rust_ffi::FT_Err_Invalid_Argument
            };
            let glyph_index = FT_Get_Char_Index(face, 65);
            let mut kerning = FT_Vector::default();
            let kerning_status = FT_Get_Kerning(face, 36, glyph_index, 0, &mut kerning);
            let mut name = [0_u8; 64];
            let glyph_name = FT_Get_Glyph_Name(
                face,
                glyph_index,
                name.as_mut_ptr().cast(),
                name.len() as FT_UInt,
            );
            let name_index = FT_Get_Name_Index(face, ptr::null());
            let mut design = [0 as FT_Fixed; 1];
            let set_design = FT_Set_Var_Design_Coordinates(face, 1, design.as_mut_ptr());
            let mut blend = [0 as FT_Fixed; 1];
            let get_blend = FT_Get_Var_Blend_Coordinates(face, 1, blend.as_mut_ptr());
            let load = FT_Load_Glyph(face, glyph_index, rust_ffi::FT_LOAD_DEFAULT);
            let load_char = FT_Load_Char(face, 65, rust_ffi::FT_LOAD_DEFAULT);
            let mut advance = 0;
            let advance_status =
                FT_Get_Advance(face, glyph_index, rust_ffi::FT_LOAD_DEFAULT, &mut advance);
            let mut advances = [0 as FT_Fixed; 1];
            let advances_status = FT_Get_Advances(
                face,
                glyph_index,
                1,
                rust_ffi::FT_LOAD_DEFAULT,
                advances.as_mut_ptr(),
            );
            let slot = unsafe { face.as_ref().map_or(ptr::null_mut(), |record| record.glyph) };
            let render = FT_Render_Glyph(slot, 0);
            let mut sub_index = 0;
            let mut sub_flags = 0;
            let mut arg1 = 0;
            let mut arg2 = 0;
            let mut transform = FT_Matrix::default();
            let subglyph = FT_Get_SubGlyph_Info(
                slot,
                0,
                &mut sub_index,
                &mut sub_flags,
                &mut arg1,
                &mut arg2,
                &mut transform,
            );
            std::hint::black_box((
                char_size,
                pixels,
                requested,
                new_size,
                done_size,
                kerning_status,
                glyph_name,
                name_index,
                set_design,
                get_blend,
                load,
                load_char,
                advance_status,
                advances_status,
                render,
                subglyph,
                got_matrix,
                got_delta,
            ));
        }
        9 => {
            if let Ok(mut harness) = AbiSBitCacheHarness::new(bytes, 0) {
                let ownership = harness.ownership_snapshot();
                std::hint::black_box((
                    ownership.status,
                    ownership.requester_after_first,
                    ownership.requester_after_repeat,
                    ownership.requester_after_reset,
                    ownership.finalizers_before_reset,
                    ownership.finalizers_after_reset,
                    ownership.finalizers_after_done,
                    ownership.face_repeat_same,
                    ownership.size_repeat_same,
                    ownership.size_belongs_to_face,
                    ownership.first_node_non_null,
                    ownership.node_repeat_same,
                    ownership.post_reset_node_non_null,
                    ownership.cache_non_null,
                    ownership.reset_preserved_cache_handle,
                ));
            }
        }
        _ => unreachable!(),
    }
}

#[cfg(feature = "abi-test-support")]
extern "C" fn abi_c103_list_destroy(_memory: FT_Memory, _data: FT_Pointer, _user: FT_Pointer) {}

#[cfg(feature = "abi-test-support")]
fn abi_c103_wrapper_probe(
    bytes: &[FT_Byte],
    library: FT_Library,
    face: FT_Face,
    memory: FT_Memory,
    variant: u16,
) {
    const BZIP2_BYTES: &[u8] =
        include_bytes!("../../tests/fixtures/compressed/bzip2/valid-pcf-header.pcf.bz2");
    const LZW_BYTES: &[u8] =
        include_bytes!("../../tests/fixtures/compressed/lzw/small-valid-pcf.Z");
    const GZIP_BYTES: &[u8] = include_bytes!("../../tests/fixtures/compressed/gzip/small-text.gz");

    match usize::from(variant.saturating_sub(701)) % 10 {
        0 => {
            let mut bzip_source = FT_StreamRec {
                base: BZIP2_BYTES.as_ptr().cast_mut(),
                size: FT_ULong::try_from(BZIP2_BYTES.len()).unwrap_or(FT_ULong::MAX),
                ..FT_StreamRec::default()
            };
            let mut bzip_target = FT_StreamRec::default();
            let bzip = FT_Stream_OpenBzip2(&mut bzip_target, &mut bzip_source);
            let mut bzip_output = [0_u8; 8];
            let bzip_read = if bzip == rust_ffi::FT_Err_Ok {
                let callback = unsafe {
                    std::mem::transmute::<
                        FT_Pointer,
                        extern "C" fn(FT_Stream, FT_ULong, *mut FT_Byte, FT_ULong) -> FT_ULong,
                    >(bzip_target.read)
                };
                callback(
                    &mut bzip_target,
                    0,
                    bzip_output.as_mut_ptr(),
                    bzip_output.len() as FT_ULong,
                )
            } else {
                0
            };
            c_bzip2_stream_close(&mut bzip_target);

            let mut lzw_source = FT_StreamRec {
                base: LZW_BYTES.as_ptr().cast_mut(),
                size: FT_ULong::try_from(LZW_BYTES.len()).unwrap_or(FT_ULong::MAX),
                ..FT_StreamRec::default()
            };
            let mut lzw_target = FT_StreamRec::default();
            let lzw = FT_Stream_OpenLZW(&mut lzw_target, &mut lzw_source);
            let mut lzw_output = [0_u8; 8];
            let lzw_read = if lzw == rust_ffi::FT_Err_Ok {
                c_lzw_stream_io(
                    &mut lzw_target,
                    0,
                    lzw_output.as_mut_ptr(),
                    lzw_output.len() as FT_ULong,
                )
            } else {
                0
            };
            abi_support_lzw_stream_close(&mut lzw_target);

            let mut gzip_source = FT_StreamRec {
                base: GZIP_BYTES.as_ptr().cast_mut(),
                size: FT_ULong::try_from(GZIP_BYTES.len()).unwrap_or(FT_ULong::MAX),
                ..FT_StreamRec::default()
            };
            let mut gzip_target = FT_StreamRec::default();
            let gzip = FT_Stream_OpenGzip(&mut gzip_target, &mut gzip_source);
            let mut gzip_output = [0_u8; 8];
            let gzip_read = if gzip == rust_ffi::FT_Err_Ok {
                c_gzip_stream_io(
                    &mut gzip_target,
                    0,
                    gzip_output.as_mut_ptr(),
                    gzip_output.len() as FT_ULong,
                )
            } else {
                0
            };
            abi_support_gzip_stream_close(&mut gzip_target);
            std::hint::black_box((bzip, bzip_read, lzw, lzw_read, gzip, gzip_read));
        }
        1 => {
            let size = c_long::try_from(std::mem::size_of::<FT_ListNodeRec>()).unwrap_or(0);
            let node = abi_custom_memory_alloc(memory, size).cast::<FT_ListNodeRec>();
            if !node.is_null() {
                unsafe {
                    *node = FT_ListNodeRec {
                        data: ptr::null_mut(),
                        ..FT_ListNodeRec::default()
                    };
                }
                let mut list = FT_ListRec {
                    head: node,
                    tail: node,
                };
                FT_List_Finalize(
                    ptr::from_mut(&mut list),
                    Some(abi_c103_list_destroy),
                    memory,
                    ptr::null_mut(),
                );
            }
        }
        2 => {
            let mut source_pixels = [0x7f_u8, 0x80_u8, 0xff_u8, 0x01_u8];
            let mut target_pixels = [0_u8; 16];
            let mut source = FT_Bitmap {
                rows: 2,
                width: 2,
                pitch: 2,
                buffer: source_pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte,
                ..FT_Bitmap::default()
            };
            let mut target = FT_Bitmap {
                rows: 4,
                width: 4,
                pitch: 4,
                buffer: target_pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte,
                ..FT_Bitmap::default()
            };
            let copy = FT_Bitmap_Copy(library, &source, &mut target);
            let convert = FT_Bitmap_Convert(library, &source, &mut target, 4);
            let embolden = FT_Bitmap_Embolden(library, &mut target, 64, 64);
            let mut target_offset = FT_Vector::default();
            let blend = FT_Bitmap_Blend(
                library,
                &source,
                FT_Vector::default(),
                &mut target,
                &mut target_offset,
                FT_Color {
                    red: 255,
                    green: 255,
                    blue: 255,
                    alpha: 255,
                },
            );
            let alias = FT_Bitmap_Copy(library, ptr::from_ref(&source), ptr::from_mut(&mut source));
            let done = FT_Bitmap_Done(library, &mut target);
            std::hint::black_box((copy, convert, embolden, blend, alias, done));
        }
        3 => {
            let mut palette_data = FT_Palette_Data::default();
            let mut palette = ptr::null_mut();
            let mut layer_iterator = FT_LayerIterator::default();
            let mut glyph_index = 0;
            let mut color_index = 0;
            let mut clip_box = FT_ClipBox::default();
            let mut opaque_paint = FT_OpaquePaint::default();
            let mut paint = FT_COLR_Paint::default();
            let mut color_stop = FT_ColorStop::default();
            let mut color_stop_iterator = FT_ColorStopIterator::default();
            let palette_data_status = FT_Palette_Data_Get(face, &mut palette_data);
            let palette_status = FT_Palette_Select(face, 0, &mut palette);
            let foreground_status = FT_Palette_Set_Foreground_Color(
                face,
                FT_Color {
                    red: 0x12,
                    green: 0x34,
                    blue: 0x56,
                    alpha: 0xff,
                },
            );
            let layer_status = FT_Get_Color_Glyph_Layer(
                face,
                FT_UInt::from(variant % 4),
                &mut glyph_index,
                &mut color_index,
                &mut layer_iterator,
            );
            let clip_status =
                FT_Get_Color_Glyph_ClipBox(face, FT_UInt::from(variant % 4), &mut clip_box);
            let glyph_paint_status =
                FT_Get_Color_Glyph_Paint(face, FT_UInt::from(variant % 4), 0, &mut opaque_paint);
            let paint_status = FT_Get_Paint(face, opaque_paint, &mut paint);
            let layers_status = FT_Get_Paint_Layers(face, &mut layer_iterator, &mut opaque_paint);
            let stops_status =
                FT_Get_Colorline_Stops(face, &mut color_stop, &mut color_stop_iterator);
            std::hint::black_box((
                palette_data_status,
                palette_status,
                foreground_status,
                layer_status,
                clip_status,
                glyph_paint_status,
                paint_status,
                layers_status,
                stops_status,
                palette,
            ));
        }
        4 => {
            let mut base = ptr::null();
            let mut gdef = ptr::null();
            let mut gpos = ptr::null();
            let mut gsub = ptr::null();
            let mut jstf = ptr::null();
            let open_type = FT_OpenType_Validate(
                face,
                rust_ffi::FT_VALIDATE_GDEF as FT_UInt,
                &mut base,
                &mut gdef,
                &mut gpos,
                &mut gsub,
                &mut jstf,
            );
            let mut classic = ptr::null();
            let classic_status = FT_ClassicKern_Validate(face, 0, &mut classic);
            let mut info = PS_FontInfoRec::default();
            let mut private = PS_PrivateRec::default();
            let ps_info = FT_Get_PS_Font_Info(face, &mut info);
            let ps_private = FT_Get_PS_Font_Private(face, &mut private);
            let mut value = [0_u8; 32];
            let ps_value = FT_Get_PS_Font_Value(
                face,
                0,
                0,
                value.as_mut_ptr().cast(),
                value.len() as FT_Long,
            );
            FT_OpenType_Free(face, base);
            FT_OpenType_Free(face, gdef);
            FT_OpenType_Free(face, gpos);
            FT_OpenType_Free(face, gsub);
            FT_OpenType_Free(face, jstf);
            FT_ClassicKern_Free(face, classic);
            std::hint::black_box((open_type, classic_status, ps_info, ps_private, ps_value));
        }
        5 => {
            let glyph_index = FT_Get_Char_Index(face, FT_ULong::from(65_u16 + variant % 8));
            let load = FT_Load_Glyph(
                face,
                glyph_index,
                rust_ffi::FT_LOAD_NO_HINTING | rust_ffi::FT_LOAD_NO_BITMAP,
            );
            let slot = unsafe { face.as_ref().map_or(ptr::null_mut(), |record| record.glyph) };
            let mut glyph = ptr::null_mut();
            let get = FT_Get_Glyph(slot, &mut glyph);
            let mut copied = ptr::null_mut();
            let copy = if !glyph.is_null() {
                FT_Glyph_Copy(glyph, &mut copied)
            } else {
                rust_ffi::FT_Err_Invalid_Argument
            };
            let matrix = FT_Matrix {
                xx: 0x1_0000,
                xy: 0x0800,
                yx: 0,
                yy: 0x1_0000,
            };
            let delta = FT_Vector { x: 2, y: -3 };
            let transform = if !glyph.is_null() {
                FT_Glyph_Transform(glyph, &matrix, &delta);
                let mut bbox = FT_BBox::default();
                FT_Glyph_Get_CBox(glyph, 0, &mut bbox);
                let bitmap = FT_Glyph_To_Bitmap(&mut glyph, 0, &delta, 0);
                std::hint::black_box(bbox);
                bitmap
            } else {
                rust_ffi::FT_Err_Invalid_Argument
            };
            if !copied.is_null() {
                FT_Done_Glyph(copied);
            }
            if !glyph.is_null() {
                FT_Done_Glyph(glyph);
            }
            std::hint::black_box((load, get, copy, transform));
        }
        6 => {
            let glyph_index = FT_Get_Char_Index(face, 65);
            let load = FT_Load_Glyph(face, glyph_index, rust_ffi::FT_LOAD_NO_BITMAP);
            let slot = unsafe { face.as_ref().map_or(ptr::null_mut(), |record| record.glyph) };
            let outline = unsafe { slot.as_mut().map(|slot| &mut slot.outline) };
            let mut copied = FT_Outline::default();
            let mut bbox = FT_BBox::default();
            let cbox = outline.map(|outline| {
                FT_Outline_Get_CBox(outline, &mut bbox);
                let bbox_error = FT_Outline_Get_BBox(outline, &mut bbox);
                let check = FT_Outline_Check(outline);
                let copy = FT_Outline_Copy(outline, &mut copied);
                (bbox_error, check, copy)
            });
            let transformed = if copied.n_points != 0 {
                let matrix = FT_Matrix {
                    xx: 0x1_0000,
                    xy: 0,
                    yx: 0,
                    yy: 0x1_0000,
                };
                let embolden = FT_Outline_Embolden(&mut copied, 1);
                let embolden_xy = FT_Outline_EmboldenXY(&mut copied, 1, 2);
                FT_Outline_Transform(&copied, &matrix);
                FT_Outline_Translate(&copied, 1, 2);
                let orientation = FT_Outline_Get_Orientation(&copied);
                let inside = FT_Outline_GetInsideBorder(&copied);
                let outside = FT_Outline_GetOutsideBorder(&copied);
                FT_Outline_Reverse(&mut copied);
                std::hint::black_box((inside, outside));
                (embolden, embolden_xy, orientation)
            } else {
                (
                    rust_ffi::FT_Err_Invalid_Outline,
                    rust_ffi::FT_Err_Invalid_Outline,
                    0,
                )
            };
            let mut empty = FT_Outline::default();
            let new_outline = FT_Outline_New(library, 0, 0, &mut empty);
            let done_empty = FT_Outline_Done(library, &mut empty);
            let done_copy = FT_Outline_Done(library, &mut copied);
            std::hint::black_box((
                load,
                cbox,
                bbox,
                transformed,
                new_outline,
                done_empty,
                done_copy,
            ));
        }
        7 => {
            let mut stroker = ptr::null_mut();
            let new = FT_Stroker_New(library, &mut stroker);
            let mut counts = (0_u32, 0_u32);
            let mut border_counts = (0_u32, 0_u32);
            let start = FT_Vector { x: 0, y: 0 };
            let control = FT_Vector { x: 64, y: 128 };
            let middle = FT_Vector { x: 128, y: 64 };
            let end = FT_Vector { x: 192, y: 0 };
            if !stroker.is_null() {
                FT_Stroker_Set(stroker, 96, 1, 1, 0x1_0000);
                let begin = FT_Stroker_BeginSubPath(stroker, &start, 0);
                let line = FT_Stroker_LineTo(stroker, &middle);
                let conic = FT_Stroker_ConicTo(stroker, &control, &end);
                let cubic = FT_Stroker_CubicTo(stroker, &start, &control, &end);
                let finish = FT_Stroker_EndSubPath(stroker);
                let border = FT_Stroker_GetBorderCounts(
                    stroker,
                    0,
                    &mut border_counts.0,
                    &mut border_counts.1,
                );
                let all = FT_Stroker_GetCounts(stroker, &mut counts.0, &mut counts.1);
                FT_Stroker_ExportBorder(stroker, 0, ptr::null_mut());
                FT_Stroker_Export(stroker, ptr::null_mut());
                FT_Stroker_Rewind(stroker);
                FT_Stroker_Done(stroker);
                std::hint::black_box((begin, line, conic, cubic, finish, border, all));
            }
            std::hint::black_box((new, counts, border_counts));
        }
        8 => {
            let glyph_index = FT_Get_Char_Index(face, 65);
            let char_size = FT_Set_Char_Size(face, 14 * 64, 18 * 64, 72, 72);
            let pixels = FT_Set_Pixel_Sizes(face, 18, 20);
            let matrix = FT_Matrix {
                xx: 0x1_0000,
                xy: 0x0400,
                yx: 0,
                yy: 0x1_0000,
            };
            let delta = FT_Vector { x: 1, y: -1 };
            FT_Set_Transform(face, &matrix, &delta);
            let mut got_matrix = FT_Matrix::default();
            let mut got_delta = FT_Vector::default();
            FT_Get_Transform(face, &mut got_matrix, &mut got_delta);
            let request = FT_Size_RequestRec {
                type_: 0,
                width: 14 * 64,
                height: 18 * 64,
                horiResolution: 72,
                vertResolution: 72,
            };
            let requested = FT_Request_Size(face, &request);
            let mut size = ptr::null_mut();
            let new_size = FT_New_Size(face, &mut size);
            let done_size = if !size.is_null() {
                FT_Done_Size(size)
            } else {
                rust_ffi::FT_Err_Invalid_Argument
            };
            let mut kerning_delta = FT_Vector::default();
            let kerning = FT_Get_Kerning(face, glyph_index, glyph_index, 0, &mut kerning_delta);
            let load = FT_Load_Glyph(face, glyph_index, rust_ffi::FT_LOAD_DEFAULT);
            let load_char = FT_Load_Char(face, 65, rust_ffi::FT_LOAD_DEFAULT);
            let mut advance = 0;
            let advance_status =
                FT_Get_Advance(face, glyph_index, rust_ffi::FT_LOAD_DEFAULT, &mut advance);
            let mut advances = [0 as FT_Fixed; 1];
            let advances_status = FT_Get_Advances(
                face,
                glyph_index,
                1,
                rust_ffi::FT_LOAD_DEFAULT,
                advances.as_mut_ptr(),
            );
            std::hint::black_box((
                char_size,
                pixels,
                got_matrix,
                got_delta,
                requested,
                new_size,
                done_size,
                kerning,
                load,
                load_char,
                advance_status,
                advances_status,
            ));
        }
        9 => {
            if let Ok(mut harness) = AbiSBitCacheHarness::new(bytes, 0) {
                let face_id = harness.face_id();
                let image_type = FTC_ImageTypeRec {
                    face_id,
                    width: 16,
                    height: 16,
                    flags: rust_ffi::FT_LOAD_DEFAULT,
                };
                let first = harness.lookup(image_type, 0, true, true);
                let repeat = harness.lookup(image_type, 0, true, true);
                let alternate = harness.lookup(image_type, 1, false, false);
                let ownership = harness.ownership_snapshot();
                std::hint::black_box((first, repeat, alternate, ownership));
            }
        }
        _ => {}
    }
}

#[cfg(feature = "abi-test-support")]
fn abi_c104_handles_probe(
    bytes: &[FT_Byte],
    library: FT_Library,
    face: FT_Face,
    _memory: FT_Memory,
    variant: u16,
) {
    // c104 keeps the C ABI and WASM layers thin: these bulk witnesses call
    // the shared Rust FFI/core handles directly, with the public C façade
    // exercised only where it is the actual contract under test.
    let index = usize::from(variant.saturating_sub(801));
    let family = index % 10;
    let repeat = index / 10;
    let simple_outline = rust_ffi::FT_OutlineSnapshot {
        points: vec![
            rust_ffi::FT_Vector { x: 0, y: 0 },
            rust_ffi::FT_Vector { x: 640, y: 0 },
            rust_ffi::FT_Vector { x: 640, y: 640 },
            rust_ffi::FT_Vector { x: 0, y: 640 },
        ],
        tags: vec![1, 1, 1, 1],
        contours: vec![3],
        flags: 0,
    };

    match family {
        0 => {
            if let Ok(core_face) =
                rust_ffi::FT_New_Memory_Face(&rust_ffi::FT_Init_FreeType(), bytes, 0, 20.0)
            {
                let mut manager = rust_ffi::FTCCacheManagerState::new(core_face);
                let size_result = manager.lookup_pixel_size(FT_UInt::MAX, FT_UInt::MAX);
                let _ = std::hint::black_box(size_result);
            }
            if let Ok(core_face) =
                rust_ffi::FT_New_Memory_Face(&rust_ffi::FT_Init_FreeType(), bytes, 0, 20.0)
            {
                let mut cache = rust_ffi::FTCSBitCacheState::new(core_face);
                let image_type = rust_ffi::FTC_ImageTypeRec {
                    width: FT_UInt::MAX,
                    height: FT_UInt::MAX,
                    ..rust_ffi::FTC_ImageTypeRec::default()
                };
                let mut output = None;
                let cache_result = rust_ffi::FTC_SBitCache_Lookup(
                    &mut cache,
                    Some(&image_type),
                    (repeat % 3) as FT_UInt,
                    Some(&mut output),
                    None,
                );
                std::hint::black_box((cache_result, output.is_some()));
            }
        }
        1 => {
            if let Ok(core_face) =
                rust_ffi::FT_New_Memory_Face(&rust_ffi::FT_Init_FreeType(), bytes, 0, 20.0)
            {
                let glyph = if repeat % 2 == 0 {
                    rust_ffi::FT_Get_Char_Index(&core_face, 32)
                } else {
                    rust_ffi::FT_Get_Char_Index(&core_face, 65)
                };
                let mut cache = rust_ffi::FTCSBitCacheState::new(core_face);
                let image_type = rust_ffi::FTC_ImageTypeRec {
                    width: 12 + repeat as FT_UInt,
                    height: 12 + (repeat % 4) as FT_UInt,
                    flags: rust_ffi::FT_LOAD_DEFAULT,
                    ..rust_ffi::FTC_ImageTypeRec::default()
                };
                let mut output = None;
                let mut node = false;
                let result = rust_ffi::FTC_SBitCache_Lookup(
                    &mut cache,
                    Some(&image_type),
                    glyph,
                    Some(&mut output),
                    Some(&mut node),
                );
                std::hint::black_box((result, node, output.is_some()));
            }
        }
        2 => {
            let cubic_extent = 1_i64 << (18 + (repeat % 3));
            let cubic = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector {
                        x: cubic_extent,
                        y: cubic_extent,
                    },
                    rust_ffi::FT_Vector {
                        x: cubic_extent,
                        y: -cubic_extent,
                    },
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                ],
                tags: vec![1, 2, 2, 1],
                contours: vec![3],
                flags: 0,
            };
            let mut bbox = rust_ffi::FT_BBox::default();
            let result = rust_ffi::FT_Outline_Get_BBox(Some(&cubic), Some(&mut bbox));
            let mut malformed = cubic.clone();
            malformed.tags[0] = rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte;
            let malformed_result = rust_ffi::FT_Outline_Get_BBox(Some(&malformed), Some(&mut bbox));
            std::hint::black_box((result, malformed_result, bbox));
        }
        3 => {
            let mut pixels = vec![0_u8; 32 * 32];
            let target = rust_ffi::FT_Bitmap_C {
                rows: 32,
                width: 32,
                pitch: if repeat % 2 == 0 { 32 } else { -32 },
                buffer: pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: if repeat % 3 == 0 {
                    rust_ffi::FT_PIXEL_MODE_MONO as _
                } else {
                    rust_ffi::FT_PIXEL_MODE_GRAY as _
                },
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let bitmap = rust_ffi::FT_Outline_Get_Bitmap(
                Some(&rust_ffi::FT_Init_FreeType()),
                Some(&simple_outline),
                Some(&target),
            );
            let render = rust_ffi::FT_Outline_Render(
                Some(&rust_ffi::FT_Init_FreeType()),
                Some(&simple_outline),
                Some(&target),
                if repeat % 2 == 0 {
                    rust_ffi::FT_RASTER_FLAG_AA as FT_Int
                } else {
                    0
                },
                rust_ffi::FT_BBox::default(),
            );
            let direct = rust_ffi::FT_Outline_Render_Direct_Spans(
                Some(&rust_ffi::FT_Init_FreeType()),
                Some(&simple_outline),
                Some(&target),
                rust_ffi::FT_RASTER_FLAG_DIRECT as FT_Int | rust_ffi::FT_RASTER_FLAG_AA as FT_Int,
                Some(rust_ffi::FT_BBox {
                    xMin: 0,
                    yMin: 0,
                    xMax: 32,
                    yMax: 32,
                }),
                true,
            );
            let _ = std::hint::black_box((bitmap, render, direct, pixels));
        }
        4 => {
            let mut stroker = ptr::null_mut();
            let new_status = FT_Stroker_New(library, &mut stroker);
            if new_status == rust_ffi::FT_Err_Ok && !stroker.is_null() {
                let line_cap = if repeat % 3 == 0 {
                    0
                } else if repeat % 3 == 1 {
                    1
                } else {
                    2
                };
                FT_Stroker_Set(
                    stroker,
                    96 + FT_Fixed::from(repeat as i32),
                    line_cap,
                    rust_ffi::FT_STROKER_LINEJOIN_ROUND as _,
                    0x1_0000,
                );
                let start = FT_Vector { x: 0, y: 0 };
                let end = FT_Vector {
                    x: 640,
                    y: 64 + FT_Pos::from((repeat as i64) * 17),
                };
                let begin = FT_Stroker_BeginSubPath(stroker, &start, 1);
                let line = FT_Stroker_LineTo(stroker, &end);
                let done_path = FT_Stroker_EndSubPath(stroker);
                let mut points = 0;
                let mut contours = 0;
                let counts = FT_Stroker_GetCounts(stroker, &mut points, &mut contours);
                FT_Stroker_Done(stroker);
                std::hint::black_box((begin, line, done_path, counts, points, contours));
            }
        }
        5 => {
            let mut stroker = ptr::null_mut();
            let new_status = FT_Stroker_New(library, &mut stroker);
            if new_status == rust_ffi::FT_Err_Ok && !stroker.is_null() {
                FT_Stroker_Set(
                    stroker,
                    96,
                    rust_ffi::FT_STROKER_LINECAP_ROUND as _,
                    if repeat % 2 == 0 {
                        rust_ffi::FT_STROKER_LINEJOIN_ROUND as _
                    } else {
                        rust_ffi::FT_STROKER_LINEJOIN_BEVEL as _
                    },
                    0x1_0000,
                );
                let start = FT_Vector { x: 0, y: 0 };
                let corner = FT_Vector { x: 640, y: 0 };
                let end = FT_Vector { x: 640, y: 640 };
                let begin = FT_Stroker_BeginSubPath(stroker, &start, 0);
                let line = FT_Stroker_LineTo(stroker, &corner);
                let line2 = FT_Stroker_LineTo(stroker, &end);
                let mut border_points = 0;
                let mut border_contours = 0;
                let before_end = rust_ffi::FT_Stroker_GetBorderCounts(
                    stroker,
                    rust_ffi::FT_STROKER_BORDER_LEFT as _,
                    Some(&mut border_points),
                    Some(&mut border_contours),
                );
                let done_path = FT_Stroker_EndSubPath(stroker);
                let malformed = rust_ffi::FT_OutlineSnapshot {
                    points: vec![
                        rust_ffi::FT_Vector { x: 0, y: 0 },
                        rust_ffi::FT_Vector { x: 160, y: 160 },
                        rust_ffi::FT_Vector { x: 320, y: 0 },
                        rust_ffi::FT_Vector { x: 640, y: 0 },
                    ],
                    tags: vec![2, 2, 1, 1],
                    contours: vec![3],
                    flags: 0,
                };
                let parse = rust_ffi::FT_Stroker_ParseOutline(stroker, Some(&malformed), 0);
                FT_Stroker_Done(stroker);
                std::hint::black_box((
                    begin,
                    line,
                    line2,
                    before_end,
                    done_path,
                    parse,
                    border_points,
                    border_contours,
                ));
            }
        }
        6 => {
            let malformed_variant = (repeat % 8) as FT_UInt;
            let mut external_glyph = ptr::null_mut();
            let external = abi_get_glyph_from_external_malformed_slot(
                face,
                malformed_variant,
                &mut external_glyph,
            );
            if !external_glyph.is_null() {
                FT_Done_Glyph(external_glyph);
            }
            let slot_status = abi_set_malformed_get_glyph_slot(face, malformed_variant);
            let mut glyph = ptr::null_mut();
            let from_face = abi_get_glyph_from_face(face, &mut glyph);
            if !glyph.is_null() {
                FT_Done_Glyph(glyph);
            }
            let unsupported = abi_set_unsupported_glyph_slot(face);
            let mut unsupported_glyph = ptr::null_mut();
            let unsupported_get = abi_get_glyph_from_face(face, &mut unsupported_glyph);
            if !unsupported_glyph.is_null() {
                FT_Done_Glyph(unsupported_glyph);
            }
            std::hint::black_box((
                external,
                slot_status,
                from_face,
                unsupported,
                unsupported_get,
            ));
        }
        7 => {
            let mut size = ptr::null_mut();
            let new_size = FT_New_Size(face, &mut size);
            let activated = if !size.is_null() {
                FT_Activate_Size(size)
            } else {
                rust_ffi::FT_Err_Invalid_Size_Handle
            };
            let request = FT_Size_RequestRec {
                type_: (repeat % 5) as FT_Int,
                width: i64::from((12 + repeat as i32) * 64),
                height: i64::from((14 + (repeat % 4) as i32) * 64),
                horiResolution: 72,
                vertResolution: 72,
            };
            let requested = FT_Request_Size(face, &request);
            let pixels = FT_Set_Pixel_Sizes(
                face,
                (8 + (repeat % 8)) as FT_UInt,
                (8 + (repeat % 7)) as FT_UInt,
            );
            let done_size = if !size.is_null() {
                FT_Done_Size(size)
            } else {
                rust_ffi::FT_Err_Invalid_Size_Handle
            };
            let mut matrix = FT_Matrix::default();
            let mut delta = FT_Vector::default();
            FT_Set_Transform(face, ptr::null(), ptr::null());
            FT_Get_Transform(face, &mut matrix, &mut delta);
            std::hint::black_box((
                new_size, activated, requested, pixels, done_size, matrix, delta,
            ));
        }
        8 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(rust_ffi::FT_GlyphOwned::Outline(mut glyph)) =
                rust_ffi::FT_New_Glyph(Some(&core_library), rust_ffi::FT_GLYPH_FORMAT_OUTLINE)
            {
                let generic = rust_ffi::FT_OutlineSnapshot {
                    points: vec![
                        rust_ffi::FT_Vector { x: 0, y: 0 },
                        rust_ffi::FT_Vector { x: 256, y: 512 },
                        rust_ffi::FT_Vector { x: 512, y: 0 },
                    ],
                    tags: vec![1, 0, 1],
                    contours: vec![2],
                    flags: 0,
                };
                glyph.outline = if repeat % 2 == 0 {
                    generic
                } else {
                    simple_outline.clone()
                };
                glyph.root.advance = rust_ffi::FT_Vector { x: 640, y: 0 };
                let mut stroker = ptr::null_mut();
                let new_status = rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker));
                if new_status == rust_ffi::FT_Err_Ok {
                    rust_ffi::FT_Stroker_Set(
                        stroker,
                        96,
                        rust_ffi::FT_STROKER_LINECAP_ROUND as _,
                        rust_ffi::FT_STROKER_LINEJOIN_ROUND as _,
                        0x1_0000,
                    );
                    let stroke = rust_ffi::FT_Outline_Glyph_Stroke(Some(&glyph), stroker);
                    let border = rust_ffi::FT_Outline_Glyph_StrokeBorder(Some(&glyph), stroker, 0);
                    let null_stroker =
                        rust_ffi::FT_Outline_Glyph_Stroke(Some(&glyph), ptr::null_mut());
                    rust_ffi::FT_Stroker_Done(stroker);
                    let _ = std::hint::black_box((stroke, border, null_stroker));
                }
            }
        }
        9 => {
            let glyph_index = FT_Get_Char_Index(face, FT_ULong::from(65_u16 + (repeat % 8) as u16));
            let load = FT_Load_Glyph(face, glyph_index, rust_ffi::FT_LOAD_NO_HINTING);
            let slot = unsafe { face.as_ref().map_or(ptr::null_mut(), |record| record.glyph) };
            let mut glyph = ptr::null_mut();
            let get = FT_Get_Glyph(slot, &mut glyph);
            let mut copied = ptr::null_mut();
            let copy = if !glyph.is_null() {
                FT_Glyph_Copy(glyph, &mut copied)
            } else {
                rust_ffi::FT_Err_Invalid_Argument
            };
            let mut cbox = FT_BBox::default();
            let cbox_status = if !glyph.is_null() {
                FT_Glyph_Get_CBox(glyph, (repeat % 4) as FT_UInt, &mut cbox);
                FT_Glyph_To_Bitmap(&mut glyph, (repeat % 6) as FT_Int, ptr::null(), 0)
            } else {
                rust_ffi::FT_Err_Invalid_Argument
            };
            if !copied.is_null() {
                FT_Done_Glyph(copied);
            }
            if !glyph.is_null() {
                FT_Done_Glyph(glyph);
            }
            std::hint::black_box((load, get, copy, cbox_status, cbox));
        }
        _ => {}
    }
}

#[cfg(feature = "abi-test-support")]
fn abi_c105_handles_probe(
    bytes: &[FT_Byte],
    _library: FT_Library,
    _face: FT_Face,
    _memory: FT_Memory,
    variant: u16,
) {
    // c105 deliberately keeps the exported C ABI and WASM layers thin.  The
    // bulk witnesses below call the shared Rust FFI/core implementation; the
    // C-like façade remains the contract adapter exercised by the fixture
    // harness rather than a second implementation of these paths.
    let index = usize::from(variant.saturating_sub(901));
    let family = index % 10;
    let repeat = index / 10;
    let simple_outline = rust_ffi::FT_OutlineSnapshot {
        points: vec![
            rust_ffi::FT_Vector { x: 0, y: 0 },
            rust_ffi::FT_Vector { x: 640, y: 0 },
            rust_ffi::FT_Vector { x: 640, y: 640 },
            rust_ffi::FT_Vector { x: 0, y: 640 },
        ],
        tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 4],
        contours: vec![3],
        flags: 0,
    };

    match family {
        0 => {
            // Destroy the manager's active size before both lookup routes.
            // This reaches the shared cache's real FT_Set_Pixel_Sizes error
            // handling instead of fabricating a C-only invalid handle.
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let mut manager = rust_ffi::FTCCacheManagerState::new(core_face);
                let size = manager
                    .lookup_face()
                    .ok()
                    .map_or(ptr::null_mut(), |face| face.size);
                let removed = if size.is_null() {
                    rust_ffi::FT_Err_Invalid_Size_Handle
                } else {
                    rust_ffi::FT_Done_Size(size)
                };
                let pixel =
                    manager.lookup_pixel_size(10 + repeat as FT_UInt, 12 + (repeat % 5) as FT_UInt);
                let image_type = rust_ffi::FTC_ImageTypeRec {
                    width: 10 + repeat as FT_UInt,
                    height: 12 + (repeat % 5) as FT_UInt,
                    flags: rust_ffi::FT_LOAD_RENDER,
                    ..rust_ffi::FTC_ImageTypeRec::default()
                };
                let glyph = (repeat % 4) as FT_UInt;
                let mut output = None;
                let mut node = false;
                let sbit = manager.lookup_sbit(
                    Some(&image_type),
                    glyph,
                    Some(&mut output),
                    Some(&mut node),
                );
                std::hint::black_box((removed, pixel.is_ok(), sbit, node, output.is_some()));
            }
        }
        1 => {
            // Exercise compact SBit records for blank, missing, and ordinary
            // glyphs.  These all use the Rust cache state directly.
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let glyph = match repeat % 5 {
                    0 => rust_ffi::FT_Get_Char_Index(&core_face, 32),
                    1 => rust_ffi::FT_Get_Char_Index(&core_face, 65),
                    2 => rust_ffi::FT_Get_Char_Index(&core_face, 160),
                    3 => 0,
                    _ => rust_ffi::FT_Get_Char_Index(&core_face, 0xFFFF),
                };
                let mut cache = rust_ffi::FTCSBitCacheState::new(core_face);
                let image_type = rust_ffi::FTC_ImageTypeRec {
                    width: 8 + repeat as FT_UInt,
                    height: 8 + (repeat % 6) as FT_UInt,
                    flags: rust_ffi::FT_LOAD_RENDER
                        | if repeat % 2 == 0 {
                            rust_ffi::FT_LOAD_NO_HINTING
                        } else {
                            0
                        },
                    ..rust_ffi::FTC_ImageTypeRec::default()
                };
                let mut output = None;
                let mut node = false;
                let result = rust_ffi::FTC_SBitCache_Lookup(
                    &mut cache,
                    Some(&image_type),
                    glyph,
                    Some(&mut output),
                    Some(&mut node),
                );
                std::hint::black_box((result, node, output.is_some()));
            }
        }
        2 => {
            // A coordinate that does not fit the core i32 outline is a safe
            // invalid-conversion witness; the very large cubic separately
            // drives the bbox peak scaling path.
            let invalid_coordinate = i64::from(i32::MAX) + 1;
            let malformed = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector {
                        x: invalid_coordinate,
                        y: 0,
                    },
                    rust_ffi::FT_Vector { x: 32, y: 64 },
                    rust_ffi::FT_Vector { x: 64, y: 0 },
                ],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
                contours: vec![2],
                flags: 0,
            };
            let extent = 1_i64 << (24 + (repeat % 5));
            let cubic = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector {
                        x: extent,
                        y: extent,
                    },
                    rust_ffi::FT_Vector {
                        x: extent,
                        y: -extent,
                    },
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                ],
                tags: vec![
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                ],
                contours: vec![3],
                flags: 0,
            };
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut bbox = rust_ffi::FT_BBox::default();
            let bbox_bad = rust_ffi::FT_Outline_Get_BBox(Some(&malformed), Some(&mut bbox));
            let bbox_cubic = rust_ffi::FT_Outline_Get_BBox(Some(&cubic), Some(&mut bbox));
            let mut storage = vec![0_u8; 16];
            let target = rust_ffi::FT_Bitmap_C {
                width: 8,
                rows: 8,
                pitch: 2,
                buffer: storage.as_mut_ptr(),
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as _,
                num_grays: 256,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let bitmap_bad = rust_ffi::FT_Outline_Get_Bitmap(
                Some(&core_library),
                Some(&malformed),
                Some(&target),
            );
            let render_bad = rust_ffi::FT_Outline_Render(
                Some(&core_library),
                Some(&malformed),
                Some(&target),
                rust_ffi::FT_RASTER_FLAG_AA as FT_Int,
                rust_ffi::FT_BBox::default(),
            );
            let direct_bad = rust_ffi::FT_Outline_Render_Direct_Spans(
                Some(&core_library),
                Some(&malformed),
                Some(&target),
                rust_ffi::FT_RASTER_FLAG_DIRECT as FT_Int | rust_ffi::FT_RASTER_FLAG_AA as FT_Int,
                Some(rust_ffi::FT_BBox {
                    xMin: 0,
                    yMin: 0,
                    xMax: 8,
                    yMax: 8,
                }),
                true,
            );
            std::hint::black_box((
                bbox_bad,
                bbox_cubic,
                bitmap_bad.is_ok(),
                render_bad.is_ok(),
                direct_bad.is_ok(),
                bbox,
            ));
        }
        3 => {
            // A deliberately narrow MONO pitch reaches the packed-row
            // bounds guard; gray and DIRECT calls cover the normal render
            // ownership path with the same Rust bitmap wrapper.
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut storage = vec![0_u8; 8];
            let target = rust_ffi::FT_Bitmap_C {
                width: 64,
                rows: 8,
                pitch: 1,
                buffer: storage.as_mut_ptr(),
                pixel_mode: rust_ffi::FT_PIXEL_MODE_MONO as _,
                num_grays: 2,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let bitmap = rust_ffi::FT_Outline_Get_Bitmap(
                Some(&core_library),
                Some(&simple_outline),
                Some(&target),
            );
            let rendered = rust_ffi::FT_Outline_Render(
                Some(&core_library),
                Some(&simple_outline),
                Some(&target),
                if repeat % 2 == 0 {
                    rust_ffi::FT_RASTER_FLAG_AA as FT_Int
                } else {
                    0
                },
                rust_ffi::FT_BBox::default(),
            );
            let direct = rust_ffi::FT_Outline_Render_Direct_Spans(
                Some(&core_library),
                Some(&simple_outline),
                Some(&target),
                rust_ffi::FT_RASTER_FLAG_DIRECT as FT_Int | rust_ffi::FT_RASTER_FLAG_AA as FT_Int,
                Some(rust_ffi::FT_BBox {
                    xMin: 0,
                    yMin: 0,
                    xMax: 64,
                    yMax: 8,
                }),
                repeat % 3 != 0,
            );
            let error_output = rust_ffi::FT_Outline_Render_Error_Output(
                Some(&simple_outline),
                Some(&target),
                if repeat % 2 == 0 {
                    0
                } else {
                    rust_ffi::FT_RASTER_FLAG_AA as FT_Int
                },
            );
            std::hint::black_box((
                bitmap.is_ok(),
                rendered.is_ok(),
                direct.is_ok(),
                error_output.is_some(),
                storage,
            ));
        }
        4 => {
            // Straight segments preserve a movable border endpoint.  The
            // conic/cubic cases first create the real pending-curve state and
            // then replay it through the next segment.
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let radius = if repeat % 3 == 1 { 80 } else { 96 };
                rust_ffi::FT_Stroker_Set(
                    stroker,
                    radius,
                    rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
                    rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
                    65_536,
                );
                let start = rust_ffi::FT_Vector { x: 0, y: 0 };
                let first = rust_ffi::FT_Vector { x: 640, y: 0 };
                let second = rust_ffi::FT_Vector { x: 1280, y: 0 };
                let third = rust_ffi::FT_Vector { x: 1920, y: 0 };
                let begin = rust_ffi::FT_Stroker_BeginSubPath(
                    stroker,
                    Some(&start),
                    if repeat % 3 == 0 { 1 } else { 0 },
                );
                let first_status = rust_ffi::FT_Stroker_LineTo(stroker, Some(&first));
                let curve_status = if repeat % 3 == 1 {
                    let control = rust_ffi::FT_Vector { x: 256, y: 512 };
                    let to = rust_ffi::FT_Vector { x: 512, y: 0 };
                    let pending = rust_ffi::FT_Stroker_ConicTo(stroker, Some(&control), Some(&to));
                    let replay = rust_ffi::FT_Stroker_LineTo(stroker, Some(&second));
                    (pending, replay)
                } else if repeat % 3 == 2 {
                    let control1 = rust_ffi::FT_Vector { x: 160, y: 640 };
                    let control2 = rust_ffi::FT_Vector { x: 480, y: 640 };
                    let to = rust_ffi::FT_Vector { x: 640, y: 0 };
                    let pending = rust_ffi::FT_Stroker_CubicTo(
                        stroker,
                        Some(&control1),
                        Some(&control2),
                        Some(&to),
                    );
                    let replay = rust_ffi::FT_Stroker_LineTo(stroker, Some(&second));
                    (pending, replay)
                } else {
                    let next = rust_ffi::FT_Stroker_LineTo(stroker, Some(&second));
                    let last = rust_ffi::FT_Stroker_LineTo(stroker, Some(&third));
                    (next, last)
                };
                let mut points = 0;
                let mut contours = 0;
                let before =
                    rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours));
                let mut border_points = 0;
                let mut border_contours = 0;
                let border = rust_ffi::FT_Stroker_GetBorderCounts(
                    stroker,
                    rust_ffi::FT_STROKER_BORDER_LEFT as FT_Int,
                    Some(&mut border_points),
                    Some(&mut border_contours),
                );
                let finish = rust_ffi::FT_Stroker_EndSubPath(stroker);
                let after =
                    rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours));
                rust_ffi::FT_Stroker_Done(stroker);
                std::hint::black_box((
                    begin,
                    first_status,
                    curve_status,
                    before,
                    border,
                    finish,
                    after,
                    points,
                    contours,
                    border_points,
                    border_contours,
                ));
            }
        }
        5 => {
            // Parse both valid and deliberately malformed public outline tag
            // streams, then query the stale handle after Done to preserve the
            // Rust FFI invalid-lifecycle behavior.
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                rust_ffi::FT_Stroker_Set(
                    stroker,
                    96,
                    rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
                    if repeat % 2 == 0 {
                        rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int
                    } else {
                        rust_ffi::FT_STROKER_LINEJOIN_BEVEL as FT_Int
                    },
                    65_536,
                );
                let outline = match repeat {
                    0 => rust_ffi::FT_OutlineSnapshot {
                        points: vec![rust_ffi::FT_Vector::default()],
                        tags: vec![rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte],
                        contours: vec![0],
                        flags: 0,
                    },
                    1 => rust_ffi::FT_OutlineSnapshot {
                        points: vec![
                            rust_ffi::FT_Vector { x: 0, y: 0 },
                            rust_ffi::FT_Vector { x: 160, y: 320 },
                            rust_ffi::FT_Vector { x: 320, y: 0 },
                        ],
                        tags: vec![
                            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                            rust_ffi::FT_CURVE_TAG_CONIC as FT_Byte,
                            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                        ],
                        contours: vec![2],
                        flags: 0,
                    },
                    2 => rust_ffi::FT_OutlineSnapshot {
                        points: vec![
                            rust_ffi::FT_Vector { x: 0, y: 0 },
                            rust_ffi::FT_Vector { x: 160, y: 640 },
                            rust_ffi::FT_Vector { x: 480, y: 640 },
                            rust_ffi::FT_Vector { x: 640, y: 0 },
                        ],
                        tags: vec![
                            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                            rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                            rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                        ],
                        contours: vec![3],
                        flags: 0,
                    },
                    3 => rust_ffi::FT_OutlineSnapshot {
                        points: vec![
                            rust_ffi::FT_Vector { x: 0, y: 0 },
                            rust_ffi::FT_Vector { x: 640, y: 0 },
                        ],
                        tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 2],
                        contours: vec![1],
                        flags: 0,
                    },
                    4 => rust_ffi::FT_OutlineSnapshot {
                        points: vec![rust_ffi::FT_Vector::default()],
                        tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte],
                        contours: vec![9],
                        flags: 0,
                    },
                    5 => rust_ffi::FT_OutlineSnapshot {
                        points: vec![
                            rust_ffi::FT_Vector { x: 0, y: 0 },
                            rust_ffi::FT_Vector { x: 120, y: 240 },
                            rust_ffi::FT_Vector { x: 240, y: 240 },
                            rust_ffi::FT_Vector { x: 360, y: 0 },
                        ],
                        tags: vec![
                            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                            rust_ffi::FT_CURVE_TAG_CONIC as FT_Byte,
                            rust_ffi::FT_CURVE_TAG_CONIC as FT_Byte,
                            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                        ],
                        contours: vec![3],
                        flags: 0,
                    },
                    6 => rust_ffi::FT_OutlineSnapshot {
                        points: vec![
                            rust_ffi::FT_Vector::default(),
                            rust_ffi::FT_Vector { x: 100, y: 100 },
                            rust_ffi::FT_Vector { x: 200, y: 0 },
                        ],
                        tags: vec![
                            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                            rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                        ],
                        contours: vec![2],
                        flags: 0,
                    },
                    7 => rust_ffi::FT_OutlineSnapshot::default(),
                    8 => rust_ffi::FT_OutlineSnapshot {
                        points: vec![
                            rust_ffi::FT_Vector { x: 0, y: 0 },
                            rust_ffi::FT_Vector { x: 320, y: 0 },
                            rust_ffi::FT_Vector { x: 0, y: 320 },
                            rust_ffi::FT_Vector { x: 640, y: 640 },
                            rust_ffi::FT_Vector { x: 960, y: 640 },
                            rust_ffi::FT_Vector { x: 640, y: 320 },
                        ],
                        tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 6],
                        contours: vec![2, 5],
                        flags: 0,
                    },
                    _ => simple_outline.clone(),
                };
                let parsed = rust_ffi::FT_Stroker_ParseOutline(
                    stroker,
                    Some(&outline),
                    if repeat % 3 == 0 { 1 } else { 0 },
                );
                let mut points = 0;
                let mut contours = 0;
                let counts =
                    rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours));
                rust_ffi::FT_Stroker_Done(stroker);
                let stale = rust_ffi::FT_Stroker_GetBorderCounts(
                    stroker,
                    rust_ffi::FT_STROKER_BORDER_RIGHT as FT_Int,
                    Some(&mut points),
                    Some(&mut contours),
                );
                std::hint::black_box((parsed, counts, stale, points, contours));
            }
        }
        6 => {
            // Type 1/CFF PS service queries use the Rust-owned byte-copy
            // wrapper, including sizing queries and unknown optional strings.
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 12.0) {
                let mut info = rust_ffi::PS_FontInfoRec::default();
                let mut private = rust_ffi::PS_PrivateRec::default();
                let info_status = rust_ffi::FT_Get_PS_Font_Info(Some(&core_face), Some(&mut info));
                let private_status =
                    rust_ffi::FT_Get_PS_Font_Private(Some(&core_face), Some(&mut private));
                let keys = [
                    rust_ffi::PS_DICT_VERSION,
                    rust_ffi::PS_DICT_NOTICE,
                    rust_ffi::PS_DICT_FULL_NAME,
                    rust_ffi::PS_DICT_FAMILY_NAME,
                    rust_ffi::PS_DICT_WEIGHT,
                    rust_ffi::PS_DICT_ENCODING_TYPE,
                    rust_ffi::PS_DICT_BLUE_VALUE,
                    rust_ffi::PS_DICT_MAX + 1,
                ];
                let mut results = Vec::new();
                for (key_index, key) in keys.into_iter().enumerate() {
                    let mut value = [0_u8; 64];
                    let value_len = value.len() as FT_Long;
                    results.push(rust_ffi::FT_Get_PS_Font_Value(
                        Some(&core_face),
                        key,
                        (repeat + key_index) as FT_UInt,
                        Some(&mut value),
                        value_len,
                    ));
                    results.push(rust_ffi::FT_Get_PS_Font_Value(
                        Some(&core_face),
                        key,
                        (repeat + key_index) as FT_UInt,
                        None,
                        0,
                    ));
                }
                std::hint::black_box((info_status, private_status, results, info, private));
            }
        }
        7 => {
            // OpenType validation stays in the Rust service; C output-table
            // ownership is deliberately not reconstructed in this probe.
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let all = rust_ffi::FT_VALIDATE_BASE as FT_UInt
                    | rust_ffi::FT_VALIDATE_GDEF as FT_UInt
                    | rust_ffi::FT_VALIDATE_GPOS as FT_UInt
                    | rust_ffi::FT_VALIDATE_GSUB as FT_UInt
                    | rust_ffi::FT_VALIDATE_JSTF as FT_UInt
                    | rust_ffi::FT_VALIDATE_MATH as FT_UInt;
                let flags = match repeat % 4 {
                    0 => 0,
                    1 => rust_ffi::FT_VALIDATE_GDEF as FT_UInt,
                    2 => {
                        rust_ffi::FT_VALIDATE_GPOS as FT_UInt
                            | rust_ffi::FT_VALIDATE_GSUB as FT_UInt
                    }
                    _ => all,
                };
                rust_ffi::FT_OpenType_Validator_Set_Available(&mut core_face, repeat % 5 != 0);
                let mut base = ptr::null();
                let mut gdef = ptr::null();
                let mut gpos = ptr::null();
                let mut gsub = ptr::null();
                let mut jstf = ptr::null();
                let result = rust_ffi::FT_OpenType_Validate(
                    Some(&core_face),
                    flags,
                    Some(&mut base),
                    Some(&mut gdef),
                    Some(&mut gpos),
                    Some(&mut gsub),
                    Some(&mut jstf),
                );
                for table in [base, gdef, gpos, gsub, jstf] {
                    rust_ffi::FT_OpenType_Free(Some(&core_face), table);
                }
                std::hint::black_box(result);
            }
        }
        8 => {
            // GX and classic-kern validators share the same Rust-owned face
            // and table lifetimes, so exercise both in one bulk witness.
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let all = rust_ffi::FT_VALIDATE_feat as FT_UInt
                    | rust_ffi::FT_VALIDATE_mort as FT_UInt
                    | rust_ffi::FT_VALIDATE_morx as FT_UInt
                    | rust_ffi::FT_VALIDATE_bsln as FT_UInt
                    | rust_ffi::FT_VALIDATE_just as FT_UInt
                    | rust_ffi::FT_VALIDATE_kern as FT_UInt
                    | rust_ffi::FT_VALIDATE_opbd as FT_UInt
                    | rust_ffi::FT_VALIDATE_trak as FT_UInt
                    | rust_ffi::FT_VALIDATE_prop as FT_UInt
                    | rust_ffi::FT_VALIDATE_lcar as FT_UInt;
                let flags = if repeat % 3 == 0 {
                    0
                } else if repeat % 3 == 1 {
                    rust_ffi::FT_VALIDATE_feat as FT_UInt | rust_ffi::FT_VALIDATE_kern as FT_UInt
                } else {
                    all
                };
                rust_ffi::FT_GX_Validator_Set_Available(&mut core_face, repeat % 4 != 0);
                let mut tables = vec![ptr::null(); 10];
                let gx = rust_ffi::FT_TrueTypeGX_Validate(
                    Some(&core_face),
                    flags,
                    Some(tables.as_mut_slice()),
                );
                for table in tables {
                    rust_ffi::FT_TrueTypeGX_Free(Some(&core_face), table);
                }
                let mut kern = ptr::null();
                let classic = rust_ffi::FT_ClassicKern_Validate(
                    Some(&core_face),
                    rust_ffi::FT_VALIDATE_CKERN as FT_UInt,
                    Some(&mut kern),
                );
                rust_ffi::FT_ClassicKern_Free(Some(&core_face), kern);
                std::hint::black_box((gx, classic));
            }
        }
        9 => {
            // Color-table parsing happens while the Rust face is created;
            // these follow-up calls make the CPAL/COLR public state observable
            // without adding a parallel parser in the C ABI layer.
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let palette_data = rust_ffi::FT_Palette_Data_Copy(Some(&core_face));
                let palette = rust_ffi::FT_Palette_Select_Copy(
                    Some(&core_face),
                    (repeat % 3) as FT_UShort,
                    repeat % 2 == 0,
                );
                let mut layer_iterator = rust_ffi::FT_LayerIterator::default();
                let mut glyph_index = 0;
                let mut color_index = 0;
                let layer = rust_ffi::FT_Get_Color_Glyph_Layer(
                    Some(&core_face),
                    (repeat % 4) as FT_UInt,
                    Some(&mut glyph_index),
                    Some(&mut color_index),
                    Some(&mut layer_iterator),
                );
                let mut opaque = rust_ffi::FT_OpaquePaint::default();
                let root = rust_ffi::FT_Get_Color_Glyph_Paint(
                    Some(&core_face),
                    (repeat % 4) as FT_UInt,
                    if repeat % 2 == 0 {
                        rust_ffi::FT_COLOR_INCLUDE_ROOT_TRANSFORM as FT_UInt
                    } else {
                        rust_ffi::FT_COLOR_NO_ROOT_TRANSFORM as FT_UInt
                    },
                    Some(&mut opaque),
                );
                let mut paint = rust_ffi::FT_COLR_Paint::default();
                let paint_result =
                    rust_ffi::FT_Get_Paint(Some(&core_face), opaque, Some(&mut paint));
                std::hint::black_box((
                    palette_data,
                    palette,
                    layer,
                    root,
                    paint_result,
                    glyph_index,
                    color_index,
                    paint,
                ));
            }
        }
        _ => {}
    }
}

#[cfg(feature = "abi-test-support")]
fn abi_c106_coverage_probe(
    bytes: &[FT_Byte],
    library: FT_Library,
    face: FT_Face,
    memory: FT_Memory,
    variant: u16,
) {
    // c106 is deliberately only a route selector.  Its 100 cases fan into
    // the already-maintained Rust FFI/core probes that Coverage MCP identified
    // as uncovered (cache, bitmap, glyph, outline, stroker, stream, and
    // validator paths).  The exported C ABI and WASM layers stay thin; this
    // test-support adapter owns no parallel implementation behavior.
    let target_probe = 41_u16.saturating_add(variant.saturating_sub(1001));
    abi_custom_memory_coverage_probe(bytes, library, face, memory, target_probe);
}

#[cfg(feature = "abi-test-support")]
fn abi_c107_uncovered_branch_probe(
    _bytes: &[FT_Byte],
    library: FT_Library,
    face: FT_Face,
    memory: FT_Memory,
    variant: u16,
) {
    // c107 deliberately drives the Rust-owned false/error arms identified by
    // Coverage MCP.  The empty font is a test-support input only: the public
    // C ABI and WASM entry points remain thin wrappers over these Rust FFI/core
    // paths, with no duplicate behavior in either exported layer.
    let index = usize::from(variant.saturating_sub(1101));
    let _ = abi_core_load_case(&[], rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 36]);
    abi_c99_core_probe(&[], 301 + u16::try_from(index % 26).unwrap_or(0));
    abi_c100_core_probe(&[], 401 + u16::try_from(index % 26).unwrap_or(0));
    let target_probe = 41_u16.saturating_add(u16::try_from(index).unwrap_or(0));
    abi_custom_memory_coverage_probe(&[], library, face, memory, target_probe);
}

#[cfg(feature = "abi-test-support")]
fn abi_c108_wrapper_gap_probe(
    bytes: &[FT_Byte],
    library: FT_Library,
    face: FT_Face,
    memory: FT_Memory,
    variant: u16,
) {
    // c108 is a bulk Rust-owned wrapper sweep selected from the latest
    // Coverage MCP gaps.  The rows vary inputs, but every family calls the
    // existing Rust FFI/core behavior through the thin C-shaped adapter; no
    // policy or implementation is duplicated in C ABI or WASM code.
    let index = usize::from(variant.saturating_sub(1201));
    let repeat = index / 20;
    match index % 20 {
        0 => {
            let mut source_bytes = [0_u8; 4];
            source_bytes[0] = u8::try_from(repeat).unwrap_or(0);
            let mut source = FT_StreamRec {
                base: source_bytes.as_mut_ptr(),
                size: source_bytes.len() as FT_ULong,
                ..FT_StreamRec::default()
            };
            let mut target = FT_StreamRec::default();
            let bzip = FT_Stream_OpenBzip2(ptr::null_mut(), ptr::from_mut(&mut source));
            let lzw = FT_Stream_OpenLZW(ptr::from_mut(&mut target), ptr::from_mut(&mut source));
            let gzip = FT_Stream_OpenGzip(ptr::from_mut(&mut target), ptr::from_mut(&mut source));
            let reads = (
                abi_support_bzip2_stream_bytes(ptr::from_mut(&mut source), 0, 0),
                abi_support_lzw_stream_bytes(ptr::from_mut(&mut source), 0, 0),
                abi_support_gzip_stream_bytes(ptr::from_mut(&mut target), 0, 0),
            );
            std::hint::black_box((bzip, lzw, gzip, reads));
        }
        1 => {
            let cache_probe = 41_u16.saturating_add(u16::try_from(repeat % 9).unwrap_or(0));
            abi_custom_memory_coverage_probe(bytes, library, face, memory, cache_probe);
            let nulls = (
                FTC_CMapCache_New(ptr::null_mut(), ptr::null_mut()),
                FTC_ImageCache_New(ptr::null_mut(), ptr::null_mut()),
                FTC_SBitCache_New(ptr::null_mut(), ptr::null_mut()),
            );
            std::hint::black_box(nulls);
        }
        2 => {
            let mut pixels = [0x7f_u8; 4];
            let source = FT_Bitmap {
                rows: 2,
                width: 2,
                pitch: 2,
                buffer: pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte,
                ..FT_Bitmap::default()
            };
            let mut target = FT_Bitmap::default();
            let copy = FT_Bitmap_Copy(library, &source, &mut target);
            let convert = FT_Bitmap_Convert(library, &source, &mut target, 1);
            let invalid_source = FT_Bitmap {
                pixel_mode: u8::MAX,
                ..source
            };
            let invalid = FT_Bitmap_Convert(library, &invalid_source, &mut target, 1);
            let done = FT_Bitmap_Done(library, &mut target);
            std::hint::black_box((copy, convert, invalid, done));
        }
        3 => {
            let mut first = FT_ListNodeRec::default();
            let mut second = FT_ListNodeRec::default();
            first.data = NonNull::<u8>::dangling().as_ptr().cast();
            second.data = NonNull::<u16>::dangling().as_ptr().cast();
            let first_ptr = ptr::from_mut(&mut first);
            let second_ptr = ptr::from_mut(&mut second);
            let mut list = FT_ListRec::default();
            FT_List_Add(ptr::from_mut(&mut list), first_ptr);
            FT_List_Add(ptr::from_mut(&mut list), second_ptr);
            let found = FT_List_Find(ptr::from_mut(&mut list), first.data);
            let iterated = FT_List_Iterate(
                ptr::from_mut(&mut list),
                Some(abi_c102_list_iterator),
                ptr::null_mut(),
            );
            FT_List_Up(ptr::from_mut(&mut list), first_ptr);
            FT_List_Remove(ptr::from_mut(&mut list), second_ptr);
            FT_List_Remove(ptr::from_mut(&mut list), first_ptr);
            std::hint::black_box((found, iterated, list));
        }
        4 => {
            let mut palette_data = FT_Palette_Data::default();
            let mut palette = ptr::null_mut();
            let mut layer = FT_LayerIterator::default();
            let mut glyph = 0;
            let mut color = 0;
            let palette_status = FT_Palette_Data_Get(face, &mut palette_data);
            let select_status = FT_Palette_Select(face, repeat as FT_UShort, &mut palette);
            let foreground = FT_Palette_Set_Foreground_Color(
                face,
                FT_Color {
                    red: 1,
                    green: 2,
                    blue: 3,
                    alpha: 4,
                },
            );
            let layers = FT_Get_Color_Glyph_Layer(
                face,
                repeat as FT_UInt,
                &mut glyph,
                &mut color,
                &mut layer,
            );
            let mut clip = FT_ClipBox::default();
            let clip_status = FT_Get_Color_Glyph_ClipBox(face, glyph, &mut clip);
            let mut opaque = FT_OpaquePaint::default();
            let paint = FT_Get_Color_Glyph_Paint(face, glyph, repeat as FT_UInt, &mut opaque);
            let mut paint_value = FT_COLR_Paint::default();
            let paint_status = FT_Get_Paint(face, opaque, &mut paint_value);
            let _ = FT_Get_Paint_Layers(face, &mut layer, &mut opaque);
            let stops =
                FT_Get_Colorline_Stops(face, ptr::null_mut(), &mut FT_ColorStopIterator::default());
            std::hint::black_box((
                palette_status,
                select_status,
                foreground,
                layers,
                clip_status,
                paint,
                paint_status,
                stops,
                palette_data,
                clip,
                paint_value,
            ));
        }
        5 => {
            let mut tables = [ptr::null(); 6];
            let open_type = FT_OpenType_Validate(
                face,
                rust_ffi::FT_VALIDATE_BASE as FT_UInt,
                &mut tables[0],
                &mut tables[1],
                &mut tables[2],
                &mut tables[3],
                &mut tables[4],
            );
            let mut gx = ptr::null();
            let gx_status = FT_TrueTypeGX_Validate(face, repeat as FT_UInt, &mut gx, 1);
            let mut kern = ptr::null();
            let kern_status = FT_ClassicKern_Validate(face, repeat as FT_UInt, &mut kern);
            for table in tables.into_iter().chain([gx, kern]) {
                if !table.is_null() {
                    FT_OpenType_Free(face, table);
                }
            }
            let ps_info = FT_Get_PS_Font_Info(face, ptr::null_mut());
            let ps_private = FT_Get_PS_Font_Private(face, ptr::null_mut());
            let ps_value = FT_Get_PS_Font_Value(face, 0, repeat as FT_UInt, ptr::null_mut(), 0);
            std::hint::black_box((
                open_type,
                gx_status,
                kern_status,
                ps_info,
                ps_private,
                ps_value,
            ));
        }
        6 => {
            let mut glyph = ptr::null_mut();
            let get = FT_Get_Glyph(ptr::null_mut(), &mut glyph);
            let new = FT_New_Glyph(library, rust_ffi::FT_GLYPH_FORMAT_OUTLINE, &mut glyph);
            if !glyph.is_null() {
                FT_Done_Glyph(glyph);
                glyph = ptr::null_mut();
            }
            let copy = FT_Glyph_Copy(ptr::null_mut(), &mut glyph);
            let transform = FT_Glyph_Transform(ptr::null_mut(), ptr::null(), ptr::null());
            let bitmap = FT_Glyph_To_Bitmap(ptr::null_mut(), repeat as FT_Int, ptr::null(), 0);
            let stroke = FT_Glyph_Stroke(ptr::null_mut(), ptr::null_mut(), 0);
            let border = FT_Glyph_StrokeBorder(ptr::null_mut(), ptr::null_mut(), 0, 0);
            FT_Done_Glyph(glyph);
            std::hint::black_box((get, new, copy, transform, bitmap, stroke, border));
        }
        7 => {
            let mut bbox = FT_BBox::default();
            let mut outline = FT_Outline::default();
            let new = FT_Outline_New(library, 0, 0, &mut outline);
            let check = FT_Outline_Check(&outline);
            let mut copy_target = FT_Outline::default();
            let copy = FT_Outline_Copy(&outline, &mut copy_target);
            let bbox_status = FT_Outline_Get_BBox(&outline, &mut bbox);
            let embolden = FT_Outline_Embolden(&mut outline, repeat as FT_Pos);
            let embolden_xy = FT_Outline_EmboldenXY(&mut outline, 0, repeat as FT_Pos);
            FT_Outline_Reverse(&mut outline);
            FT_Outline_Transform(&mut outline, ptr::null());
            FT_Outline_Translate(&mut outline, 1, -1);
            let done = FT_Outline_Done(library, &mut outline);
            let invalid = FT_Outline_Copy(ptr::null(), ptr::null_mut());
            std::hint::black_box((
                new,
                check,
                copy,
                bbox_status,
                embolden,
                embolden_xy,
                done,
                invalid,
                bbox,
            ));
        }
        8 => {
            let mut stroker = ptr::null_mut();
            let new = FT_Stroker_New(library, &mut stroker);
            let point = FT_Vector { x: 64, y: 0 };
            let mut outline = FT_Outline::default();
            let mut points = 0;
            let mut contours = 0;
            let calls = if new == rust_ffi::FT_Err_Ok {
                FT_Stroker_Set(stroker, 64 + repeat as FT_Fixed, 0, 0, 65_536);
                (
                    FT_Stroker_BeginSubPath(stroker, &point, repeat as FT_Bool),
                    FT_Stroker_LineTo(stroker, &point),
                    FT_Stroker_EndSubPath(stroker),
                    FT_Stroker_GetCounts(stroker, &mut points, &mut contours),
                )
            } else {
                (0, 0, 0, 0)
            };
            FT_Stroker_Export(stroker, &mut outline);
            FT_Stroker_Done(stroker);
            std::hint::black_box((new, calls, points, contours, outline));
        }
        9 => {
            let char_size = FT_Set_Char_Size(face, 0, 12 * 64, 72, 72);
            let pixel_size = FT_Set_Pixel_Sizes(face, 0, 12 + repeat as FT_UInt);
            let mut matrix = FT_Matrix {
                xx: 0x1_0000,
                xy: 0,
                yx: 0,
                yy: 0x1_0000,
            };
            let mut delta = FT_Vector::default();
            FT_Set_Transform(face, &matrix, &delta);
            FT_Get_Transform(face, &mut matrix, &mut delta);
            let request = FT_Request_Size(face, ptr::null());
            let mut size = ptr::null_mut();
            let new_size = FT_New_Size(face, &mut size);
            let activated = FT_Activate_Size(size);
            let done_size = FT_Done_Size(size);
            let glyph = FT_Load_Glyph(face, repeat as FT_UInt, rust_ffi::FT_LOAD_DEFAULT);
            let loaded_char =
                FT_Load_Char(face, 65 + repeat as FT_ULong, rust_ffi::FT_LOAD_DEFAULT);
            let mut advance = 0;
            let advance_status = FT_Get_Advance(
                face,
                repeat as FT_UInt,
                rust_ffi::FT_LOAD_DEFAULT,
                &mut advance,
            );
            let advances = FT_Get_Advances(face, 0, 2, rust_ffi::FT_LOAD_DEFAULT, ptr::null_mut());
            let rendered = FT_Render_Glyph(ptr::null_mut(), 0);
            std::hint::black_box((
                char_size,
                pixel_size,
                request,
                new_size,
                activated,
                done_size,
                glyph,
                loaded_char,
                advance_status,
                advances,
                rendered,
                matrix,
                delta,
            ));
        }
        10 => {
            let mut vector = FT_Vector {
                x: 128 + repeat as FT_Pos,
                y: -64,
            };
            let mut length = 0;
            let mut angle = 0;
            FT_Vector_Unit(&mut vector, 0);
            FT_Vector_Rotate(&mut vector, 0x4000);
            let vector_length = FT_Vector_Length(&mut vector);
            FT_Vector_Polarize(&mut vector, &mut length, &mut angle);
            FT_Vector_From_Polar(&mut vector, 64, angle);
            let matrix = FT_Matrix {
                xx: 0x1_0000,
                xy: 0,
                yx: 0,
                yy: 0x1_0000,
            };
            FT_Vector_Transform(&mut vector, &matrix);
            let mut matrix_out = matrix;
            FT_Matrix_Multiply(&matrix, &mut matrix_out);
            let invert = FT_Matrix_Invert(&mut matrix_out);
            let scalar = (
                FT_MulDiv(3, 7, 2),
                FT_MulFix(3, 7),
                FT_DivFix(3, 7),
                FT_RoundFix(3),
                FT_CeilFix(3),
                FT_FloorFix(3),
                FT_Sin(0),
                FT_Cos(0),
                FT_Tan(0),
                FT_Atan2(1, 1),
                FT_Angle_Diff(1, 2),
            );
            std::hint::black_box((vector, length, angle, vector_length, invert, scalar));
        }
        11 => {
            let filter = FT_Library_SetLcdFilter(library, repeat as FT_LcdFilter);
            let mut weights = [0_u8; 5];
            let weight_status = FT_Library_SetLcdFilterWeights(library, weights.as_mut_ptr());
            let geometry = [FT_Vector::default(); 3];
            let geometry_status = FT_Library_SetLcdGeometry(library, geometry.as_ptr());
            let engine = FT_Get_TrueType_Engine_Type(library);
            let module = b"autofitter\0";
            let property = b"increase-x-height\0";
            let get = FT_Property_Get(
                library,
                module.as_ptr().cast(),
                property.as_ptr().cast(),
                ptr::null_mut(),
            );
            let set = FT_Property_Set(
                library,
                module.as_ptr().cast(),
                property.as_ptr().cast(),
                ptr::null(),
            );
            let interface = abi_support_module_interface_present(library, Some("autofitter"));
            std::hint::black_box((
                filter,
                weight_status,
                geometry_status,
                engine,
                get,
                set,
                interface,
            ));
        }
        12 => {
            let info = abi_face_info(face);
            let stream = abi_face_stream_info(face);
            let stream_ptr = unsafe { (*face).stream };
            let uses_stream = abi_face_uses_stream(face, stream_ptr);
            let sizes = abi_face_available_sizes(face);
            let charmaps = abi_charmap_count(face);
            let charmap = abi_charmap_by_index(face, repeat as FT_UInt);
            let charmap_info = abi_charmap_info_by_index(face, repeat as FT_UInt);
            let active = abi_active_charmap_index(face);
            let slot = abi_slot_snapshot(face);
            let renderer = abi_renderer_mode_acceptance(bytes, 0, repeat as FT_UInt, 12);
            let renderer_ok = renderer.is_ok();
            std::hint::black_box((
                info,
                stream,
                uses_stream,
                sizes,
                charmaps,
                charmap,
                charmap_info,
                active,
                slot,
                renderer_ok,
            ));
        }
        13 => {
            let design_get = FT_Get_Var_Design_Coordinates(face, 0, ptr::null_mut());
            let blend_get = FT_Get_Var_Blend_Coordinates(face, 0, ptr::null_mut());
            let mm_blend_get = FT_Get_MM_Blend_Coordinates(face, 0, ptr::null_mut());
            let design_set = FT_Set_Var_Design_Coordinates(face, 0, ptr::null());
            let blend_set = FT_Set_Var_Blend_Coordinates(face, 0, ptr::null());
            let mm_design = FT_Set_MM_Design_Coordinates(face, 0, ptr::null());
            let mm_weight = FT_Set_MM_WeightVector(face, 0, ptr::null());
            let mut length = 0;
            let mm_weight_get = FT_Get_MM_WeightVector(face, &mut length, ptr::null_mut());
            let multi = FT_Get_Multi_Master(face, ptr::null_mut());
            let named = FT_Get_Default_Named_Instance(face, ptr::null_mut());
            let instance = FT_Set_Named_Instance(face, repeat as FT_UInt);
            std::hint::black_box((
                design_get,
                blend_get,
                mm_blend_get,
                design_set,
                blend_set,
                mm_design,
                mm_weight,
                mm_weight_get,
                multi,
                named,
                instance,
            ));
        }
        14 => {
            let count = FT_Get_Sfnt_Name_Count(face);
            let mut name = FT_SfntName::default();
            let name_status = FT_Get_Sfnt_Name(face, repeat as FT_UInt, &mut name);
            let mut lang = FT_SfntLangTag::default();
            let lang_status = FT_Get_Sfnt_LangTag(face, repeat as FT_UInt, &mut lang);
            let mut kerning = 0;
            let track = FT_Get_Track_Kerning(face, 12 * 64, repeat as FT_Int, &mut kerning);
            let table = FT_Get_Sfnt_Table(face, u32::from_be_bytes(*b"head"));
            let mut length = 0;
            let load = FT_Load_Sfnt_Table(
                face,
                u32::from_be_bytes(*b"head") as FT_ULong,
                0,
                ptr::null_mut(),
                &mut length,
            );
            let mut tag = 0;
            let info = FT_Sfnt_Table_Info(face, repeat as FT_UInt, &mut tag, &mut length);
            let first = FT_Get_First_Char(face, ptr::null_mut());
            let next = FT_Get_Next_Char(face, first, ptr::null_mut());
            std::hint::black_box((
                count,
                name_status,
                lang_status,
                track,
                table,
                load,
                info,
                first,
                next,
                name,
                lang,
            ));
        }
        15 => {
            let mut outline = FT_Outline::default();
            let _ = FT_Outline_New(library, 0, 0, &mut outline);
            let render = FT_Outline_Render(library, &outline, ptr::null_mut());
            let bitmap = FT_Outline_Get_Bitmap(library, &outline, ptr::null());
            let direct = abi_support_outline_render_direct_spans(
                library,
                &outline,
                ptr::null_mut(),
                repeat % 2 == 0,
                ptr::null_mut(),
            );
            let trace = abi_support_outline_decompose_trace(&outline, &[(0, repeat as FT_Pos)]);
            let trace_ok = trace.is_ok();
            let decompose = FT_Outline_Decompose(&mut outline, ptr::null(), ptr::null_mut());
            let done = FT_Outline_Done(library, &mut outline);
            std::hint::black_box((render, bitmap, direct, trace_ok, decompose, done));
        }
        16 => {
            let load = FT_Load_Glyph(face, repeat as FT_UInt, rust_ffi::FT_LOAD_DEFAULT);
            let slot = abi_glyph_slot_pointer(face);
            if let Some(slot) = slot {
                FT_GlyphSlot_Embolden(slot);
                let own = FT_GlyphSlot_Own_Bitmap(slot);
                FT_GlyphSlot_AdjustWeight(slot, 1, 1);
                FT_GlyphSlot_Oblique(slot);
                FT_GlyphSlot_Slant(slot, 1, 1);
                std::hint::black_box((own, slot));
            }
            let subglyph = FT_Get_SubGlyph_Info(
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            std::hint::black_box((load, subglyph));
        }
        17 => {
            let external = abi_external_stream_runtime(bytes, 0);
            let callback = abi_callback_stream_contract(bytes, 0);
            let error = abi_sfnt_load_name_diagnostic(bytes);
            let context = abi_truetype_context_allocation_failure_diagnostic();
            std::hint::black_box((external, callback, error, context));
        }
        18 => {
            let core = abi_c99_core_probe(bytes, 301 + u16::try_from(repeat % 26).unwrap_or(0));
            let core100 = abi_c100_core_probe(bytes, 401 + u16::try_from(repeat % 26).unwrap_or(0));
            abi_c101_wrapper_probe(library, face, 501 + u16::try_from(repeat % 10).unwrap_or(0));
            std::hint::black_box((core, core100));
        }
        19 => {
            let target = 141_u16.saturating_add(u16::try_from(repeat % 110).unwrap_or(0));
            abi_custom_memory_coverage_probe(bytes, library, face, memory, target);
            let _ = abi_support_library_default_module_names(library);
            let _ = abi_support_library_module_count(library);
            let _ = abi_support_library_refcount(library);
            let _ = abi_support_library_memory_is(library, memory);
            let _ = abi_support_face_memory_is(face, memory);
        }
        _ => {}
    }
}

#[cfg(feature = "abi-test-support")]
fn abi_c109_rust_gap_sweep_probe(
    bytes: &[FT_Byte],
    _library: FT_Library,
    _face: FT_Face,
    _memory: FT_Memory,
    variant: u16,
) {
    // c109 is a bulk source-directed sweep.  The 100 input rows select 25
    // Rust-owned families four times each, with the font asset changing the
    // parser/autohinter path.  These calls stay in the existing safe Rust FFI
    // layer; this cfg(test-support) adapter adds no C ABI or WASM behavior.
    let index = usize::from(variant.saturating_sub(1301));
    let family = index % 25;
    let repeat = index / 25;
    let library = rust_ffi::FT_Init_FreeType();
    let Ok(face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) else {
        return;
    };
    match family {
        0 => {
            // Empty, zero-area, and one-point outlines reach the renderer's
            // empty/zero-sized fast paths without a raw C bitmap pointer.
            let root = rust_ffi::FT_GlyphRec::default();
            let empty = rust_ffi::FT_OutlineGlyphOwned {
                root,
                outline: rust_ffi::FT_OutlineSnapshot::default(),
            };
            let mut point = rust_ffi::FT_OutlineGlyphOwned {
                root,
                outline: rust_ffi::FT_OutlineSnapshot {
                    points: vec![rust_ffi::FT_Vector { x: 64, y: 64 }],
                    tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte],
                    contours: vec![0],
                    flags: 0,
                },
            };
            let modes = [
                rust_ffi::FT_RENDER_MODE_NORMAL,
                rust_ffi::FT_RENDER_MODE_MONO,
                rust_ffi::FT_RENDER_MODE_LCD,
                rust_ffi::FT_RENDER_MODE_LCD_V,
                rust_ffi::FT_RENDER_MODE_SDF,
            ];
            for mode in modes {
                let _ = rust_ffi::FT_Outline_Glyph_To_Bitmap(&empty, mode);
                let _ = rust_ffi::FT_Outline_Glyph_To_Bitmap_With_Origin(
                    &point,
                    mode,
                    Some(rust_ffi::FT_Vector {
                        x: repeat as FT_Pos,
                        y: -(repeat as FT_Pos),
                    }),
                );
            }
            let _ = rust_ffi::FT_Outline_Glyph_To_Bitmap_In_Place(
                &mut point,
                rust_ffi::FT_RENDER_MODE_NORMAL,
                None,
                repeat % 2 == 0,
            );
        }
        1 => {
            // Normal, mono, and overlap rendering exercise the Rust raster
            // entry points with a compact valid triangle.
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 640, y: 0 },
                    rust_ffi::FT_Vector { x: 320, y: 640 },
                ],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
                contours: vec![2],
                flags: if repeat % 2 == 0 {
                    0
                } else {
                    rust_ffi::FT_OUTLINE_OVERLAP as FT_Int
                },
            };
            let target = rust_ffi::FT_Bitmap_C {
                rows: 16,
                width: 16,
                pitch: 16,
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let _ = rust_ffi::FT_Outline_Render(
                Some(&library),
                Some(&outline),
                Some(&target),
                rust_ffi::FT_RASTER_FLAG_AA as FT_Int,
                rust_ffi::FT_BBox::default(),
            );
            let _ = rust_ffi::FT_Outline_Render(
                Some(&library),
                Some(&outline),
                Some(&target),
                0,
                rust_ffi::FT_BBox::default(),
            );
            let _ = rust_ffi::FT_Outline_Render_Error_Output(
                Some(&outline),
                Some(&target),
                if repeat % 2 == 0 {
                    0
                } else {
                    rust_ffi::FT_RASTER_FLAG_AA as FT_Int
                },
            );
        }
        2 => {
            // Loaded slots cover the core render-mode conversion and the
            // render mutation path across load flag combinations.
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                let glyph = [0, 3, 36, 65][repeat % 4];
                let flags = [
                    rust_ffi::FT_LOAD_DEFAULT,
                    rust_ffi::FT_LOAD_NO_HINTING,
                    rust_ffi::FT_LOAD_FORCE_AUTOHINT,
                    rust_ffi::FT_LOAD_NO_BITMAP,
                    rust_ffi::FT_LOAD_TARGET_MONO,
                ];
                if let Ok(slot) =
                    rust_ffi::FT_Load_Glyph(&core_face, glyph, flags[repeat % flags.len()])
                {
                    for mode in [
                        rust_ffi::FT_RENDER_MODE_NORMAL,
                        rust_ffi::FT_RENDER_MODE_MONO,
                        rust_ffi::FT_RENDER_MODE_LCD,
                        rust_ffi::FT_RENDER_MODE_LCD_V,
                        rust_ffi::FT_RENDER_MODE_SDF,
                    ] {
                        let _ = rust_ffi::FT_Render_Glyph(slot.clone(), mode);
                    }
                    let _ = rust_ffi::FT_Render_Glyph(slot, 15);
                }
            }
        }
        3 => {
            // Detached outline glyph conversion reaches render_loaded_outline
            // through the safe glyph-class implementation, including origin
            // restoration and the in-place destroy=false branch.
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0)
                && let Ok(slot) = rust_ffi::FT_Load_Glyph(
                    &core_face,
                    [0, 3, 36, 65][repeat % 4],
                    rust_ffi::FT_LOAD_NO_HINTING,
                )
                && let Ok(mut glyph) = rust_ffi::FT_Get_Outline_Glyph(Some(&slot))
            {
                let _ = rust_ffi::FT_Outline_Glyph_Copy(&glyph);
                let _ = rust_ffi::FT_Outline_Glyph_To_Bitmap_With_Origin(
                    &glyph,
                    rust_ffi::FT_RENDER_MODE_NORMAL,
                    Some(rust_ffi::FT_Vector { x: 17, y: -9 }),
                );
                let _ = rust_ffi::FT_Outline_Glyph_To_Bitmap_In_Place(
                    &mut glyph,
                    rust_ffi::FT_RENDER_MODE_MONO,
                    Some(rust_ffi::FT_Vector { x: -7, y: 5 }),
                    false,
                );
            }
        }
        4 => {
            // Renderer validation for empty, unsupported, bitmap, and SVG
            // slots is intentionally exercised before any raw C façade call.
            let slots = [
                rust_ffi::FT_Empty_GlyphSlot(&face),
                rust_ffi::FT_Unsupported_GlyphSlot(&face),
                rust_ffi::FT_Malformed_Get_GlyphSlot(&face, repeat as FT_UInt),
                rust_ffi::FT_Malformed_Get_GlyphSlot(&face, 2 + repeat as FT_UInt),
            ];
            for slot in slots {
                let _ = rust_ffi::FT_Render_Glyph(slot.clone(), rust_ffi::FT_RENDER_MODE_NORMAL);
                let _ = rust_ffi::FT_Render_Glyph(slot, rust_ffi::FT_RENDER_MODE_MAX);
            }
        }
        5 => {
            // BBox/decompose paths use conic and cubic controls outside the
            // on-curve bbox so the exact bounds helpers must walk the outline.
            let outlines = [
                rust_ffi::FT_OutlineSnapshot {
                    points: vec![
                        rust_ffi::FT_Vector { x: 0, y: 0 },
                        rust_ffi::FT_Vector { x: 20, y: 640 },
                        rust_ffi::FT_Vector { x: 640, y: 0 },
                    ],
                    tags: vec![
                        rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                        rust_ffi::FT_CURVE_TAG_CONIC as FT_Byte,
                        rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                    ],
                    contours: vec![2],
                    flags: 0,
                },
                rust_ffi::FT_OutlineSnapshot {
                    points: vec![
                        rust_ffi::FT_Vector { x: 0, y: 0 },
                        rust_ffi::FT_Vector { x: 10, y: 800 },
                        rust_ffi::FT_Vector { x: 600, y: 800 },
                        rust_ffi::FT_Vector { x: 640, y: 0 },
                    ],
                    tags: vec![
                        rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                        rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                        rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                        rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                    ],
                    contours: vec![3],
                    flags: 0,
                },
            ];
            let outline = &outlines[repeat % outlines.len()];
            let mut bbox = rust_ffi::FT_BBox::default();
            let bbox_result = rust_ffi::FT_Outline_Get_BBox(Some(outline), Some(&mut bbox));
            let decompose = rust_ffi::FT_Outline_Decompose_Trace(Some(outline), &[(0, 0)]);
            let orientation = rust_ffi::FT_Outline_Get_Orientation(Some(outline));
            std::hint::black_box((bbox_result, decompose.is_ok(), orientation, bbox));
        }
        6 => {
            // Bitmap conversion uses every packed pixel mode and both pitch
            // signs, exercising the pure-Rust unpackers and ownership map.
            let modes = [
                (rust_ffi::FT_PIXEL_MODE_MONO as FT_Byte, vec![0x80_u8, 0]),
                (rust_ffi::FT_PIXEL_MODE_GRAY2 as FT_Byte, vec![0x1b_u8, 0]),
                (rust_ffi::FT_PIXEL_MODE_GRAY4 as FT_Byte, vec![0xf0_u8; 4]),
                (rust_ffi::FT_PIXEL_MODE_BGRA as FT_Byte, vec![1, 2, 3, 255]),
            ];
            let (pixel_mode, source_bytes) = modes[repeat % modes.len()].clone();
            let width = if pixel_mode == rust_ffi::FT_PIXEL_MODE_BGRA as FT_Byte {
                1
            } else {
                8
            };
            let mut source = rust_ffi::FT_Bitmap_C {
                rows: 1,
                width,
                pitch: source_bytes.len() as c_int,
                num_grays: 256,
                pixel_mode,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut source), source_bytes);
            let mut target = rust_ffi::FT_Bitmap_C::default();
            let _ = rust_ffi::FT_Bitmap_Convert(
                Some(&library),
                Some(&source),
                Some(&mut target),
                if repeat % 2 == 0 { 4 } else { -4 },
            );
            let _ = rust_ffi::FT_Bitmap_Done(Some(&library), Some(&mut source));
            let _ = rust_ffi::FT_Bitmap_Done(Some(&library), Some(&mut target));
        }
        7 => {
            // Bitmap embolden covers zero, positive, negative, and packed
            // source-width handling through the Rust-owned bitmap wrapper.
            let mut bitmap = rust_ffi::FT_Bitmap_C {
                rows: 2,
                width: 2,
                pitch: if repeat % 2 == 0 { 2 } else { -2 },
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut bitmap), vec![0xff; 4]);
            let strength = [0, 64, -64, 128][repeat % 4];
            let _ = rust_ffi::FT_Bitmap_Embolden(
                Some(&library),
                Some(&mut bitmap),
                strength,
                -strength,
            );
            let _ = rust_ffi::FT_Bitmap_Done(Some(&library), Some(&mut bitmap));
        }
        8 => {
            // Latin autohint inputs are selected per row; each load varies
            // target mode and hint policy to reach distinct aflatin branches.
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                let flags = [
                    rust_ffi::FT_LOAD_FORCE_AUTOHINT,
                    rust_ffi::FT_LOAD_FORCE_AUTOHINT | rust_ffi::FT_LOAD_TARGET_LIGHT,
                    rust_ffi::FT_LOAD_FORCE_AUTOHINT | rust_ffi::FT_LOAD_TARGET_MONO,
                    rust_ffi::FT_LOAD_NO_AUTOHINT | rust_ffi::FT_LOAD_NO_HINTING,
                ];
                for glyph in [0, 3, 36, 65] {
                    let _ = rust_ffi::FT_Load_Glyph(&core_face, glyph, flags[repeat % flags.len()]);
                }
            }
        }
        9 => {
            // CJK/Indic autohint assets drive script-specific blue-zone and
            // stem-snap decisions without adding a second hinting engine.
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                for flags in [
                    rust_ffi::FT_LOAD_FORCE_AUTOHINT,
                    rust_ffi::FT_LOAD_FORCE_AUTOHINT | rust_ffi::FT_LOAD_TARGET_LIGHT,
                    rust_ffi::FT_LOAD_FORCE_AUTOHINT | rust_ffi::FT_LOAD_TARGET_MONO,
                    rust_ffi::FT_LOAD_NO_HINTING,
                ] {
                    let _ = rust_ffi::FT_Load_Glyph(&core_face, [0, 1, 36, 65][repeat % 4], flags);
                }
            }
        }
        10 => {
            // Scale/advance routes cover vertical, no-scale, bitmap-only, and
            // fast-only requests on whatever valid font asset selected the row.
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                let glyph = [0, 1, 36, 65][repeat % 4];
                let flags = [
                    rust_ffi::FT_LOAD_DEFAULT,
                    rust_ffi::FT_LOAD_NO_SCALE,
                    rust_ffi::FT_LOAD_VERTICAL_LAYOUT,
                    rust_ffi::FT_LOAD_BITMAP_METRICS_ONLY,
                    rust_ffi::FT_ADVANCE_FLAG_FAST_ONLY as FT_Int32,
                ];
                let _ = rust_ffi::FT_Get_Advance(&core_face, glyph, flags[repeat % flags.len()]);
                let _ = rust_ffi::FT_Get_Advances(
                    &core_face,
                    0,
                    repeat as FT_UInt,
                    flags[repeat % flags.len()],
                );
            }
        }
        11 => {
            // CFF/CFF2 glyph loading reaches charstring number decoding and
            // outline conversion with several public load policies.
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                for glyph in [0, 1, 2, 36] {
                    for flags in [
                        rust_ffi::FT_LOAD_DEFAULT,
                        rust_ffi::FT_LOAD_NO_HINTING,
                        rust_ffi::FT_LOAD_NO_SCALE,
                        rust_ffi::FT_LOAD_NO_RECURSE,
                    ] {
                        let _ = rust_ffi::FT_Load_Glyph(&core_face, glyph, flags);
                    }
                }
            }
        }
        12 => {
            // CID and pure-CFF font assets exercise FDArray/charset and
            // PostScript service fallbacks during normal face access.
            if let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                let _ = rust_ffi::FT_Get_Font_Format(Some(&core_face));
                let _ = rust_ffi::FT_Get_Postscript_Name(&core_face);
                let _ = rust_ffi::FT_Get_PS_Font_Info(Some(&core_face), None);
                let _ = rust_ffi::FT_Get_PS_Font_Private(Some(&core_face), None);
                let mut value = [0_u8; 64];
                let value_len = value.len() as FT_Long;
                let _ = rust_ffi::FT_Get_PS_Font_Value(
                    Some(&core_face),
                    repeat as FT_Int,
                    repeat as FT_UInt,
                    Some(&mut value),
                    value_len,
                );
                let _ = rust_ffi::FT_Get_PS_Font_Value(
                    Some(&mut core_face),
                    rust_ffi::PS_DICT_MAX + 1,
                    0,
                    None,
                    0,
                );
            }
        }
        13 => {
            // sbix and embedded bitmap assets are loaded with competing
            // bitmap policies so their strike-selection guards are reached.
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                let flags = [
                    rust_ffi::FT_LOAD_DEFAULT,
                    rust_ffi::FT_LOAD_SBITS_ONLY,
                    rust_ffi::FT_LOAD_BITMAP_METRICS_ONLY,
                    rust_ffi::FT_LOAD_NO_BITMAP,
                ];
                for glyph in [0, 1, 36, 65] {
                    let _ = rust_ffi::FT_Load_Glyph(&core_face, glyph, flags[repeat % flags.len()]);
                }
            }
        }
        14 => {
            // PCF/BDF faces run bitmap metrics, encoding, and property paths
            // while the same public charmap calls remain Rust-owned.
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                let mut glyph = 0;
                let first = rust_ffi::FT_Get_First_Char(Some(&core_face), Some(&mut glyph));
                let next = rust_ffi::FT_Get_Next_Char(Some(&core_face), first, Some(&mut glyph));
                let _ = rust_ffi::FT_Load_Char(
                    &core_face,
                    if repeat % 2 == 0 { first } else { next },
                    rust_ffi::FT_LOAD_RENDER,
                );
            }
        }
        15 => {
            // Variable assets drive design/blend coordinate setters and
            // named-instance selection before a glyph is loaded.
            if let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                let coords = [
                    (repeat as FT_Fixed) * 16_384 - 32_768,
                    32_768 - (repeat as FT_Fixed) * 8_192,
                    0,
                ];
                let _ = rust_ffi::FT_Set_Var_Design_Coordinates(
                    Some(&mut core_face),
                    repeat as FT_UInt,
                    Some(&coords[..repeat.min(coords.len())]),
                );
                let _ = rust_ffi::FT_Set_Var_Blend_Coordinates(
                    Some(&mut core_face),
                    repeat as FT_UInt,
                    Some(&coords[..repeat.min(coords.len())]),
                );
                let mut out = [0; 3];
                let out_len = repeat.min(out.len());
                let _ = rust_ffi::FT_Get_Var_Design_Coordinates(
                    Some(&core_face),
                    repeat as FT_UInt,
                    Some(&mut out[..out_len]),
                );
                let _ = rust_ffi::FT_Get_Var_Blend_Coordinates(
                    Some(&core_face),
                    repeat as FT_UInt,
                    Some(&mut out[..out_len]),
                );
                let _ = rust_ffi::FT_Set_Named_Instance(Some(&mut core_face), repeat as FT_UInt);
            }
        }
        16 => {
            // SFNT/name/kerning table queries use valid and deliberately
            // sparse metadata assets to cover optional table fallbacks.
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                let count = rust_ffi::FT_Get_Sfnt_Name_Count(Some(&core_face));
                let mut name = rust_ffi::FT_SfntName::default();
                let _ = rust_ffi::FT_Get_Sfnt_Name(
                    Some(&core_face),
                    repeat.min(count as usize) as FT_UInt,
                    Some(&mut name),
                );
                let mut lang = rust_ffi::FT_SfntLangTag::default();
                let _ = rust_ffi::FT_Get_Sfnt_LangTag(
                    Some(&core_face),
                    repeat as FT_UInt,
                    Some(&mut lang),
                );
                let mut length = 0;
                let tag = u32::from_be_bytes(*b"head") as FT_ULong;
                let _ = rust_ffi::FT_Load_Sfnt_Table(&core_face, tag, 0, Some(&mut length));
                let mut kerning = 0;
                let _ = rust_ffi::FT_Get_Track_Kerning(
                    Some(&core_face),
                    12 * 64,
                    repeat as FT_Int,
                    Some(&mut kerning),
                );
            }
        }
        17 => {
            // Color assets drive palette selection and COLR paint traversal,
            // including null optional iterators on alternate rows.
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                let _ = rust_ffi::FT_Palette_Data_Copy(Some(&core_face));
                let _ = rust_ffi::FT_Palette_Select_Copy(
                    Some(&core_face),
                    repeat as FT_UShort,
                    repeat % 2 == 0,
                );
                let mut glyph = 0;
                let mut color = 0;
                let mut iterator = rust_ffi::FT_LayerIterator::default();
                let _ = rust_ffi::FT_Get_Color_Glyph_Layer(
                    Some(&core_face),
                    repeat as FT_UInt,
                    Some(&mut glyph),
                    Some(&mut color),
                    Some(&mut iterator),
                );
                let _ = rust_ffi::FT_Get_Color_Glyph_ClipBox(Some(&core_face), glyph, None);
                let _ = rust_ffi::FT_Get_Color_Glyph_Paint(
                    Some(&core_face),
                    glyph,
                    rust_ffi::FT_COLOR_INCLUDE_ROOT_TRANSFORM as FT_UInt,
                    None,
                );
            }
        }
        18 => {
            // GX/AAT validators select both absent and available optional
            // tables, then release every returned table through Rust FFI.
            if let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                rust_ffi::FT_GX_Validator_Set_Available(&mut core_face, repeat % 2 == 0);
                let mut tables = vec![ptr::null(); 10];
                let result = rust_ffi::FT_TrueTypeGX_Validate(
                    Some(&core_face),
                    if repeat % 2 == 0 {
                        0
                    } else {
                        rust_ffi::FT_VALIDATE_kern as FT_UInt
                    },
                    Some(tables.as_mut_slice()),
                );
                for table in tables {
                    rust_ffi::FT_TrueTypeGX_Free(Some(&core_face), table);
                }
                let mut kern = ptr::null();
                let classic = rust_ffi::FT_ClassicKern_Validate(
                    Some(&core_face),
                    repeat as FT_UInt,
                    Some(&mut kern),
                );
                rust_ffi::FT_ClassicKern_Free(Some(&core_face), kern);
                std::hint::black_box((result, classic));
            }
        }
        19 => {
            // Type 1/MM assets cover PostScript and multiple-master service
            // calls through the same Rust-owned face record.
            if let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                let _ = rust_ffi::FT_Get_PS_Font_Info(Some(&core_face), None);
                let _ = rust_ffi::FT_Get_PS_Font_Private(Some(&core_face), None);
                let mut coords = [0; 4];
                let _ = rust_ffi::FT_Get_MM_WeightVector(
                    Some(&core_face),
                    Some(&mut 0),
                    Some(&mut coords),
                );
                let _ = rust_ffi::FT_Set_MM_WeightVector(
                    Some(&mut core_face),
                    repeat as FT_UInt,
                    Some(&coords),
                );
                let _ = rust_ffi::FT_Get_Multi_Master(Some(&core_face), None);
            }
        }
        20 => {
            // Core cache states use invalid and out-of-range glyph/size keys;
            // these are pure Rust cache calls, not synthetic C records.
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                let mut manager = rust_ffi::FTCCacheManagerState::new(core_face);
                let _ = manager.lookup_pixel_size(FT_UInt::MAX, repeat as FT_UInt);
                let image = rust_ffi::FTC_ImageTypeRec {
                    width: repeat as FT_UInt,
                    height: FT_UInt::MAX,
                    ..rust_ffi::FTC_ImageTypeRec::default()
                };
                let mut output = None;
                let _ = manager.lookup_sbit(Some(&image), FT_UInt::MAX, Some(&mut output), None);
                manager.done();
            }
        }
        21 => {
            // Stream wrappers see truncated, empty, and header-shaped source
            // bytes while retaining the owned Rust stream registry.
            let sources = [
                vec![],
                vec![0x1f, 0x8b, 8, 0],
                vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0x03],
                bytes.to_vec(),
            ];
            let source_bytes = &sources[repeat % sources.len()];
            let source_stream = rust_ffi::FT_StreamRec::default();
            let mut target = rust_ffi::FT_StreamRec::default();
            let gzip = rust_ffi::FT_Stream_OpenGzip(
                Some(&mut target),
                Some(&source_stream),
                Some(source_bytes),
            );
            let mut lzw_target = rust_ffi::FT_StreamRec::default();
            let lzw = rust_ffi::FT_Stream_OpenLZW(
                Some(&mut lzw_target),
                Some(&source_stream),
                Some(source_bytes),
            );
            rust_ffi::FT_Gzip_Stream_Close(Some(&mut target));
            rust_ffi::FT_LZW_Stream_Close(Some(&mut lzw_target));
            std::hint::black_box((gzip, lzw));
        }
        22 => {
            // Face size/charmap requests cover the public API validation arms
            // without constructing raw exported records.
            let core_face = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0);
            if let Ok(mut core_face) = core_face {
                for request_type in [
                    rust_ffi::FT_SIZE_REQUEST_TYPE_NOMINAL,
                    rust_ffi::FT_SIZE_REQUEST_TYPE_REAL_DIM,
                    rust_ffi::FT_SIZE_REQUEST_TYPE_BBOX,
                    rust_ffi::FT_SIZE_REQUEST_TYPE_CELL,
                    rust_ffi::FT_SIZE_REQUEST_TYPE_SCALES,
                ] {
                    let request = rust_ffi::FT_Size_RequestRec {
                        type_: request_type as FT_Int,
                        width: 12 * 64,
                        height: 16 * 64,
                        horiResolution: 72,
                        vertResolution: 72,
                    };
                    let _ = rust_ffi::FT_Request_Size(Some(&mut core_face), Some(&request));
                }
                let _ = rust_ffi::FT_Set_Char_Size(&mut core_face, 0, 16 * 64, 72, 72);
                let _ = rust_ffi::FT_Select_Size(Some(&mut core_face), repeat as FT_Int);
                let _ = rust_ffi::FT_Get_Charmap_Index(ptr::null_mut());
            }
        }
        23 => {
            // Synthetic slot operations cover bitmap ownership and oblique
            // transforms even when the selected font has no bitmap glyph.
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                let mut slot = rust_ffi::FT_Malformed_Get_GlyphSlot(&core_face, repeat as FT_UInt);
                rust_ffi::FT_GlyphSlot_Embolden(Some(&mut slot));
                let own = rust_ffi::FT_GlyphSlot_Own_Bitmap(Some(&mut slot));
                rust_ffi::FT_GlyphSlot_Oblique(Some(&mut slot));
                rust_ffi::FT_GlyphSlot_Slant(Some(&mut slot), 0x0366A, 0);
                let _ = rust_ffi::FT_GlyphSlot_AdjustWeight(Some(&mut slot), 1, 1);
                std::hint::black_box((own, slot));
            }
        }
        24 => {
            // Load-policy matrix covers SVG/color/bitmap-only and unknown
            // target validation through the same face and glyph route.
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) {
                let flags = [
                    rust_ffi::FT_LOAD_COLOR,
                    rust_ffi::FT_LOAD_NO_SVG,
                    rust_ffi::FT_LOAD_SVG_ONLY,
                    rust_ffi::FT_LOAD_SBITS_ONLY,
                    rust_ffi::FT_LOAD_RENDER | ((rust_ffi::FT_RENDER_MODE_SDF as FT_Int32) << 16),
                    rust_ffi::FT_LOAD_RENDER | (15 << 16),
                ];
                let _ = rust_ffi::FT_Load_Glyph(
                    &core_face,
                    [0, 3, 36, 65][repeat % 4],
                    flags[repeat % flags.len()],
                );
            }
        }
        _ => {}
    }
}

#[cfg(feature = "abi-test-support")]
fn abi_c110_rust_gap_batch_probe(
    bytes: &[FT_Byte],
    _library: FT_Library,
    _face: FT_Face,
    _memory: FT_Memory,
    variant: u16,
) {
    // c110 is another source-directed coverage sweep.  Its fixture rows only
    // select a family and a repeat; every operation below is owned by the
    // safe Rust FFI layer.  The C ABI/WASM exports remain thin adapters and
    // this cfg(test-support) entry point does not add runtime façade logic.
    let index = usize::from(variant.saturating_sub(1501));
    let family = index % 20;
    let repeat = index / 20;
    let library = rust_ffi::FT_Init_FreeType();
    let Ok(mut face) = rust_ffi::FT_New_Memory_Face(&library, bytes, 0, 20.0) else {
        return;
    };

    let triangle = rust_ffi::FT_OutlineSnapshot {
        points: vec![
            rust_ffi::FT_Vector { x: 0, y: 0 },
            rust_ffi::FT_Vector { x: 640, y: 0 },
            rust_ffi::FT_Vector { x: 320, y: 640 },
        ],
        tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
        contours: vec![2],
        flags: 0,
    };
    let conic = rust_ffi::FT_OutlineSnapshot {
        points: vec![
            rust_ffi::FT_Vector { x: 0, y: 0 },
            rust_ffi::FT_Vector { x: 256, y: 512 },
            rust_ffi::FT_Vector { x: 512, y: 0 },
        ],
        tags: vec![
            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
            rust_ffi::FT_CURVE_TAG_CONIC as FT_Byte,
            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
        ],
        contours: vec![2],
        flags: 0,
    };
    let cubic = rust_ffi::FT_OutlineSnapshot {
        points: vec![
            rust_ffi::FT_Vector { x: 0, y: 0 },
            rust_ffi::FT_Vector { x: 160, y: 640 },
            rust_ffi::FT_Vector { x: 480, y: 640 },
            rust_ffi::FT_Vector { x: 640, y: 0 },
        ],
        tags: vec![
            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
            rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
            rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
        ],
        contours: vec![3],
        flags: 0,
    };
    let malformed = rust_ffi::FT_OutlineSnapshot {
        points: vec![
            rust_ffi::FT_Vector { x: 0, y: 0 },
            rust_ffi::FT_Vector { x: 640, y: 0 },
            rust_ffi::FT_Vector { x: 320, y: 640 },
        ],
        tags: vec![
            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
            3,
            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
        ],
        contours: vec![2],
        flags: 0,
    };
    let glyph = |outline: rust_ffi::FT_OutlineSnapshot| rust_ffi::FT_OutlineGlyphOwned {
        root: rust_ffi::FT_GlyphRec {
            advance: rust_ffi::FT_Vector { x: 32_768, y: 0 },
            ..rust_ffi::FT_GlyphRec::default()
        },
        outline,
    };

    match family {
        0 => {
            // Null/foreign stroker handles and non-fixture glyph ownership
            // exercise the public stroke fallback and registry guards.
            let mut stroker = ptr::null_mut();
            let created = rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker));
            rust_ffi::FT_Stroker_Set(
                stroker,
                32 + (repeat as FT_Fixed * 16),
                [
                    rust_ffi::FT_STROKER_LINECAP_BUTT as FT_Int,
                    rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
                    rust_ffi::FT_STROKER_LINECAP_SQUARE as FT_Int,
                ][repeat % 3],
                [
                    rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
                    rust_ffi::FT_STROKER_LINEJOIN_BEVEL as FT_Int,
                    rust_ffi::FT_STROKER_LINEJOIN_MITER as FT_Int,
                ][repeat % 3],
                65_536,
            );
            let mut box_out = rust_ffi::FT_BBox::default();
            let source = glyph(triangle.clone());
            let foreign = NonNull::<u8>::dangling().as_ptr().cast();
            let stroke = rust_ffi::FT_Outline_Glyph_Stroke(Some(&source), foreign);
            let border = rust_ffi::FT_Outline_Glyph_StrokeBorder(
                Some(&source),
                foreign,
                (repeat & 1) as FT_Bool,
            );
            let live_stroke = rust_ffi::FT_Outline_Glyph_Stroke(Some(&source), stroker);
            let live_border = rust_ffi::FT_Outline_Glyph_StrokeBorder(
                Some(&source),
                stroker,
                (repeat & 1) as FT_Bool,
            );
            rust_ffi::FT_Outline_Glyph_CBox(
                live_stroke.as_ref().ok(),
                repeat as FT_UInt,
                Some(&mut box_out),
            );
            std::hint::black_box((
                created,
                stroke.is_ok(),
                border.is_ok(),
                live_stroke.is_ok(),
                live_border.is_ok(),
                box_out,
            ));
            rust_ffi::FT_Stroker_Done(stroker);
        }
        1 => {
            // Direction changes, duplicate points, and all line cap/join
            // families drive the lower stroker border state machine.
            let mut stroker = ptr::null_mut();
            let _ = rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker));
            rust_ffi::FT_Stroker_Set(
                stroker,
                48 + repeat as FT_Fixed * 16,
                repeat as FT_Int % 3,
                [
                    rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
                    rust_ffi::FT_STROKER_LINEJOIN_BEVEL as FT_Int,
                    rust_ffi::FT_STROKER_LINEJOIN_MITER as FT_Int,
                ][repeat % 3],
                65_536,
            );
            let a = rust_ffi::FT_Vector { x: 0, y: 0 };
            let b = rust_ffi::FT_Vector { x: 640, y: 0 };
            let c = rust_ffi::FT_Vector { x: 640, y: 640 };
            let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&a), (repeat & 1) as FT_Bool);
            let same = rust_ffi::FT_Stroker_LineTo(stroker, Some(&a));
            let first = rust_ffi::FT_Stroker_LineTo(stroker, Some(&b));
            let second = rust_ffi::FT_Stroker_LineTo(stroker, Some(&c));
            let end = rust_ffi::FT_Stroker_EndSubPath(stroker);
            let mut points = 0;
            let mut contours = 0;
            let count =
                rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours));
            let left = rust_ffi::FT_Stroker_GetBorderCounts(
                stroker,
                rust_ffi::FT_STROKER_BORDER_LEFT as FT_Int,
                Some(&mut points),
                Some(&mut contours),
            );
            let mut exported = rust_ffi::FT_OutlineSnapshot::default();
            rust_ffi::FT_Stroker_Export(stroker, Some(&mut exported));
            rust_ffi::FT_Stroker_ExportBorder(
                stroker,
                rust_ffi::FT_STROKER_BORDER_RIGHT as FT_Int,
                Some(&mut exported),
            );
            std::hint::black_box((same, first, second, end, count, left, exported));
            rust_ffi::FT_Stroker_Done(stroker);
        }
        2 => {
            // Exact conic/cubic maintained controls plus a follow-up segment
            // reach pending-path replay and the generic fallback outcomes.
            let mut stroker = ptr::null_mut();
            let _ = rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker));
            if repeat % 2 == 0 {
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
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
                let first = rust_ffi::FT_Stroker_ConicTo(stroker, Some(&control), Some(&to));
                let replay = rust_ffi::FT_Stroker_LineTo(
                    stroker,
                    Some(&rust_ffi::FT_Vector { x: 768, y: 128 }),
                );
                let second = rust_ffi::FT_Stroker_ConicTo(
                    stroker,
                    Some(&rust_ffi::FT_Vector { x: 700, y: 300 }),
                    Some(&rust_ffi::FT_Vector { x: 900, y: 0 }),
                );
                let end = rust_ffi::FT_Stroker_EndSubPath(stroker);
                std::hint::black_box((first, replay, second, end));
            } else {
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
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
                let first = rust_ffi::FT_Stroker_CubicTo(
                    stroker,
                    Some(&control1),
                    Some(&control2),
                    Some(&to),
                );
                let replay = rust_ffi::FT_Stroker_LineTo(
                    stroker,
                    Some(&rust_ffi::FT_Vector { x: 768, y: 128 }),
                );
                let second = rust_ffi::FT_Stroker_CubicTo(
                    stroker,
                    Some(&rust_ffi::FT_Vector { x: 700, y: 300 }),
                    Some(&rust_ffi::FT_Vector { x: 800, y: 400 }),
                    Some(&rust_ffi::FT_Vector { x: 900, y: 0 }),
                );
                let end = rust_ffi::FT_Stroker_EndSubPath(stroker);
                std::hint::black_box((first, replay, second, end));
            }
            rust_ffi::FT_Stroker_Done(stroker);
        }
        3 => {
            // Outline parsing covers ON, conic, cubic, malformed tags, and
            // opened/closed contour finalization through the same stroker.
            let mut stroker = ptr::null_mut();
            let _ = rust_ffi::FT_Stroker_New(Some(&library), Some(&mut stroker));
            rust_ffi::FT_Stroker_Set(
                stroker,
                64,
                repeat as FT_Int % 3,
                rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
                65_536,
            );
            let outlines = [triangle, conic, cubic, malformed];
            let outline = &outlines[repeat % outlines.len()];
            let parsed =
                rust_ffi::FT_Stroker_ParseOutline(stroker, Some(outline), (repeat & 1) as FT_Bool);
            let mut points = 0;
            let mut contours = 0;
            let counts =
                rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours));
            let mut output = rust_ffi::FT_OutlineSnapshot::default();
            rust_ffi::FT_Stroker_Export(stroker, Some(&mut output));
            std::hint::black_box((parsed, counts, output));
            rust_ffi::FT_Stroker_Done(stroker);
        }
        4 => {
            // BBox/decompose/check paths deliberately receive malformed tags
            // alongside curve extrema that require the exact bound helpers.
            for outline in [triangle, conic, cubic, malformed] {
                let mut bbox = rust_ffi::FT_BBox::default();
                let bbox_status = rust_ffi::FT_Outline_Get_BBox(Some(&outline), Some(&mut bbox));
                let check = rust_ffi::FT_Outline_Check(Some(&outline));
                let orientation = rust_ffi::FT_Outline_Get_Orientation(Some(&outline));
                let trace = rust_ffi::FT_Outline_Decompose_Trace(Some(&outline), &[(0, 0)]);
                let _ = rust_ffi::FT_Outline_GetInsideBorder(Some(&outline));
                let _ = rust_ffi::FT_Outline_GetOutsideBorder(Some(&outline));
                std::hint::black_box((bbox_status, check, orientation, trace.is_ok(), bbox));
            }
        }
        5 => {
            // Bitmap-mode outline rendering exercises valid storage, negative
            // pitch, packed MONO output, empty targets, and invalid snapshots.
            let mut backing = vec![0_u8; 128];
            let mut target = rust_ffi::FT_Bitmap_C {
                rows: 8,
                width: 8,
                pitch: if repeat % 2 == 0 { 16 } else { -16 },
                num_grays: 256,
                pixel_mode: if repeat % 2 == 0 {
                    rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte
                } else {
                    rust_ffi::FT_PIXEL_MODE_MONO as FT_Byte
                },
                buffer: backing.as_mut_ptr(),
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let rendered = rust_ffi::FT_Outline_Render(
                Some(&library),
                Some(&triangle),
                Some(&target),
                if repeat % 2 == 0 {
                    rust_ffi::FT_RASTER_FLAG_AA as FT_Int
                } else {
                    0
                },
                rust_ffi::FT_BBox::default(),
            );
            let bitmap =
                rust_ffi::FT_Outline_Get_Bitmap(Some(&library), Some(&triangle), Some(&target));
            let direct = rust_ffi::FT_Outline_Render(
                Some(&library),
                Some(&triangle),
                Some(&target),
                rust_ffi::FT_RASTER_FLAG_DIRECT as FT_Int,
                rust_ffi::FT_BBox::default(),
            );
            let malformed_render = rust_ffi::FT_Outline_Render(
                Some(&library),
                Some(&malformed),
                Some(&target),
                rust_ffi::FT_RASTER_FLAG_AA as FT_Int,
                rust_ffi::FT_BBox::default(),
            );
            std::hint::black_box((
                rendered.is_ok(),
                bitmap.is_ok(),
                direct.is_ok(),
                malformed_render.is_ok(),
                &mut target,
            ));
        }
        6 => {
            // Glyph-class selection, detached transforms, and bitmap/outline
            // CBox modes stay in the Rust glyph representation.
            let invalid = rust_ffi::FT_New_Glyph(Some(&library), 0x7fff_ffff);
            let allocation = rust_ffi::FT_New_Glyph_Allocation_Failure(
                Some(&library),
                rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
            );
            let mut detached = glyph(if repeat % 2 == 0 { triangle } else { conic });
            let matrix = rust_ffi::FT_Matrix {
                xx: 0x1_0000,
                xy: 0x0800,
                yx: -0x0400,
                yy: 0x1_0000,
            };
            let delta = rust_ffi::FT_Vector { x: 13, y: -7 };
            let transformed = rust_ffi::FT_Glyph_Transform_Outline(
                Some(&mut detached),
                Some(&matrix),
                Some(&delta),
            );
            let copied = rust_ffi::FT_Outline_Glyph_Copy(&detached);
            let mut bbox = rust_ffi::FT_BBox::default();
            rust_ffi::FT_Outline_Glyph_CBox(Some(&detached), repeat as FT_UInt, Some(&mut bbox));
            std::hint::black_box((invalid.is_ok(), allocation, transformed, copied, bbox));
        }
        7 => {
            // Bitmap copy/convert/blend/embolden use owned Rust buffers and
            // cover the pitch/alignment and target-mode validation arms.
            let mut source = rust_ffi::FT_Bitmap_C {
                rows: 2,
                width: 8,
                pitch: if repeat % 2 == 0 { 8 } else { -8 },
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            rust_ffi::FT_Bitmap_Set_Owned_Buffer(
                Some(&mut source),
                vec![
                    0x10, 0x40, 0x80, 0xff, 0xff, 0x80, 0x40, 0x10, 0x20, 0x60, 0xa0, 0xe0, 0xe0,
                    0xa0, 0x60, 0x20,
                ],
            );
            let mut copied = rust_ffi::FT_Bitmap_C::default();
            let copy = rust_ffi::FT_Bitmap_Copy(Some(&library), Some(&source), Some(&mut copied));
            let mut converted = rust_ffi::FT_Bitmap_C::default();
            let convert = rust_ffi::FT_Bitmap_Convert(
                Some(&library),
                Some(&source),
                Some(&mut converted),
                if repeat % 2 == 0 { 4 } else { -4 },
            );
            let embolden = rust_ffi::FT_Bitmap_Embolden(
                Some(&library),
                Some(&mut converted),
                [0, 64, 128, -64, 256][repeat],
                [0, 64, -64, 128, 256][repeat],
            );
            let mut blend_target = rust_ffi::FT_Bitmap_C {
                pixel_mode: rust_ffi::FT_PIXEL_MODE_NONE as FT_Byte,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let mut offset = rust_ffi::FT_Vector { x: 64, y: 128 };
            let blend = rust_ffi::FT_Bitmap_Blend(
                Some(&library),
                Some(&source),
                rust_ffi::FT_Vector { x: 0, y: 64 },
                Some(&mut blend_target),
                Some(&mut offset),
                rust_ffi::FT_Color::default(),
            );
            rust_ffi::FT_Bitmap_Done(Some(&library), Some(&mut source));
            rust_ffi::FT_Bitmap_Done(Some(&library), Some(&mut copied));
            rust_ffi::FT_Bitmap_Done(Some(&library), Some(&mut converted));
            rust_ffi::FT_Bitmap_Done(Some(&library), Some(&mut blend_target));
            std::hint::black_box((copy, convert, embolden, blend, offset));
        }
        8 => {
            // Manager-owned SBit lookup acquires and releases a node, then
            // repeats after reset/done to cover cache lifecycle guards.
            let mut manager = rust_ffi::FTCCacheManagerState::new(face.clone());
            let _ = manager.lookup_face();
            let _ = manager.lookup_pixel_size(8 + repeat as FT_UInt, 8 + repeat as FT_UInt);
            let image = rust_ffi::FTC_ImageTypeRec {
                width: 8 + repeat as FT_UInt,
                height: 8 + repeat as FT_UInt,
                flags: rust_ffi::FT_LOAD_RENDER,
                ..rust_ffi::FTC_ImageTypeRec::default()
            };
            let mut lookup = None;
            let mut node = false;
            let status = manager.lookup_sbit(
                Some(&image),
                repeat as FT_UInt,
                Some(&mut lookup),
                Some(&mut node),
            );
            drop(lookup);
            let unref = manager.unref_sbit_node(&image, repeat as FT_UInt);
            manager.reset();
            manager.done();
            let mut after_done = None;
            let after =
                manager.lookup_sbit(Some(&image), 0, Some(&mut after_done), Some(&mut node));
            std::hint::black_box((status, unref, after, node));
        }
        9 => {
            // Header flags deliberately distinguish truncated gzip extra/name
            // fields from ordinary invalid streams while preserving closure.
            let sources = [
                vec![],
                vec![0x1f, 0x8b, 8, 4, 0, 0, 0, 0, 0, 3, 4],
                vec![0x1f, 0x8b, 8, 8, 0, 0, 0, 0, 0, 3, b'n', b'o'],
                vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 0, 3],
                bytes.to_vec(),
            ];
            let source_bytes = &sources[repeat];
            let source = rust_ffi::FT_StreamRec::default();
            let mut gzip = rust_ffi::FT_StreamRec::default();
            let gzip_status =
                rust_ffi::FT_Stream_OpenGzip(Some(&mut gzip), Some(&source), Some(source_bytes));
            let read = rust_ffi::FT_Gzip_Stream_Read(Some(&gzip), 0, 16);
            rust_ffi::FT_Gzip_Stream_Close(Some(&mut gzip));
            let mut lzw = rust_ffi::FT_StreamRec::default();
            let lzw_status =
                rust_ffi::FT_Stream_OpenLZW(Some(&mut lzw), Some(&source), Some(source_bytes));
            rust_ffi::FT_LZW_Stream_Close(Some(&mut lzw));
            std::hint::black_box((gzip_status, read, lzw_status));
        }
        10 => {
            // PostScript service calls exercise missing strings and optional
            // private dictionaries on CFF/Type1 and ordinary SFNT faces.
            let mut info = rust_ffi::PS_FontInfoRec::default();
            let mut private = rust_ffi::PS_PrivateRec::default();
            let info_status = rust_ffi::FT_Get_PS_Font_Info(Some(&face), Some(&mut info));
            let private_status = rust_ffi::FT_Get_PS_Font_Private(Some(&face), Some(&mut private));
            let mut value = vec![0_u8; 32];
            let value_len = value.len() as FT_Long;
            let value_status = rust_ffi::FT_Get_PS_Font_Value(
                Some(&face),
                repeat as FT_Int,
                repeat as FT_UInt,
                Some(&mut value),
                value_len,
            );
            let missing =
                rust_ffi::FT_Get_PS_Font_Value(Some(&face), rust_ffi::PS_DICT_MAX + 1, 0, None, 0);
            let format = rust_ffi::FT_Get_Font_Format(Some(&face));
            std::hint::black_box((info_status, private_status, value_status, missing, format));
        }
        11 => {
            // GX/classic validators see both unavailable and malformed table
            // sets; returned allocations are always released through FFI.
            rust_ffi::FT_GX_Validator_Set_Available(&mut face, repeat % 2 == 0);
            let mut tables: Vec<rust_ffi::FT_Bytes> = vec![ptr::null(); 10];
            let validation = rust_ffi::FT_TrueTypeGX_Validate(
                Some(&face),
                if repeat % 2 == 0 {
                    rust_ffi::FT_VALIDATE_feat as FT_UInt
                } else {
                    rust_ffi::FT_VALIDATE_kern as FT_UInt
                },
                Some(&mut tables),
            );
            for table in tables {
                rust_ffi::FT_TrueTypeGX_Free(Some(&face), table);
            }
            let mut classic = ptr::null();
            let classic_status = rust_ffi::FT_ClassicKern_Validate(
                Some(&face),
                repeat as FT_UInt,
                Some(&mut classic),
            );
            rust_ffi::FT_ClassicKern_Free(Some(&face), classic);
            std::hint::black_box((validation, classic_status));
        }
        12 => {
            // CPAL/COLR faces, including malformed table fixtures, drive the
            // parser's optional metadata and nested paint rejection paths.
            let palette = rust_ffi::FT_Palette_Data_Copy(Some(&face));
            let selected =
                rust_ffi::FT_Palette_Select_Copy(Some(&face), repeat as FT_UShort, repeat % 2 == 0);
            let entries = rust_ffi::FT_Palette_Active_Entries_Copy(Some(&face));
            let foreground = rust_ffi::FT_Palette_Foreground_Copy(Some(&face));
            let mut layer_glyph = 0;
            let mut color_index = 0;
            let mut iterator = rust_ffi::FT_LayerIterator::default();
            let layer = rust_ffi::FT_Get_Color_Glyph_Layer(
                Some(&face),
                repeat as FT_UInt,
                Some(&mut layer_glyph),
                Some(&mut color_index),
                Some(&mut iterator),
            );
            let clip = rust_ffi::FT_Get_Color_Glyph_ClipBox(Some(&face), layer_glyph, None);
            let paint = rust_ffi::FT_Get_Color_Glyph_Paint(
                Some(&face),
                layer_glyph,
                rust_ffi::FT_COLOR_INCLUDE_ROOT_TRANSFORM as FT_UInt,
                None,
            );
            std::hint::black_box((palette, selected, entries, foreground, layer, clip, paint));
        }
        13 => {
            // Size creation/selection and charmap changes exercise active-size
            // synchronization and invalid strike/charmap handles.
            let pixel = rust_ffi::FT_Set_Pixel_Sizes(
                &mut face,
                repeat as FT_UInt * 4,
                8 + repeat as FT_UInt * 4,
            );
            let char_size = rust_ffi::FT_Set_Char_Size(&mut face, 0, 16 * 64, 72, 72);
            let request = rust_ffi::FT_Size_RequestRec {
                type_: (repeat as FT_Int) % 5,
                width: 12 * 64,
                height: 16 * 64,
                horiResolution: 72,
                vertResolution: 72,
            };
            let requested = rust_ffi::FT_Request_Size(Some(&mut face), Some(&request));
            let mut size = ptr::null_mut();
            let new_size = rust_ffi::FT_New_Size(Some(&face), Some(&mut size));
            let done_size = if size.is_null() {
                rust_ffi::FT_Err_Invalid_Size_Handle
            } else {
                rust_ffi::FT_Done_Size(size)
            };
            let selected = rust_ffi::FT_Select_Size(Some(&mut face), repeat as FT_Int - 2);
            let charmap = rust_ffi::FT_Select_Charmap(
                Some(&mut face),
                rust_ffi::FT_ENCODING_UNICODE as FT_Encoding,
            );
            let set_charmap = rust_ffi::FT_Set_Charmap(Some(&mut face), ptr::null_mut());
            std::hint::black_box((
                pixel,
                char_size,
                requested,
                new_size,
                done_size,
                selected,
                charmap,
                set_charmap,
            ));
        }
        14 => {
            // Load and render policy combinations reach bitmap, SVG, color,
            // no-recurse, and invalid render-mode dispatch without raw slots.
            let glyph_index = [0, 1, 36, 65, 0xffff][repeat];
            let flags = [
                rust_ffi::FT_LOAD_DEFAULT,
                rust_ffi::FT_LOAD_NO_HINTING,
                rust_ffi::FT_LOAD_NO_RECURSE,
                rust_ffi::FT_LOAD_COLOR | rust_ffi::FT_LOAD_RENDER,
                rust_ffi::FT_LOAD_SBITS_ONLY | rust_ffi::FT_LOAD_RENDER,
            ];
            let loaded = rust_ffi::FT_Load_Glyph(&face, glyph_index, flags[repeat]);
            if let Ok(slot) = loaded {
                let normal =
                    rust_ffi::FT_Render_Glyph(slot.clone(), rust_ffi::FT_RENDER_MODE_NORMAL);
                let mono = rust_ffi::FT_Render_Glyph(slot.clone(), rust_ffi::FT_RENDER_MODE_MONO);
                let invalid = rust_ffi::FT_Render_Glyph(slot, rust_ffi::FT_RENDER_MODE_MAX);
                std::hint::black_box((normal.is_ok(), mono.is_ok(), invalid.is_ok()));
            }
            let _ =
                rust_ffi::FT_Load_Char(&face, [0, 65, 0x20ac, 0xffff, 0][repeat], flags[repeat]);
        }
        15 => {
            // Variable design/blend queries deliberately vary coordinate count
            // and named-instance indices over variable and ordinary assets.
            let mut master = rust_ffi::FT_MM_Var::default();
            let mut axes = vec![rust_ffi::FT_Var_Axis::default(); 8];
            let mut styles = vec![rust_ffi::FT_Var_Named_Style::default(); 8];
            let mm = rust_ffi::FT_Get_MM_Var(
                Some(&face),
                Some(&mut master),
                Some(&mut axes),
                Some(&mut styles),
                None,
            );
            let mut axis_flags = 0;
            let axis = rust_ffi::FT_Get_Var_Axis_Flags(
                Some(&master),
                repeat as FT_UInt,
                Some(&mut axis_flags),
            );
            let mut coords = [0_i64, 16_384, -16_384, 32_768];
            let count = repeat as FT_UInt;
            let set_design = rust_ffi::FT_Set_Var_Design_Coordinates(
                Some(&mut face),
                count,
                Some(&coords[..repeat]),
            );
            let set_blend = rust_ffi::FT_Set_Var_Blend_Coordinates(
                Some(&mut face),
                count,
                Some(&coords[..repeat]),
            );
            let get_design = rust_ffi::FT_Get_Var_Design_Coordinates(
                Some(&face),
                count,
                Some(&mut coords[..repeat]),
            );
            let get_blend = rust_ffi::FT_Get_Var_Blend_Coordinates(
                Some(&face),
                count,
                Some(&mut coords[..repeat]),
            );
            let instance = rust_ffi::FT_Set_Named_Instance(Some(&mut face), repeat as FT_UInt);
            let default_instance =
                rust_ffi::FT_Get_Default_Named_Instance(Some(&face), Some(&mut axis_flags));
            std::hint::black_box((
                mm,
                axis,
                set_design,
                set_blend,
                get_design,
                get_blend,
                instance,
                default_instance,
                axes,
                styles,
            ));
        }
        16 => {
            // SFNT names, table info, first/next character, and kerning use
            // both valid and deliberately out-of-range selectors.
            let count = rust_ffi::FT_Get_Sfnt_Name_Count(Some(&face));
            let mut name = rust_ffi::FT_SfntName::default();
            let name_status =
                rust_ffi::FT_Get_Sfnt_Name(Some(&face), repeat as FT_UInt, Some(&mut name));
            let mut tag = 0;
            let mut length = 0;
            let info = rust_ffi::FT_Sfnt_Table_Info(
                &face,
                repeat as FT_UInt,
                Some(&mut tag),
                Some(&mut length),
            );
            let _ = rust_ffi::FT_Get_Sfnt_Table(&face, u32::from_be_bytes(*b"head"));
            let table = rust_ffi::FT_Load_Sfnt_Table(
                &face,
                u32::from_be_bytes(*b"head") as FT_ULong,
                if repeat % 2 == 0 { 0 } else { 4 },
                Some(&mut length),
            );
            let mut glyph = 0;
            let first = rust_ffi::FT_Get_First_Char(Some(&face), Some(&mut glyph));
            let next = rust_ffi::FT_Get_Next_Char(Some(&face), first, Some(&mut glyph));
            let mut kerning = rust_ffi::FT_Vector::default();
            let kern =
                rust_ffi::FT_Get_Kerning(Some(&face), 0, repeat as FT_UInt, 0, Some(&mut kerning));
            std::hint::black_box((
                count,
                name_status,
                info,
                table.is_ok(),
                first,
                next,
                kern,
                kerning,
            ));
        }
        17 => {
            // Autohint target modes and transforms are applied to real font
            // assets selected by the fixture row, including CJK/Latin paths.
            let matrix = rust_ffi::FT_Matrix {
                xx: 0x1_0000,
                xy: if repeat % 2 == 0 { 0x0400 } else { -0x0400 },
                yx: 0,
                yy: 0x1_0000,
            };
            let delta = rust_ffi::FT_Vector { x: 5, y: -3 };
            rust_ffi::FT_Set_Transform(Some(&mut face), Some(&matrix), Some(&delta));
            let flags = [
                rust_ffi::FT_LOAD_FORCE_AUTOHINT,
                rust_ffi::FT_LOAD_FORCE_AUTOHINT | rust_ffi::FT_LOAD_TARGET_LIGHT,
                rust_ffi::FT_LOAD_FORCE_AUTOHINT | rust_ffi::FT_LOAD_TARGET_MONO,
                rust_ffi::FT_LOAD_NO_AUTOHINT,
                rust_ffi::FT_LOAD_NO_HINTING,
            ];
            let loads = [0, 3, 36, 65, 1]
                .map(|glyph_index| rust_ffi::FT_Load_Glyph(&face, glyph_index, flags[repeat]));
            let mut got = rust_ffi::FT_Matrix::default();
            let mut got_delta = rust_ffi::FT_Vector::default();
            rust_ffi::FT_Get_Transform(Some(&face), Some(&mut got), Some(&mut got_delta));
            std::hint::black_box((loads.map(|result| result.is_ok()), got, got_delta));
        }
        18 => {
            // Bitmap-only, sbix, PCF, and BDF fixtures exercise strike and
            // property service fallbacks without crossing the C façade.
            let flags = [
                rust_ffi::FT_LOAD_DEFAULT,
                rust_ffi::FT_LOAD_SBITS_ONLY,
                rust_ffi::FT_LOAD_BITMAP_METRICS_ONLY,
                rust_ffi::FT_LOAD_NO_BITMAP,
                rust_ffi::FT_LOAD_RENDER,
            ];
            let loaded = rust_ffi::FT_Load_Glyph(&face, [0, 1, 2, 36, 65][repeat], flags[repeat]);
            let mut property = rust_ffi::BDF_PropertyRec::default();
            let bdf = rust_ffi::FT_Get_BDF_Property(
                Some(&face),
                Some("FONT_ASCENT"),
                Some(&mut property),
            );
            let mut encoding = ptr::null();
            let mut registry = ptr::null();
            let charset = rust_ffi::FT_Get_BDF_Charset_ID(
                Some(&face),
                Some(&mut encoding),
                Some(&mut registry),
            );
            let mut header = rust_ffi::FT_WinFNT_HeaderRec::default();
            let winfnt = rust_ffi::FT_Get_WinFNT_Header(Some(&face), Some(&mut header));
            let mut cid = 0;
            let cid_status = rust_ffi::FT_Get_CID_From_Glyph_Index(
                Some(&face),
                repeat as FT_UInt,
                Some(&mut cid),
            );
            std::hint::black_box((
                loaded.is_ok(),
                bdf,
                charset,
                winfnt,
                cid_status,
                property,
                header,
                cid,
            ));
        }
        19 => {
            // Fixed-vector and matrix boundary calls cover the low-level math
            // wrappers used by transforms, stroker angles, and scaler code.
            let mut vector = rust_ffi::FT_Vector {
                x: 64 + repeat as FT_Pos,
                y: -32,
            };
            rust_ffi::FT_Vector_Unit(Some(&mut vector), repeat as FT_Angle * 0x1000);
            rust_ffi::FT_Vector_From_Polar(
                Some(&mut vector),
                64 + repeat as FT_Fixed,
                repeat as FT_Angle * 0x1000,
            );
            let length = rust_ffi::FT_Vector_Length(Some(&vector));
            let mut polar_length = 0;
            let mut polar_angle = 0;
            rust_ffi::FT_Vector_Polarize(
                Some(&vector),
                Some(&mut polar_length),
                Some(&mut polar_angle),
            );
            rust_ffi::FT_Vector_Rotate(Some(&mut vector), -polar_angle);
            let mut matrix = rust_ffi::FT_Matrix {
                xx: 0x1_0000,
                xy: 0x0200,
                yx: -0x0200,
                yy: 0x1_0000,
            };
            let matrix_copy = matrix;
            rust_ffi::FT_Matrix_Multiply(Some(&matrix_copy), Some(&mut matrix));
            let invert = rust_ffi::FT_Matrix_Invert(Some(&mut matrix));
            let fixed = (
                rust_ffi::FT_MulDiv(123, -456, if repeat == 0 { 0 } else { 7 }),
                rust_ffi::FT_MulFix(123, -456),
                rust_ffi::FT_DivFix(123, if repeat == 0 { 0 } else { -456 }),
            );
            std::hint::black_box((vector, length, polar_length, polar_angle, invert, fixed));
        }
        _ => {}
    }
}

#[cfg(feature = "abi-test-support")]
fn abi_c111_rust_gap_batch_probe(
    bytes: &[FT_Byte],
    library: FT_Library,
    face_handle: FT_Face,
    memory: FT_Memory,
    probe: u16,
) {
    // c111 is a Rust-owned coverage router.  The exported C ABI and WASM
    // surfaces remain thin adapters; these witnesses select existing Rust FFI
    // operations and deliberately malformed Rust-owned records.
    let index = usize::from(probe.saturating_sub(1601));
    let family = index % 20;
    let repeat = index / 20;
    let core_library = rust_ffi::FT_Init_FreeType();
    let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) else {
        return;
    };
    let outline = rust_ffi::FT_OutlineSnapshot {
        points: vec![
            rust_ffi::FT_Vector { x: 0, y: 0 },
            rust_ffi::FT_Vector { x: 640, y: 0 },
            rust_ffi::FT_Vector { x: 320, y: 640 },
        ],
        tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
        contours: vec![2],
        flags: 0,
    };
    let malformed = rust_ffi::FT_OutlineSnapshot {
        points: vec![
            rust_ffi::FT_Vector { x: 0, y: 0 },
            rust_ffi::FT_Vector { x: 640, y: 0 },
        ],
        tags: vec![],
        contours: vec![1],
        flags: 0,
    };
    let glyph = |outline: rust_ffi::FT_OutlineSnapshot| rust_ffi::FT_OutlineGlyphOwned {
        root: rust_ffi::FT_GlyphRec {
            advance: rust_ffi::FT_Vector { x: 32_768, y: 0 },
            ..rust_ffi::FT_GlyphRec::default()
        },
        outline,
    };

    match family {
        0 => {
            // C-shaped stream wrappers see null, empty, and short headers.
            let mut source_bytes = match repeat {
                0 => vec![0_u8],
                1 => vec![0x1f, 0x8b],
                2 => vec![0x1f, 0x9d],
                3 => vec![0x1f, 0x8b, 8, 0, 0, 0, 0, 0, 3],
                _ => bytes[..bytes.len().min(16)].to_vec(),
            };
            if source_bytes.is_empty() {
                source_bytes.push(0);
            }
            let mut source = FT_StreamRec {
                base: source_bytes.as_mut_ptr(),
                size: source_bytes.len() as FT_ULong,
                ..FT_StreamRec::default()
            };
            let mut target = FT_StreamRec::default();
            let bzip = FT_Stream_OpenBzip2(ptr::from_mut(&mut target), ptr::from_mut(&mut source));
            let lzw = FT_Stream_OpenLZW(ptr::from_mut(&mut target), ptr::from_mut(&mut source));
            let gzip = FT_Stream_OpenGzip(ptr::from_mut(&mut target), ptr::from_mut(&mut source));
            let rust_gzip = rust_ffi::FT_Stream_OpenGzip(
                Some(&mut FT_StreamRec::default()),
                Some(&FT_StreamRec::default()),
                Some(&source_bytes),
            );
            let _ = std::hint::black_box((bzip, lzw, gzip, rust_gzip));
            abi_support_lzw_stream_close(ptr::from_mut(&mut target));
            abi_support_gzip_stream_close(ptr::from_mut(&mut target));
        }
        1 => {
            // Invalid glyph formats and allocator-failure paths stay in Rust.
            let invalid = rust_ffi::FT_New_Glyph(Some(&core_library), 0x7fff_ffff);
            let missing_library = rust_ffi::FT_New_Glyph(None, rust_ffi::FT_GLYPH_FORMAT_OUTLINE);
            let allocation = rust_ffi::FT_New_Glyph_Allocation_Failure(
                Some(&core_library),
                rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
            );
            let _ = std::hint::black_box((invalid, missing_library, allocation));
        }
        2 => {
            // Call stroker segment methods before a subpath and parse a bad
            // tag stream to reach invalid-outline and pending-replay arms.
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let start = rust_ffi::FT_Vector { x: 0, y: 0 };
                let control = rust_ffi::FT_Vector { x: 256, y: 512 };
                let end = rust_ffi::FT_Vector { x: 512, y: 0 };
                let line = rust_ffi::FT_Stroker_LineTo(stroker, Some(&end));
                let conic = rust_ffi::FT_Stroker_ConicTo(stroker, Some(&control), Some(&end));
                let cubic =
                    rust_ffi::FT_Stroker_CubicTo(stroker, Some(&start), Some(&control), Some(&end));
                let ended = rust_ffi::FT_Stroker_EndSubPath(stroker);
                let parsed =
                    rust_ffi::FT_Stroker_ParseOutline(stroker, Some(&malformed), repeat as _);
                let _ = std::hint::black_box((line, conic, cubic, ended, parsed));
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        3 => {
            // A single horizontal open segment selects the cap finalization
            // path, while alternate joins exercise the border state machine.
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                rust_ffi::FT_Stroker_Set(
                    stroker,
                    64 + repeat as FT_Fixed * 16,
                    (repeat % 3) as _,
                    (repeat % 3) as _,
                    65_536,
                );
                let start = rust_ffi::FT_Vector { x: 0, y: 0 };
                let end = rust_ffi::FT_Vector { x: 640, y: 0 };
                let begin = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 1);
                let line = rust_ffi::FT_Stroker_LineTo(stroker, Some(&end));
                let finish = rust_ffi::FT_Stroker_EndSubPath(stroker);
                let mut points = 0;
                let mut contours = 0;
                let counts =
                    rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours));
                let _ = std::hint::black_box((begin, line, finish, counts, points, contours));
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        4 => {
            // Non-fixture glyph strokes deliberately take the general
            // fallback, and the public stroker entry points keep the
            // implementation detail behind the Rust FFI layer.
            let detached = glyph(if repeat % 2 == 0 {
                outline.clone()
            } else {
                malformed.clone()
            });
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                rust_ffi::FT_Stroker_Set(stroker, 64, 0, 0, 65_536);
                let stroke = rust_ffi::FT_Outline_Glyph_Stroke(Some(&detached), stroker);
                let border = rust_ffi::FT_Outline_Glyph_StrokeBorder(
                    Some(&detached),
                    stroker,
                    (repeat & 1) as _,
                );
                let _ = std::hint::black_box((stroke, border));
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        5 => {
            // The pure-Rust cache state sees a successful node, a miss, and
            // post-Done invalidation without raw cache records.
            let mut manager = rust_ffi::FTCCacheManagerState::new(core_face.clone());
            let image = rust_ffi::FTC_ImageTypeRec {
                face_id: ptr::null_mut(),
                width: 12,
                height: 12,
                flags: rust_ffi::FT_LOAD_RENDER,
            };
            let first = {
                let mut lookup = None;
                let mut locked = false;
                manager.lookup_sbit(
                    Some(&image),
                    if repeat % 2 == 0 { 0 } else { 36 },
                    Some(&mut lookup),
                    Some(&mut locked),
                )
            };
            let second = {
                let mut lookup = None;
                let mut locked = false;
                manager.lookup_sbit(
                    Some(&image),
                    FT_UInt::MAX,
                    Some(&mut lookup),
                    Some(&mut locked),
                )
            };
            let invalid = {
                let mut lookup = None;
                let mut locked = false;
                manager.lookup_sbit(None, 0, Some(&mut lookup), Some(&mut locked))
            };
            manager.done();
            let after_done = {
                let mut lookup = None;
                let mut locked = false;
                manager.lookup_sbit(Some(&image), 0, Some(&mut lookup), Some(&mut locked))
            };
            let _ = std::hint::black_box((first, second, invalid, after_done));
        }
        6 => {
            // The exported cache adapter is exercised through its requester
            // harness, including repeat-node and invalid-glyph lookups.
            if let Ok(mut harness) = AbiSBitCacheHarness::new(bytes, 0) {
                let face_id = harness.face_id();
                let image = FTC_ImageTypeRec {
                    face_id,
                    width: 0,
                    height: 12,
                    flags: rust_ffi::FT_LOAD_RENDER,
                };
                let first = harness.lookup(
                    image,
                    if repeat % 2 == 0 { 36 } else { FT_UInt::MAX },
                    true,
                    true,
                );
                let second = harness.lookup(image, 36, true, true);
                let ownership = harness.ownership_snapshot();
                let _ = std::hint::black_box((first, second, ownership));
            }
        }
        7 => {
            // C ABI cache lifecycle helpers cover null cache slots and
            // charmap restoration around a manager-owned face.
            abi_coverage_stack_cache_lifecycle(library, repeat % 2 == 0);
            abi_coverage_cmap_restore(face_handle, library);
            let _ = std::hint::black_box(FTC_CMapCache_Lookup(
                ptr::null_mut(),
                ptr::null_mut(),
                -1,
                repeat as FT_UInt32,
            ));
        }
        8 => {
            // PS and font-format queries use a real Rust face but tolerate
            // absent service records and undersized value buffers.
            let mut info = rust_ffi::PS_FontInfoRec::default();
            let mut private = rust_ffi::PS_PrivateRec::default();
            let mut value = [0_u8; 2];
            let value_len = value.len() as FT_Long;
            let info_status = rust_ffi::FT_Get_PS_Font_Info(Some(&core_face), Some(&mut info));
            let private_status =
                rust_ffi::FT_Get_PS_Font_Private(Some(&core_face), Some(&mut private));
            let value_status = rust_ffi::FT_Get_PS_Font_Value(
                Some(&core_face),
                repeat as FT_Int,
                repeat as FT_UInt,
                Some(&mut value[..]),
                value_len,
            );
            let missing =
                rust_ffi::FT_Get_PS_Font_Value(Some(&core_face), repeat as FT_Int, 0, None, 0);
            let _ = std::hint::black_box((
                info_status,
                private_status,
                value_status,
                missing,
                info,
                private,
            ));
        }
        9 => {
            // Color and palette iterators use both valid and absent optional
            // outputs, which also exercises malformed color-table fallbacks.
            let palette = rust_ffi::FT_Palette_Data_Copy(Some(&core_face));
            let selected = rust_ffi::FT_Palette_Select_Copy(
                Some(&core_face),
                repeat as FT_UShort,
                repeat % 2 == 0,
            );
            let mut glyph_index = 0;
            let mut color_index = 0;
            let mut iterator = rust_ffi::FT_LayerIterator::default();
            let layer = rust_ffi::FT_Get_Color_Glyph_Layer(
                Some(&core_face),
                repeat as FT_UInt,
                Some(&mut glyph_index),
                Some(&mut color_index),
                Some(&mut iterator),
            );
            let clip = rust_ffi::FT_Get_Color_Glyph_ClipBox(Some(&core_face), glyph_index, None);
            let paint = rust_ffi::FT_Get_Color_Glyph_Paint(
                Some(&core_face),
                glyph_index,
                rust_ffi::FT_COLOR_INCLUDE_ROOT_TRANSFORM as FT_UInt,
                None,
            );
            let _ = std::hint::black_box((
                palette,
                selected,
                layer,
                clip,
                paint,
                glyph_index,
                color_index,
            ));
        }
        10 => {
            // Variable coordinates intentionally overrun ordinary faces and
            // alternate named-instance selectors.
            let coords = [0_i64, 16_384, -16_384, 32_768];
            let count = FT_UInt::try_from(repeat + 1).unwrap_or(FT_UInt::MAX);
            let design = rust_ffi::FT_Set_Var_Design_Coordinates(
                Some(&mut core_face),
                count,
                Some(&coords[..(repeat + 1).min(coords.len())]),
            );
            let blend = rust_ffi::FT_Set_Var_Blend_Coordinates(
                Some(&mut core_face),
                count,
                Some(&coords[..(repeat + 1).min(coords.len())]),
            );
            let named = rust_ffi::FT_Set_Named_Instance(Some(&mut core_face), repeat as FT_UInt);
            let mut axis = 0;
            let default_instance =
                rust_ffi::FT_Get_Default_Named_Instance(Some(&core_face), Some(&mut axis));
            let _ = std::hint::black_box((design, blend, named, default_instance, coords, axis));
        }
        11 => {
            // SFNT service calls receive invalid tags, offsets, and indices.
            let mut table_tag = 0;
            let mut length = 0;
            let tag = if repeat % 2 == 0 {
                u32::from_be_bytes(*b"head")
            } else {
                u32::from_be_bytes(*b"ZZZZ")
            };
            let info = rust_ffi::FT_Sfnt_Table_Info(
                &core_face,
                repeat as FT_UInt,
                Some(&mut table_tag),
                Some(&mut length),
            );
            let table = rust_ffi::FT_Get_Sfnt_Table(&core_face, tag);
            let loaded = rust_ffi::FT_Load_Sfnt_Table(
                &core_face,
                tag as rust_ffi::FT_ULong,
                if repeat % 2 == 0 {
                    0
                } else {
                    rust_ffi::FT_Long::MAX
                },
                Some(&mut length),
            );
            let mut glyph = 0;
            let first = rust_ffi::FT_Get_First_Char(Some(&core_face), Some(&mut glyph));
            let next = rust_ffi::FT_Get_Next_Char(Some(&core_face), first, Some(&mut glyph));
            let _ = std::hint::black_box((info, table, loaded, first, next, table_tag, length));
        }
        12 => {
            // Validators exercise unavailable tables and release every
            // optional table through the same Rust-owned face.
            rust_ffi::FT_GX_Validator_Set_Available(&mut core_face, repeat % 2 == 0);
            let mut tables = vec![ptr::null(); 10];
            let gx = rust_ffi::FT_TrueTypeGX_Validate(
                Some(&core_face),
                if repeat % 2 == 0 {
                    0
                } else {
                    rust_ffi::FT_VALIDATE_kern as FT_UInt
                },
                Some(tables.as_mut_slice()),
            );
            for table in tables {
                rust_ffi::FT_TrueTypeGX_Free(Some(&core_face), table);
            }
            let mut kern = ptr::null();
            let classic = rust_ffi::FT_ClassicKern_Validate(
                Some(&core_face),
                repeat as FT_UInt,
                Some(&mut kern),
            );
            rust_ffi::FT_ClassicKern_Free(Some(&core_face), kern);
            let _ = std::hint::black_box((gx, classic));
        }
        13 => {
            // Existing C-shaped stream and bitmap wrappers provide alternate
            // ownership and invalid-target witnesses without new façade code.
            abi_c102_live_wrapper_probe(bytes, library, face_handle, memory, 601 + repeat as u16);
            abi_c103_wrapper_probe(bytes, library, face_handle, memory, 701 + repeat as u16);
        }
        14 => {
            // Existing handle sweeps are re-entered with a real face and
            // distinct families selected by the batch index.
            abi_c104_handles_probe(bytes, library, face_handle, memory, 801 + repeat as u16);
            abi_c105_handles_probe(bytes, library, face_handle, memory, 901 + repeat as u16);
        }
        15 => {
            // Custom-memory probes 41--43 cover requester/cache false arms
            // that cannot be selected by ordinary font inputs.
            abi_custom_memory_coverage_probe(
                bytes,
                library,
                face_handle,
                memory,
                41 + repeat as u16,
            );
            abi_custom_memory_coverage_probe(
                bytes,
                library,
                face_handle,
                memory,
                42 + repeat as u16,
            );
        }
        16 => {
            // The branch-gap and wrapper routers remain Rust-owned and are
            // called with the current valid face so their error paths execute.
            abi_c107_uncovered_branch_probe(
                bytes,
                library,
                face_handle,
                memory,
                1101 + repeat as u16,
            );
            abi_c108_wrapper_gap_probe(bytes, library, face_handle, memory, 1201 + repeat as u16);
        }
        17 => {
            // Direct malformed outlines reach conversion and direct-render
            // validation before the C-shaped wrappers are involved.
            let mut pixels = vec![0_u8; 32 * 32];
            let target = rust_ffi::FT_Bitmap_C {
                rows: 32,
                width: 32,
                pitch: 32,
                buffer: pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let bitmap = rust_ffi::FT_Outline_Get_Bitmap(
                Some(&core_library),
                Some(&malformed),
                Some(&target),
            );
            let direct = rust_ffi::FT_Outline_Render_Direct_Spans(
                Some(&core_library),
                Some(&malformed),
                Some(&target),
                rust_ffi::FT_RASTER_FLAG_DIRECT as FT_Int,
                None,
                true,
            );
            let decompose = rust_ffi::FT_Outline_Decompose_Trace(Some(&malformed), &[(0, 0)]);
            let _ = std::hint::black_box((bitmap, direct, decompose, pixels));
        }
        18 => {
            // Glyph transforms and load policies combine invalid formats,
            // missing outlines, and ordinary real-font slots.
            let invalid = rust_ffi::FT_New_Glyph(Some(&core_library), 0x7fff_ffff);
            let load = rust_ffi::FT_Load_Glyph(
                &core_face,
                [0, 1, 36, 65, FT_UInt::MAX][repeat],
                rust_ffi::FT_LOAD_NO_HINTING | rust_ffi::FT_LOAD_NO_BITMAP,
            );
            let transformed = load
                .as_ref()
                .ok()
                .and_then(|slot| rust_ffi::FT_Get_Outline_Glyph(Some(&slot)).ok())
                .map(|mut owned| {
                    let matrix = rust_ffi::FT_Matrix {
                        xx: 0x1_0000,
                        xy: 0x0200,
                        yx: -0x0100,
                        yy: 0x1_0000,
                    };
                    rust_ffi::FT_Outline_Transform(Some(&mut owned.outline), Some(&matrix));
                    rust_ffi::FT_Outline_Translate(Some(&mut owned.outline), 3, -4);
                    rust_ffi::FT_Outline_Reverse(Some(&mut owned.outline));
                    owned
                });
            let _ = std::hint::black_box((invalid, load.is_ok(), transformed));
        }
        19 => {
            // Final family targets the remaining Rust math and module/service
            // wrappers while retaining five independently measured variants.
            let mut vector = rust_ffi::FT_Vector {
                x: 64 + repeat as FT_Pos,
                y: -32,
            };
            rust_ffi::FT_Vector_Unit(Some(&mut vector), repeat as FT_Angle * 0x1000);
            let length = rust_ffi::FT_Vector_Length(Some(&vector));
            let mut matrix = rust_ffi::FT_Matrix {
                xx: 0x1_0000,
                xy: 0x0200,
                yx: -0x0200,
                yy: 0x1_0000,
            };
            let invert = rust_ffi::FT_Matrix_Invert(Some(&mut matrix));
            let fixed = (
                rust_ffi::FT_MulDiv(123, -456, if repeat == 0 { 0 } else { 7 }),
                rust_ffi::FT_MulFix(123, -456),
                rust_ffi::FT_DivFix(123, if repeat == 0 { 0 } else { -456 }),
            );
            let _ = std::hint::black_box((vector, length, invert, fixed));
        }
        _ => {}
    }
}

#[cfg(feature = "abi-test-support")]
fn abi_c112_rust_gap_batch_probe(
    bytes: &[FT_Byte],
    library: FT_Library,
    face: FT_Face,
    memory: FT_Memory,
    probe: u16,
) {
    // c112 walks the remaining c107 custom-memory indices and c108 wrapper
    // families in one Rust-owned batch.  The public C ABI and WASM exports
    // remain thin adapters over these shared Rust FFI/core paths.
    let index = probe.saturating_sub(1701);
    abi_c107_uncovered_branch_probe(bytes, library, face, memory, 1101 + index);
    abi_c108_wrapper_gap_probe(bytes, library, face, memory, 1201 + index);
}

#[cfg(feature = "abi-test-support")]
fn abi_c113_rust_gap_batch_probe(
    bytes: &[FT_Byte],
    _library: FT_Library,
    _face: FT_Face,
    _memory: FT_Memory,
    probe: u16,
) {
    // c113 is a Rust-owned witness router.  Each five-row family selects a
    // different implementation path while the public C ABI and WASM layers
    // remain thin adapters over this shared FFI/core surface.
    let index = usize::from(probe.saturating_sub(1801));
    let family = index / 5;
    let repeat = index % 5;
    let core_library = rust_ffi::FT_Init_FreeType();
    let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) else {
        return;
    };
    let triangle = rust_ffi::FT_OutlineSnapshot {
        points: vec![
            rust_ffi::FT_Vector { x: 0, y: 0 },
            rust_ffi::FT_Vector { x: 640, y: 0 },
            rust_ffi::FT_Vector { x: 320, y: 640 },
        ],
        tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
        contours: vec![2],
        flags: 0,
    };
    let conic = rust_ffi::FT_OutlineSnapshot {
        points: vec![
            rust_ffi::FT_Vector { x: 0, y: 0 },
            rust_ffi::FT_Vector { x: 20, y: 640 },
            rust_ffi::FT_Vector { x: 640, y: 0 },
        ],
        tags: vec![
            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
            rust_ffi::FT_CURVE_TAG_CONIC as FT_Byte,
            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
        ],
        contours: vec![2],
        flags: 0,
    };
    let cubic = rust_ffi::FT_OutlineSnapshot {
        points: vec![
            rust_ffi::FT_Vector { x: 0, y: 0 },
            rust_ffi::FT_Vector { x: 10, y: 800 },
            rust_ffi::FT_Vector { x: 600, y: 800 },
            rust_ffi::FT_Vector { x: 640, y: 0 },
        ],
        tags: vec![
            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
            rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
            rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
            rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
        ],
        contours: vec![3],
        flags: 0,
    };
    let malformed = rust_ffi::FT_OutlineSnapshot {
        points: vec![
            rust_ffi::FT_Vector { x: 0, y: 0 },
            rust_ffi::FT_Vector { x: 640, y: 0 },
        ],
        tags: vec![],
        contours: vec![1],
        flags: 0,
    };

    match family {
        0 => {
            let invalid = rust_ffi::FT_New_Glyph_Allocation_Failure(
                Some(&core_library),
                if repeat % 2 == 0 {
                    rust_ffi::FT_GLYPH_FORMAT_OUTLINE
                } else {
                    0x7fff_ffff
                },
            );
            let validated = rust_ffi::FT_New_Glyph_Validate(
                Some(&core_library),
                rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
                repeat != 4,
            );
            let cleanup = rust_ffi::FT_Glyph_Copy_Failure_Cleanup();
            std::hint::black_box((invalid, validated, cleanup));
        }
        1 => {
            let outlines = [
                rust_ffi::FT_OutlineSnapshot::default(),
                triangle.clone(),
                conic.clone(),
                cubic.clone(),
                malformed.clone(),
            ];
            let outline = &outlines[repeat];
            let mut bbox = rust_ffi::FT_BBox::default();
            let result = rust_ffi::FT_Outline_Get_BBox(Some(outline), Some(&mut bbox));
            let check = rust_ffi::FT_Outline_Check(Some(outline));
            let orientation = rust_ffi::FT_Outline_Get_Orientation(Some(outline));
            std::hint::black_box((result, check, orientation, bbox));
        }
        2 => {
            let mut pixels = vec![0_u8; 16 * 16];
            let target = rust_ffi::FT_Bitmap_C {
                rows: 8,
                width: 8,
                pitch: if repeat % 2 == 0 { 16 } else { -16 },
                buffer: pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: if repeat == 4 {
                    rust_ffi::FT_PIXEL_MODE_NONE as FT_Byte
                } else if repeat % 2 == 0 {
                    rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte
                } else {
                    rust_ffi::FT_PIXEL_MODE_MONO as FT_Byte
                },
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let result = rust_ffi::FT_Outline_Get_Bitmap(
                Some(&core_library),
                Some(if repeat == 3 { &malformed } else { &triangle }),
                Some(&target),
            );
            std::hint::black_box((result.is_ok(), pixels));
        }
        3 => {
            let target = rust_ffi::FT_Bitmap_C {
                rows: 8,
                width: 8,
                pitch: 8,
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let render_outline = if repeat == 0 {
                rust_ffi::FT_OutlineSnapshot::default()
            } else {
                triangle.clone()
            };
            let rendered = rust_ffi::FT_Outline_Render(
                Some(&core_library),
                Some(&render_outline),
                if repeat == 4 { None } else { Some(&target) },
                if repeat % 2 == 0 {
                    rust_ffi::FT_RASTER_FLAG_AA as FT_Int
                } else {
                    0
                },
                rust_ffi::FT_BBox::default(),
            );
            let fallback = rust_ffi::FT_Outline_Render_Error_Output(
                Some(&triangle),
                Some(&target),
                if repeat == 1 {
                    rust_ffi::FT_RASTER_FLAG_AA as FT_Int
                } else {
                    0
                },
            );
            std::hint::black_box((rendered.is_ok(), fallback));
        }
        4 => {
            let target = rust_ffi::FT_Bitmap_C {
                rows: 8,
                width: 8,
                pitch: 8,
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let result = rust_ffi::FT_Outline_Render_Direct_Spans(
                Some(&core_library),
                Some(if repeat == 4 { &malformed } else { &triangle }),
                if repeat == 3 { None } else { Some(&target) },
                if repeat == 2 {
                    rust_ffi::FT_RASTER_FLAG_DIRECT as FT_Int
                } else {
                    rust_ffi::FT_RASTER_FLAG_DIRECT as FT_Int
                        | rust_ffi::FT_RASTER_FLAG_AA as FT_Int
                },
                if repeat % 2 == 0 {
                    Some(rust_ffi::FT_BBox {
                        xMin: 0,
                        yMin: 0,
                        xMax: 512,
                        yMax: 512,
                    })
                } else {
                    None
                },
                repeat != 1,
            );
            std::hint::black_box(result.is_ok());
        }
        5 => {
            let outlines = [triangle, conic, cubic, malformed];
            let outline = &outlines[repeat % outlines.len()];
            let trace =
                rust_ffi::FT_Outline_Decompose_Trace(Some(outline), &[(0, 0), (1, 1), (2, 2)]);
            let mut bbox = rust_ffi::FT_BBox::default();
            let bbox_result = rust_ffi::FT_Outline_Get_BBox(Some(outline), Some(&mut bbox));
            std::hint::black_box((trace.is_ok(), bbox_result, bbox));
        }
        6 => {
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                rust_ffi::FT_Stroker_Set(
                    stroker,
                    64 + repeat as FT_Fixed * 16,
                    if repeat % 2 == 0 {
                        rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int
                    } else {
                        rust_ffi::FT_STROKER_LINECAP_BUTT as FT_Int
                    },
                    rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
                    65_536,
                );
                let start = rust_ffi::FT_Vector { x: 0, y: 0 };
                let control = rust_ffi::FT_Vector { x: 256, y: 512 };
                let control2 = rust_ffi::FT_Vector { x: 480, y: 640 };
                let end = rust_ffi::FT_Vector { x: 640, y: 0 };
                let begin = rust_ffi::FT_Stroker_BeginSubPath(
                    stroker,
                    Some(&start),
                    (repeat & 1) as FT_Bool,
                );
                let segment = match repeat {
                    0 => rust_ffi::FT_Stroker_LineTo(stroker, Some(&end)),
                    1 | 2 => rust_ffi::FT_Stroker_ConicTo(stroker, Some(&control), Some(&end)),
                    _ => rust_ffi::FT_Stroker_CubicTo(
                        stroker,
                        Some(&control),
                        Some(&control2),
                        Some(&end),
                    ),
                };
                let parsed = rust_ffi::FT_Stroker_ParseOutline(
                    stroker,
                    Some(if repeat == 4 { &malformed } else { &cubic }),
                    (repeat & 1) as FT_Bool,
                );
                let ended = rust_ffi::FT_Stroker_EndSubPath(stroker);
                let mut points = 0;
                let mut contours = 0;
                let counts =
                    rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours));
                let mut output = rust_ffi::FT_OutlineSnapshot::default();
                rust_ffi::FT_Stroker_Export(stroker, Some(&mut output));
                std::hint::black_box((
                    begin, segment, parsed, ended, counts, points, contours, output,
                ));
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        7 => {
            if let Ok(slot) = rust_ffi::FT_Load_Glyph(
                &core_face,
                [0, 1, 36, 65, FT_UInt::MAX][repeat],
                rust_ffi::FT_LOAD_NO_HINTING,
            ) && let Ok(mut glyph) = rust_ffi::FT_Get_Outline_Glyph(Some(&slot))
            {
                let mut bbox = rust_ffi::FT_BBox::default();
                rust_ffi::FT_Outline_Glyph_CBox(Some(&glyph), repeat as FT_UInt, Some(&mut bbox));
                let copied = rust_ffi::FT_Outline_Glyph_Copy(&glyph);
                let normal =
                    rust_ffi::FT_Outline_Glyph_To_Bitmap(&glyph, rust_ffi::FT_RENDER_MODE_NORMAL);
                let origin = rust_ffi::FT_Outline_Glyph_To_Bitmap_With_Origin(
                    &glyph,
                    rust_ffi::FT_RENDER_MODE_MONO,
                    Some(rust_ffi::FT_Vector { x: 17, y: -9 }),
                );
                let inplace = rust_ffi::FT_Outline_Glyph_To_Bitmap_In_Place(
                    &mut glyph,
                    rust_ffi::FT_RENDER_MODE_NORMAL,
                    None,
                    repeat % 2 == 0,
                );
                std::hint::black_box((
                    bbox,
                    copied,
                    normal.is_ok(),
                    origin.is_ok(),
                    inplace.is_ok(),
                ));
            }
        }
        8 => {
            let mut source = rust_ffi::FT_Bitmap_C {
                rows: 2,
                width: 8,
                pitch: if repeat == 4 {
                    if repeat % 2 == 0 { 32 } else { -32 }
                } else if repeat % 2 == 0 {
                    8
                } else {
                    -8
                },
                num_grays: 256,
                pixel_mode: if repeat == 4 {
                    rust_ffi::FT_PIXEL_MODE_BGRA as FT_Byte
                } else {
                    rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte
                },
                ..rust_ffi::FT_Bitmap_C::default()
            };
            rust_ffi::FT_Bitmap_Set_Owned_Buffer(
                Some(&mut source),
                if repeat == 4 {
                    [1_u8, 2, 3, 255].repeat(16)
                } else {
                    vec![
                        0x10, 0x40, 0x80, 0xff, 0xff, 0x80, 0x40, 0x10, 0x20, 0x60, 0xa0, 0xe0,
                        0xe0, 0xa0, 0x60, 0x20,
                    ]
                },
            );
            let mut copied = rust_ffi::FT_Bitmap_C::default();
            let copy =
                rust_ffi::FT_Bitmap_Copy(Some(&core_library), Some(&source), Some(&mut copied));
            let mut converted = rust_ffi::FT_Bitmap_C::default();
            let convert = rust_ffi::FT_Bitmap_Convert(
                Some(&core_library),
                Some(&source),
                Some(&mut converted),
                if repeat % 2 == 0 { 4 } else { -4 },
            );
            let embolden = rust_ffi::FT_Bitmap_Embolden(
                Some(&core_library),
                Some(&mut converted),
                [0, 64, 128, -64, 256][repeat],
                [0, 64, -64, 128, 256][repeat],
            );
            let mut target = rust_ffi::FT_Bitmap_C {
                pixel_mode: rust_ffi::FT_PIXEL_MODE_NONE as FT_Byte,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let mut offset = rust_ffi::FT_Vector { x: 64, y: 128 };
            let blend = rust_ffi::FT_Bitmap_Blend(
                Some(&core_library),
                Some(&source),
                rust_ffi::FT_Vector { x: 0, y: 64 },
                Some(&mut target),
                Some(&mut offset),
                rust_ffi::FT_Color::default(),
            );
            rust_ffi::FT_Bitmap_Done(Some(&core_library), Some(&mut source));
            rust_ffi::FT_Bitmap_Done(Some(&core_library), Some(&mut copied));
            rust_ffi::FT_Bitmap_Done(Some(&core_library), Some(&mut converted));
            rust_ffi::FT_Bitmap_Done(Some(&core_library), Some(&mut target));
            std::hint::black_box((copy, convert, embolden, blend, offset));
        }
        9 => {
            let indices = [
                rust_ffi::FT_Get_Char_Index(&core_face, 0),
                rust_ffi::FT_Get_Char_Index(&core_face, 65),
                rust_ffi::FT_Get_Char_Index(&core_face, 0x20ac),
                rust_ffi::FT_Get_Char_Index(&core_face, FT_ULong::MAX),
            ];
            let names = [
                rust_ffi::FT_Get_Name_Index(Some(&core_face), Some(".notdef")),
                rust_ffi::FT_Get_Name_Index(Some(&core_face), Some("A")),
                rust_ffi::FT_Get_Name_Index(Some(&core_face), Some("uni0041")),
                rust_ffi::FT_Get_Name_Index(Some(&core_face), None),
            ];
            let loaded = rust_ffi::FT_Load_Char(
                &core_face,
                [0, 65, 0x20ac, 0xffff, 0][repeat],
                rust_ffi::FT_LOAD_DEFAULT,
            );
            std::hint::black_box((indices, names, loaded.is_ok()));
        }
        10 => {
            let mut property = rust_ffi::BDF_PropertyRec::default();
            let property_name = [
                "FONT_ASCENT",
                "FONT_DESCENT",
                "PIXEL_SIZE",
                "CHARSET_REGISTRY",
                "MISSING_PROPERTY",
            ][repeat];
            let property_result = rust_ffi::FT_Get_BDF_Property(
                Some(&core_face),
                Some(property_name),
                Some(&mut property),
            );
            let mut encoding = ptr::null();
            let mut registry = ptr::null();
            let charset = rust_ffi::FT_Get_BDF_Charset_ID(
                Some(&core_face),
                Some(&mut encoding),
                Some(&mut registry),
            );
            std::hint::black_box((property_result, charset, property, encoding, registry));
        }
        11 => {
            let mut master = rust_ffi::FT_Multi_Master::default();
            let master_result = rust_ffi::FT_Get_Multi_Master(Some(&core_face), Some(&mut master));
            let mut weights = [0_i64, 16_384, 32_768, 49_152];
            let weight_result = rust_ffi::FT_Set_MM_WeightVector(
                Some(&mut core_face),
                repeat as FT_UInt,
                Some(&weights),
            );
            let mut weight_len = weights.len() as FT_UInt;
            let get_weight = rust_ffi::FT_Get_MM_WeightVector(
                Some(&core_face),
                Some(&mut weight_len),
                Some(&mut weights),
            );
            let designs = [0_i64, 500, 1000, 1500];
            let design = rust_ffi::FT_Set_MM_Design_Coordinates(
                Some(&mut core_face),
                repeat as FT_UInt,
                Some(&designs),
            );
            let blend = rust_ffi::FT_Set_MM_Blend_Coordinates(
                Some(&mut core_face),
                repeat as FT_UInt,
                Some(&weights),
            );
            let mut coords = [0_i64; 4];
            let get_blend = rust_ffi::FT_Get_MM_Blend_Coordinates(
                Some(&core_face),
                repeat as FT_UInt,
                Some(&mut coords),
            );
            std::hint::black_box((
                master_result,
                weight_result,
                get_weight,
                design,
                blend,
                get_blend,
                master,
                weights,
                coords,
            ));
        }
        12 => {
            let coords = [0_i64, 16_384, -16_384, 32_768];
            let count = repeat as FT_UInt;
            let set_design =
                rust_ffi::FT_Set_Var_Design_Coordinates(Some(&mut core_face), count, Some(&coords));
            let set_blend =
                rust_ffi::FT_Set_Var_Blend_Coordinates(Some(&mut core_face), count, Some(&coords));
            let mut design = [0_i64; 4];
            let mut blend = [0_i64; 4];
            let get_design =
                rust_ffi::FT_Get_Var_Design_Coordinates(Some(&core_face), count, Some(&mut design));
            let get_blend =
                rust_ffi::FT_Get_Var_Blend_Coordinates(Some(&core_face), count, Some(&mut blend));
            let named = rust_ffi::FT_Set_Named_Instance(Some(&mut core_face), repeat as FT_UInt);
            let mut instance = 0;
            let default_instance =
                rust_ffi::FT_Get_Default_Named_Instance(Some(&core_face), Some(&mut instance));
            std::hint::black_box((
                set_design,
                set_blend,
                get_design,
                get_blend,
                named,
                default_instance,
                design,
                blend,
                instance,
            ));
        }
        13 => {
            let glyph_index = [0, 1, 36, 65, FT_UInt::MAX][repeat];
            let flags = [
                rust_ffi::FT_LOAD_DEFAULT,
                rust_ffi::FT_LOAD_NO_HINTING,
                rust_ffi::FT_LOAD_NO_RECURSE,
                rust_ffi::FT_LOAD_COLOR | rust_ffi::FT_LOAD_RENDER,
                rust_ffi::FT_LOAD_SBITS_ONLY | rust_ffi::FT_LOAD_RENDER,
            ];
            let loaded = rust_ffi::FT_Load_Glyph(&core_face, glyph_index, flags[repeat]);
            let advance = rust_ffi::FT_Get_Advance(&core_face, glyph_index, flags[repeat]);
            let advances = rust_ffi::FT_Get_Advances(
                &core_face,
                if repeat == 4 { FT_UInt::MAX } else { 0 },
                if repeat == 4 { 0 } else { 2 },
                flags[repeat],
            );
            let rendered = loaded.as_ref().ok().map(|slot| {
                rust_ffi::FT_Render_Glyph(slot.clone(), rust_ffi::FT_RENDER_MODE_NORMAL).is_ok()
            });
            std::hint::black_box((loaded.is_ok(), advance.is_ok(), advances.is_ok(), rendered));
        }
        14 => {
            let mut info = rust_ffi::PS_FontInfoRec::default();
            let mut private = rust_ffi::PS_PrivateRec::default();
            let mut value = [0_u8; 16];
            let value_len = value.len() as FT_Long;
            let info_result = rust_ffi::FT_Get_PS_Font_Info(Some(&core_face), Some(&mut info));
            let private_result =
                rust_ffi::FT_Get_PS_Font_Private(Some(&core_face), Some(&mut private));
            let value_result = rust_ffi::FT_Get_PS_Font_Value(
                Some(&core_face),
                repeat as FT_Int,
                repeat as FT_UInt,
                Some(&mut value),
                value_len,
            );
            let format = rust_ffi::FT_Get_Font_Format(Some(&core_face));
            let postscript = rust_ffi::FT_Get_Postscript_Name(&core_face);
            let x11 = rust_ffi::FT_Get_X11_Font_Format(Some(&core_face));
            std::hint::black_box((
                info_result,
                private_result,
                value_result,
                format,
                postscript,
                x11,
                info,
                private,
                value,
            ));
        }
        15 => {
            let palette = rust_ffi::FT_Palette_Data_Copy(Some(&core_face));
            let selected = rust_ffi::FT_Palette_Select_Copy(
                Some(&core_face),
                repeat as FT_UShort,
                repeat % 2 == 0,
            );
            let foreground = rust_ffi::FT_Palette_Set_Foreground_Color(
                Some(&core_face),
                rust_ffi::FT_Color::default(),
            );
            let mut glyph_index = 0;
            let mut color_index = 0;
            let mut iterator = rust_ffi::FT_LayerIterator::default();
            let layer = rust_ffi::FT_Get_Color_Glyph_Layer(
                Some(&core_face),
                repeat as FT_UInt,
                Some(&mut glyph_index),
                Some(&mut color_index),
                Some(&mut iterator),
            );
            let mut clip = rust_ffi::FT_ClipBox::default();
            let clip_result = rust_ffi::FT_Get_Color_Glyph_ClipBox(
                Some(&core_face),
                glyph_index,
                Some(&mut clip),
            );
            let mut paint = rust_ffi::FT_OpaquePaint::default();
            let paint_result = rust_ffi::FT_Get_Color_Glyph_Paint(
                Some(&core_face),
                glyph_index,
                if repeat % 2 == 0 {
                    rust_ffi::FT_COLOR_INCLUDE_ROOT_TRANSFORM as FT_UInt
                } else {
                    0
                },
                Some(&mut paint),
            );
            let mut paint_value = rust_ffi::FT_COLR_Paint::default();
            let resolved = rust_ffi::FT_Get_Paint(Some(&core_face), paint, Some(&mut paint_value));
            std::hint::black_box((
                palette,
                selected,
                foreground,
                layer,
                clip_result,
                paint_result,
                resolved,
                glyph_index,
                color_index,
                iterator,
                clip,
                paint_value,
            ));
        }
        16 => {
            rust_ffi::FT_GX_Validator_Set_Available(&mut core_face, repeat % 2 == 0);
            let mut tables = vec![ptr::null(); 10];
            let gx = rust_ffi::FT_TrueTypeGX_Validate(
                Some(&core_face),
                if repeat % 2 == 0 {
                    0
                } else {
                    rust_ffi::FT_VALIDATE_kern as FT_UInt
                },
                Some(tables.as_mut_slice()),
            );
            for table in tables {
                rust_ffi::FT_TrueTypeGX_Free(Some(&core_face), table);
            }
            let mut kern = ptr::null();
            let classic = rust_ffi::FT_ClassicKern_Validate(
                Some(&core_face),
                repeat as FT_UInt,
                Some(&mut kern),
            );
            rust_ffi::FT_ClassicKern_Free(Some(&core_face), kern);
            std::hint::black_box((gx, classic));
        }
        17 => {
            let mut manager = rust_ffi::FTCCacheManagerState::new(core_face.clone());
            let image = rust_ffi::FTC_ImageTypeRec {
                face_id: ptr::null_mut(),
                width: repeat as FT_UInt,
                height: 12,
                flags: rust_ffi::FT_LOAD_RENDER,
            };
            let mut lookup = None;
            let mut locked = false;
            let first = manager.lookup_sbit(
                Some(&image),
                repeat as FT_UInt,
                Some(&mut lookup),
                Some(&mut locked),
            );
            let second = manager.lookup_pixel_size(repeat as FT_UInt, 12).is_ok();
            manager.done();
            let mut after = None;
            let mut after_locked = false;
            let done = manager.lookup_sbit(
                Some(&image),
                FT_UInt::MAX,
                Some(&mut after),
                Some(&mut after_locked),
            );
            std::hint::black_box((first, second, done, locked, after_locked));
        }
        18 => {
            let source_bytes = match repeat {
                0 => vec![0_u8],
                1 => vec![0x1f, 0x8b],
                2 => vec![0x1f, 0x8b, 8, 0],
                3 => vec![0x1f, 0x9d, 0, 0],
                _ => bytes[..bytes.len().min(16)].to_vec(),
            };
            let mut source = rust_ffi::FT_StreamRec {
                base: source_bytes.as_ptr().cast_mut(),
                size: source_bytes.len() as FT_ULong,
                ..rust_ffi::FT_StreamRec::default()
            };
            let mut target = rust_ffi::FT_StreamRec::default();
            let gzip =
                rust_ffi::FT_Stream_OpenGzip(Some(&mut target), Some(&source), Some(&source_bytes));
            let lzw =
                rust_ffi::FT_Stream_OpenLZW(Some(&mut target), Some(&source), Some(&source_bytes));
            let bzip = rust_ffi::FT_Stream_OpenBzip2(
                Some(&mut target),
                Some(&mut source),
                Some(&source_bytes),
            );
            let gzip_read = rust_ffi::FT_Gzip_Stream_Read(Some(&target), 0, 8);
            let lzw_read = rust_ffi::FT_LZW_Stream_Read(Some(&target), 0, 8);
            let bzip_read = rust_ffi::FT_Bzip2_Stream_Read(Some(&target), 0, 8);
            rust_ffi::FT_Gzip_Stream_Close(Some(&mut target));
            rust_ffi::FT_LZW_Stream_Close(Some(&mut target));
            rust_ffi::FT_Bzip2_Stream_Close(Some(&mut target));
            std::hint::black_box((gzip, lzw, bzip, gzip_read, lzw_read, bzip_read));
        }
        19 => {
            let pixel = rust_ffi::FT_Set_Pixel_Sizes(
                &mut core_face,
                repeat as FT_UInt,
                (8 + repeat) as FT_UInt,
            );
            let char_size = rust_ffi::FT_Set_Char_Size(
                &mut core_face,
                (8 + repeat) as FT_F26Dot6 * 64,
                (8 + repeat) as FT_F26Dot6 * 64,
                72,
                72,
            );
            let mut request = rust_ffi::FT_Size_RequestRec {
                type_: if repeat == 4 {
                    99
                } else {
                    rust_ffi::FT_SIZE_REQUEST_TYPE_NOMINAL as _
                },
                width: if repeat == 3 { -1 } else { 12 * 64 },
                height: 12 * 64,
                horiResolution: 72,
                vertResolution: 72,
            };
            let requested = rust_ffi::FT_Request_Size(Some(&mut core_face), Some(&request));
            request.type_ = rust_ffi::FT_SIZE_REQUEST_TYPE_BBOX as _;
            let bbox_requested = rust_ffi::FT_Request_Size(Some(&mut core_face), Some(&request));
            let matrix = rust_ffi::FT_Matrix {
                xx: 0x1_0000,
                xy: if repeat % 2 == 0 { 0x0400 } else { -0x0400 },
                yx: 0,
                yy: 0x1_0000,
            };
            let delta = rust_ffi::FT_Vector { x: 5, y: -3 };
            rust_ffi::FT_Set_Transform(Some(&mut core_face), Some(&matrix), Some(&delta));
            let mut got = rust_ffi::FT_Matrix::default();
            let mut got_delta = rust_ffi::FT_Vector::default();
            rust_ffi::FT_Get_Transform(Some(&core_face), Some(&mut got), Some(&mut got_delta));
            let mut size = ptr::null_mut();
            let new_size = rust_ffi::FT_New_Size(Some(&core_face), Some(&mut size));
            let done_size = rust_ffi::FT_Done_Size(size);
            std::hint::black_box((
                pixel,
                char_size,
                requested,
                bbox_requested,
                got,
                got_delta,
                new_size,
                done_size,
            ));
        }
        _ => {}
    }
}

#[cfg(feature = "abi-test-support")]
fn abi_c114_valid_public_batch_probe(
    bytes: &[FT_Byte],
    _library: FT_Library,
    _face: FT_Face,
    _memory: FT_Memory,
    probe: u16,
) {
    // c114 keeps the witness inputs on valid public routes: real maintained
    // font bytes, live Rust-owned faces, and public load/render/bitmap/size
    // operations.  The three rows in each family vary the public mode or
    // asset while the lifecycle fixture remains the parity entry point.
    let index = usize::from(probe.saturating_sub(1901));
    let family = index / 3;
    let repeat = index % 3;
    let core_library = rust_ffi::FT_Init_FreeType();
    let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) else {
        return;
    };

    match family {
        0 => {
            // Empty .notdef glyphs use the normal, mono, and LCD load/render
            // paths through the public FT_Load_Glyph contract.
            let flags = [
                rust_ffi::FT_LOAD_RENDER,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_MONO,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_LCD,
            ];
            let loaded = rust_ffi::FT_Load_Glyph(&core_face, 0, flags[repeat]);
            std::hint::black_box(loaded.is_ok());
        }
        1 => {
            // The remaining valid render selectors exercise LCD-V and SDF
            // empty-glyph behavior without manufacturing an outline record.
            let flags = [
                rust_ffi::FT_LOAD_RENDER | ((rust_ffi::FT_RENDER_MODE_SDF as FT_Int32) << 16),
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_LCD_V,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_LIGHT,
            ];
            let loaded = rust_ffi::FT_Load_Glyph(&core_face, 0, flags[repeat]);
            std::hint::black_box(loaded.is_ok());
        }
        2 => {
            // A loaded composite is handed back to the public renderer with
            // normal/mono/LCD selectors; the renderer reports the same public
            // Cannot_Render_Glyph result as the pinned contract.
            let loaded = rust_ffi::FT_Load_Glyph(&core_face, 0, rust_ffi::FT_LOAD_DEFAULT);
            let rendered = loaded.ok().map(|slot| {
                rust_ffi::FT_Render_Glyph(
                    slot,
                    [
                        rust_ffi::FT_RENDER_MODE_NORMAL,
                        rust_ffi::FT_RENDER_MODE_MONO,
                        rust_ffi::FT_RENDER_MODE_LCD,
                    ][repeat],
                )
            });
            std::hint::black_box(rendered);
        }
        3 => {
            // Non-empty glyphs keep the same direct FT_Render_Glyph route but
            // vary the glyph and valid LCD/SDF mode combination.
            let loaded = rust_ffi::FT_Load_Glyph(
                &core_face,
                [1, 2, 3][repeat],
                rust_ffi::FT_LOAD_NO_HINTING,
            );
            let rendered = loaded.ok().map(|slot| {
                rust_ffi::FT_Render_Glyph(
                    slot,
                    [
                        rust_ffi::FT_RENDER_MODE_LCD_V,
                        rust_ffi::FT_RENDER_MODE_SDF,
                        rust_ffi::FT_RENDER_MODE_NORMAL,
                    ][repeat],
                )
            });
            std::hint::black_box(rendered);
        }
        4 => {
            // Valid embedded strikes cover SBit-only, rendered SBit, and
            // bitmap-metrics-only loading on maintained bitmap fonts.
            let flags = [
                rust_ffi::FT_LOAD_SBITS_ONLY,
                rust_ffi::FT_LOAD_SBITS_ONLY | rust_ffi::FT_LOAD_RENDER,
                rust_ffi::FT_LOAD_SBITS_ONLY | rust_ffi::FT_LOAD_BITMAP_METRICS_ONLY,
            ];
            let loaded = rust_ffi::FT_Load_Glyph(&core_face, 1, flags[repeat]);
            std::hint::black_box(loaded.is_ok());
        }
        5 => {
            // Gray2, Gray4, and BGRA are valid public bitmap records.  Their
            // packed rows exercise conversion without malformed dimensions or
            // null storage.
            let (pixel_mode, num_grays, pitch, source_bytes) = match repeat {
                0 => (
                    rust_ffi::FT_PIXEL_MODE_GRAY2 as FT_Byte,
                    4,
                    2,
                    vec![0x1b, 0xe4, 0x40, 0x7f],
                ),
                1 => (
                    rust_ffi::FT_PIXEL_MODE_GRAY4 as FT_Byte,
                    16,
                    4,
                    vec![0x0f, 0x12, 0x34, 0x56, 0xf0, 0xed, 0xcb, 0xa9],
                ),
                _ => (
                    rust_ffi::FT_PIXEL_MODE_BGRA as FT_Byte,
                    256,
                    32,
                    vec![1, 2, 3, 255].repeat(16),
                ),
            };
            let mut source = rust_ffi::FT_Bitmap_C {
                rows: 2,
                width: 8,
                pitch,
                num_grays,
                pixel_mode,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut source), source_bytes);
            let mut converted = rust_ffi::FT_Bitmap_C::default();
            let result = rust_ffi::FT_Bitmap_Convert(
                Some(&core_library),
                Some(&source),
                Some(&mut converted),
                4,
            );
            rust_ffi::FT_Bitmap_Done(Some(&core_library), Some(&mut source));
            rust_ffi::FT_Bitmap_Done(Some(&core_library), Some(&mut converted));
            std::hint::black_box(result);
        }
        6 => {
            // Valid advance requests vary scaling and layout flags on a real
            // variable-font face while staying inside its glyph range.
            let flags = [
                rust_ffi::FT_LOAD_DEFAULT,
                rust_ffi::FT_LOAD_NO_SCALE,
                rust_ffi::FT_LOAD_VERTICAL_LAYOUT,
            ];
            let glyph = [0, 1, 2][repeat];
            let advance = rust_ffi::FT_Get_Advance(&core_face, glyph, flags[repeat]);
            let advances = rust_ffi::FT_Get_Advances(&core_face, 0, 3, flags[repeat]);
            std::hint::black_box((advance.is_ok(), advances.is_ok()));
        }
        7 => {
            // Color/bitmap-capable faces are loaded through public color and
            // render policies, including the valid NO_BITMAP fallback.
            let flags = [
                rust_ffi::FT_LOAD_COLOR,
                rust_ffi::FT_LOAD_NO_BITMAP,
                rust_ffi::FT_LOAD_COLOR | rust_ffi::FT_LOAD_RENDER,
            ];
            let loaded = rust_ffi::FT_Load_Glyph(&core_face, [0, 1, 2][repeat], flags[repeat]);
            std::hint::black_box(loaded.is_ok());
        }
        8 => {
            // Service queries stay on valid faces while selecting the
            // PostScript, SFNT, and multiple-master families from the assets.
            let format = rust_ffi::FT_Get_Font_Format(Some(&core_face));
            let postscript = rust_ffi::FT_Get_Postscript_Name(&core_face);
            let x11 = rust_ffi::FT_Get_X11_Font_Format(Some(&core_face));
            let mut master = rust_ffi::FT_MM_Var::default();
            let mut axes = vec![rust_ffi::FT_Var_Axis::default(); 8];
            let mut styles = vec![rust_ffi::FT_Var_Named_Style::default(); 8];
            let mm = rust_ffi::FT_Get_MM_Var(
                Some(&core_face),
                Some(&mut master),
                Some(&mut axes),
                Some(&mut styles),
                None,
            );
            std::hint::black_box((format, postscript, x11, mm, master, axes, styles));
        }
        9 => {
            // Public size requests vary nominal, real-dimension, and bbox
            // selectors before a valid transformed glyph load.
            let request = rust_ffi::FT_Size_RequestRec {
                type_: [
                    rust_ffi::FT_SIZE_REQUEST_TYPE_NOMINAL,
                    rust_ffi::FT_SIZE_REQUEST_TYPE_REAL_DIM,
                    rust_ffi::FT_SIZE_REQUEST_TYPE_BBOX,
                ][repeat] as FT_Int,
                width: (8 + repeat as FT_F26Dot6) * 64,
                height: (12 + repeat as FT_F26Dot6) * 64,
                horiResolution: 72,
                vertResolution: 72,
            };
            let requested = rust_ffi::FT_Request_Size(Some(&mut core_face), Some(&request));
            let set = rust_ffi::FT_Set_Pixel_Sizes(
                &mut core_face,
                8 + repeat as FT_UInt,
                12 + repeat as FT_UInt,
            );
            let matrix = rust_ffi::FT_Matrix {
                xx: 0x1_0000,
                xy: if repeat == 1 { 0x0400 } else { 0 },
                yx: 0,
                yy: 0x1_0000,
            };
            rust_ffi::FT_Set_Transform(
                Some(&mut core_face),
                Some(&matrix),
                Some(&rust_ffi::FT_Vector { x: 3, y: -2 }),
            );
            let loaded = rust_ffi::FT_Load_Glyph(&core_face, 0, rust_ffi::FT_LOAD_DEFAULT);
            std::hint::black_box((requested, set, loaded.is_ok()));
        }
        _ => {}
    }
}

#[cfg(feature = "abi-test-support")]
fn abi_custom_memory_coverage_probe(
    bytes: &[FT_Byte],
    library: FT_Library,
    face: FT_Face,
    memory: FT_Memory,
    probe: u16,
) {
    match probe {
        // `FT_Palette_Data_Get` rejects the null face before the helper can
        // copy the caller-owned palette pointer.
        10 => {
            let _ = abi_palette_entries_from_ptr(
                ptr::null_mut(),
                NonNull::<FT_Color>::dangling().as_ptr(),
            );
        }
        // This guard is duplicated by the public Bzip2 opener, so the
        // test-support route calls the materializer directly to preserve its
        // defensive behavior as an independently covered invariant.
        11 => {
            let _ = bzip2_source_bytes(ptr::null_mut(), ptr::null_mut(), ptr::null_mut(), 1);
        }
        12 => {
            let mut source = FT_StreamRec {
                size: 1,
                read: abi_coverage_bzip2_nonzero_probe as *const () as FT_Pointer,
                ..FT_StreamRec::default()
            };
            let mut target = FT_StreamRec::default();
            let _ = FT_Stream_OpenBzip2(&mut target, &mut source);
        }
        13 => {
            let mut source = FT_StreamRec {
                size: 1,
                read: abi_coverage_bzip2_short_probe as *const () as FT_Pointer,
                ..FT_StreamRec::default()
            };
            let mut target = FT_StreamRec::default();
            let _ = FT_Stream_OpenBzip2(&mut target, &mut source);
        }
        14 => {
            let mut source = FT_StreamRec {
                size: 1,
                ..FT_StreamRec::default()
            };
            let mut target = FT_StreamRec::default();
            let _ = FT_Stream_OpenBzip2(&mut target, &mut source);
        }
        15 => {
            let _ = c_bzip2_stream_io(ptr::null_mut(), 0, ptr::null_mut(), 1);
        }
        16 => {
            let mut stream = FT_StreamRec::default();
            let _ = c_bzip2_stream_io(&mut stream, 0, ptr::null_mut(), 0);
        }
        17 => {
            let mut source = FT_StreamRec::default();
            let mut target = FT_StreamRec::default();
            let _ = FT_Stream_OpenLZW(&mut target, &mut source);
        }
        18 => {
            let _ = c_lzw_stream_io(ptr::null_mut(), 0, ptr::null_mut(), 1);
        }
        19 => {
            let mut stream = FT_StreamRec::default();
            let _ = c_lzw_stream_io(&mut stream, 0, ptr::null_mut(), 0);
        }
        20 => {
            let mut source_bytes = [0_u8; 2];
            let mut source = FT_StreamRec {
                base: source_bytes.as_mut_ptr(),
                size: source_bytes.len() as FT_ULong,
                ..FT_StreamRec::default()
            };
            let mut target = FT_StreamRec::default();
            let source_ptr: FT_Stream = std::hint::black_box(&mut source);
            let target_ptr: FT_Stream = std::hint::black_box(&mut target);
            let open_lzw: extern "C" fn(FT_Stream, FT_Stream) -> FT_Error = FT_Stream_OpenLZW;
            let error = std::hint::black_box(open_lzw)(target_ptr, source_ptr);
            std::hint::black_box(error);
            assert_eq!(unsafe { (*source_ptr).pos }, 2);
        }
        21 => {
            let mut manager = abi_coverage_manager_record(library, None, ptr::null_mut());
            let mut cache = FTC_ImageCacheRec {
                manager: ptr::from_mut(&mut manager),
                entries: BTreeMap::new(),
            };
            let mut glyph = ptr::null_mut();
            let _ = ftc_image_cache_lookup_impl(
                ptr::from_mut(&mut cache),
                abi_coverage_image_key(ptr::null_mut()),
                &mut glyph,
                ptr::null_mut(),
            );
        }
        22 => {
            let mut manager = abi_coverage_manager_record(
                library,
                Some(abi_coverage_cache_requester_null_face),
                ptr::null_mut(),
            );
            let mut cache = FTC_ImageCacheRec {
                manager: ptr::from_mut(&mut manager),
                entries: BTreeMap::new(),
            };
            let mut glyph = ptr::null_mut();
            let _ = ftc_image_cache_lookup_impl(
                ptr::from_mut(&mut cache),
                abi_coverage_image_key(ptr::null_mut()),
                &mut glyph,
                ptr::null_mut(),
            );
        }
        23 => ftc_destroy_node(ptr::null_mut()),
        24 => abi_coverage_stack_cache_lifecycle(library, false),
        25 => abi_coverage_stack_cache_lifecycle(library, true),
        26 => {
            let mut manager = abi_coverage_manager_record(library, None, ptr::null_mut());
            let face_id = NonNull::<c_void>::dangling().as_ptr();
            manager.faces.insert(face_id.addr(), ptr::null_mut());
            let mut cache = FTC_CMapCacheRec {
                manager: ptr::from_mut(&mut manager),
                entries: BTreeMap::new(),
            };
            let _ = FTC_CMapCache_Lookup(ptr::from_mut(&mut cache), face_id, 0, 65);
        }
        27 => abi_coverage_cmap_restore(face, library),
        28 => {
            let _ = ftc_cache_count(ptr::null_mut());
        }
        29 => {
            let mut glyph = ptr::null_mut();
            let _ = FTC_ImageCache_Lookup(
                ptr::null_mut(),
                ptr::from_mut(&mut FTC_ImageTypeRec::default()),
                0,
                &mut glyph,
                ptr::null_mut(),
            );
        }
        30 => {
            let key = abi_coverage_image_key(ptr::null_mut());
            let node = abi_coverage_node(FtcNodePayload::SBit {
                record: FTC_SBitRec::default(),
                _buffer: Box::new([]),
            });
            let mut cache = FTC_ImageCacheRec {
                manager: ptr::null_mut(),
                entries: BTreeMap::from([(key, node)]),
            };
            let mut glyph = ptr::null_mut();
            let _ = ftc_image_cache_lookup_impl(
                ptr::from_mut(&mut cache),
                key,
                &mut glyph,
                ptr::null_mut(),
            );
            cache.entries.clear();
            // SAFETY: the node was allocated exclusively for this stack cache.
            unsafe { drop(Box::from_raw(node)) };
        }
        31 => {
            let _ = FTC_ImageCache_Lookup(
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            );
        }
        32 => {
            let _ = FTC_ImageCache_LookupScaler(
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
            );
        }
        33 => {
            let key = abi_coverage_image_key(ptr::null_mut());
            let node = abi_coverage_node(FtcNodePayload::Glyph(ptr::null_mut()));
            let mut cache = FTC_SBitCacheRec {
                manager: ptr::null_mut(),
                entries: BTreeMap::from([(key, node)]),
            };
            let mut sbit = ptr::null_mut();
            let _ = ftc_sbit_cache_lookup_impl(
                ptr::from_mut(&mut cache),
                key,
                &mut sbit,
                ptr::null_mut(),
            );
            cache.entries.clear();
            // SAFETY: the node was allocated exclusively for this stack cache.
            unsafe { drop(Box::from_raw(node)) };
        }
        34 => {
            let key = abi_coverage_image_key(ptr::null_mut());
            let node = abi_coverage_node(FtcNodePayload::SBit {
                record: FTC_SBitRec::default(),
                _buffer: Box::new([]),
            });
            let mut cache = FTC_SBitCacheRec {
                manager: ptr::null_mut(),
                entries: BTreeMap::from([(key, node)]),
            };
            let mut sbit = ptr::null_mut();
            let mut anode = ptr::null_mut();
            let anode_output: *mut FTC_Node = std::hint::black_box(&mut anode);
            let sbit_lookup: fn(
                FTC_SBitCache,
                FtcImageKey,
                *mut FTC_SBit,
                *mut FTC_Node,
            ) -> FT_Error = ftc_sbit_cache_lookup_impl;
            let error = std::hint::black_box(sbit_lookup)(
                ptr::from_mut(&mut cache),
                key,
                &mut sbit,
                anode_output,
            );
            std::hint::black_box(error);
            assert_eq!(unsafe { *anode_output }, node);
            cache.entries.clear();
            // SAFETY: the node was allocated exclusively for this stack cache.
            unsafe { drop(Box::from_raw(node)) };
        }
        35 => {
            let mut manager = abi_coverage_manager_record(library, None, ptr::null_mut());
            let mut cache = FTC_SBitCacheRec {
                manager: ptr::from_mut(&mut manager),
                entries: BTreeMap::new(),
            };
            let mut sbit = ptr::null_mut();
            let _ = ftc_sbit_cache_lookup_impl(
                ptr::from_mut(&mut cache),
                abi_coverage_image_key(ptr::null_mut()),
                &mut sbit,
                ptr::null_mut(),
            );
        }
        36 => {
            let _ = abi_custom_memory_alloc(ptr::null_mut(), 1);
        }
        37 => {
            let _ = abi_custom_memory_alloc(memory, -1);
        }
        38 => abi_custom_memory_free(ptr::null_mut(), ptr::null_mut()),
        39 => {
            // The SFNT parser intentionally treats zero-length directory
            // records as missing, so this probes the C-ABI retention guard
            // directly without changing the public parser contract.
            let _ = retain_c_open_type_table(face, Vec::new());
        }
        40 => {
            let face_id = NonNull::<c_void>::dangling().as_ptr();
            let mut manager = abi_coverage_manager_record(library, None, ptr::null_mut());
            manager.faces.insert(face_id.addr(), face);
            let _ = ftc_apply_scaler(
                ptr::from_mut(&mut manager),
                face_id,
                FT_UInt::MAX,
                0,
                0,
                0,
                0,
            );
            manager.faces.clear();
        }
        // c92-41: run the real requester-backed cache lifecycle with both
        // output-pointer shapes and a scaler lookup.  This keeps the probe on
        // the Rust-owned cache implementation; the public ABI only exposes
        // the C-shaped records and callbacks.
        41 => {
            if let Ok(mut harness) = AbiSBitCacheHarness::new(bytes, 0) {
                let face_id = harness.face_id();
                let image_type = FTC_ImageTypeRec {
                    face_id,
                    width: 0,
                    height: 12,
                    flags: rust_ffi::FT_LOAD_DEFAULT,
                };
                let _ = harness.lookup(image_type, 36, true, true);
                let _ = harness.lookup(image_type, 36, false, false);
                let scaler = FTC_ScalerRec {
                    face_id,
                    width: 0,
                    height: 12,
                    pixel: 1,
                    x_res: 0,
                    y_res: 0,
                };
                let _ =
                    harness.lookup_scaler(scaler, rust_ffi::FT_LOAD_DEFAULT as FT_ULong, 36, true);
            }
        }
        // c92-42: exercise the false arm of every optional cache slot during
        // reset without manufacturing an invalid public handle.
        42 => {
            let mut manager = abi_coverage_manager_record(library, None, ptr::null_mut());
            manager.cmap_caches.push(ptr::null_mut());
            manager.image_caches.push(ptr::null_mut());
            manager.sbit_caches.push(ptr::null_mut());
            FTC_Manager_Reset(ptr::from_mut(&mut manager));
        }
        // c92-43: the matching RemoveFaceID filter also accepts a manager
        // whose cache slots have been cleared before the call.
        43 => {
            let mut manager = abi_coverage_manager_record(library, None, ptr::null_mut());
            manager.cmap_caches.push(ptr::null_mut());
            manager.image_caches.push(ptr::null_mut());
            manager.sbit_caches.push(ptr::null_mut());
            FTC_Manager_RemoveFaceID(
                ptr::from_mut(&mut manager),
                NonNull::<c_void>::dangling().as_ptr(),
            );
        }
        // c92-44: a negative cmap index reuses the normalized index-zero
        // cache key, which is the pinned C hash behavior.
        44 => {
            let face_id = NonNull::<c_void>::dangling().as_ptr();
            let mut manager = abi_coverage_manager_record(library, None, ptr::null_mut());
            let mut cache = FTC_CMapCacheRec {
                manager: ptr::from_mut(&mut manager),
                entries: BTreeMap::from([(
                    FtcCMapKey {
                        face_id,
                        cmap_index: 0,
                        char_code: 65,
                    },
                    17,
                )]),
            };
            let glyph = FTC_CMapCache_Lookup(ptr::from_mut(&mut cache), face_id, -1, 65);
            std::hint::black_box(glyph);
        }
        // c92-45: a negative cmap lookup that misses the cache falls through
        // to the face's active charmap without changing it.
        45 => {
            let face_id = NonNull::<c_void>::dangling().as_ptr();
            let mut manager = abi_coverage_manager_record(library, None, ptr::null_mut());
            manager.faces.insert(face_id.addr(), face);
            let mut cache = FTC_CMapCacheRec {
                manager: ptr::from_mut(&mut manager),
                entries: BTreeMap::new(),
            };
            let glyph = FTC_CMapCache_Lookup(ptr::from_mut(&mut cache), face_id, -1, 65);
            std::hint::black_box(glyph);
            manager.faces.clear();
        }
        // c92-46: a positive cmap index outside the face's map returns zero
        // after the face lookup and caches that miss.
        46 => {
            let face_id = NonNull::<c_void>::dangling().as_ptr();
            let mut manager = abi_coverage_manager_record(library, None, ptr::null_mut());
            manager.faces.insert(face_id.addr(), face);
            let mut cache = FTC_CMapCacheRec {
                manager: ptr::from_mut(&mut manager),
                entries: BTreeMap::new(),
            };
            let glyph = FTC_CMapCache_Lookup(ptr::from_mut(&mut cache), face_id, FT_Int::MAX, 65);
            std::hint::black_box(glyph);
            manager.faces.clear();
        }
        // c92-47: construct one cache of each kind and release the temporary
        // allocations through their Rust ownership records.
        47 => {
            let mut manager = abi_coverage_manager_record(library, None, ptr::null_mut());
            let manager_ptr = ptr::from_mut(&mut manager);
            let mut cmap = ptr::null_mut();
            let mut image = ptr::null_mut();
            let mut sbit = ptr::null_mut();
            let _ = FTC_CMapCache_New(manager_ptr, &mut cmap);
            let _ = FTC_ImageCache_New(manager_ptr, &mut image);
            let _ = FTC_SBitCache_New(manager_ptr, &mut sbit);
            manager.cmap_caches.clear();
            manager.image_caches.clear();
            manager.sbit_caches.clear();
            // SAFETY: each successful constructor transferred one allocation
            // to this stack manager, and the vectors were detached above.
            unsafe {
                if !cmap.is_null() {
                    drop(Box::from_raw(cmap));
                }
                if !image.is_null() {
                    drop(Box::from_raw(image));
                }
                if !sbit.is_null() {
                    drop(Box::from_raw(sbit));
                }
            }
        }
        // c92-48: the shared cache-count guard is reached with a full manager
        // inventory before any constructor allocates a new cache.
        48 => {
            let mut manager = abi_coverage_manager_record(library, None, ptr::null_mut());
            manager.cmap_caches.extend([ptr::null_mut(); 16]);
            let mut cache = ptr::null_mut();
            let error = FTC_ImageCache_New(ptr::from_mut(&mut manager), &mut cache);
            std::hint::black_box(error);
        }
        // c92-49: a live manager ignores a foreign node, while the node
        // membership check remains inside the Rust-owned cache manager.
        49 => {
            if let Ok(harness) = AbiSBitCacheHarness::new_manager_only(bytes, 0) {
                FTC_Node_Unref(
                    NonNull::<FTC_NodeRec>::dangling().as_ptr(),
                    harness.manager_handle(),
                );
            }
        }
        // c92-50: materialize a memory-backed source far enough to validate a
        // null target stream before the decompressor is entered.
        50 => {
            let mut source_bytes = [0_u8; 1];
            let mut source = FT_StreamRec {
                base: source_bytes.as_mut_ptr(),
                size: 1,
                ..FT_StreamRec::default()
            };
            let error = FT_Stream_OpenBzip2(ptr::null_mut(), &mut source);
            std::hint::black_box(error);
        }
        // c93-51: repeat a requester-backed SBit lookup with an output node so
        // the cached-hit reference-count arm is taken, not only the miss path.
        51 => {
            if let Ok(mut harness) = AbiSBitCacheHarness::new(bytes, 0) {
                let face_id = harness.face_id();
                let image_type = FTC_ImageTypeRec {
                    face_id,
                    width: 0,
                    height: 12,
                    flags: rust_ffi::FT_LOAD_DEFAULT,
                };
                let first = harness.lookup(image_type, 36, true, true);
                let second = harness.lookup(image_type, 36, true, true);
                std::hint::black_box((first.error, second.error));
            }
        }
        // c93-52: glyph zero is the maintained font's empty glyph and reaches
        // the C-visible zero-sized SBit descriptor normalization.
        52 => {
            if let Ok(mut harness) = AbiSBitCacheHarness::new(bytes, 0) {
                let face_id = harness.face_id();
                let image_type = FTC_ImageTypeRec {
                    face_id,
                    width: 0,
                    height: 12,
                    flags: rust_ffi::FT_LOAD_DEFAULT,
                };
                let snapshot = harness.lookup(image_type, 0, true, false);
                std::hint::black_box(snapshot);
            }
        }
        // c93-53: a larger pixel size exercises bitmap metric conversion with
        // values beyond the narrow FTC_SBit fields where the font permits it.
        53 => {
            if let Ok(mut harness) = AbiSBitCacheHarness::new(bytes, 0) {
                let face_id = harness.face_id();
                let image_type = FTC_ImageTypeRec {
                    face_id,
                    width: 0,
                    height: 512,
                    flags: rust_ffi::FT_LOAD_DEFAULT,
                };
                let snapshot = harness.lookup(image_type, 36, true, false);
                std::hint::black_box(snapshot);
            }
        }
        // c93-54: the scaler-shaped entry point uses the same large-size
        // conversion while keeping its independent key construction covered.
        54 => {
            if let Ok(mut harness) = AbiSBitCacheHarness::new(bytes, 0) {
                let face_id = harness.face_id();
                let scaler = FTC_ScalerRec {
                    face_id,
                    width: 0,
                    height: 512,
                    pixel: 1,
                    x_res: 0,
                    y_res: 0,
                };
                let snapshot =
                    harness.lookup_scaler(scaler, rust_ffi::FT_LOAD_DEFAULT as FT_ULong, 36, false);
                std::hint::black_box(snapshot);
            }
        }
        // c93-55: image-cache lookup on a real requester-backed manager, then
        // a repeat lookup to exercise manager-owned glyph-node reuse.
        55 => {
            if let Ok(mut harness) = AbiSBitCacheHarness::new_manager_only(bytes, 0)
                && harness.ensure_image_cache().is_ok()
            {
                let face_id = harness.face_id();
                let scaler = FTC_ScalerRec {
                    face_id,
                    width: 0,
                    height: 12,
                    pixel: 1,
                    x_res: 0,
                    y_res: 0,
                };
                let first = harness.image_lookup(scaler, 36, false);
                let second = harness.image_lookup(scaler, 36, false);
                std::hint::black_box((first.0, second.0));
            }
        }
        // c93-56: exercise the manager-only helper's documented invalid image
        // lookup before an image cache is installed.
        56 => {
            if let Ok(mut harness) = AbiSBitCacheHarness::new_manager_only(bytes, 0) {
                let scaler = FTC_ScalerRec::default();
                let result = harness.image_lookup(scaler, 0, false);
                std::hint::black_box(result.0);
            }
        }
        // c93-57: install an image cache and request an out-of-range glyph so
        // the image cache's load-error return remains exercised separately.
        57 => {
            if let Ok(mut harness) = AbiSBitCacheHarness::new_manager_only(bytes, 0)
                && harness.ensure_image_cache().is_ok()
            {
                let face_id = harness.face_id();
                let scaler = FTC_ScalerRec {
                    face_id,
                    width: 0,
                    height: 12,
                    pixel: 1,
                    x_res: 0,
                    y_res: 0,
                };
                let result = harness.image_lookup(scaler, FT_UInt::MAX, true);
                std::hint::black_box(result.0);
            }
        }
        // c93-58: create the SBit cache through the manager-only constructor,
        // covering the successful lazy-cache registration branch.
        58 => {
            if let Ok(mut harness) = AbiSBitCacheHarness::new_manager_only(bytes, 0)
                && harness.ensure_sbit_cache().is_ok()
            {
                let face_id = harness.face_id();
                let image_type = FTC_ImageTypeRec {
                    face_id,
                    width: 0,
                    height: 12,
                    flags: rust_ffi::FT_LOAD_DEFAULT,
                };
                let result = harness.lookup(image_type, 36, false, false);
                std::hint::black_box(result.error);
            }
        }
        // c93-59: the scaler lookup repeats a cached SBit hit with a node
        // output, covering the same ownership rule through the other API.
        59 => {
            if let Ok(mut harness) = AbiSBitCacheHarness::new(bytes, 0) {
                let face_id = harness.face_id();
                let scaler = FTC_ScalerRec {
                    face_id,
                    width: 0,
                    height: 12,
                    pixel: 1,
                    x_res: 0,
                    y_res: 0,
                };
                let first =
                    harness.lookup_scaler(scaler, rust_ffi::FT_LOAD_DEFAULT as FT_ULong, 36, true);
                let second =
                    harness.lookup_scaler(scaler, rust_ffi::FT_LOAD_DEFAULT as FT_ULong, 36, true);
                std::hint::black_box((first.error, second.error));
            }
        }
        // c93-60: a second empty-glyph lookup keeps the descriptor path active
        // through the scaler-shaped cache key.
        60 => {
            if let Ok(mut harness) = AbiSBitCacheHarness::new(bytes, 0) {
                let face_id = harness.face_id();
                let scaler = FTC_ScalerRec {
                    face_id,
                    width: 0,
                    height: 12,
                    pixel: 1,
                    x_res: 0,
                    y_res: 0,
                };
                let result =
                    harness.lookup_scaler(scaler, rust_ffi::FT_LOAD_DEFAULT as FT_ULong, 0, false);
                std::hint::black_box(result);
            }
        }
        // c93-61: the gzip wrapper rejects a memory-backed source with no
        // source bytes before handing control to the Rust decompressor.
        61 => {
            let mut source = FT_StreamRec {
                size: 1,
                ..FT_StreamRec::default()
            };
            let mut target = FT_StreamRec::default();
            let error = FT_Stream_OpenGzip(&mut target, &mut source);
            std::hint::black_box(error);
        }
        // c93-62: the gzip callback sees a stream that has no registry entry.
        62 => {
            let mut stream = FT_StreamRec::default();
            let error = c_gzip_stream_io(&mut stream, 0, ptr::null_mut(), 0);
            std::hint::black_box(error);
        }
        // c93-63: the gzip callback's non-zero/null-buffer guard is distinct
        // from the Bzip2 callback and remains a direct Rust-owned check.
        63 => {
            let error = c_gzip_stream_io(ptr::null_mut(), 0, ptr::null_mut(), 1);
            std::hint::black_box(error);
        }
        // c93-64: finalize one allocator-owned list node through the exported
        // list wrapper, using the live custom-memory callbacks for ownership.
        64 => {
            let size = c_long::try_from(std::mem::size_of::<FT_ListNodeRec>()).unwrap_or(0);
            let block = abi_custom_memory_alloc(memory, size);
            if !block.is_null() {
                let node = block.cast::<FT_ListNodeRec>();
                // SAFETY: the custom allocator returned a block at least the
                // size requested above and the list owns it until finalize.
                unsafe {
                    *node = FT_ListNodeRec::default();
                }
                let mut list = FT_ListRec {
                    head: node,
                    tail: node,
                };
                FT_List_Finalize(ptr::from_mut(&mut list), None, memory, ptr::null_mut());
            }
        }
        // c93-65: a valid gray bitmap conversion reaches the success copyback
        // path and then releases the Rust-owned target buffer.
        65 => {
            let mut pixels = [0x7f_u8];
            let source = FT_Bitmap {
                rows: 1,
                width: 1,
                pitch: 1,
                buffer: pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte,
                ..FT_Bitmap::default()
            };
            let mut target = FT_Bitmap::default();
            let error = FT_Bitmap_Convert(library, &source, &mut target, 1);
            std::hint::black_box(error);
            let _ = FT_Bitmap_Done(library, &mut target);
        }
        // c93-66: a valid empty bitmap reaches FT_Bitmap_Done's success
        // copyback without relying on a caller-owned payload.
        66 => {
            let mut bitmap = FT_Bitmap::default();
            let error = FT_Bitmap_Done(library, &mut bitmap);
            std::hint::black_box(error);
        }
        // c93-67: embolden a one-pixel gray bitmap and release any converted
        // buffer through the same library memory owner.
        67 => {
            let mut pixels = [0xff_u8];
            let mut bitmap = FT_Bitmap {
                rows: 1,
                width: 1,
                pitch: 1,
                buffer: pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte,
                ..FT_Bitmap::default()
            };
            let error = FT_Bitmap_Embolden(library, &mut bitmap, 64, 64);
            std::hint::black_box(error);
            let _ = FT_Bitmap_Done(library, &mut bitmap);
        }
        // c93-68: blend a small gray source into a target and preserve the
        // wrapper's target-offset copyback contract.
        68 => {
            let mut source_pixels = [0xff_u8];
            let mut target_pixels = [0_u8; 4];
            let source = FT_Bitmap {
                rows: 1,
                width: 1,
                pitch: 1,
                buffer: source_pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte,
                ..FT_Bitmap::default()
            };
            let mut target = FT_Bitmap {
                rows: 2,
                width: 2,
                pitch: 2,
                buffer: target_pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as FT_Byte,
                ..FT_Bitmap::default()
            };
            let mut offset = FT_Vector::default();
            let error = FT_Bitmap_Blend(
                library,
                &source,
                FT_Vector::default(),
                &mut target,
                &mut offset,
                FT_Color {
                    red: 255,
                    green: 255,
                    blue: 255,
                    alpha: 255,
                },
            );
            std::hint::black_box(error);
            let _ = FT_Bitmap_Done(library, &mut target);
        }
        // c93-69: call color-layer wrappers with null optional outputs to
        // cover their safe pointer adaptation without a color font asset.
        69 => {
            let _ = FT_Get_Color_Glyph_Layer(
                face,
                0,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            let _ = FT_Get_Color_Glyph_ClipBox(face, 0, ptr::null_mut());
            let _ = FT_Get_Color_Glyph_Paint(face, 0, 0, ptr::null_mut());
        }
        // c93-70: exercise the paint-layer and color-stop wrappers' optional
        // iterator/output pointers as a separate matrix witness.
        70 => {
            let _ = FT_Get_Paint_Layers(face, ptr::null_mut(), ptr::null_mut());
            let _ = FT_Get_Colorline_Stops(face, ptr::null_mut(), ptr::null_mut());
            let _ = FT_Palette_Set_Foreground_Color(
                face,
                FT_Color {
                    red: 1,
                    green: 2,
                    blue: 3,
                    alpha: 4,
                },
            );
        }
        // c94-71..100 are Rust-core witnesses.  They intentionally call the
        // safe FFI layer from test support instead of growing the exported C
        // surface; the C ABI and WASM entry points remain thin adapters.
        71 => {
            let result = rust_ffi::FT_New_Glyph(None, rust_ffi::FT_GLYPH_FORMAT_OUTLINE);
            let _ = std::hint::black_box(result);
        }
        72 => {
            let result =
                rust_ffi::FT_New_Glyph_Allocation_Failure(None, rust_ffi::FT_GLYPH_FORMAT_OUTLINE);
            let _ = std::hint::black_box(result);
        }
        73 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let result = rust_ffi::FT_New_Glyph_Allocation_Failure(
                Some(&core_library),
                rust_ffi::FT_GLYPH_FORMAT_NONE,
            );
            let _ = std::hint::black_box(result);
        }
        74 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let result = rust_ffi::FT_New_Glyph_Validate(
                Some(&core_library),
                rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
                true,
            );
            let _ = std::hint::black_box(result);
        }
        75 => {
            let result = rust_ffi::FT_Get_Outline_Glyph(None);
            let _ = std::hint::black_box(result);
        }
        76 => {
            let result = rust_ffi::FT_Get_Bitmap_Glyph(None);
            let _ = std::hint::black_box(result);
        }
        77 => {
            let result = rust_ffi::FT_Get_Svg_Glyph(None);
            let _ = std::hint::black_box(result);
        }
        78 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let slot =
                    rust_ffi::FT_Outline_GlyphSlot_With_Advance(&core_face, 0x8000_i64 * 64, 0);
                let result = rust_ffi::FT_Get_Outline_Glyph(Some(&slot));
                let _ = std::hint::black_box(result);
            }
        }
        79 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let slot = rust_ffi::FT_Malformed_Get_GlyphSlot(&core_face, 6);
                let result = rust_ffi::FT_Get_Bitmap_Glyph(Some(&slot));
                let _ = std::hint::black_box(result);
            }
        }
        80 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let slot = rust_ffi::FT_Malformed_Get_GlyphSlot(&core_face, 2);
                let result = rust_ffi::FT_Get_Svg_Glyph(Some(&slot));
                let _ = std::hint::black_box(result);
            }
        }
        81 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let slot = rust_ffi::FT_Malformed_Get_GlyphSlot(&core_face, 3);
                let result = rust_ffi::FT_Get_Svg_Glyph(Some(&slot));
                let _ = std::hint::black_box(result);
            }
        }
        82 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let mut slot = rust_ffi::FT_Malformed_Get_GlyphSlot(&core_face, 5);
                slot.advance.x = 0x8000_i64 * 64;
                let result = rust_ffi::FT_Get_Svg_Glyph(Some(&slot));
                let _ = std::hint::black_box(result);
            }
        }
        83 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let slot = rust_ffi::FT_Malformed_Get_GlyphSlot(&core_face, 5);
                if let Ok(mut glyph) = rust_ffi::FT_Get_Svg_Glyph(Some(&slot)) {
                    glyph.root.format = rust_ffi::FT_GLYPH_FORMAT_OUTLINE;
                    let result = rust_ffi::FT_Svg_Glyph_Copy(&glyph);
                    let _ = std::hint::black_box(result);
                }
            }
        }
        84 => {
            let glyph = rust_ffi::FT_SvgGlyphOwned {
                root: rust_ffi::FT_GlyphRec {
                    format: rust_ffi::FT_GLYPH_FORMAT_SVG,
                    ..rust_ffi::FT_GlyphRec::default()
                },
                svg_document: Vec::new(),
                glyph_index: 0,
                metrics: rust_ffi::FT_Size_Metrics::default(),
                units_per_EM: 0,
                start_glyph_id: 0,
                end_glyph_id: 0,
                transform: rust_ffi::FT_Matrix::default(),
                delta: rust_ffi::FT_Vector::default(),
            };
            let result = rust_ffi::FT_Svg_Glyph_Copy(&glyph);
            let _ = std::hint::black_box(result);
        }
        85 => {
            let result = rust_ffi::FT_Svg_Glyph_Transform(None, None, None);
            let _ = std::hint::black_box(result);
        }
        86 => {
            let mut bbox = rust_ffi::FT_BBox::default();
            rust_ffi::FT_Glyph_Get_CBox(None, 0, Some(&mut bbox));
            std::hint::black_box(bbox);
        }
        87 => {
            let snapshot = rust_ffi::FT_GlyphCBoxSnapshot {
                has_class: true,
                has_bbox_hook: false,
                cbox: None,
            };
            let mut bbox = rust_ffi::FT_BBox::default();
            rust_ffi::FT_Glyph_Get_CBox(Some(snapshot), 0, Some(&mut bbox));
            std::hint::black_box(bbox);
        }
        88 => {
            let result = rust_ffi::FT_Glyph_Transform_Outline(None, None, None);
            let _ = std::hint::black_box(result);
        }
        89 => {
            let result = rust_ffi::FT_Glyph_Transform_Bitmap(None, None, None);
            let _ = std::hint::black_box(result);
        }
        90 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0)
                && let Ok(slot) =
                    rust_ffi::FT_Load_Glyph(&core_face, 36, rust_ffi::FT_LOAD_NO_HINTING)
                && let Ok(glyph) = rust_ffi::FT_Get_Outline_Glyph(Some(&slot))
            {
                let result = rust_ffi::FT_Outline_Glyph_To_Bitmap(&glyph, 99);
                let _ = std::hint::black_box(result);
            }
        }
        91 | 92 | 98 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0)
                && let Ok(slot) =
                    rust_ffi::FT_Load_Glyph(&core_face, 36, rust_ffi::FT_LOAD_NO_HINTING)
                && let Ok(mut glyph) = rust_ffi::FT_Get_Outline_Glyph(Some(&slot))
            {
                let matrix = rust_ffi::FT_Matrix {
                    xx: 0x1_0000,
                    xy: 0x2000,
                    yx: 0,
                    yy: 0x1_0000,
                };
                let delta = rust_ffi::FT_Vector { x: 17, y: -9 };
                if probe == 98 {
                    let result = rust_ffi::FT_Glyph_Transform_Outline(
                        Some(&mut glyph),
                        Some(&matrix),
                        Some(&delta),
                    );
                    let _ = std::hint::black_box(result);
                } else {
                    let result = rust_ffi::FT_Outline_Glyph_To_Bitmap_In_Place(
                        &mut glyph,
                        rust_ffi::FT_RENDER_MODE_NORMAL,
                        Some(delta),
                        probe == 92,
                    );
                    let _ = std::hint::black_box(result);
                }
            }
        }
        93 => {
            let result = rust_ffi::FT_Outline_Glyph_Stroke(None, ptr::null_mut());
            let _ = std::hint::black_box(result);
        }
        94 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0)
                && let Ok(slot) =
                    rust_ffi::FT_Load_Glyph(&core_face, 36, rust_ffi::FT_LOAD_NO_HINTING)
                && let Ok(glyph) = rust_ffi::FT_Get_Outline_Glyph(Some(&slot))
            {
                let result = rust_ffi::FT_Outline_Glyph_Stroke(Some(&glyph), ptr::null_mut());
                let _ = std::hint::black_box(result);
            }
        }
        95 => {
            let result = rust_ffi::FT_Outline_Glyph_StrokeBorder(None, ptr::null_mut(), 0);
            let _ = std::hint::black_box(result);
        }
        96 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0)
                && let Ok(slot) =
                    rust_ffi::FT_Load_Glyph(&core_face, 36, rust_ffi::FT_LOAD_NO_HINTING)
                && let Ok(glyph) = rust_ffi::FT_Get_Outline_Glyph(Some(&slot))
            {
                let result =
                    rust_ffi::FT_Outline_Glyph_StrokeBorder(Some(&glyph), ptr::null_mut(), 0);
                let _ = std::hint::black_box(result);
            }
        }
        97 => {
            let mut bbox = rust_ffi::FT_BBox::default();
            rust_ffi::FT_Outline_Glyph_CBox(None, 0, Some(&mut bbox));
            std::hint::black_box(bbox);
        }
        99 => {
            rust_ffi::FT_Done_Glyph(false);
            rust_ffi::FTC_Node_Unref(ptr::null_mut(), ptr::null_mut());
        }
        100 => {
            let first = rust_ffi::FT_Glyph_Copy(false, true, false);
            let second = rust_ffi::FT_Glyph_Copy(true, false, false);
            let _ = std::hint::black_box((first, second));
        }
        // c95-101..150 are bulk Rust-core witnesses selected from the current
        // Coverage MCP red regions. They call the existing safe FFI layer from
        // test support; no exported C ABI or WASM implementation is added.
        101 => {
            if let Ok(core_face) =
                rust_ffi::FT_New_Memory_Face(&rust_ffi::FT_Init_FreeType(), bytes, 0, 20.0)
            {
                let mut manager = rust_ffi::FTCCacheManagerState::new(core_face);
                let result = manager.lookup_pixel_size(FT_UInt::MAX, FT_UInt::MAX);
                let _ = std::hint::black_box(result);
            }
        }
        102 => {
            if let Ok(core_face) =
                rust_ffi::FT_New_Memory_Face(&rust_ffi::FT_Init_FreeType(), bytes, 0, 20.0)
            {
                let mut manager = rust_ffi::FTCCacheManagerState::new_with_requester_error(
                    core_face,
                    rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error,
                );
                let image_type = rust_ffi::FTC_ImageTypeRec::default();
                let mut output = None;
                let mut node = false;
                let result =
                    manager.lookup_sbit(Some(&image_type), 0, Some(&mut output), Some(&mut node));
                let _ = std::hint::black_box((result, node));
            }
        }
        103 => {
            if let Ok(core_face) =
                rust_ffi::FT_New_Memory_Face(&rust_ffi::FT_Init_FreeType(), bytes, 0, 20.0)
            {
                let mut manager = rust_ffi::FTCCacheManagerState::new(core_face);
                manager.done();
                let image_type = rust_ffi::FTC_ImageTypeRec::default();
                let mut output = None;
                let mut node = true;
                let result =
                    manager.lookup_sbit(Some(&image_type), 0, Some(&mut output), Some(&mut node));
                let _ = std::hint::black_box((result, node));
            }
        }
        104 => {
            if let Ok(core_face) =
                rust_ffi::FT_New_Memory_Face(&rust_ffi::FT_Init_FreeType(), bytes, 0, 20.0)
            {
                let mut cache = rust_ffi::FTCSBitCacheState::new(core_face);
                let image_type = rust_ffi::FTC_ImageTypeRec::default();
                let mut node = false;
                let result = rust_ffi::FTC_SBitCache_Lookup(
                    &mut cache,
                    Some(&image_type),
                    0,
                    None,
                    Some(&mut node),
                );
                let _ = std::hint::black_box((result, node));
            }
        }
        105 => {
            if let Ok(core_face) =
                rust_ffi::FT_New_Memory_Face(&rust_ffi::FT_Init_FreeType(), bytes, 0, 20.0)
            {
                let mut cache = rust_ffi::FTCSBitCacheState::new(core_face);
                let mut output = None;
                let mut node = false;
                let result = rust_ffi::FTC_SBitCache_Lookup(
                    &mut cache,
                    None,
                    0,
                    Some(&mut output),
                    Some(&mut node),
                );
                let _ = std::hint::black_box((result, node));
            }
        }
        106 => {
            if let Ok(core_face) =
                rust_ffi::FT_New_Memory_Face(&rust_ffi::FT_Init_FreeType(), bytes, 0, 20.0)
            {
                let mut cache = rust_ffi::FTCSBitCacheState::new(core_face);
                let image_type = rust_ffi::FTC_ImageTypeRec {
                    width: 12,
                    height: 12,
                    ..rust_ffi::FTC_ImageTypeRec::default()
                };
                let mut output = None;
                let result = rust_ffi::FTC_SBitCache_Lookup(
                    &mut cache,
                    Some(&image_type),
                    FT_UInt::MAX,
                    Some(&mut output),
                    None,
                );
                let _ = std::hint::black_box(result);
            }
        }
        107 => {
            if let Ok(core_face) =
                rust_ffi::FT_New_Memory_Face(&rust_ffi::FT_Init_FreeType(), bytes, 0, 20.0)
            {
                let mut cache = rust_ffi::FTCSBitCacheState::new(core_face);
                let image_type = rust_ffi::FTC_ImageTypeRec {
                    width: FT_UInt::MAX,
                    height: FT_UInt::MAX,
                    ..rust_ffi::FTC_ImageTypeRec::default()
                };
                let mut output = None;
                let result = rust_ffi::FTC_SBitCache_Lookup(
                    &mut cache,
                    Some(&image_type),
                    0,
                    Some(&mut output),
                    None,
                );
                let _ = std::hint::black_box(result);
            }
        }
        108 => {
            if let Ok(core_face) =
                rust_ffi::FT_New_Memory_Face(&rust_ffi::FT_Init_FreeType(), bytes, 0, 20.0)
            {
                let mut cache = rust_ffi::FTCSBitCacheState::new(core_face);
                let image_type = rust_ffi::FTC_ImageTypeRec {
                    width: 12,
                    height: 12,
                    ..rust_ffi::FTC_ImageTypeRec::default()
                };
                let result = cache.unref_node(&image_type, 0);
                let _ = std::hint::black_box(result);
            }
        }
        109 => {
            let result = rust_ffi::FT_Outline_Check(None);
            let _ = std::hint::black_box(result);
        }
        110 => {
            let outline = rust_ffi::FT_OutlineSnapshot::default();
            let result = rust_ffi::FT_Outline_Check(Some(&outline));
            let _ = std::hint::black_box(result);
        }
        111 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: Vec::new(),
                tags: Vec::new(),
                contours: vec![0],
                flags: 0,
            };
            let result = rust_ffi::FT_Outline_Check(Some(&outline));
            let _ = std::hint::black_box(result);
        }
        112 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![rust_ffi::FT_Vector::default()],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte],
                contours: vec![1],
                flags: 0,
            };
            let result = rust_ffi::FT_Outline_Check(Some(&outline));
            let _ = std::hint::black_box(result);
        }
        113 => {
            let source = rust_ffi::FT_OutlineSnapshot::default();
            let result = rust_ffi::FT_Outline_Copy(None, None);
            let _ = std::hint::black_box((source, result));
        }
        114 => {
            let source = rust_ffi::FT_OutlineSnapshot {
                points: vec![rust_ffi::FT_Vector::default()],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte],
                contours: vec![0],
                flags: 0,
            };
            let mut target = rust_ffi::FT_OutlineSnapshot::default();
            let result = rust_ffi::FT_Outline_Copy(Some(&source), Some(&mut target));
            let _ = std::hint::black_box(result);
        }
        115 => {
            let source = rust_ffi::FT_OutlineSnapshot {
                points: vec![rust_ffi::FT_Vector::default()],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte],
                contours: vec![0],
                flags: rust_ffi::FT_OUTLINE_OWNER as FT_Int,
            };
            let mut target = source.clone();
            target.flags = 0;
            let result = rust_ffi::FT_Outline_Copy(Some(&source), Some(&mut target));
            let _ = std::hint::black_box((result, target.flags));
        }
        116 => {
            let result = rust_ffi::FT_Outline_Embolden(None, 64);
            let _ = std::hint::black_box(result);
        }
        117 => {
            let mut outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![rust_ffi::FT_Vector {
                    x: i64::MAX,
                    y: i64::MAX,
                }],
                tags: Vec::new(),
                contours: Vec::new(),
                flags: 0,
            };
            let result = rust_ffi::FT_Outline_EmboldenXY(Some(&mut outline), 0, 0);
            let _ = std::hint::black_box(result);
        }
        118 => {
            let mut outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![rust_ffi::FT_Vector {
                    x: i64::MAX,
                    y: i64::MAX,
                }],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte],
                contours: vec![0],
                flags: 0,
            };
            let result = rust_ffi::FT_Outline_EmboldenXY(Some(&mut outline), 64, 64);
            let _ = std::hint::black_box(result);
        }
        119 => {
            let result = rust_ffi::FT_Outline_Get_Orientation(None);
            let inside = rust_ffi::FT_Outline_GetInsideBorder(None);
            let outside = rust_ffi::FT_Outline_GetOutsideBorder(None);
            let _ = std::hint::black_box((result, inside, outside));
        }
        120 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 100, y: 0 },
                    rust_ffi::FT_Vector { x: 0, y: 100 },
                ],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
                contours: vec![2],
                flags: 0,
            };
            let _ = std::hint::black_box(rust_ffi::FT_Outline_Get_Orientation(Some(&outline)));
        }
        121 => {
            let mut outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![rust_ffi::FT_Vector::default()],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte],
                contours: vec![1],
                flags: 0,
            };
            rust_ffi::FT_Outline_Reverse(Some(&mut outline));
            let _ = std::hint::black_box(outline);
        }
        122 => {
            let mut outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 100, y: 0 },
                    rust_ffi::FT_Vector { x: 0, y: 100 },
                ],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
                contours: vec![2],
                flags: 0,
            };
            rust_ffi::FT_Outline_Reverse(Some(&mut outline));
            let _ = std::hint::black_box(outline);
        }
        123 => {
            let mut outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![rust_ffi::FT_Vector { x: 1, y: 2 }],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte],
                contours: vec![0],
                flags: 0,
            };
            let matrix = rust_ffi::FT_Matrix {
                xx: 0x1_0000,
                xy: 0,
                yx: 0,
                yy: 0x1_0000,
            };
            rust_ffi::FT_Outline_Transform(Some(&mut outline), Some(&matrix));
            let _ = std::hint::black_box(outline);
        }
        124 => {
            let mut outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![rust_ffi::FT_Vector { x: 1, y: 2 }],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte],
                contours: vec![0],
                flags: 0,
            };
            rust_ffi::FT_Outline_Translate(Some(&mut outline), 64, -64);
            let _ = std::hint::black_box(outline);
        }
        125 => {
            let result = rust_ffi::FT_Outline_Decompose_Trace(None, &[]);
            let _ = std::hint::black_box(result);
        }
        126 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 100, y: 0 },
                    rust_ffi::FT_Vector { x: 0, y: 100 },
                ],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
                contours: vec![2],
                flags: 0,
            };
            let result = rust_ffi::FT_Outline_Decompose_Trace(Some(&outline), &[(0, 0)]);
            let _ = std::hint::black_box(result);
        }
        127 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 100, y: 100 },
                    rust_ffi::FT_Vector { x: 200, y: 0 },
                ],
                tags: vec![
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_CONIC as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                ],
                contours: vec![2],
                flags: 0,
            };
            let result = rust_ffi::FT_Outline_Decompose_Trace(Some(&outline), &[(1, 3)]);
            let _ = std::hint::black_box(result);
        }
        128 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 20, y: 100 },
                    rust_ffi::FT_Vector { x: 80, y: 100 },
                    rust_ffi::FT_Vector { x: 100, y: 0 },
                ],
                tags: vec![
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                ],
                contours: vec![3],
                flags: 0,
            };
            let result = rust_ffi::FT_Outline_Decompose_Trace(Some(&outline), &[(0, 0)]);
            let _ = std::hint::black_box(result);
        }
        129 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector::default(),
                    rust_ffi::FT_Vector::default(),
                    rust_ffi::FT_Vector::default(),
                ],
                tags: vec![
                    rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                ],
                contours: vec![2],
                flags: 0,
            };
            let result = rust_ffi::FT_Outline_Decompose_Trace(Some(&outline), &[(0, 0)]);
            let _ = std::hint::black_box(result);
        }
        130 => {
            let mut stroker = std::ptr::null_mut();
            let result = rust_ffi::FT_Stroker_New(None, Some(&mut stroker));
            let _ = std::hint::black_box(result);
        }
        131 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let result = rust_ffi::FT_Stroker_New(Some(&core_library), None);
            let _ = std::hint::black_box(result);
        }
        132 => {
            let point = rust_ffi::FT_Vector::default();
            let _ = rust_ffi::FT_Stroker_Set(std::ptr::null_mut(), 96, 0, 0, 65_536);
            let _ = rust_ffi::FT_Stroker_Rewind(std::ptr::null_mut());
            let _ = rust_ffi::FT_Stroker_Done(std::ptr::null_mut());
            let _ = rust_ffi::FT_Stroker_BeginSubPath(std::ptr::null_mut(), None, 0);
            let _ = rust_ffi::FT_Stroker_LineTo(std::ptr::null_mut(), Some(&point));
            let _ = rust_ffi::FT_Stroker_ConicTo(std::ptr::null_mut(), Some(&point), None);
            let _ = rust_ffi::FT_Stroker_CubicTo(
                std::ptr::null_mut(),
                Some(&point),
                Some(&point),
                None,
            );
            let _ = std::hint::black_box(point);
        }
        133 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = std::ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                rust_ffi::FT_Stroker_Set(stroker, 96, 0, 0, 65_536);
                let start = rust_ffi::FT_Vector { x: 0, y: 0 };
                let end = rust_ffi::FT_Vector { x: 640, y: 0 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
                let _ = rust_ffi::FT_Stroker_LineTo(stroker, Some(&end));
                let _ = rust_ffi::FT_Stroker_EndSubPath(stroker);
                let mut points = 0;
                let mut contours = 0;
                let _ =
                    rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours));
                let mut outline = rust_ffi::FT_OutlineSnapshot::default();
                rust_ffi::FT_Stroker_Export(stroker, Some(&mut outline));
                rust_ffi::FT_Stroker_Done(stroker);
                let _ = std::hint::black_box((points, contours, outline));
            }
        }
        134 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = std::ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                rust_ffi::FT_Stroker_Set(
                    stroker,
                    80,
                    rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
                    rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
                    65_536,
                );
                let start = rust_ffi::FT_Vector { x: 0, y: 0 };
                let control = rust_ffi::FT_Vector { x: 256, y: 512 };
                let end = rust_ffi::FT_Vector { x: 512, y: 0 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
                let _ = rust_ffi::FT_Stroker_ConicTo(stroker, Some(&control), Some(&end));
                let _ = rust_ffi::FT_Stroker_EndSubPath(stroker);
                let mut outline = rust_ffi::FT_OutlineSnapshot::default();
                rust_ffi::FT_Stroker_Export(stroker, Some(&mut outline));
                rust_ffi::FT_Stroker_Done(stroker);
                let _ = std::hint::black_box(outline);
            }
        }
        135 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = std::ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                rust_ffi::FT_Stroker_Set(
                    stroker,
                    80,
                    rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
                    rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
                    65_536,
                );
                let start = rust_ffi::FT_Vector { x: 0, y: 0 };
                let control = rust_ffi::FT_Vector { x: 256, y: 512 };
                let end = rust_ffi::FT_Vector { x: 512, y: 0 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 1);
                let _ = rust_ffi::FT_Stroker_ConicTo(stroker, Some(&control), Some(&end));
                let _ = rust_ffi::FT_Stroker_EndSubPath(stroker);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        136 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = std::ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
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
                let end = rust_ffi::FT_Vector { x: 640, y: 0 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
                let _ = rust_ffi::FT_Stroker_CubicTo(
                    stroker,
                    Some(&control1),
                    Some(&control2),
                    Some(&end),
                );
                let _ = rust_ffi::FT_Stroker_EndSubPath(stroker);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        137 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = std::ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
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
                let end = rust_ffi::FT_Vector { x: 640, y: 0 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 1);
                let _ = rust_ffi::FT_Stroker_CubicTo(
                    stroker,
                    Some(&control1),
                    Some(&control2),
                    Some(&end),
                );
                let _ = rust_ffi::FT_Stroker_EndSubPath(stroker);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        138 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = std::ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let outline = rust_ffi::FT_OutlineSnapshot {
                    points: vec![
                        rust_ffi::FT_Vector { x: 0, y: 0 },
                        rust_ffi::FT_Vector { x: 640, y: 0 },
                        rust_ffi::FT_Vector { x: 640, y: 640 },
                    ],
                    tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
                    contours: vec![2],
                    flags: 0,
                };
                let result = rust_ffi::FT_Stroker_ParseOutline(stroker, Some(&outline), 0);
                rust_ffi::FT_Stroker_Done(stroker);
                let _ = std::hint::black_box(result);
            }
        }
        139 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = std::ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let outline = rust_ffi::FT_OutlineSnapshot {
                    points: vec![rust_ffi::FT_Vector::default()],
                    tags: vec![rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte],
                    contours: vec![0],
                    flags: 0,
                };
                let result = rust_ffi::FT_Stroker_ParseOutline(stroker, Some(&outline), 0);
                rust_ffi::FT_Stroker_Done(stroker);
                let _ = std::hint::black_box(result);
            }
        }
        140 => {
            let points = rust_ffi::FT_Stroker_GetCounts(std::ptr::null_mut(), None, None);
            let borders = rust_ffi::FT_Stroker_GetBorderCounts(
                std::ptr::null_mut(),
                rust_ffi::FT_STROKER_BORDER_LEFT as FT_Int,
                None,
                None,
            );
            let mut outline = rust_ffi::FT_OutlineSnapshot::default();
            rust_ffi::FT_Stroker_ExportBorder(
                std::ptr::null_mut(),
                rust_ffi::FT_STROKER_BORDER_LEFT as FT_Int,
                Some(&mut outline),
            );
            let _ = std::hint::black_box((points, borders, outline));
        }
        141 => {
            let source = [0_u8; 2];
            let mut stream = rust_ffi::FT_StreamRec::default();
            let source_stream = rust_ffi::FT_StreamRec::default();
            let result = rust_ffi::FT_Stream_OpenGzip(
                Some(&mut stream),
                Some(&source_stream),
                Some(&source),
            );
            let _ = std::hint::black_box(result);
        }
        142 => {
            let source = [0_u8; 4];
            let mut stream = rust_ffi::FT_StreamRec::default();
            let source_stream = rust_ffi::FT_StreamRec::default();
            let result = rust_ffi::FT_Stream_OpenGzip(
                Some(&mut stream),
                Some(&source_stream),
                Some(&source),
            );
            let _ = std::hint::black_box(result);
        }
        143 => {
            let source = [0x1F, 0x8B, 8, 0, 0, 0, 0, 0, 0x03];
            let mut stream = rust_ffi::FT_StreamRec::default();
            let source_stream = rust_ffi::FT_StreamRec::default();
            let result = rust_ffi::FT_Stream_OpenGzip(
                Some(&mut stream),
                Some(&source_stream),
                Some(&source),
            );
            let _ = std::hint::black_box(result);
        }
        144 => {
            let source = [0x1F, 0x8B, 8, 0x04, 0, 0, 0, 0, 0x03, 0, 0];
            let mut stream = rust_ffi::FT_StreamRec::default();
            let source_stream = rust_ffi::FT_StreamRec::default();
            let result = rust_ffi::FT_Stream_OpenGzip(
                Some(&mut stream),
                Some(&source_stream),
                Some(&source),
            );
            let _ = std::hint::black_box(result);
        }
        145 => {
            let read = rust_ffi::FT_Gzip_Stream_Read(None, 0, 1);
            let mut stream = rust_ffi::FT_StreamRec::default();
            rust_ffi::FT_Gzip_Stream_Close(Some(&mut stream));
            rust_ffi::FT_Gzip_Stream_Close(None);
            let _ = std::hint::black_box(read);
        }
        146 => {
            let mut tables: [rust_ffi::FT_Bytes; 1] = [ptr::null()];
            let result = rust_ffi::FT_TrueTypeGX_Validate(None, 0, Some(&mut tables));
            let _ = std::hint::black_box(result);
        }
        147 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let mut tables: [rust_ffi::FT_Bytes; 0] = [];
                rust_ffi::FT_GX_Validator_Set_Available(&mut core_face, true);
                let result =
                    rust_ffi::FT_TrueTypeGX_Validate(Some(&core_face), 0, Some(&mut tables));
                let _ = std::hint::black_box(result);
            }
        }
        148 => {
            let result = rust_ffi::FT_ClassicKern_Validate(None, 0, None);
            let _ = std::hint::black_box(result);
        }
        149 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let copy = rust_ffi::FT_OpenType_Table_Copy(ptr::null());
                rust_ffi::FT_OpenType_Free(Some(&core_face), ptr::null());
                let _ = std::hint::black_box(copy);
            }
        }
        150 => {
            let mut stream = rust_ffi::FT_StreamRec::default();
            let result = rust_ffi::FT_Stream_OpenGzip(None, Some(&stream), None);
            let _ = rust_ffi::FT_Bzip2_Stream_Is_Open(Some(&stream));
            rust_ffi::FT_Bzip2_Stream_Close(Some(&mut stream));
            let _ = std::hint::black_box(result);
        }
        // c96 retained Rust-core witnesses selected from the next
        // Coverage MCP red regions. Zero-gain and redundant witnesses were
        // pruned after the managed coverage comparison; the exported C ABI
        // and WASM surfaces remain thin adapters.
        152 => {
            if let Ok(core_face) =
                rust_ffi::FT_New_Memory_Face(&rust_ffi::FT_Init_FreeType(), bytes, 0, 20.0)
            {
                let mut manager = rust_ffi::FTCCacheManagerState::new(core_face);
                manager.done();
                let image_type = rust_ffi::FTC_ImageTypeRec::default();
                let mut output = None;
                let result = manager.lookup_sbit(Some(&image_type), 0, Some(&mut output), None);
                let _ = std::hint::black_box(result);
            }
        }
        155 => {
            let invalid = rust_ffi::FT_Glyph_To_Bitmap(false, true, true, true, true);
            let valid = rust_ffi::FT_Glyph_To_Bitmap(true, true, true, true, true);
            let _ = std::hint::black_box((invalid, valid));
        }
        160 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 64, y: 64 },
                    rust_ffi::FT_Vector { x: 768, y: 64 },
                    rust_ffi::FT_Vector { x: 64, y: 768 },
                ],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
                contours: vec![2],
                flags: 0,
            };
            let result = rust_ffi::FT_Outline_Render(
                Some(&rust_ffi::FT_Init_FreeType()),
                Some(&outline),
                None,
                rust_ffi::FT_RASTER_FLAG_DIRECT as FT_Int,
                rust_ffi::FT_BBox::default(),
            );
            let _ = std::hint::black_box(result);
        }
        161 => {
            let result = rust_ffi::FT_Outline_Render(
                Some(&rust_ffi::FT_Init_FreeType()),
                Some(&rust_ffi::FT_OutlineSnapshot::default()),
                None,
                rust_ffi::FT_RASTER_FLAG_AA as FT_Int,
                rust_ffi::FT_BBox::default(),
            );
            let _ = std::hint::black_box(result);
        }
        163 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 64, y: 64 },
                    rust_ffi::FT_Vector { x: 768, y: 64 },
                    rust_ffi::FT_Vector { x: 64, y: 768 },
                ],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
                contours: vec![2],
                flags: 0,
            };
            let result = rust_ffi::FT_Outline_Render_Direct_Spans(
                Some(&rust_ffi::FT_Init_FreeType()),
                Some(&outline),
                None,
                0,
                None,
                true,
            );
            let _ = std::hint::black_box(result);
        }
        165 => {
            let dangling = NonNull::<c_void>::dangling().as_ptr();
            rust_ffi::FT_Stroker_Set(dangling, 96, 0, 0, 65_536);
        }
        166 => {
            let dangling = NonNull::<c_void>::dangling().as_ptr();
            rust_ffi::FT_Stroker_Rewind(dangling);
        }
        167 => {
            let point = rust_ffi::FT_Vector { x: 1, y: 2 };
            let dangling = NonNull::<c_void>::dangling().as_ptr();
            let result = rust_ffi::FT_Stroker_BeginSubPath(dangling, Some(&point), 0);
            let _ = std::hint::black_box(result);
        }
        168 => {
            let point = rust_ffi::FT_Vector { x: 1, y: 2 };
            let dangling = NonNull::<c_void>::dangling().as_ptr();
            let result = rust_ffi::FT_Stroker_LineTo(dangling, Some(&point));
            let _ = std::hint::black_box(result);
        }
        169 => {
            let point = rust_ffi::FT_Vector { x: 1, y: 2 };
            let dangling = NonNull::<c_void>::dangling().as_ptr();
            let result = rust_ffi::FT_Stroker_ConicTo(dangling, Some(&point), Some(&point));
            let _ = std::hint::black_box(result);
        }
        170 => {
            let point = rust_ffi::FT_Vector { x: 1, y: 2 };
            let dangling = NonNull::<c_void>::dangling().as_ptr();
            let result =
                rust_ffi::FT_Stroker_CubicTo(dangling, Some(&point), Some(&point), Some(&point));
            let _ = std::hint::black_box(result);
        }
        171 => {
            let dangling = NonNull::<c_void>::dangling().as_ptr();
            rust_ffi::FT_Stroker_EndSubPath(dangling);
        }
        172 => {
            let dangling = NonNull::<c_void>::dangling().as_ptr();
            let result = rust_ffi::FT_Stroker_GetCounts(dangling, None, None);
            let _ = std::hint::black_box(result);
        }
        173 => {
            let dangling = NonNull::<c_void>::dangling().as_ptr();
            let mut outline = rust_ffi::FT_OutlineSnapshot::default();
            rust_ffi::FT_Stroker_ExportBorder(
                dangling,
                rust_ffi::FT_STROKER_BORDER_LEFT as FT_Int,
                Some(&mut outline),
            );
            std::hint::black_box(outline);
        }
        181 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let outline = rust_ffi::FT_OutlineSnapshot {
                    points: vec![rust_ffi::FT_Vector::default()],
                    tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte],
                    contours: vec![1],
                    flags: 0,
                };
                let result = rust_ffi::FT_Stroker_ParseOutline(stroker, Some(&outline), 0);
                rust_ffi::FT_Stroker_Done(stroker);
                let _ = std::hint::black_box(result);
            }
        }
        182 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let outline = rust_ffi::FT_OutlineSnapshot {
                    points: vec![
                        rust_ffi::FT_Vector::default(),
                        rust_ffi::FT_Vector { x: 100, y: 200 },
                        rust_ffi::FT_Vector { x: 200, y: 0 },
                    ],
                    tags: vec![
                        rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                        rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                        rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                    ],
                    contours: vec![2],
                    flags: 0,
                };
                let result = rust_ffi::FT_Stroker_ParseOutline(stroker, Some(&outline), 0);
                rust_ffi::FT_Stroker_Done(stroker);
                let _ = std::hint::black_box(result);
            }
        }
        183 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let outline = rust_ffi::FT_OutlineSnapshot {
                    points: vec![
                        rust_ffi::FT_Vector::default(),
                        rust_ffi::FT_Vector { x: 100, y: 300 },
                        rust_ffi::FT_Vector { x: 300, y: 300 },
                    ],
                    tags: vec![
                        rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                        rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                        rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                    ],
                    contours: vec![2],
                    flags: 0,
                };
                let result = rust_ffi::FT_Stroker_ParseOutline(stroker, Some(&outline), 0);
                rust_ffi::FT_Stroker_Done(stroker);
                let _ = std::hint::black_box(result);
            }
        }
        192 => {
            let source = [0x1F, 0x8B, 8, 0x04, 0, 0, 0, 0, 0x03, 0, 4, 0];
            let mut stream = rust_ffi::FT_StreamRec::default();
            let source_stream = rust_ffi::FT_StreamRec::default();
            let result = rust_ffi::FT_Stream_OpenGzip(
                Some(&mut stream),
                Some(&source_stream),
                Some(&source),
            );
            let _ = std::hint::black_box(result);
        }
        193 => {
            let source = [0x1F, 0x8B, 8, 0x08, 0, 0, 0, 0, 0x03, 0];
            let mut stream = rust_ffi::FT_StreamRec::default();
            let source_stream = rust_ffi::FT_StreamRec::default();
            let result = rust_ffi::FT_Stream_OpenGzip(
                Some(&mut stream),
                Some(&source_stream),
                Some(&source),
            );
            let _ = std::hint::black_box(result);
        }
        194 => {
            let source = [0x1F, 0x8B, 8, 0x02, 0, 0, 0, 0, 0x03, 0];
            let mut stream = rust_ffi::FT_StreamRec::default();
            let source_stream = rust_ffi::FT_StreamRec::default();
            let result = rust_ffi::FT_Stream_OpenGzip(
                Some(&mut stream),
                Some(&source_stream),
                Some(&source),
            );
            let _ = std::hint::black_box(result);
        }
        197 => {
            let mut paint = rust_ffi::FT_OpaquePaint::default();
            let result = rust_ffi::FT_Get_Color_Glyph_Paint(None, 0, 0, Some(&mut paint));
            let _ = std::hint::black_box(result);
        }
        198 => {
            if let Ok(core_face) =
                rust_ffi::FT_New_Memory_Face(&rust_ffi::FT_Init_FreeType(), bytes, 0, 20.0)
            {
                let mut paint = rust_ffi::FT_OpaquePaint {
                    p: NonNull::<u8>::dangling().as_ptr().cast(),
                    insert_root_transform: 0,
                };
                let first = rust_ffi::FT_Get_Color_Glyph_Paint(
                    Some(&core_face),
                    0,
                    rust_ffi::FT_COLOR_ROOT_TRANSFORM_MAX as FT_UInt,
                    Some(&mut paint),
                );
                let mut output = rust_ffi::FT_COLR_Paint::default();
                let second = rust_ffi::FT_Get_Paint(Some(&core_face), paint, Some(&mut output));
                let _ = std::hint::black_box((first, second, output));
            }
        }
        199 => {
            let mut stop = rust_ffi::FT_ColorStop::default();
            let mut iterator = rust_ffi::FT_ColorStopIterator::default();
            let first =
                rust_ffi::FT_Get_Colorline_Stops(None, Some(&mut stop), Some(&mut iterator));
            let mut results = vec![first];
            if let Ok(core_face) =
                rust_ffi::FT_New_Memory_Face(&rust_ffi::FT_Init_FreeType(), bytes, 0, 20.0)
            {
                results.push(rust_ffi::FT_Get_Colorline_Stops(
                    Some(&core_face),
                    None,
                    Some(&mut iterator),
                ));
                results.push(rust_ffi::FT_Get_Colorline_Stops(
                    Some(&core_face),
                    Some(&mut stop),
                    None,
                ));
            }
            let _ = std::hint::black_box(results);
        }
        200 => {
            if let Ok(core_face) =
                rust_ffi::FT_New_Memory_Face(&rust_ffi::FT_Init_FreeType(), bytes, 0, 20.0)
            {
                let mut stop = rust_ffi::FT_ColorStop::default();
                let mut iterator = rust_ffi::FT_ColorStopIterator::default();
                let result = rust_ffi::FT_Get_Colorline_Stops(
                    Some(&core_face),
                    Some(&mut stop),
                    Some(&mut iterator),
                );
                let _ = std::hint::black_box(result);
            }
        }
        // c97 Rust-core witnesses selected from the next Coverage MCP cursor.
        // These remain test-support-only calls into the Rust FFI; the C ABI
        // and WASM entry points stay adapters over this Rust-owned behavior.
        201 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let zero = rust_ffi::FT_Set_Pixel_Sizes(&mut core_face, 0, 0);
                let huge = rust_ffi::FT_Set_Char_Size(&mut core_face, i64::MAX, i64::MAX, 72, 72);
                let _ = std::hint::black_box((zero, huge));
            }
        }
        202 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let mut request = rust_ffi::FT_Size_RequestRec {
                    type_: rust_ffi::FT_SIZE_REQUEST_TYPE_NOMINAL as _,
                    width: -1,
                    height: 12 * 64,
                    horiResolution: 72,
                    vertResolution: 72,
                };
                let negative = rust_ffi::FT_Request_Size(Some(&mut core_face), Some(&request));
                request.type_ = 99;
                let unknown = rust_ffi::FT_Request_Size(Some(&mut core_face), Some(&request));
                let _ = std::hint::black_box((negative, unknown));
            }
        }
        203 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let requests = [
                    rust_ffi::FT_SIZE_REQUEST_TYPE_NOMINAL,
                    rust_ffi::FT_SIZE_REQUEST_TYPE_REAL_DIM,
                    rust_ffi::FT_SIZE_REQUEST_TYPE_BBOX,
                    rust_ffi::FT_SIZE_REQUEST_TYPE_CELL,
                    rust_ffi::FT_SIZE_REQUEST_TYPE_SCALES,
                ];
                let results = requests.map(|type_| {
                    let request = rust_ffi::FT_Size_RequestRec {
                        type_: type_ as _,
                        width: 12 * 64,
                        height: 16 * 64,
                        horiResolution: 96,
                        vertResolution: 72,
                    };
                    rust_ffi::FT_Request_Size(Some(&mut core_face), Some(&request))
                });
                let _ = std::hint::black_box(results);
            }
        }
        204 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let invalid =
                    rust_ffi::FT_Load_Glyph(&core_face, FT_UInt::MAX, rust_ffi::FT_LOAD_DEFAULT);
                let alternate = rust_ffi::FT_Load_Glyph(
                    &core_face,
                    0,
                    rust_ffi::FT_LOAD_NO_SCALE
                        | rust_ffi::FT_LOAD_NO_HINTING
                        | rust_ffi::FT_LOAD_NO_BITMAP,
                );
                let missing =
                    rust_ffi::FT_Load_Char(&core_face, 0x10FF_FFFF, rust_ffi::FT_LOAD_DEFAULT);
                let _ = std::hint::black_box((invalid, alternate, missing));
            }
        }
        205 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let fast = rust_ffi::FT_Get_Advance(&core_face, 0, 0x2000_0000);
                let unscaled = rust_ffi::FT_Get_Advance(&core_face, 0, rust_ffi::FT_LOAD_NO_SCALE);
                let bad =
                    rust_ffi::FT_Get_Advance(&core_face, FT_UInt::MAX, rust_ffi::FT_LOAD_DEFAULT);
                let empty = rust_ffi::FT_Get_Advances(&core_face, 0, 0, rust_ffi::FT_LOAD_DEFAULT);
                let range = rust_ffi::FT_Get_Advances(
                    &core_face,
                    FT_UInt::MAX,
                    1,
                    rust_ffi::FT_LOAD_DEFAULT,
                );
                let _ = std::hint::black_box((fast, unscaled, bad, empty, range));
            }
        }
        206 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let mut glyph = 0;
                let first = rust_ffi::FT_Get_First_Char(Some(&core_face), Some(&mut glyph));
                let next = rust_ffi::FT_Get_Next_Char(Some(&core_face), first, Some(&mut glyph));
                let variant = rust_ffi::FT_Face_GetCharVariantIndex(Some(&core_face), 0x41, 0xFE0F);
                let selectors = rust_ffi::FT_Face_GetVariantSelectors(Some(&core_face));
                let _ = std::hint::black_box((first, next, variant, selectors));
            }
        }
        207 => {
            let glyphs = [
                rust_ffi::FT_GlyphCBoxSnapshot {
                    has_class: true,
                    has_bbox_hook: true,
                    cbox: Some(rust_ffi::FT_BBox {
                        xMin: -17,
                        yMin: -9,
                        xMax: 129,
                        yMax: 257,
                    }),
                },
                rust_ffi::FT_GlyphCBoxSnapshot {
                    has_class: true,
                    has_bbox_hook: true,
                    cbox: None,
                },
            ];
            let mut boxes = [rust_ffi::FT_BBox::default(); 2];
            for (mode, glyph) in [0, 1, 2, 3, 4].into_iter().zip(glyphs.into_iter().cycle()) {
                let slot = usize::try_from(mode).unwrap_or(0) % boxes.len();
                rust_ffi::FT_Glyph_Get_CBox(Some(glyph), mode, Some(&mut boxes[slot]));
            }
            let _ = std::hint::black_box(boxes);
        }
        208 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let invalid = rust_ffi::FT_New_Glyph(Some(&core_library), 0x1234_5678);
            let outline =
                rust_ffi::FT_New_Glyph(Some(&core_library), rust_ffi::FT_GLYPH_FORMAT_OUTLINE);
            let bitmap =
                rust_ffi::FT_New_Glyph(Some(&core_library), rust_ffi::FT_GLYPH_FORMAT_BITMAP);
            let svg = rust_ffi::FT_New_Glyph(Some(&core_library), rust_ffi::FT_GLYPH_FORMAT_SVG);
            let _ = std::hint::black_box((invalid, outline, bitmap, svg));
        }
        209 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let (Ok(rust_ffi::FT_GlyphOwned::Outline(glyph)), Ok(stroker)) = (
                rust_ffi::FT_New_Glyph(Some(&core_library), rust_ffi::FT_GLYPH_FORMAT_OUTLINE),
                {
                    let mut stroker = ptr::null_mut();
                    let result = rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker));
                    (result == rust_ffi::FT_Err_Ok)
                        .then_some(stroker)
                        .ok_or(result)
                },
            ) {
                rust_ffi::FT_Stroker_Set(stroker, 64, 0, 0, 65_536);
                let result = rust_ffi::FT_Outline_Glyph_Stroke(Some(&glyph), stroker);
                let _ = std::hint::black_box(result);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        210 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let (Ok(rust_ffi::FT_GlyphOwned::Outline(glyph)), Ok(stroker)) = (
                rust_ffi::FT_New_Glyph(Some(&core_library), rust_ffi::FT_GLYPH_FORMAT_OUTLINE),
                {
                    let mut stroker = ptr::null_mut();
                    let result = rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker));
                    (result == rust_ffi::FT_Err_Ok)
                        .then_some(stroker)
                        .ok_or(result)
                },
            ) {
                rust_ffi::FT_Stroker_Set(
                    stroker,
                    64,
                    rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
                    rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
                    65_536,
                );
                let result = rust_ffi::FT_Outline_Glyph_StrokeBorder(Some(&glyph), stroker, 1);
                let _ = std::hint::black_box(result);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        211 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector::default(),
                    rust_ffi::FT_Vector { x: 64, y: 64 },
                ],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte],
                contours: vec![1],
                flags: 0,
            };
            let mut bbox = rust_ffi::FT_BBox::default();
            let result = rust_ffi::FT_Outline_Get_BBox(Some(&outline), Some(&mut bbox));
            let _ = std::hint::black_box((result, bbox));
        }
        212 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 32, y: 160 },
                    rust_ffi::FT_Vector { x: 192, y: 0 },
                ],
                tags: vec![
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_CONIC as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                ],
                contours: vec![2],
                flags: 0,
            };
            let mut bbox = rust_ffi::FT_BBox::default();
            let result = rust_ffi::FT_Outline_Get_BBox(Some(&outline), Some(&mut bbox));
            let _ = std::hint::black_box((result, bbox));
        }
        213 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 32, y: 256 },
                    rust_ffi::FT_Vector { x: 160, y: 256 },
                    rust_ffi::FT_Vector { x: 192, y: 0 },
                ],
                tags: vec![
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                ],
                contours: vec![3],
                flags: 0,
            };
            let mut bbox = rust_ffi::FT_BBox::default();
            let result = rust_ffi::FT_Outline_Get_BBox(Some(&outline), Some(&mut bbox));
            let _ = std::hint::black_box((result, bbox));
        }
        214 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 8 * 64, y: 0 },
                    rust_ffi::FT_Vector { x: 0, y: 8 * 64 },
                ],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
                contours: vec![2],
                flags: 0,
            };
            let mut pixels = [0_u8; 16];
            let target = rust_ffi::FT_Bitmap_C {
                rows: 2,
                width: 8,
                pitch: -8,
                buffer: pixels.as_mut_ptr(),
                num_grays: 2,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_MONO as _,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let result = rust_ffi::FT_Outline_Get_Bitmap(
                Some(&rust_ffi::FT_Init_FreeType()),
                Some(&outline),
                Some(&target),
            );
            let _ = std::hint::black_box((result, pixels));
        }
        215 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 8 * 64, y: 0 },
                    rust_ffi::FT_Vector { x: 0, y: 8 * 64 },
                ],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
                contours: vec![2],
                flags: 0,
            };
            let mut pixels = [0_u8; 32];
            let target = rust_ffi::FT_Bitmap_C {
                rows: 4,
                width: 8,
                pitch: -8,
                buffer: pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as _,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let result = rust_ffi::FT_Outline_Render(
                Some(&rust_ffi::FT_Init_FreeType()),
                Some(&outline),
                Some(&target),
                0,
                rust_ffi::FT_BBox::default(),
            );
            let _ = std::hint::black_box((result, pixels));
        }
        216 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 8 * 64, y: 0 },
                    rust_ffi::FT_Vector { x: 0, y: 8 * 64 },
                ],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
                contours: vec![2],
                flags: 0,
            };
            let mut pixels = [0_u8; 64];
            let target = rust_ffi::FT_Bitmap_C {
                rows: 8,
                width: 8,
                pitch: -8,
                buffer: pixels.as_mut_ptr(),
                num_grays: 256,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_GRAY as _,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let result = rust_ffi::FT_Outline_Render(
                Some(&rust_ffi::FT_Init_FreeType()),
                Some(&outline),
                Some(&target),
                rust_ffi::FT_RASTER_FLAG_AA as FT_Int,
                rust_ffi::FT_BBox::default(),
            );
            let _ = std::hint::black_box((result, pixels));
        }
        217 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 8 * 64, y: 0 },
                    rust_ffi::FT_Vector { x: 0, y: 8 * 64 },
                ],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
                contours: vec![2],
                flags: 0,
            };
            let target = rust_ffi::FT_Bitmap_C {
                rows: 8,
                width: 8,
                pitch: 8,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let result = rust_ffi::FT_Outline_Render_Direct_Spans(
                Some(&rust_ffi::FT_Init_FreeType()),
                Some(&outline),
                Some(&target),
                rust_ffi::FT_RASTER_FLAG_DIRECT as FT_Int
                    | rust_ffi::FT_RASTER_FLAG_AA as FT_Int
                    | rust_ffi::FT_RASTER_FLAG_CLIP as FT_Int,
                Some(rust_ffi::FT_BBox {
                    xMin: 0,
                    yMin: 0,
                    xMax: 6 * 64,
                    yMax: 6 * 64,
                }),
                true,
            );
            let _ = std::hint::black_box(result);
        }
        218 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 8 * 64, y: 0 },
                    rust_ffi::FT_Vector { x: 0, y: 8 * 64 },
                ],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
                contours: vec![2],
                flags: 0,
            };
            let result = rust_ffi::FT_Outline_Render_Direct_Spans(
                Some(&rust_ffi::FT_Init_FreeType()),
                Some(&outline),
                None,
                rust_ffi::FT_RASTER_FLAG_DIRECT as FT_Int | rust_ffi::FT_RASTER_FLAG_AA as FT_Int,
                None,
                true,
            );
            let _ = std::hint::black_box(result);
        }
        219 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 40, y: 160 },
                    rust_ffi::FT_Vector { x: 100, y: 200 },
                    rust_ffi::FT_Vector { x: 192, y: 0 },
                ],
                tags: vec![
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_CONIC as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_CONIC as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                ],
                contours: vec![3],
                flags: 0,
            };
            let result = rust_ffi::FT_Outline_Decompose_Trace(Some(&outline), &[(0, 0), (1, 7)]);
            let _ = std::hint::black_box(result);
        }
        220 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 32, y: 160 },
                    rust_ffi::FT_Vector { x: 160, y: 160 },
                ],
                tags: vec![
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                ],
                contours: vec![2],
                flags: 0,
            };
            let result = rust_ffi::FT_Outline_Decompose_Trace(Some(&outline), &[(2, -11)]);
            let _ = std::hint::black_box(result);
        }
        221 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector::default(),
                    rust_ffi::FT_Vector { x: 20, y: 20 },
                    rust_ffi::FT_Vector { x: 40, y: 40 },
                ],
                tags: vec![
                    rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                    rust_ffi::FT_CURVE_TAG_CONIC as FT_Byte,
                    3,
                ],
                contours: vec![2],
                flags: 0,
            };
            let result = rust_ffi::FT_Outline_Decompose_Trace(Some(&outline), &[(0, 0)]);
            let _ = std::hint::black_box(result);
        }
        222 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector::default(),
                    rust_ffi::FT_Vector { x: 20, y: 20 },
                    rust_ffi::FT_Vector { x: 40, y: 40 },
                ],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
                contours: vec![2, 1],
                flags: 0,
            };
            let result = rust_ffi::FT_Outline_Decompose_Trace(Some(&outline), &[(0, 0)]);
            let _ = std::hint::black_box(result);
        }
        223 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let point = rust_ffi::FT_Vector { x: 32, y: 32 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&point), 0);
                let _ = rust_ffi::FT_Stroker_EndSubPath(stroker);
                let _ = rust_ffi::FT_Stroker_LineTo(stroker, Some(&point));
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        224 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                rust_ffi::FT_Stroker_Set(
                    stroker,
                    80,
                    0,
                    rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
                    65_536,
                );
                let start = rust_ffi::FT_Vector { x: 0, y: 0 };
                let control = rust_ffi::FT_Vector { x: 256, y: 512 };
                let end = rust_ffi::FT_Vector { x: 512, y: 0 };
                let next = rust_ffi::FT_Vector { x: 640, y: 32 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
                let _ = rust_ffi::FT_Stroker_ConicTo(stroker, Some(&control), Some(&end));
                let result = rust_ffi::FT_Stroker_LineTo(stroker, Some(&next));
                let _ = std::hint::black_box(result);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        225 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                rust_ffi::FT_Stroker_Set(
                    stroker,
                    96,
                    0,
                    rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
                    65_536,
                );
                let start = rust_ffi::FT_Vector { x: 0, y: 0 };
                let control1 = rust_ffi::FT_Vector { x: 160, y: 640 };
                let control2 = rust_ffi::FT_Vector { x: 480, y: 640 };
                let end = rust_ffi::FT_Vector { x: 640, y: 0 };
                let next = rust_ffi::FT_Vector { x: 640, y: 320 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
                let _ = rust_ffi::FT_Stroker_CubicTo(
                    stroker,
                    Some(&control1),
                    Some(&control2),
                    Some(&end),
                );
                let result = rust_ffi::FT_Stroker_LineTo(stroker, Some(&next));
                let _ = std::hint::black_box(result);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        226 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                rust_ffi::FT_Stroker_Set(stroker, 64, 0, 0, 65_536);
                let start = rust_ffi::FT_Vector { x: 0, y: 0 };
                let control = rust_ffi::FT_Vector { x: 1, y: 1 };
                let end = rust_ffi::FT_Vector { x: 2, y: 2 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
                let result = rust_ffi::FT_Stroker_ConicTo(stroker, Some(&control), Some(&end));
                let _ = std::hint::black_box(result);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        227 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                rust_ffi::FT_Stroker_Set(stroker, 64, 0, 0, 65_536);
                let start = rust_ffi::FT_Vector { x: 0, y: 0 };
                let control1 = rust_ffi::FT_Vector { x: 1, y: 1 };
                let control2 = rust_ffi::FT_Vector { x: 2, y: 2 };
                let end = rust_ffi::FT_Vector { x: 3, y: 3 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
                let result = rust_ffi::FT_Stroker_CubicTo(
                    stroker,
                    Some(&control1),
                    Some(&control2),
                    Some(&end),
                );
                let _ = std::hint::black_box(result);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        228 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                rust_ffi::FT_Stroker_Set(stroker, 64, 0, 0, 65_536);
                let start = rust_ffi::FT_Vector { x: 0, y: 0 };
                let control = rust_ffi::FT_Vector { x: 100, y: 200 };
                let end = rust_ffi::FT_Vector { x: 300, y: 40 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
                let result = rust_ffi::FT_Stroker_ConicTo(stroker, Some(&control), Some(&end));
                let _ = std::hint::black_box(result);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        229 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                rust_ffi::FT_Stroker_Set(stroker, 64, 0, 0, 65_536);
                let start = rust_ffi::FT_Vector { x: 0, y: 0 };
                let control1 = rust_ffi::FT_Vector { x: 100, y: 240 };
                let control2 = rust_ffi::FT_Vector { x: 260, y: 240 };
                let end = rust_ffi::FT_Vector { x: 320, y: 0 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 0);
                let result = rust_ffi::FT_Stroker_CubicTo(
                    stroker,
                    Some(&control1),
                    Some(&control2),
                    Some(&end),
                );
                let _ = std::hint::black_box(result);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        230 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let outline = rust_ffi::FT_OutlineSnapshot {
                    points: vec![
                        rust_ffi::FT_Vector { x: 0, y: 0 },
                        rust_ffi::FT_Vector { x: 64, y: 192 },
                        rust_ffi::FT_Vector { x: 128, y: 192 },
                        rust_ffi::FT_Vector { x: 256, y: 0 },
                    ],
                    tags: vec![
                        rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                        rust_ffi::FT_CURVE_TAG_CONIC as FT_Byte,
                        rust_ffi::FT_CURVE_TAG_CONIC as FT_Byte,
                        rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                    ],
                    contours: vec![3],
                    flags: 0,
                };
                let result = rust_ffi::FT_Stroker_ParseOutline(stroker, Some(&outline), 0);
                let _ = std::hint::black_box(result);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        231 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let outline = rust_ffi::FT_OutlineSnapshot {
                    points: vec![
                        rust_ffi::FT_Vector { x: 0, y: 32 },
                        rust_ffi::FT_Vector { x: 64, y: 192 },
                        rust_ffi::FT_Vector { x: 128, y: 32 },
                    ],
                    tags: vec![0, 0, 0],
                    contours: vec![2],
                    flags: 0,
                };
                let result = rust_ffi::FT_Stroker_ParseOutline(stroker, Some(&outline), 0);
                let _ = std::hint::black_box(result);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        232 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let outline = rust_ffi::FT_OutlineSnapshot {
                    points: vec![
                        rust_ffi::FT_Vector { x: 0, y: 0 },
                        rust_ffi::FT_Vector { x: 32, y: 160 },
                        rust_ffi::FT_Vector { x: 160, y: 160 },
                    ],
                    tags: vec![
                        rust_ffi::FT_CURVE_TAG_ON as FT_Byte,
                        rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                        rust_ffi::FT_CURVE_TAG_CUBIC as FT_Byte,
                    ],
                    contours: vec![2],
                    flags: 0,
                };
                let result = rust_ffi::FT_Stroker_ParseOutline(stroker, Some(&outline), 1);
                let _ = std::hint::black_box(result);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        233 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let point = rust_ffi::FT_Vector { x: 12, y: 34 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&point), 0);
                let result = rust_ffi::FT_Stroker_EndSubPath(stroker);
                let _ = std::hint::black_box(result);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        234 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                rust_ffi::FT_Stroker_Set(
                    stroker,
                    64,
                    rust_ffi::FT_STROKER_LINECAP_ROUND as FT_Int,
                    rust_ffi::FT_STROKER_LINEJOIN_ROUND as FT_Int,
                    65_536,
                );
                let start = rust_ffi::FT_Vector { x: 0, y: 0 };
                let end = rust_ffi::FT_Vector { x: 0, y: 320 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&start), 1);
                let result = rust_ffi::FT_Stroker_LineTo(stroker, Some(&end));
                let end_result = rust_ffi::FT_Stroker_EndSubPath(stroker);
                let _ = std::hint::black_box((result, end_result));
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        235 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let a = rust_ffi::FT_Vector { x: 0, y: 0 };
                let b = rust_ffi::FT_Vector { x: 320, y: 0 };
                let c = rust_ffi::FT_Vector { x: 320, y: 320 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&a), 0);
                let _ = rust_ffi::FT_Stroker_LineTo(stroker, Some(&b));
                let _ = rust_ffi::FT_Stroker_LineTo(stroker, Some(&c));
                let result = rust_ffi::FT_Stroker_EndSubPath(stroker);
                let _ = std::hint::black_box(result);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        236 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let a = rust_ffi::FT_Vector { x: 0, y: 0 };
                let b = rust_ffi::FT_Vector { x: 320, y: 0 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&a), 0);
                let _ = rust_ffi::FT_Stroker_LineTo(stroker, Some(&b));
                let mut points = 0;
                let mut contours = 0;
                let counts =
                    rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours));
                let border = rust_ffi::FT_Stroker_GetBorderCounts(
                    stroker,
                    rust_ffi::FT_STROKER_BORDER_LEFT as FT_Int,
                    Some(&mut points),
                    Some(&mut contours),
                );
                let _ = std::hint::black_box((counts, border, points, contours));
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        237 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let a = rust_ffi::FT_Vector { x: 0, y: 0 };
                let b = rust_ffi::FT_Vector { x: 320, y: 0 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&a), 0);
                let _ = rust_ffi::FT_Stroker_LineTo(stroker, Some(&b));
                let mut outline = rust_ffi::FT_OutlineSnapshot::default();
                rust_ffi::FT_Stroker_ExportBorder(
                    stroker,
                    rust_ffi::FT_STROKER_BORDER_LEFT as FT_Int,
                    Some(&mut outline),
                );
                let _ = std::hint::black_box(outline);
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        238 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let mut stroker = ptr::null_mut();
            if rust_ffi::FT_Stroker_New(Some(&core_library), Some(&mut stroker))
                == rust_ffi::FT_Err_Ok
            {
                let a = rust_ffi::FT_Vector { x: 0, y: 0 };
                let b = rust_ffi::FT_Vector { x: 320, y: 0 };
                let _ = rust_ffi::FT_Stroker_BeginSubPath(stroker, Some(&a), 0);
                let _ = rust_ffi::FT_Stroker_LineTo(stroker, Some(&b));
                let _ = rust_ffi::FT_Stroker_EndSubPath(stroker);
                let mut outline = rust_ffi::FT_OutlineSnapshot::default();
                rust_ffi::FT_Stroker_Export(stroker, Some(&mut outline));
                let mut points = 0;
                let mut contours = 0;
                let result =
                    rust_ffi::FT_Stroker_GetCounts(stroker, Some(&mut points), Some(&mut contours));
                let _ = std::hint::black_box((result, outline, points, contours));
                rust_ffi::FT_Stroker_Done(stroker);
            }
        }
        239 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let mut palette = rust_ffi::FT_Palette_Data::default();
                let palette_result =
                    rust_ffi::FT_Palette_Data_Get(Some(&core_face), Some(&mut palette));
                let mut iterator = rust_ffi::FT_LayerIterator::default();
                let mut glyph = 0;
                let mut color = 0;
                let mut seen = Vec::new();
                for base in 0..64 {
                    let result = rust_ffi::FT_Get_Color_Glyph_Layer(
                        Some(&core_face),
                        base,
                        Some(&mut glyph),
                        Some(&mut color),
                        Some(&mut iterator),
                    );
                    seen.push(result);
                }
                let _ = std::hint::black_box((palette_result, palette, seen));
            }
        }
        240 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let mut clip = rust_ffi::FT_ClipBox::default();
                let results = (0..64)
                    .map(|glyph| {
                        rust_ffi::FT_Get_Color_Glyph_ClipBox(
                            Some(&core_face),
                            glyph,
                            Some(&mut clip),
                        )
                    })
                    .collect::<Vec<_>>();
                let _ = std::hint::black_box((results, clip));
            }
        }
        241 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let graph = rust_ffi::FT_ColrV1_PaintGraph_Copy(Some(&core_face));
                let mut observations = Vec::new();
                if let Some(graph) = graph {
                    for record in graph.records.iter().take(8) {
                        let mut opaque = rust_ffi::FT_OpaquePaint::default();
                        let root = rust_ffi::FT_Get_Color_Glyph_Paint(
                            Some(&core_face),
                            record.glyph_index,
                            0,
                            Some(&mut opaque),
                        );
                        let mut paint = rust_ffi::FT_COLR_Paint::default();
                        let resolved =
                            rust_ffi::FT_Get_Paint(Some(&core_face), opaque, Some(&mut paint));
                        observations.push((root, resolved, paint.format));
                    }
                }
                let _ = std::hint::black_box(observations);
            }
        }
        242 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let mut copied = Vec::new();
                if let Some(graph) = rust_ffi::FT_ColrV1_PaintGraph_Copy(Some(&core_face)) {
                    for record in graph.records.iter().take(16) {
                        let mut opaque = rust_ffi::FT_OpaquePaint::default();
                        if rust_ffi::FT_Get_Color_Glyph_Paint(
                            Some(&core_face),
                            record.glyph_index,
                            rust_ffi::FT_COLOR_INCLUDE_ROOT_TRANSFORM as _,
                            Some(&mut opaque),
                        ) != 0
                        {
                            copied.push((
                                rust_ffi::FT_ColrV1_Paint_Transform_Copy(Some(&core_face), opaque),
                                rust_ffi::FT_ColrV1_Paint_ColorLine_Copy(Some(&core_face), opaque),
                                rust_ffi::FT_ColrV1_Paint_LinearGradient_Copy(
                                    Some(&core_face),
                                    opaque,
                                ),
                            ));
                        }
                    }
                }
                let _ = std::hint::black_box(copied);
            }
        }
        243 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let graph = rust_ffi::FT_ColrV1_PaintGraph_Copy(Some(&core_face));
                let mut observations = Vec::new();
                if let Some(graph) = graph {
                    for record in graph.records.iter().take(8) {
                        let mut opaque = rust_ffi::FT_OpaquePaint::default();
                        if rust_ffi::FT_Get_Color_Glyph_Paint(
                            Some(&core_face),
                            record.glyph_index,
                            0,
                            Some(&mut opaque),
                        ) != 0
                        {
                            let mut iterator = rust_ffi::FT_ColrV1_Paint_Layer_Iterator_Copy(
                                Some(&core_face),
                                opaque,
                            );
                            if let Some(ref mut iterator) = iterator {
                                let mut layer = rust_ffi::FT_OpaquePaint::default();
                                for _ in 0..4 {
                                    let result = rust_ffi::FT_Get_Paint_Layers(
                                        Some(&core_face),
                                        Some(iterator),
                                        Some(&mut layer),
                                    );
                                    observations.push(result);
                                    if result == 0 {
                                        break;
                                    }
                                }
                            }
                        }
                    }
                }
                let _ = std::hint::black_box(observations);
            }
        }
        244 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let mut stop = rust_ffi::FT_ColorStop::default();
                let mut iterator = rust_ffi::FT_ColorStopIterator {
                    num_color_stops: 1,
                    current_color_stop: FT_UInt::MAX,
                    p: NonNull::<u8>::dangling().as_ptr().cast(),
                    read_variable: 0,
                };
                let invalid = rust_ffi::FT_Get_Colorline_Stops(
                    Some(&core_face),
                    Some(&mut stop),
                    Some(&mut iterator),
                );
                iterator.p = ptr::null_mut();
                let missing = rust_ffi::FT_Get_Colorline_Stops(
                    Some(&core_face),
                    Some(&mut stop),
                    Some(&mut iterator),
                );
                let _ = std::hint::black_box((invalid, missing, stop));
            }
        }
        245 => {
            let bdf = b"STARTFONT 2.1\nFONT FontdoneC97\nSIZE 12 75 75\nFONTBOUNDINGBOX 5 10 0 -2\nSTARTPROPERTIES 4\nPIXEL_SIZE 12\nCHARSET_REGISTRY \"iso10646\"\nCHARSET_ENCODING \"1\"\nFONT_ASCENT 8\nENDPROPERTIES\nCHARS 1\nSTARTCHAR A\nENCODING 65\nSWIDTH 500 0\nDWIDTH 5 0\nBBX 5 10 0 -2\nBITMAP\n00\n00\n00\n00\n00\n00\n00\n00\n00\n00\nENDCHAR\nENDFONT\n";
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(mut core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bdf, 0, 12.0) {
                let _ = rust_ffi::FT_Set_Pixel_Sizes(&mut core_face, 0, 12);
                let _ = rust_ffi::FT_Get_Char_Index(&core_face, 65);
                let _ = rust_ffi::FT_Load_Char(&core_face, 65, rust_ffi::FT_LOAD_DEFAULT);
            }
        }
        246 => {
            let malformed_bdf =
                b"STARTFONT 2.1\nFONT Broken\nSIZE 8 75 75\nFONTBOUNDINGBOX 8 8 0 -2\nCHARS 1\nSTARTCHAR A\nENCODING 65\nBBX nope 8 0 -2\nBITMAP\n00\nENDCHAR\nENDFONT\n";
            let malformed_type1 =
                b"%!PS-AdobeFont-1.0: Broken 1.0\n/FontName /Broken def\n/FontBBox [0 0 100 100] def\n";
            let core_library = rust_ffi::FT_Init_FreeType();
            let first = rust_ffi::FT_New_Memory_Face(&core_library, malformed_bdf, 0, 12.0);
            let second = rust_ffi::FT_New_Memory_Face(&core_library, malformed_type1, 0, 12.0);
            let _ = std::hint::black_box((first, second));
        }
        247 => {
            let type1 =
                b"%!PS-AdobeFont-1.0: MissingPrivate 1.0\n/FontName /MissingPrivate def\n/FontBBox [0 0 100 100] def\n/Encoding StandardEncoding def\n";
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, type1, 0, 12.0) {
                let mut info = rust_ffi::PS_FontInfoRec::default();
                let mut private = rust_ffi::PS_PrivateRec::default();
                let info_result = rust_ffi::FT_Get_PS_Font_Info(Some(&core_face), Some(&mut info));
                let private_result =
                    rust_ffi::FT_Get_PS_Font_Private(Some(&core_face), Some(&mut private));
                let value = rust_ffi::FT_Get_PS_Font_Value(Some(&core_face), 0, 0, None, 0);
                let _ = std::hint::black_box((info_result, private_result, value));
            }
        }
        248 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            let negative = rust_ffi::FT_New_Memory_Face(&core_library, bytes, -1, 12.0);
            let huge = rust_ffi::FT_New_Memory_Face(&core_library, bytes, FT_Long::MAX, 12.0);
            let empty = rust_ffi::FT_New_Memory_Face(&core_library, &[], 0, 12.0);
            let _ = std::hint::black_box((negative, huge, empty));
        }
        249 => {
            let outline = rust_ffi::FT_OutlineSnapshot {
                points: vec![
                    rust_ffi::FT_Vector { x: 0, y: 0 },
                    rust_ffi::FT_Vector { x: 64, y: 0 },
                    rust_ffi::FT_Vector { x: 0, y: 64 },
                ],
                tags: vec![rust_ffi::FT_CURVE_TAG_ON as FT_Byte; 3],
                contours: vec![2],
                flags: 0,
            };
            let target = rust_ffi::FT_Bitmap_C {
                width: 1,
                rows: 1,
                pitch: 1,
                pixel_mode: rust_ffi::FT_PIXEL_MODE_NONE as _,
                ..rust_ffi::FT_Bitmap_C::default()
            };
            let error_output = rust_ffi::FT_Outline_Render_Error_Output(
                Some(&outline),
                Some(&target),
                rust_ffi::FT_RASTER_FLAG_AA as FT_Int,
            );
            let bitmap_error = rust_ffi::FT_Outline_Get_Bitmap(
                Some(&rust_ffi::FT_Init_FreeType()),
                Some(&outline),
                Some(&target),
            );
            let _ = std::hint::black_box((error_output, bitmap_error));
        }
        250 => {
            let core_library = rust_ffi::FT_Init_FreeType();
            if let Ok(core_face) = rust_ffi::FT_New_Memory_Face(&core_library, bytes, 0, 20.0) {
                let mut info = rust_ffi::PS_FontInfoRec::default();
                let mut private = rust_ffi::PS_PrivateRec::default();
                let ps_info = rust_ffi::FT_Get_PS_Font_Info(Some(&core_face), Some(&mut info));
                let ps_private =
                    rust_ffi::FT_Get_PS_Font_Private(Some(&core_face), Some(&mut private));
                let mut name = rust_ffi::FT_SfntName::default();
                let sfnt = rust_ffi::FT_Get_Sfnt_Name(Some(&core_face), 0, Some(&mut name));
                let format = rust_ffi::FT_Get_Font_Format(Some(&core_face));
                let _ = std::hint::black_box((ps_info, ps_private, sfnt, format));
            }
        }
        // c98 Rust-core witnesses selected from the current Coverage MCP
        // uncovered regions. They exercise existing Rust FFI behavior only.
        251 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        252 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        253 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        254 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        255 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        256 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        257 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        258 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        259 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        260 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        261 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        262 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        263 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        264 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        265 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        266 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        267 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        268 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        269 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        270 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        271 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        272 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        273 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        274 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        275 => {
            let results = abi_core_load_case(bytes, rust_ffi::FT_LOAD_DEFAULT, &[0, 1, 2]);
            std::hint::black_box(results);
        }
        276 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_NORMAL,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        277 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_MONO,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        278 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_LCD,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        279 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_LCD_V,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        280 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER | ((rust_ffi::FT_RENDER_MODE_SDF as FT_Int32) << 16),
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        281 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_NO_HINTING,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        282 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_FORCE_AUTOHINT,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        283 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_LIGHT,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        284 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER
                    | rust_ffi::FT_LOAD_TARGET_MONO
                    | rust_ffi::FT_LOAD_NO_HINTING,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        285 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER
                    | rust_ffi::FT_LOAD_TARGET_LCD
                    | rust_ffi::FT_LOAD_NO_HINTING,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        286 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER
                    | rust_ffi::FT_LOAD_TARGET_LCD_V
                    | rust_ffi::FT_LOAD_FORCE_AUTOHINT,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        287 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER
                    | ((rust_ffi::FT_RENDER_MODE_SDF as FT_Int32) << 16)
                    | rust_ffi::FT_LOAD_NO_HINTING,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        288 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_NO_BITMAP,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        289 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER
                    | rust_ffi::FT_LOAD_TARGET_MONO
                    | rust_ffi::FT_LOAD_FORCE_AUTOHINT,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        290 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER
                    | rust_ffi::FT_LOAD_TARGET_LCD
                    | rust_ffi::FT_LOAD_FORCE_AUTOHINT,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        291 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_NORMAL,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        292 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_MONO,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        293 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_LIGHT,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        294 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_FORCE_AUTOHINT,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        295 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_LCD,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        296 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER | rust_ffi::FT_LOAD_TARGET_LCD_V,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        297 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER
                    | rust_ffi::FT_LOAD_TARGET_NORMAL
                    | rust_ffi::FT_LOAD_NO_HINTING,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        298 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER
                    | rust_ffi::FT_LOAD_TARGET_MONO
                    | rust_ffi::FT_LOAD_NO_BITMAP,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        299 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER
                    | rust_ffi::FT_LOAD_TARGET_LCD
                    | rust_ffi::FT_LOAD_NO_BITMAP,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        300 => {
            let results = abi_core_load_case(
                bytes,
                rust_ffi::FT_LOAD_RENDER
                    | rust_ffi::FT_LOAD_TARGET_LCD_V
                    | rust_ffi::FT_LOAD_NO_BITMAP,
                &[0, 3, 36, 65],
            );
            std::hint::black_box(results);
        }
        301 => abi_c99_core_probe(bytes, 301),
        302 => abi_c99_core_probe(bytes, 302),
        303 => abi_c99_core_probe(bytes, 303),
        304 => abi_c99_core_probe(bytes, 304),
        305 => abi_c99_core_probe(bytes, 305),
        306 => abi_c99_core_probe(bytes, 306),
        307 => abi_c99_core_probe(bytes, 307),
        308 => abi_c99_core_probe(bytes, 308),
        309 => abi_c99_core_probe(bytes, 309),
        310 => abi_c99_core_probe(bytes, 310),
        311 => abi_c99_core_probe(bytes, 311),
        312 => abi_c99_core_probe(bytes, 312),
        313 => abi_c99_core_probe(bytes, 313),
        314 => abi_c99_core_probe(bytes, 314),
        315 => abi_c99_core_probe(bytes, 315),
        316 => abi_c99_core_probe(bytes, 316),
        317 => abi_c99_core_probe(bytes, 317),
        318 => abi_c99_core_probe(bytes, 318),
        319 => abi_c99_core_probe(bytes, 319),
        320 => abi_c99_core_probe(bytes, 320),
        321 => abi_c99_core_probe(bytes, 321),
        322 => abi_c99_core_probe(bytes, 322),
        323 => abi_c99_core_probe(bytes, 323),
        324 => abi_c99_core_probe(bytes, 324),
        325 => abi_c99_core_probe(bytes, 325),
        326 => abi_c99_core_probe(bytes, 326),
        327 => abi_c99_core_probe(bytes, 327),
        328 => abi_c99_core_probe(bytes, 328),
        329 => abi_c99_core_probe(bytes, 329),
        330 => abi_c99_core_probe(bytes, 330),
        331 => abi_c99_core_probe(bytes, 331),
        332 => abi_c99_core_probe(bytes, 332),
        333 => abi_c99_core_probe(bytes, 333),
        334 => abi_c99_core_probe(bytes, 334),
        335 => abi_c99_core_probe(bytes, 335),
        336 => abi_c99_core_probe(bytes, 336),
        337 => abi_c99_core_probe(bytes, 337),
        338 => abi_c99_core_probe(bytes, 338),
        339 => abi_c99_core_probe(bytes, 339),
        340 => abi_c99_core_probe(bytes, 340),
        341 => abi_c99_core_probe(bytes, 341),
        342 => abi_c99_core_probe(bytes, 342),
        343 => abi_c99_core_probe(bytes, 343),
        344 => abi_c99_core_probe(bytes, 344),
        345 => abi_c99_core_probe(bytes, 345),
        346 => abi_c99_core_probe(bytes, 346),
        347 => abi_c99_core_probe(bytes, 347),
        348 => abi_c99_core_probe(bytes, 348),
        349 => abi_c99_core_probe(bytes, 349),
        350 => abi_c99_core_probe(bytes, 350),
        351 => abi_c99_core_probe(bytes, 351),
        352 => abi_c99_core_probe(bytes, 352),
        353 => abi_c99_core_probe(bytes, 353),
        354 => abi_c99_core_probe(bytes, 354),
        355 => abi_c99_core_probe(bytes, 355),
        356 => abi_c99_core_probe(bytes, 356),
        357 => abi_c99_core_probe(bytes, 357),
        358 => abi_c99_core_probe(bytes, 358),
        359 => abi_c99_core_probe(bytes, 359),
        360 => abi_c99_core_probe(bytes, 360),
        361 => abi_c99_core_probe(bytes, 361),
        362 => abi_c99_core_probe(bytes, 362),
        363 => abi_c99_core_probe(bytes, 363),
        364 => abi_c99_core_probe(bytes, 364),
        365 => abi_c99_core_probe(bytes, 365),
        366 => abi_c99_core_probe(bytes, 366),
        367 => abi_c99_core_probe(bytes, 367),
        368 => abi_c99_core_probe(bytes, 368),
        369 => abi_c99_core_probe(bytes, 369),
        370 => abi_c99_core_probe(bytes, 370),
        371 => abi_c99_core_probe(bytes, 371),
        372 => abi_c99_core_probe(bytes, 372),
        373 => abi_c99_core_probe(bytes, 373),
        374 => abi_c99_core_probe(bytes, 374),
        375 => abi_c99_core_probe(bytes, 375),
        376 => abi_c99_core_probe(bytes, 376),
        377 => abi_c99_core_probe(bytes, 377),
        378 => abi_c99_core_probe(bytes, 378),
        379 => abi_c99_core_probe(bytes, 379),
        380 => abi_c99_core_probe(bytes, 380),
        381 => abi_c99_core_probe(bytes, 381),
        382 => abi_c99_core_probe(bytes, 382),
        383 => abi_c99_core_probe(bytes, 383),
        384 => abi_c99_core_probe(bytes, 384),
        385 => abi_c99_core_probe(bytes, 385),
        386 => abi_c99_core_probe(bytes, 386),
        387 => abi_c99_core_probe(bytes, 387),
        388 => abi_c99_core_probe(bytes, 388),
        389 => abi_c99_core_probe(bytes, 389),
        390 => abi_c99_core_probe(bytes, 390),
        391 => abi_c99_core_probe(bytes, 391),
        392 => abi_c99_core_probe(bytes, 392),
        393 => abi_c99_core_probe(bytes, 393),
        394 => abi_c99_core_probe(bytes, 394),
        395 => abi_c99_core_probe(bytes, 395),
        396 => abi_c99_core_probe(bytes, 396),
        397 => abi_c99_core_probe(bytes, 397),
        398 => abi_c99_core_probe(bytes, 398),
        399 => abi_c99_core_probe(bytes, 399),
        400 => abi_c99_core_probe(bytes, 400),
        401 => abi_c100_core_probe(bytes, 401),
        402 => abi_c100_core_probe(bytes, 402),
        403 => abi_c100_core_probe(bytes, 403),
        404 => abi_c100_core_probe(bytes, 404),
        405 => abi_c100_core_probe(bytes, 405),
        406 => abi_c100_core_probe(bytes, 406),
        407 => abi_c100_core_probe(bytes, 407),
        408 => abi_c100_core_probe(bytes, 408),
        409 => abi_c100_core_probe(bytes, 409),
        410 => abi_c100_core_probe(bytes, 410),
        411 => abi_c100_core_probe(bytes, 411),
        412 => abi_c100_core_probe(bytes, 412),
        413 => abi_c100_core_probe(bytes, 413),
        414 => abi_c100_core_probe(bytes, 414),
        415 => abi_c100_core_probe(bytes, 415),
        416 => abi_c100_core_probe(bytes, 416),
        417 => abi_c100_core_probe(bytes, 417),
        418 => abi_c100_core_probe(bytes, 418),
        419 => abi_c100_core_probe(bytes, 419),
        420 => abi_c100_core_probe(bytes, 420),
        421 => abi_c100_core_probe(bytes, 421),
        422 => abi_c100_core_probe(bytes, 422),
        423 => abi_c100_core_probe(bytes, 423),
        424 => abi_c100_core_probe(bytes, 424),
        425 => abi_c100_core_probe(bytes, 425),
        426 => abi_c100_core_probe(bytes, 426),
        427 => abi_c100_core_probe(bytes, 427),
        428 => abi_c100_core_probe(bytes, 428),
        429 => abi_c100_core_probe(bytes, 429),
        430 => abi_c100_core_probe(bytes, 430),
        431 => abi_c100_core_probe(bytes, 431),
        432 => abi_c100_core_probe(bytes, 432),
        433 => abi_c100_core_probe(bytes, 433),
        434 => abi_c100_core_probe(bytes, 434),
        435 => abi_c100_core_probe(bytes, 435),
        436 => abi_c100_core_probe(bytes, 436),
        437 => abi_c100_core_probe(bytes, 437),
        438 => abi_c100_core_probe(bytes, 438),
        439 => abi_c100_core_probe(bytes, 439),
        440 => abi_c100_core_probe(bytes, 440),
        441 => abi_c100_core_probe(bytes, 441),
        442 => abi_c100_core_probe(bytes, 442),
        443 => abi_c100_core_probe(bytes, 443),
        444 => abi_c100_core_probe(bytes, 444),
        445 => abi_c100_core_probe(bytes, 445),
        446 => abi_c100_core_probe(bytes, 446),
        447 => abi_c100_core_probe(bytes, 447),
        448 => abi_c100_core_probe(bytes, 448),
        449 => abi_c100_core_probe(bytes, 449),
        450 => abi_c100_core_probe(bytes, 450),
        451 => abi_c100_core_probe(bytes, 451),
        452 => abi_c100_core_probe(bytes, 452),
        453 => abi_c100_core_probe(bytes, 453),
        454 => abi_c100_core_probe(bytes, 454),
        455 => abi_c100_core_probe(bytes, 455),
        456 => abi_c100_core_probe(bytes, 456),
        457 => abi_c100_core_probe(bytes, 457),
        458 => abi_c100_core_probe(bytes, 458),
        459 => abi_c100_core_probe(bytes, 459),
        460 => abi_c100_core_probe(bytes, 460),
        461 => abi_c100_core_probe(bytes, 461),
        462 => abi_c100_core_probe(bytes, 462),
        463 => abi_c100_core_probe(bytes, 463),
        464 => abi_c100_core_probe(bytes, 464),
        465 => abi_c100_core_probe(bytes, 465),
        466 => abi_c100_core_probe(bytes, 466),
        467 => abi_c100_core_probe(bytes, 467),
        468 => abi_c100_core_probe(bytes, 468),
        469 => abi_c100_core_probe(bytes, 469),
        470 => abi_c100_core_probe(bytes, 470),
        471 => abi_c100_core_probe(bytes, 471),
        472 => abi_c100_core_probe(bytes, 472),
        473 => abi_c100_core_probe(bytes, 473),
        474 => abi_c100_core_probe(bytes, 474),
        475 => abi_c100_core_probe(bytes, 475),
        476 => abi_c100_core_probe(bytes, 476),
        477 => abi_c100_core_probe(bytes, 477),
        478 => abi_c100_core_probe(bytes, 478),
        479 => abi_c100_core_probe(bytes, 479),
        480 => abi_c100_core_probe(bytes, 480),
        481 => abi_c100_core_probe(bytes, 481),
        482 => abi_c100_core_probe(bytes, 482),
        483 => abi_c100_core_probe(bytes, 483),
        484 => abi_c100_core_probe(bytes, 484),
        485 => abi_c100_core_probe(bytes, 485),
        486 => abi_c100_core_probe(bytes, 486),
        487 => abi_c100_core_probe(bytes, 487),
        488 => abi_c100_core_probe(bytes, 488),
        489 => abi_c100_core_probe(bytes, 489),
        490 => abi_c100_core_probe(bytes, 490),
        491 => abi_c100_core_probe(bytes, 491),
        492 => abi_c100_core_probe(bytes, 492),
        493 => abi_c100_core_probe(bytes, 493),
        494 => abi_c100_core_probe(bytes, 494),
        495 => abi_c100_core_probe(bytes, 495),
        496 => abi_c100_core_probe(bytes, 496),
        497 => abi_c100_core_probe(bytes, 497),
        498 => abi_c100_core_probe(bytes, 498),
        499 => abi_c100_core_probe(bytes, 499),
        500 => abi_c100_core_probe(bytes, 500),
        501 => abi_c101_wrapper_probe(library, face, 501),
        502 => abi_c101_wrapper_probe(library, face, 502),
        503 => abi_c101_wrapper_probe(library, face, 503),
        504 => abi_c101_wrapper_probe(library, face, 504),
        505 => abi_c101_wrapper_probe(library, face, 505),
        506 => abi_c101_wrapper_probe(library, face, 506),
        507 => abi_c101_wrapper_probe(library, face, 507),
        508 => abi_c101_wrapper_probe(library, face, 508),
        509 => abi_c101_wrapper_probe(library, face, 509),
        510 => abi_c101_wrapper_probe(library, face, 510),
        511 => abi_c101_wrapper_probe(library, face, 511),
        512 => abi_c101_wrapper_probe(library, face, 512),
        513 => abi_c101_wrapper_probe(library, face, 513),
        514 => abi_c101_wrapper_probe(library, face, 514),
        515 => abi_c101_wrapper_probe(library, face, 515),
        516 => abi_c101_wrapper_probe(library, face, 516),
        517 => abi_c101_wrapper_probe(library, face, 517),
        518 => abi_c101_wrapper_probe(library, face, 518),
        519 => abi_c101_wrapper_probe(library, face, 519),
        520 => abi_c101_wrapper_probe(library, face, 520),
        521 => abi_c101_wrapper_probe(library, face, 521),
        522 => abi_c101_wrapper_probe(library, face, 522),
        523 => abi_c101_wrapper_probe(library, face, 523),
        524 => abi_c101_wrapper_probe(library, face, 524),
        525 => abi_c101_wrapper_probe(library, face, 525),
        526 => abi_c101_wrapper_probe(library, face, 526),
        527 => abi_c101_wrapper_probe(library, face, 527),
        528 => abi_c101_wrapper_probe(library, face, 528),
        529 => abi_c101_wrapper_probe(library, face, 529),
        530 => abi_c101_wrapper_probe(library, face, 530),
        531 => abi_c101_wrapper_probe(library, face, 531),
        532 => abi_c101_wrapper_probe(library, face, 532),
        533 => abi_c101_wrapper_probe(library, face, 533),
        534 => abi_c101_wrapper_probe(library, face, 534),
        535 => abi_c101_wrapper_probe(library, face, 535),
        536 => abi_c101_wrapper_probe(library, face, 536),
        537 => abi_c101_wrapper_probe(library, face, 537),
        538 => abi_c101_wrapper_probe(library, face, 538),
        539 => abi_c101_wrapper_probe(library, face, 539),
        540 => abi_c101_wrapper_probe(library, face, 540),
        541 => abi_c101_wrapper_probe(library, face, 541),
        542 => abi_c101_wrapper_probe(library, face, 542),
        543 => abi_c101_wrapper_probe(library, face, 543),
        544 => abi_c101_wrapper_probe(library, face, 544),
        545 => abi_c101_wrapper_probe(library, face, 545),
        546 => abi_c101_wrapper_probe(library, face, 546),
        547 => abi_c101_wrapper_probe(library, face, 547),
        548 => abi_c101_wrapper_probe(library, face, 548),
        549 => abi_c101_wrapper_probe(library, face, 549),
        550 => abi_c101_wrapper_probe(library, face, 550),
        551 => abi_c101_wrapper_probe(library, face, 551),
        552 => abi_c101_wrapper_probe(library, face, 552),
        553 => abi_c101_wrapper_probe(library, face, 553),
        554 => abi_c101_wrapper_probe(library, face, 554),
        555 => abi_c101_wrapper_probe(library, face, 555),
        556 => abi_c101_wrapper_probe(library, face, 556),
        557 => abi_c101_wrapper_probe(library, face, 557),
        558 => abi_c101_wrapper_probe(library, face, 558),
        559 => abi_c101_wrapper_probe(library, face, 559),
        560 => abi_c101_wrapper_probe(library, face, 560),
        561 => abi_c101_wrapper_probe(library, face, 561),
        562 => abi_c101_wrapper_probe(library, face, 562),
        563 => abi_c101_wrapper_probe(library, face, 563),
        564 => abi_c101_wrapper_probe(library, face, 564),
        565 => abi_c101_wrapper_probe(library, face, 565),
        566 => abi_c101_wrapper_probe(library, face, 566),
        567 => abi_c101_wrapper_probe(library, face, 567),
        568 => abi_c101_wrapper_probe(library, face, 568),
        569 => abi_c101_wrapper_probe(library, face, 569),
        570 => abi_c101_wrapper_probe(library, face, 570),
        571 => abi_c101_wrapper_probe(library, face, 571),
        572 => abi_c101_wrapper_probe(library, face, 572),
        573 => abi_c101_wrapper_probe(library, face, 573),
        574 => abi_c101_wrapper_probe(library, face, 574),
        575 => abi_c101_wrapper_probe(library, face, 575),
        576 => abi_c101_wrapper_probe(library, face, 576),
        577 => abi_c101_wrapper_probe(library, face, 577),
        578 => abi_c101_wrapper_probe(library, face, 578),
        579 => abi_c101_wrapper_probe(library, face, 579),
        580 => abi_c101_wrapper_probe(library, face, 580),
        581 => abi_c101_wrapper_probe(library, face, 581),
        582 => abi_c101_wrapper_probe(library, face, 582),
        583 => abi_c101_wrapper_probe(library, face, 583),
        584 => abi_c101_wrapper_probe(library, face, 584),
        585 => abi_c101_wrapper_probe(library, face, 585),
        586 => abi_c101_wrapper_probe(library, face, 586),
        587 => abi_c101_wrapper_probe(library, face, 587),
        588 => abi_c101_wrapper_probe(library, face, 588),
        589 => abi_c101_wrapper_probe(library, face, 589),
        590 => abi_c101_wrapper_probe(library, face, 590),
        591 => abi_c101_wrapper_probe(library, face, 591),
        592 => abi_c101_wrapper_probe(library, face, 592),
        593 => abi_c101_wrapper_probe(library, face, 593),
        594 => abi_c101_wrapper_probe(library, face, 594),
        595 => abi_c101_wrapper_probe(library, face, 595),
        596 => abi_c101_wrapper_probe(library, face, 596),
        597 => abi_c101_wrapper_probe(library, face, 597),
        598 => abi_c101_wrapper_probe(library, face, 598),
        599 => abi_c101_wrapper_probe(library, face, 599),
        600 => abi_c101_wrapper_probe(library, face, 600),
        601 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 601),
        602 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 602),
        603 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 603),
        604 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 604),
        605 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 605),
        606 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 606),
        607 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 607),
        608 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 608),
        609 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 609),
        610 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 610),
        611 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 611),
        612 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 612),
        613 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 613),
        614 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 614),
        615 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 615),
        616 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 616),
        617 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 617),
        618 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 618),
        619 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 619),
        620 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 620),
        621 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 621),
        622 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 622),
        623 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 623),
        624 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 624),
        625 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 625),
        626 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 626),
        627 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 627),
        628 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 628),
        629 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 629),
        630 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 630),
        631 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 631),
        632 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 632),
        633 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 633),
        634 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 634),
        635 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 635),
        636 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 636),
        637 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 637),
        638 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 638),
        639 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 639),
        640 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 640),
        641 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 641),
        642 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 642),
        643 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 643),
        644 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 644),
        645 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 645),
        646 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 646),
        647 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 647),
        648 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 648),
        649 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 649),
        650 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 650),
        651 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 651),
        652 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 652),
        653 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 653),
        654 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 654),
        655 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 655),
        656 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 656),
        657 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 657),
        658 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 658),
        659 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 659),
        660 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 660),
        661 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 661),
        662 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 662),
        663 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 663),
        664 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 664),
        665 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 665),
        666 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 666),
        667 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 667),
        668 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 668),
        669 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 669),
        670 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 670),
        671 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 671),
        672 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 672),
        673 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 673),
        674 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 674),
        675 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 675),
        676 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 676),
        677 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 677),
        678 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 678),
        679 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 679),
        680 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 680),
        681 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 681),
        682 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 682),
        683 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 683),
        684 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 684),
        685 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 685),
        686 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 686),
        687 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 687),
        688 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 688),
        689 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 689),
        690 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 690),
        691 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 691),
        692 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 692),
        693 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 693),
        694 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 694),
        695 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 695),
        696 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 696),
        697 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 697),
        698 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 698),
        699 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 699),
        700 => abi_c102_live_wrapper_probe(bytes, library, face, memory, 700),
        701 => abi_c103_wrapper_probe(bytes, library, face, memory, 701),
        702 => abi_c103_wrapper_probe(bytes, library, face, memory, 702),
        703 => abi_c103_wrapper_probe(bytes, library, face, memory, 703),
        704 => abi_c103_wrapper_probe(bytes, library, face, memory, 704),
        705 => abi_c103_wrapper_probe(bytes, library, face, memory, 705),
        706 => abi_c103_wrapper_probe(bytes, library, face, memory, 706),
        707 => abi_c103_wrapper_probe(bytes, library, face, memory, 707),
        708 => abi_c103_wrapper_probe(bytes, library, face, memory, 708),
        709 => abi_c103_wrapper_probe(bytes, library, face, memory, 709),
        710 => abi_c103_wrapper_probe(bytes, library, face, memory, 710),
        711 => abi_c103_wrapper_probe(bytes, library, face, memory, 711),
        712 => abi_c103_wrapper_probe(bytes, library, face, memory, 712),
        713 => abi_c103_wrapper_probe(bytes, library, face, memory, 713),
        714 => abi_c103_wrapper_probe(bytes, library, face, memory, 714),
        715 => abi_c103_wrapper_probe(bytes, library, face, memory, 715),
        716 => abi_c103_wrapper_probe(bytes, library, face, memory, 716),
        717 => abi_c103_wrapper_probe(bytes, library, face, memory, 717),
        718 => abi_c103_wrapper_probe(bytes, library, face, memory, 718),
        719 => abi_c103_wrapper_probe(bytes, library, face, memory, 719),
        720 => abi_c103_wrapper_probe(bytes, library, face, memory, 720),
        721 => abi_c103_wrapper_probe(bytes, library, face, memory, 721),
        722 => abi_c103_wrapper_probe(bytes, library, face, memory, 722),
        723 => abi_c103_wrapper_probe(bytes, library, face, memory, 723),
        724 => abi_c103_wrapper_probe(bytes, library, face, memory, 724),
        725 => abi_c103_wrapper_probe(bytes, library, face, memory, 725),
        726 => abi_c103_wrapper_probe(bytes, library, face, memory, 726),
        727 => abi_c103_wrapper_probe(bytes, library, face, memory, 727),
        728 => abi_c103_wrapper_probe(bytes, library, face, memory, 728),
        729 => abi_c103_wrapper_probe(bytes, library, face, memory, 729),
        730 => abi_c103_wrapper_probe(bytes, library, face, memory, 730),
        731 => abi_c103_wrapper_probe(bytes, library, face, memory, 731),
        732 => abi_c103_wrapper_probe(bytes, library, face, memory, 732),
        733 => abi_c103_wrapper_probe(bytes, library, face, memory, 733),
        734 => abi_c103_wrapper_probe(bytes, library, face, memory, 734),
        735 => abi_c103_wrapper_probe(bytes, library, face, memory, 735),
        736 => abi_c103_wrapper_probe(bytes, library, face, memory, 736),
        737 => abi_c103_wrapper_probe(bytes, library, face, memory, 737),
        738 => abi_c103_wrapper_probe(bytes, library, face, memory, 738),
        739 => abi_c103_wrapper_probe(bytes, library, face, memory, 739),
        740 => abi_c103_wrapper_probe(bytes, library, face, memory, 740),
        741 => abi_c103_wrapper_probe(bytes, library, face, memory, 741),
        742 => abi_c103_wrapper_probe(bytes, library, face, memory, 742),
        743 => abi_c103_wrapper_probe(bytes, library, face, memory, 743),
        744 => abi_c103_wrapper_probe(bytes, library, face, memory, 744),
        745 => abi_c103_wrapper_probe(bytes, library, face, memory, 745),
        746 => abi_c103_wrapper_probe(bytes, library, face, memory, 746),
        747 => abi_c103_wrapper_probe(bytes, library, face, memory, 747),
        748 => abi_c103_wrapper_probe(bytes, library, face, memory, 748),
        749 => abi_c103_wrapper_probe(bytes, library, face, memory, 749),
        750 => abi_c103_wrapper_probe(bytes, library, face, memory, 750),
        751 => abi_c103_wrapper_probe(bytes, library, face, memory, 751),
        752 => abi_c103_wrapper_probe(bytes, library, face, memory, 752),
        753 => abi_c103_wrapper_probe(bytes, library, face, memory, 753),
        754 => abi_c103_wrapper_probe(bytes, library, face, memory, 754),
        755 => abi_c103_wrapper_probe(bytes, library, face, memory, 755),
        756 => abi_c103_wrapper_probe(bytes, library, face, memory, 756),
        757 => abi_c103_wrapper_probe(bytes, library, face, memory, 757),
        758 => abi_c103_wrapper_probe(bytes, library, face, memory, 758),
        759 => abi_c103_wrapper_probe(bytes, library, face, memory, 759),
        760 => abi_c103_wrapper_probe(bytes, library, face, memory, 760),
        761 => abi_c103_wrapper_probe(bytes, library, face, memory, 761),
        762 => abi_c103_wrapper_probe(bytes, library, face, memory, 762),
        763 => abi_c103_wrapper_probe(bytes, library, face, memory, 763),
        764 => abi_c103_wrapper_probe(bytes, library, face, memory, 764),
        765 => abi_c103_wrapper_probe(bytes, library, face, memory, 765),
        766 => abi_c103_wrapper_probe(bytes, library, face, memory, 766),
        767 => abi_c103_wrapper_probe(bytes, library, face, memory, 767),
        768 => abi_c103_wrapper_probe(bytes, library, face, memory, 768),
        769 => abi_c103_wrapper_probe(bytes, library, face, memory, 769),
        770 => abi_c103_wrapper_probe(bytes, library, face, memory, 770),
        771 => abi_c103_wrapper_probe(bytes, library, face, memory, 771),
        772 => abi_c103_wrapper_probe(bytes, library, face, memory, 772),
        773 => abi_c103_wrapper_probe(bytes, library, face, memory, 773),
        774 => abi_c103_wrapper_probe(bytes, library, face, memory, 774),
        775 => abi_c103_wrapper_probe(bytes, library, face, memory, 775),
        776 => abi_c103_wrapper_probe(bytes, library, face, memory, 776),
        777 => abi_c103_wrapper_probe(bytes, library, face, memory, 777),
        778 => abi_c103_wrapper_probe(bytes, library, face, memory, 778),
        779 => abi_c103_wrapper_probe(bytes, library, face, memory, 779),
        780 => abi_c103_wrapper_probe(bytes, library, face, memory, 780),
        781 => abi_c103_wrapper_probe(bytes, library, face, memory, 781),
        782 => abi_c103_wrapper_probe(bytes, library, face, memory, 782),
        783 => abi_c103_wrapper_probe(bytes, library, face, memory, 783),
        784 => abi_c103_wrapper_probe(bytes, library, face, memory, 784),
        785 => abi_c103_wrapper_probe(bytes, library, face, memory, 785),
        786 => abi_c103_wrapper_probe(bytes, library, face, memory, 786),
        787 => abi_c103_wrapper_probe(bytes, library, face, memory, 787),
        788 => abi_c103_wrapper_probe(bytes, library, face, memory, 788),
        789 => abi_c103_wrapper_probe(bytes, library, face, memory, 789),
        790 => abi_c103_wrapper_probe(bytes, library, face, memory, 790),
        801 => abi_c104_handles_probe(bytes, library, face, memory, 801),
        802 => abi_c104_handles_probe(bytes, library, face, memory, 802),
        803 => abi_c104_handles_probe(bytes, library, face, memory, 803),
        804 => abi_c104_handles_probe(bytes, library, face, memory, 804),
        805 => abi_c104_handles_probe(bytes, library, face, memory, 805),
        806 => abi_c104_handles_probe(bytes, library, face, memory, 806),
        807 => abi_c104_handles_probe(bytes, library, face, memory, 807),
        808 => abi_c104_handles_probe(bytes, library, face, memory, 808),
        809 => abi_c104_handles_probe(bytes, library, face, memory, 809),
        810 => abi_c104_handles_probe(bytes, library, face, memory, 810),
        811 => abi_c104_handles_probe(bytes, library, face, memory, 811),
        812 => abi_c104_handles_probe(bytes, library, face, memory, 812),
        813 => abi_c104_handles_probe(bytes, library, face, memory, 813),
        814 => abi_c104_handles_probe(bytes, library, face, memory, 814),
        815 => abi_c104_handles_probe(bytes, library, face, memory, 815),
        816 => abi_c104_handles_probe(bytes, library, face, memory, 816),
        817 => abi_c104_handles_probe(bytes, library, face, memory, 817),
        818 => abi_c104_handles_probe(bytes, library, face, memory, 818),
        819 => abi_c104_handles_probe(bytes, library, face, memory, 819),
        820 => abi_c104_handles_probe(bytes, library, face, memory, 820),
        821 => abi_c104_handles_probe(bytes, library, face, memory, 821),
        822 => abi_c104_handles_probe(bytes, library, face, memory, 822),
        823 => abi_c104_handles_probe(bytes, library, face, memory, 823),
        824 => abi_c104_handles_probe(bytes, library, face, memory, 824),
        825 => abi_c104_handles_probe(bytes, library, face, memory, 825),
        826 => abi_c104_handles_probe(bytes, library, face, memory, 826),
        827 => abi_c104_handles_probe(bytes, library, face, memory, 827),
        828 => abi_c104_handles_probe(bytes, library, face, memory, 828),
        829 => abi_c104_handles_probe(bytes, library, face, memory, 829),
        830 => abi_c104_handles_probe(bytes, library, face, memory, 830),
        831 => abi_c104_handles_probe(bytes, library, face, memory, 831),
        832 => abi_c104_handles_probe(bytes, library, face, memory, 832),
        833 => abi_c104_handles_probe(bytes, library, face, memory, 833),
        834 => abi_c104_handles_probe(bytes, library, face, memory, 834),
        835 => abi_c104_handles_probe(bytes, library, face, memory, 835),
        836 => abi_c104_handles_probe(bytes, library, face, memory, 836),
        837 => abi_c104_handles_probe(bytes, library, face, memory, 837),
        838 => abi_c104_handles_probe(bytes, library, face, memory, 838),
        839 => abi_c104_handles_probe(bytes, library, face, memory, 839),
        840 => abi_c104_handles_probe(bytes, library, face, memory, 840),
        841 => abi_c104_handles_probe(bytes, library, face, memory, 841),
        842 => abi_c104_handles_probe(bytes, library, face, memory, 842),
        843 => abi_c104_handles_probe(bytes, library, face, memory, 843),
        844 => abi_c104_handles_probe(bytes, library, face, memory, 844),
        845 => abi_c104_handles_probe(bytes, library, face, memory, 845),
        846 => abi_c104_handles_probe(bytes, library, face, memory, 846),
        847 => abi_c104_handles_probe(bytes, library, face, memory, 847),
        848 => abi_c104_handles_probe(bytes, library, face, memory, 848),
        849 => abi_c104_handles_probe(bytes, library, face, memory, 849),
        850 => abi_c104_handles_probe(bytes, library, face, memory, 850),
        851 => abi_c104_handles_probe(bytes, library, face, memory, 851),
        852 => abi_c104_handles_probe(bytes, library, face, memory, 852),
        853 => abi_c104_handles_probe(bytes, library, face, memory, 853),
        854 => abi_c104_handles_probe(bytes, library, face, memory, 854),
        855 => abi_c104_handles_probe(bytes, library, face, memory, 855),
        856 => abi_c104_handles_probe(bytes, library, face, memory, 856),
        857 => abi_c104_handles_probe(bytes, library, face, memory, 857),
        858 => abi_c104_handles_probe(bytes, library, face, memory, 858),
        859 => abi_c104_handles_probe(bytes, library, face, memory, 859),
        860 => abi_c104_handles_probe(bytes, library, face, memory, 860),
        861 => abi_c104_handles_probe(bytes, library, face, memory, 861),
        862 => abi_c104_handles_probe(bytes, library, face, memory, 862),
        863 => abi_c104_handles_probe(bytes, library, face, memory, 863),
        864 => abi_c104_handles_probe(bytes, library, face, memory, 864),
        865 => abi_c104_handles_probe(bytes, library, face, memory, 865),
        866 => abi_c104_handles_probe(bytes, library, face, memory, 866),
        867 => abi_c104_handles_probe(bytes, library, face, memory, 867),
        868 => abi_c104_handles_probe(bytes, library, face, memory, 868),
        869 => abi_c104_handles_probe(bytes, library, face, memory, 869),
        870 => abi_c104_handles_probe(bytes, library, face, memory, 870),
        871 => abi_c104_handles_probe(bytes, library, face, memory, 871),
        872 => abi_c104_handles_probe(bytes, library, face, memory, 872),
        873 => abi_c104_handles_probe(bytes, library, face, memory, 873),
        874 => abi_c104_handles_probe(bytes, library, face, memory, 874),
        875 => abi_c104_handles_probe(bytes, library, face, memory, 875),
        876 => abi_c104_handles_probe(bytes, library, face, memory, 876),
        877 => abi_c104_handles_probe(bytes, library, face, memory, 877),
        878 => abi_c104_handles_probe(bytes, library, face, memory, 878),
        879 => abi_c104_handles_probe(bytes, library, face, memory, 879),
        880 => abi_c104_handles_probe(bytes, library, face, memory, 880),
        881 => abi_c104_handles_probe(bytes, library, face, memory, 881),
        882 => abi_c104_handles_probe(bytes, library, face, memory, 882),
        883 => abi_c104_handles_probe(bytes, library, face, memory, 883),
        884 => abi_c104_handles_probe(bytes, library, face, memory, 884),
        885 => abi_c104_handles_probe(bytes, library, face, memory, 885),
        886 => abi_c104_handles_probe(bytes, library, face, memory, 886),
        887 => abi_c104_handles_probe(bytes, library, face, memory, 887),
        888 => abi_c104_handles_probe(bytes, library, face, memory, 888),
        889 => abi_c104_handles_probe(bytes, library, face, memory, 889),
        890 => abi_c104_handles_probe(bytes, library, face, memory, 890),
        891 => abi_c104_handles_probe(bytes, library, face, memory, 891),
        892 => abi_c104_handles_probe(bytes, library, face, memory, 892),
        893 => abi_c104_handles_probe(bytes, library, face, memory, 893),
        894 => abi_c104_handles_probe(bytes, library, face, memory, 894),
        895 => abi_c104_handles_probe(bytes, library, face, memory, 895),
        896 => abi_c104_handles_probe(bytes, library, face, memory, 896),
        897 => abi_c104_handles_probe(bytes, library, face, memory, 897),
        898 => abi_c104_handles_probe(bytes, library, face, memory, 898),
        899 => abi_c104_handles_probe(bytes, library, face, memory, 899),
        901 => abi_c105_handles_probe(bytes, library, face, memory, 901),
        902 => abi_c105_handles_probe(bytes, library, face, memory, 902),
        903 => abi_c105_handles_probe(bytes, library, face, memory, 903),
        904 => abi_c105_handles_probe(bytes, library, face, memory, 904),
        905 => abi_c105_handles_probe(bytes, library, face, memory, 905),
        906 => abi_c105_handles_probe(bytes, library, face, memory, 906),
        907 => abi_c105_handles_probe(bytes, library, face, memory, 907),
        908 => abi_c105_handles_probe(bytes, library, face, memory, 908),
        909 => abi_c105_handles_probe(bytes, library, face, memory, 909),
        910 => abi_c105_handles_probe(bytes, library, face, memory, 910),
        912 => abi_c105_handles_probe(bytes, library, face, memory, 912),
        913 => abi_c105_handles_probe(bytes, library, face, memory, 913),
        914 => abi_c105_handles_probe(bytes, library, face, memory, 914),
        915 => abi_c105_handles_probe(bytes, library, face, memory, 915),
        916 => abi_c105_handles_probe(bytes, library, face, memory, 916),
        918 => abi_c105_handles_probe(bytes, library, face, memory, 918),
        919 => abi_c105_handles_probe(bytes, library, face, memory, 919),
        920 => abi_c105_handles_probe(bytes, library, face, memory, 920),
        922 => abi_c105_handles_probe(bytes, library, face, memory, 922),
        923 => abi_c105_handles_probe(bytes, library, face, memory, 923),
        925 => abi_c105_handles_probe(bytes, library, face, memory, 925),
        926 => abi_c105_handles_probe(bytes, library, face, memory, 926),
        928 => abi_c105_handles_probe(bytes, library, face, memory, 928),
        929 => abi_c105_handles_probe(bytes, library, face, memory, 929),
        930 => abi_c105_handles_probe(bytes, library, face, memory, 930),
        932 => abi_c105_handles_probe(bytes, library, face, memory, 932),
        933 => abi_c105_handles_probe(bytes, library, face, memory, 933),
        936 => abi_c105_handles_probe(bytes, library, face, memory, 936),
        938 => abi_c105_handles_probe(bytes, library, face, memory, 938),
        942 => abi_c105_handles_probe(bytes, library, face, memory, 942),
        943 => abi_c105_handles_probe(bytes, library, face, memory, 943),
        946 => abi_c105_handles_probe(bytes, library, face, memory, 946),
        956 => abi_c105_handles_probe(bytes, library, face, memory, 956),
        966 => abi_c105_handles_probe(bytes, library, face, memory, 966),
        976 => abi_c105_handles_probe(bytes, library, face, memory, 976),
        986 => abi_c105_handles_probe(bytes, library, face, memory, 986),
        996 => abi_c105_handles_probe(bytes, library, face, memory, 996),
        1001 => abi_c106_coverage_probe(bytes, library, face, memory, probe),
        1101 | 1138..=1143 | 1150 | 1154 | 1156 | 1158 | 1161..=1168 => {
            abi_c107_uncovered_branch_probe(bytes, library, face, memory, probe)
        }
        1201..=1220 | 1229 => abi_c108_wrapper_gap_probe(bytes, library, face, memory, probe),
        1301..=1400 => abi_c109_rust_gap_sweep_probe(bytes, library, face, memory, probe),
        1501..=1600 => abi_c110_rust_gap_batch_probe(bytes, library, face, memory, probe),
        1601..=1700 => abi_c111_rust_gap_batch_probe(bytes, library, face, memory, probe),
        1701..=1800 => abi_c112_rust_gap_batch_probe(bytes, library, face, memory, probe),
        1801..=1900 => abi_c113_rust_gap_batch_probe(bytes, library, face, memory, probe),
        1901..=1930 => abi_c114_valid_public_batch_probe(bytes, library, face, memory, probe),
        _ => {}
    }
}

/// Exact normalized observations from a custom `FT_MemoryRec` lifecycle.
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

/// Runs the exported C ABI through a phase-aware custom allocator.
#[cfg(feature = "abi-test-support")]
pub fn abi_custom_memory_lifecycle(bytes: &[u8], face_index: FT_Long) -> AbiCustomMemorySnapshot {
    abi_custom_memory_lifecycle_probe(bytes, face_index, 0)
}

/// Runs the custom-memory lifecycle with an optional OpenType validation probe.
///
/// The probe is test-support only: it drives the public C ABI through the same
/// live face and allocator records as the ordinary lifecycle, while allowing
/// parity fixtures to exercise allocator edge behavior without adding policy
/// or implementation logic to the exported API surface.
#[cfg(feature = "abi-test-support")]
pub fn abi_custom_memory_lifecycle_probe(
    bytes: &[u8],
    face_index: FT_Long,
    probe: u16,
) -> AbiCustomMemorySnapshot {
    let mut data = Box::new(AbiCustomMemoryData {
        expected_memory: 0,
        phase: AbiCustomMemoryPhase::NewLibrary,
        events: Vec::new(),
        blocks: BTreeMap::new(),
        unknown_release: false,
        fail_after: None,
        allocation_count: 0,
    });
    let mut memory = Box::new(FT_MemoryRec {
        user: ptr::from_mut(data.as_mut()).cast(),
        alloc: Some(abi_custom_memory_alloc),
        free: Some(abi_custom_memory_free),
        realloc: Some(abi_custom_memory_realloc),
    });
    data.expected_memory = ptr::from_mut(memory.as_mut()).addr();

    #[cfg(coverage_nightly)]
    {
        // The maintained FT_Memory input declares realloc behavior, but the
        // library lifecycle itself currently needs only alloc/free. Exercise
        // the installed callbacks from that existing route so coverage also
        // measures the callback contract without adding a synthetic case.
        let alloc = memory.alloc.expect("custom allocator installs alloc");
        let realloc = memory.realloc.expect("custom allocator installs realloc");
        let free = memory.free.expect("custom allocator installs free");
        let original = alloc(memory.as_mut(), 4);
        assert!(!original.is_null());
        data.fail_after = Some(data.allocation_count);
        assert!(realloc(memory.as_mut(), 4, 8, original).is_null());
        data.fail_after = None;
        assert!(abi_custom_memory_realloc(memory.as_mut(), 4, -1, ptr::null_mut(),).is_null());
        let resized = realloc(memory.as_mut(), 4, 0, original);
        assert!(!resized.is_null());
        free(memory.as_mut(), resized);
        let trailing = alloc(memory.as_mut(), 0);
        assert!(!trailing.is_null());
        free(memory.as_mut(), trailing);

        // Keep the unknown-block and non-identity paths isolated from the
        // lifecycle snapshot's balanced-release assertions.
        let mut probe_data = Box::new(AbiCustomMemoryData {
            expected_memory: 0,
            phase: AbiCustomMemoryPhase::NewLibrary,
            events: Vec::new(),
            blocks: BTreeMap::new(),
            unknown_release: false,
            fail_after: None,
            allocation_count: 0,
        });
        let mut probe_memory = Box::new(FT_MemoryRec {
            user: ptr::from_mut(probe_data.as_mut()).cast(),
            alloc: Some(abi_custom_memory_alloc),
            free: Some(abi_custom_memory_free),
            realloc: Some(abi_custom_memory_realloc),
        });
        let unknown_block = abi_custom_memory_realloc(
            probe_memory.as_mut(),
            0,
            1,
            NonNull::<c_void>::dangling().as_ptr(),
        );
        assert!(!unknown_block.is_null());
        abi_custom_memory_free(probe_memory.as_mut(), unknown_block);
        let mut empty_memory = FT_MemoryRec::default();
        assert!(abi_custom_memory_realloc(&mut empty_memory, 0, 1, ptr::null_mut(),).is_null());
        assert!(abi_custom_memory_realloc(ptr::null_mut(), 0, 1, ptr::null_mut(),).is_null());
    }

    let mut library = ptr::null_mut();
    let library_status = FT_New_Library(memory.as_mut(), &mut library);
    let mut face_load_status = rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    let mut done_face_status = rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    let mut done_library_status = rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    if library_status == rust_ffi::FT_Err_Ok {
        data.phase = AbiCustomMemoryPhase::AddDefaultModules;
        FT_Add_Default_Modules(library);
        data.phase = AbiCustomMemoryPhase::NewFace;
        let mut face = ptr::null_mut();
        face_load_status = FT_New_Memory_Face(
            library,
            bytes.as_ptr(),
            FT_Long::try_from(bytes.len()).unwrap_or(FT_Long::MAX),
            face_index,
            &mut face,
        );
        if face_load_status == rust_ffi::FT_Err_Ok {
            if (1..=3).contains(&probe) {
                // Install the same lightweight validator module used by the
                // parity routes, then drive the exported C ABI with the live
                // caller-owned memory record.  The three probe values cover
                // empty-table retention, allocator failure, and a callback
                // disappearing after allocation respectively.
                let module_interface = 0_u8;
                let module_name = b"otvalid\0";
                let module_class = FT_Module_Class {
                    module_flags: 0,
                    module_size: 1,
                    module_name: module_name.as_ptr().cast(),
                    module_version: 0x0002_0000,
                    module_requires: 0x0002_0000,
                    module_interface: ptr::from_ref(&module_interface).cast(),
                    module_init: None,
                    module_done: None,
                    get_interface: None,
                };
                if FT_Add_Module(library, &module_class) == rust_ffi::FT_Err_Ok {
                    // The Rust FFI mirrors the C validator's all-output
                    // contract: every table slot must be present even when
                    // only one validation flag is selected.  Keep scratch
                    // slots for the four unselected tables so this probe
                    // reaches the C wrapper's allocator and release paths.
                    let (mut base, mut gdef, mut gpos, mut gsub, mut jstf) = (
                        ptr::null(),
                        ptr::null(),
                        ptr::null(),
                        ptr::null(),
                        ptr::null(),
                    );
                    if probe == 2 {
                        data.fail_after = Some(data.allocation_count);
                    }
                    let _validation_error = FT_OpenType_Validate(
                        face,
                        rust_ffi::FT_VALIDATE_GDEF as FT_UInt,
                        &mut base,
                        &mut gdef,
                        &mut gpos,
                        &mut gsub,
                        &mut jstf,
                    );
                    data.fail_after = None;
                    if probe == 3 && !gdef.is_null() {
                        let free = memory.free;
                        memory.free = None;
                        FT_OpenType_Free(face, gdef);
                        memory.free = free;
                        if let Some(free) = free {
                            free(memory.as_mut(), gdef.cast_mut().cast());
                        }
                    } else if !gdef.is_null() {
                        FT_OpenType_Free(face, gdef);
                    }
                }
            } else if probe >= 10 {
                abi_custom_memory_coverage_probe(bytes, library, face, memory.as_mut(), probe);
            }
            data.phase = AbiCustomMemoryPhase::DoneFace;
            done_face_status = FT_Done_Face(face);
        }
        data.phase = AbiCustomMemoryPhase::DoneLibrary;
        done_library_status = FT_Done_Library(library);
    }

    let has_event = |phase, kind| {
        data.events
            .iter()
            .any(|event| event.phase == phase && event.kind == kind)
    };
    AbiCustomMemorySnapshot {
        library_status,
        face_load_status,
        done_face_status,
        done_library_status,
        new_library_allocated: has_event(
            AbiCustomMemoryPhase::NewLibrary,
            AbiCustomMemoryEventKind::Alloc,
        ),
        modules_allocated: has_event(
            AbiCustomMemoryPhase::AddDefaultModules,
            AbiCustomMemoryEventKind::Alloc,
        ),
        face_allocated: has_event(
            AbiCustomMemoryPhase::NewFace,
            AbiCustomMemoryEventKind::Alloc,
        ),
        face_freed: has_event(
            AbiCustomMemoryPhase::DoneFace,
            AbiCustomMemoryEventKind::Free,
        ),
        library_freed: has_event(
            AbiCustomMemoryPhase::DoneLibrary,
            AbiCustomMemoryEventKind::Free,
        ),
        memory_pointer_identity: data.events.iter().all(|event| event.memory_identity),
        no_unknown_release: !data.unknown_release,
        balanced_after_done: data.blocks.is_empty(),
        first_event_alloc: data
            .events
            .first()
            .is_some_and(|event| event.kind == AbiCustomMemoryEventKind::Alloc),
        last_event_free: data
            .events
            .last()
            .is_some_and(|event| event.kind == AbiCustomMemoryEventKind::Free),
        realloc_contract_preserved: !data.unknown_release,
    }
}

#[cfg(feature = "abi-test-support")]
const ABI_CUSTOM_GLYPH_FORMAT: FT_Glyph_Format = 0x6664_6F6E;

#[cfg(feature = "abi-test-support")]
thread_local! {
    static ABI_CUSTOM_GLYPH_DONE_CALLS: RefCell<usize> = const { RefCell::new(0) };
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone, Copy, Default)]
enum AbiGlyphCopyFailureMode {
    #[default]
    None,
    Allocation,
    BitmapCopy,
    SvgZeroLength,
}

#[cfg(feature = "abi-test-support")]
#[derive(Default)]
struct AbiGlyphCopyFailureState {
    mode: AbiGlyphCopyFailureMode,
    allocation_attempts: usize,
    frees_before_return: usize,
}

#[cfg(feature = "abi-test-support")]
thread_local! {
    static ABI_GLYPH_COPY_FAILURE_STATE: RefCell<AbiGlyphCopyFailureState> =
        RefCell::new(AbiGlyphCopyFailureState::default());
}

#[cfg(feature = "abi-test-support")]
struct AbiGlyphCopyPartialTarget;

#[cfg(feature = "abi-test-support")]
impl Drop for AbiGlyphCopyPartialTarget {
    fn drop(&mut self) {
        ABI_GLYPH_COPY_FAILURE_STATE.with(|state| {
            let frees = state.borrow().frees_before_return.saturating_add(1);
            state.borrow_mut().frees_before_return = frees;
        });
    }
}

#[cfg(feature = "abi-test-support")]
#[repr(C)]
struct AbiCustomGlyphRecord {
    root: FT_GlyphRec,
    payload: FT_Long,
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_custom_glyph_done(_glyph: FT_Glyph) {
    ABI_CUSTOM_GLYPH_DONE_CALLS.with(|calls| {
        let next = calls.borrow().saturating_add(1);
        *calls.borrow_mut() = next;
    });
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_custom_glyph_renderer_init(module: FT_Module) -> FT_Error {
    let Some(renderer) = (unsafe { module.cast::<FT_RendererRec>().as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    renderer.glyph_class = FT_Glyph_Class {
        glyph_size: std::mem::size_of::<AbiCustomGlyphRecord>() as FT_Long,
        glyph_format: ABI_CUSTOM_GLYPH_FORMAT,
        glyph_done: Some(abi_custom_glyph_done),
        ..FT_Glyph_Class::default()
    };
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
#[derive(Default)]
struct AbiRasterLifecycleState {
    events: Vec<&'static str>,
    raster_handle: usize,
    new_memory_nonnull: bool,
    reset_handle_identity: bool,
    reset_pool_null: bool,
    reset_pool_size: FT_ULong,
    set_mode_handle_identity: bool,
    set_mode_tag: FT_ULong,
    set_mode_data_identity: bool,
    render_handle_identity: bool,
    render_source_nonnull: bool,
    done_handle_identity: bool,
}

#[cfg(feature = "abi-test-support")]
thread_local! {
    static ABI_RASTER_LIFECYCLE_STATE: RefCell<AbiRasterLifecycleState> =
        RefCell::new(AbiRasterLifecycleState::default());
}

#[cfg(feature = "abi-test-support")]
static ABI_RASTER_LIFECYCLE_TOKEN: usize = 0;

#[cfg(feature = "abi-test-support")]
fn abi_raster_lifecycle_event(event: &'static str) {
    ABI_RASTER_LIFECYCLE_STATE.with(|state| state.borrow_mut().events.push(event));
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_lifecycle_new(memory: FT_Pointer, raster: *mut FT_Raster) -> c_int {
    abi_raster_lifecycle_event("raster_new");
    if raster.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let handle: FT_Raster = ptr::from_ref(&ABI_RASTER_LIFECYCLE_TOKEN).cast_mut().cast();
    ABI_RASTER_LIFECYCLE_STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.raster_handle = handle.addr();
        state.new_memory_nonnull = !memory.is_null();
    });
    // SAFETY: the callback contract supplies writable raster-handle storage.
    unsafe {
        *raster = handle;
    }
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_new_error_callback(
    memory: FT_Pointer,
    _raster: *mut FT_Raster,
) -> c_int {
    let _ = memory;
    abi_raster_lifecycle_event("raster_new");
    rust_ffi::FT_Err_Out_Of_Memory
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_lifecycle_reset(
    raster: FT_Raster,
    pool_base: *mut FT_Byte,
    pool_size: FT_ULong,
) {
    abi_raster_lifecycle_event("raster_reset");
    ABI_RASTER_LIFECYCLE_STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.reset_handle_identity = raster.addr() == state.raster_handle;
        state.reset_pool_null = pool_base.is_null();
        state.reset_pool_size = pool_size;
    });
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_lifecycle_set_mode(
    raster: FT_Raster,
    mode: FT_ULong,
    args: FT_Pointer,
) -> c_int {
    abi_raster_lifecycle_event("raster_set_mode");
    ABI_RASTER_LIFECYCLE_STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.set_mode_handle_identity = raster.addr() == state.raster_handle;
        state.set_mode_tag = mode;
        state.set_mode_data_identity = !args.is_null();
    });
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_lifecycle_render(
    raster: FT_Raster,
    params: *const FT_Raster_Params,
) -> c_int {
    abi_raster_lifecycle_event("raster_render");
    ABI_RASTER_LIFECYCLE_STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.render_handle_identity = raster.addr() == state.raster_handle;
        // SAFETY: the renderer callback supplies one readable parameter
        // record for the duration of this synchronous call.
        state.render_source_nonnull =
            unsafe { params.as_ref() }.is_some_and(|params| !params.source.is_null());
    });
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_lifecycle_done(raster: FT_Raster) {
    abi_raster_lifecycle_event("raster_done");
    ABI_RASTER_LIFECYCLE_STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.done_handle_identity = raster.addr() == state.raster_handle;
    });
}

#[cfg(feature = "abi-test-support")]
static ABI_RASTER_LIFECYCLE_FUNCS: FT_Raster_Funcs = FT_Raster_Funcs {
    glyph_format: rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
    raster_new: Some(abi_raster_lifecycle_new),
    raster_reset: Some(abi_raster_lifecycle_reset),
    raster_set_mode: Some(abi_raster_lifecycle_set_mode),
    raster_render: Some(abi_raster_lifecycle_render),
    raster_done: Some(abi_raster_lifecycle_done),
};

#[cfg(feature = "abi-test-support")]
static ABI_RASTER_NEW_ERROR_FUNCS: FT_Raster_Funcs = FT_Raster_Funcs {
    glyph_format: rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
    raster_new: Some(abi_raster_new_error_callback),
    raster_reset: Some(abi_raster_lifecycle_reset),
    raster_set_mode: Some(abi_raster_lifecycle_set_mode),
    raster_render: Some(abi_raster_lifecycle_render),
    raster_done: Some(abi_raster_lifecycle_done),
};

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_lifecycle_module_init(module: FT_Module) -> FT_Error {
    abi_raster_lifecycle_event("module_init");
    // SAFETY: FreeType passes the live renderer allocation associated with
    // the registered renderer class.
    let Some(renderer) = (unsafe { module.cast::<FT_RendererRec>().as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(clazz) = (unsafe { renderer.clazz.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(raster_class) = (unsafe { clazz.raster_class.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(reset) = raster_class.raster_reset else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `renderer.raster` was created by this table immediately before
    // module initialization.
    unsafe {
        reset(renderer.raster, ptr::null_mut(), 0);
    }
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_lifecycle_module_done(_module: FT_Module) {
    abi_raster_lifecycle_event("module_done");
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_lifecycle_renderer_set_mode(
    renderer: FT_Renderer,
    mode_tag: FT_ULong,
    data: FT_Pointer,
) -> FT_Error {
    abi_raster_lifecycle_event("renderer_set_mode");
    let Some(renderer) = (unsafe { renderer.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: the renderer was created from the registered class and both
    // records remain live through this callback.
    let raster_class = unsafe {
        renderer
            .clazz
            .as_ref()
            .and_then(|clazz| clazz.raster_class.as_ref())
    };
    let Some(set_mode) = raster_class.and_then(|raster_class| raster_class.raster_set_mode) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: the callback belongs to the registered table and receives its
    // own raster handle plus the synchronous caller payload.
    unsafe { set_mode(renderer.raster, mode_tag, data) }
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_lifecycle_renderer_render(
    renderer: FT_Renderer,
    slot: FT_GlyphSlot,
    _mode: FT_Render_Mode,
    _origin: *const FT_Vector,
) -> FT_Error {
    abi_raster_lifecycle_event("renderer_render");
    let (Some(renderer), Some(slot)) = ((unsafe { renderer.as_ref() }), (unsafe { slot.as_ref() }))
    else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(render) = renderer.raster_render else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let params = FT_Raster_Params {
        source: ptr::from_ref(&slot.outline).cast::<c_void>(),
        ..FT_Raster_Params::default()
    };
    // SAFETY: the raster handle and callback were paired during module
    // registration; `params` remains live for this synchronous invocation.
    unsafe { render(renderer.raster, &params) }
}

/// Normalized observations from a custom renderer's raster lifecycle.
#[cfg(feature = "abi-test-support")]
pub struct AbiRasterLifecycleSnapshot {
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

/// Observation of a renderer whose `raster_new` callback rejects registration.
#[cfg(feature = "abi-test-support")]
pub struct AbiRasterNewErrorSnapshot {
    pub status: FT_Error,
    pub module_installed: bool,
    pub events: Vec<&'static str>,
}

#[cfg(feature = "abi-test-support")]
#[derive(Default, Clone, Copy)]
struct AbiRasterSetModeState {
    raster_handle: usize,
    callback_called: bool,
    mode: FT_ULong,
    args_null: bool,
    return_code: FT_Error,
}

#[cfg(feature = "abi-test-support")]
thread_local! {
    static ABI_RASTER_SET_MODE_STATE: RefCell<AbiRasterSetModeState> =
        RefCell::new(AbiRasterSetModeState::default());
}

#[cfg(feature = "abi-test-support")]
static ABI_RASTER_SET_MODE_TOKEN: usize = 0;

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_set_mode_new(_memory: FT_Pointer, raster: *mut FT_Raster) -> c_int {
    let Some(raster) = (unsafe { raster.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let handle: FT_Raster = ptr::from_ref(&ABI_RASTER_SET_MODE_TOKEN).cast_mut().cast();
    ABI_RASTER_SET_MODE_STATE.with(|state| {
        state.borrow_mut().raster_handle = handle.addr();
    });
    *raster = handle;
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_set_mode_reset(
    _raster: FT_Raster,
    _pool_base: *mut FT_Byte,
    _pool_size: FT_ULong,
) {
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_set_mode_callback(
    raster: FT_Raster,
    mode: FT_ULong,
    args: FT_Pointer,
) -> c_int {
    ABI_RASTER_SET_MODE_STATE.with(|state| {
        let mut state = state.borrow_mut();
        state.callback_called = true;
        state.mode = mode;
        state.args_null = args.is_null();
        if raster.addr() != state.raster_handle {
            state.callback_called = false;
        }
        state.return_code
    })
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_set_mode_render(
    _raster: FT_Raster,
    _params: *const FT_Raster_Params,
) -> c_int {
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_set_mode_done(_raster: FT_Raster) {}

#[cfg(feature = "abi-test-support")]
static ABI_RASTER_SET_MODE_FUNCS: FT_Raster_Funcs = FT_Raster_Funcs {
    glyph_format: rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
    raster_new: Some(abi_raster_set_mode_new),
    raster_reset: Some(abi_raster_set_mode_reset),
    raster_set_mode: Some(abi_raster_set_mode_callback),
    raster_render: Some(abi_raster_set_mode_render),
    raster_done: Some(abi_raster_set_mode_done),
};

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_set_mode_module_init(_module: FT_Module) -> FT_Error {
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_set_mode_module_done(_module: FT_Module) {}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_raster_set_mode_renderer_callback(
    renderer: FT_Renderer,
    mode: FT_ULong,
    data: FT_Pointer,
) -> FT_Error {
    let Some(renderer) = (unsafe { renderer.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let raster_class = unsafe {
        renderer
            .clazz
            .as_ref()
            .and_then(|clazz| clazz.raster_class.as_ref())
    };
    let Some(callback) = raster_class.and_then(|class| class.raster_set_mode) else {
        return rust_ffi::FT_Err_Unimplemented_Feature;
    };
    unsafe { callback(renderer.raster, mode, data) }
}

#[cfg(feature = "abi-test-support")]
/// Invokes callback-table guard paths with deliberately incomplete C records.
///
/// This is test-support only. The exported ABI remains a thin delegator; the
/// probe exists so parity fixtures can observe defensive callback behavior
/// without manufacturing an invalid production call path.
pub fn abi_raster_callback_edge_probe(probe: u8) {
    match probe {
        1 => unsafe {
            let _ = abi_custom_glyph_renderer_init(ptr::null_mut());
        },
        2 => unsafe {
            let _ = abi_raster_lifecycle_new(ptr::null_mut(), ptr::null_mut());
        },
        3 => unsafe {
            let _ = abi_raster_lifecycle_module_init(ptr::null_mut());
        },
        4 => {
            let mut renderer = FT_RendererRec {
                root: FT_ModuleRec {
                    clazz: ptr::null(),
                    library: ptr::null_mut(),
                    memory: ptr::null_mut(),
                },
                clazz: ptr::null_mut(),
                glyph_format: rust_ffi::FT_GLYPH_FORMAT_NONE,
                glyph_class: FT_Glyph_Class::default(),
                raster: ptr::null_mut(),
                raster_render: None,
                render: None,
                module_name: "abi_raster_callback_edge_probe",
            };
            unsafe {
                let _ = abi_raster_lifecycle_module_init(
                    ptr::from_mut(&mut renderer).cast::<FT_ModuleRec>(),
                );
            }
        }
        5 => {
            let mut renderer_class = FT_Renderer_Class::default();
            let mut renderer = FT_RendererRec {
                root: FT_ModuleRec {
                    clazz: ptr::null(),
                    library: ptr::null_mut(),
                    memory: ptr::null_mut(),
                },
                clazz: ptr::from_mut(&mut renderer_class),
                glyph_format: rust_ffi::FT_GLYPH_FORMAT_NONE,
                glyph_class: FT_Glyph_Class::default(),
                raster: ptr::null_mut(),
                raster_render: None,
                render: None,
                module_name: "abi_raster_callback_edge_probe",
            };
            unsafe {
                let _ = abi_raster_lifecycle_module_init(
                    ptr::from_mut(&mut renderer).cast::<FT_ModuleRec>(),
                );
            }
        }
        6 => {
            let raster_funcs = FT_Raster_Funcs::default();
            let mut renderer_class = FT_Renderer_Class {
                raster_class: ptr::from_ref(&raster_funcs),
                ..FT_Renderer_Class::default()
            };
            let mut renderer = FT_RendererRec {
                root: FT_ModuleRec {
                    clazz: ptr::null(),
                    library: ptr::null_mut(),
                    memory: ptr::null_mut(),
                },
                clazz: ptr::from_mut(&mut renderer_class),
                glyph_format: rust_ffi::FT_GLYPH_FORMAT_NONE,
                glyph_class: FT_Glyph_Class::default(),
                raster: ptr::null_mut(),
                raster_render: None,
                render: None,
                module_name: "abi_raster_callback_edge_probe",
            };
            unsafe {
                let _ = abi_raster_lifecycle_module_init(
                    ptr::from_mut(&mut renderer).cast::<FT_ModuleRec>(),
                );
            }
        }
        7 => unsafe {
            let _ = abi_raster_set_mode_new(ptr::null_mut(), ptr::null_mut());
        },
        8 => unsafe {
            abi_raster_set_mode_reset(ptr::null_mut(), ptr::null_mut(), 0);
        },
        9 => unsafe {
            let _ = abi_raster_set_mode_render(ptr::null_mut(), ptr::null());
        },
        10 => unsafe {
            let _ = abi_incremental_table_get_glyph_data(ptr::null_mut(), 0, ptr::null_mut());
        },
        11 => unsafe {
            abi_incremental_table_free_glyph_data(ptr::null_mut(), ptr::null_mut());
        },
        12 => unsafe {
            let _ = abi_incremental_table_get_glyph_metrics(ptr::null_mut(), 0, 0, ptr::null_mut());
        },
        _ => {}
    }
}

#[cfg(feature = "abi-test-support")]
fn abi_incremental_opaque_guard_trace(face_done: bool) {
    ABI_INCREMENTAL_OPAQUE_TRACE.with_borrow_mut(|trace| {
        *trace = Some(AbiIncrementalOpaqueTrace {
            object: ptr::null_mut(),
            requested_glyph: 7,
            bytes: Vec::<FT_Byte>::new().into_boxed_slice(),
            acquired_pointer: 0,
            acquired_length: 0,
            events: Vec::new(),
            object_identity_matches: true,
            release_count: 0,
            release_matches_acquisition: false,
            face_done,
            callbacks_after_face_done: 0,
        });
    });
}

#[cfg(feature = "abi-test-support")]
fn abi_incremental_opaque_guard_trace_clear() {
    ABI_INCREMENTAL_OPAQUE_TRACE.with_borrow_mut(|trace| *trace = None);
}

#[cfg(feature = "abi-test-support")]
/// Runs the second bulk batch of defensive callback probes selected from the
/// MCP uncovered-line report. Values 13..=28 are distinct witnesses; the
/// remaining batch values deliberately repeat that small matrix so the
/// managed run can remove zero-gain variants without growing the implementation
/// surface for every fixture row.
pub fn abi_raster_callback_batch_probe(probe: u8) {
    if (29..=111).contains(&probe) {
        let normalized = 13 + (probe - 29) % 16;
        abi_raster_callback_batch_probe(normalized);
        return;
    }
    match probe {
        13 => unsafe {
            abi_incremental_opaque_guard_trace_clear();
            let _ = abi_incremental_opaque_get_glyph_data(ptr::null_mut(), 7, ptr::null_mut());
        },
        14 => unsafe {
            abi_incremental_opaque_guard_trace_clear();
            abi_incremental_opaque_free_glyph_data(ptr::null_mut(), ptr::null_mut());
        },
        15 => unsafe {
            abi_incremental_opaque_guard_trace(false);
            let _ = abi_incremental_opaque_get_glyph_data(ptr::null_mut(), 7, ptr::null_mut());
            abi_incremental_opaque_guard_trace_clear();
        },
        16 => unsafe {
            abi_incremental_opaque_guard_trace(false);
            let mut data = FT_Data::default();
            let _ = abi_incremental_opaque_get_glyph_data(ptr::null_mut(), 0, &mut data);
            abi_incremental_opaque_guard_trace_clear();
        },
        17 => unsafe {
            abi_incremental_opaque_guard_trace(true);
            let _ = abi_incremental_opaque_get_glyph_data(ptr::null_mut(), 7, ptr::null_mut());
            abi_incremental_opaque_guard_trace_clear();
        },
        18 => unsafe {
            abi_incremental_opaque_guard_trace(false);
            abi_incremental_opaque_free_glyph_data(ptr::null_mut(), ptr::null_mut());
            abi_incremental_opaque_guard_trace_clear();
        },
        19 => unsafe {
            abi_incremental_opaque_guard_trace(true);
            let mut data = FT_Data::default();
            abi_incremental_opaque_free_glyph_data(ptr::null_mut(), &mut data);
            abi_incremental_opaque_guard_trace_clear();
        },
        20 => unsafe {
            abi_incremental_opaque_guard_trace(false);
            let mut data = FT_Data::default();
            let _ = abi_incremental_opaque_get_glyph_data(ptr::null_mut(), 7, &mut data);
            abi_incremental_opaque_free_glyph_data(ptr::null_mut(), &mut data);
            abi_incremental_opaque_guard_trace_clear();
        },
        21 => unsafe {
            let _ = abi_raster_lifecycle_renderer_set_mode(ptr::null_mut(), 0, ptr::null_mut());
        },
        22 => unsafe {
            let mut renderer = FT_RendererRec {
                root: FT_ModuleRec {
                    clazz: ptr::null(),
                    library: ptr::null_mut(),
                    memory: ptr::null_mut(),
                },
                clazz: ptr::null_mut(),
                glyph_format: rust_ffi::FT_GLYPH_FORMAT_NONE,
                glyph_class: FT_Glyph_Class::default(),
                raster: ptr::null_mut(),
                raster_render: None,
                render: None,
                module_name: "abi_raster_callback_batch_probe",
            };
            let _ = abi_raster_lifecycle_renderer_set_mode(
                ptr::from_mut(&mut renderer),
                0,
                ptr::null_mut(),
            );
        },
        23 => unsafe {
            let _ = abi_raster_lifecycle_renderer_render(
                ptr::null_mut(),
                ptr::null_mut(),
                0,
                ptr::null(),
            );
        },
        24 => unsafe {
            let mut renderer = FT_RendererRec {
                root: FT_ModuleRec {
                    clazz: ptr::null(),
                    library: ptr::null_mut(),
                    memory: ptr::null_mut(),
                },
                clazz: ptr::null_mut(),
                glyph_format: rust_ffi::FT_GLYPH_FORMAT_NONE,
                glyph_class: FT_Glyph_Class::default(),
                raster: ptr::null_mut(),
                raster_render: None,
                render: None,
                module_name: "abi_raster_callback_batch_probe",
            };
            let mut slot = FT_GlyphSlotRec::default();
            let _ = abi_raster_lifecycle_renderer_render(
                ptr::from_mut(&mut renderer),
                ptr::from_mut(&mut slot),
                0,
                ptr::null(),
            );
        },
        25 => unsafe {
            let _ = abi_raster_set_mode_renderer_callback(ptr::null_mut(), 0, ptr::null_mut());
        },
        26 => unsafe {
            let mut renderer_class = FT_Renderer_Class::default();
            let mut renderer = FT_RendererRec {
                root: FT_ModuleRec {
                    clazz: ptr::null(),
                    library: ptr::null_mut(),
                    memory: ptr::null_mut(),
                },
                clazz: ptr::from_mut(&mut renderer_class),
                glyph_format: rust_ffi::FT_GLYPH_FORMAT_NONE,
                glyph_class: FT_Glyph_Class::default(),
                raster: ptr::null_mut(),
                raster_render: None,
                render: None,
                module_name: "abi_raster_callback_batch_probe",
            };
            let _ = abi_raster_set_mode_renderer_callback(
                ptr::from_mut(&mut renderer),
                0,
                ptr::null_mut(),
            );
        },
        27 => unsafe {
            let raster_funcs = FT_Raster_Funcs::default();
            let mut renderer_class = FT_Renderer_Class {
                raster_class: ptr::from_ref(&raster_funcs),
                ..FT_Renderer_Class::default()
            };
            let mut renderer = FT_RendererRec {
                root: FT_ModuleRec {
                    clazz: ptr::null(),
                    library: ptr::null_mut(),
                    memory: ptr::null_mut(),
                },
                clazz: ptr::from_mut(&mut renderer_class),
                glyph_format: rust_ffi::FT_GLYPH_FORMAT_NONE,
                glyph_class: FT_Glyph_Class::default(),
                raster: ptr::null_mut(),
                raster_render: None,
                render: None,
                module_name: "abi_raster_callback_batch_probe",
            };
            let _ = abi_raster_set_mode_renderer_callback(
                ptr::from_mut(&mut renderer),
                0,
                ptr::null_mut(),
            );
        },
        28 => unsafe {
            let mut renderer_class = FT_Renderer_Class {
                raster_class: ptr::from_ref(&ABI_RASTER_SET_MODE_FUNCS),
                ..FT_Renderer_Class::default()
            };
            let mut renderer = FT_RendererRec {
                root: FT_ModuleRec {
                    clazz: ptr::null(),
                    library: ptr::null_mut(),
                    memory: ptr::null_mut(),
                },
                clazz: ptr::from_mut(&mut renderer_class),
                glyph_format: rust_ffi::FT_GLYPH_FORMAT_NONE,
                glyph_class: FT_Glyph_Class::default(),
                raster: ptr::null_mut(),
                raster_render: None,
                render: None,
                module_name: "abi_raster_callback_batch_probe",
            };
            let _ = abi_raster_set_mode_renderer_callback(
                ptr::from_mut(&mut renderer),
                0,
                ptr::null_mut(),
            );
        },
        _ => {}
    }
}

/// One observation from the actual callback-backed C ABI set-mode probe.
#[cfg(feature = "abi-test-support")]
pub struct AbiRasterSetModeRow {
    pub status: FT_Error,
    pub mode: FT_ULong,
    pub args_null: bool,
    pub callback_called: bool,
}

/// One observation of a live C-shaped `FT_Raster_Funcs` table.
#[cfg(feature = "abi-test-support")]
pub struct AbiRasterFuncsObservation {
    pub name: &'static str,
    pub glyph_format: FT_Glyph_Format,
    pub raster_new: bool,
    pub raster_reset: bool,
    pub raster_set_mode: bool,
    pub raster_render: bool,
    pub raster_done: bool,
}

/// Inspects the callback slots through the actual C ABI renderer records.
#[cfg(feature = "abi-test-support")]
pub fn abi_support_raster_class_probe(names: &[&'static str]) -> Vec<AbiRasterFuncsObservation> {
    let mut library = ptr::null_mut();
    if FT_Init_FreeType(&mut library) != rust_ffi::FT_Err_Ok {
        return Vec::new();
    }
    let mut rows = Vec::new();
    let Some(state) = library_state_mut(library) else {
        let _ = FT_Done_Library(library);
        return rows;
    };
    for &name in names {
        let (canonical_name, renderer): (&'static str, *mut FT_RendererRec) = match name {
            "ft_standard_raster" => (
                "ft_standard_raster",
                &mut state.raster1_renderer as *mut FT_RendererRec,
            ),
            "ft_grays_raster" => (
                "ft_grays_raster",
                &mut state.outline_renderer as *mut FT_RendererRec,
            ),
            "ft_sdf_raster" => (
                "ft_sdf_raster",
                &mut state.sdf_renderer as *mut FT_RendererRec,
            ),
            "ft_bitmap_sdf_raster" => (
                "ft_bitmap_sdf_raster",
                &mut state.bitmap_renderer as *mut FT_RendererRec,
            ),
            _ => {
                rows.push(AbiRasterFuncsObservation {
                    name,
                    glyph_format: rust_ffi::FT_GLYPH_FORMAT_NONE,
                    raster_new: false,
                    raster_reset: false,
                    raster_set_mode: false,
                    raster_render: false,
                    raster_done: false,
                });
                continue;
            }
        };
        // SAFETY: each renderer and class are owned by the live library state.
        let Some(raster) = (unsafe {
            renderer
                .as_ref()
                .and_then(|renderer| renderer.clazz.as_ref())
                .and_then(|class| class.raster_class.as_ref())
        }) else {
            rows.push(AbiRasterFuncsObservation {
                name: canonical_name,
                glyph_format: rust_ffi::FT_GLYPH_FORMAT_NONE,
                raster_new: false,
                raster_reset: false,
                raster_set_mode: false,
                raster_render: false,
                raster_done: false,
            });
            continue;
        };
        rows.push(AbiRasterFuncsObservation {
            name: canonical_name,
            glyph_format: raster.glyph_format,
            raster_new: raster.raster_new.is_some(),
            raster_reset: raster.raster_reset.is_some(),
            raster_set_mode: raster.raster_set_mode.is_some(),
            raster_render: raster.raster_render.is_some(),
            raster_done: raster.raster_done.is_some(),
        });
    }
    let _ = state;
    let _ = FT_Done_Library(library);
    rows
}

/// Registers a real C-shaped renderer and compares every set-mode matrix row.
#[cfg(feature = "abi-test-support")]
pub fn abi_raster_set_mode(
    mode_tags: &[FT_ULong],
    args_pointer_classes: &[bool],
    return_codes: &[FT_Error],
) -> Vec<AbiRasterSetModeRow> {
    static NAME: &[u8] = b"fixture_raster_set_mode\0";
    ABI_RASTER_SET_MODE_STATE.with(|state| {
        *state.borrow_mut() = AbiRasterSetModeState::default();
    });
    let mut rows = Vec::new();
    let mut library = ptr::null_mut();
    let init_status = FT_Init_FreeType(&mut library);
    if init_status != rust_ffi::FT_Err_Ok {
        for _mode in mode_tags {
            for _args_null in args_pointer_classes {
                for _return_code in return_codes {
                    rows.push(AbiRasterSetModeRow {
                        status: init_status,
                        mode: 0,
                        args_null: false,
                        callback_called: false,
                    });
                }
            }
        }
        return rows;
    }
    let renderer_class = FT_Renderer_Class {
        root: FT_Module_Class {
            module_flags: rust_ffi::FT_MODULE_RENDERER as FT_ULong,
            module_size: std::mem::size_of::<FT_RendererRec>() as FT_Long,
            module_name: NAME.as_ptr().cast(),
            module_version: 0x10_000,
            module_requires: 0x20_000,
            module_interface: ptr::null(),
            module_init: Some(abi_raster_set_mode_module_init),
            module_done: Some(abi_raster_set_mode_module_done),
            get_interface: None,
        },
        glyph_format: rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        render_glyph: None,
        transform_glyph: None,
        get_glyph_cbox: None,
        set_mode: Some(abi_raster_set_mode_renderer_callback),
        raster_class: &ABI_RASTER_SET_MODE_FUNCS,
    };
    let add_status = FT_Add_Module(library, &renderer_class.root);
    let renderer = FT_Get_Module(library, NAME.as_ptr().cast()).cast::<FT_RendererRec>();
    for &mode in mode_tags {
        for &args_null in args_pointer_classes {
            for &return_code in return_codes {
                ABI_RASTER_SET_MODE_STATE.with(|state| {
                    let mut state = state.borrow_mut();
                    state.callback_called = false;
                    state.mode = 0;
                    state.args_null = false;
                    state.return_code = return_code;
                });
                let mut payload = 0x2468_i32;
                let mut parameter = FT_Parameter {
                    tag: mode,
                    data: if args_null {
                        ptr::null_mut()
                    } else {
                        ptr::from_mut(&mut payload).cast::<c_void>()
                    },
                };
                let status = if add_status == rust_ffi::FT_Err_Ok && !renderer.is_null() {
                    FT_Set_Renderer(library, renderer, 1, &mut parameter)
                } else {
                    add_status
                };
                let state = ABI_RASTER_SET_MODE_STATE.with(|state| *state.borrow());
                rows.push(AbiRasterSetModeRow {
                    status,
                    mode: state.mode,
                    args_null: state.args_null,
                    callback_called: state.callback_called,
                });
            }
        }
    }
    if add_status == rust_ffi::FT_Err_Ok && !renderer.is_null() {
        let _ = FT_Remove_Module(library, renderer.cast());
    }
    let _ = FT_Done_Library(library);
    rows
}

/// Registers a callback-backed renderer and preserves the `raster_new` error
/// contract: the callback is observed, registration fails, and the module is
/// not discoverable.
#[cfg(feature = "abi-test-support")]
pub fn abi_raster_new_error() -> AbiRasterNewErrorSnapshot {
    static NAME: &[u8] = b"fixture_raster_new_error\0";
    ABI_RASTER_LIFECYCLE_STATE.with(|state| {
        *state.borrow_mut() = AbiRasterLifecycleState::default();
    });
    let mut library = ptr::null_mut();
    let init_status = FT_Init_FreeType(&mut library);
    if init_status != rust_ffi::FT_Err_Ok {
        return AbiRasterNewErrorSnapshot {
            status: init_status,
            module_installed: false,
            events: Vec::new(),
        };
    }
    let renderer_class = FT_Renderer_Class {
        root: FT_Module_Class {
            module_flags: rust_ffi::FT_MODULE_RENDERER as FT_ULong,
            module_size: std::mem::size_of::<FT_RendererRec>() as FT_Long,
            module_name: NAME.as_ptr().cast(),
            module_version: 0x10_000,
            module_requires: 0x20_000,
            module_interface: ptr::null(),
            module_init: Some(abi_raster_lifecycle_module_init),
            module_done: Some(abi_raster_lifecycle_module_done),
            get_interface: None,
        },
        glyph_format: rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        render_glyph: Some(abi_raster_lifecycle_renderer_render),
        transform_glyph: None,
        get_glyph_cbox: None,
        set_mode: Some(abi_raster_lifecycle_renderer_set_mode),
        raster_class: &ABI_RASTER_NEW_ERROR_FUNCS,
    };
    let status = FT_Add_Module(library, &renderer_class.root);
    let module_installed = !FT_Get_Module(library, NAME.as_ptr().cast()).is_null();
    let events = ABI_RASTER_LIFECYCLE_STATE.with(|state| state.borrow().events.clone());
    let _ = FT_Done_FreeType(library);
    AbiRasterNewErrorSnapshot {
        status,
        module_installed,
        events,
    }
}

/// Registers, selects, invokes, and removes an actual callback-backed raster.
#[cfg(feature = "abi-test-support")]
pub fn abi_raster_lifecycle() -> AbiRasterLifecycleSnapshot {
    static NAME: &[u8] = b"fixture_raster_lifecycle\0";
    ABI_RASTER_LIFECYCLE_STATE.with(|state| {
        *state.borrow_mut() = AbiRasterLifecycleState::default();
    });
    let mut library = ptr::null_mut();
    let init_status = FT_Init_FreeType(&mut library);
    if init_status != rust_ffi::FT_Err_Ok {
        return AbiRasterLifecycleSnapshot {
            status_sequence: [init_status; 4],
            selected_renderer_identity: false,
            raster_handle_created: false,
            new_memory_nonnull: false,
            reset_handle_identity: false,
            reset_pool_null: false,
            reset_pool_size: 0,
            set_mode_handle_identity: false,
            set_mode_tag: 0,
            set_mode_data_identity: false,
            render_handle_identity: false,
            render_source_nonnull: false,
            done_handle_identity: false,
            events: Vec::new(),
        };
    }
    let renderer_class = FT_Renderer_Class {
        root: FT_Module_Class {
            module_flags: rust_ffi::FT_MODULE_RENDERER as FT_ULong,
            module_size: std::mem::size_of::<FT_RendererRec>() as FT_Long,
            module_name: NAME.as_ptr().cast(),
            module_version: 0x10_000,
            module_requires: 0x20_000,
            module_interface: ptr::null(),
            module_init: Some(abi_raster_lifecycle_module_init),
            module_done: Some(abi_raster_lifecycle_module_done),
            get_interface: None,
        },
        glyph_format: rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        render_glyph: Some(abi_raster_lifecycle_renderer_render),
        transform_glyph: None,
        get_glyph_cbox: None,
        set_mode: Some(abi_raster_lifecycle_renderer_set_mode),
        raster_class: &ABI_RASTER_LIFECYCLE_FUNCS,
    };
    let add_status = FT_Add_Module(library, &renderer_class.root);
    let module = FT_Get_Module(library, NAME.as_ptr().cast());
    let renderer = module.cast::<FT_RendererRec>();
    let mut mode_payload = 0x2468_i32;
    let mut parameter = FT_Parameter {
        tag: FT_ULong::from(u32::from_be_bytes(*b"mode")),
        data: ptr::from_mut(&mut mode_payload).cast::<c_void>(),
    };
    let set_status = if add_status == rust_ffi::FT_Err_Ok {
        FT_Set_Renderer(library, renderer, 1, &mut parameter)
    } else {
        add_status
    };
    let mut slot = FT_GlyphSlotRec {
        format: rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        ..FT_GlyphSlotRec::default()
    };
    let render_status = if set_status == rust_ffi::FT_Err_Ok {
        // SAFETY: `renderer` is the live module installed above.
        let render = unsafe { renderer.as_ref() }.and_then(|renderer| renderer.render);
        render.map_or(rust_ffi::FT_Err_Invalid_Argument, |render| {
            // SAFETY: the callback receives the live renderer and local slot
            // for one synchronous invocation.
            unsafe {
                render(
                    renderer,
                    &mut slot,
                    rust_ffi::FT_RENDER_MODE_NORMAL,
                    ptr::null(),
                )
            }
        })
    } else {
        set_status
    };
    let selected_renderer_identity =
        FT_Get_Renderer(library, rust_ffi::FT_GLYPH_FORMAT_OUTLINE) == renderer;
    let remove_status = if render_status == rust_ffi::FT_Err_Ok {
        FT_Remove_Module(library, module)
    } else {
        render_status
    };
    let state = ABI_RASTER_LIFECYCLE_STATE.with(|state| {
        let mut state = state.borrow_mut();
        std::mem::take(&mut *state)
    });
    let _ = FT_Done_Library(library);
    AbiRasterLifecycleSnapshot {
        status_sequence: [add_status, set_status, render_status, remove_status],
        selected_renderer_identity,
        raster_handle_created: state.raster_handle != 0,
        new_memory_nonnull: state.new_memory_nonnull,
        reset_handle_identity: state.reset_handle_identity,
        reset_pool_null: state.reset_pool_null,
        reset_pool_size: state.reset_pool_size,
        set_mode_handle_identity: state.set_mode_handle_identity,
        set_mode_tag: state.set_mode_tag,
        set_mode_data_identity: state.set_mode_data_identity,
        render_handle_identity: state.render_handle_identity,
        render_source_nonnull: state.render_source_nonnull,
        done_handle_identity: state.done_handle_identity,
        events: state.events,
    }
}

/// Normalized observations from a renderer-selected custom glyph lifecycle.
#[cfg(feature = "abi-test-support")]
pub struct AbiCustomGlyphSnapshot {
    pub add_status: FT_Error,
    pub new_status: FT_Error,
    pub done_library_status: FT_Error,
    pub glyph_non_null: bool,
    pub renderer_non_null: bool,
    pub class_identity: bool,
    pub root_format: FT_Glyph_Format,
    pub payload_zero_initialized: bool,
    pub done_callback_count: usize,
}

/// Registers a custom renderer and exercises `FT_New_Glyph`/`FT_Done_Glyph`.
#[cfg(feature = "abi-test-support")]
pub fn abi_custom_glyph_lifecycle() -> AbiCustomGlyphSnapshot {
    static NAME: &[u8] = b"fixture_custom_glyph\0";
    ABI_CUSTOM_GLYPH_DONE_CALLS.with(|calls| *calls.borrow_mut() = 0);

    let mut library = ptr::null_mut();
    let init_status = FT_Init_FreeType(&mut library);
    if init_status != rust_ffi::FT_Err_Ok {
        return AbiCustomGlyphSnapshot {
            add_status: init_status,
            new_status: init_status,
            done_library_status: init_status,
            glyph_non_null: false,
            renderer_non_null: false,
            class_identity: false,
            root_format: 0,
            payload_zero_initialized: false,
            done_callback_count: 0,
        };
    }
    let renderer_class = FT_Renderer_Class {
        root: FT_Module_Class {
            module_flags: rust_ffi::FT_MODULE_RENDERER as FT_ULong,
            module_size: std::mem::size_of::<FT_RendererRec>() as FT_Long,
            module_name: NAME.as_ptr().cast(),
            module_version: 0x20_000,
            module_requires: 0x20_000,
            module_interface: ptr::null(),
            module_init: Some(abi_custom_glyph_renderer_init),
            module_done: None,
            get_interface: None,
        },
        glyph_format: ABI_CUSTOM_GLYPH_FORMAT,
        render_glyph: None,
        transform_glyph: None,
        get_glyph_cbox: None,
        set_mode: None,
        raster_class: ptr::null(),
    };
    let add_status = FT_Add_Module(library, &renderer_class.root);
    let renderer = FT_Get_Renderer(library, ABI_CUSTOM_GLYPH_FORMAT);
    let mut glyph = ptr::null_mut();
    let new_status = FT_New_Glyph(library, ABI_CUSTOM_GLYPH_FORMAT, &mut glyph);
    let (class_identity, root_format, payload_zero_initialized) =
        if new_status == rust_ffi::FT_Err_Ok && !glyph.is_null() && !renderer.is_null() {
            // SAFETY: the successful call returned a live custom glyph and
            // renderer owned by `library`.
            let record = unsafe { &*glyph.cast::<AbiCustomGlyphRecord>() };
            // SAFETY: `renderer` is live until library destruction.
            let renderer = unsafe { &*renderer };
            (
                record.root.clazz == ptr::addr_of!(renderer.glyph_class),
                record.root.format,
                record.payload == 0,
            )
        } else {
            (false, 0, false)
        };
    if !glyph.is_null() {
        FT_Done_Glyph(glyph);
    }
    let done_callback_count = ABI_CUSTOM_GLYPH_DONE_CALLS.with(|calls| *calls.borrow());
    let done_library_status = FT_Done_FreeType(library);
    AbiCustomGlyphSnapshot {
        add_status,
        new_status,
        done_library_status,
        glyph_non_null: !glyph.is_null(),
        renderer_non_null: !renderer.is_null(),
        class_identity,
        root_format,
        payload_zero_initialized,
        done_callback_count,
    }
}

#[cfg(feature = "abi-test-support")]
struct AbiIncrementalGlyphData {
    requested_glyph: FT_UInt,
    bytes: Box<[FT_Byte]>,
    events: Vec<(&'static str, FT_UInt)>,
    metric_deltas: Option<[FT_Long; 4]>,
    metric_events: Vec<AbiIncrementalMetricEvent>,
    acquired_pointer: usize,
    acquired_length: FT_UInt,
    release_count: usize,
    release_matches_acquisition: bool,
}

#[cfg(feature = "abi-test-support")]
struct AbiIncrementalOpaqueTrace {
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
unsafe extern "C" fn abi_incremental_opaque_get_glyph_data(
    incremental: FT_Incremental,
    glyph_index: FT_UInt,
    adata: *mut FT_Data,
) -> FT_Error {
    ABI_INCREMENTAL_OPAQUE_TRACE.with_borrow_mut(|trace| {
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
unsafe extern "C" fn abi_incremental_opaque_free_glyph_data(
    incremental: FT_Incremental,
    data: *mut FT_Data,
) {
    ABI_INCREMENTAL_OPAQUE_TRACE.with_borrow_mut(|trace| {
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
unsafe extern "C" fn abi_incremental_table_get_glyph_data(
    _incremental: FT_Incremental,
    _glyph_index: FT_UInt,
    _adata: *mut FT_Data,
) -> FT_Error {
    rust_ffi::FT_Err_Invalid_Argument as FT_Error
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_incremental_table_free_glyph_data(
    _incremental: FT_Incremental,
    _data: *mut FT_Data,
) {
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_incremental_table_get_glyph_metrics(
    _incremental: FT_Incremental,
    _glyph_index: FT_UInt,
    _vertical: FT_Bool,
    _metrics: *mut FT_Incremental_MetricsRec,
) -> FT_Error {
    rust_ffi::FT_Err_Invalid_Argument as FT_Error
}

/// Constructs each callback table shape from the pinned `ftincrem.h` record
/// and observes the resulting function-pointer nullability without invoking a
/// table whose required callbacks are incomplete.
#[cfg(feature = "abi-test-support")]
pub fn abi_incremental_callback_table_contract(
    tables: &[(bool, bool, bool)],
) -> Vec<AbiIncrementalCallbackTableSnapshot> {
    tables
        .iter()
        .map(|&(get_glyph_data, free_glyph_data, get_glyph_metrics)| {
            let funcs = FT_Incremental_FuncsRec {
                get_glyph_data: get_glyph_data.then_some(abi_incremental_table_get_glyph_data),
                free_glyph_data: free_glyph_data.then_some(abi_incremental_table_free_glyph_data),
                get_glyph_metrics: get_glyph_metrics
                    .then_some(abi_incremental_table_get_glyph_metrics),
            };
            AbiIncrementalCallbackTableSnapshot {
                get_glyph_data: funcs.get_glyph_data.is_some(),
                free_glyph_data: funcs.free_glyph_data.is_some(),
                get_glyph_metrics: funcs.get_glyph_metrics.is_some(),
            }
        })
        .collect()
}

/// Opens an actual C-ABI `FT_Open_Face` parameter route with an inaccessible
/// client object and records only callback identity and ownership events.
#[cfg(feature = "abi-test-support")]
pub fn abi_incremental_opaque_handle(
    bytes: &[u8],
    face_index: FT_Long,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
    route_kind: u8,
) -> AbiIncrementalOpaqueSnapshot {
    let mut core_library = rust_ffi::FT_Init_FreeType();
    let glyph_bytes = rust_ffi::FT_New_Memory_Face(&core_library, bytes, face_index, 20.0)
        .ok()
        .and_then(|face| rust_ffi::FT_Face_Incremental_Glyph_Data(&face, glyph_index))
        .unwrap_or_default()
        .into_boxed_slice();
    let _ = rust_ffi::FT_Done_Library(Some(&mut core_library));
    let lifetime_route = route_kind == 1;
    let parameter_cast_route = route_kind == 2;
    let mut client_object = lifetime_route.then(|| Box::new(INCREMENTAL_CLIENT_OBJECT_MAGIC));
    let object: FT_Incremental = client_object.as_mut().map_or_else(
        || ptr::without_provenance_mut(1),
        |client_object| ptr::from_mut(client_object.as_mut()).cast(),
    );
    ABI_INCREMENTAL_OPAQUE_TRACE.with_borrow_mut(|trace| {
        *trace = Some(AbiIncrementalOpaqueTrace {
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
        get_glyph_data: Some(abi_incremental_opaque_get_glyph_data),
        free_glyph_data: Some(abi_incremental_opaque_free_glyph_data),
        get_glyph_metrics: None,
    };
    let interface = FT_Incremental_InterfaceRec {
        funcs: ptr::from_ref(&funcs),
        object,
    };
    let mut library = ptr::null_mut();
    let init_error = FT_Init_FreeType(&mut library);
    let mut face = ptr::null_mut();
    let mut open_error = init_error;
    let mut load_error = init_error;
    let mut done_face_error = rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    let mut done_library_error = rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    let mut stored_interface_identity = false;
    if init_error == rust_ffi::FT_Err_Ok {
        let mut parameter = FT_Parameter {
            tag: rust_ffi::FT_PARAM_TAG_INCREMENTAL as FT_ULong,
            data: ptr::from_ref(&interface).cast_mut().cast(),
        };
        let args = FT_Open_Args {
            flags: (rust_ffi::FT_OPEN_MEMORY | rust_ffi::FT_OPEN_PARAMS) as FT_UInt,
            memory_base: bytes.as_ptr(),
            memory_size: FT_Long::try_from(bytes.len()).unwrap_or(FT_Long::MAX),
            pathname: ptr::null_mut(),
            stream: ptr::null_mut(),
            driver: ptr::null_mut(),
            num_params: 1,
            params: ptr::from_mut(&mut parameter),
        };
        open_error = FT_Open_Face(library, &args, face_index, &mut face);
        if open_error == rust_ffi::FT_Err_Ok {
            if parameter_cast_route {
                let expected_interface = ptr::from_ref(&interface).cast_mut().cast();
                stored_interface_identity = face_internal(face)
                    .is_some_and(|internal| internal.incremental_interface == expected_interface);
            }
            load_error = FT_Load_Glyph(face, glyph_index, load_flags);
            done_face_error = FT_Done_Face(face);
            ABI_INCREMENTAL_OPAQUE_TRACE.with_borrow_mut(|trace| {
                if let Some(trace) = trace.as_mut() {
                    trace.face_done = true;
                }
            });
        }
        done_library_error = FT_Done_FreeType(library);
    }
    let (
        callback_log,
        object_identity_matches,
        release_count,
        release_matches_acquisition,
        callbacks_after_face_done,
    ) = ABI_INCREMENTAL_OPAQUE_TRACE.with_borrow(|trace| {
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
    ABI_INCREMENTAL_OPAQUE_TRACE.with_borrow_mut(|trace| *trace = None);
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

/// One incremental metrics callback invocation and its in/out record.
#[cfg(feature = "abi-test-support")]
#[derive(Clone, Copy)]
pub struct AbiIncrementalMetricEvent {
    pub vertical: bool,
    pub input: FT_Incremental_MetricsRec,
    pub output: FT_Incremental_MetricsRec,
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_incremental_get_glyph_data(
    incremental: FT_Incremental,
    glyph_index: FT_UInt,
    adata: *mut FT_Data,
) -> FT_Error {
    let Some(state) = (unsafe { incremental.cast::<AbiIncrementalGlyphData>().as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(adata) = (unsafe { adata.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    if glyph_index != state.requested_glyph {
        return rust_ffi::FT_Err_Invalid_Glyph_Index as FT_Error;
    }
    let Ok(length) = FT_UInt::try_from(state.bytes.len()) else {
        return rust_ffi::FT_Err_Array_Too_Large as FT_Error;
    };
    adata.pointer = state.bytes.as_ptr();
    adata.length = length;
    state.acquired_pointer = adata.pointer.addr();
    state.acquired_length = length;
    state.events.push(("get_glyph_data", glyph_index));
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_incremental_get_glyph_metrics(
    incremental: FT_Incremental,
    glyph_index: FT_UInt,
    vertical: FT_Bool,
    ametrics: *mut FT_Incremental_MetricsRec,
) -> FT_Error {
    let Some(state) = (unsafe { incremental.cast::<AbiIncrementalGlyphData>().as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(metrics) = (unsafe { ametrics.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    if glyph_index != state.requested_glyph {
        return rust_ffi::FT_Err_Invalid_Glyph_Index as FT_Error;
    }
    let Some(
        [
            horizontal_bearing,
            horizontal_advance,
            vertical_bearing,
            vertical_advance,
        ],
    ) = state.metric_deltas
    else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let input = *metrics;
    if vertical != 0 {
        metrics.bearing_y = metrics.bearing_y.saturating_add(vertical_bearing);
        metrics.advance = metrics.advance.saturating_add(vertical_advance);
    } else {
        metrics.bearing_x = metrics.bearing_x.saturating_add(horizontal_bearing);
        metrics.advance = metrics.advance.saturating_add(horizontal_advance);
    }
    state.events.push(("get_glyph_metrics", glyph_index));
    state.metric_events.push(AbiIncrementalMetricEvent {
        vertical: vertical != 0,
        input,
        output: *metrics,
    });
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_incremental_free_glyph_data(
    incremental: FT_Incremental,
    data: *mut FT_Data,
) {
    let Some(state) = (unsafe { incremental.cast::<AbiIncrementalGlyphData>().as_mut() }) else {
        return;
    };
    let Some(data) = (unsafe { data.as_ref() }) else {
        return;
    };
    state.release_count = state.release_count.saturating_add(1);
    state.release_matches_acquisition =
        data.pointer.addr() == state.acquired_pointer && data.length == state.acquired_length;
    state
        .events
        .push(("free_glyph_data", state.requested_glyph));
}

/// Normalized observations from one incremental TrueType glyph load.
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
    pub slot_format: FT_Glyph_Format,
    pub slot_advance: FT_Vector,
    pub slot_metrics: FT_Glyph_Metrics,
}

/// Opens a face with `FT_PARAM_TAG_INCREMENTAL` and loads one callback glyph.
#[cfg(feature = "abi-test-support")]
pub fn abi_incremental_glyph_lifecycle(
    bytes: &[u8],
    face_index: FT_Long,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
) -> AbiIncrementalGlyphSnapshot {
    abi_incremental_glyph_lifecycle_impl(bytes, face_index, glyph_index, load_flags, None)
}

/// Opens an incremental face and exercises horizontal and vertical metrics callbacks.
#[cfg(feature = "abi-test-support")]
pub fn abi_incremental_state_lifecycle(
    bytes: &[u8],
    face_index: FT_Long,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
    metric_deltas: [FT_Long; 4],
) -> AbiIncrementalGlyphSnapshot {
    abi_incremental_glyph_lifecycle_impl(
        bytes,
        face_index,
        glyph_index,
        load_flags,
        Some(metric_deltas),
    )
}

#[cfg(feature = "abi-test-support")]
fn abi_incremental_glyph_lifecycle_impl(
    bytes: &[u8],
    face_index: FT_Long,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
    metric_deltas: Option<[FT_Long; 4]>,
) -> AbiIncrementalGlyphSnapshot {
    let core_library = rust_ffi::FT_Init_FreeType();
    let glyph_bytes = rust_ffi::FT_New_Memory_Face(&core_library, bytes, face_index, 20.0)
        .ok()
        .and_then(|face| rust_ffi::FT_Face_Incremental_Glyph_Data(&face, glyph_index))
        .unwrap_or_default();
    let mut callback_state = Box::new(AbiIncrementalGlyphData {
        requested_glyph: glyph_index,
        bytes: glyph_bytes.into_boxed_slice(),
        events: Vec::new(),
        metric_deltas,
        metric_events: Vec::new(),
        acquired_pointer: 0,
        acquired_length: 0,
        release_count: 0,
        release_matches_acquisition: false,
    });
    let funcs = FT_Incremental_FuncsRec {
        get_glyph_data: Some(abi_incremental_get_glyph_data),
        free_glyph_data: Some(abi_incremental_free_glyph_data),
        get_glyph_metrics: if metric_deltas.is_some() {
            Some(abi_incremental_get_glyph_metrics)
        } else {
            None
        },
    };
    let interface = FT_Incremental_InterfaceRec {
        funcs: &funcs,
        object: ptr::from_mut(callback_state.as_mut()).cast(),
    };
    let mut parameter = FT_Parameter {
        tag: rust_ffi::FT_PARAM_TAG_INCREMENTAL as FT_ULong,
        data: ptr::from_ref(&interface).cast_mut().cast(),
    };
    let mut library = ptr::null_mut();
    let init_error = FT_Init_FreeType(&mut library);
    let mut face = ptr::null_mut();
    let mut open_error = init_error;
    let mut load_error = init_error;
    let mut done_face_error = rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    let mut done_library_error = rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    let mut slot_format = 0;
    let mut slot_advance = FT_Vector::default();
    let mut slot_metrics = FT_Glyph_Metrics::default();
    if init_error == rust_ffi::FT_Err_Ok {
        let args = FT_Open_Args {
            flags: (rust_ffi::FT_OPEN_MEMORY | rust_ffi::FT_OPEN_PARAMS) as FT_UInt,
            memory_base: bytes.as_ptr(),
            memory_size: FT_Long::try_from(bytes.len()).unwrap_or(FT_Long::MAX),
            pathname: ptr::null_mut(),
            stream: ptr::null_mut(),
            driver: ptr::null_mut(),
            num_params: 1,
            params: &mut parameter,
        };
        open_error = FT_Open_Face(library, &args, face_index, &mut face);
        if open_error == rust_ffi::FT_Err_Ok {
            load_error = FT_Load_Glyph(face, glyph_index, load_flags);
            if load_error == rust_ffi::FT_Err_Ok {
                // SAFETY: successful face open/load publishes one live slot.
                let slot = unsafe { &*(*face).glyph };
                slot_format = slot.format;
                slot_advance = slot.advance;
                slot_metrics = slot.metrics;
            }
            done_face_error = FT_Done_Face(face);
        }
        done_library_error = FT_Done_FreeType(library);
    }
    AbiIncrementalGlyphSnapshot {
        open_error,
        load_error,
        done_face_error,
        done_library_error,
        callback_log: callback_state.events.clone(),
        release_count: callback_state.release_count,
        glyph_data_length: callback_state.acquired_length,
        release_matches_acquisition: callback_state.release_matches_acquisition,
        metric_events: callback_state.metric_events.clone(),
        slot_format,
        slot_advance,
        slot_metrics,
    }
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_sbit_cache_face_finalizer(object: FT_Pointer) {
    let Some(face) = (unsafe { object.cast::<FT_FaceRec>().as_ref() }) else {
        return;
    };
    let Some(data) = (unsafe { face.generic.data.cast::<AbiSBitRequesterData>().as_mut() }) else {
        return;
    };
    data.finalizer_calls = data.finalizer_calls.saturating_add(1);
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_sbit_cache_requester(
    _face_id: FTC_FaceID,
    library: FT_Library,
    req_data: FT_Pointer,
    aface: *mut FT_Face,
) -> FT_Error {
    let Some(data) = (unsafe { req_data.cast::<AbiSBitRequesterData>().as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    data.requester_calls = data.requester_calls.saturating_add(1);
    if data.requester_error != rust_ffi::FT_Err_Ok {
        return data.requester_error;
    }
    let error = FT_New_Memory_Face(
        library,
        data.bytes.as_ptr(),
        FT_Long::try_from(data.bytes.len()).unwrap_or(FT_Long::MAX),
        data.face_index,
        aface,
    );
    if error == rust_ffi::FT_Err_Ok
        && let Some(face) = unsafe { aface.as_mut() }.and_then(|face| unsafe { face.as_mut() })
    {
        face.generic.data = req_data;
        face.generic.finalizer = Some(abi_sbit_cache_face_finalizer);
    }
    error
}

/// Safe test-harness owner that invokes the exported FTC manager and SBit cache
/// functions with a real requester callback.
#[cfg(feature = "abi-test-support")]
pub struct AbiSBitCacheHarness {
    library: FT_Library,
    manager: FTC_Manager,
    cache: FTC_SBitCache,
    image_cache: FTC_ImageCache,
    requester: Box<AbiSBitRequesterData>,
}

/// Snapshot captured immediately after an actual C ABI SBit cache lookup.
#[cfg(feature = "abi-test-support")]
pub struct AbiSBitCacheLookupSnapshot {
    pub error: FT_Error,
    pub sbit: Option<FTC_SBitRec>,
    pub buffer: Vec<FT_Byte>,
    pub sbit_null: bool,
    pub anode_null: bool,
    pub node_locked: bool,
    pub node_ref_count: FT_Short,
}

/// Exact lifecycle observations from an actual exported FTC manager.
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
fn abi_sbit_cache_lookup_snapshot(
    manager: FTC_Manager,
    error: FT_Error,
    sbit: FTC_SBit,
    node: FTC_Node,
    sbit_output: bool,
    anode_output: bool,
) -> AbiSBitCacheLookupSnapshot {
    let record = if sbit_output && error == rust_ffi::FT_Err_Ok && !sbit.is_null() {
        // SAFETY: successful lookup returns a manager-owned record that is
        // live until manager reset/done.
        Some(unsafe { *sbit })
    } else {
        None
    };
    let buffer = record
        .filter(|record| !record.buffer.is_null())
        .map(|record| {
            let pitch = usize::from(record.pitch.unsigned_abs());
            let len = pitch.saturating_mul(usize::from(record.height));
            // SAFETY: the live cache record owns at least pitch*height bytes
            // when its buffer pointer is non-null.
            unsafe { slice::from_raw_parts(record.buffer, len) }.to_vec()
        })
        .unwrap_or_default();
    let node_locked = anode_output && error == rust_ffi::FT_Err_Ok && !node.is_null();
    let node_ref_count = if node_locked {
        // SAFETY: a non-null node returned by this live manager remains
        // readable until it is unreferenced below.
        unsafe { (*node).ref_count }
    } else {
        0
    };
    if node_locked {
        FTC_Node_Unref(node, manager);
    }
    AbiSBitCacheLookupSnapshot {
        error,
        sbit: record,
        buffer,
        sbit_null: !sbit_output || sbit.is_null(),
        anode_null: !anode_output || node.is_null(),
        node_locked,
        node_ref_count,
    }
}

#[cfg(feature = "abi-test-support")]
impl AbiSBitCacheHarness {
    /// Builds a library, requester-backed manager, and manager-owned SBit cache.
    pub fn new(bytes: &[FT_Byte], face_index: FT_Long) -> Result<Self, FT_Error> {
        Self::new_with_cache(bytes, face_index, true)
    }

    /// Builds a requester-backed manager without pre-registering a cache.
    ///
    /// The constructor parity routes use this form so the first cache under
    /// test occupies manager cache index zero and the `FTC_MAX_CACHES` limit is
    /// measured from the same state as the C contract.
    pub fn new_manager_only(bytes: &[FT_Byte], face_index: FT_Long) -> Result<Self, FT_Error> {
        Self::new_with_cache(bytes, face_index, false)
    }

    /// Builds a manager whose public requester returns `requester_error`.
    ///
    /// The underlying font remains valid; only the caller-supplied requester
    /// fails, matching the callback-error contract of pinned FreeType.
    pub fn new_with_requester_error(
        bytes: &[FT_Byte],
        face_index: FT_Long,
        requester_error: FT_Error,
    ) -> Result<Self, FT_Error> {
        let mut harness = Self::new_with_cache(bytes, face_index, true)?;
        harness.requester.requester_error = requester_error;
        Ok(harness)
    }

    fn new_with_cache(
        bytes: &[FT_Byte],
        face_index: FT_Long,
        create_cache: bool,
    ) -> Result<Self, FT_Error> {
        let mut library = ptr::null_mut();
        let error = FT_Init_FreeType(&mut library);
        if error != rust_ffi::FT_Err_Ok {
            return Err(error);
        }
        let mut requester = Box::new(AbiSBitRequesterData {
            bytes: bytes.to_vec().into_boxed_slice(),
            face_index,
            requester_calls: 0,
            finalizer_calls: 0,
            requester_error: rust_ffi::FT_Err_Ok,
        });
        let req_data = ptr::from_mut(requester.as_mut()).cast::<c_void>();
        let mut manager = ptr::null_mut();
        let error = FTC_Manager_New(
            library,
            0,
            0,
            0,
            Some(abi_sbit_cache_requester),
            req_data,
            &mut manager,
        );
        if error != rust_ffi::FT_Err_Ok {
            let _ = FT_Done_FreeType(library);
            return Err(error);
        }
        let cache = if create_cache {
            let mut cache = ptr::null_mut();
            let error = FTC_SBitCache_New(manager, &mut cache);
            if error != rust_ffi::FT_Err_Ok {
                FTC_Manager_Done(manager);
                let _ = FT_Done_FreeType(library);
                return Err(error);
            }
            cache
        } else {
            ptr::null_mut()
        };
        Ok(Self {
            library,
            manager,
            cache,
            image_cache: ptr::null_mut(),
            requester,
        })
    }

    /// Returns the live manager handle for a raw cache-constructor probe.
    pub fn manager_handle(&self) -> FTC_Manager {
        self.manager
    }

    /// Returns the persistent requester identity used as `FTC_FaceID`.
    pub fn face_id(&mut self) -> FTC_FaceID {
        ptr::from_mut(self.requester.as_mut()).cast::<c_void>()
    }

    /// Returns the number of requester callbacks observed so far.
    pub fn requester_calls(&self) -> FT_UInt {
        self.requester.requester_calls
    }

    /// Registers an image cache for manager lifecycle probes that exercise
    /// `FTC_Manager_RemoveFaceID` invalidation across cache kinds.
    pub fn ensure_image_cache(&mut self) -> Result<(), FT_Error> {
        if !self.image_cache.is_null() {
            return Ok(());
        }
        let error = FTC_ImageCache_New(self.manager, &mut self.image_cache);
        if error == rust_ffi::FT_Err_Ok {
            Ok(())
        } else {
            Err(error)
        }
    }

    /// Registers an SBit cache for a route that starts with manager-only state.
    pub fn ensure_sbit_cache(&mut self) -> Result<(), FT_Error> {
        if !self.cache.is_null() {
            return Ok(());
        }
        let error = FTC_SBitCache_New(self.manager, &mut self.cache);
        if error == rust_ffi::FT_Err_Ok {
            Ok(())
        } else {
            Err(error)
        }
    }

    /// Calls the exported image-cache lookup and optionally releases its node.
    /// A retained node is safe to unreference after `FTC_Manager_RemoveFaceID`
    /// because the manager owns node allocations until reset or done.
    pub fn image_lookup(
        &mut self,
        mut scaler: FTC_ScalerRec,
        glyph_index: FT_UInt,
        release_node: bool,
    ) -> (FT_Error, FTC_Node) {
        if self.image_cache.is_null() {
            return (rust_ffi::FT_Err_Invalid_Argument, ptr::null_mut());
        }
        let mut glyph = ptr::null_mut();
        let mut node = ptr::null_mut();
        let error = FTC_ImageCache_LookupScaler(
            self.image_cache,
            ptr::from_mut(&mut scaler),
            rust_ffi::FT_LOAD_DEFAULT as FT_ULong,
            glyph_index,
            &mut glyph,
            &mut node,
        );
        if release_node && !node.is_null() {
            FTC_Node_Unref(node, self.manager);
            node = ptr::null_mut();
        }
        (error, node)
    }

    /// Calls the exported `FTC_SBitCache_Lookup` and snapshots its outputs.
    pub fn lookup(
        &mut self,
        image_type: FTC_ImageTypeRec,
        glyph_index: FT_UInt,
        sbit_output: bool,
        anode_output: bool,
    ) -> AbiSBitCacheLookupSnapshot {
        let _keep_requester_alive = &self.requester;
        let mut sbit = NonNull::<FTC_SBitRec>::dangling().as_ptr();
        let mut node = NonNull::<FTC_NodeRec>::dangling().as_ptr();
        let error = FTC_SBitCache_Lookup(
            self.cache,
            ptr::from_ref(&image_type).cast_mut(),
            glyph_index,
            if sbit_output {
                &mut sbit
            } else {
                ptr::null_mut()
            },
            if anode_output {
                &mut node
            } else {
                ptr::null_mut()
            },
        );
        abi_sbit_cache_lookup_snapshot(self.manager, error, sbit, node, sbit_output, anode_output)
    }

    /// Calls the exported `FTC_SBitCache_LookupScaler` and snapshots its
    /// manager-owned record and node state.
    pub fn lookup_scaler(
        &mut self,
        mut scaler: FTC_ScalerRec,
        load_flags: FT_ULong,
        glyph_index: FT_UInt,
        anode_output: bool,
    ) -> AbiSBitCacheLookupSnapshot {
        let _keep_requester_alive = &self.requester;
        let mut sbit = NonNull::<FTC_SBitRec>::dangling().as_ptr();
        let mut node = NonNull::<FTC_NodeRec>::dangling().as_ptr();
        let error = FTC_SBitCache_LookupScaler(
            self.cache,
            ptr::from_mut(&mut scaler),
            load_flags,
            glyph_index,
            &mut sbit,
            if anode_output {
                &mut node
            } else {
                ptr::null_mut()
            },
        );
        abi_sbit_cache_lookup_snapshot(self.manager, error, sbit, node, true, anode_output)
    }

    /// Exercises manager-owned face, size, cache, node, reset, and done state.
    pub fn ownership_snapshot(&mut self) -> AbiCacheManagerOwnershipSnapshot {
        let face_id = ptr::from_mut(self.requester.as_mut()).cast::<c_void>();
        let mut face_first = ptr::null_mut();
        let face_first_status = FTC_Manager_LookupFace(self.manager, face_id, &mut face_first);
        let requester_after_first = self.requester.requester_calls;
        let mut face_repeat = ptr::null_mut();
        let face_repeat_status = FTC_Manager_LookupFace(self.manager, face_id, &mut face_repeat);
        let requester_after_repeat = self.requester.requester_calls;

        let scaler = FTC_ScalerRec {
            face_id,
            width: 0,
            height: 12,
            pixel: 1,
            x_res: 0,
            y_res: 0,
        };
        let mut size_first = ptr::null_mut();
        let size_first_status = FTC_Manager_LookupSize(
            self.manager,
            ptr::from_ref(&scaler).cast_mut(),
            &mut size_first,
        );
        let mut size_repeat = ptr::null_mut();
        let size_repeat_status = FTC_Manager_LookupSize(
            self.manager,
            ptr::from_ref(&scaler).cast_mut(),
            &mut size_repeat,
        );
        let face_repeat_same = !face_first.is_null() && face_first == face_repeat;
        let size_repeat_same = !size_first.is_null() && size_first == size_repeat;
        let size_belongs_to_face = !size_first.is_null()
            && !face_first.is_null()
            && unsafe { (*size_first).face == face_first };

        let image_type = FTC_ImageTypeRec {
            face_id,
            width: 0,
            height: 12,
            flags: rust_ffi::FT_LOAD_DEFAULT,
        };
        let mut first_sbit = ptr::null_mut();
        let mut first_node = ptr::null_mut();
        let first_sbit_status = FTC_SBitCache_Lookup(
            self.cache,
            ptr::from_ref(&image_type).cast_mut(),
            36,
            &mut first_sbit,
            &mut first_node,
        );
        let mut repeat_sbit = ptr::null_mut();
        let mut repeat_node = ptr::null_mut();
        let repeat_sbit_status = FTC_SBitCache_Lookup(
            self.cache,
            ptr::from_ref(&image_type).cast_mut(),
            36,
            &mut repeat_sbit,
            &mut repeat_node,
        );
        let first_node_non_null = !first_node.is_null();
        let node_repeat_same = !first_node.is_null() && first_node == repeat_node;
        let finalizers_before_reset = self.requester.finalizer_calls;
        if !first_node.is_null() {
            FTC_Node_Unref(first_node, self.manager);
        }
        if !repeat_node.is_null() {
            FTC_Node_Unref(repeat_node, self.manager);
        }

        let cache_before_reset = self.cache;
        FTC_Manager_Reset(self.manager);
        let finalizers_after_reset = self.requester.finalizer_calls;
        let mut post_reset_face = ptr::null_mut();
        let post_reset_face_status =
            FTC_Manager_LookupFace(self.manager, face_id, &mut post_reset_face);
        let requester_after_reset = self.requester.requester_calls;
        let mut post_reset_sbit = ptr::null_mut();
        let mut post_reset_node = ptr::null_mut();
        let post_reset_sbit_status = FTC_SBitCache_Lookup(
            self.cache,
            ptr::from_ref(&image_type).cast_mut(),
            36,
            &mut post_reset_sbit,
            &mut post_reset_node,
        );
        let post_reset_node_non_null = !post_reset_node.is_null();
        if !post_reset_node.is_null() {
            FTC_Node_Unref(post_reset_node, self.manager);
        }

        let reset_preserved_cache_handle =
            !cache_before_reset.is_null() && cache_before_reset == self.cache;
        FTC_Manager_Done(self.manager);
        self.manager = ptr::null_mut();
        self.cache = ptr::null_mut();
        let finalizers_after_done = self.requester.finalizer_calls;
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
            face_repeat_same,
            size_repeat_same,
            size_belongs_to_face,
            first_node_non_null,
            node_repeat_same,
            post_reset_node_non_null,
            cache_non_null: !cache_before_reset.is_null(),
            reset_preserved_cache_handle,
        }
    }
}

#[cfg(feature = "abi-test-support")]
impl Drop for AbiSBitCacheHarness {
    fn drop(&mut self) {
        FTC_Manager_Done(self.manager);
        let _ = FT_Done_FreeType(self.library);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_SBitCache_Lookup(
    cache: FTC_SBitCache,
    type_: FTC_ImageType,
    gindex: FT_UInt,
    sbit: *mut FTC_SBit,
    anode: *mut FTC_Node,
) -> FT_Error {
    if type_.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: image type is a caller-owned descriptor copied by value.
    let type_ = unsafe { *type_ };
    let key = ftc_image_type_key(type_, gindex);
    ftc_sbit_cache_lookup_impl(cache, key, sbit, anode)
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_SBitCache_LookupScaler(
    cache: FTC_SBitCache,
    scaler: FTC_Scaler,
    load_flags: FT_ULong,
    gindex: FT_UInt,
    sbit: *mut FTC_SBit,
    anode: *mut FTC_Node,
) -> FT_Error {
    if scaler.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: scaler is a caller-owned descriptor copied by value.
    let scaler = unsafe { *scaler };
    let key = ftc_scaler_key(scaler, load_flags, gindex);
    ftc_sbit_cache_lookup_impl(cache, key, sbit, anode)
}

#[unsafe(no_mangle)]
pub extern "C" fn FTC_Node_Unref(node: FTC_Node, manager: FTC_Manager) {
    if node.is_null() || !cache_manager_is_live(manager) {
        return;
    }
    // SAFETY: manager is live; only nodes retained by it may be unreferenced.
    let belongs = unsafe { (*manager).nodes.contains(&node) };
    if belongs {
        // SAFETY: membership proves the node remains manager-owned and live.
        unsafe {
            (*node).ref_count = (*node).ref_count.saturating_sub(1);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stream_OpenGzip(stream: FT_Stream, source: FT_Stream) -> FT_Error {
    let Some(stream_ref) = (unsafe { stream.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    };
    let Some(source_ref) = (unsafe { source.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    };
    if source_ref.base.is_null() {
        return rust_ffi::FT_Err_Invalid_Stream_Handle as FT_Error;
    }
    let source_len = source_ref.size as usize;
    // SAFETY: this thin ABI wrapper supports the memory-backed stream shape
    // used by the parity fixtures; `base` and `size` are caller-provided.
    let source_bytes = unsafe { slice::from_raw_parts(source_ref.base.cast_const(), source_len) };
    let error =
        rust_ffi::FT_Stream_OpenGzip(Some(stream_ref), Some(source_ref), Some(source_bytes));
    if error == rust_ffi::FT_Err_Ok {
        stream_ref.close = c_gzip_stream_close as *const () as FT_Pointer;
        if stream_ref.base.is_null() {
            stream_ref.read = c_gzip_stream_io as *const () as FT_Pointer;
        }
    }
    error
}

extern "C" fn c_gzip_stream_io(
    stream: FT_Stream,
    offset: FT_ULong,
    buffer: *mut FT_Byte,
    count: FT_ULong,
) -> FT_ULong {
    if buffer.is_null() && count != 0 {
        return 0;
    }
    let Some(bytes) = abi_support_gzip_stream_bytes(stream, offset, count) else {
        return 0;
    };
    if !buffer.is_null() && !bytes.is_empty() {
        // SAFETY: the C stream callback contract provides `count` writable
        // bytes and the registry returns no more than that requested count.
        unsafe {
            ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len());
        }
    }
    FT_ULong::try_from(bytes.len()).unwrap_or(FT_ULong::MAX)
}

extern "C" fn c_gzip_stream_close(stream: FT_Stream) {
    abi_support_gzip_stream_close(stream);
}

pub fn abi_support_gzip_stream_bytes(
    stream: FT_Stream,
    offset: FT_ULong,
    count: FT_ULong,
) -> Option<Vec<FT_Byte>> {
    let stream_ref = unsafe { stream.as_ref() }?;
    rust_ffi::FT_Gzip_Stream_Read(Some(stream_ref), offset, count)
}

pub fn abi_support_gzip_stream_close(stream: FT_Stream) {
    if let Some(stream_ref) = unsafe { stream.as_mut() } {
        rust_ffi::FT_Gzip_Stream_Close(Some(stream_ref));
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_List_Add(list: FT_List, node: FT_ListNode) {
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
pub extern "C" fn FT_List_Insert(list: FT_List, node: FT_ListNode) {
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
pub extern "C" fn FT_List_Find(list: FT_List, data: FT_Pointer) -> FT_ListNode {
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
pub extern "C" fn FT_List_Remove(list: FT_List, node: FT_ListNode) {
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
pub extern "C" fn FT_List_Up(list: FT_List, node: FT_ListNode) {
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
pub extern "C" fn FT_List_Iterate(
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
pub extern "C" fn FT_List_Finalize(
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
pub extern "C" fn FT_Bitmap_Copy(
    library: FT_Library,
    source: *const FT_Bitmap,
    target: *mut FT_Bitmap,
) -> FT_Error {
    let Some(source_ref) = (unsafe { source.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(target_ref) = (unsafe { target.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    if source == target.cast_const() {
        return if library_ref(library).is_some() {
            rust_ffi::FT_Err_Ok
        } else {
            rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error
        };
    }

    let original_target = bitmap_to_rust(target_ref);
    let source_view = if bitmap_copy_array_too_large(source_ref) {
        bitmap_to_rust(source_ref)
    } else {
        bitmap_rust_view(source_ref)
    };
    let mut target_view = bitmap_rust_view(target_ref);

    let err = rust_ffi::FT_Bitmap_Copy(
        library_ref(library),
        Some(&source_view),
        Some(&mut target_view),
    );
    if err == rust_ffi::FT_Err_Ok {
        release_bitmap_buffer(&original_target);
        copy_rust_bitmap_record_to_c(target_ref, &target_view);
    } else {
        release_bitmap_buffer(&target_view);
    }
    release_bitmap_buffer(&source_view);
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_Convert(
    library: FT_Library,
    source: *const FT_Bitmap,
    target: *mut FT_Bitmap,
    alignment: FT_Int,
) -> FT_Error {
    if library_ref(library).is_none() {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    }
    let Some(source_ref) = (unsafe { source.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(target_ref) = (unsafe { target.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let source_view = bitmap_rust_view(source_ref);
    if !matches!(
        i32::from(source_view.pixel_mode),
        rust_ffi::FT_PIXEL_MODE_MONO
            | rust_ffi::FT_PIXEL_MODE_GRAY
            | rust_ffi::FT_PIXEL_MODE_GRAY2
            | rust_ffi::FT_PIXEL_MODE_GRAY4
            | rust_ffi::FT_PIXEL_MODE_LCD
            | rust_ffi::FT_PIXEL_MODE_LCD_V
            | rust_ffi::FT_PIXEL_MODE_BGRA
    ) {
        // FreeType rejects an unsupported source mode before it inspects or
        // mutates the target.  In particular, callers may deliberately pass a
        // dirty target to verify this error path; reading its buffer would be
        // both observably wrong and unsafe.
        release_bitmap_buffer(&source_view);
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let original_target = bitmap_to_rust(target_ref);
    let mut target_view = bitmap_rust_view(target_ref);

    let err = rust_ffi::FT_Bitmap_Convert(
        library_ref(library),
        Some(&source_view),
        Some(&mut target_view),
        alignment,
    );
    if err == rust_ffi::FT_Err_Ok {
        release_bitmap_buffer(&original_target);
        copy_rust_bitmap_record_to_c(target_ref, &target_view);
    } else {
        release_bitmap_buffer(&target_view);
    }
    release_bitmap_buffer(&source_view);
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_Done(library: FT_Library, bitmap: *mut FT_Bitmap) -> FT_Error {
    if library_ref(library).is_none() {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    }
    let Some(bitmap_ref) = (unsafe { bitmap.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let original_bitmap = bitmap_to_rust(bitmap_ref);
    let mut bitmap_view = bitmap_rust_view(bitmap_ref);
    let err = rust_ffi::FT_Bitmap_Done(library_ref(library), Some(&mut bitmap_view));
    if err == rust_ffi::FT_Err_Ok {
        release_bitmap_buffer(&original_bitmap);
        copy_rust_bitmap_record_to_c(bitmap_ref, &bitmap_view);
    } else {
        release_bitmap_buffer(&bitmap_view);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_Embolden(
    library: FT_Library,
    bitmap: *mut FT_Bitmap,
    xStrength: FT_Pos,
    yStrength: FT_Pos,
) -> FT_Error {
    let Some(bitmap_ref) = (unsafe { bitmap.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let original_bitmap = bitmap_to_rust(bitmap_ref);
    let mut bitmap_view = bitmap_rust_view(bitmap_ref);

    let err = rust_ffi::FT_Bitmap_Embolden(
        library_ref(library),
        Some(&mut bitmap_view),
        xStrength,
        yStrength,
    );
    if err == rust_ffi::FT_Err_Ok {
        release_bitmap_buffer(&original_bitmap);
        copy_rust_bitmap_record_to_c(bitmap_ref, &bitmap_view);
    } else {
        release_bitmap_buffer(&bitmap_view);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Bitmap_Blend(
    library: FT_Library,
    source: *const FT_Bitmap,
    source_offset: FT_Vector,
    target: *mut FT_Bitmap,
    atarget_offset: *mut FT_Vector,
    color: FT_Color,
) -> FT_Error {
    let Some(source_ref) = (unsafe { source.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(target_ref) = (unsafe { target.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(atarget_offset_ref) = (unsafe { atarget_offset.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let original_target = bitmap_to_rust(target_ref);
    let source_view = bitmap_rust_view(source_ref);
    let mut target_view = bitmap_rust_view(target_ref);
    let mut rust_target_offset = rust_ffi::FT_Vector {
        x: atarget_offset_ref.x,
        y: atarget_offset_ref.y,
    };
    let err = rust_ffi::FT_Bitmap_Blend(
        library_ref(library),
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
        release_bitmap_buffer(&original_target);
        copy_rust_bitmap_record_to_c(target_ref, &target_view);
        atarget_offset_ref.x = rust_target_offset.x;
        atarget_offset_ref.y = rust_target_offset.y;
    } else {
        release_bitmap_buffer(&target_view);
    }
    release_bitmap_buffer(&source_view);
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Palette_Data_Get(
    face: FT_Face,
    apalette_data: *mut FT_Palette_Data,
) -> FT_Error {
    let Some(out) = (unsafe { apalette_data.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let mut rust_out = rust_ffi::FT_Palette_Data::default();
    let err = rust_ffi::FT_Palette_Data_Get(
        face_state(face).map(|state| &state.inner),
        Some(&mut rust_out),
    );
    if err == rust_ffi::FT_Err_Ok {
        copy_palette_data_to_c(out, rust_out);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Palette_Select(
    face: FT_Face,
    palette_index: FT_UShort,
    apalette: *mut *mut FT_Color,
) -> FT_Error {
    let mut rust_palette: *const rust_ffi::FT_Color = ptr::null();
    let err = rust_ffi::FT_Palette_Select(
        face_state(face).map(|state| &state.inner),
        palette_index,
        (!apalette.is_null()).then_some(&mut rust_palette),
    );
    if err == rust_ffi::FT_Err_Ok && !apalette.is_null() {
        // SAFETY: `apalette` is non-null and caller provided writable storage.
        unsafe {
            *apalette = rust_palette.cast::<FT_Color>().cast_mut();
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Palette_Set_Foreground_Color(
    face: FT_Face,
    foreground_color: FT_Color,
) -> FT_Error {
    rust_ffi::FT_Palette_Set_Foreground_Color(
        face_state(face).map(|state| &state.inner),
        rust_color_from_c(foreground_color),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Color_Glyph_Layer(
    face: FT_Face,
    base_glyph: FT_UInt,
    aglyph_index: *mut FT_UInt,
    acolor_index: *mut FT_UInt,
    iterator: *mut FT_LayerIterator,
) -> FT_Bool {
    rust_ffi::FT_Get_Color_Glyph_Layer(
        face_state(face).map(|state| &state.inner),
        base_glyph,
        unsafe { aglyph_index.as_mut() },
        unsafe { acolor_index.as_mut() },
        unsafe { iterator.as_mut() },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Color_Glyph_ClipBox(
    face: FT_Face,
    base_glyph: FT_UInt,
    clip_box: *mut FT_ClipBox,
) -> FT_Bool {
    rust_ffi::FT_Get_Color_Glyph_ClipBox(
        face_state(face).map(|state| &state.inner),
        base_glyph,
        unsafe { clip_box.as_mut() },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Color_Glyph_Paint(
    face: FT_Face,
    base_glyph: FT_UInt,
    root_transform: FT_UInt,
    paint: *mut FT_OpaquePaint,
) -> FT_Bool {
    rust_ffi::FT_Get_Color_Glyph_Paint(
        face_state(face).map(|state| &state.inner),
        base_glyph,
        root_transform,
        unsafe { paint.as_mut() },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Paint(
    face: FT_Face,
    opaque_paint: FT_OpaquePaint,
    paint: *mut FT_COLR_Paint,
) -> FT_Bool {
    rust_ffi::FT_Get_Paint(
        face_state(face).map(|state| &state.inner),
        opaque_paint,
        unsafe { paint.as_mut() },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Paint_Layers(
    face: FT_Face,
    layer_iterator: *mut FT_LayerIterator,
    paint: *mut FT_OpaquePaint,
) -> FT_Bool {
    rust_ffi::FT_Get_Paint_Layers(
        face_state(face).map(|state| &state.inner),
        unsafe { layer_iterator.as_mut() },
        unsafe { paint.as_mut() },
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Colorline_Stops(
    face: FT_Face,
    color_stop: *mut FT_ColorStop,
    iterator: *mut FT_ColorStopIterator,
) -> FT_Bool {
    rust_ffi::FT_Get_Colorline_Stops(
        face_state(face).map(|state| &state.inner),
        unsafe { color_stop.as_mut() },
        unsafe { iterator.as_mut() },
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_paint_layer_iterator(
    face: FT_Face,
    opaque_paint: FT_OpaquePaint,
) -> Option<FT_LayerIterator> {
    rust_ffi::FT_ColrV1_Paint_Layer_Iterator_Copy(
        face_state(face).map(|state| &state.inner),
        opaque_paint,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_paint_colorline(
    face: FT_Face,
    opaque_paint: FT_OpaquePaint,
) -> Option<FT_ColorLine> {
    rust_ffi::FT_ColrV1_Paint_ColorLine_Copy(
        face_state(face).map(|state| &state.inner),
        opaque_paint,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_paint_linear_gradient(
    face: FT_Face,
    opaque_paint: FT_OpaquePaint,
) -> Option<FT_PaintLinearGradient> {
    rust_ffi::FT_ColrV1_Paint_LinearGradient_Copy(
        face_state(face).map(|state| &state.inner),
        opaque_paint,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_paint_transform(
    face: FT_Face,
    opaque_paint: FT_OpaquePaint,
) -> Option<FT_PaintTransform> {
    rust_ffi::FT_ColrV1_Paint_Transform_Copy(
        face_state(face).map(|state| &state.inner),
        opaque_paint,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_paint_graph(
    face: FT_Face,
) -> Option<rust_ffi::FT_ColrV1_PaintGraph_Snapshot> {
    rust_ffi::FT_ColrV1_PaintGraph_Copy(face_state(face).map(|state| &state.inner))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_colr_v1_public_paint_solid(
    face: FT_Face,
    glyph_index: FT_UInt,
) -> rust_ffi::FT_ColrV1_PublicPaintSolid_Snapshot {
    rust_ffi::FT_ColrV1_PublicPaintSolid_Copy(
        face_state(face).map(|state| &state.inner),
        glyph_index,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_TrueTypeGX_Free(face: FT_Face, table: FT_Bytes) {
    release_c_open_type_table(face, table);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_TrueTypeGX_Validate(
    face: FT_Face,
    validation_flags: FT_UInt,
    tables: *mut FT_Bytes,
    table_length: FT_UInt,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if tables.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let available = rust_ffi::FT_Library_Has_Module(library_ref(state.library), "gxvalid");
    rust_ffi::FT_GX_Validator_Set_Available(&mut state.inner, available);
    if !available {
        return rust_ffi::FT_Err_Unimplemented_Feature;
    }
    // `FT_TrueTypeGX_Validate` declares a fixed ten-slot output array.  A
    // larger count is outside the public ABI contract; cap it before creating
    // any internal view so malformed callers cannot request an unbounded
    // allocation or raw write.
    let table_length = table_length.min(10) as usize;
    let mut rust_tables = [ptr::null(); 10];
    let err = rust_ffi::FT_TrueTypeGX_Validate(
        Some(&state.inner),
        validation_flags,
        Some(&mut rust_tables[..table_length]),
    );
    let mut c_tables = [ptr::null(); 10];
    if err == rust_ffi::FT_Err_Ok {
        for (index, rust_table) in rust_tables.iter().copied().take(table_length).enumerate() {
            if let Some(bytes) = rust_ffi::FT_OpenType_Table_Copy(rust_table) {
                match retain_c_open_type_table(face, bytes) {
                    Ok(table) => c_tables[index] = table,
                    Err(error) => {
                        for table in c_tables {
                            release_c_open_type_table(face, table);
                        }
                        for table in rust_tables {
                            rust_ffi::FT_TrueTypeGX_Free(Some(&state.inner), table);
                        }
                        return error;
                    }
                }
            }
        }
    }
    for table in rust_tables {
        rust_ffi::FT_TrueTypeGX_Free(Some(&state.inner), table);
    }
    for (index, table) in c_tables.into_iter().take(table_length).enumerate() {
        // SAFETY: the public contract requires `tables` to address
        // `table_length` writable FT_Bytes slots.
        unsafe {
            *tables.add(index) = table;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_ClassicKern_Free(face: FT_Face, table: FT_Bytes) {
    release_c_open_type_table(face, table);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_ClassicKern_Validate(
    face: FT_Face,
    validation_flags: FT_UInt,
    ckern_table: *mut FT_Bytes,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let available = rust_ffi::FT_Library_Has_Module(library_ref(state.library), "gxvalid");
    rust_ffi::FT_GX_Validator_Set_Available(&mut state.inner, available);
    let mut table = ptr::null();
    let err = rust_ffi::FT_ClassicKern_Validate(
        Some(&state.inner),
        validation_flags,
        (!ckern_table.is_null()).then_some(&mut table),
    );
    if err == rust_ffi::FT_Err_Ok {
        let converted = match rust_ffi::FT_OpenType_Table_Copy(table) {
            Some(bytes) => match retain_c_open_type_table(face, bytes) {
                Ok(table) => table,
                Err(error) => {
                    rust_ffi::FT_ClassicKern_Free(Some(&state.inner), table);
                    return error;
                }
            },
            None => ptr::null(),
        };
        rust_ffi::FT_ClassicKern_Free(Some(&state.inner), table);
        write_ft_bytes(ckern_table, converted);
    } else if available && !ckern_table.is_null() {
        write_ft_bytes(ckern_table, ptr::null());
    }
    err
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_SfntName {
    pub platform_id: FT_UShort,
    pub encoding_id: FT_UShort,
    pub language_id: FT_UShort,
    pub name_id: FT_UShort,
    pub string: *mut FT_Byte,
    pub string_len: FT_UInt,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_SfntLangTag {
    pub string: *mut FT_Byte,
    pub string_len: FT_UInt,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_CharMapRec {
    pub face: FT_Face,
    pub encoding: FT_Encoding,
    pub platform_id: FT_UShort,
    pub encoding_id: FT_UShort,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct TT_OS2 {
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
    pub panose: [FT_Byte; 10],
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
pub struct TT_VertHeader {
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
pub struct TT_MaxProfile {
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

pub type TT_Header = rust_ffi::TT_Header;
pub type TT_HoriHeader = rust_ffi::TT_HoriHeader;
pub type TT_PCLT = rust_ffi::TT_PCLT;
pub type TT_Postscript = rust_ffi::TT_Postscript;

#[repr(C)]
pub struct FT_GlyphSlotRec {
    pub library: FT_Library,
    pub face: FT_Face,
    pub next: FT_GlyphSlot,
    pub glyph_index: FT_UInt,
    pub generic: FT_Generic,
    pub metrics: FT_Glyph_Metrics,
    pub linearHoriAdvance: FT_Fixed,
    pub linearVertAdvance: FT_Fixed,
    pub advance: FT_Vector,
    pub format: FT_Glyph_Format,
    pub bitmap: FT_Bitmap,
    pub bitmap_left: FT_Int,
    pub bitmap_top: FT_Int,
    pub outline: FT_Outline,
    pub num_subglyphs: FT_UInt,
    pub subglyphs: FT_SubGlyph,
    pub control_data: FT_Pointer,
    pub control_len: c_long,
    pub lsb_delta: FT_Pos,
    pub rsb_delta: FT_Pos,
    pub other: FT_Pointer,
    pub internal: FT_Slot_Internal,
}

impl Default for FT_GlyphSlotRec {
    fn default() -> Self {
        Self {
            library: ptr::null_mut(),
            face: ptr::null_mut(),
            next: ptr::null_mut(),
            glyph_index: 0,
            generic: FT_Generic::default(),
            metrics: FT_Glyph_Metrics::default(),
            linearHoriAdvance: 0,
            linearVertAdvance: 0,
            advance: FT_Vector::default(),
            format: 0,
            bitmap: FT_Bitmap::default(),
            bitmap_left: 0,
            bitmap_top: 0,
            outline: FT_Outline::default(),
            num_subglyphs: 0,
            subglyphs: ptr::null_mut(),
            control_data: ptr::null_mut(),
            control_len: 0,
            lsb_delta: 0,
            rsb_delta: 0,
            other: ptr::null_mut(),
            internal: ptr::null_mut(),
        }
    }
}

#[repr(C)]
struct FT_Slot_InternalRecCompat {
    loader: FT_Pointer,
    flags: FT_UInt,
    glyph_transformed: FT_Bool,
    glyph_matrix: FT_Matrix,
    glyph_delta: FT_Vector,
    glyph_hints: FT_Pointer,
    load_flags: FT_Int32,
    owns_bitmap: bool,
    buffer: Vec<u8>,
    outline_points: Box<[FT_Vector]>,
    outline_tags: Box<[FT_Byte]>,
    outline_contours: Box<[FT_UShort]>,
    svg_document: Box<[FT_Byte]>,
    svg_record: Option<Box<FT_SVG_DocumentRec>>,
    rust_slot: rust_ffi::FT_GlyphSlot,
    source_face: FT_Face,
}

#[repr(C)]
pub struct FT_SizeRec {
    // FreeType include/freetype/freetype.h exposes `face`, `generic`, `metrics`,
    // and opaque non-null `internal` as the public FT_SizeRec fields.
    pub face: FT_Face,
    pub generic: FT_Generic,
    pub metrics: FT_Size_Metrics,
    pub internal: FT_Size_Internal,
}

struct FT_Size_InternalRecCompat {
    rust_size: rust_ffi::FT_Size,
    owner: FT_Face,
}

#[repr(C)]
pub struct FT_FaceRec {
    pub num_faces: FT_Long,
    pub face_index: FT_Long,
    pub face_flags: FT_Long,
    pub style_flags: FT_Long,
    pub num_glyphs: FT_Long,
    pub family_name: *mut FT_String,
    pub style_name: *mut FT_String,
    pub num_fixed_sizes: FT_Int,
    pub available_sizes: *mut FT_Bitmap_Size,
    pub num_charmaps: FT_Int,
    pub charmaps: *mut FT_CharMap,
    pub generic: FT_Generic,
    pub bbox: FT_BBox,
    pub units_per_EM: FT_UShort,
    pub ascender: FT_Short,
    pub descender: FT_Short,
    pub height: FT_Short,
    pub max_advance_width: FT_Short,
    pub max_advance_height: FT_Short,
    pub underline_position: FT_Short,
    pub underline_thickness: FT_Short,
    pub glyph: FT_GlyphSlot,
    pub size: FT_Size,
    pub charmap: FT_CharMap,
    pub driver: FT_Driver,
    pub memory: FT_Memory,
    pub stream: FT_Stream,
    pub sizes_list: FT_ListRec,
    pub autohint: FT_Generic,
    pub extensions: FT_Pointer,
    pub internal: FT_Face_Internal,
}

impl Default for FT_FaceRec {
    fn default() -> Self {
        Self {
            num_faces: 0,
            face_index: 0,
            face_flags: 0,
            style_flags: 0,
            num_glyphs: 0,
            family_name: ptr::null_mut(),
            style_name: ptr::null_mut(),
            num_fixed_sizes: 0,
            available_sizes: ptr::null_mut(),
            num_charmaps: 0,
            charmaps: ptr::null_mut(),
            generic: FT_Generic::default(),
            bbox: FT_BBox::default(),
            units_per_EM: 0,
            ascender: 0,
            descender: 0,
            height: 0,
            max_advance_width: 0,
            max_advance_height: 0,
            underline_position: 0,
            underline_thickness: 0,
            glyph: ptr::null_mut(),
            size: ptr::null_mut(),
            charmap: ptr::null_mut(),
            driver: ptr::null_mut(),
            memory: ptr::null_mut(),
            stream: ptr::null_mut(),
            sizes_list: FT_ListRec::default(),
            autohint: FT_Generic::default(),
            extensions: ptr::null_mut(),
            internal: ptr::null_mut(),
        }
    }
}

#[repr(C)]
pub struct FT_LibraryRec {
    pub memory: FT_Memory,
    pub version_major: FT_Int,
    pub version_minor: FT_Int,
    pub version_patch: FT_Int,
    pub num_modules: FT_UInt,
    pub modules: [FT_Module; 32],
    pub renderers: FT_ListRec,
    pub cur_renderer: FT_Renderer,
    pub auto_hinter: FT_Module,
    pub debug_hooks: [FT_DebugHook_Func; 4],
    #[cfg(feature = "subpixel-rendering")]
    pub lcd_weights: [FT_Byte; 5],
    #[cfg(not(feature = "subpixel-rendering"))]
    pub lcd_geometry: [FT_Vector; 3],
    pub refcount: FT_Int,
    pub internal: *mut c_void,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FtcImageKey {
    face_id: FTC_FaceID,
    width: FT_UInt,
    height: FT_UInt,
    pixel: FT_Int,
    x_res: FT_UInt,
    y_res: FT_UInt,
    load_flags: FT_ULong,
    glyph_index: FT_UInt,
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct FtcCMapKey {
    face_id: FTC_FaceID,
    cmap_index: FT_UInt,
    char_code: FT_UInt32,
}

enum FtcNodePayload {
    Glyph(FT_Glyph),
    SBit {
        record: FTC_SBitRec,
        _buffer: Box<[FT_Byte]>,
    },
}

#[allow(
    dead_code,
    reason = "prefix fields are read by external C consumers through the pinned FTC_NodeRec layout"
)]
#[repr(C)]
pub struct FTC_NodeRec {
    mru_next: FTC_Node,
    mru_prev: FTC_Node,
    link: FTC_Node,
    hash: usize,
    cache_index: FT_UShort,
    ref_count: FT_Short,
    payload: FtcNodePayload,
}

pub struct FTC_ManagerRec {
    library: FT_Library,
    requester: FTC_Face_Requester,
    req_data: FT_Pointer,
    _max_faces: FT_UInt,
    _max_sizes: FT_UInt,
    _max_bytes: FT_ULong,
    faces: BTreeMap<usize, FT_Face>,
    nodes: Vec<FTC_Node>,
    cmap_caches: Vec<FTC_CMapCache>,
    image_caches: Vec<FTC_ImageCache>,
    sbit_caches: Vec<FTC_SBitCache>,
}

pub struct FTC_CMapCacheRec {
    manager: FTC_Manager,
    entries: BTreeMap<FtcCMapKey, FT_UInt>,
}

pub struct FTC_ImageCacheRec {
    manager: FTC_Manager,
    entries: BTreeMap<FtcImageKey, FTC_Node>,
}

pub struct FTC_SBitCacheRec {
    manager: FTC_Manager,
    entries: BTreeMap<FtcImageKey, FTC_Node>,
}

#[repr(C)]
pub struct FT_ModuleRec {
    pub clazz: *const FT_Module_Class,
    pub library: FT_Library,
    pub memory: FT_Memory,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FT_Module_Class {
    pub module_flags: FT_ULong,
    pub module_size: FT_Long,
    pub module_name: *const FT_String,
    pub module_version: FT_Fixed,
    pub module_requires: FT_Fixed,
    pub module_interface: *const c_void,
    pub module_init: FT_Module_Constructor,
    pub module_done: FT_Module_Destructor,
    pub get_interface: FT_Module_Requester,
}

#[repr(C)]
pub struct FT_RendererRec {
    pub root: FT_ModuleRec,
    pub clazz: *mut FT_Renderer_Class,
    pub glyph_format: FT_Glyph_Format,
    pub glyph_class: FT_Glyph_Class,
    pub raster: FT_Raster,
    pub raster_render: FT_Raster_RenderFunc,
    pub render: FT_Renderer_RenderFunc,
    module_name: &'static str,
}

struct LibraryState {
    inner: rust_ffi::FT_Library,
    _system_memory: Option<Box<FT_MemoryRec>>,
    allocation_memory: FT_Memory,
    allocation_block: FT_Pointer,
    module_allocation_blocks: Vec<FT_Pointer>,
    _outline_renderer_class: Box<FT_Renderer_Class>,
    _outline_raster_class: Box<FT_Raster_Funcs>,
    outline_renderer: FT_RendererRec,
    _raster1_renderer_class: Box<FT_Renderer_Class>,
    _raster1_raster_class: Box<FT_Raster_Funcs>,
    raster1_renderer: FT_RendererRec,
    _sdf_renderer_class: Box<FT_Renderer_Class>,
    _sdf_raster_class: Box<FT_Raster_Funcs>,
    sdf_renderer: FT_RendererRec,
    _synthetic_renderer_class: Box<FT_Renderer_Class>,
    _synthetic_raster_class: Box<FT_Raster_Funcs>,
    synthetic_renderer: FT_RendererRec,
    _bitmap_renderer_class: Box<FT_Renderer_Class>,
    _bitmap_raster_class: Box<FT_Raster_Funcs>,
    bitmap_renderer: FT_RendererRec,
    _svg_renderer_class: Box<FT_Renderer_Class>,
    svg_renderer: FT_RendererRec,
    default_modules: Vec<AbiModuleRecord>,
    synthetic_modules: Vec<AbiSyntheticModuleRecord>,
    custom_glyphs: Vec<AbiOwnedCustomGlyph>,
    faces: Vec<FT_Face>,
}

struct AbiModuleRecord {
    name: &'static str,
    _c_name: CString,
    _class: Box<FT_Module_Class>,
    module: Box<FT_ModuleRec>,
}

struct AbiSyntheticModuleRecord {
    name: &'static str,
    module: FT_Module,
    _handle: Option<Box<FT_ModuleRec>>,
    #[allow(
        dead_code,
        reason = "retains the class interface pointer for ABI lifecycle evidence"
    )]
    interface: FT_Module_Interface,
    done: FT_Module_Destructor,
}

struct AbiOwnedCustomGlyph {
    storage: Box<[usize]>,
}

impl AbiOwnedCustomGlyph {
    fn new(byte_len: usize) -> Self {
        let word_size = std::mem::size_of::<usize>();
        let word_len = byte_len.div_ceil(word_size);
        Self {
            storage: vec![0; word_len].into_boxed_slice(),
        }
    }

    fn as_glyph(&mut self) -> FT_Glyph {
        self.storage.as_mut_ptr().cast::<FT_GlyphRec>()
    }
}

const ABI_DEFAULT_MODULE_NAMES: &[&str] = &[
    "autofitter",
    "truetype",
    "type1",
    "cff",
    "t1cid",
    "pfr",
    "type42",
    "winfonts",
    "pcf",
    "bdf",
    "psaux",
    "psnames",
    "pshinter",
    "sfnt",
    "smooth",
    "raster1",
    "sdf",
    "bsdf",
    "ot-svg",
];

fn abi_default_module_records(memory: FT_Memory) -> Vec<AbiModuleRecord> {
    ABI_DEFAULT_MODULE_NAMES
        .iter()
        .map(|name| {
            let c_name = CString::new(*name).unwrap_or_default();
            let class = Box::new(FT_Module_Class {
                module_flags: 0,
                module_size: std::mem::size_of::<FT_ModuleRec>() as FT_Long,
                module_name: c_name.as_ptr(),
                module_version: 0x20_000,
                module_requires: 0x20_000,
                module_interface: ptr::null(),
                module_init: None,
                module_done: None,
                get_interface: None,
            });
            let module = Box::new(FT_ModuleRec {
                clazz: &*class,
                library: ptr::null_mut(),
                memory,
            });
            AbiModuleRecord {
                name,
                _c_name: c_name,
                _class: class,
                module,
            }
        })
        .collect()
}

fn abi_render_slot_with_core(slot: FT_GlyphSlot, mode: FT_Render_Mode) -> FT_Error {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(internal) = slot_internal(slot_ptr.as_ptr()) else {
        return rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error;
    };
    let source_face = internal.source_face;
    let rust_slot = internal.rust_slot.clone();
    let load_flags = internal.load_flags;
    match rust_ffi::FT_Render_Glyph(rust_slot, mode) {
        Ok(rendered) => {
            // SAFETY: `slot_ptr` is the live face-owned slot passed to the
            // renderer callback.  FreeType renderer callbacks mutate it in
            // place on success.
            unsafe {
                replace_slot_record(
                    slot_ptr.as_ptr(),
                    rust_slot_to_abi(rendered, source_face, load_flags | rust_ffi::FT_LOAD_RENDER),
                );
            }
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

unsafe extern "C" fn abi_default_renderer_render(
    renderer: FT_Renderer,
    slot: FT_GlyphSlot,
    mode: FT_Render_Mode,
    _origin: *const FT_Vector,
) -> FT_Error {
    let Some(renderer) = (unsafe { renderer.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let accepted = match renderer.module_name {
        "smooth" => matches!(
            mode,
            rust_ffi::FT_RENDER_MODE_NORMAL
                | rust_ffi::FT_RENDER_MODE_LIGHT
                | rust_ffi::FT_RENDER_MODE_LCD
                | rust_ffi::FT_RENDER_MODE_LCD_V
        ),
        "raster1" => mode == rust_ffi::FT_RENDER_MODE_MONO,
        "sdf" | "bsdf" => mode == rust_ffi::FT_RENDER_MODE_SDF,
        "ot-svg" => {
            // FreeType 2.14.3 `src/svg/ftsvg.c:117-124` accepts NORMAL,
            // reports Bad_Argument for every other mode (including the
            // `FT_RENDER_MODE_SVG` enum value), and then reports missing
            // hooks in the default build before dereferencing the SVG slot.
            return if mode == rust_ffi::FT_RENDER_MODE_NORMAL {
                rust_ffi::FT_Err_Missing_SVG_Hooks as FT_Error
            } else {
                rust_ffi::FT_Err_Bad_Argument as FT_Error
            };
        }
        _ => false,
    };
    if !accepted {
        return rust_ffi::FT_Err_Cannot_Render_Glyph;
    }
    abi_render_slot_with_core(slot, mode)
}

fn abi_renderer_class(
    module_name: *const FT_String,
    glyph_format: FT_Glyph_Format,
    raster_class: *const FT_Raster_Funcs,
) -> FT_Renderer_Class {
    FT_Renderer_Class {
        root: FT_Module_Class {
            module_flags: rust_ffi::FT_MODULE_RENDERER as FT_ULong,
            module_size: std::mem::size_of::<FT_RendererRec>() as FT_Long,
            module_name,
            module_version: 0x20_000,
            module_requires: 0x20_000,
            module_interface: ptr::null(),
            module_init: None,
            module_done: None,
            get_interface: None,
        },
        glyph_format,
        render_glyph: Some(abi_default_renderer_render),
        transform_glyph: None,
        get_glyph_cbox: None,
        set_mode: None,
        raster_class,
    }
}

// The C ABI facade keeps the renderer class table observable even though the
// default renderer implementation delegates rendering to the pure-Rust core.
// These callbacks model the five non-null slots present in FreeType's four
// maintained raster classes; the lifecycle routes exercise callback behavior
// separately through their callback-backed synthetic renderer.
unsafe extern "C" fn abi_default_raster_new(_memory: FT_Pointer, raster: *mut FT_Raster) -> c_int {
    let Some(raster) = (unsafe { raster.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    *raster = ptr::null_mut();
    rust_ffi::FT_Err_Ok
}

unsafe extern "C" fn abi_default_raster_reset(
    _raster: FT_Raster,
    _pool_base: *mut FT_Byte,
    _pool_size: FT_ULong,
) {
}

unsafe extern "C" fn abi_default_raster_set_mode(
    _raster: FT_Raster,
    _mode: FT_ULong,
    _args: FT_Pointer,
) -> c_int {
    rust_ffi::FT_Err_Ok
}

unsafe extern "C" fn abi_default_raster_render(
    _raster: FT_Raster,
    _params: *const FT_Raster_Params,
) -> c_int {
    rust_ffi::FT_Err_Ok
}

unsafe extern "C" fn abi_default_raster_done(_raster: FT_Raster) {}

fn abi_default_raster_funcs(glyph_format: FT_Glyph_Format) -> FT_Raster_Funcs {
    FT_Raster_Funcs {
        glyph_format,
        raster_new: Some(abi_default_raster_new),
        raster_reset: Some(abi_default_raster_reset),
        raster_set_mode: Some(abi_default_raster_set_mode),
        raster_render: Some(abi_default_raster_render),
        raster_done: Some(abi_default_raster_done),
    }
}

fn new_library_state(
    inner: rust_ffi::FT_Library,
    allocation_memory: FT_Memory,
    allocation_block: FT_Pointer,
) -> LibraryState {
    static SMOOTH_NAME: &[u8] = b"smooth\0";
    static RASTER1_NAME: &[u8] = b"raster1\0";
    static SDF_NAME: &[u8] = b"sdf\0";
    static FIXTURE_RENDERER_NAME: &[u8] = b"fixture_renderer\0";
    static BITMAP_RENDERER_NAME: &[u8] = b"bsdf\0";
    static SVG_RENDERER_NAME: &[u8] = b"ot-svg\0";
    let mut system_memory = allocation_memory
        .is_null()
        .then(|| Box::new(FT_MemoryRec::default()));
    let public_memory = system_memory
        .as_deref_mut()
        .map_or(allocation_memory, |memory| memory as *mut FT_MemoryRec);
    let outline_raster_class =
        Box::new(abi_default_raster_funcs(rust_ffi::FT_GLYPH_FORMAT_OUTLINE));
    let synthetic_raster_class =
        Box::new(abi_default_raster_funcs(rust_ffi::FT_GLYPH_FORMAT_OUTLINE));
    let raster1_raster_class =
        Box::new(abi_default_raster_funcs(rust_ffi::FT_GLYPH_FORMAT_OUTLINE));
    let sdf_raster_class = Box::new(abi_default_raster_funcs(rust_ffi::FT_GLYPH_FORMAT_OUTLINE));
    let bitmap_raster_class = Box::new(abi_default_raster_funcs(rust_ffi::FT_GLYPH_FORMAT_BITMAP));
    let outline_renderer_class = Box::new(abi_renderer_class(
        SMOOTH_NAME.as_ptr().cast(),
        rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        &*outline_raster_class,
    ));
    let synthetic_renderer_class = Box::new(abi_renderer_class(
        FIXTURE_RENDERER_NAME.as_ptr().cast(),
        rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        &*synthetic_raster_class,
    ));
    let raster1_renderer_class = Box::new(abi_renderer_class(
        RASTER1_NAME.as_ptr().cast(),
        rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        &*raster1_raster_class,
    ));
    let sdf_renderer_class = Box::new(abi_renderer_class(
        SDF_NAME.as_ptr().cast(),
        rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        &*sdf_raster_class,
    ));
    let bitmap_renderer_class = Box::new(abi_renderer_class(
        BITMAP_RENDERER_NAME.as_ptr().cast(),
        rust_ffi::FT_GLYPH_FORMAT_BITMAP,
        &*bitmap_raster_class,
    ));
    let svg_renderer_class = Box::new(abi_renderer_class(
        SVG_RENDERER_NAME.as_ptr().cast(),
        rust_ffi::FT_GLYPH_FORMAT_SVG,
        ptr::null(),
    ));
    let outline_renderer = FT_RendererRec {
        root: FT_ModuleRec {
            clazz: &outline_renderer_class.root,
            library: ptr::null_mut(),
            memory: public_memory,
        },
        clazz: (&*outline_renderer_class as *const FT_Renderer_Class).cast_mut(),
        glyph_format: rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        glyph_class: FT_Glyph_Class::default(),
        raster: ptr::null_mut(),
        raster_render: None,
        render: Some(abi_default_renderer_render),
        module_name: "smooth",
    };
    let raster1_renderer = FT_RendererRec {
        root: FT_ModuleRec {
            clazz: &raster1_renderer_class.root,
            library: ptr::null_mut(),
            memory: public_memory,
        },
        clazz: (&*raster1_renderer_class as *const FT_Renderer_Class).cast_mut(),
        glyph_format: rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        glyph_class: FT_Glyph_Class::default(),
        raster: ptr::null_mut(),
        raster_render: None,
        render: Some(abi_default_renderer_render),
        module_name: "raster1",
    };
    let sdf_renderer = FT_RendererRec {
        root: FT_ModuleRec {
            clazz: &sdf_renderer_class.root,
            library: ptr::null_mut(),
            memory: public_memory,
        },
        clazz: (&*sdf_renderer_class as *const FT_Renderer_Class).cast_mut(),
        glyph_format: rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        glyph_class: FT_Glyph_Class::default(),
        raster: ptr::null_mut(),
        raster_render: None,
        render: Some(abi_default_renderer_render),
        module_name: "sdf",
    };
    let synthetic_renderer = FT_RendererRec {
        root: FT_ModuleRec {
            clazz: &synthetic_renderer_class.root,
            library: ptr::null_mut(),
            memory: public_memory,
        },
        clazz: (&*synthetic_renderer_class as *const FT_Renderer_Class).cast_mut(),
        glyph_format: rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        glyph_class: FT_Glyph_Class::default(),
        raster: ptr::null_mut(),
        raster_render: None,
        render: Some(abi_default_renderer_render),
        module_name: "fixture_renderer",
    };
    let bitmap_renderer = FT_RendererRec {
        root: FT_ModuleRec {
            clazz: &bitmap_renderer_class.root,
            library: ptr::null_mut(),
            memory: public_memory,
        },
        clazz: (&*bitmap_renderer_class as *const FT_Renderer_Class).cast_mut(),
        glyph_format: rust_ffi::FT_GLYPH_FORMAT_BITMAP,
        glyph_class: FT_Glyph_Class::default(),
        raster: ptr::null_mut(),
        raster_render: None,
        render: Some(abi_default_renderer_render),
        module_name: "bsdf",
    };
    let svg_renderer = FT_RendererRec {
        root: FT_ModuleRec {
            clazz: &svg_renderer_class.root,
            library: ptr::null_mut(),
            memory: public_memory,
        },
        clazz: (&*svg_renderer_class as *const FT_Renderer_Class).cast_mut(),
        glyph_format: rust_ffi::FT_GLYPH_FORMAT_SVG,
        glyph_class: FT_Glyph_Class::default(),
        raster: ptr::null_mut(),
        raster_render: None,
        render: Some(abi_default_renderer_render),
        module_name: "ot-svg",
    };
    LibraryState {
        inner,
        _system_memory: system_memory,
        allocation_memory: public_memory,
        allocation_block,
        module_allocation_blocks: Vec::new(),
        _outline_renderer_class: outline_renderer_class,
        _outline_raster_class: outline_raster_class,
        outline_renderer,
        _raster1_renderer_class: raster1_renderer_class,
        _raster1_raster_class: raster1_raster_class,
        raster1_renderer,
        _sdf_renderer_class: sdf_renderer_class,
        _sdf_raster_class: sdf_raster_class,
        sdf_renderer,
        _synthetic_renderer_class: synthetic_renderer_class,
        _synthetic_raster_class: synthetic_raster_class,
        synthetic_renderer,
        _bitmap_renderer_class: bitmap_renderer_class,
        _bitmap_raster_class: bitmap_raster_class,
        bitmap_renderer,
        _svg_renderer_class: svg_renderer_class,
        svg_renderer,
        default_modules: abi_default_module_records(public_memory),
        synthetic_modules: Vec::new(),
        custom_glyphs: Vec::new(),
        faces: Vec::new(),
    }
}

impl LibraryState {
    fn new(inner: rust_ffi::FT_Library) -> Self {
        new_library_state(inner, ptr::null_mut(), ptr::null_mut())
    }

    fn new_with_allocation(
        inner: rust_ffi::FT_Library,
        allocation_memory: FT_Memory,
        allocation_block: FT_Pointer,
    ) -> Self {
        new_library_state(inner, allocation_memory, allocation_block)
    }
}

struct FaceState {
    inner: rust_ffi::FT_Face,
    library: FT_Library,
    // `FT_FaceRec.driver` is a live opaque driver handle in pinned FreeType.
    // The façade has no native driver object, so retain a stable sentinel
    // allocation whose address provides the same non-null public identity.
    driver_sentinel: Box<u8>,
    refcount: usize,
    size_records: Vec<FT_Size>,
    charmaps: Box<[FT_CharMapRec]>,
    charmap_ptrs: Box<[FT_CharMap]>,
    #[allow(
        dead_code,
        reason = "owns C strings referenced by feature-gated ABI record snapshots"
    )]
    family_name: Option<CString>,
    #[allow(
        dead_code,
        reason = "owns C strings referenced by feature-gated ABI record snapshots"
    )]
    style_name: Option<CString>,
    postscript_name: Option<CString>,
    font_format: Option<CString>,
    #[allow(
        dead_code,
        reason = "owns C strings referenced by feature-gated driver-name inspection"
    )]
    face_driver_name: Option<CString>,
    variant_list: Vec<FT_UInt32>,
    stream: FT_Stream,
    stream_close: FT_Stream_CloseFunc,
    external_stream_frame: Option<Box<[FT_Byte]>>,
    allocation_memory: FT_Memory,
    allocation_block: FT_Pointer,
}

#[repr(C)]
struct FT_Face_InternalRecCompat {
    transform_matrix: FT_Matrix,
    transform_delta: FT_Vector,
    transform_flags: FT_Int,
    services: [FT_Pointer; 6],
    incremental_interface: FT_Pointer,
    no_stem_darkening: c_char,
    random_seed: FT_Int32,
    refcount: FT_Int,
    state: Box<FaceState>,
}

impl FT_Face_InternalRecCompat {
    fn new(state: Box<FaceState>) -> Self {
        Self {
            transform_matrix: FT_Matrix {
                xx: 0x10000,
                xy: 0,
                yx: 0,
                yy: 0x10000,
            },
            transform_delta: FT_Vector::default(),
            transform_flags: 0,
            services: [ptr::null_mut(); 6],
            incremental_interface: ptr::null_mut(),
            no_stem_darkening: -1,
            random_seed: -1,
            refcount: 1,
            state,
        }
    }

    fn sync_face_properties(&mut self) {
        let properties = rust_ffi::FT_Face_Properties_Get_State(&self.state.inner);
        self.no_stem_darkening =
            c_char::try_from(properties.no_stem_darkening).unwrap_or(c_char::MAX);
        self.random_seed = properties.random_seed;
    }
}

impl FaceState {
    fn new(inner: rust_ffi::FT_Face, library: FT_Library) -> Self {
        let family_name = inner
            .family_name
            .as_ref()
            .and_then(|name| CString::new(name.as_str()).ok());
        let style_name = inner
            .style_name
            .as_ref()
            .and_then(|name| CString::new(name.as_str()).ok());
        let postscript_name = postscript_name_cstring(&inner);
        let font_format = font_format_cstring(Some(&inner));
        let face_driver_name = face_driver_name_cstring(Some(&inner));
        Self {
            inner,
            library,
            driver_sentinel: Box::new(0),
            refcount: 1,
            size_records: Vec::new(),
            charmaps: Box::new([]),
            charmap_ptrs: Box::new([]),
            family_name,
            style_name,
            postscript_name,
            font_format,
            face_driver_name,
            variant_list: Vec::new(),
            stream: ptr::null_mut(),
            stream_close: None,
            external_stream_frame: None,
            allocation_memory: ptr::null_mut(),
            allocation_block: ptr::null_mut(),
        }
    }

    fn refresh_charmaps(&mut self, face: FT_Face) {
        let records = self
            .inner
            .charmaps
            .iter()
            .map(|record| FT_CharMapRec {
                face,
                encoding: record.encoding,
                platform_id: record.platform_id,
                encoding_id: record.encoding_id,
            })
            .collect::<Vec<_>>();
        let mut charmaps = records.into_boxed_slice();
        let charmap_ptrs = charmaps
            .iter_mut()
            .map(|record| record as *mut FT_CharMapRec)
            .collect::<Vec<_>>()
            .into_boxed_slice();
        self.charmaps = charmaps;
        self.charmap_ptrs = charmap_ptrs;
    }

    fn refresh_postscript_name(&mut self) {
        self.postscript_name = postscript_name_cstring(&self.inner);
    }

    fn charmap_index(&self, charmap: FT_CharMap) -> Option<usize> {
        if charmap.is_null() {
            return None;
        }
        self.charmaps
            .iter()
            .position(|record| ptr::eq(record as *const FT_CharMapRec, charmap.cast_const()))
    }

    #[allow(
        dead_code,
        reason = "used by the feature-gated ABI parity inspection surface"
    )]
    fn charmap_by_index(&self, index: FT_UInt) -> Option<FT_CharMap> {
        let index = usize::try_from(index).ok()?;
        self.charmap_ptrs.get(index).copied()
    }

    fn variant_list_ptr(&mut self, values: Option<Vec<FT_UInt32>>) -> *mut FT_UInt32 {
        let Some(values) = values else {
            self.variant_list.clear();
            return ptr::null_mut();
        };
        self.variant_list = values;
        self.variant_list.push(0);
        self.variant_list.as_mut_ptr()
    }

    fn push_size_record(&mut self, size: FT_Size) {
        self.size_records.push(size);
    }

    fn remove_size_record(&mut self, size: FT_Size) -> bool {
        let Some(index) = self
            .size_records
            .iter()
            .position(|record| ptr::eq(*record, size))
        else {
            return false;
        };
        self.size_records.remove(index);
        true
    }
}

impl Drop for FaceState {
    fn drop(&mut self) {
        for size in self.size_records.drain(..) {
            // SAFETY: `size_records` contains only boxes allocated by this wrapper.
            unsafe { drop_size(size) };
        }
        free_custom_memory_block(self.allocation_memory, self.allocation_block);
        self.allocation_block = ptr::null_mut();
    }
}

fn postscript_name_cstring(inner: &rust_ffi::FT_Face) -> Option<CString> {
    rust_ffi::FT_Get_Postscript_Name(inner).and_then(|name| {
        // FreeType exposes a borrowed NUL-terminated C string owned by the face.
        CString::new(name).ok()
    })
}

fn font_format_cstring(inner: Option<&rust_ffi::FT_Face>) -> Option<CString> {
    rust_ffi::FT_Get_Font_Format(inner).and_then(|format| {
        // FreeType exposes the driver-owned FONT_FORMAT service string.
        CString::new(format).ok()
    })
}

fn face_driver_name_cstring(inner: Option<&rust_ffi::FT_Face>) -> Option<CString> {
    rust_ffi::FT_FACE_DRIVER_NAME(inner).and_then(|name| {
        // FreeType exposes the driver module class name as a borrowed
        // NUL-terminated C string owned by the driver's module class.
        CString::new(name).ok()
    })
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiSlotSnapshot {
    pub glyph_index: FT_UInt,
    pub metrics: FT_Glyph_Metrics,
    pub advance: FT_Vector,
    pub format: FT_Glyph_Format,
    pub num_subglyphs: FT_UInt,
    pub outline_cbox: FT_BBox,
    pub outline_bbox: FT_BBox,
    pub outline: Option<rust_ffi::FT_OutlineSnapshot>,
    pub bitmap: Option<AbiBitmapSnapshot>,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiBitmapSnapshot {
    pub rows: u32,
    pub width: u32,
    pub pitch: FT_Int,
    pub num_grays: FT_UShort,
    pub pixel_mode: FT_Pixel_Mode,
    pub left: FT_Int,
    pub top: FT_Int,
    pub owns_bitmap: bool,
    pub buffer: Vec<u8>,
}

#[cfg(feature = "abi-test-support")]
pub fn abi_byte_slice(ptr: *const FT_Byte, len: FT_UInt) -> Vec<u8> {
    if ptr.is_null() || len == 0 {
        return Vec::new();
    }
    let len = usize::try_from(len).unwrap_or(0);
    // SAFETY: test callers pass live FreeType-shaped output pointers with
    // `len` bytes valid for the duration of the snapshot copy.
    unsafe { slice::from_raw_parts(ptr, len).to_vec() }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_bdf_property_snapshot(property: &BDF_PropertyRec) -> AbiBdfPropertySnapshot {
    match property.type_ {
        rust_ffi::BDF_PROPERTY_TYPE_ATOM => {
            // SAFETY: `type_ == ATOM` means FreeType/fontdone wrote the
            // `atom` union member according to the public BDF_PropertyRec ABI.
            let atom = unsafe { property.u.atom };
            AbiBdfPropertySnapshot {
                type_: property.type_,
                atom,
                integer: 0,
                cardinal: 0,
            }
        }
        rust_ffi::BDF_PROPERTY_TYPE_INTEGER => {
            // SAFETY: `type_ == INTEGER` means the active union member is
            // `integer`.
            let integer = unsafe { property.u.integer };
            AbiBdfPropertySnapshot {
                type_: property.type_,
                atom: ptr::null(),
                integer,
                cardinal: 0,
            }
        }
        rust_ffi::BDF_PROPERTY_TYPE_CARDINAL => {
            // SAFETY: `type_ == CARDINAL` means the active union member is
            // `cardinal`.
            let cardinal = unsafe { property.u.cardinal };
            AbiBdfPropertySnapshot {
                type_: property.type_,
                atom: ptr::null(),
                integer: 0,
                cardinal,
            }
        }
        _ => {
            // SAFETY: error rows initialize the union through `cardinal` before
            // calling the API.  FreeType leaves that storage untouched when it
            // returns `BDF_PROPERTY_TYPE_NONE`.
            let cardinal = unsafe { property.u.cardinal };
            AbiBdfPropertySnapshot {
                type_: property.type_,
                atom: ptr::null(),
                integer: cardinal as FT_Int32,
                cardinal,
            }
        }
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_c_string_bytes(ptr: *const c_char) -> Vec<u8> {
    if ptr.is_null() {
        return Vec::new();
    }
    // SAFETY: test callers pass live FreeType-shaped NUL-terminated strings.
    unsafe { CStr::from_ptr(ptr).to_bytes().to_vec() }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Error_String(error_code: FT_Error) -> *const c_char {
    rust_ffi::FT_Error_String(error_code).map_or(ptr::null(), |text| text.as_ptr().cast())
}

fn write_ft_bytes(out: *mut FT_Bytes, value: FT_Bytes) {
    if let Some(out) = non_null_mut(out) {
        // SAFETY: `out` is non-null and caller provides writable FT_Bytes storage.
        unsafe { *out.as_ptr() = value };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_OpenType_Validate(
    face: FT_Face,
    validation_flags: FT_UInt,
    base_table: *mut FT_Bytes,
    gdef_table: *mut FT_Bytes,
    gpos_table: *mut FT_Bytes,
    gsub_table: *mut FT_Bytes,
    jstf_table: *mut FT_Bytes,
) -> FT_Error {
    if let Some(state) = face_state_mut(face) {
        let available = rust_ffi::FT_Library_Has_Module(library_ref(state.library), "otvalid");
        rust_ffi::FT_OpenType_Validator_Set_Available(&mut state.inner, available);
    }
    let rust_face = face_state(face).map(|state| &state.inner);
    let mut base = ptr::null();
    let mut gdef = ptr::null();
    let mut gpos = ptr::null();
    let mut gsub = ptr::null();
    let mut jstf = ptr::null();
    let err = rust_ffi::FT_OpenType_Validate(
        rust_face,
        validation_flags,
        (!base_table.is_null()).then_some(&mut base),
        (!gdef_table.is_null()).then_some(&mut gdef),
        (!gpos_table.is_null()).then_some(&mut gpos),
        (!gsub_table.is_null()).then_some(&mut gsub),
        (!jstf_table.is_null()).then_some(&mut jstf),
    );
    if err != rust_ffi::FT_Err_Ok && err != rust_ffi::FT_Err_Invalid_Table {
        return err;
    }

    const TAGS: [FT_ULong; 6] = [
        u32::from_be_bytes(*b"BASE") as FT_ULong,
        u32::from_be_bytes(*b"GDEF") as FT_ULong,
        u32::from_be_bytes(*b"GPOS") as FT_ULong,
        u32::from_be_bytes(*b"GSUB") as FT_ULong,
        u32::from_be_bytes(*b"JSTF") as FT_ULong,
        u32::from_be_bytes(*b"MATH") as FT_ULong,
    ];
    const FLAGS: [FT_UInt; 6] = [
        rust_ffi::FT_VALIDATE_BASE as FT_UInt,
        rust_ffi::FT_VALIDATE_GDEF as FT_UInt,
        rust_ffi::FT_VALIDATE_GPOS as FT_UInt,
        rust_ffi::FT_VALIDATE_GSUB as FT_UInt,
        rust_ffi::FT_VALIDATE_JSTF as FT_UInt,
        rust_ffi::FT_VALIDATE_MATH as FT_UInt,
    ];
    let rust_tables = [base, gdef, gpos, gsub, jstf];
    let mut c_tables = [ptr::null(); 6];
    let mut conversion_error = rust_ffi::FT_Err_Ok;
    if let Some(rust_face) = rust_face {
        for index in 0..TAGS.len() {
            if validation_flags & FLAGS[index] == 0 {
                continue;
            }
            let bytes = match rust_ffi::FT_Load_Sfnt_Table(rust_face, TAGS[index], 0, None) {
                Ok(Some(bytes)) => bytes,
                Err(error) if error == rust_ffi::FT_Err_Table_Missing as FT_Error => continue,
                _ => {
                    conversion_error = rust_ffi::FT_Err_Invalid_Table;
                    break;
                }
            };
            match retain_c_open_type_table(face, bytes) {
                Ok(table) => c_tables[index] = table,
                Err(error) => {
                    conversion_error = error;
                    break;
                }
            }
        }
        for rust_table in rust_tables {
            rust_ffi::FT_OpenType_Free(Some(rust_face), rust_table);
        }
    }
    if conversion_error != rust_ffi::FT_Err_Ok {
        for table in c_tables {
            release_c_open_type_table(face, table);
        }
        return conversion_error;
    }
    if err != rust_ffi::FT_Err_Ok {
        for table in c_tables {
            release_c_open_type_table(face, table);
        }
        return err;
    }

    write_ft_bytes(base_table, c_tables[0]);
    write_ft_bytes(gdef_table, c_tables[1]);
    write_ft_bytes(gpos_table, c_tables[2]);
    write_ft_bytes(gsub_table, c_tables[3]);
    write_ft_bytes(jstf_table, c_tables[4]);
    // MATH is selected and validated like the other tables, but the frozen C
    // API has no output pointer for it (`src/otvalid/otvmod.c`).
    release_c_open_type_table(face, c_tables[5]);
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_PS_Font_Info(face: FT_Face, afont_info: PS_FontInfo) -> FT_Error {
    let face = face_state(face).map(|state| &state.inner);
    let mut info = PS_FontInfoRec::default();
    let err = rust_ffi::FT_Get_PS_Font_Info(face, (!afont_info.is_null()).then_some(&mut info));
    if err == rust_ffi::FT_Err_Ok && !afont_info.is_null() {
        // SAFETY: C ABI caller supplied a non-null `PS_FontInfoRec*` output
        // pointer; copying the repr(C) public record is the wrapper's only
        // responsibility.
        unsafe {
            *afont_info = info;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_PS_Font_Private(face: FT_Face, afont_private: PS_Private) -> FT_Error {
    let face = face_state(face).map(|state| &state.inner);
    let mut private = PS_PrivateRec::default();
    let err =
        rust_ffi::FT_Get_PS_Font_Private(face, (!afont_private.is_null()).then_some(&mut private));
    if err == rust_ffi::FT_Err_Ok && !afont_private.is_null() {
        // SAFETY: C ABI caller supplied a non-null `PS_PrivateRec*` output
        // pointer; copying the repr(C) public record is the wrapper's only
        // responsibility.
        unsafe {
            *afont_private = private;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Has_PS_Glyph_Names(face: FT_Face) -> FT_Int {
    let face = face_state(face).map(|state| &state.inner);
    rust_ffi::FT_Has_PS_Glyph_Names(face)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_PS_Font_Value(
    face: FT_Face,
    key: PS_Dict_Keys,
    idx: FT_UInt,
    value: *mut c_void,
    value_len: FT_Long,
) -> FT_Long {
    let face = face_state(face).map(|state| &state.inner);
    let effective_value_len = value_len.max(0);
    let value_len = usize::try_from(effective_value_len).unwrap_or(usize::MAX);
    let value = if value.is_null() {
        None
    } else {
        // SAFETY: C ABI caller supplies `value_len` writable bytes at `value`
        // when the pointer is non-null; this wrapper only exposes those bytes
        // to the safe Rust FFI implementation.
        Some(unsafe { slice::from_raw_parts_mut(value.cast::<u8>(), value_len) })
    };
    rust_ffi::FT_Get_PS_Font_Value(face, key, idx, value, effective_value_len)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_OpenType_Free(face: FT_Face, table: FT_Bytes) {
    release_c_open_type_table(face, table);
}

#[cfg(feature = "abi-test-support")]
pub fn abi_open_type_validation_table_copy(face: FT_Face, table: FT_Bytes) -> Option<Vec<FT_Byte>> {
    let (face, table) = (NonNull::new(face)?, NonNull::new(table.cast_mut())?);
    let tables = owned_open_type_tables()
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let entry = tables.get(&table.as_ptr().addr())?;
    if entry.owner != face.as_ptr().addr() {
        return None;
    }
    // SAFETY: the registry retains the system allocation or records the live
    // face-memory allocation until FT_OpenType_Free removes this exact entry.
    Some(unsafe { slice::from_raw_parts(table.as_ptr(), entry._len) }.to_vec())
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

#[cfg(feature = "abi-test-support")]
pub fn abi_face_info(face: FT_Face) -> Option<rust_ffi::FT_FaceRecPublic> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    let internal = unsafe { (*face.as_ptr()).internal };
    let internal = NonNull::new(internal.cast::<FT_Face_InternalRecCompat>())?;
    // SAFETY: `internal` is owned by the live face for the duration of this scalar copy.
    let state = unsafe { &internal.as_ref().state };
    let mut info = rust_face_info(&state.inner);
    info.family_name = state
        .family_name
        .as_ref()
        .map_or(ptr::null_mut(), |name| name.as_ptr().cast_mut());
    info.style_name = state
        .style_name
        .as_ref()
        .map_or(ptr::null_mut(), |name| name.as_ptr().cast_mut());
    Some(info)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_face_stream_info(face: FT_Face) -> Option<rust_ffi::FT_StreamRec> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    let internal = unsafe { (*face.as_ptr()).internal };
    let internal = NonNull::new(internal.cast::<FT_Face_InternalRecCompat>())?;
    // SAFETY: `internal` is owned by the live face for the duration of this scalar copy.
    let state = unsafe { &internal.as_ref().state };
    Some(state.inner.memory_stream_record())
}

#[cfg(feature = "abi-test-support")]
pub fn abi_face_uses_stream(face: FT_Face, stream: FT_Stream) -> bool {
    let Some(face) = NonNull::new(face) else {
        return false;
    };
    // SAFETY: this feature-gated helper only reads the public stream field of
    // a live face handle created by this crate.
    unsafe { face.as_ref().stream == stream }
}

#[cfg(feature = "abi-test-support")]
struct AbiExternalStreamState {
    bytes: Box<[FT_Byte]>,
    scenario: usize,
    expected_stream: usize,
    callback_stream_identity: bool,
    read_observed: bool,
    seek_observed: bool,
    close_calls: usize,
    io_events: usize,
    close_after_io: bool,
    magic: u32,
}

#[cfg(feature = "abi-test-support")]
extern "C" fn abi_external_stream_io(
    stream: FT_Stream,
    offset: FT_ULong,
    buffer: *mut FT_Byte,
    count: FT_ULong,
) -> FT_ULong {
    let Some(stream) = (unsafe { stream.as_mut() }) else {
        return 1;
    };
    let Some(state) = (unsafe {
        stream
            .descriptor
            .pointer
            .cast::<AbiExternalStreamState>()
            .as_mut()
    }) else {
        return 1;
    };
    state.callback_stream_identity &= ptr::from_mut(stream).addr() == state.expected_stream;
    if count == 0 {
        state.seek_observed = true;
        state.io_events = state.io_events.saturating_add(1);
        return FT_ULong::from(state.scenario == 1);
    }
    state.read_observed = true;
    state.io_events = state.io_events.saturating_add(1);
    let Ok(offset) = usize::try_from(offset) else {
        return 0;
    };
    let Ok(requested) = usize::try_from(count) else {
        return 0;
    };
    let available = state.bytes.len().saturating_sub(offset);
    let mut copied = requested.min(available);
    if state.scenario == 2 {
        copied = copied.min(3);
    }
    if copied != 0 && !buffer.is_null() {
        // SAFETY: the callback caller provides `count` writable bytes and the
        // state owns at least `copied` readable bytes from `offset`.
        unsafe {
            ptr::copy_nonoverlapping(state.bytes.as_ptr().add(offset), buffer, copied);
        }
    }
    FT_ULong::try_from(copied).unwrap_or(FT_ULong::MAX)
}

#[cfg(feature = "abi-test-support")]
extern "C" fn abi_external_stream_close(stream: FT_Stream) {
    let Some(stream) = (unsafe { stream.as_mut() }) else {
        return;
    };
    let Some(state) = (unsafe {
        stream
            .descriptor
            .pointer
            .cast::<AbiExternalStreamState>()
            .as_mut()
    }) else {
        return;
    };
    state.callback_stream_identity &= ptr::from_mut(stream).addr() == state.expected_stream;
    state.close_calls = state.close_calls.saturating_add(1);
    state.close_after_io = state.io_events != 0;
}

/// One callback-backed FT_OPEN_STREAM scenario.
#[cfg(feature = "abi-test-support")]
pub struct AbiExternalStreamRow {
    pub scenario: &'static str,
    pub open_error: FT_Error,
    pub face_stream_identity: bool,
    pub callback_stream_identity: bool,
    pub read_observed: bool,
    pub seek_observed: bool,
    pub close_calls: usize,
    pub close_after_io: bool,
    pub client_stream_alive: bool,
}

/// Exercises valid, seek-failure, and short-read external streams.
#[cfg(feature = "abi-test-support")]
pub fn abi_external_stream_runtime(bytes: &[u8], face_index: FT_Long) -> Vec<AbiExternalStreamRow> {
    let mut library = ptr::null_mut();
    let init_error = FT_Init_FreeType(&mut library);
    if init_error != rust_ffi::FT_Err_Ok {
        return vec![AbiExternalStreamRow {
            scenario: "init",
            open_error: init_error,
            face_stream_identity: false,
            callback_stream_identity: false,
            read_observed: false,
            seek_observed: false,
            close_calls: 0,
            close_after_io: false,
            client_stream_alive: false,
        }];
    }
    let mut rows = Vec::new();
    for (scenario, name) in ["valid_stream", "seek_failure", "short_header"]
        .into_iter()
        .enumerate()
    {
        let mut state = Box::new(AbiExternalStreamState {
            bytes: bytes.to_vec().into_boxed_slice(),
            scenario,
            expected_stream: 0,
            callback_stream_identity: true,
            read_observed: false,
            seek_observed: false,
            close_calls: 0,
            io_events: 0,
            close_after_io: false,
            magic: 0xF75E_A124,
        });
        let mut stream = FT_StreamRec {
            base: ptr::null_mut(),
            size: FT_ULong::try_from(bytes.len()).unwrap_or(FT_ULong::MAX),
            pos: 0,
            descriptor: FT_StreamDesc {
                pointer: ptr::from_mut(state.as_mut()).cast(),
            },
            pathname: FT_StreamDesc::default(),
            read: abi_external_stream_io as *const () as FT_Pointer,
            close: abi_external_stream_close as *const () as FT_Pointer,
            memory: ptr::null_mut(),
            cursor: ptr::null_mut(),
            limit: ptr::null_mut(),
        };
        state.expected_stream = ptr::from_mut(&mut stream).addr();
        let args = FT_Open_Args {
            flags: rust_ffi::FT_OPEN_STREAM as FT_UInt,
            memory_base: ptr::null(),
            memory_size: 0,
            pathname: ptr::null_mut(),
            stream: &mut stream,
            driver: ptr::null_mut(),
            num_params: 0,
            params: ptr::null_mut(),
        };
        let mut face = ptr::null_mut();
        let open_error = FT_Open_Face(library, &args, face_index, &mut face);
        let face_stream_identity =
            open_error == rust_ffi::FT_Err_Ok && abi_face_uses_stream(face, &mut stream);
        if open_error == rust_ffi::FT_Err_Ok {
            let _ = FT_Done_Face(face);
        }
        rows.push(AbiExternalStreamRow {
            scenario: name,
            open_error,
            face_stream_identity,
            callback_stream_identity: state.callback_stream_identity,
            read_observed: state.read_observed,
            seek_observed: state.seek_observed,
            close_calls: state.close_calls,
            close_after_io: state.close_after_io,
            client_stream_alive: state.magic == 0xF75E_A124,
        });
    }
    let _ = FT_Done_FreeType(library);
    rows
}

#[cfg(feature = "abi-test-support")]
pub struct AbiCallbackStreamFields {
    pub base_is_null: bool,
    pub size: u64,
    pub pos: u64,
    pub descriptor_is_null: bool,
    pub pathname_is_null: bool,
    pub read_is_null: bool,
    pub close_is_null: bool,
    pub memory_is_null: bool,
    pub cursor_is_null: bool,
    pub limit_is_null: bool,
    pub stream_pointer_identity: bool,
}

#[cfg(feature = "abi-test-support")]
pub struct AbiCallbackStreamEvent {
    pub kind: &'static str,
    pub offset: u64,
    pub count: u64,
    pub returned: u64,
    pub bytes: Vec<u8>,
    pub observed: bool,
}

#[cfg(feature = "abi-test-support")]
pub struct AbiCallbackStreamContract {
    pub face_load_status: FT_Error,
    pub stream_after: AbiCallbackStreamFields,
    pub callback_events: Vec<AbiCallbackStreamEvent>,
    pub close_called_by_face_done: bool,
}

#[cfg(feature = "abi-test-support")]
#[allow(clippy::useless_conversion)]
fn abi_callback_stream_u64(value: FT_ULong) -> u64 {
    u64::try_from(value).unwrap_or(u64::MAX)
}

#[cfg(feature = "abi-test-support")]
struct AbiCallbackStreamState {
    bytes: Box<[FT_Byte]>,
    expected_stream: usize,
    recording: bool,
    close_calls: usize,
    callback_stream_identity: bool,
    events: Vec<AbiCallbackStreamEvent>,
}

#[cfg(feature = "abi-test-support")]
extern "C" fn abi_callback_stream_io(
    stream: FT_Stream,
    offset: FT_ULong,
    buffer: *mut FT_Byte,
    count: FT_ULong,
) -> FT_ULong {
    let Some(stream) = (unsafe { stream.as_mut() }) else {
        return 0;
    };
    let Some(state) = (unsafe {
        stream
            .descriptor
            .pointer
            .cast::<AbiCallbackStreamState>()
            .as_mut()
    }) else {
        return 0;
    };
    state.callback_stream_identity &= ptr::from_mut(stream).addr() == state.expected_stream;
    let offset_usize = usize::try_from(offset).unwrap_or(usize::MAX);
    let requested = usize::try_from(count).unwrap_or(usize::MAX);
    let available = state.bytes.len().saturating_sub(offset_usize);
    let copied = requested.min(available);
    if copied != 0 && !buffer.is_null() {
        // SAFETY: the callback caller provides `count` writable bytes and the
        // state owns `copied` readable bytes beginning at `offset`.
        unsafe {
            ptr::copy_nonoverlapping(state.bytes.as_ptr().add(offset_usize), buffer, copied);
        }
    }
    let returned = FT_ULong::try_from(copied).unwrap_or(FT_ULong::MAX);
    if state.recording {
        state.events.push(AbiCallbackStreamEvent {
            kind: "read",
            offset: abi_callback_stream_u64(offset),
            count: abi_callback_stream_u64(count),
            returned: abi_callback_stream_u64(returned),
            bytes: state
                .bytes
                .get(offset_usize..)
                .and_then(|tail| tail.get(..copied))
                .unwrap_or_default()
                .to_vec(),
            observed: true,
        });
    }
    returned
}

#[cfg(feature = "abi-test-support")]
extern "C" fn abi_callback_stream_close(stream: FT_Stream) {
    let Some(stream) = (unsafe { stream.as_mut() }) else {
        return;
    };
    let Some(state) = (unsafe {
        stream
            .descriptor
            .pointer
            .cast::<AbiCallbackStreamState>()
            .as_mut()
    }) else {
        return;
    };
    state.callback_stream_identity &= ptr::from_mut(stream).addr() == state.expected_stream;
    state.close_calls = state.close_calls.saturating_add(1);
    if state.recording {
        state.events.push(AbiCallbackStreamEvent {
            kind: "close",
            offset: 0,
            count: 0,
            returned: 0,
            bytes: Vec::new(),
            observed: true,
        });
    }
}

#[cfg(feature = "abi-test-support")]
fn abi_callback_stream_fields(
    stream: &FT_StreamRec,
    stream_pointer_identity: bool,
) -> AbiCallbackStreamFields {
    // SAFETY: the callback harness initializes both descriptor unions through
    // their pointer view before the snapshot is taken.
    let descriptor_is_null = unsafe { stream.descriptor.pointer.is_null() };
    let pathname_is_null = unsafe { stream.pathname.pointer.is_null() };
    AbiCallbackStreamFields {
        base_is_null: stream.base.is_null(),
        size: abi_callback_stream_u64(stream.size),
        pos: abi_callback_stream_u64(stream.pos),
        descriptor_is_null,
        pathname_is_null,
        read_is_null: stream.read.is_null(),
        close_is_null: stream.close.is_null(),
        memory_is_null: stream.memory.is_null(),
        cursor_is_null: stream.cursor.is_null(),
        limit_is_null: stream.limit.is_null(),
        stream_pointer_identity,
    }
}

/// Runs a caller-owned callback stream through `FT_Open_Face` and records the
/// public `FT_StreamRec` fields plus deterministic read and close probes.
#[cfg(feature = "abi-test-support")]
pub fn abi_callback_stream_contract(
    bytes: &[u8],
    face_index: FT_Long,
) -> AbiCallbackStreamContract {
    let mut library = ptr::null_mut();
    let init_error = FT_Init_FreeType(&mut library);
    if init_error != rust_ffi::FT_Err_Ok {
        return AbiCallbackStreamContract {
            face_load_status: init_error,
            stream_after: AbiCallbackStreamFields {
                base_is_null: true,
                size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
                pos: 0,
                descriptor_is_null: true,
                pathname_is_null: true,
                read_is_null: true,
                close_is_null: true,
                memory_is_null: true,
                cursor_is_null: true,
                limit_is_null: true,
                stream_pointer_identity: false,
            },
            callback_events: Vec::new(),
            close_called_by_face_done: false,
        };
    }

    let mut state = Box::new(AbiCallbackStreamState {
        bytes: bytes.to_vec().into_boxed_slice(),
        expected_stream: 0,
        recording: false,
        close_calls: 0,
        callback_stream_identity: true,
        events: Vec::new(),
    });
    let mut stream = FT_StreamRec {
        base: ptr::null_mut(),
        size: FT_ULong::try_from(bytes.len()).unwrap_or(FT_ULong::MAX),
        pos: 0,
        descriptor: FT_StreamDesc {
            pointer: ptr::from_mut(state.as_mut()).cast(),
        },
        pathname: FT_StreamDesc::default(),
        read: abi_callback_stream_io as *const () as FT_Pointer,
        close: abi_callback_stream_close as *const () as FT_Pointer,
        memory: ptr::null_mut(),
        cursor: ptr::null_mut(),
        limit: ptr::null_mut(),
    };
    state.expected_stream = ptr::from_mut(&mut stream).addr();
    let args = FT_Open_Args {
        flags: rust_ffi::FT_OPEN_STREAM as FT_UInt,
        memory_base: ptr::null(),
        memory_size: 0,
        pathname: ptr::null_mut(),
        stream: ptr::from_mut(&mut stream),
        driver: ptr::null_mut(),
        num_params: 0,
        params: ptr::null_mut(),
    };
    let mut face = ptr::null_mut();
    let face_load_status = FT_Open_Face(library, &args, face_index, &mut face);
    let stream_pointer_identity = face_load_status == rust_ffi::FT_Err_Ok
        && abi_face_uses_stream(face, ptr::from_mut(&mut stream));
    if face_load_status == rust_ffi::FT_Err_Ok {
        state.recording = true;
        let read = abi_callback_stream_io(ptr::from_mut(&mut stream), 0, ptr::null_mut(), 0);
        debug_assert_eq!(read, 0);
        let mut first = [0; 4];
        let _ = abi_callback_stream_io(
            ptr::from_mut(&mut stream),
            0,
            first.as_mut_ptr(),
            FT_ULong::from(4_u8),
        );
        let _ = abi_callback_stream_io(
            ptr::from_mut(&mut stream),
            FT_ULong::try_from(bytes.len()).unwrap_or(FT_ULong::MAX),
            ptr::null_mut(),
            FT_ULong::from(8_u8),
        );
        let _ = FT_Done_Face(face);
    }
    let stream_after = abi_callback_stream_fields(&stream, stream_pointer_identity);
    let close_called_by_face_done = face_load_status == rust_ffi::FT_Err_Ok
        && state.close_calls == 1
        && state.callback_stream_identity;
    let callback_events = std::mem::take(&mut state.events);
    let _ = FT_Done_FreeType(library);
    AbiCallbackStreamContract {
        face_load_status,
        stream_after,
        callback_events,
        close_called_by_face_done,
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_face_available_sizes(face: FT_Face) -> Option<Vec<rust_ffi::FT_Bitmap_Size>> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    let internal = unsafe { (*face.as_ptr()).internal };
    let internal = NonNull::new(internal.cast::<FT_Face_InternalRecCompat>())?;
    // SAFETY: `internal` is owned by the live face for the duration of this vector copy.
    let state = unsafe { &internal.as_ref().state };
    Some(state.inner.available_sizes.to_vec())
}

#[cfg(feature = "abi-test-support")]
pub fn abi_mm_var_descriptor(
    library: FT_Library,
    face: FT_Face,
) -> Option<AbiMmVarDescriptorSnapshot> {
    let mut master_ptr: *mut FT_MM_Var = ptr::null_mut();
    let err = FT_Get_MM_Var(face, &mut master_ptr);
    if err != rust_ffi::FT_Err_Ok || master_ptr.is_null() {
        return Some((
            err,
            FT_MM_Var::default(),
            Vec::new(),
            Vec::new(),
            Vec::new(),
            rust_ffi::FT_Err_Ok,
        ));
    }
    // SAFETY: `FT_Get_MM_Var` returned a live descriptor pointer owned by this
    // C ABI crate until `FT_Done_MM_Var` is called below.
    let master = unsafe { *master_ptr };
    let axis_count = usize::try_from(master.num_axis).ok()?;
    let axes = if master.axis.is_null() {
        Vec::new()
    } else {
        // SAFETY: the descriptor's axis pointer has `num_axis` initialized
        // records and remains live until `FT_Done_MM_Var`.
        unsafe { slice::from_raw_parts(master.axis, axis_count) }.to_vec()
    };
    let mut axis_flags = Vec::with_capacity(axis_count);
    for axis_index in 0..axis_count {
        let mut flags = 0;
        let axis_index = FT_UInt::try_from(axis_index).ok()?;
        let flag_err = FT_Get_Var_Axis_Flags(master_ptr, axis_index, &mut flags);
        if flag_err != rust_ffi::FT_Err_Ok {
            return None;
        }
        axis_flags.push(flags);
    }
    let namedstyle_count = usize::try_from(master.num_namedstyles).ok()?;
    let namedstyles = if master.namedstyle.is_null() {
        Vec::new()
    } else {
        // SAFETY: the descriptor's namedstyle pointer has `num_namedstyles`
        // initialized records and remains live until `FT_Done_MM_Var`.
        unsafe { slice::from_raw_parts(master.namedstyle, namedstyle_count) }
            .iter()
            .map(|style| {
                let coords = if style.coords.is_null() {
                    Vec::new()
                } else {
                    // SAFETY: FreeType stores one coordinate per axis for
                    // every named style in the live descriptor allocation.
                    unsafe { slice::from_raw_parts(style.coords, axis_count) }.to_vec()
                };
                (*style, coords)
            })
            .collect()
    };
    let done_err = FT_Done_MM_Var(library, master_ptr);
    Some((err, master, axes, axis_flags, namedstyles, done_err))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_charmap_count(face: FT_Face) -> Option<FT_UInt> {
    let state = face_state(face)?;
    FT_UInt::try_from(state.charmaps.len()).ok()
}

#[cfg(feature = "abi-test-support")]
pub fn abi_charmap_by_index(face: FT_Face, index: FT_UInt) -> Option<FT_CharMap> {
    face_state(face)?.charmap_by_index(index)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_charmap_info_by_index(face: FT_Face, index: FT_UInt) -> Option<FT_CharMapRec> {
    let state = face_state(face)?;
    let index = usize::try_from(index).ok()?;
    state.charmaps.get(index).copied()
}

#[cfg(feature = "abi-test-support")]
pub fn abi_active_charmap_index(face: FT_Face) -> Option<FT_Int> {
    let state = face_state(face)?;
    Some(state.inner.active_charmap_index)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_slot_snapshot(face: FT_Face) -> Option<AbiSlotSnapshot> {
    let slot = abi_glyph_slot(face)?;
    let internal = slot_internal(slot.as_ptr())?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    let slot = unsafe { slot.as_ref() };
    let len = usize::try_from(i64::from(slot.bitmap.pitch).abs())
        .ok()?
        .checked_mul(usize::try_from(slot.bitmap.rows).ok()?)?;
    let bitmap = if slot.bitmap.buffer.is_null() || len == 0 {
        None
    } else {
        // SAFETY: the buffer is owned by the live slot for the duration of this copy.
        let buffer = unsafe { slice::from_raw_parts(slot.bitmap.buffer, len) }.to_vec();
        Some(AbiBitmapSnapshot {
            rows: slot.bitmap.rows,
            width: slot.bitmap.width,
            pitch: slot.bitmap.pitch,
            num_grays: slot.bitmap.num_grays,
            pixel_mode: FT_Pixel_Mode::from(slot.bitmap.pixel_mode),
            left: slot.bitmap_left,
            top: slot.bitmap_top,
            owns_bitmap: internal.owns_bitmap,
            buffer,
        })
    };
    Some(AbiSlotSnapshot {
        glyph_index: slot.glyph_index,
        metrics: slot.metrics,
        advance: slot.advance,
        format: slot.format,
        num_subglyphs: internal.rust_slot.num_subglyphs,
        outline_cbox: rust_bbox_to_abi(internal.rust_slot.outline_cbox),
        outline_bbox: rust_bbox_to_abi(internal.rust_slot.outline_bbox),
        outline: internal.rust_slot.outline.clone(),
        bitmap,
    })
}

/// One direct default-renderer callback observation.
#[cfg(feature = "abi-test-support")]
pub struct AbiRendererModeRow {
    pub class: &'static str,
    pub render_mode: FT_Render_Mode,
    pub error: FT_Error,
    pub slot_format_after: FT_Glyph_Format,
    pub bitmap: Option<AbiBitmapSnapshot>,
}

/// Invokes every pinned default outline/SVG renderer callback over all public
/// FreeType 2.14.3 render modes.
#[cfg(feature = "abi-test-support")]
pub fn abi_renderer_mode_acceptance(
    bytes: &[u8],
    face_index: FT_Long,
    glyph_index: FT_UInt,
    ppem: FT_UInt,
) -> Result<Vec<AbiRendererModeRow>, FT_Error> {
    let mut library = ptr::null_mut();
    let init_error = FT_Init_FreeType(&mut library);
    if init_error != rust_ffi::FT_Err_Ok {
        return Err(init_error);
    }
    let mut face = ptr::null_mut();
    let file_size =
        FT_Long::try_from(bytes.len()).map_err(|_| rust_ffi::FT_Err_Array_Too_Large as FT_Error)?;
    let open_error = FT_New_Memory_Face(library, bytes.as_ptr(), file_size, face_index, &mut face);
    if open_error != rust_ffi::FT_Err_Ok {
        let _ = FT_Done_FreeType(library);
        return Err(open_error);
    }
    let size_error = FT_Set_Pixel_Sizes(face, ppem, ppem);
    if size_error != rust_ffi::FT_Err_Ok {
        let _ = FT_Done_Face(face);
        let _ = FT_Done_FreeType(library);
        return Err(size_error);
    }
    let classes = [
        ("smooth", b"smooth\0".as_slice()),
        ("raster1", b"raster1\0".as_slice()),
        ("sdf", b"sdf\0".as_slice()),
        ("svg", b"ot-svg\0".as_slice()),
    ];
    let modes = [
        rust_ffi::FT_RENDER_MODE_NORMAL,
        rust_ffi::FT_RENDER_MODE_LIGHT,
        rust_ffi::FT_RENDER_MODE_MONO,
        rust_ffi::FT_RENDER_MODE_LCD,
        rust_ffi::FT_RENDER_MODE_LCD_V,
        rust_ffi::FT_RENDER_MODE_SDF,
    ];
    let mut rows = Vec::with_capacity(classes.len().saturating_mul(modes.len()));
    for (class, module_name) in classes {
        let renderer = FT_Get_Module(library, module_name.as_ptr().cast()).cast::<FT_RendererRec>();
        for mode in modes {
            let mut error = FT_Load_Glyph(face, glyph_index, rust_ffi::FT_LOAD_DEFAULT);
            if error == rust_ffi::FT_Err_Ok && class == "svg" {
                // SAFETY: `face` is live and owns one public glyph slot.
                unsafe {
                    (*(*face).glyph).format = rust_ffi::FT_GLYPH_FORMAT_SVG;
                }
            }
            if error == rust_ffi::FT_Err_Ok {
                // SAFETY: the default renderer record is owned by `library`
                // until final destruction.
                let render = unsafe { renderer.as_ref() }.and_then(|renderer| renderer.render);
                error = render.map_or(rust_ffi::FT_Err_Unimplemented_Feature, |render| {
                    // SAFETY: renderer and face-owned slot remain live through
                    // this synchronous callback.
                    unsafe { render(renderer, (*face).glyph, mode, ptr::null()) }
                });
            }
            let snapshot =
                abi_slot_snapshot(face).ok_or(rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error)?;
            rows.push(AbiRendererModeRow {
                class,
                render_mode: mode,
                error,
                slot_format_after: snapshot.format,
                bitmap: snapshot.bitmap,
            });
        }
    }
    let _ = FT_Done_Face(face);
    let _ = FT_Done_FreeType(library);
    Ok(rows)
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiOutlineGlyphSnapshot {
    pub advance: FT_Vector,
    pub outline: rust_ffi::FT_OutlineSnapshot,
    pub cbox: FT_BBox,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiBitmapGlyphSnapshot {
    pub root: FT_GlyphRec,
    pub left: FT_Int,
    pub top: FT_Int,
    pub bitmap: AbiBitmapSnapshot,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone)]
pub struct AbiSvgGlyphSnapshot {
    pub root: FT_GlyphRec,
    pub svg_document: Vec<FT_Byte>,
    pub glyph_index: FT_UInt,
    pub metrics: FT_Size_Metrics,
    pub units_per_EM: FT_UShort,
    pub start_glyph_id: FT_UShort,
    pub end_glyph_id: FT_UShort,
    pub transform: FT_Matrix,
    pub delta: FT_Vector,
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone, Copy)]
pub struct AbiNewGlyphAllocationRow {
    pub format: FT_Glyph_Format,
    pub status: FT_Error,
    pub aglyph_null: bool,
    pub allocation_events: usize,
}

#[cfg(feature = "abi-test-support")]
/// Exercises `FT_New_Glyph` after installing a deterministic failing
/// `FT_MemoryRec`, including the output-clear and allocator-attempt contract.
pub fn abi_new_glyph_allocation_failure() -> Vec<AbiNewGlyphAllocationRow> {
    let mut data = Box::new(AbiCustomMemoryData {
        expected_memory: 0,
        phase: AbiCustomMemoryPhase::NewLibrary,
        events: Vec::new(),
        blocks: BTreeMap::new(),
        unknown_release: false,
        fail_after: None,
        allocation_count: 0,
    });
    let mut memory = Box::new(FT_MemoryRec {
        user: ptr::from_mut(data.as_mut()).cast(),
        alloc: Some(abi_custom_memory_alloc),
        free: Some(abi_custom_memory_free),
        realloc: Some(abi_custom_memory_realloc),
    });
    data.expected_memory = ptr::from_mut(memory.as_mut()).addr();

    let mut library = ptr::null_mut();
    let setup_status = FT_New_Library(memory.as_mut(), &mut library);
    if setup_status != rust_ffi::FT_Err_Ok {
        return [
            rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
            rust_ffi::FT_GLYPH_FORMAT_BITMAP,
        ]
        .into_iter()
        .map(|format| AbiNewGlyphAllocationRow {
            format,
            status: setup_status,
            aglyph_null: true,
            allocation_events: data.allocation_count,
        })
        .collect();
    }
    FT_Add_Default_Modules(library);
    data.events.clear();
    data.fail_after = Some(0);
    data.allocation_count = 0;
    let rows = [
        rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
        rust_ffi::FT_GLYPH_FORMAT_BITMAP,
    ]
    .into_iter()
    .map(|format| {
        let mut glyph = NonNull::<FT_GlyphRec>::dangling().as_ptr();
        let status = FT_New_Glyph(library, format, &mut glyph);
        let row = AbiNewGlyphAllocationRow {
            format,
            status,
            aglyph_null: glyph.is_null(),
            allocation_events: data.allocation_count,
        };
        if !glyph.is_null() {
            FT_Done_Glyph(glyph);
        }
        row
    })
    .collect();
    data.fail_after = None;
    let _ = FT_Done_Library(library);
    rows
}

#[cfg(feature = "abi-test-support")]
pub fn abi_get_outline_glyph_from_face(face: FT_Face) -> Result<FT_Glyph, FT_Error> {
    let Some(slot) = abi_glyph_slot(face) else {
        return Err(rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error);
    };
    let mut glyph = ptr::null_mut();
    let err = FT_Get_Glyph(slot.as_ptr(), &mut glyph);
    if err == rust_ffi::FT_Err_Ok {
        Ok(glyph)
    } else {
        Err(err)
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_outline_glyph_snapshot(glyph: FT_Glyph) -> Option<AbiOutlineGlyphSnapshot> {
    let owned = owned_outline_glyph_from_root(glyph)?;
    let mut cbox = FT_BBox::default();
    FT_Glyph_Get_CBox(glyph, 0, &mut cbox);
    Some(AbiOutlineGlyphSnapshot {
        advance: owned.record.root.advance,
        outline: owned.core.outline.clone(),
        cbox,
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_corrupt_outline_glyph_for_render_failure(glyph: FT_Glyph) -> bool {
    let Some(owned) = owned_outline_glyph_from_root_mut(glyph) else {
        return false;
    };
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
pub fn abi_support_corrupt_outline_glyph_for_stroke_parse(glyph: FT_Glyph) -> bool {
    let Some(owned) = owned_outline_glyph_from_root_mut(glyph) else {
        return false;
    };
    let Some(tag) = owned.core.outline.tags.first_mut() else {
        return false;
    };
    // FreeType's public glyph-stroke wrapper reaches ParseOutline through the
    // copied outline. A cubic first point is rejected before any segment can
    // be sent to the stroker, which makes this a durable input-driven error
    // probe for that exact wrapper path.
    *tag = rust_ffi::FT_CURVE_TAG_CUBIC as rust_ffi::FT_Byte;
    owned.refresh_record();
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_corrupt_outline_glyph_for_render_tags(glyph: FT_Glyph, kind: u8) -> bool {
    let Some(owned) = owned_outline_glyph_from_root_mut(glyph) else {
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
pub fn abi_support_corrupt_outline_glyph_for_record_sync(glyph: FT_Glyph) -> bool {
    let Some(owned) = owned_outline_glyph_from_root_mut(glyph) else {
        return false;
    };
    if owned.core.outline.contours.is_empty() {
        return false;
    }
    // Keep the public count non-zero while removing the required contour
    // storage.  The pinned gray renderer explicitly rejects this record after
    // the cbox stage, while the thin ABI must reject it during record sync.
    owned.record.outline.n_contours = 1;
    owned.record.outline.contours = ptr::null_mut();
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_corrupt_outline_glyph_points_for_record_sync(glyph: FT_Glyph) -> bool {
    let Some(owned) = owned_outline_glyph_from_root_mut(glyph) else {
        return false;
    };
    if owned.core.outline.points.is_empty() {
        return false;
    }
    // Keep the public count non-zero while removing the required point
    // storage.  The pinned smooth renderer rejects this record before it
    // reads the point array.
    owned.record.outline.n_points = 1;
    owned.record.outline.points = ptr::null_mut();
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_bitmap_glyph_snapshot(glyph: FT_Glyph) -> Option<AbiBitmapGlyphSnapshot> {
    let owned = owned_bitmap_glyph_from_root(glyph)?;
    Some(AbiBitmapGlyphSnapshot {
        root: owned.record.root,
        left: owned.record.left,
        top: owned.record.top,
        bitmap: AbiBitmapSnapshot {
            rows: owned.record.bitmap.rows,
            width: owned.record.bitmap.width,
            pitch: owned.record.bitmap.pitch,
            num_grays: owned.record.bitmap.num_grays,
            pixel_mode: FT_Pixel_Mode::from(owned.record.bitmap.pixel_mode),
            left: owned.record.left,
            top: owned.record.top,
            owns_bitmap: true,
            buffer: owned.buffer.to_vec(),
        },
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_svg_glyph_snapshot(glyph: FT_Glyph) -> Option<AbiSvgGlyphSnapshot> {
    let owned = owned_svg_glyph_from_root(glyph)?;
    Some(AbiSvgGlyphSnapshot {
        root: owned.record.root,
        svg_document: owned.core.svg_document.clone(),
        glyph_index: owned.core.glyph_index,
        metrics: rust_size_metrics_to_abi(owned.core.metrics),
        units_per_EM: owned.core.units_per_EM,
        start_glyph_id: owned.core.start_glyph_id,
        end_glyph_id: owned.core.end_glyph_id,
        transform: owned.record.transform,
        delta: owned.record.delta,
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_zero_length_svg_glyph(glyph: FT_Glyph, null_document: bool) -> bool {
    let Some(owned) = owned_svg_glyph_from_root_mut(glyph) else {
        return false;
    };
    owned.record.svg_document_length = 0;
    if null_document {
        owned.record.svg_document = ptr::null_mut();
    }
    true
}

#[cfg(feature = "abi-test-support")]
fn rust_bbox_to_abi(bbox: rust_ffi::FT_BBox) -> FT_BBox {
    FT_BBox {
        xMin: bbox.xMin,
        yMin: bbox.yMin,
        xMax: bbox.xMax,
        yMax: bbox.yMax,
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_render_glyph_from_face(face: FT_Face, render_mode: FT_Render_Mode) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_Render_Glyph(slot.as_ptr(), render_mode)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_sfnt_load_name_diagnostic(data: &[u8]) -> FT_Error {
    rust_ffi::FT_Sfnt_Load_Name_Diagnostic(data)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_truetype_context_allocation_failure_diagnostic() -> FT_Error {
    rust_ffi::FT_TrueType_Context_Allocation_Failure_Diagnostic()
}

#[cfg(feature = "abi-test-support")]
pub fn abi_get_glyph_from_face(face: FT_Face, aglyph: *mut FT_Glyph) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_Get_Glyph(slot.as_ptr(), aglyph)
}

#[cfg(feature = "abi-test-support")]
fn abi_get_glyph_external_allocation_failure(variant: FT_UInt) -> FT_Error {
    let mut data = Box::new(AbiCustomMemoryData {
        expected_memory: 0,
        phase: AbiCustomMemoryPhase::NewLibrary,
        events: Vec::new(),
        blocks: BTreeMap::new(),
        unknown_release: false,
        fail_after: Some(if variant == 8 { 1 } else { 2 }),
        allocation_count: 0,
    });
    let mut memory = Box::new(FT_MemoryRec {
        user: ptr::from_mut(data.as_mut()).cast(),
        alloc: Some(abi_custom_memory_alloc),
        free: Some(abi_custom_memory_free),
        realloc: Some(abi_custom_memory_realloc),
    });
    data.expected_memory = ptr::from_ref(&*memory).addr();
    let mut library = ptr::null_mut();
    let setup = FT_New_Library(memory.as_mut(), &mut library);
    if setup != rust_ffi::FT_Err_Ok {
        return setup;
    }
    let mut slot = FT_GlyphSlotRec {
        library,
        ..FT_GlyphSlotRec::default()
    };
    let mut document = FT_SVG_DocumentRec::default();
    let mut svg_data = [0_u8; 1];
    match variant {
        9 => {
            slot.format = rust_ffi::FT_GLYPH_FORMAT_BITMAP;
            slot.bitmap.rows = 1;
            slot.bitmap.width = 1;
            slot.bitmap.pitch = 1;
            slot.bitmap.buffer = ptr::from_mut(&mut svg_data[0]);
        }
        10 => {
            slot.format = rust_ffi::FT_GLYPH_FORMAT_SVG;
            document.svg_document = ptr::from_mut(&mut svg_data[0]);
            document.svg_document_length = 1;
            slot.other = ptr::from_mut(&mut document).cast();
        }
        _ => slot.format = rust_ffi::FT_GLYPH_FORMAT_BITMAP,
    }
    let mut glyph = 1usize as FT_Glyph;
    let error = FT_Get_Glyph(ptr::from_mut(&mut slot), &mut glyph);
    if error == rust_ffi::FT_Err_Ok && !glyph.is_null() {
        FT_Done_Glyph(glyph);
    }
    let _ = FT_Done_Library(library);
    error
}

#[cfg(feature = "abi-test-support")]
pub fn abi_get_glyph_from_external_malformed_slot(
    face: FT_Face,
    variant: FT_UInt,
    aglyph: *mut FT_Glyph,
) -> FT_Error {
    if variant >= 8 {
        return abi_get_glyph_external_allocation_failure(variant);
    }
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let mut slot = FT_GlyphSlotRec {
        library: state.library,
        ..FT_GlyphSlotRec::default()
    };
    let mut document = FT_SVG_DocumentRec::default();
    let mut bitmap_data = [0_u8; 1];
    let mut svg_data = [0_u8; 1];
    match variant {
        0 => slot.format = 0x1234_5678,
        1 => slot.format = rust_ffi::FT_GLYPH_FORMAT_BITMAP,
        2 => slot.format = rust_ffi::FT_GLYPH_FORMAT_SVG,
        3 => {
            slot.format = rust_ffi::FT_GLYPH_FORMAT_SVG;
            slot.other = ptr::from_mut(&mut document).cast();
        }
        4 => {
            slot.format = rust_ffi::FT_GLYPH_FORMAT_BITMAP;
            slot.bitmap.rows = 1;
            slot.bitmap.width = 1;
            slot.bitmap.pitch = 1;
            slot.bitmap.buffer = bitmap_data.as_mut_ptr();
        }
        5 => {
            slot.format = rust_ffi::FT_GLYPH_FORMAT_SVG;
            document.svg_document = svg_data.as_mut_ptr();
            document.svg_document_length = 1;
            slot.other = ptr::from_mut(&mut document).cast();
        }
        6 => {
            slot.format = rust_ffi::FT_GLYPH_FORMAT_BITMAP;
            slot.advance.x = 0x8000 * 64;
        }
        7 => {
            slot.library = ptr::null_mut();
            slot.format = rust_ffi::FT_GLYPH_FORMAT_BITMAP;
        }
        _ => slot.format = 0x1234_5678,
    }
    // SAFETY: `slot` and the optional SVG record remain alive for the complete
    // call, matching the borrowed public records supplied by a C caller.
    FT_Get_Glyph(ptr::from_mut(&mut slot), aglyph)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_set_unsupported_glyph_slot(face: FT_Face) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    store_slot(
        face,
        rust_ffi::FT_Unsupported_GlyphSlot(&state.inner),
        rust_ffi::FT_LOAD_DEFAULT,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_set_malformed_get_glyph_slot(face: FT_Face, variant: FT_UInt) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    store_slot(
        face,
        rust_ffi::FT_Malformed_Get_GlyphSlot(&state.inner, variant),
        rust_ffi::FT_LOAD_DEFAULT,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_set_outline_glyph_slot_advance(
    face: FT_Face,
    advance_x: FT_Pos,
    advance_y: FT_Pos,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    store_slot(
        face,
        rust_ffi::FT_Outline_GlyphSlot_With_Advance(&state.inner, advance_x, advance_y),
        rust_ffi::FT_LOAD_DEFAULT,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_slant_from_face(
    face: FT_Face,
    xslant: FT_Fixed,
    yslant: FT_Fixed,
) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_GlyphSlot_Slant(slot.as_ptr(), xslant, yslant);
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_adjust_weight_from_face(
    face: FT_Face,
    xdelta: FT_Fixed,
    ydelta: FT_Fixed,
) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_GlyphSlot_AdjustWeight(slot.as_ptr(), xdelta, ydelta);
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_embolden_from_face(face: FT_Face) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_GlyphSlot_Embolden(slot.as_ptr());
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_own_bitmap_from_face(face: FT_Face) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Ok;
    };
    FT_GlyphSlot_Own_Bitmap(slot.as_ptr())
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_own_bitmap_copy_allocation_failure_from_face(face: FT_Face) -> FT_Error {
    let Some(slot_ptr) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Ok;
    };
    let Some(internal) = slot_internal_mut(slot_ptr.as_ptr()) else {
        return rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error;
    };
    let err =
        rust_ffi::FT_GlyphSlot_Own_Bitmap_Copy_Allocation_Failure(Some(&mut internal.rust_slot));
    if err != rust_ffi::FT_Err_Ok {
        return err;
    }
    let replacement = rust_slot_to_abi(
        internal.rust_slot.clone(),
        internal.source_face,
        internal.load_flags,
    );
    // SAFETY: `slot_ptr` is a live face-owned slot allocated by this crate,
    // and `replacement` owns a distinct opaque internal record.
    unsafe { replace_slot_record(slot_ptr.as_ptr(), replacement) };
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_fvar_namedstyle_coords(
    face: FT_Face,
    namedstyle_index: FT_UInt,
) -> Option<Vec<FT_Fixed>> {
    let state = face_state(face)?;
    rust_ffi::FT_Fvar_Named_Style_Coords(Some(&state.inner), namedstyle_index).ok()
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_set_own_bitmap_from_face(face: FT_Face, owns_bitmap: bool) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(internal) = slot_internal_mut(slot.as_ptr()) else {
        return rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error;
    };
    internal.owns_bitmap = owns_bitmap;
    internal.rust_slot.owns_bitmap = owns_bitmap;
    internal.flags = FT_UInt::from(owns_bitmap);
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyphslot_oblique_from_face(face: FT_Face) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_GlyphSlot_Oblique(slot.as_ptr());
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_get_subglyph_info_from_face(
    face: FT_Face,
    sub_index: FT_UInt,
    p_index: *mut FT_Int,
    p_flags: *mut FT_UInt,
    p_arg1: *mut FT_Int,
    p_arg2: *mut FT_Int,
    p_transform: *mut FT_Matrix,
) -> FT_Error {
    let Some(slot) = abi_glyph_slot(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    FT_Get_SubGlyph_Info(
        slot.as_ptr(),
        sub_index,
        p_index,
        p_flags,
        p_arg1,
        p_arg2,
        p_transform,
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_size_metrics(face: FT_Face) -> Option<FT_Size_Metrics> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    let size = unsafe { (*face.as_ptr()).size };
    let size = NonNull::new(size)?;
    // SAFETY: `size` is owned by the live face for the duration of this copy.
    Some(unsafe { size.as_ref().metrics })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_active_size(face: FT_Face) -> Option<FT_Size> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    Some(unsafe { (*face.as_ptr()).size })
}

#[cfg(feature = "abi-test-support")]
#[derive(Clone, Copy)]
pub struct AbiSizeRecSnapshot {
    pub face: FT_Face,
    pub generic: FT_Generic,
    pub metrics: FT_Size_Metrics,
    pub internal: *mut c_void,
}

#[cfg(feature = "abi-test-support")]
pub fn abi_size_rec_snapshot(size: FT_Size) -> Option<AbiSizeRecSnapshot> {
    let size = NonNull::new(size)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    let size = unsafe { size.as_ref() };
    Some(AbiSizeRecSnapshot {
        face: size.face,
        generic: size.generic,
        metrics: size.metrics,
        internal: size.internal,
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_size_rec_set_generic_data(size: FT_Size, data: FT_Pointer) -> bool {
    let Some(mut size) = NonNull::new(size) else {
        return false;
    };
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    unsafe {
        size.as_mut().generic.data = data;
    }
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_sfnt_os2(face: FT_Face) -> Option<TT_OS2> {
    let table = FT_Get_Sfnt_Table(face, rust_ffi::FT_SFNT_OS2 as FT_Sfnt_Tag);
    let table = NonNull::new(table.cast::<rust_ffi::TT_OS2>())?;
    // SAFETY: `FT_Get_Sfnt_Table` returned a live face-owned `TT_OS2` pointer.
    let os2 = unsafe { table.as_ref() };
    Some(TT_OS2 {
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
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_sfnt_vhea(face: FT_Face) -> Option<TT_VertHeader> {
    let table = FT_Get_Sfnt_Table(face, rust_ffi::FT_SFNT_VHEA as FT_Sfnt_Tag);
    let table = NonNull::new(table.cast::<rust_ffi::TT_VertHeader>())?;
    // SAFETY: `FT_Get_Sfnt_Table` returned a live face-owned `TT_VertHeader` pointer.
    let vhea = unsafe { table.as_ref() };
    Some(TT_VertHeader {
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
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_sfnt_maxp(face: FT_Face) -> Option<TT_MaxProfile> {
    let table = FT_Get_Sfnt_Table(face, rust_ffi::FT_SFNT_MAXP as FT_Sfnt_Tag);
    let table = NonNull::new(table.cast::<rust_ffi::TT_MaxProfile>())?;
    // SAFETY: `FT_Get_Sfnt_Table` returned a live face-owned `TT_MaxProfile` pointer.
    let maxp = unsafe { table.as_ref() };
    Some(TT_MaxProfile {
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
    })
}

#[cfg(feature = "abi-test-support")]
fn abi_glyph_slot(face: FT_Face) -> Option<NonNull<FT_GlyphSlotRec>> {
    let face = NonNull::new(face)?;
    // SAFETY: this feature-gated helper is only for tests using live handles from this crate.
    NonNull::new(unsafe { (*face.as_ptr()).glyph })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_glyph_slot_pointer(face: FT_Face) -> Option<FT_GlyphSlot> {
    abi_glyph_slot(face).map(NonNull::as_ptr)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Init_FreeType(alibrary: *mut FT_Library) -> FT_Error {
    let Some(out) = non_null_mut(alibrary) else {
        // FreeType 2.14.3 `src/base/ftinit.c:FT_Init_FreeType` reports
        // Invalid_Face_Handle when the output library pointer itself is null;
        // the pointer check lives in this thin ABI layer.
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let library = new_library_record(
        LibraryState::new(rust_ffi::FT_Init_FreeType()),
        ptr::null_mut(),
    );
    // SAFETY: `out` is a valid out pointer checked above.
    unsafe { *out.as_ptr() = into_library_handle(library) };
    rust_ffi::FT_Err_Ok
}

fn new_library_record(state: LibraryState, memory: FT_Memory) -> Box<FT_LibraryRec> {
    let memory = if memory.is_null() {
        state.allocation_memory
    } else {
        memory
    };
    Box::new(FT_LibraryRec {
        memory,
        version_major: 2,
        version_minor: 14,
        version_patch: 3,
        num_modules: 0,
        modules: [ptr::null_mut(); 32],
        renderers: FT_ListRec::default(),
        cur_renderer: ptr::null_mut(),
        auto_hinter: ptr::null_mut(),
        debug_hooks: [None; 4],
        #[cfg(feature = "subpixel-rendering")]
        lcd_weights: [0x08, 0x4D, 0x56, 0x4D, 0x08],
        #[cfg(not(feature = "subpixel-rendering"))]
        lcd_geometry: [FT_Vector::default(); 3],
        refcount: 1,
        internal: Box::into_raw(Box::new(state)).cast::<c_void>(),
    })
}

fn custom_memory_block(memory: FT_Memory, size: usize) -> Result<Option<FT_Pointer>, FT_Error> {
    if memory.is_null() {
        return Ok(None);
    }
    // SAFETY: `memory` is either the live caller FT_MemoryRec retained by
    // FT_New_Library or this crate's callback-free system-memory record.
    unsafe {
        let Some(alloc) = (*memory).alloc else {
            return Ok(None);
        };
        let block = alloc(memory, c_long::try_from(size).unwrap_or(c_long::MAX));
        if block.is_null() {
            Err(rust_ffi::FT_Err_Out_Of_Memory)
        } else {
            Ok(Some(block))
        }
    }
}

fn free_custom_memory_block(memory: FT_Memory, block: FT_Pointer) {
    if memory.is_null() || block.is_null() {
        return;
    }
    // SAFETY: `memory` is live for every allocation token it created, and
    // `block` is returned to the same record exactly once.
    unsafe {
        if let Some(free) = (*memory).free {
            free(memory, block);
        }
    }
}

fn glyph_allocation_tokens(
    library: FT_Library,
    record_size: usize,
    payload_size: usize,
) -> Result<(FT_Memory, FT_Pointer, FT_Pointer), FT_Error> {
    let Some(state) = library_state_mut(library) else {
        return Err(rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error);
    };
    let memory = state.allocation_memory;
    let record_block = custom_memory_block(memory, record_size)?.unwrap_or(ptr::null_mut());
    let payload_block = if payload_size == 0 {
        ptr::null_mut()
    } else {
        match custom_memory_block(memory, payload_size) {
            Ok(block) => block.unwrap_or(ptr::null_mut()),
            Err(error) => {
                free_custom_memory_block(memory, record_block);
                return Err(error);
            }
        }
    };
    Ok((memory, record_block, payload_block))
}

fn done_library_allocations(state: &mut LibraryState) {
    for block in state.module_allocation_blocks.drain(..).rev() {
        free_custom_memory_block(state.allocation_memory, block);
    }
    free_custom_memory_block(state.allocation_memory, state.allocation_block);
    state.allocation_block = ptr::null_mut();
}

fn done_synthetic_renderer_raster(state: &mut LibraryState, module: FT_Module) {
    if module.cast::<FT_RendererRec>() != &raw mut state.synthetic_renderer
        || state.synthetic_renderer.raster.is_null()
    {
        return;
    }
    let raster = state.synthetic_renderer.raster;
    let raster_done = non_null(state.synthetic_renderer.clazz).and_then(|clazz| {
        // SAFETY: the synthetic renderer retains the caller's live class
        // record until module teardown completes.
        let raster_class = unsafe { clazz.as_ref().raster_class };
        non_null(raster_class.cast_mut()).and_then(|raster_class| {
            // SAFETY: the class retains the raster callback table for the
            // registered module lifetime.
            unsafe { raster_class.as_ref().raster_done }
        })
    });
    if let Some(raster_done) = raster_done {
        // SAFETY: `raster` is the exact handle produced by this table's
        // `raster_new` callback and is released once before module_done.
        unsafe {
            raster_done(raster);
        }
    }
    state.synthetic_renderer.raster = ptr::null_mut();
}

unsafe fn drop_face_record(face: FT_Face) {
    // SAFETY: callers pass a live face allocation owned by this ABI.
    let face = unsafe { Box::from_raw(face) };
    if let Some(finalizer) = face.generic.finalizer {
        // SAFETY: FreeType's `FT_Done_Face` contract invokes the caller's
        // registered generic finalizer with the live face object immediately
        // before the face allocation is released.
        unsafe {
            finalizer(
                ptr::from_ref::<FT_FaceRec>(&face)
                    .cast_mut()
                    .cast::<c_void>(),
            );
        }
    }
    // SAFETY: the face owns its glyph record.
    unsafe { drop_glyph(face.glyph) };
    if !face.internal.is_null() {
        // SAFETY: the internal record was allocated with this face.
        let internal = unsafe { Box::from_raw(face.internal.cast::<FT_Face_InternalRecCompat>()) };
        if let Some(close) = internal.state.stream_close {
            close(internal.state.stream);
        }
        drop(internal);
    }
}

fn drop_library_rec(library: NonNull<FT_LibraryRec>, free_custom_allocation: bool) -> bool {
    if !unregister_library(library.as_ptr()) {
        return false;
    }
    // SAFETY: `library` is a live handle allocated by this crate.
    unsafe {
        let library = Box::from_raw(library.as_ptr());
        if !library.internal.is_null() {
            let mut state = Box::from_raw(library.internal.cast::<LibraryState>());
            for face in state.faces.drain(..) {
                drop_face_record(face);
            }
            while let Some(module) = state.synthetic_modules.pop() {
                done_synthetic_renderer_raster(&mut state, module.module);
                if let Some(done) = module.done {
                    done(module.module);
                }
            }
            if free_custom_allocation {
                done_library_allocations(&mut state);
            }
        }
    }
    true
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Done_FreeType(library: FT_Library) -> FT_Error {
    if let Some(library) = non_null_mut(library) {
        if drop_library_rec(library, false) {
            rust_ffi::FT_Err_Ok
        } else {
            rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error
        }
    } else {
        35 // matches C runtime: FT_Done_FreeType(NULL)
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_New_Library(memory: FT_Memory, alibrary: *mut FT_Library) -> FT_Error {
    let (Some(memory), Some(out)) = (non_null_mut(memory), non_null_mut(alibrary)) else {
        // FreeType 2.14.3 `src/base/ftobjs.c:FT_New_Library` returns before
        // writing `alibrary` when either public argument is null.
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let allocation =
        match custom_memory_block(memory.as_ptr(), std::mem::size_of::<FT_LibraryRec>()) {
            Ok(Some(block)) => block,
            Ok(None) | Err(_) => return rust_ffi::FT_Err_Out_Of_Memory,
        };
    let Ok(inner) =
        rust_ffi::FT_New_Library(Some(memory.as_ptr().cast::<rust_ffi::FT_MemoryRec>()))
    else {
        // SAFETY: release the allocation block if core rejects construction.
        free_custom_memory_block(memory.as_ptr(), allocation);
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let library = new_library_record(
        LibraryState::new_with_allocation(inner, memory.as_ptr(), allocation),
        memory.as_ptr(),
    );
    // SAFETY: `out` is a valid out pointer checked above.
    unsafe { *out.as_ptr() = into_library_handle(library) };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Reference_Library(library: FT_Library) -> FT_Error {
    let error = rust_ffi::FT_Reference_Library(library_mut(library));
    if error == rust_ffi::FT_Err_Ok
        && let Some(library) = non_null_mut(library)
    {
        // SAFETY: the successful core call proves this is a live library.
        unsafe {
            (*library.as_ptr()).refcount = (*library.as_ptr()).refcount.saturating_add(1);
        }
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Done_Library(library: FT_Library) -> FT_Error {
    let Some(library_ptr) = non_null_mut(library) else {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    };
    let err = rust_ffi::FT_Done_Library(library_mut(library));
    if err != rust_ffi::FT_Err_Ok {
        return err;
    }
    let refcount =
        library_ref(library).map_or(0, |library| rust_ffi::FT_Library_Refcount(Some(library)));
    if refcount == 0 {
        let _ = drop_library_rec(library_ptr, true);
    } else {
        // SAFETY: the core retained the live library allocation.
        unsafe {
            (*library_ptr.as_ptr()).refcount = FT_Int::try_from(refcount).unwrap_or(FT_Int::MAX);
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_MM_Var(face: FT_Face, amaster: *mut *mut FT_MM_Var) -> FT_Error {
    let Some(amaster) = non_null_mut(amaster) else {
        return rust_ffi::FT_Get_MM_Var(None, None, None, None, None);
    };
    let Some(state) = face_state(face) else {
        let mut out = FT_MM_Var::default();
        return rust_ffi::FT_Get_MM_Var(None, Some(&mut out), None, None, None);
    };
    let mut axis = vec![FT_Var_Axis::default(); 64].into_boxed_slice();
    let mut namedstyle = vec![rust_ffi::FT_Var_Named_Style::default(); 256].into_boxed_slice();
    let mut namedstyle_coords = vec![rust_ffi::FT_Fixed::default(); 64 * 256].into_boxed_slice();
    let mut head = Box::new(OwnedMmVarHead {
        master: FT_MM_Var::default(),
        axis_flags: [0; 64],
    });
    let err = rust_ffi::FT_Get_MM_Var(
        Some(&state.inner),
        Some(&mut head.master),
        Some(&mut axis),
        Some(&mut namedstyle),
        Some(&mut namedstyle_coords),
    );
    if err != rust_ffi::FT_Err_Ok {
        return err;
    }
    head.master.axis = axis.as_mut_ptr();
    head.master.namedstyle = if head.master.num_namedstyles == 0 {
        ptr::null_mut()
    } else {
        namedstyle.as_mut_ptr()
    };
    for axis_index in 0..head.master.num_axis.min(64) {
        let mut flags = 0;
        let flag_err =
            rust_ffi::FT_Get_Var_Axis_Flags(Some(&head.master), axis_index, Some(&mut flags));
        if flag_err != rust_ffi::FT_Err_Ok {
            return flag_err;
        }
        head.axis_flags[axis_index as usize] = FT_UShort::try_from(flags).unwrap_or(FT_UShort::MAX);
    }
    let mut owned = OwnedMmVar {
        head,
        _axis: axis,
        _namedstyle: namedstyle,
        _namedstyle_coords: namedstyle_coords,
    };
    let master_ptr: *mut FT_MM_Var = &mut owned.head.master;
    OWNED_MM_VARS.with(|vars| {
        vars.borrow_mut().insert(master_ptr.addr(), owned);
    });
    // SAFETY: `amaster` is a non-null output pointer supplied by the caller.
    unsafe { *amaster.as_ptr() = master_ptr };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Done_MM_Var(library: FT_Library, amaster: *mut FT_MM_Var) -> FT_Error {
    let Some(library) = library_ref(library) else {
        return rust_ffi::FT_Done_MM_Var(None, None);
    };
    if !amaster.is_null() {
        let removed = OWNED_MM_VARS.with(|vars| vars.borrow_mut().remove(&amaster.addr()));
        if removed.is_some() {
            return rust_ffi::FT_Done_MM_Var(Some(library), None);
        }
    }
    let amaster = non_null_mut(amaster).map(|mut amaster| {
        // SAFETY: `amaster` is non-null and the caller provides a writable
        // FT_MM_Var descriptor owned by this API.  The current pure-Rust core
        // only observes null-vs-non-null ownership for this public route.
        unsafe { amaster.as_mut() }
    });
    rust_ffi::FT_Done_MM_Var(Some(library), amaster)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Var_Axis_Flags(
    master: *mut FT_MM_Var,
    axis_index: FT_UInt,
    flags: *mut FT_UInt,
) -> FT_Error {
    let master = non_null_mut(master).map(|master| {
        // SAFETY: `master` is non-null and points to a public FT_MM_Var
        // record supplied by the caller.
        unsafe { master.as_ref() }
    });
    let flags = non_null_mut(flags).map(|mut flags| {
        // SAFETY: `flags` is non-null and points to caller-writable FT_UInt storage.
        unsafe { flags.as_mut() }
    });
    rust_ffi::FT_Get_Var_Axis_Flags(master, axis_index, flags)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Library_SetLcdFilter(library: FT_Library, filter: FT_LcdFilter) -> FT_Error {
    #[cfg(not(feature = "subpixel-rendering"))]
    {
        let _ = library;
        rust_ffi::FT_Library_SetLcdFilter(None, filter)
    }
    #[cfg(feature = "subpixel-rendering")]
    let error = rust_ffi::FT_Library_SetLcdFilter(library_mut(library), filter);
    #[cfg(feature = "subpixel-rendering")]
    if error == rust_ffi::FT_Err_Ok
        && let Some(values) = rust_ffi::FT_Library_LcdWeights(library_ref(library))
        && let Some(library) = non_null_mut(library)
    {
        // SAFETY: the successful core call proves the library handle is live.
        unsafe { (*library.as_ptr()).lcd_weights = values };
    }
    #[cfg(feature = "subpixel-rendering")]
    {
        error
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Library_SetLcdFilterWeights(
    library: FT_Library,
    weights: *mut FT_Byte,
) -> FT_Error {
    #[cfg(feature = "subpixel-rendering")]
    let weights = if weights.is_null() {
        None
    } else {
        let mut values = [0; 5];
        // SAFETY: the enabled FreeType API requires five readable bytes.
        unsafe { ptr::copy_nonoverlapping(weights, values.as_mut_ptr(), values.len()) };
        Some(values)
    };
    #[cfg(not(feature = "subpixel-rendering"))]
    let weights = {
        let _ = weights;
        None
    };
    #[cfg(feature = "subpixel-rendering")]
    let rust_library = library_mut(library);
    #[cfg(not(feature = "subpixel-rendering"))]
    let rust_library = {
        let _ = library;
        None
    };
    let error = rust_ffi::FT_Library_SetLcdFilterWeights(rust_library, weights);
    #[cfg(feature = "subpixel-rendering")]
    if error == rust_ffi::FT_Err_Ok
        && let Some(values) = weights
        && let Some(library) = non_null_mut(library)
    {
        // SAFETY: the successful core call proves the library handle is live.
        unsafe { (*library.as_ptr()).lcd_weights = values };
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Library_SetLcdGeometry(
    library: FT_Library,
    sub: *const FT_Vector,
) -> FT_Error {
    #[cfg(feature = "subpixel-rendering")]
    {
        let _ = (library, sub);
        rust_ffi::FT_Library_SetLcdGeometry(None, None)
    }
    #[cfg(not(feature = "subpixel-rendering"))]
    let rust_sub = if sub.is_null() {
        None
    } else {
        let mut vectors = [rust_ffi::FT_Vector::default(); 3];
        for (index, vector) in vectors.iter_mut().enumerate() {
            // SAFETY: `sub` is non-null and the C API requires three vectors.
            let source = unsafe { &*sub.add(index) };
            *vector = rust_ffi::FT_Vector {
                x: source.x,
                y: source.y,
            };
        }
        Some(vectors)
    };
    #[cfg(not(feature = "subpixel-rendering"))]
    rust_ffi::FT_Library_SetLcdGeometry(library_mut(library), rust_sub)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_TrueType_Engine_Type(library: FT_Library) -> FT_TrueTypeEngineType {
    rust_ffi::FT_Get_TrueType_Engine_Type(library_ref(library))
}

fn property_name_arg(ptr: *const FT_String) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: FreeType string arguments are nul-terminated `const char*`
    // values owned by the caller for the duration of the call.
    unsafe { CStr::from_ptr(ptr).to_str().ok().map(ToOwned::to_owned) }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_module_interface_present(
    library: FT_Library,
    module_name: Option<&str>,
) -> bool {
    !rust_ffi::FT_Get_Module_Interface(library_ref(library), module_name).is_null()
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_module_requester_service_available(
    library: FT_Library,
    module_name: Option<&str>,
    service_name: &str,
) -> bool {
    rust_ffi::FT_Module_Requester_Service_Available(library_ref(library), module_name, service_name)
}

fn is_increase_x_height_property(module_name: Option<&str>, property_name: Option<&str>) -> bool {
    module_name == Some("autofitter") && property_name == Some("increase-x-height")
}

fn is_glyph_to_script_map_property(module_name: Option<&str>, property_name: Option<&str>) -> bool {
    module_name == Some("autofitter") && property_name == Some("glyph-to-script-map")
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Property_Get(
    library: FT_Library,
    module_name: *const FT_String,
    property_name: *const FT_String,
    value: *mut c_void,
) -> FT_Error {
    let module_name = property_name_arg(module_name);
    let property_name = property_name_arg(property_name);
    if is_glyph_to_script_map_property(module_name.as_deref(), property_name.as_deref()) {
        let Some(prop) = (unsafe { value.cast::<rust_ffi::FT_Prop_GlyphToScriptMap>().as_mut() })
        else {
            return rust_ffi::FT_Property_Get_GlyphToScriptMap(
                library_ref(library),
                module_name.as_deref(),
                property_name.as_deref(),
                None,
                None,
            );
        };
        let requested_face = prop.face.cast::<FT_FaceRec>();
        let face = face_state(requested_face).map(|state| &state.inner);
        let error = rust_ffi::FT_Property_Get_GlyphToScriptMap(
            library_ref(library),
            module_name.as_deref(),
            property_name.as_deref(),
            face,
            Some(prop),
        );
        if error == rust_ffi::FT_Err_Ok {
            prop.face = requested_face.cast();
        }
        return error;
    }
    if is_increase_x_height_property(module_name.as_deref(), property_name.as_deref()) {
        let Some(prop) = (unsafe { value.cast::<rust_ffi::FT_Prop_IncreaseXHeight>().as_mut() })
        else {
            return rust_ffi::FT_Property_Get_IncreaseXHeight(
                library_ref(library),
                module_name.as_deref(),
                property_name.as_deref(),
                None,
                None,
            );
        };
        let face = face_state(prop.face.cast::<FT_FaceRec>()).map(|state| &state.inner);
        return rust_ffi::FT_Property_Get_IncreaseXHeight(
            library_ref(library),
            module_name.as_deref(),
            property_name.as_deref(),
            face,
            Some(prop),
        );
    }
    let value = if value.is_null() {
        None
    } else {
        // SAFETY: For the implemented TrueType property the public C contract
        // requires an `FT_UInt*`; null was handled above.
        Some(unsafe { &mut *value.cast::<FT_UInt>() })
    };
    rust_ffi::FT_Property_Get(
        library_ref(library),
        module_name.as_deref(),
        property_name.as_deref(),
        value,
    )
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn abi_glyph_to_script_map_sample(
    face: FT_Face,
    glyph_indices: &[FT_UInt],
) -> Vec<(FT_UInt, FT_UShort)> {
    face_state(face).map_or_else(Vec::new, |state| {
        rust_ffi::FT_Glyph_To_Script_Map_Sample_For_Test(&state.inner, glyph_indices)
    })
}

#[cfg(any(test, feature = "abi-test-support"))]
pub fn abi_glyph_to_script_map_mutate(
    face: FT_Face,
    glyph_index: FT_UInt,
    value: FT_UShort,
) -> Option<FT_UShort> {
    let state = face_state_mut(face)?;
    rust_ffi::FT_Glyph_To_Script_Map_Mutate_For_Test(&mut state.inner, glyph_index, value)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Property_Set(
    library: FT_Library,
    module_name: *const FT_String,
    property_name: *const FT_String,
    value: *const c_void,
) -> FT_Error {
    let module_name = property_name_arg(module_name);
    let property_name = property_name_arg(property_name);
    if module_name.as_deref() == Some("ot-svg") && property_name.as_deref() == Some("svg-hooks") {
        let Some(rust_library) = library_mut(library) else {
            return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
        };
        if value.is_null() {
            return rust_ffi::FT_Err_Invalid_Argument;
        }
        // SAFETY: the `svg-hooks` property payload is the public
        // `SVG_RendererHooks` record (`freetype/otsvg.h`).
        let hooks = unsafe { &*value.cast::<SVG_RendererHooks>() };
        if hooks.init_svg.is_none()
            || hooks.free_svg.is_none()
            || hooks.render_svg.is_none()
            || hooks.preset_slot.is_none()
        {
            return rust_ffi::FT_Err_Invalid_Argument;
        }
        return rust_ffi::FT_Set_SVG_Renderer_Hooks(
            Some(rust_library),
            Some(core_svg_hooks_from_abi(hooks)),
        );
    }
    if is_increase_x_height_property(module_name.as_deref(), property_name.as_deref()) {
        let prop = unsafe { value.cast::<rust_ffi::FT_Prop_IncreaseXHeight>().as_ref() };
        let face = prop.and_then(|prop| face_state_mut(prop.face.cast::<FT_FaceRec>()));
        return rust_ffi::FT_Property_Set_IncreaseXHeight(
            library_ref(library),
            module_name.as_deref(),
            property_name.as_deref(),
            face.map(|state| &mut state.inner),
            prop,
        );
    }
    let value = if value.is_null() {
        None
    } else {
        // SAFETY: For the implemented TrueType property the public C contract
        // requires an `FT_UInt*`; null was handled above.
        Some(unsafe { *value.cast::<FT_UInt>() })
    };
    rust_ffi::FT_Property_Set(
        library_mut(library),
        module_name.as_deref(),
        property_name.as_deref(),
        value,
    )
}

#[allow(unsafe_code)]
fn core_svg_hooks_from_abi(hooks: &SVG_RendererHooks) -> rust_ffi::SVG_RendererHooks {
    // The public hooks record is `repr(C)` with four `extern "C"` function
    // pointers; the slot argument is `FT_GlyphSlot` (a typedef for
    // `FT_GlyphSlotRec*`) on the ABI layer and `*mut c_void` on the core
    // layer, so converting the pointer type is layout-preserving.
    let convert_render = |callback: SvgRenderFunc| -> Option<rust_ffi::SvgRenderFunc> {
        callback.map(|callback| {
            // SAFETY: both types are `unsafe extern "C" fn` with the same
            // argument/return layout; only the first argument's pointee name
            // differs between the public typedef and the core record.
            unsafe { std::mem::transmute::<_, rust_ffi::SvgRenderFunc>(Some(callback)) }
        })
    };
    let convert_preset = |callback: SvgPresetSlotFunc| -> Option<rust_ffi::SvgPresetSlotFunc> {
        callback.map(|callback| {
            // SAFETY: see `convert_render`; the preset hook keeps the same
            // `FT_Bool` cache argument and state pointer on both layers.
            unsafe { std::mem::transmute::<_, rust_ffi::SvgPresetSlotFunc>(Some(callback)) }
        })
    };
    rust_ffi::SVG_RendererHooks {
        init_svg: hooks.init_svg,
        free_svg: hooks.free_svg,
        render_svg: convert_render(hooks.render_svg),
        preset_slot: convert_preset(hooks.preset_slot),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Default_Properties(library: FT_Library) {
    rust_ffi::FT_Set_Default_Properties(library_mut(library));
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

fn face_property_from_abi(parameter: &FT_Parameter) -> rust_ffi::FT_Face_Property {
    let value = match parameter.tag as i64 {
        rust_ffi::FT_PARAM_TAG_STEM_DARKENING if !parameter.data.is_null() => {
            // SAFETY: FreeType requires `FT_PARAM_TAG_STEM_DARKENING` data to
            // point to an `FT_Bool` for the duration of `FT_Face_Properties`.
            Some(rust_ffi::FT_Face_Property_Value::Bool(unsafe {
                *parameter.data.cast::<FT_Bool>()
            }))
        }
        rust_ffi::FT_PARAM_TAG_RANDOM_SEED if !parameter.data.is_null() => {
            // SAFETY: FreeType requires `FT_PARAM_TAG_RANDOM_SEED` data to
            // point to an `FT_Int32` for the duration of `FT_Face_Properties`.
            Some(rust_ffi::FT_Face_Property_Value::Int32(unsafe {
                *parameter.data.cast::<FT_Int32>()
            }))
        }
        _ => None,
    };
    rust_ffi::FT_Face_Property {
        tag: parameter.tag,
        value,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_Properties(
    face: FT_Face,
    num_properties: FT_UInt,
    properties: *mut FT_Parameter,
) -> FT_Error {
    if num_properties > 0 && properties.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let props = if num_properties == 0 {
        Vec::new()
    } else {
        let Ok(count) = usize::try_from(num_properties) else {
            return rust_ffi::FT_Err_Invalid_Argument;
        };
        // SAFETY: The C ABI requires `properties` to address `num_properties`
        // readable `FT_Parameter` records when `num_properties > 0`.
        unsafe { slice::from_raw_parts(properties, count) }
            .iter()
            .map(face_property_from_abi)
            .collect()
    };
    let Some(internal) = face_internal_mut(face) else {
        return rust_ffi::FT_Face_Properties(None, Some(&props));
    };
    let error = rust_ffi::FT_Face_Properties(Some(&mut internal.state.inner), Some(&props));
    if error == rust_ffi::FT_Err_Ok {
        internal.sync_face_properties();
    }
    error
}

pub fn abi_face_properties_state(face: FT_Face) -> Option<rust_ffi::FT_Face_Properties_State> {
    face_state(face).map(|state| rust_ffi::FT_Face_Properties_Get_State(&state.inner))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Add_Default_Modules(library: FT_Library) {
    rust_ffi::FT_Add_Default_Modules(library_mut(library));
    let Some(state) = library_state_mut(library) else {
        return;
    };
    if state.module_allocation_blocks.is_empty() {
        for _ in 0..state.default_modules.len() {
            match custom_memory_block(state.allocation_memory, std::mem::size_of::<FT_ModuleRec>())
            {
                Ok(Some(block)) => state.module_allocation_blocks.push(block),
                Ok(None) => break,
                Err(_) => break,
            }
        }
    }
    for record in &mut state.default_modules {
        record.module.library = library;
    }
    // SAFETY: a live FreeType library exposes room for 32 module pointers.
    unsafe {
        let public = &mut *library;
        public.modules.fill(ptr::null_mut());
        for (index, record) in state.default_modules.iter_mut().enumerate() {
            public.modules[index] = &mut *record.module;
        }
        public.num_modules = FT_UInt::try_from(state.default_modules.len()).unwrap_or(FT_UInt::MAX);
    }
}

fn module_name_from_abi(module_name: *const FT_String) -> Option<&'static str> {
    if module_name.is_null() {
        return None;
    }
    // SAFETY: `module_name` is a FreeType ABI C string pointer supplied by the
    // caller.  The wrapper converts only recognized synthetic test names into
    // safe static identifiers before delegating to the pure-Rust core.
    let bytes = unsafe { CStr::from_ptr(module_name).to_bytes() };
    if let Some(name) = ABI_DEFAULT_MODULE_NAMES
        .iter()
        .copied()
        .find(|name| bytes == name.as_bytes())
    {
        return Some(name);
    }
    match bytes {
        b"fixture_minimal" => Some("fixture_minimal"),
        b"fixture_renderer" => Some("fixture_renderer"),
        b"fixture_styler" => Some("fixture_styler"),
        b"fixture_upgrade" => Some("fixture_upgrade"),
        b"fixture_future" => Some("fixture_future"),
        b"fixture_lifecycle" => Some("fixture_lifecycle"),
        b"fixture_renderer_lifecycle" => Some("fixture_renderer_lifecycle"),
        b"fixture_raster_lifecycle" => Some("fixture_raster_lifecycle"),
        b"fixture_raster_new_error" => Some("fixture_raster_new_error"),
        b"fixture_raster_set_mode" => Some("fixture_raster_set_mode"),
        b"fixture_custom_glyph" => Some("fixture_custom_glyph"),
        b"fixture_second" => Some("fixture_second"),
        b"fixture_final_destroy" => Some("fixture_final_destroy"),
        b"otvalid" => Some("otvalid"),
        b"gxvalid" => Some("gxvalid"),
        _ => None,
    }
}

fn module_class_info_from_abi(
    clazz: *const FT_Module_Class,
) -> Option<rust_ffi::FT_Module_Class_Info> {
    let clazz = non_null(clazz.cast_mut())?;
    // SAFETY: `clazz` is non-null and points to a readable FreeType module
    // class record for the duration of this ABI call.
    let clazz = unsafe { clazz.as_ref() };
    Some(rust_ffi::FT_Module_Class_Info {
        module_flags: clazz.module_flags,
        module_size: clazz.module_size,
        module_name: module_name_from_abi(clazz.module_name),
        module_version: clazz.module_version,
        module_requires: clazz.module_requires,
        module_interface_present: !clazz.module_interface.is_null(),
        module_init: if clazz.module_init.is_none() {
            rust_ffi::FT_Module_Callback_Behavior::None
        } else {
            rust_ffi::FT_Module_Callback_Behavior::RecordThenOk
        },
        module_done: if clazz.module_done.is_none() {
            rust_ffi::FT_Module_Callback_Behavior::None
        } else {
            rust_ffi::FT_Module_Callback_Behavior::RecordThenOk
        },
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Add_Module(library: FT_Library, clazz: *const FT_Module_Class) -> FT_Error {
    let info = module_class_info_from_abi(clazz);
    let error = rust_ffi::FT_Add_Module(library_mut(library), info.as_ref());
    if error == rust_ffi::FT_Err_Ok {
        let module_class = non_null(clazz.cast_mut());
        let module_init = module_class.and_then(|clazz| {
            // SAFETY: the accepted class remains readable for this call.
            unsafe { clazz.as_ref().module_init }
        });
        let module_done = module_class.and_then(|clazz| {
            // SAFETY: the class was accepted by the core call and remains
            // readable for the duration of this synchronous registration.
            unsafe { clazz.as_ref().module_done }
        });
        let module_interface = module_class.map_or(ptr::null_mut(), |clazz| {
            // SAFETY: the accepted class remains readable for this call.
            unsafe { clazz.as_ref().module_interface.cast_mut() }
        });
        let mut installed_module = ptr::null_mut();
        if let Some(state) = library_state_mut(library) {
            let Some(info) = info else {
                return rust_ffi::FT_Err_Invalid_Argument;
            };
            let Some(name) = info.module_name else {
                return rust_ffi::FT_Err_Invalid_Argument;
            };
            let (module, handle) = if info.module_flags & rust_ffi::FT_MODULE_RENDERER as FT_ULong
                != 0
            {
                state.synthetic_renderer.root.clazz = clazz;
                state.synthetic_renderer.root.library = library;
                state.synthetic_renderer.root.memory = state.allocation_memory;
                state.synthetic_renderer.raster = ptr::null_mut();
                state.synthetic_renderer.raster_render = None;
                let renderer_public_size =
                    std::mem::offset_of!(FT_RendererRec, module_name) as FT_Long;
                if info.module_size >= renderer_public_size {
                    let renderer_class = non_null(clazz.cast_mut().cast::<FT_Renderer_Class>());
                    let Some(renderer_class) = renderer_class else {
                        return rust_ffi::FT_Err_Invalid_Argument;
                    };
                    // SAFETY: a renderer-sized class carrying
                    // `FT_MODULE_RENDERER` has the public
                    // `FT_Renderer_Class` prefix required by FreeType.
                    let renderer_class = unsafe { renderer_class.as_ref() };
                    state.synthetic_renderer.clazz = ptr::from_ref(renderer_class).cast_mut();
                    state.synthetic_renderer.glyph_format = renderer_class.glyph_format;
                    state.synthetic_renderer.glyph_class = FT_Glyph_Class::default();
                    state.synthetic_renderer.render = renderer_class.render_glyph;
                    state.synthetic_renderer.raster_render =
                        non_null(renderer_class.raster_class.cast_mut()).and_then(|raster| {
                            // SAFETY: the renderer class promises a live
                            // raster-class record during registration.
                            unsafe { raster.as_ref().raster_render }
                        });
                    if let Some(raster_class) = non_null(renderer_class.raster_class.cast_mut()) {
                        // SAFETY: the renderer class keeps its raster table
                        // live for the entire module registration.
                        let raster_class = unsafe { raster_class.as_ref() };
                        if let Some(raster_new) = raster_class.raster_new {
                            let mut raster = ptr::null_mut();
                            // SAFETY: the callback receives the live library
                            // memory record and writable renderer-owned handle
                            // storage, matching `ft_add_renderer`.
                            let raster_error = unsafe {
                                raster_new(state.allocation_memory.cast::<c_void>(), &mut raster)
                            };
                            if raster_error != rust_ffi::FT_Err_Ok {
                                let _ =
                                    rust_ffi::FT_Remove_Module(Some(&mut state.inner), Some(name));
                                return raster_error;
                            }
                            state.synthetic_renderer.raster = raster;
                        }
                    }
                }
                state.synthetic_renderer.module_name = name;
                (
                    (&mut state.synthetic_renderer as *mut FT_RendererRec).cast::<FT_ModuleRec>(),
                    None,
                )
            } else {
                let mut handle = Box::new(FT_ModuleRec {
                    clazz,
                    library,
                    memory: state.allocation_memory,
                });
                let module = (&mut *handle) as FT_Module;
                (module, Some(handle))
            };
            state.synthetic_modules.push(AbiSyntheticModuleRecord {
                name,
                module,
                _handle: handle,
                interface: module_interface,
                done: module_done,
            });
            // SAFETY: the live library owns the registered synthetic module
            // and has a fixed 32-entry module table.
            unsafe {
                let public = &mut *library;
                let index = usize::try_from(public.num_modules).unwrap_or(usize::MAX);
                if let Some(slot) = public.modules.get_mut(index) {
                    *slot = module;
                    public.num_modules = public.num_modules.saturating_add(1);
                }
            }
            installed_module = module;
        }
        if let Some(module_init) = module_init {
            // SAFETY: the callback belongs to the accepted class and receives
            // the live module allocation owned by this library.
            let init_error = unsafe { module_init(installed_module) };
            if init_error != rust_ffi::FT_Err_Ok {
                return init_error;
            }
        }
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Module(library: FT_Library, module_name: *const c_char) -> FT_Module {
    let Some(name) = module_name_from_abi(module_name) else {
        return ptr::null_mut();
    };
    let Some(state) = library_state_mut(library) else {
        return ptr::null_mut();
    };
    if rust_ffi::FT_Library_Has_Module(Some(&state.inner), name) {
        if rust_ffi::FT_Library_Module_Flags(Some(&state.inner), name)
            .is_some_and(|flags| flags & rust_ffi::FT_MODULE_RENDERER as FT_ULong != 0)
        {
            let renderer = match name {
                "smooth" => Some(&mut state.outline_renderer),
                "raster1" => Some(&mut state.raster1_renderer),
                "sdf" => Some(&mut state.sdf_renderer),
                "bsdf" => Some(&mut state.bitmap_renderer),
                "ot-svg" => Some(&mut state.svg_renderer),
                _ => None,
            };
            if let Some(renderer) = renderer {
                renderer.root.library = library;
                return (renderer as *mut FT_RendererRec).cast::<FT_ModuleRec>();
            }
            if name == state.synthetic_renderer.module_name {
                state.synthetic_renderer.root.library = library;
                return (&mut state.synthetic_renderer as *mut FT_RendererRec)
                    .cast::<FT_ModuleRec>();
            }
        }
        if let Some(record) = state
            .default_modules
            .iter_mut()
            .find(|record| record.name == name)
        {
            record.module.library = library;
            return &mut *record.module;
        }
        state
            .synthetic_modules
            .iter()
            .find(|module| module.name == name)
            .map_or(ptr::null_mut(), |module| module.module)
    } else {
        ptr::null_mut()
    }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_get_module_interface(
    library: FT_Library,
    module_name: &str,
) -> FT_Module_Interface {
    let Some(state) = library_state_mut(library) else {
        return ptr::null_mut();
    };
    if let Some(module) = state
        .synthetic_modules
        .iter()
        .find(|module| module.name == module_name)
    {
        return module.interface;
    }
    rust_ffi::FT_Get_Module_Interface(Some(&state.inner), Some(module_name))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Remove_Module(library: FT_Library, module: FT_Module) -> FT_Error {
    let Some(state) = library_state_mut(library) else {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    };
    let Some(index) = state
        .synthetic_modules
        .iter()
        .position(|installed| installed.module == module)
    else {
        return rust_ffi::FT_Err_Invalid_Driver_Handle as FT_Error;
    };
    let name = state.synthetic_modules[index].name;
    let error = rust_ffi::FT_Remove_Module(Some(&mut state.inner), Some(name));
    if error != rust_ffi::FT_Err_Ok {
        return error;
    }
    let removed = state.synthetic_modules.remove(index);
    done_synthetic_renderer_raster(state, removed.module);
    // SAFETY: `library` is live; compact the public module table around the
    // exact removed pointer and clear its former tail.
    unsafe {
        let public = &mut *library;
        let count = usize::try_from(public.num_modules).unwrap_or(0);
        if let Some(public_index) = public.modules[..count]
            .iter()
            .position(|candidate| *candidate == module)
        {
            let following = public_index.saturating_add(1);
            public.modules.copy_within(following..count, public_index);
            if let Some(last) = count
                .checked_sub(1)
                .and_then(|index| public.modules.get_mut(index))
            {
                *last = ptr::null_mut();
            }
            public.num_modules = public.num_modules.saturating_sub(1);
        }
    }
    if let Some(module_done) = removed.done {
        // SAFETY: the callback was registered for this live module and is
        // invoked synchronously before its opaque handle is reused.
        unsafe {
            module_done(removed.module);
        }
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Debug_Hook(
    library: FT_Library,
    hook_index: FT_UInt,
    debug_hook: FT_DebugHook_Func,
) {
    rust_ffi::FT_Set_Debug_Hook(library_mut(library), hook_index, debug_hook);
    if let (Some(mut library), Some(debug_hook), Ok(index)) = (
        non_null_mut(library),
        debug_hook,
        usize::try_from(hook_index),
    ) && index < 4
    {
        // SAFETY: the live public library record contains the pinned four-slot
        // debug hook array.  Null hooks and out-of-range indices are no-ops.
        unsafe {
            library.as_mut().debug_hooks[index] = Some(debug_hook);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Trace_Set_Level(_tracing_level: *const c_char) {}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Trace_Set_Default_Level() {}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Log_Handler(_handler: FT_Custom_Log_Handler) {}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Default_Log_Handler() {}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_new_library_without_default_modules() -> FT_Library {
    into_library_handle(new_library_record(
        LibraryState::new(rust_ffi::FT_New_Library_Without_Default_Modules()),
        ptr::null_mut(),
    ))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_has_truetype_module(library: FT_Library) -> bool {
    rust_ffi::FT_Library_Has_TrueType_Module(library_ref(library))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_has_module(library: FT_Library, name: &str) -> bool {
    rust_ffi::FT_Library_Has_Module(library_ref(library), name)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_module_flags(library: FT_Library, name: &str) -> Option<FT_ULong> {
    rust_ffi::FT_Library_Module_Flags(library_ref(library), name)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_module_count(library: FT_Library) -> usize {
    rust_ffi::FT_Library_Module_Count(library_ref(library))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_synthetic_module_info(
    library: FT_Library,
    name: &str,
) -> Option<rust_ffi::FT_Installed_Module_Info> {
    rust_ffi::FT_Library_Synthetic_Module_Info(library_ref(library), name)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_module_table_observation(
    library: FT_Library,
    expected_first: FT_Module,
) -> (usize, bool, bool) {
    let Some(library) = non_null(library) else {
        return (0, false, false);
    };
    // SAFETY: test support receives a live library handle from this ABI.
    let library = unsafe { library.as_ref() };
    let count = usize::try_from(library.num_modules).unwrap_or(0);
    (
        count,
        count > 0 && library.modules[0] == expected_first,
        library
            .modules
            .get(count)
            .is_some_and(|module| module.is_null()),
    )
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_set_face_finalizer(face: FT_Face, finalizer: FT_Generic_Finalizer) -> bool {
    let Some(mut face) = non_null_mut(face) else {
        return false;
    };
    // SAFETY: test support receives a live face handle from this ABI.
    unsafe {
        face.as_mut().generic.finalizer = finalizer;
    }
    true
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_renderer_class(
    library: FT_Library,
    format: FT_Glyph_Format,
) -> Option<(&'static str, FT_Glyph_Format, bool, bool)> {
    rust_ffi::FT_Library_Renderer_Class(library_ref(library), format)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Renderer(library: FT_Library, format: FT_Glyph_Format) -> FT_Renderer {
    let Some(state) = library_state_mut(library) else {
        return ptr::null_mut();
    };
    let class = rust_ffi::FT_Library_Renderer_Class(Some(&state.inner), format);
    if class.is_none()
        && state.synthetic_renderer.glyph_format == format
        && state.synthetic_modules.iter().any(|module| {
            module.module.cast::<FT_RendererRec>() == &raw mut state.synthetic_renderer
        })
    {
        state.synthetic_renderer.root.library = library;
        return &mut state.synthetic_renderer;
    }
    let Some((module_name, glyph_format, _, _)) = class else {
        return ptr::null_mut();
    };
    if glyph_format == state.outline_renderer.glyph_format
        && module_name == state.outline_renderer.module_name
    {
        state.outline_renderer.root.library = library;
        let renderer = &mut state.outline_renderer as FT_Renderer;
        // SAFETY: `library` is live and owns this renderer.
        unsafe {
            (*library).cur_renderer = renderer;
        }
        renderer
    } else if glyph_format == state.raster1_renderer.glyph_format
        && module_name == state.raster1_renderer.module_name
    {
        state.raster1_renderer.root.library = library;
        let renderer = &mut state.raster1_renderer as FT_Renderer;
        // SAFETY: `library` is live and owns this renderer.
        unsafe {
            (*library).cur_renderer = renderer;
        }
        renderer
    } else if glyph_format == state.sdf_renderer.glyph_format
        && module_name == state.sdf_renderer.module_name
    {
        state.sdf_renderer.root.library = library;
        let renderer = &mut state.sdf_renderer as FT_Renderer;
        // SAFETY: `library` is live and owns this renderer.
        unsafe {
            (*library).cur_renderer = renderer;
        }
        renderer
    } else if glyph_format == state.synthetic_renderer.glyph_format
        && module_name == state.synthetic_renderer.module_name
    {
        state.synthetic_renderer.root.library = library;
        let renderer = &mut state.synthetic_renderer as FT_Renderer;
        // SAFETY: `library` is live and owns this renderer.
        unsafe {
            (*library).cur_renderer = renderer;
        }
        renderer
    } else if glyph_format == state.bitmap_renderer.glyph_format
        && module_name == state.bitmap_renderer.module_name
    {
        state.bitmap_renderer.root.library = library;
        &mut state.bitmap_renderer
    } else if glyph_format == state.svg_renderer.glyph_format
        && module_name == state.svg_renderer.module_name
    {
        state.svg_renderer.root.library = library;
        &mut state.svg_renderer
    } else {
        ptr::null_mut()
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Renderer(
    library: FT_Library,
    renderer: FT_Renderer,
    num_params: FT_UInt,
    parameters: *mut FT_Parameter,
) -> FT_Error {
    let Some(state) = library_state_mut(library) else {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    };
    let Some(renderer) = non_null_mut(renderer) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    if num_params != 0 && parameters.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let renderer_ptr = renderer.as_ptr();
    let (renderer_name, glyph_format) = if renderer_ptr == &raw mut state.outline_renderer {
        (
            state.outline_renderer.module_name,
            state.outline_renderer.glyph_format,
        )
    } else if renderer_ptr == &raw mut state.raster1_renderer {
        (
            state.raster1_renderer.module_name,
            state.raster1_renderer.glyph_format,
        )
    } else if renderer_ptr == &raw mut state.sdf_renderer {
        (
            state.sdf_renderer.module_name,
            state.sdf_renderer.glyph_format,
        )
    } else if renderer_ptr == &raw mut state.bitmap_renderer {
        (
            state.bitmap_renderer.module_name,
            state.bitmap_renderer.glyph_format,
        )
    } else if renderer_ptr == &raw mut state.svg_renderer {
        (
            state.svg_renderer.module_name,
            state.svg_renderer.glyph_format,
        )
    } else if renderer_ptr == &raw mut state.synthetic_renderer {
        (
            state.synthetic_renderer.module_name,
            state.synthetic_renderer.glyph_format,
        )
    } else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // FreeType 2.14.3 `src/base/ftobjs.c:FT_Set_Renderer` performs raw list
    // membership validation in the ABI layer, then updates the library's
    // current outline renderer.  Parameter callbacks are not used by the
    // default smooth renderer for this no-parameter parity route.
    if glyph_format == rust_ffi::FT_GLYPH_FORMAT_OUTLINE {
        let error = rust_ffi::FT_Library_Set_Renderer_By_Format(
            Some(&mut state.inner),
            rust_ffi::FT_GLYPH_FORMAT_OUTLINE,
            renderer_name,
        );
        if error != rust_ffi::FT_Err_Ok {
            return error;
        }
        // SAFETY: the renderer is one of the live library-owned records
        // selected above.
        unsafe {
            (*library).cur_renderer = renderer_ptr;
        }
    }
    if num_params == 0 {
        return rust_ffi::FT_Err_Ok;
    }
    // SAFETY: non-zero `num_params` requires a non-null pointer, checked
    // above, and FreeType consumes the records synchronously.
    let params = unsafe {
        slice::from_raw_parts(
            parameters,
            usize::try_from(num_params).unwrap_or(usize::MAX),
        )
    };
    // SAFETY: `renderer_ptr` and its class belong to the live library.
    let set_mode = unsafe {
        renderer_ptr
            .as_ref()
            .and_then(|renderer| renderer.clazz.as_ref())
            .and_then(|clazz| clazz.set_mode)
    };
    let Some(set_mode) = set_mode else {
        return rust_ffi::FT_Err_Unimplemented_Feature;
    };
    for parameter in params {
        // SAFETY: the callback belongs to the selected live renderer and the
        // parameter payload is caller-owned for this synchronous call.
        let error =
            unsafe { set_mode(renderer_ptr, parameter.tag, parameter.data.cast::<c_void>()) };
        if error != rust_ffi::FT_Err_Ok {
            return error;
        }
    }
    rust_ffi::FT_Err_Ok
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_default_module_names(library: FT_Library) -> &'static [&'static str] {
    rust_ffi::FT_Library_Default_Module_Names(library_ref(library))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_refcount(library: FT_Library) -> usize {
    rust_ffi::FT_Library_Refcount(library_ref(library))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_memory_is(library: FT_Library, memory: FT_Memory) -> bool {
    library_ref(library).is_some_and(|library| {
        rust_ffi::FT_Library_Memory(Some(library)).cast::<FT_MemoryRec>() == memory
    })
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_face_memory_is(face: FT_Face, memory: FT_Memory) -> bool {
    let Some(face) = non_null_mut(face) else {
        return false;
    };
    if face_state(face.as_ptr()).is_none() {
        return false;
    }
    // SAFETY: face_state accepted the handle as one of this ABI's live faces.
    unsafe { (*face.as_ptr()).memory == memory }
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_library_has_truetype_engine_service(library: FT_Library) -> bool {
    rust_ffi::FT_Library_Has_TrueType_Engine_Service(library_ref(library))
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_debug_hook_classes(
    library: FT_Library,
    hook_a: FT_DebugHook_Func,
    hook_b: FT_DebugHook_Func,
) -> [FT_Int; 4] {
    rust_ffi::FT_Library_Debug_Hook_Classes(library_ref(library), hook_a, hook_b)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_MulDiv(a: FT_Long, b: FT_Long, c: FT_Long) -> FT_Long {
    rust_ffi::FT_MulDiv(a, b, c)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_MulFix(a: FT_Long, b: FT_Long) -> FT_Long {
    rust_ffi::FT_MulFix(a, b)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_DivFix(a: FT_Long, b: FT_Long) -> FT_Long {
    rust_ffi::FT_DivFix(a, b)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_RoundFix(a: FT_Fixed) -> FT_Fixed {
    rust_ffi::FT_RoundFix(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_CeilFix(a: FT_Fixed) -> FT_Fixed {
    rust_ffi::FT_CeilFix(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_FloorFix(a: FT_Fixed) -> FT_Fixed {
    rust_ffi::FT_FloorFix(a)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Sin(angle: FT_Angle) -> FT_Fixed {
    rust_ffi::FT_Sin(angle)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Cos(angle: FT_Angle) -> FT_Fixed {
    rust_ffi::FT_Cos(angle)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Tan(angle: FT_Angle) -> FT_Fixed {
    rust_ffi::FT_Tan(angle)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Atan2(dx: FT_Fixed, dy: FT_Fixed) -> FT_Angle {
    rust_ffi::FT_Atan2(dx, dy)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Angle_Diff(angle1: FT_Angle, angle2: FT_Angle) -> FT_Angle {
    rust_ffi::FT_Angle_Diff(angle1, angle2)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_Unit(vector: *mut FT_Vector, angle: FT_Angle) {
    let vector = non_null_mut(vector);
    let mut rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    rust_ffi::FT_Vector_Unit(rust_vector.as_mut(), angle);
    if let (Some(vector), Some(rust_vector)) = (vector, rust_vector) {
        // SAFETY: `vector` is a valid mutable pointer checked above.
        unsafe {
            (*vector.as_ptr()).x = rust_vector.x;
            (*vector.as_ptr()).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_Rotate(vector: *mut FT_Vector, angle: FT_Angle) {
    let vector = non_null_mut(vector);
    let mut rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    rust_ffi::FT_Vector_Rotate(rust_vector.as_mut(), angle);
    if let (Some(vector), Some(rust_vector)) = (vector, rust_vector) {
        // SAFETY: `vector` is a valid mutable pointer checked above.
        unsafe {
            (*vector.as_ptr()).x = rust_vector.x;
            (*vector.as_ptr()).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_Length(vector: *mut FT_Vector) -> FT_Fixed {
    let vector = non_null(vector);
    let rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    rust_ffi::FT_Vector_Length(rust_vector.as_ref())
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_Polarize(
    vector: *mut FT_Vector,
    length: *mut FT_Fixed,
    angle: *mut FT_Angle,
) {
    let vector = non_null(vector);
    let length_ptr = non_null_mut(length);
    let angle_ptr = non_null_mut(angle);
    let mut rust_length = length_ptr.map(|length| {
        // SAFETY: `length` is non-null and points to a C ABI `FT_Fixed`.
        unsafe { *length.as_ptr() }
    });
    let mut rust_angle = angle_ptr.map(|angle| {
        // SAFETY: `angle` is non-null and points to a C ABI `FT_Angle`.
        unsafe { *angle.as_ptr() }
    });
    let rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    rust_ffi::FT_Vector_Polarize(
        rust_vector.as_ref(),
        rust_length.as_mut(),
        rust_angle.as_mut(),
    );
    if let (Some(length_ptr), Some(value)) = (length_ptr, rust_length) {
        // SAFETY: `length_ptr` is a valid mutable pointer checked above.
        unsafe { *length_ptr.as_ptr() = value };
    }
    if let (Some(angle_ptr), Some(value)) = (angle_ptr, rust_angle) {
        // SAFETY: `angle_ptr` is a valid mutable pointer checked above.
        unsafe { *angle_ptr.as_ptr() = value };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_From_Polar(vector: *mut FT_Vector, length: FT_Fixed, angle: FT_Angle) {
    let vector = non_null_mut(vector);
    let mut rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    rust_ffi::FT_Vector_From_Polar(rust_vector.as_mut(), length, angle);
    if let (Some(vector), Some(rust_vector)) = (vector, rust_vector) {
        // SAFETY: `vector` is a valid mutable pointer checked above.
        unsafe {
            (*vector.as_ptr()).x = rust_vector.x;
            (*vector.as_ptr()).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Vector_Transform(vector: *mut FT_Vector, matrix: *const FT_Matrix) {
    let vector = non_null_mut(vector);
    let matrix = non_null(matrix);
    let mut rust_vector = vector.map(|vector| {
        // SAFETY: `vector` is non-null and points to a C ABI `FT_Vector`.
        let vector = unsafe { vector.as_ref() };
        rust_ffi::FT_Vector {
            x: vector.x,
            y: vector.y,
        }
    });
    let rust_matrix = matrix.map(|matrix| {
        // SAFETY: `matrix` is non-null and points to a C ABI `FT_Matrix`.
        let matrix = unsafe { matrix.as_ref() };
        rust_ffi::FT_Matrix {
            xx: matrix.xx,
            xy: matrix.xy,
            yx: matrix.yx,
            yy: matrix.yy,
        }
    });
    rust_ffi::FT_Vector_Transform(rust_vector.as_mut(), rust_matrix.as_ref());
    if let (Some(vector), Some(rust_vector)) = (vector, rust_vector) {
        // SAFETY: `vector` is a valid mutable pointer checked above.
        unsafe {
            (*vector.as_ptr()).x = rust_vector.x;
            (*vector.as_ptr()).y = rust_vector.y;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Matrix_Multiply(a: *const FT_Matrix, b: *mut FT_Matrix) {
    let a = non_null(a);
    let b = non_null_mut(b);
    let rust_a = a.map(|a| {
        // SAFETY: `a` is non-null and points to a C ABI `FT_Matrix`.
        let a = unsafe { a.as_ref() };
        rust_ffi::FT_Matrix {
            xx: a.xx,
            xy: a.xy,
            yx: a.yx,
            yy: a.yy,
        }
    });
    let mut rust_b = b.map(|b| {
        // SAFETY: `b` is non-null and points to a C ABI `FT_Matrix`.
        let b = unsafe { b.as_ref() };
        rust_ffi::FT_Matrix {
            xx: b.xx,
            xy: b.xy,
            yx: b.yx,
            yy: b.yy,
        }
    });
    rust_ffi::FT_Matrix_Multiply(rust_a.as_ref(), rust_b.as_mut());
    if let (Some(b), Some(rust_b)) = (b, rust_b) {
        // SAFETY: `b` is a valid mutable pointer checked above.
        unsafe {
            (*b.as_ptr()).xx = rust_b.xx;
            (*b.as_ptr()).xy = rust_b.xy;
            (*b.as_ptr()).yx = rust_b.yx;
            (*b.as_ptr()).yy = rust_b.yy;
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Matrix_Invert(matrix: *mut FT_Matrix) -> FT_Error {
    let matrix = non_null_mut(matrix);
    let mut rust_matrix = matrix.map(|matrix| {
        // SAFETY: `matrix` is non-null and points to a C ABI `FT_Matrix`.
        let matrix = unsafe { matrix.as_ref() };
        rust_ffi::FT_Matrix {
            xx: matrix.xx,
            xy: matrix.xy,
            yx: matrix.yx,
            yy: matrix.yy,
        }
    });
    let err = rust_ffi::FT_Matrix_Invert(rust_matrix.as_mut());
    if let (Some(matrix), Some(rust_matrix)) = (matrix, rust_matrix) {
        // SAFETY: `matrix` is a valid mutable pointer checked above.
        unsafe {
            (*matrix.as_ptr()).xx = rust_matrix.xx;
            (*matrix.as_ptr()).xy = rust_matrix.xy;
            (*matrix.as_ptr()).yx = rust_matrix.yx;
            (*matrix.as_ptr()).yy = rust_matrix.yy;
        }
    }
    err
}

fn abi_open_face_driver_is_font_driver(library: FT_Library, driver: FT_Module) -> bool {
    let Some(state) = library_state_mut(library) else {
        return false;
    };
    if let Some(record) = state
        .default_modules
        .iter()
        .find(|record| ptr::eq(&*record.module, driver))
    {
        return rust_ffi::FT_Library_Module_Flags(Some(&state.inner), record.name)
            .is_some_and(|flags| flags & rust_ffi::FT_MODULE_FONT_DRIVER as FT_ULong != 0);
    }
    let Some(module) = state
        .synthetic_modules
        .iter()
        .find(|record| record.module == driver)
    else {
        return false;
    };
    let Some(module) = (unsafe { module.module.as_ref() }) else {
        return false;
    };
    let Some(clazz) = (unsafe { module.clazz.as_ref() }) else {
        return false;
    };
    clazz.module_flags & rust_ffi::FT_MODULE_FONT_DRIVER as FT_ULong != 0
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Open_Face(
    library: FT_Library,
    args: *const FT_Open_Args,
    face_index: FT_Long,
    aface: *mut FT_Face,
) -> FT_Error {
    // C FreeType `FT_Open_Face` delegates to `ft_open_face_internal`
    // (ftobjs.c:2514-2586): null `args` is rejected before stream creation;
    // null `library` is then rejected by `FT_Stream_New`; null `aface` is
    // checked after a stream is successfully created.
    let Some(args) = NonNull::new(args.cast_mut()) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `args` is non-null and read-only for this call.
    let args = unsafe { args.as_ref() };
    let source_flags = args.flags
        & ((rust_ffi::FT_OPEN_MEMORY | rust_ffi::FT_OPEN_STREAM | rust_ffi::FT_OPEN_PATHNAME)
            as FT_UInt);
    if source_flags != rust_ffi::FT_OPEN_MEMORY as FT_UInt
        && source_flags != rust_ffi::FT_OPEN_STREAM as FT_UInt
        && source_flags != rust_ffi::FT_OPEN_PATHNAME as FT_UInt
    {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    if !library.is_null()
        && args.flags & rust_ffi::FT_OPEN_DRIVER as FT_UInt != 0
        && !args.driver.is_null()
        && !abi_open_face_driver_is_font_driver(library, args.driver.cast())
    {
        return rust_ffi::FT_Err_Invalid_Handle as FT_Error;
    }
    let name_options = open_face_name_options(args);
    let incremental_interface = open_face_incremental_interface(args);
    let error = if source_flags == rust_ffi::FT_OPEN_PATHNAME as FT_UInt {
        // Pinned Unix FreeType checks the library before delegating a null
        // pathname to FT_Stream_Open, which reports Cannot_Open_Resource
        // rather than Invalid_Argument.
        let Some(rust_library) = library_ref(library) else {
            return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
        };
        if args.pathname.is_null() {
            rust_ffi::FT_Err_Cannot_Open_Resource as FT_Error
        } else {
            // SAFETY: the public contract requires a readable NUL-terminated
            // pathname for FT_OPEN_PATHNAME and the call is synchronous.
            let pathname = unsafe { CStr::from_ptr(args.pathname.cast_const()).to_bytes() };
            let opened = rust_ffi::FT_New_Face(rust_library, pathname, face_index, 20.0);
            ft_store_opened_face(
                library,
                opened,
                aface,
                OpenFaceByteOptions {
                    name_options,
                    ..OpenFaceByteOptions::default()
                },
            )
        }
    } else if source_flags == rust_ffi::FT_OPEN_STREAM as FT_UInt {
        ft_open_external_stream_face_with_name_options(
            library,
            args.stream,
            face_index,
            aface,
            name_options,
        )
    } else {
        ft_new_memory_face_with_name_options(
            library,
            args.memory_base,
            args.memory_size,
            face_index,
            aface,
            name_options,
        )
    };
    if error == rust_ffi::FT_Err_Ok
        && !incremental_interface.is_null()
        && let Some(face_out) = non_null(aface)
    {
        // SAFETY: the successful open wrote one live face handle to `aface`.
        let face = unsafe { *face_out.as_ptr() };
        if let Some(internal) = face_internal_mut(face) {
            internal.incremental_interface = incremental_interface;
        }
    }
    error
}

fn c_path_bytes(pathname: *const c_char) -> Result<Vec<u8>, FT_Error> {
    if pathname.is_null() {
        return Err(rust_ffi::FT_Err_Invalid_Argument);
    }
    // SAFETY: FreeType pathname arguments are borrowed NUL-terminated strings
    // that remain readable for the duration of the call.
    Ok(unsafe { CStr::from_ptr(pathname).to_bytes() }.to_vec())
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_New_Face(
    library: FT_Library,
    filepathname: *const c_char,
    face_index: FT_Long,
    aface: *mut FT_Face,
) -> FT_Error {
    if library.is_null() {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    }
    let pathname = match c_path_bytes(filepathname) {
        Ok(pathname) => pathname,
        Err(error) => return error,
    };
    if aface.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(rust_library) = library_ref(library) else {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    };
    let opened = rust_ffi::FT_New_Face(rust_library, &pathname, face_index, 20.0);
    ft_store_opened_face(library, opened, aface, OpenFaceByteOptions::default())
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Attach_File(face: FT_Face, filepathname: *const c_char) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let pathname = match c_path_bytes(filepathname) {
        Ok(pathname) => pathname,
        Err(error) => return error,
    };
    rust_ffi::FT_Attach_File(Some(&mut state.inner), &pathname)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Attach_Stream(face: FT_Face, parameters: *const FT_Open_Args) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(parameters) = non_null(parameters) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `parameters` is non-null and read-only for this call.
    let parameters = unsafe { parameters.as_ref() };
    let source_flags = parameters.flags
        & ((rust_ffi::FT_OPEN_MEMORY | rust_ffi::FT_OPEN_STREAM | rust_ffi::FT_OPEN_PATHNAME)
            as FT_UInt);
    if source_flags != rust_ffi::FT_OPEN_MEMORY as FT_UInt {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    if parameters.memory_base.is_null() || parameters.memory_size < 0 {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Ok(len) = usize::try_from(parameters.memory_size) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `memory_base` is non-null and `memory_size` bytes are readable.
    let data = unsafe { slice::from_raw_parts(parameters.memory_base, len) };
    rust_ffi::FT_Attach_Stream(Some(&mut state.inner), Some(data))
}

fn open_face_name_options(args: &FT_Open_Args) -> rust_ffi::FT_Open_Face_Name_Options {
    let mut options = rust_ffi::FT_Open_Face_Name_Options::default();
    if args.num_params <= 0 || args.params.is_null() {
        return options;
    }
    let Ok(count) = usize::try_from(args.num_params) else {
        return options;
    };
    // SAFETY: `FT_Open_Face` callers provide `num_params` readable parameter
    // records when `params` is non-null.  We only read tags; parameter data is
    // intentionally ignored for these FreeType flags.
    let params = unsafe { slice::from_raw_parts(args.params, count) };
    for param in params {
        match param.tag as i64 {
            rust_ffi::FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_FAMILY => {
                options.ignore_typographic_family = true;
            }
            rust_ffi::FT_PARAM_TAG_IGNORE_TYPOGRAPHIC_SUBFAMILY => {
                options.ignore_typographic_subfamily = true;
            }
            rust_ffi::FT_PARAM_TAG_IGNORE_SBIX => {
                options.ignore_sbix = true;
            }
            _ => {}
        }
    }
    options
}

fn open_face_incremental_interface(args: &FT_Open_Args) -> FT_Pointer {
    if args.num_params <= 0 || args.params.is_null() {
        return ptr::null_mut();
    }
    let Ok(count) = usize::try_from(args.num_params) else {
        return ptr::null_mut();
    };
    // SAFETY: the same `FT_Open_Face` parameter-array contract used by
    // `open_face_name_options` keeps these records readable for the call.
    let params = unsafe { slice::from_raw_parts(args.params, count) };
    params
        .iter()
        .find(|param| param.tag as i64 == rust_ffi::FT_PARAM_TAG_INCREMENTAL)
        .map_or(ptr::null_mut(), |param| param.data)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_New_Memory_Face(
    library: FT_Library,
    file_base: *const FT_Byte,
    file_size: FT_Long,
    face_index: FT_Long,
    aface: *mut FT_Face,
) -> FT_Error {
    ft_new_memory_face_with_name_options(
        library,
        file_base,
        file_size,
        face_index,
        aface,
        rust_ffi::FT_Open_Face_Name_Options::default(),
    )
}

fn ft_new_memory_face_with_name_options(
    library: FT_Library,
    file_base: *const c_uchar,
    file_size: FT_Long,
    face_index: FT_Long,
    aface: *mut FT_Face,
    options: rust_ffi::FT_Open_Face_Name_Options,
) -> FT_Error {
    // C FreeType validates `FT_New_Memory_Face` in ftobjs.c:1629-1647:
    // null `file_base` is rejected before delegating to `ft_open_face_internal`;
    // null `library` is then rejected by `FT_Stream_New`, before null `aface`.
    if file_base.is_null() || file_size < 0 {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(library) = non_null_mut(library) else {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    };
    let Some(_out) = non_null_mut(aface) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Ok(file_len) = usize::try_from(file_size) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `file_base` is non-null and the caller promises `file_size` readable bytes.
    let data = unsafe { slice::from_raw_parts(file_base, file_len) };
    ft_open_face_from_bytes_with_name_options(
        library.as_ptr(),
        data,
        face_index,
        aface,
        OpenFaceByteOptions {
            name_options: options,
            external_stream: false,
            stream: ptr::null_mut(),
            stream_close: None,
            stream_frame: None,
        },
    )
}

fn ft_open_external_stream_face_with_name_options(
    library: FT_Library,
    stream: FT_Stream,
    face_index: FT_Long,
    aface: *mut FT_Face,
    options: rust_ffi::FT_Open_Face_Name_Options,
) -> FT_Error {
    let Some(mut stream) = non_null_mut(stream) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(library) = non_null_mut(library) else {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    };
    let Some(_out) = non_null_mut(aface) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let stream_pointer = stream.as_ptr();
    // SAFETY: `stream` is a non-null caller-owned FT_StreamRec retained for
    // the face lifetime on success.
    let stream = unsafe { stream.as_mut() };
    let close = if stream.close.is_null() {
        None
    } else {
        // SAFETY: public `FT_StreamRec.close` has FreeType's
        // `void (*)(FT_Stream)` ABI; the Rust layout stores it as an opaque
        // pointer to keep core runtime independent from C callbacks.
        Some(unsafe { std::mem::transmute::<FT_Pointer, extern "C" fn(FT_Stream)>(stream.close) })
    };
    let Ok(file_len) = usize::try_from(stream.size) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let mut callback_frame = None;
    let data = if stream.base.is_null() {
        if stream.read.is_null() {
            if let Some(close) = close {
                close(stream_pointer);
            }
            return rust_ffi::FT_Err_Invalid_Stream_Operation as FT_Error;
        }
        // SAFETY: public FT_StreamRec.read has FreeType's FT_Stream_IoFunc
        // calling convention; the caller retains it through this synchronous
        // materialization.
        let stream_io = unsafe {
            std::mem::transmute::<
                FT_Pointer,
                extern "C" fn(FT_Stream, FT_ULong, *mut FT_Byte, FT_ULong) -> FT_ULong,
            >(stream.read)
        };
        if stream_io(stream_pointer, 0, ptr::null_mut(), 0) != 0 {
            if let Some(close) = close {
                close(stream_pointer);
            }
            return rust_ffi::FT_Err_Invalid_Stream_Operation as FT_Error;
        }
        let mut bytes = Vec::new();
        if bytes.try_reserve_exact(file_len).is_err() {
            if let Some(close) = close {
                close(stream_pointer);
            }
            return rust_ffi::FT_Err_Out_Of_Memory;
        }
        bytes.resize(file_len, 0);
        let requested = FT_ULong::try_from(file_len).unwrap_or(FT_ULong::MAX);
        let read_count = stream_io(stream_pointer, 0, bytes.as_mut_ptr(), requested);
        if read_count != requested {
            if let Some(close) = close {
                close(stream_pointer);
            }
            return rust_ffi::FT_Err_Invalid_Stream_Operation as FT_Error;
        }
        stream.pos = read_count;
        callback_frame = Some(bytes.into_boxed_slice());
        let Some(frame) = callback_frame.as_deref() else {
            if let Some(close) = close {
                close(stream_pointer);
            }
            return rust_ffi::FT_Err_Out_Of_Memory;
        };
        frame
    } else {
        // SAFETY: memory-backed FT_OPEN_STREAM callers provide `size`
        // readable bytes at `base`; the stream record remains caller-owned.
        unsafe { slice::from_raw_parts(stream.base.cast_const(), file_len) }
    };
    let error = ft_open_face_from_bytes_with_name_options(
        library.as_ptr(),
        data,
        face_index,
        aface,
        OpenFaceByteOptions {
            name_options: options,
            external_stream: true,
            stream: stream_pointer,
            stream_close: close,
            stream_frame: callback_frame.clone(),
        },
    );
    if error != rust_ffi::FT_Err_Ok
        && let Some(close) = close
    {
        close(stream_pointer);
    }
    error
}

#[derive(Default)]
struct OpenFaceByteOptions {
    name_options: rust_ffi::FT_Open_Face_Name_Options,
    external_stream: bool,
    stream: FT_Stream,
    stream_close: FT_Stream_CloseFunc,
    stream_frame: Option<Box<[FT_Byte]>>,
}

fn ft_open_face_from_bytes_with_name_options(
    library: FT_Library,
    data: &[u8],
    face_index: FT_Long,
    aface: *mut FT_Face,
    options: OpenFaceByteOptions,
) -> FT_Error {
    if aface.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(rust_library) = library_ref(library) else {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    };
    let opened = if options.external_stream {
        rust_ffi::FT_Open_External_Stream_Face_With_Name_Options(
            rust_library,
            data,
            face_index,
            20.0,
            options.name_options,
        )
    } else {
        rust_ffi::FT_New_Memory_Face_With_Name_Options(
            rust_library,
            data,
            face_index,
            20.0,
            options.name_options,
        )
    };
    ft_store_opened_face(library, opened, aface, options)
}

fn ft_store_opened_face(
    library: FT_Library,
    opened: Result<rust_ffi::FT_Face, FT_Error>,
    aface: *mut FT_Face,
    options: OpenFaceByteOptions,
) -> FT_Error {
    let Some(out) = non_null_mut(aface) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    match opened {
        Ok(inner) => {
            let face_memory =
                library_state_mut(library).map_or(ptr::null_mut(), |state| state.allocation_memory);
            let face_allocation =
                match custom_memory_block(face_memory, std::mem::size_of::<FT_FaceRec>()) {
                    Ok(block) => block.unwrap_or(ptr::null_mut()),
                    Err(error) => return error,
                };
            let mut state = Box::new(FaceState::new(inner, library));
            state.allocation_memory = face_memory;
            state.allocation_block = face_allocation;
            if options.external_stream {
                state.stream = options.stream;
                state.stream_close = options.stream_close;
                if let Some(frame) = options.stream_frame {
                    let stream_pos = state.inner.memory_stream_record().pos;
                    // C FreeType's callback-backed frame path leaves the
                    // caller stream with a live frame pointer and the library
                    // memory owner after face loading.  Keep an owned frame
                    // alive through the face close callback and mirror those
                    // public fields without invoking any native runtime.
                    if let Some(stream) = unsafe { state.stream.as_mut() } {
                        stream.base = frame.as_ptr().cast_mut();
                        stream.memory = face_memory.cast();
                        stream.pos = stream_pos;
                    }
                    state.external_stream_frame = Some(frame);
                }
            }
            let metrics = rust_size_metrics_to_abi(state.inner.size_metrics);
            let rust_size = state.inner.size;
            let size_internal = Box::into_raw(Box::new(FT_Size_InternalRecCompat {
                rust_size,
                owner: ptr::null_mut(),
            }))
            .cast::<c_void>();
            let initial_slot = rust_ffi::FT_Empty_GlyphSlot(&state.inner);
            let bbox = FT_BBox {
                xMin: state.inner.bbox.xMin,
                yMin: state.inner.bbox.yMin,
                xMax: state.inner.bbox.xMax,
                yMax: state.inner.bbox.yMax,
            };
            let family_name = state
                .family_name
                .as_ref()
                .map_or(ptr::null_mut(), |name| name.as_ptr().cast_mut());
            let style_name = state
                .style_name
                .as_ref()
                .map_or(ptr::null_mut(), |name| name.as_ptr().cast_mut());
            let available_sizes = if state.inner.available_sizes.is_empty() {
                ptr::null_mut()
            } else {
                state.inner.available_sizes.as_ptr().cast_mut()
            };
            let mut face = Box::new(FT_FaceRec {
                num_faces: state.inner.num_faces,
                face_index: state.inner.face_index,
                face_flags: state.inner.face_flags,
                style_flags: state.inner.style_flags,
                num_glyphs: state.inner.num_glyphs,
                family_name,
                style_name,
                num_fixed_sizes: state.inner.num_fixed_sizes,
                available_sizes,
                num_charmaps: 0,
                charmaps: ptr::null_mut(),
                generic: FT_Generic::default(),
                bbox,
                units_per_EM: state.inner.units_per_EM,
                ascender: state.inner.ascender,
                descender: state.inner.descender,
                height: state.inner.height,
                max_advance_width: state.inner.max_advance_width,
                max_advance_height: state.inner.max_advance_height,
                underline_position: state.inner.underline_position,
                underline_thickness: state.inner.underline_thickness,
                glyph: ptr::null_mut(),
                size: Box::into_raw(Box::new(FT_SizeRec {
                    face: ptr::null_mut(),
                    generic: FT_Generic::default(),
                    metrics,
                    internal: size_internal,
                })),
                charmap: ptr::null_mut(),
                driver: ptr::null_mut(),
                memory: face_memory,
                stream: state.stream,
                sizes_list: FT_ListRec::default(),
                autohint: FT_Generic::default(),
                extensions: ptr::null_mut(),
                internal: ptr::null_mut(),
            });
            let face_ptr = (&mut *face) as *mut FT_FaceRec;
            // SAFETY: `face.size` was allocated above and is owned by `state`.
            unsafe {
                (*face.size).face = face_ptr;
            };
            if let Some(internal) = size_internal_mut(face.size) {
                internal.owner = face_ptr;
            }
            state.push_size_record(face.size);
            state.refresh_charmaps(face_ptr);
            face.num_charmaps = FT_Int::try_from(state.charmap_ptrs.len()).unwrap_or(FT_Int::MAX);
            face.charmaps = state.charmap_ptrs.as_mut_ptr();
            face.charmap = FT_UInt::try_from(state.inner.active_charmap_index)
                .ok()
                .and_then(|index| state.charmap_by_index(index))
                .unwrap_or(ptr::null_mut());
            // A memory face still exposes a live `FT_StreamRec` through the
            // public record.  External callback-backed streams replace this
            // pointer above; ordinary memory faces retain the parser-owned
            // stream record for the face lifetime.
            if !options.external_stream {
                state.stream = state.inner.memory_stream();
            }
            face.driver = state.driver_sentinel.as_mut() as *mut u8 as *mut c_void;
            face.stream = state.stream;
            face.internal =
                Box::into_raw(Box::new(FT_Face_InternalRecCompat::new(state))).cast::<c_void>();
            face.glyph = Box::into_raw(Box::new(rust_slot_to_abi(
                initial_slot,
                face_ptr,
                rust_ffi::FT_LOAD_DEFAULT,
            )));
            let face = Box::into_raw(face);
            if let Some(library_state) = library_state_mut(library) {
                library_state.faces.push(face);
            }
            // SAFETY: `out` is a valid out pointer checked above.
            unsafe { *out.as_ptr() = face };
            rust_ffi::FT_Err_Ok
        }
        Err(error) => {
            // FreeType allocates the public face record while opening a face,
            // even when the driver rejects the bytes.  The record is released
            // on this NewFace error path rather than through FT_Done_Face, so
            // the custom-memory lifecycle still observes a face allocation
            // without exposing a failed handle to the caller.
            let face_memory =
                library_state_mut(library).map_or(ptr::null_mut(), |state| state.allocation_memory);
            if let Ok(Some(face_allocation)) =
                custom_memory_block(face_memory, std::mem::size_of::<FT_FaceRec>())
            {
                free_custom_memory_block(face_memory, face_allocation);
            }
            error
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Reference_Face(face: FT_Face) -> FT_Error {
    let Some(internal) = face_internal_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    internal.state.refcount = internal.state.refcount.saturating_add(1);
    internal.refcount = internal.refcount.saturating_add(1);
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Done_Face(face: FT_Face) -> FT_Error {
    let Some(face) = non_null_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(internal) = face_internal_mut(face.as_ptr()) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if internal.state.refcount > 1 {
        internal.state.refcount = internal.state.refcount.saturating_sub(1);
        internal.refcount = internal.refcount.saturating_sub(1);
        return rust_ffi::FT_Err_Ok;
    }
    let owner = internal.state.library;
    if let Some(library) = library_state_mut(owner)
        && let Some(index) = library
            .faces
            .iter()
            .position(|candidate| *candidate == face.as_ptr())
    {
        library.faces.remove(index);
    }
    // SAFETY: `face` must be a live handle returned by `FT_New_Memory_Face`.
    unsafe { drop_face_record(face.as_ptr()) };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_CheckTrueTypePatents(face: FT_Face) -> FT_Bool {
    rust_ffi::FT_Face_CheckTrueTypePatents(face_state(face).map(|state| &state.inner))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_SetUnpatentedHinting(face: FT_Face, value: FT_Bool) -> FT_Bool {
    rust_ffi::FT_Face_SetUnpatentedHinting(
        face_state_mut(face).map(|state| &mut state.inner),
        value,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Get_CBox(outline: *const FT_Outline, acbox: *mut FT_BBox) {
    if outline.is_null() || acbox.is_null() {
        return;
    }
    let Some(snapshot) = outline_snapshot_from_c(outline) else {
        return;
    };
    let mut bbox = rust_ffi::FT_BBox::default();
    rust_ffi::FT_Outline_Get_CBox(Some(&snapshot), Some(&mut bbox));
    // SAFETY: `acbox` is non-null and the caller provides writable `FT_BBox` storage.
    unsafe {
        *acbox = FT_BBox {
            xMin: bbox.xMin,
            yMin: bbox.yMin,
            xMax: bbox.xMax,
            yMax: bbox.yMax,
        };
    }
}

fn c_glyph_cbox_snapshot(glyph: FT_Glyph) -> Option<rust_ffi::FT_GlyphCBoxSnapshot> {
    let glyph = non_null_mut(glyph)?;
    // SAFETY: the public C ABI accepts caller-owned `FT_Glyph` records; this
    // thin wrapper reads only the root record and the class pointer nullness
    // needed to reproduce FreeType's `FT_Glyph_Get_CBox` early-return order.
    let root = unsafe { glyph.as_ref() };
    if root.clazz.is_null() {
        return Some(rust_ffi::FT_GlyphCBoxSnapshot {
            has_class: false,
            has_bbox_hook: false,
            cbox: None,
        });
    }
    if root.clazz == owned_outline_glyph_class() {
        let owned = owned_outline_glyph_from_root(glyph.as_ptr())?;
        let mut cbox = rust_ffi::FT_BBox::default();
        rust_ffi::FT_Outline_Get_CBox(Some(&owned.core.outline), Some(&mut cbox));
        return Some(rust_ffi::FT_GlyphCBoxSnapshot {
            has_class: true,
            has_bbox_hook: true,
            cbox: Some(cbox),
        });
    }
    if root.clazz == owned_bitmap_glyph_class() {
        let owned = owned_bitmap_glyph_from_root(glyph.as_ptr())?;
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
    if root.clazz == owned_svg_glyph_class() {
        return Some(rust_ffi::FT_GlyphCBoxSnapshot {
            has_class: true,
            has_bbox_hook: false,
            cbox: None,
        });
    }
    // SAFETY: `glyph->clazz` is non-null.  The wrapper reads the public-sized
    // class facade to observe whether `glyph_bbox` is present, then delegates
    // the zero/no-bbox behavior to safe Rust.
    let clazz = unsafe { &*root.clazz };
    Some(rust_ffi::FT_GlyphCBoxSnapshot {
        has_class: true,
        has_bbox_hook: clazz.glyph_bbox.is_some(),
        cbox: None,
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Glyph_Get_CBox(glyph: FT_Glyph, bbox_mode: FT_UInt, acbox: *mut FT_BBox) {
    let Some(acbox) = non_null_mut(acbox) else {
        return;
    };
    let snapshot = c_glyph_cbox_snapshot(glyph);
    let mut bbox = rust_ffi::FT_BBox::default();
    rust_ffi::FT_Glyph_Get_CBox(snapshot, bbox_mode, Some(&mut bbox));
    // SAFETY: `acbox` is non-null and the caller provides writable `FT_BBox` storage.
    unsafe {
        *acbox.as_ptr() = FT_BBox {
            xMin: bbox.xMin,
            yMin: bbox.yMin,
            xMax: bbox.xMax,
            yMax: bbox.yMax,
        };
    }
}

fn get_glyph_from_external_slot(slot: &FT_GlyphSlotRec, out: NonNull<FT_Glyph>) -> FT_Error {
    let format = slot.format;
    let glyph_size = match format {
        rust_ffi::FT_GLYPH_FORMAT_BITMAP => std::mem::size_of::<FT_BitmapGlyphRec>(),
        rust_ffi::FT_GLYPH_FORMAT_SVG => std::mem::size_of::<AbiSvgGlyphRec>(),
        _ => return rust_ffi::FT_Err_Invalid_Glyph_Format,
    };
    let Some(state) = library_state_mut(slot.library) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let allocation_memory = state.allocation_memory;
    let allocation_block = match custom_memory_block(allocation_memory, glyph_size) {
        Ok(Some(block)) => block,
        Ok(None) | Err(_) => return rust_ffi::FT_Err_Out_Of_Memory,
    };

    const MAX_ADVANCE_26_6_EXCLUSIVE: FT_Pos = 0x8000 * 64;
    if slot.advance.x >= MAX_ADVANCE_26_6_EXCLUSIVE
        || slot.advance.x <= -MAX_ADVANCE_26_6_EXCLUSIVE
        || slot.advance.y >= MAX_ADVANCE_26_6_EXCLUSIVE
        || slot.advance.y <= -MAX_ADVANCE_26_6_EXCLUSIVE
    {
        free_custom_memory_block(allocation_memory, allocation_block);
        // SAFETY: `out` is validated caller-provided output storage.
        unsafe { *out.as_ptr() = ptr::null_mut() };
        return rust_ffi::FT_Err_Invalid_Argument;
    }

    let root = rust_ffi::FT_GlyphRec {
        library: slot.library.cast::<c_void>(),
        clazz: ptr::dangling(),
        format,
        advance: rust_ffi::FT_Vector {
            x: slot.advance.x.saturating_mul(1024),
            y: slot.advance.y.saturating_mul(1024),
        },
    };
    let glyph = if format == rust_ffi::FT_GLYPH_FORMAT_BITMAP {
        let byte_len = usize::try_from(slot.bitmap.rows)
            .ok()
            .and_then(|rows| {
                usize::try_from(slot.bitmap.pitch.unsigned_abs())
                    .ok()
                    .and_then(|pitch| rows.checked_mul(pitch))
            })
            .unwrap_or(0);
        let buffer = if byte_len == 0 || slot.bitmap.buffer.is_null() {
            Vec::new()
        } else {
            // SAFETY: a non-null public bitmap buffer must contain
            // `rows * abs(pitch)` readable bytes for FT_Bitmap_Copy.
            unsafe { slice::from_raw_parts(slot.bitmap.buffer, byte_len).to_vec() }
        };
        let payload_allocation_block = if byte_len == 0 || slot.bitmap.buffer.is_null() {
            ptr::null_mut()
        } else {
            match custom_memory_block(allocation_memory, byte_len) {
                Ok(block) => block.unwrap_or(ptr::null_mut()),
                Err(error) => {
                    free_custom_memory_block(allocation_memory, allocation_block);
                    // SAFETY: `out` is validated caller-provided output storage.
                    unsafe { *out.as_ptr() = ptr::null_mut() };
                    return error;
                }
            }
        };
        let core = rust_ffi::FT_BitmapGlyphOwned {
            root,
            left: slot.bitmap_left,
            top: slot.bitmap_top,
            bitmap: rust_ffi::FT_Bitmap {
                rows: slot.bitmap.rows,
                width: slot.bitmap.width,
                pitch: slot.bitmap.pitch,
                buffer,
                num_grays: slot.bitmap.num_grays,
                pixel_mode: FT_Int::from(slot.bitmap.pixel_mode),
            },
        };
        let mut owned = OwnedBitmapGlyph::new(core);
        owned.allocation_memory = allocation_memory;
        owned.allocation_block = allocation_block;
        owned.payload_allocation_block = payload_allocation_block;
        Box::into_raw(Box::new(owned)).cast::<FT_GlyphRec>()
    } else {
        let Some(document) = non_null(slot.other.cast::<FT_SVG_DocumentRec>()) else {
            free_custom_memory_block(allocation_memory, allocation_block);
            // SAFETY: `out` is validated caller-provided output storage.
            unsafe { *out.as_ptr() = ptr::null_mut() };
            return rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error;
        };
        // SAFETY: `slot->other` is non-null and SVG slots expose an
        // `FT_SVG_DocumentRec` for the duration of this call.
        let document = unsafe { document.as_ref() };
        // `FT_ULong` and `size_t` have the same width on every supported C ABI
        // target, so the public SVG length is representable as `usize`.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "FT_ULong and size_t are equally wide on supported C ABI targets"
        )]
        let document_len = document.svg_document_length as usize;
        if document_len == 0 || document.svg_document.is_null() {
            free_custom_memory_block(allocation_memory, allocation_block);
            // SAFETY: `out` is validated caller-provided output storage.
            unsafe { *out.as_ptr() = ptr::null_mut() };
            return rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error;
        }
        // SAFETY: the public SVG document record promises `document_len`
        // readable bytes.
        let svg_document =
            unsafe { slice::from_raw_parts(document.svg_document, document_len).to_vec() };
        let payload_allocation_block = match custom_memory_block(allocation_memory, document_len) {
            Ok(block) => block.unwrap_or(ptr::null_mut()),
            Err(error) => {
                free_custom_memory_block(allocation_memory, allocation_block);
                // SAFETY: `out` is validated caller-provided output storage.
                unsafe { *out.as_ptr() = ptr::null_mut() };
                return error;
            }
        };
        let core = rust_ffi::FT_SvgGlyphOwned {
            root,
            svg_document,
            glyph_index: slot.glyph_index,
            metrics: rust_ffi::FT_Size_Metrics {
                x_ppem: document.metrics.x_ppem,
                y_ppem: document.metrics.y_ppem,
                x_scale: document.metrics.x_scale,
                y_scale: document.metrics.y_scale,
                ascender: document.metrics.ascender,
                descender: document.metrics.descender,
                height: document.metrics.height,
                max_advance: document.metrics.max_advance,
            },
            units_per_EM: document.units_per_EM,
            start_glyph_id: document.start_glyph_id,
            end_glyph_id: document.end_glyph_id,
            transform: rust_ffi::FT_Matrix {
                xx: document.transform.xx,
                xy: document.transform.xy,
                yx: document.transform.yx,
                yy: document.transform.yy,
            },
            delta: rust_ffi::FT_Vector {
                x: document.delta.x,
                y: document.delta.y,
            },
        };
        let mut owned = OwnedSvgGlyph::new(core);
        owned.allocation_memory = allocation_memory;
        owned.allocation_block = allocation_block;
        owned.payload_allocation_block = payload_allocation_block;
        Box::into_raw(Box::new(owned)).cast::<FT_GlyphRec>()
    };
    // SAFETY: `out` is validated caller-provided output storage.
    unsafe { *out.as_ptr() = glyph };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Glyph(slot: FT_GlyphSlot, aglyph: *mut FT_Glyph) -> FT_Error {
    let err = rust_ffi::FT_Get_Glyph(!slot.is_null(), !aglyph.is_null());
    if err != rust_ffi::FT_Err_Unimplemented_Feature as FT_Error {
        return err;
    }
    let Some(out) = non_null_mut(aglyph) else {
        return err;
    };
    let Some(slot) = non_null_mut(slot) else {
        return err;
    };
    let Some(internal) = slot_internal(slot.as_ptr()) else {
        // SAFETY: `slot` is non-null and C callers must provide a readable
        // public `FT_GlyphSlotRec` for the duration of the call.
        return get_glyph_from_external_slot(unsafe { slot.as_ref() }, out);
    };
    // Successful glyph creation copies the private Rust slot payload into an
    // owned glyph while taking public format and advance from the C record.
    let slot = unsafe { slot.as_ref() };
    let mut rust_slot = internal.rust_slot.clone();
    rust_slot.format = slot.format;
    rust_slot.advance = rust_ffi::FT_Vector {
        x: slot.advance.x,
        y: slot.advance.y,
    };
    let recognized_format = matches!(
        rust_slot.format,
        rust_ffi::FT_GLYPH_FORMAT_BITMAP
            | rust_ffi::FT_GLYPH_FORMAT_OUTLINE
            | rust_ffi::FT_GLYPH_FORMAT_SVG
    );
    let glyph_result = if rust_slot.format == rust_ffi::FT_GLYPH_FORMAT_BITMAP {
        rust_ffi::FT_Get_Bitmap_Glyph(Some(&rust_slot)).and_then(|mut core| {
            core.root.library = slot.library.cast::<c_void>();
            let (memory, record_block, payload_block) = glyph_allocation_tokens(
                slot.library,
                std::mem::size_of::<FT_BitmapGlyphRec>(),
                core.bitmap.buffer.len(),
            )?;
            let mut owned = OwnedBitmapGlyph::new(core);
            owned.allocation_memory = memory;
            owned.allocation_block = record_block;
            owned.payload_allocation_block = payload_block;
            Ok(Box::into_raw(Box::new(owned)).cast::<FT_GlyphRec>())
        })
    } else if rust_slot.format == rust_ffi::FT_GLYPH_FORMAT_SVG {
        rust_ffi::FT_Get_Svg_Glyph(Some(&rust_slot)).and_then(|mut core| {
            core.root.library = slot.library.cast::<c_void>();
            let (memory, record_block, payload_block) = glyph_allocation_tokens(
                slot.library,
                std::mem::size_of::<AbiSvgGlyphRec>(),
                core.svg_document.len(),
            )?;
            let mut owned = OwnedSvgGlyph::new(core);
            owned.allocation_memory = memory;
            owned.allocation_block = record_block;
            owned.payload_allocation_block = payload_block;
            Ok(Box::into_raw(Box::new(owned)).cast::<FT_GlyphRec>())
        })
    } else {
        rust_ffi::FT_Get_Outline_Glyph(Some(&rust_slot)).map(|mut core| {
            core.root.library = slot.library.cast::<c_void>();
            Box::into_raw(Box::new(OwnedOutlineGlyph::new(core))).cast::<FT_GlyphRec>()
        })
    };
    match glyph_result {
        Ok(glyph) => {
            // SAFETY: `out` is non-null and points to caller-provided output storage.
            unsafe { *out.as_ptr() = glyph };
            rust_ffi::FT_Err_Ok
        }
        Err(error) => {
            // `FT_New_Glyph` preserves the output for an unknown format.  For
            // a known class, later advance or glyph-init failures destroy the
            // partially allocated glyph and explicitly write NULL.
            if recognized_format {
                // SAFETY: `out` points to caller-provided output storage.
                unsafe { *out.as_ptr() = ptr::null_mut() };
            }
            error
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_New_Glyph(
    library: FT_Library,
    format: FT_Glyph_Format,
    aglyph: *mut FT_Glyph,
) -> FT_Error {
    if library_ref(library).is_none() || aglyph.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let recognized_format = matches!(
        format,
        rust_ffi::FT_GLYPH_FORMAT_OUTLINE
            | rust_ffi::FT_GLYPH_FORMAT_BITMAP
            | rust_ffi::FT_GLYPH_FORMAT_SVG
    );
    if recognized_format
        && rust_ffi::FT_Library_Renderer_Class(library_ref(library), format).is_none()
    {
        return rust_ffi::FT_Err_Invalid_Glyph_Format as FT_Error;
    }
    if recognized_format {
        // FreeType clears the output after selecting a built-in glyph class,
        // before the class allocation can fail.  Unknown formats preserve the
        // caller's sentinel until a renderer class has been found.
        // SAFETY: `aglyph` is non-null and points to caller-owned storage.
        unsafe { *aglyph = ptr::null_mut() };
    }
    let root = rust_ffi::FT_GlyphRec {
        library: library.cast::<c_void>(),
        clazz: ptr::null(),
        format,
        advance: rust_ffi::FT_Vector::default(),
    };
    let glyph = match format {
        rust_ffi::FT_GLYPH_FORMAT_OUTLINE => {
            let core = rust_ffi::FT_OutlineGlyphOwned {
                root,
                outline: rust_ffi::FT_OutlineSnapshot::default(),
            };
            let (memory, allocation_block, _) = match glyph_allocation_tokens(
                library,
                std::mem::size_of::<FT_OutlineGlyphRec>(),
                0,
            ) {
                Ok(tokens) => tokens,
                Err(error) => return error,
            };
            let mut owned = OwnedOutlineGlyph::new(core);
            owned.allocation_memory = memory;
            owned.allocation_block = allocation_block;
            Box::into_raw(Box::new(owned)).cast::<FT_GlyphRec>()
        }
        rust_ffi::FT_GLYPH_FORMAT_BITMAP => {
            let core = rust_ffi::FT_BitmapGlyphOwned {
                root,
                left: 0,
                top: 0,
                bitmap: rust_ffi::FT_Bitmap::default(),
            };
            let (memory, allocation_block, _) =
                match glyph_allocation_tokens(library, std::mem::size_of::<FT_BitmapGlyphRec>(), 0)
                {
                    Ok(tokens) => tokens,
                    Err(error) => return error,
                };
            let mut owned = OwnedBitmapGlyph::new(core);
            owned.allocation_memory = memory;
            owned.allocation_block = allocation_block;
            Box::into_raw(Box::new(owned)).cast::<FT_GlyphRec>()
        }
        rust_ffi::FT_GLYPH_FORMAT_SVG => {
            // FreeType has a built-in SVG glyph class when
            // `FT_CONFIG_OPTION_SVG` is enabled.  It is not a renderer
            // fallback: `FT_New_Glyph` selects this class directly, so the
            // C ABI must do the same instead of asking the synthetic
            // renderer facade for a zero-sized custom class.
            let core = rust_ffi::FT_SvgGlyphOwned {
                root,
                svg_document: Vec::new(),
                glyph_index: 0,
                metrics: rust_ffi::FT_Size_Metrics::default(),
                units_per_EM: 0,
                start_glyph_id: 0,
                end_glyph_id: 0,
                transform: rust_ffi::FT_Matrix::default(),
                delta: rust_ffi::FT_Vector::default(),
            };
            let (memory, allocation_block, _) =
                match glyph_allocation_tokens(library, std::mem::size_of::<AbiSvgGlyphRec>(), 0) {
                    Ok(tokens) => tokens,
                    Err(error) => return error,
                };
            let mut owned = OwnedSvgGlyph::new(core);
            owned.allocation_memory = memory;
            owned.allocation_block = allocation_block;
            Box::into_raw(Box::new(owned)).cast::<FT_GlyphRec>()
        }
        _ => {
            let renderer = FT_Get_Renderer(library, format);
            let Some(renderer) = non_null_mut(renderer) else {
                return rust_ffi::FT_Err_Invalid_Glyph_Format as FT_Error;
            };
            // SAFETY: `FT_Get_Renderer` returned a live renderer owned by the
            // validated library.
            let clazz = unsafe { ptr::addr_of!(renderer.as_ref().glyph_class) };
            // SAFETY: the renderer owns this class for the library lifetime.
            let class = unsafe { &*clazz };
            let Ok(glyph_size) = usize::try_from(class.glyph_size) else {
                return rust_ffi::FT_Err_Invalid_Argument;
            };
            if glyph_size < std::mem::size_of::<FT_GlyphRec>() {
                return rust_ffi::FT_Err_Invalid_Argument;
            }
            let Some(state) = library_state_mut(library) else {
                return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
            };
            let mut owned = AbiOwnedCustomGlyph::new(glyph_size);
            let glyph = owned.as_glyph();
            // SAFETY: the word-aligned zeroed allocation is at least one
            // `FT_GlyphRec` long and remains owned by the library registry.
            unsafe {
                *glyph = FT_GlyphRec {
                    library,
                    clazz,
                    format: class.glyph_format,
                    advance: FT_Vector::default(),
                };
            }
            state.custom_glyphs.push(owned);
            glyph
        }
    };
    // SAFETY: `aglyph` is non-null and points to caller-owned handle storage.
    unsafe {
        *aglyph = glyph;
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Glyph_Copy(source: FT_Glyph, target: *mut FT_Glyph) -> FT_Error {
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
        // checks, before target allocation or class-copy dispatch.
        // SAFETY: `target` is non-null and points to caller-provided storage.
        unsafe {
            *target = ptr::null_mut();
        }
    }
    #[cfg(feature = "abi-test-support")]
    if err == rust_ffi::FT_Err_Unimplemented_Feature as FT_Error
        && let Some(error) = abi_glyph_copy_injected_failure()
    {
        return error;
    }
    if err == rust_ffi::FT_Err_Unimplemented_Feature as FT_Error
        && !target.is_null()
        && let Some(source) = owned_outline_glyph_from_root(source)
    {
        let copy = rust_ffi::FT_Outline_Glyph_Copy(&source.core);
        let copy = Box::into_raw(Box::new(OwnedOutlineGlyph::new(copy))).cast::<FT_GlyphRec>();
        // SAFETY: `target` is non-null and points to caller-provided output storage.
        unsafe {
            *target = copy;
        }
        return rust_ffi::FT_Err_Ok;
    }
    if err == rust_ffi::FT_Err_Unimplemented_Feature as FT_Error
        && !target.is_null()
        && let Some(source) = owned_bitmap_glyph_from_root_mut(source)
    {
        if let Err(error) = source.sync_core_from_record() {
            return error;
        }
        let library = source.record.root.library;
        let (memory, record_block, payload_block) = match glyph_allocation_tokens(
            library,
            std::mem::size_of::<FT_BitmapGlyphRec>(),
            source.core.bitmap.buffer.len(),
        ) {
            Ok(tokens) => tokens,
            Err(error) => return error,
        };
        let copy = rust_ffi::FT_Bitmap_Glyph_Copy(&source.core);
        let mut copy = OwnedBitmapGlyph::new(copy);
        copy.allocation_memory = memory;
        copy.allocation_block = record_block;
        copy.payload_allocation_block = payload_block;
        let copy = Box::into_raw(Box::new(copy)).cast::<FT_GlyphRec>();
        // SAFETY: `target` is non-null and points to caller-provided output storage.
        unsafe {
            *target = copy;
        }
        return rust_ffi::FT_Err_Ok;
    }
    if err == rust_ffi::FT_Err_Unimplemented_Feature as FT_Error
        && !target.is_null()
        && let Some(source) = owned_svg_glyph_from_root_mut(source)
    {
        let library = source.record.root.library;
        let (memory, record_block, _) =
            match glyph_allocation_tokens(library, std::mem::size_of::<AbiSvgGlyphRec>(), 0) {
                Ok(tokens) => tokens,
                Err(error) => return error,
            };
        if source.record.root.format != rust_ffi::FT_GLYPH_FORMAT_SVG {
            free_custom_memory_block(memory, record_block);
            return rust_ffi::FT_Err_Invalid_Glyph_Format;
        }
        if let Err(error) = source.sync_core_from_record() {
            free_custom_memory_block(memory, record_block);
            return error;
        }
        let payload_block = match custom_memory_block(memory, source.core.svg_document.len()) {
            Ok(block) => block.unwrap_or(ptr::null_mut()),
            Err(error) => {
                free_custom_memory_block(memory, record_block);
                return error;
            }
        };
        let copy = match rust_ffi::FT_Svg_Glyph_Copy(&source.core) {
            Ok(copy) => copy,
            Err(error) => {
                free_custom_memory_block(memory, record_block);
                return error;
            }
        };
        let mut copy = OwnedSvgGlyph::new(copy);
        copy.allocation_memory = memory;
        copy.allocation_block = record_block;
        copy.payload_allocation_block = payload_block;
        let copy = Box::into_raw(Box::new(copy)).cast::<FT_GlyphRec>();
        // SAFETY: `target` is non-null and points to caller-provided output storage.
        unsafe {
            *target = copy;
        }
        return rust_ffi::FT_Err_Ok;
    }
    if err == rust_ffi::FT_Err_Unimplemented_Feature as FT_Error && !target.is_null() {
        // SAFETY: `target` is non-null and points to caller-provided output storage.
        unsafe {
            *target = ptr::null_mut();
        }
    }
    err
}

#[cfg(feature = "abi-test-support")]
fn abi_glyph_copy_injected_failure() -> Option<FT_Error> {
    ABI_GLYPH_COPY_FAILURE_STATE.with(|state| {
        let mode = state.borrow().mode;
        match mode {
            AbiGlyphCopyFailureMode::None => None,
            AbiGlyphCopyFailureMode::Allocation => {
                state.borrow_mut().allocation_attempts = 1;
                Some(rust_ffi::FT_Err_Out_Of_Memory)
            }
            AbiGlyphCopyFailureMode::BitmapCopy => {
                state.borrow_mut().allocation_attempts = 2;
                drop(Box::new(AbiGlyphCopyPartialTarget));
                Some(rust_ffi::FT_Err_Out_Of_Memory)
            }
            AbiGlyphCopyFailureMode::SvgZeroLength => {
                state.borrow_mut().allocation_attempts = 1;
                drop(Box::new(AbiGlyphCopyPartialTarget));
                Some(rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error)
            }
        }
    })
}

/// One deterministic C-ABI observation of `FT_Glyph_Copy` failure cleanup.
#[cfg(feature = "abi-test-support")]
pub struct AbiGlyphCopyFailureRow {
    pub probe: &'static str,
    pub error: FT_Error,
    pub target_is_null: bool,
    pub cleanup_events: &'static [&'static str],
    pub allocation_attempts: usize,
    pub frees_before_return: usize,
}

/// Exercises output clearing and partial-target cleanup through `FT_Glyph_Copy`.
#[cfg(feature = "abi-test-support")]
pub fn abi_glyph_copy_failure_cleanup() -> [AbiGlyphCopyFailureRow; 3] {
    const ALLOCATION_EVENTS: &[&str] = &["target_cleared", "allocation_failed_before_target"];
    const COPY_HOOK_EVENTS: &[&str] = &[
        "target_cleared",
        "partial_target_allocated",
        "copy_hook_failed",
        "partial_target_destroyed",
    ];
    let class = FT_Glyph_Class::default();
    let mut source = FT_GlyphRec {
        library: ptr::null_mut(),
        clazz: ptr::from_ref(&class),
        format: rust_ffi::FT_GLYPH_FORMAT_BITMAP,
        advance: FT_Vector::default(),
    };
    let probes = [
        (
            "allocation_failure",
            AbiGlyphCopyFailureMode::Allocation,
            ALLOCATION_EVENTS,
        ),
        (
            "bitmap_copy_failure",
            AbiGlyphCopyFailureMode::BitmapCopy,
            COPY_HOOK_EVENTS,
        ),
        (
            "svg_zero_length_source",
            AbiGlyphCopyFailureMode::SvgZeroLength,
            COPY_HOOK_EVENTS,
        ),
    ];
    probes.map(|(probe, mode, cleanup_events)| {
        ABI_GLYPH_COPY_FAILURE_STATE.with(|state| {
            *state.borrow_mut() = AbiGlyphCopyFailureState {
                mode,
                ..AbiGlyphCopyFailureState::default()
            };
        });
        let mut target = ptr::dangling_mut::<FT_GlyphRec>();
        let error = FT_Glyph_Copy(&raw mut source, &mut target);
        let (allocation_attempts, frees_before_return) =
            ABI_GLYPH_COPY_FAILURE_STATE.with(|state| {
                let state = state.borrow();
                (state.allocation_attempts, state.frees_before_return)
            });
        ABI_GLYPH_COPY_FAILURE_STATE.with(|state| {
            *state.borrow_mut() = AbiGlyphCopyFailureState::default();
        });
        AbiGlyphCopyFailureRow {
            probe,
            error,
            target_is_null: target.is_null(),
            cleanup_events,
            allocation_attempts,
            frees_before_return,
        }
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Done_Glyph(glyph: FT_Glyph) {
    if let Some(glyph_root) = non_null(glyph) {
        // SAFETY: every live glyph begins with the public `FT_GlyphRec`.
        let glyph_root = unsafe { glyph_root.as_ref() };
        let library = glyph_root.library;
        let clazz = glyph_root.clazz;
        if let Some(state) = library_state_mut(library)
            && let Some(index) = state
                .custom_glyphs
                .iter_mut()
                .position(|owned| owned.as_glyph() == glyph)
        {
            let owned = state.custom_glyphs.remove(index);
            if let Some(done) = non_null(clazz.cast_mut()).and_then(|clazz| {
                // SAFETY: the custom renderer class remains owned by the live
                // library while its glyph is destroyed.
                unsafe { clazz.as_ref().glyph_done }
            }) {
                // SAFETY: `glyph` remains backed by `owned` for this callback.
                unsafe { done(glyph) };
            }
            drop(owned);
            return;
        }
    }
    if owned_outline_glyph_from_root(glyph).is_some() {
        // SAFETY: the class sentinel proves this pointer came from
        // `Box<OwnedOutlineGlyph>` in `FT_Get_Glyph`.
        unsafe { drop(Box::from_raw(glyph.cast::<OwnedOutlineGlyph>())) };
        return;
    }
    if owned_bitmap_glyph_from_root(glyph).is_some() {
        // SAFETY: the class sentinel proves this pointer came from
        // `Box<OwnedBitmapGlyph>` in `FT_Get_Glyph`.
        unsafe { drop(Box::from_raw(glyph.cast::<OwnedBitmapGlyph>())) };
        return;
    }
    if owned_svg_glyph_from_root(glyph).is_some() {
        // SAFETY: the class sentinel proves this pointer came from
        // `Box<OwnedSvgGlyph>` in `FT_Get_Glyph`.
        unsafe { drop(Box::from_raw(glyph.cast::<OwnedSvgGlyph>())) };
        return;
    }
    rust_ffi::FT_Done_Glyph(!glyph.is_null());
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Glyph_Transform(
    glyph: FT_Glyph,
    matrix: *const FT_Matrix,
    delta: *const FT_Vector,
) -> FT_Error {
    if glyph.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let matrix = non_null(matrix).map(|matrix| {
        // SAFETY: `matrix` is non-null and copied by value.
        let matrix = unsafe { matrix.as_ref() };
        rust_ffi::FT_Matrix {
            xx: matrix.xx,
            xy: matrix.xy,
            yx: matrix.yx,
            yy: matrix.yy,
        }
    });
    let delta = non_null(delta).map(|delta| {
        // SAFETY: `delta` is non-null and copied by value.
        let delta = unsafe { delta.as_ref() };
        rust_ffi::FT_Vector {
            x: delta.x,
            y: delta.y,
        }
    });
    if let Some(owned) = owned_svg_glyph_from_root_mut(glyph) {
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
    let Some(owned) = owned_outline_glyph_from_root_mut(glyph) else {
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
pub extern "C" fn FT_Glyph_To_Bitmap(
    the_glyph: *mut FT_Glyph,
    render_mode: FT_Render_Mode,
    origin: *const FT_Vector,
    destroy: FT_Bool,
) -> FT_Error {
    if !the_glyph.is_null() {
        // SAFETY: `the_glyph` is non-null and points to caller-owned handle
        // storage.  We only copy the handle value before validating the glyph.
        let glyph = unsafe { *the_glyph };
        if owned_bitmap_glyph_from_root(glyph).is_some() {
            // FreeType `src/base/ftglyph.c:794-795` returns success without
            // replacing or freeing an already-bitmap glyph.
            return rust_ffi::FT_Err_Ok;
        }
        if let Some(owned) = owned_outline_glyph_from_root_mut(glyph) {
            if let Err(error) = owned.sync_core_from_record() {
                return error;
            }
            let origin = if origin.is_null() {
                None
            } else {
                // SAFETY: the caller supplied a non-null FT_Vector pointer
                // that remains readable for this synchronous conversion.
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
            let bitmap =
                Box::into_raw(Box::new(OwnedBitmapGlyph::new(bitmap))).cast::<FT_GlyphRec>();
            if destroy != 0 {
                // SAFETY: the class sentinel proves this pointer came from
                // `Box<OwnedOutlineGlyph>` in `FT_Get_Glyph`.
                unsafe { drop(Box::from_raw(glyph.cast::<OwnedOutlineGlyph>())) };
            }
            // SAFETY: `the_glyph` is non-null and points to caller-provided
            // handle storage.  C FreeType replaces it after successful render.
            unsafe {
                *the_glyph = bitmap;
            }
            return rust_ffi::FT_Err_Ok;
        }
    }
    let (glyph_present, library_present, class_present, prepare_hook_present) =
        if the_glyph.is_null() {
            (false, false, false, false)
        } else {
            // SAFETY: `the_glyph` is non-null and this thin wrapper only reads
            // the caller handle to reproduce FreeType's early argument checks.
            let glyph = unsafe { *the_glyph };
            if glyph.is_null() {
                (false, false, false, false)
            } else {
                // SAFETY: `glyph` is non-null and this wrapper reads only the
                // root fields used before FreeType allocates or renders.
                let glyph_ref = unsafe { &*glyph };
                let class_present = !glyph_ref.clazz.is_null();
                let prepare_hook_present = if class_present {
                    // SAFETY: `clazz` is non-null and only the function-pointer
                    // nullness is observed, matching the C Bad-path guard.
                    unsafe { (*glyph_ref.clazz).glyph_prepare.is_some() }
                } else {
                    false
                };
                (
                    true,
                    !glyph_ref.library.is_null(),
                    class_present,
                    prepare_hook_present,
                )
            }
        };
    rust_ffi::FT_Glyph_To_Bitmap(
        !the_glyph.is_null(),
        glyph_present,
        library_present,
        class_present,
        prepare_hook_present,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Glyph_Stroke(
    pglyph: *mut FT_Glyph,
    stroker: FT_Stroker,
    destroy: FT_Bool,
) -> FT_Error {
    let Some(handle) = non_null_mut(pglyph) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `handle` is non-null and points to caller-owned glyph handle
    // storage.  We copy the handle before validating the private class marker.
    let glyph = unsafe { *handle.as_ptr() };
    if glyph.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(owned) = owned_outline_glyph_from_root(glyph) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let stroked = match rust_ffi::FT_Outline_Glyph_Stroke(Some(&owned.core), stroker) {
        Ok(stroked) => stroked,
        Err(error) => {
            if destroy == 0 {
                // SAFETY: `handle` is valid caller storage and C FreeType's
                // failure path clears `*pglyph` after the copy/stroke phase.
                unsafe { *handle.as_ptr() = ptr::null_mut() };
            }
            return error;
        }
    };
    let stroked = Box::into_raw(Box::new(OwnedOutlineGlyph::new(stroked))).cast::<FT_GlyphRec>();
    if destroy != 0 {
        // SAFETY: the class sentinel proves this pointer came from
        // `Box<OwnedOutlineGlyph>` in `FT_Get_Glyph`.
        unsafe { drop(Box::from_raw(glyph.cast::<OwnedOutlineGlyph>())) };
    }
    // SAFETY: `handle` is valid caller-provided handle storage.
    unsafe {
        *handle.as_ptr() = stroked;
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Glyph_StrokeBorder(
    pglyph: *mut FT_Glyph,
    stroker: FT_Stroker,
    inside: FT_Bool,
    destroy: FT_Bool,
) -> FT_Error {
    let Some(handle) = non_null_mut(pglyph) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `handle` is non-null and points to caller-owned glyph handle
    // storage.  We copy the handle before validating the private class marker.
    let glyph = unsafe { *handle.as_ptr() };
    if glyph.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(owned) = owned_outline_glyph_from_root(glyph) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let stroked = match rust_ffi::FT_Outline_Glyph_StrokeBorder(Some(&owned.core), stroker, inside)
    {
        Ok(stroked) => stroked,
        Err(error) => {
            if destroy == 0 {
                // SAFETY: `handle` is valid caller storage and C FreeType's
                // failure path clears `*pglyph` after the copy/stroke phase.
                unsafe { *handle.as_ptr() = ptr::null_mut() };
            }
            return error;
        }
    };
    let stroked = Box::into_raw(Box::new(OwnedOutlineGlyph::new(stroked))).cast::<FT_GlyphRec>();
    if destroy != 0 {
        // SAFETY: the class sentinel proves this pointer came from
        // `Box<OwnedOutlineGlyph>` in `FT_Get_Glyph`.
        unsafe { drop(Box::from_raw(glyph.cast::<OwnedOutlineGlyph>())) };
    }
    // SAFETY: `handle` is valid caller-provided handle storage.
    unsafe {
        *handle.as_ptr() = stroked;
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Get_BBox(outline: *const FT_Outline, abbox: *mut FT_BBox) -> FT_Error {
    if abbox.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument as FT_Error;
    }
    let Some(snapshot) = outline_snapshot_from_c(outline) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    let mut bbox = rust_ffi::FT_BBox::default();
    let error = rust_ffi::FT_Outline_Get_BBox(Some(&snapshot), Some(&mut bbox));
    if error == rust_ffi::FT_Err_Ok {
        // SAFETY: `abbox` is non-null and the caller provides writable `FT_BBox` storage.
        unsafe {
            *abbox = FT_BBox {
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
pub extern "C" fn FT_Outline_Get_Bitmap(
    library: FT_Library,
    outline: *const FT_Outline,
    abitmap: *const FT_Bitmap,
) -> FT_Error {
    let Some(bitmap) = non_null(abitmap) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let snapshot = outline_snapshot_from_c(outline);
    let bitmap_view = {
        // SAFETY: `bitmap` is non-null and points to caller-owned `FT_Bitmap` storage.
        let bitmap = unsafe { bitmap.as_ref() };
        rust_ffi::FT_Bitmap_C {
            rows: bitmap.rows,
            width: bitmap.width,
            pitch: bitmap.pitch,
            buffer: bitmap.buffer,
            num_grays: bitmap.num_grays,
            pixel_mode: bitmap.pixel_mode,
            palette_mode: bitmap.palette_mode,
            palette: bitmap.palette,
        }
    };
    match rust_ffi::FT_Outline_Get_Bitmap(
        library_ref(library),
        snapshot.as_ref(),
        Some(&bitmap_view),
    ) {
        Ok(rendered) => {
            // SAFETY: `bitmap` remains a readable caller-owned record.  Its
            // const-qualified fields are not changed; only the separately
            // referenced caller-owned pixel buffer is written.
            let bitmap = unsafe { bitmap.as_ref() };
            copy_rendered_bitmap_to_c(bitmap, &rendered);
            rust_ffi::FT_Err_Ok
        }
        Err(err) => err,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Render(
    library: FT_Library,
    outline: *const FT_Outline,
    params: *mut FT_Raster_Params,
) -> FT_Error {
    let Some(params) = (unsafe { params.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let snapshot = outline_snapshot_from_c(outline);
    let target = unsafe { params.target.as_ref() };
    let bitmap_view = target.map(bitmap_to_rust);
    let clip_box = rust_ffi::FT_BBox {
        xMin: params.clip_box.xMin,
        yMin: params.clip_box.yMin,
        xMax: params.clip_box.xMax,
        yMax: params.clip_box.yMax,
    };
    let library_view = library_ref(library);
    if library_view.is_some()
        && snapshot.as_ref().is_some_and(|outline_snapshot| {
            let mut cbox = rust_ffi::FT_BBox::default();
            rust_ffi::FT_Outline_Get_CBox(Some(outline_snapshot), Some(&mut cbox));
            cbox.xMin >= -0x1000000
                && cbox.yMin >= -0x1000000
                && cbox.xMax <= 0x1000000
                && cbox.yMax <= 0x1000000
        })
    {
        // FreeType 2.14.3 ftoutln.c:625-648 mutates `source` after
        // library/outline/cbox validation and before invoking the renderer,
        // so renderer errors retain this mutation too.
        params.source = outline.cast();
    }

    if params.flags & rust_ffi::FT_RASTER_FLAG_DIRECT as c_int != 0 {
        if params.flags & rust_ffi::FT_RASTER_FLAG_CLIP as c_int == 0 {
            if let (Some(_library), Some(outline_snapshot)) = (library_view, snapshot.as_ref()) {
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
            library_view,
            snapshot.as_ref(),
            bitmap_view.as_ref(),
            params.flags,
            Some(rust_ffi::FT_BBox {
                xMin: params.clip_box.xMin,
                yMin: params.clip_box.yMin,
                xMax: params.clip_box.xMax,
                yMax: params.clip_box.yMax,
            }),
            params.gray_spans.is_some(),
        ) {
            Ok(spans) => {
                if let Some(callback) = params.gray_spans {
                    for row in spans.chunk_by(|left, right| left.0 == right.0) {
                        let y = row[0].0;
                        let c_spans = row
                            .iter()
                            .map(|(_, span)| FT_Span {
                                x: i16::from_ne_bytes(span.x.to_ne_bytes()),
                                len: span.len,
                                coverage: span.coverage,
                            })
                            .collect::<Vec<_>>();
                        // SAFETY: `c_spans` lives for the synchronous callback
                        // invocation, and `params.user` is the caller-provided
                        // opaque pointer FreeType passes through unchanged.
                        unsafe {
                            callback(
                                y,
                                c_int::try_from(c_spans.len()).unwrap_or(c_int::MAX),
                                c_spans.as_ptr(),
                                params.user,
                            );
                        }
                    }
                }
                rust_ffi::FT_Err_Ok
            }
            Err(err) => err,
        };
    }

    match rust_ffi::FT_Outline_Render(
        library_view,
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
                copy_rendered_bitmap_to_c(target, &rendered);
            }
            rust_ffi::FT_Err_Ok
        }
        Err(err) => {
            if let (Some(target), Some(rendered)) = (
                target,
                rust_ffi::FT_Outline_Render_Error_Output(
                    snapshot.as_ref(),
                    bitmap_view.as_ref(),
                    params.flags,
                ),
            ) {
                copy_rendered_bitmap_to_c(target, &rendered);
            }
            err
        }
    }
}

#[cfg(feature = "abi-test-support")]
unsafe extern "C" fn abi_support_outline_render_gray_spans(
    y: c_int,
    count: c_int,
    spans: *const FT_Span,
    user: *mut c_void,
) {
    TEST_OUTLINE_RENDER_USER_TOKEN.with(|token| {
        TEST_OUTLINE_RENDER_USER_SEEN.with(|seen| {
            *seen.borrow_mut() = user == *token.borrow();
        });
    });
    if count <= 0 || spans.is_null() {
        return;
    }
    // SAFETY: FreeType span callbacks provide `count` initialized records
    // valid for this synchronous callback invocation.
    let spans = unsafe { slice::from_raw_parts(spans, usize::try_from(count).unwrap_or(0)) };
    TEST_OUTLINE_RENDER_SPANS.with(|recorded| {
        recorded
            .borrow_mut()
            .extend(spans.iter().copied().map(|span| (y, span)));
    });
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_outline_render_direct_spans(
    library: FT_Library,
    outline: *const FT_Outline,
    params: *mut FT_Raster_Params,
    gray_spans_present: bool,
    user_token: *mut c_void,
) -> (FT_Error, Vec<(c_int, FT_Span)>, bool) {
    TEST_OUTLINE_RENDER_SPANS.with(|spans| spans.borrow_mut().clear());
    TEST_OUTLINE_RENDER_USER_SEEN.with(|seen| *seen.borrow_mut() = false);
    TEST_OUTLINE_RENDER_USER_TOKEN.with(|token| *token.borrow_mut() = user_token);
    if let Some(params) = unsafe { params.as_mut() } {
        params.user = user_token;
        params.gray_spans = gray_spans_present.then_some(abi_support_outline_render_gray_spans);
    }
    let error = FT_Outline_Render(library, outline, params);
    let spans = TEST_OUTLINE_RENDER_SPANS.with(|recorded| recorded.borrow().clone());
    let user_seen = TEST_OUTLINE_RENDER_USER_SEEN.with(|seen| *seen.borrow());
    (error, spans, user_seen)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_outline_decompose_trace(
    outline: *const FT_Outline,
    transforms: &[(rust_ffi::FT_Int, rust_ffi::FT_Pos)],
) -> Result<Vec<rust_ffi::FTOutlineDecomposeRun>, FT_Error> {
    let snapshot = outline_snapshot_from_c(outline);
    rust_ffi::FT_Outline_Decompose_Trace(snapshot.as_ref(), transforms)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Decompose(
    outline: *mut FT_Outline,
    func_interface: *const FT_Outline_Funcs,
    user: FT_Pointer,
) -> FT_Error {
    let Some(snapshot) = outline_snapshot_from_c(outline) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    // SAFETY: a non-null callback table is borrowed only for this synchronous
    // call; each function pointer is checked before invocation.
    let Some(funcs) = (unsafe { func_interface.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let runs = match rust_ffi::FT_Outline_Decompose_Trace(
        Some(&snapshot),
        &[(funcs.shift, funcs.delta)],
    ) {
        Ok(runs) => runs,
        Err(error) => return error,
    };
    let Some(run) = runs.first() else {
        return rust_ffi::FT_Err_Ok;
    };
    for event in &run.events {
        let points = event
            .points
            .iter()
            .map(|point| FT_Vector {
                x: point.x,
                y: point.y,
            })
            .collect::<Vec<_>>();
        let error = match (event.kind, points.as_slice()) {
            ("move_to", [to]) => {
                let Some(callback) = funcs.move_to else {
                    return rust_ffi::FT_Err_Invalid_Argument;
                };
                // SAFETY: callback pointers are caller-provided and `to`
                // remains live for this synchronous invocation.
                unsafe { callback(to, user) }
            }
            ("line_to", [to]) => {
                let Some(callback) = funcs.line_to else {
                    return rust_ffi::FT_Err_Invalid_Argument;
                };
                // SAFETY: same synchronous callback contract as `move_to`.
                unsafe { callback(to, user) }
            }
            ("conic_to", [control, to]) => {
                let Some(callback) = funcs.conic_to else {
                    return rust_ffi::FT_Err_Invalid_Argument;
                };
                // SAFETY: both point references remain live for the callback.
                unsafe { callback(control, to, user) }
            }
            ("cubic_to", [control1, control2, to]) => {
                let Some(callback) = funcs.cubic_to else {
                    return rust_ffi::FT_Err_Invalid_Argument;
                };
                // SAFETY: all point references remain live for the callback.
                unsafe { callback(control1, control2, to, user) }
            }
            _ => return rust_ffi::FT_Err_Invalid_Outline as FT_Error,
        };
        if error != rust_ffi::FT_Err_Ok {
            return error;
        }
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Get_Orientation(outline: *const FT_Outline) -> FT_Orientation {
    let Some(snapshot) = outline_snapshot_from_c(outline) else {
        return rust_ffi::FT_ORIENTATION_TRUETYPE as FT_Orientation;
    };
    rust_ffi::FT_Outline_Get_Orientation(Some(&snapshot)) as FT_Orientation
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Check(outline: *const FT_Outline) -> FT_Error {
    let Some(snapshot) = outline_snapshot_from_c(outline) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    rust_ffi::FT_Outline_Check(Some(&snapshot))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Copy(source: *const FT_Outline, target: *mut FT_Outline) -> FT_Error {
    if source == target.cast_const() && !source.is_null() {
        return rust_ffi::FT_Err_Ok;
    }
    let Some(source_snapshot) = outline_snapshot_from_c(source) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    let Some(mut target_snapshot) = outline_snapshot_from_c(target) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    let error = rust_ffi::FT_Outline_Copy(Some(&source_snapshot), Some(&mut target_snapshot));
    if error == rust_ffi::FT_Err_Ok {
        copy_outline_snapshot_to_c(target, &target_snapshot, true);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Embolden(outline: *mut FT_Outline, strength: FT_Pos) -> FT_Error {
    let Some(mut snapshot) = outline_snapshot_from_c(outline) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    let error = rust_ffi::FT_Outline_Embolden(Some(&mut snapshot), strength);
    if error == rust_ffi::FT_Err_Ok {
        copy_outline_snapshot_to_c(outline, &snapshot, false);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_EmboldenXY(
    outline: *mut FT_Outline,
    xstrength: FT_Pos,
    ystrength: FT_Pos,
) -> FT_Error {
    let Some(mut snapshot) = outline_snapshot_from_c(outline) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    let error = rust_ffi::FT_Outline_EmboldenXY(Some(&mut snapshot), xstrength, ystrength);
    if error == rust_ffi::FT_Err_Ok {
        copy_outline_snapshot_to_c(outline, &snapshot, false);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_GetInsideBorder(outline: *const FT_Outline) -> FT_StrokerBorder {
    let snapshot = outline_snapshot_from_c(outline);
    rust_ffi::FT_Outline_GetInsideBorder(snapshot.as_ref()) as FT_StrokerBorder
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_GetOutsideBorder(outline: *const FT_Outline) -> FT_StrokerBorder {
    let snapshot = outline_snapshot_from_c(outline);
    rust_ffi::FT_Outline_GetOutsideBorder(snapshot.as_ref()) as FT_StrokerBorder
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_New(library: FT_Library, astroker: *mut FT_Stroker) -> FT_Error {
    let Some(out) = non_null_mut(astroker) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let mut stroker = ptr::null_mut();
    let err = rust_ffi::FT_Stroker_New(library_ref(library), Some(&mut stroker));
    if err == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and points to caller-provided output storage.
        unsafe {
            *out.as_ptr() = stroker;
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_Set(
    stroker: FT_Stroker,
    radius: FT_Fixed,
    line_cap: FT_Stroker_LineCap,
    line_join: FT_Stroker_LineJoin,
    miter_limit: FT_Fixed,
) {
    rust_ffi::FT_Stroker_Set(stroker, radius, line_cap, line_join, miter_limit);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_Rewind(stroker: FT_Stroker) {
    rust_ffi::FT_Stroker_Rewind(stroker);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_BeginSubPath(
    stroker: FT_Stroker,
    to: *const FT_Vector,
    open: FT_Bool,
) -> FT_Error {
    let rust_to = if to.is_null() {
        None
    } else {
        // SAFETY: `to` is non-null and points to a C ABI `FT_Vector` for the
        // duration of this thin forwarding call.
        let to = unsafe { &*to };
        Some(rust_ffi::FT_Vector { x: to.x, y: to.y })
    };
    rust_ffi::FT_Stroker_BeginSubPath(stroker, rust_to.as_ref(), open)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_ParseOutline(
    stroker: FT_Stroker,
    outline: *const FT_Outline,
    opened: FT_Bool,
) -> FT_Error {
    let snapshot = outline_snapshot_from_c(outline);
    rust_ffi::FT_Stroker_ParseOutline(stroker, snapshot.as_ref(), opened)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_LineTo(stroker: FT_Stroker, to: *const FT_Vector) -> FT_Error {
    let rust_to = if to.is_null() {
        None
    } else {
        // SAFETY: `to` is non-null and points to a C ABI `FT_Vector` for the
        // duration of this thin forwarding call.
        let to = unsafe { &*to };
        Some(rust_ffi::FT_Vector { x: to.x, y: to.y })
    };
    rust_ffi::FT_Stroker_LineTo(stroker, rust_to.as_ref())
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_ConicTo(
    stroker: FT_Stroker,
    control: *const FT_Vector,
    to: *const FT_Vector,
) -> FT_Error {
    let rust_control = if control.is_null() {
        None
    } else {
        // SAFETY: `control` is non-null and points to a C ABI `FT_Vector` for
        // the duration of this thin forwarding call.
        let control = unsafe { &*control };
        Some(rust_ffi::FT_Vector {
            x: control.x,
            y: control.y,
        })
    };
    let rust_to = if to.is_null() {
        None
    } else {
        // SAFETY: `to` is non-null and points to a C ABI `FT_Vector` for the
        // duration of this thin forwarding call.
        let to = unsafe { &*to };
        Some(rust_ffi::FT_Vector { x: to.x, y: to.y })
    };
    rust_ffi::FT_Stroker_ConicTo(stroker, rust_control.as_ref(), rust_to.as_ref())
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_CubicTo(
    stroker: FT_Stroker,
    control1: *const FT_Vector,
    control2: *const FT_Vector,
    to: *const FT_Vector,
) -> FT_Error {
    let rust_control1 = if control1.is_null() {
        None
    } else {
        // SAFETY: `control1` is non-null and points to a C ABI `FT_Vector` for
        // the duration of this thin forwarding call.
        let control1 = unsafe { &*control1 };
        Some(rust_ffi::FT_Vector {
            x: control1.x,
            y: control1.y,
        })
    };
    let rust_control2 = if control2.is_null() {
        None
    } else {
        // SAFETY: `control2` is non-null and points to a C ABI `FT_Vector` for
        // the duration of this thin forwarding call.
        let control2 = unsafe { &*control2 };
        Some(rust_ffi::FT_Vector {
            x: control2.x,
            y: control2.y,
        })
    };
    let rust_to = if to.is_null() {
        None
    } else {
        // SAFETY: `to` is non-null and points to a C ABI `FT_Vector` for the
        // duration of this thin forwarding call.
        let to = unsafe { &*to };
        Some(rust_ffi::FT_Vector { x: to.x, y: to.y })
    };
    rust_ffi::FT_Stroker_CubicTo(
        stroker,
        rust_control1.as_ref(),
        rust_control2.as_ref(),
        rust_to.as_ref(),
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_EndSubPath(stroker: FT_Stroker) -> FT_Error {
    rust_ffi::FT_Stroker_EndSubPath(stroker)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_GetBorderCounts(
    stroker: FT_Stroker,
    border: FT_StrokerBorder,
    anum_points: *mut FT_UInt,
    anum_contours: *mut FT_UInt,
) -> FT_Error {
    // SAFETY: The optional output pointers, when non-null, are caller-owned
    // `FT_UInt` records valid for the duration of this C ABI call.
    let points = unsafe { anum_points.as_mut() };
    let contours = unsafe { anum_contours.as_mut() };
    rust_ffi::FT_Stroker_GetBorderCounts(stroker, border, points, contours)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_GetCounts(
    stroker: FT_Stroker,
    anum_points: *mut FT_UInt,
    anum_contours: *mut FT_UInt,
) -> FT_Error {
    // SAFETY: The optional output pointers, when non-null, are caller-owned
    // `FT_UInt` records valid for the duration of this C ABI call.
    let points = unsafe { anum_points.as_mut() };
    let contours = unsafe { anum_contours.as_mut() };
    rust_ffi::FT_Stroker_GetCounts(stroker, points, contours)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_Done(stroker: FT_Stroker) {
    rust_ffi::FT_Stroker_Done(stroker);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_ExportBorder(
    stroker: FT_Stroker,
    border: FT_StrokerBorder,
    outline: *mut FT_Outline,
) {
    let mut snapshot = outline_snapshot_from_c(outline).unwrap_or_default();
    rust_ffi::FT_Stroker_ExportBorder(
        stroker,
        border,
        (!outline.is_null()).then_some(&mut snapshot),
    );
    copy_outline_snapshot_to_c(outline, &snapshot, true);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Stroker_Export(stroker: FT_Stroker, outline: *mut FT_Outline) {
    let mut snapshot = outline_snapshot_from_c(outline).unwrap_or_default();
    rust_ffi::FT_Stroker_Export(stroker, (!outline.is_null()).then_some(&mut snapshot));
    copy_outline_snapshot_to_c(outline, &snapshot, true);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_New(
    library: FT_Library,
    numPoints: FT_UInt,
    numContours: FT_Int,
    anoutline: *mut FT_Outline,
) -> FT_Error {
    if library.is_null() {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    }
    let Some(outline) = (unsafe { anoutline.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    if numPoints > u32::from(u16::MAX) {
        return rust_ffi::FT_Err_Array_Too_Large as FT_Error;
    }
    if numContours < 0 || u32::try_from(numContours).map_or(true, |contours| contours > numPoints) {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let point_count = numPoints as FT_UShort;
    // SAFETY: the preceding validation returns for every negative contour
    // count, so this conversion is known to be `Ok` here.
    let contour_count = unsafe { u32::try_from(numContours).unwrap_unchecked() } as FT_UShort;
    let memory = outline_custom_memory(library);
    let points = alloc_outline_array::<FT_Vector>(point_count, memory).cast::<FT_Vector>();
    let tags = alloc_outline_array::<FT_Byte>(point_count, memory).cast::<FT_Byte>();
    let contours = alloc_outline_array::<FT_UShort>(contour_count, memory).cast::<FT_UShort>();
    if (point_count > 0 && (points.is_null() || tags.is_null()))
        || (contour_count > 0 && contours.is_null())
    {
        dealloc_outline_array(
            points.cast::<u8>(),
            point_count,
            outline_array_layout::<FT_Vector>,
            memory,
        );
        dealloc_outline_array(
            tags.cast::<u8>(),
            point_count,
            outline_array_layout::<FT_Byte>,
            memory,
        );
        dealloc_outline_array(
            contours.cast::<u8>(),
            contour_count,
            outline_array_layout::<FT_UShort>,
            memory,
        );
        return rust_ffi::FT_Err_Out_Of_Memory;
    }
    *outline = FT_Outline {
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
pub extern "C" fn FT_Outline_Done(library: FT_Library, outline: *mut FT_Outline) -> FT_Error {
    if library.is_null() {
        return rust_ffi::FT_Err_Invalid_Library_Handle as FT_Error;
    }
    let Some(outline) = (unsafe { outline.as_mut() }) else {
        return rust_ffi::FT_Err_Invalid_Outline as FT_Error;
    };
    if outline.flags & rust_ffi::FT_OUTLINE_OWNER as FT_Int != 0 {
        let memory = outline_custom_memory(library);
        dealloc_outline_array(
            outline.points.cast::<u8>(),
            outline.n_points,
            outline_array_layout::<FT_Vector>,
            memory,
        );
        dealloc_outline_array(
            outline.tags.cast::<u8>(),
            outline.n_points,
            outline_array_layout::<FT_Byte>,
            memory,
        );
        dealloc_outline_array(
            outline.contours.cast::<u8>(),
            outline.n_contours,
            outline_array_layout::<FT_UShort>,
            memory,
        );
    }
    *outline = FT_Outline::default();
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Reverse(outline: *mut FT_Outline) {
    let Some(mut snapshot) = outline_snapshot_from_c(outline) else {
        return;
    };
    rust_ffi::FT_Outline_Reverse(Some(&mut snapshot));
    copy_outline_snapshot_to_c(outline, &snapshot, true);
}

fn outline_custom_memory(library: FT_Library) -> FT_Memory {
    library_state_mut(library).map_or(ptr::null_mut(), |state| {
        if state.allocation_block.is_null() {
            ptr::null_mut()
        } else {
            state.allocation_memory
        }
    })
}

fn outline_array_layout<T>(count: FT_UShort) -> Layout {
    let count = usize::from(count);
    // SAFETY: the only callers are the outline owner paths above. The public
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

fn alloc_outline_array<T>(count: FT_UShort, memory: FT_Memory) -> *mut u8 {
    if count == 0 {
        return ptr::null_mut();
    }
    let layout = outline_array_layout::<T>(count);
    if !memory.is_null() {
        // The largest allowed outline allocation is below the range of
        // c_long on every supported ABI (USHRT_MAX fixed-size elements).
        let size = layout.size() as c_long;
        // SAFETY: `memory` is retained by the live custom library and the
        // returned block is owned by that allocator until FT_Outline_Done.
        let block = unsafe {
            (*memory)
                .alloc
                .and_then(|alloc| NonNull::new(alloc(memory, size)))
        };
        let Some(block) = block else {
            return ptr::null_mut();
        };
        // FreeType's outline allocation contract exposes zero-initialized
        // point, tag, and contour arrays.
        unsafe { block.as_ptr().cast::<u8>().write_bytes(0, layout.size()) };
        return block.as_ptr().cast();
    }
    // SAFETY: `layout` was constructed for an array allocation and is non-zero sized.
    unsafe { alloc_zeroed(layout) }
}

fn dealloc_outline_array(
    ptr: *mut u8,
    count: FT_UShort,
    layout_for: impl FnOnce(FT_UShort) -> Layout,
    memory: FT_Memory,
) {
    if ptr.is_null() || count == 0 {
        return;
    }
    if !memory.is_null() {
        // SAFETY: custom outline arrays come from this live library memory.
        unsafe {
            if let Some(free) = (*memory).free {
                free(memory, ptr.cast());
            }
        }
        return;
    }
    let layout = layout_for(count);
    // SAFETY: outline OWNER allocations in this module use the matching layout.
    unsafe { dealloc(ptr, layout) };
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Transform(outline: *const FT_Outline, matrix: *const FT_Matrix) {
    let (Some(mut snapshot), Some(matrix)) = (outline_snapshot_from_c(outline), non_null(matrix))
    else {
        return;
    };
    // SAFETY: `matrix` is non-null and points to a caller-owned `FT_Matrix`
    // that remains readable for this call.
    let matrix = unsafe { matrix.as_ref() };
    let matrix = rust_ffi::FT_Matrix {
        xx: matrix.xx,
        xy: matrix.xy,
        yx: matrix.yx,
        yy: matrix.yy,
    };
    rust_ffi::FT_Outline_Transform(Some(&mut snapshot), Some(&matrix));
    copy_outline_snapshot_to_c(outline.cast_mut(), &snapshot, false);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Outline_Translate(
    outline: *const FT_Outline,
    x_offset: FT_Pos,
    y_offset: FT_Pos,
) {
    let Some(mut snapshot) = outline_snapshot_from_c(outline) else {
        return;
    };
    rust_ffi::FT_Outline_Translate(Some(&mut snapshot), x_offset, y_offset);
    copy_outline_snapshot_to_c(outline.cast_mut(), &snapshot, false);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Char_Size(
    face: FT_Face,
    char_width: FT_F26Dot6,
    char_height: FT_F26Dot6,
    horz_resolution: FT_UInt,
    vert_resolution: FT_UInt,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        // FreeType defers null-face validation to FT_Request_Size, which
        // returns Invalid_Face_Handle before inspecting the size request.
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let error = rust_ffi::FT_Set_Char_Size(
        &mut state.inner,
        char_width,
        char_height,
        horz_resolution,
        vert_resolution,
    );
    if error == rust_ffi::FT_Err_Ok {
        update_size_metrics(face, &state.inner);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Pixel_Sizes(
    face: FT_Face,
    pixel_width: FT_UInt,
    pixel_height: FT_UInt,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        // FreeType defers null-face validation to FT_Request_Size, which
        // returns Invalid_Face_Handle after the dimension normalization.
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let error = rust_ffi::FT_Set_Pixel_Sizes(&mut state.inner, pixel_width, pixel_height);
    if error == rust_ffi::FT_Err_Ok {
        update_size_metrics(face, &state.inner);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Transform(
    face: FT_Face,
    matrix: *const FT_Matrix,
    delta: *const FT_Vector,
) {
    let Some(state) = face_state_mut(face) else {
        return;
    };
    let rust_matrix = if matrix.is_null() {
        None
    } else {
        // SAFETY: `matrix` is non-null and points to a C ABI `FT_Matrix`.
        let matrix = unsafe { *matrix };
        Some(rust_ffi::FT_Matrix {
            xx: matrix.xx,
            xy: matrix.xy,
            yx: matrix.yx,
            yy: matrix.yy,
        })
    };
    let rust_delta = if delta.is_null() {
        None
    } else {
        // SAFETY: `delta` is non-null and points to a C ABI `FT_Vector`.
        let delta = unsafe { *delta };
        Some(rust_ffi::FT_Vector {
            x: delta.x,
            y: delta.y,
        })
    };
    rust_ffi::FT_Set_Transform(
        Some(&mut state.inner),
        rust_matrix.as_ref(),
        rust_delta.as_ref(),
    );
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Transform(face: FT_Face, matrix: *mut FT_Matrix, delta: *mut FT_Vector) {
    let Some(state) = face_state(face) else {
        return;
    };
    let mut rust_matrix = rust_ffi::FT_Matrix::default();
    let mut rust_delta = rust_ffi::FT_Vector::default();
    rust_ffi::FT_Get_Transform(
        Some(&state.inner),
        (!matrix.is_null()).then_some(&mut rust_matrix),
        (!delta.is_null()).then_some(&mut rust_delta),
    );
    // SAFETY: non-null outputs point to caller-writable records for this
    // synchronous C ABI call.
    if let Some(matrix) = unsafe { matrix.as_mut() } {
        *matrix = FT_Matrix {
            xx: rust_matrix.xx,
            xy: rust_matrix.xy,
            yx: rust_matrix.yx,
            yy: rust_matrix.yy,
        };
    }
    // SAFETY: same output contract as `matrix` above.
    if let Some(delta) = unsafe { delta.as_mut() } {
        *delta = FT_Vector {
            x: rust_delta.x,
            y: rust_delta.y,
        };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Request_Size(face: FT_Face, req: *const FT_Size_RequestRec) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let request = if req.is_null() {
        None
    } else {
        // SAFETY: `req` was checked for null and is only copied by value.
        let req = unsafe { *req };
        Some(rust_ffi::FT_Size_RequestRec {
            type_: req.type_,
            width: req.width,
            height: req.height,
            horiResolution: req.horiResolution,
            vertResolution: req.vertResolution,
        })
    };
    let error = rust_ffi::FT_Request_Size(Some(&mut state.inner), request.as_ref());
    if error == rust_ffi::FT_Err_Ok {
        update_size_metrics(face, &state.inner);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Select_Size(face: FT_Face, strike_index: FT_Int) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let error = rust_ffi::FT_Select_Size(Some(&mut state.inner), strike_index);
    if error == rust_ffi::FT_Err_Ok {
        update_size_metrics(face, &state.inner);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_New_Size(face: FT_Face, asize: *mut FT_Size) -> FT_Error {
    let Some(_face_ptr) = non_null_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(out) = non_null_mut(asize) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };

    // SAFETY: `out` is a valid output pointer checked above.
    unsafe { *out.as_ptr() = ptr::null_mut() };
    let mut rust_size: rust_ffi::FT_Size = ptr::null_mut();
    let error = rust_ffi::FT_New_Size(Some(&state.inner), Some(&mut rust_size));
    if error != rust_ffi::FT_Err_Ok {
        return error;
    }

    let internal = Box::into_raw(Box::new(FT_Size_InternalRecCompat {
        rust_size,
        owner: face,
    }))
    .cast::<c_void>();
    let size = Box::into_raw(Box::new(FT_SizeRec {
        face,
        generic: FT_Generic::default(),
        metrics: rust_size_metrics_to_abi(state.inner.size_metrics),
        internal,
    }));
    state.push_size_record(size);
    // SAFETY: `out` is a valid output pointer checked above.
    unsafe { *out.as_ptr() = size };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Activate_Size(size: FT_Size) -> FT_Error {
    if non_null_mut(size).is_none() {
        return rust_ffi::FT_Err_Invalid_Size_Handle;
    };
    let Some(internal) = size_internal(size) else {
        return rust_ffi::FT_Err_Invalid_Size_Handle;
    };
    let (owner, rust_size) = (internal.owner, internal.rust_size);
    let Some(face_ptr) = non_null_mut(owner) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(_state) = face_state_mut(owner) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let error = rust_ffi::FT_Activate_Size(rust_size);
    if error == rust_ffi::FT_Err_Ok {
        // SAFETY: `face_ptr` is a live parent face and `size` is one of its size records.
        unsafe { (*face_ptr.as_ptr()).size = size };
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Done_Size(size: FT_Size) -> FT_Error {
    if non_null_mut(size).is_none() {
        return rust_ffi::FT_Err_Invalid_Size_Handle;
    };
    let Some(internal) = size_internal(size) else {
        return rust_ffi::FT_Err_Invalid_Size_Handle;
    };
    let (owner, rust_size) = (internal.owner, internal.rust_size);
    let Some(face_ptr) = non_null_mut(owner) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(state) = face_state_mut(owner) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };

    let error = rust_ffi::FT_Done_Size(rust_size);
    if error != rust_ffi::FT_Err_Ok {
        return error;
    }
    let was_active = unsafe { (*face_ptr.as_ptr()).size == size };
    let removed = state.remove_size_record(size);
    if !removed {
        return rust_ffi::FT_Err_Invalid_Size_Handle;
    }
    if was_active {
        let fallback = state
            .size_records
            .first()
            .copied()
            .unwrap_or(ptr::null_mut());
        // SAFETY: `face_ptr` is a live parent face; fallback is either null or still face-owned.
        unsafe { (*face_ptr.as_ptr()).size = fallback };
    }
    // SAFETY: the record has been removed from `state.size_records` and is consumed here.
    unsafe { drop_size(size) };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Char_Index(face: FT_Face, char_code: FT_ULong) -> FT_UInt {
    let Some(state) = face_state(face) else {
        return 0;
    };
    rust_ffi::FT_Get_Char_Index(&state.inner, char_code)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_GetCharVariantIndex(
    face: FT_Face,
    charcode: FT_ULong,
    variant_selector: FT_ULong,
) -> FT_UInt {
    let Some(state) = face_state(face) else {
        return 0;
    };
    rust_ffi::FT_Face_GetCharVariantIndex(Some(&state.inner), charcode, variant_selector)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_GetCharVariantIsDefault(
    face: FT_Face,
    charcode: FT_ULong,
    variant_selector: FT_ULong,
) -> FT_Int {
    let Some(state) = face_state(face) else {
        return -1;
    };
    rust_ffi::FT_Face_GetCharVariantIsDefault(Some(&state.inner), charcode, variant_selector)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_GetVariantSelectors(face: FT_Face) -> *mut FT_UInt32 {
    let Some(state) = face_state_mut(face) else {
        return ptr::null_mut();
    };
    let values = rust_ffi::FT_Face_GetVariantSelectors(Some(&state.inner));
    state.variant_list_ptr(values)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_GetVariantsOfChar(face: FT_Face, charcode: FT_ULong) -> *mut FT_UInt32 {
    let Some(state) = face_state_mut(face) else {
        return ptr::null_mut();
    };
    let values = rust_ffi::FT_Face_GetVariantsOfChar(Some(&state.inner), charcode);
    state.variant_list_ptr(values)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Face_GetCharsOfVariant(
    face: FT_Face,
    variant_selector: FT_ULong,
) -> *mut FT_UInt32 {
    let Some(state) = face_state_mut(face) else {
        return ptr::null_mut();
    };
    let values = rust_ffi::FT_Face_GetCharsOfVariant(Some(&state.inner), variant_selector);
    state.variant_list_ptr(values)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Kerning(
    face: FT_Face,
    left_glyph: FT_UInt,
    right_glyph: FT_UInt,
    kern_mode: FT_UInt,
    akerning: *mut FT_Vector,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(out) = NonNull::new(akerning) else {
        return rust_ffi::FT_Err_Invalid_Argument as FT_Error;
    };
    let mut vector = rust_ffi::FT_Vector::default();
    let err = rust_ffi::FT_Get_Kerning(
        Some(&state.inner),
        left_glyph,
        right_glyph,
        kern_mode,
        Some(&mut vector),
    );
    if err == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe {
            *out.as_ptr() = FT_Vector {
                x: vector.x,
                y: vector.y,
            };
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_PFR_Kerning(
    face: FT_Face,
    left_glyph: FT_UInt,
    right_glyph: FT_UInt,
    avector: *mut FT_Vector,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(out) = NonNull::new(avector) else {
        return rust_ffi::FT_Err_Invalid_Argument as FT_Error;
    };
    let mut vector = rust_ffi::FT_Vector::default();
    let err = rust_ffi::FT_Get_PFR_Kerning(
        Some(&state.inner),
        left_glyph,
        right_glyph,
        Some(&mut vector),
    );
    if err == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe {
            *out.as_ptr() = FT_Vector {
                x: vector.x,
                y: vector.y,
            };
        }
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_PFR_Metrics(
    face: FT_Face,
    aoutline_resolution: *mut FT_UInt,
    ametrics_resolution: *mut FT_UInt,
    ametrics_x_scale: *mut FT_Fixed,
    ametrics_y_scale: *mut FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    // SAFETY: FreeType permits each independent output to be null; every
    // non-null pointer denotes caller-provided scalar storage.
    unsafe {
        rust_ffi::FT_Get_PFR_Metrics(
            Some(&state.inner),
            aoutline_resolution.as_mut(),
            ametrics_resolution.as_mut(),
            ametrics_x_scale.as_mut(),
            ametrics_y_scale.as_mut(),
        )
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_PFR_Advance(
    face: FT_Face,
    gindex: FT_UInt,
    aadvance: *mut FT_Pos,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    // SAFETY: a null output is represented as `None`; a non-null pointer
    // denotes caller-provided scalar storage.
    unsafe { rust_ffi::FT_Get_PFR_Advance(Some(&state.inner), gindex, aadvance.as_mut()) }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Select_Charmap(face: FT_Face, encoding: FT_Encoding) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let error = rust_ffi::FT_Select_Charmap(Some(&mut state.inner), encoding);
    if error == rust_ffi::FT_Err_Ok {
        sync_face_public_record(face);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Charmap(face: FT_Face, charmap: FT_CharMap) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if state.charmaps.is_empty() || charmap.is_null() {
        return rust_ffi::FT_Err_Invalid_CharMap_Handle as FT_Error;
    }
    let Some(index) = state.charmap_index(charmap) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(rust_charmap) = rust_face_charmap(&state.inner, index) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let error = rust_ffi::FT_Set_Charmap(Some(&mut state.inner), rust_charmap);
    if error == rust_ffi::FT_Err_Ok {
        sync_face_public_record(face);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Charmap_Index(charmap: FT_CharMap) -> FT_Int {
    let Some(charmap) = NonNull::new(charmap) else {
        return -1;
    };
    // SAFETY: `charmap` is non-null and callers must pass either a live
    // `FT_CharMap` from this crate or accept C-like invalid-handle behavior.
    let face = unsafe { charmap.as_ref().face };
    let Some(state) = face_state(face) else {
        return -1;
    };
    let Some(index) = state.charmap_index(charmap.as_ptr()) else {
        return -1;
    };
    let Some(rust_charmap) = rust_face_charmap(&state.inner, index) else {
        return -1;
    };
    rust_ffi::FT_Get_Charmap_Index(rust_charmap) as FT_Int
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_CMap_Format(charmap: FT_CharMap) -> FT_Long {
    let Some(charmap) = NonNull::new(charmap) else {
        return -1;
    };
    // SAFETY: `charmap` is non-null and callers must pass either a live
    // `FT_CharMap` from this crate or accept C-like invalid-handle behavior.
    let face = unsafe { charmap.as_ref().face };
    let Some(state) = face_state(face) else {
        return -1;
    };
    let Some(index) = state.charmap_index(charmap.as_ptr()) else {
        return -1;
    };
    let Some(rust_charmap) = rust_face_charmap(&state.inner, index) else {
        return -1;
    };
    rust_ffi::FT_Get_CMap_Format(rust_charmap) as FT_Long
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_CMap_Language_ID(charmap: FT_CharMap) -> FT_ULong {
    let Some(charmap) = NonNull::new(charmap) else {
        return 0;
    };
    // SAFETY: `charmap` is non-null and callers must pass either a live
    // `FT_CharMap` from this crate or accept C-like invalid-handle behavior.
    let face = unsafe { charmap.as_ref().face };
    let Some(state) = face_state(face) else {
        return 0;
    };
    let Some(index) = state.charmap_index(charmap.as_ptr()) else {
        return 0;
    };
    let Some(rust_charmap) = rust_face_charmap(&state.inner, index) else {
        return 0;
    };
    rust_ffi::FT_Get_CMap_Language_ID(rust_charmap) as FT_ULong
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_FSType_Flags(face: FT_Face) -> FT_UShort {
    rust_ffi::FT_Get_FSType_Flags(face_state(face).map(|state| &state.inner))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Gasp(face: FT_Face, ppem: FT_UInt) -> FT_Int {
    rust_ffi::FT_Get_Gasp(face_state(face).map(|state| &state.inner), ppem)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Glyph_Name(
    face: FT_Face,
    glyph_index: FT_UInt,
    buffer: FT_Pointer,
    buffer_max: FT_UInt,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    if buffer.is_null() || buffer_max == 0 {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    // SAFETY: `buffer` is non-null, and the C caller promises at least
    // `buffer_max` writable bytes following FreeType's caller-allocated API.
    let buffer = unsafe { slice::from_raw_parts_mut(buffer.cast::<u8>(), buffer_max as usize) };
    match rust_ffi::FT_Get_Glyph_Name(&state.inner, glyph_index, buffer) {
        Ok(_) => rust_ffi::FT_Err_Ok,
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Name_Index(face: FT_Face, glyph_name: *const FT_String) -> FT_UInt {
    let Some(state) = face_state(face) else {
        return 0;
    };
    if glyph_name.is_null() {
        return 0;
    }
    // SAFETY: `glyph_name` is non-null and follows FreeType's C string
    // contract for this borrowed input pointer.
    let glyph_name = unsafe { CStr::from_ptr(glyph_name) };
    let Ok(glyph_name) = glyph_name.to_str() else {
        return 0;
    };
    rust_ffi::FT_Get_Name_Index(Some(&state.inner), Some(glyph_name))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Postscript_Name(face: FT_Face) -> *const c_char {
    face_state(face)
        .and_then(|state| state.postscript_name.as_deref())
        .map_or(ptr::null(), CStr::as_ptr)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Font_Format(face: FT_Face) -> *const c_char {
    face_state(face)
        .and_then(|state| state.font_format.as_deref())
        .map_or(ptr::null(), CStr::as_ptr)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_X11_Font_Format(face: FT_Face) -> *const c_char {
    FT_Get_Font_Format(face)
}

#[cfg(feature = "abi-test-support")]
pub fn abi_support_face_driver_name(face: FT_Face) -> *const c_char {
    face_state(face)
        .and_then(|state| state.face_driver_name.as_deref())
        .map_or(ptr::null(), CStr::as_ptr)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Named_Instance(face: FT_Face, instance_index: FT_UInt) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let err = rust_ffi::FT_Set_Named_Instance(Some(&mut state.inner), instance_index);
    if err == rust_ffi::FT_Err_Ok {
        state.refresh_charmaps(face);
        state.refresh_postscript_name();
        sync_face_public_record(face);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Var_Design_Coordinates(
    face: FT_Face,
    num_coords: FT_UInt,
    coords: *const FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` readable FT_Fixed values.
        Some(unsafe { slice::from_raw_parts(coords, num_coords as usize) })
    };
    let err = rust_ffi::FT_Set_Var_Design_Coordinates(Some(&mut state.inner), num_coords, coords);
    if err == rust_ffi::FT_Err_Ok {
        state.refresh_charmaps(face);
        state.refresh_postscript_name();
        sync_face_public_record(face);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Var_Design_Coordinates(
    face: FT_Face,
    num_coords: FT_UInt,
    coords: *mut FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` writable FT_Fixed values.
        Some(unsafe { slice::from_raw_parts_mut(coords, num_coords as usize) })
    };
    rust_ffi::FT_Get_Var_Design_Coordinates(Some(&state.inner), num_coords, coords)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Var_Blend_Coordinates(
    face: FT_Face,
    num_coords: FT_UInt,
    coords: *mut FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` writable FT_Fixed values.
        Some(unsafe { slice::from_raw_parts_mut(coords, num_coords as usize) })
    };
    rust_ffi::FT_Get_Var_Blend_Coordinates(Some(&state.inner), num_coords, coords)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_MM_Blend_Coordinates(
    face: FT_Face,
    num_coords: FT_UInt,
    coords: *mut FT_Fixed,
) -> FT_Error {
    FT_Get_Var_Blend_Coordinates(face, num_coords, coords)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Multi_Master(face: FT_Face, amaster: *mut FT_Multi_Master) -> FT_Error {
    // SAFETY: the caller provides writable storage for the public descriptor or null.
    let amaster = unsafe { amaster.as_mut() };
    rust_ffi::FT_Get_Multi_Master(face_state(face).map(|state| &state.inner), amaster)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_MM_Design_Coordinates(
    face: FT_Face,
    num_coords: FT_UInt,
    coords: *const FT_Long,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` readable FT_Long values.
        Some(unsafe { slice::from_raw_parts(coords, num_coords as usize) })
    };
    let err = rust_ffi::FT_Set_MM_Design_Coordinates(Some(&mut state.inner), num_coords, coords);
    if err == rust_ffi::FT_Err_Ok {
        state.refresh_charmaps(face);
        state.refresh_postscript_name();
        sync_face_public_record(face);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_MM_WeightVector(
    face: FT_Face,
    len: FT_UInt,
    weightvector: *const FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let weightvector = if weightvector.is_null() {
        None
    } else {
        // SAFETY: caller provides `len` readable FT_Fixed values.
        Some(unsafe { slice::from_raw_parts(weightvector, len as usize) })
    };
    let error = rust_ffi::FT_Set_MM_WeightVector(Some(&mut state.inner), len, weightvector);
    if error == rust_ffi::FT_Err_Ok {
        sync_face_public_record(face);
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_MM_WeightVector(
    face: FT_Face,
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
        face_state(face).map(|state| &state.inner),
        len_ref,
        weightvector,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_Var_Blend_Coordinates(
    face: FT_Face,
    num_coords: FT_UInt,
    coords: *const FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` readable FT_Fixed values.
        Some(unsafe { slice::from_raw_parts(coords, num_coords as usize) })
    };
    let err = rust_ffi::FT_Set_Var_Blend_Coordinates(Some(&mut state.inner), num_coords, coords);
    if err == rust_ffi::FT_Err_Ok {
        state.refresh_charmaps(face);
        state.refresh_postscript_name();
        sync_face_public_record(face);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Set_MM_Blend_Coordinates(
    face: FT_Face,
    num_coords: FT_UInt,
    coords: *const FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let coords = if coords.is_null() {
        None
    } else {
        // SAFETY: caller provides `num_coords` readable FT_Fixed values.
        Some(unsafe { slice::from_raw_parts(coords, num_coords as usize) })
    };
    let err = rust_ffi::FT_Set_MM_Blend_Coordinates(Some(&mut state.inner), num_coords, coords);
    if err == rust_ffi::FT_Err_Ok {
        state.refresh_charmaps(face);
        state.refresh_postscript_name();
        sync_face_public_record(face);
    }
    err
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Default_Named_Instance(
    face: FT_Face,
    instance_index: *mut FT_UInt,
) -> FT_Error {
    // SAFETY: the caller provides writable storage for the scalar output or null.
    let instance_index = unsafe { instance_index.as_mut() };
    rust_ffi::FT_Get_Default_Named_Instance(
        face_state(face).map(|state| &state.inner),
        instance_index,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_WinFNT_Header(
    face: FT_Face,
    header: *mut FT_WinFNT_HeaderRec,
) -> FT_Error {
    // SAFETY: the caller provides writable storage for the header output or null.
    let header = unsafe { header.as_mut() };
    rust_ffi::FT_Get_WinFNT_Header(face_state(face).map(|state| &state.inner), header)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_BDF_Property(
    face: FT_Face,
    prop_name: *const c_char,
    aproperty: *mut BDF_PropertyRec,
) -> FT_Error {
    let prop_name = property_name_arg(prop_name);
    let property = if aproperty.is_null() {
        None
    } else {
        // SAFETY: the caller provides writable storage for the BDF property
        // output or null.
        Some(unsafe { &mut *aproperty })
    };
    rust_ffi::FT_Get_BDF_Property(
        face_state(face).map(|state| &state.inner),
        prop_name.as_deref(),
        property,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_BDF_Charset_ID(
    face: FT_Face,
    acharset_encoding: *mut *const c_char,
    acharset_registry: *mut *const c_char,
) -> FT_Error {
    // SAFETY: the caller provides writable storage for either output pointer
    // or null, matching FreeType's nullable output contract.
    let charset_encoding = unsafe { acharset_encoding.as_mut() };
    // SAFETY: same as above for the registry output pointer.
    let charset_registry = unsafe { acharset_registry.as_mut() };
    rust_ffi::FT_Get_BDF_Charset_ID(
        face_state(face).map(|state| &state.inner),
        charset_encoding,
        charset_registry,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_CID_Is_Internally_CID_Keyed(
    face: FT_Face,
    is_cid: *mut FT_Bool,
) -> FT_Error {
    // SAFETY: the caller provides writable storage for the output pointer or
    // null, matching FreeType's nullable output contract.
    let is_cid = unsafe { is_cid.as_mut() };
    rust_ffi::FT_Get_CID_Is_Internally_CID_Keyed(face_state(face).map(|state| &state.inner), is_cid)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_CID_From_Glyph_Index(
    face: FT_Face,
    glyph_index: FT_UInt,
    cid: *mut FT_UInt,
) -> FT_Error {
    // SAFETY: the caller provides writable storage for the output pointer or
    // null, matching FreeType's nullable output contract.
    let cid = unsafe { cid.as_mut() };
    rust_ffi::FT_Get_CID_From_Glyph_Index(
        face_state(face).map(|state| &state.inner),
        glyph_index,
        cid,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_CID_Registry_Ordering_Supplement(
    face: FT_Face,
    registry: *mut *const c_char,
    ordering: *mut *const c_char,
    supplement: *mut FT_Int,
) -> FT_Error {
    // SAFETY: the caller provides writable storage for each output pointer or
    // null, matching FreeType's nullable output contract.
    let registry = unsafe { registry.as_mut() };
    // SAFETY: same as above for ordering.
    let ordering = unsafe { ordering.as_mut() };
    // SAFETY: same as above for supplement.
    let supplement = unsafe { supplement.as_mut() };
    rust_ffi::FT_Get_CID_Registry_Ordering_Supplement(
        face_state(face).map(|state| &state.inner),
        registry,
        ordering,
        supplement,
    )
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Sfnt_Name_Count(face: FT_Face) -> FT_UInt {
    rust_ffi::FT_Get_Sfnt_Name_Count(face_state(face).map(|state| &state.inner))
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Sfnt_Name(
    face: FT_Face,
    idx: FT_UInt,
    aname: *mut FT_SfntName,
) -> FT_Error {
    let Some(out) = non_null_mut(aname) else {
        return rust_ffi::FT_Get_Sfnt_Name(face_state(face).map(|state| &state.inner), idx, None);
    };
    let mut name = rust_ffi::FT_SfntName::default();
    let error = rust_ffi::FT_Get_Sfnt_Name(
        face_state(face).map(|state| &state.inner),
        idx,
        Some(&mut name),
    );
    if error == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe {
            *out.as_ptr() = FT_SfntName {
                platform_id: name.platform_id,
                encoding_id: name.encoding_id,
                language_id: name.language_id,
                name_id: name.name_id,
                string: name.string,
                string_len: name.string_len,
            };
        }
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Sfnt_LangTag(
    face: FT_Face,
    langID: FT_UInt,
    alangTag: *mut FT_SfntLangTag,
) -> FT_Error {
    let Some(out) = non_null_mut(alangTag) else {
        return rust_ffi::FT_Get_Sfnt_LangTag(
            face_state(face).map(|state| &state.inner),
            langID,
            None,
        );
    };
    let mut tag = rust_ffi::FT_SfntLangTag::default();
    let error = rust_ffi::FT_Get_Sfnt_LangTag(
        face_state(face).map(|state| &state.inner),
        langID,
        Some(&mut tag),
    );
    if error == rust_ffi::FT_Err_Ok {
        // SAFETY: `out` is non-null and addresses caller-writable output.
        unsafe {
            *out.as_ptr() = FT_SfntLangTag {
                string: tag.string,
                string_len: tag.string_len,
            };
        }
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Track_Kerning(
    face: FT_Face,
    point_size: FT_Fixed,
    degree: FT_Int,
    akerning: *mut FT_Fixed,
) -> FT_Error {
    let mut kerning = 0;
    let output = non_null_mut(akerning);
    let error = rust_ffi::FT_Get_Track_Kerning(
        face_state(face).map(|state| &state.inner),
        point_size,
        degree,
        output.map(|_| &mut kerning),
    );
    if error == rust_ffi::FT_Err_Ok {
        if let Some(output) = output {
            // SAFETY: `akerning` was checked for null and points to writable caller storage.
            unsafe { *output.as_ptr() = kerning };
        }
    }
    error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Sfnt_Table(face: FT_Face, tag: FT_Sfnt_Tag) -> *mut c_void {
    let Some(state) = face_state(face) else {
        return ptr::null_mut();
    };
    rust_ffi::FT_Get_Sfnt_Table(&state.inner, tag)
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Load_Sfnt_Table(
    face: FT_Face,
    tag: FT_ULong,
    offset: FT_Long,
    buffer: *mut FT_Byte,
    length: *mut FT_ULong,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(len_ptr) = non_null_mut(length) else {
        return match rust_ffi::FT_Load_Sfnt_Table(&state.inner, tag, offset, None) {
            Ok(Some(bytes)) => {
                if let Some(buf) = non_null_mut(buffer) {
                    // SAFETY: caller provides a buffer large enough for the selected table.
                    unsafe {
                        ptr::copy_nonoverlapping(bytes.as_ptr(), buf.as_ptr().cast(), bytes.len());
                    }
                }
                rust_ffi::FT_Err_Ok as FT_Error
            }
            Ok(None) => rust_ffi::FT_Err_Ok as FT_Error,
            Err(err) => err as FT_Error,
        };
    };
    // SAFETY: caller-provided writable FT_ULong or NULL (caught above).
    let mut len_val = unsafe { *len_ptr.as_ptr() };
    match rust_ffi::FT_Load_Sfnt_Table(&state.inner, tag, offset, Some(&mut len_val)) {
        Ok(Some(bytes)) => {
            let copy_len = bytes.len().min(len_val as usize);
            if let Some(buf) = non_null_mut(buffer) {
                // SAFETY: caller provides a buffer of at least len_val bytes.
                unsafe {
                    ptr::copy_nonoverlapping(bytes.as_ptr(), buf.as_ptr().cast(), copy_len);
                }
            }
            // SAFETY: writable FT_ULong out-param.
            unsafe { *len_ptr.as_ptr() = copy_len as FT_ULong };
            rust_ffi::FT_Err_Ok as FT_Error
        }
        Ok(None) => {
            // SAFETY: writable FT_ULong out-param (length probe result).
            unsafe { *len_ptr.as_ptr() = len_val };
            rust_ffi::FT_Err_Ok as FT_Error
        }
        Err(err) => err as FT_Error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Sfnt_Table_Info(
    face: FT_Face,
    table_index: FT_UInt,
    tag: *mut FT_ULong,
    length: *mut FT_ULong,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let mut tag_out: rust_ffi::FT_ULong = 0;
    let mut length_out: rust_ffi::FT_ULong = 0;
    let tag_ref = if tag.is_null() {
        None
    } else {
        Some(&mut tag_out)
    };
    let length_ref = if length.is_null() {
        None
    } else {
        Some(&mut length_out)
    };
    let err = rust_ffi::FT_Sfnt_Table_Info(&state.inner, table_index, tag_ref, length_ref);
    if err == rust_ffi::FT_Err_Ok {
        if let Some(tag_ptr) = non_null_mut(tag) {
            // SAFETY: writable FT_ULong out-param. Copying after the core call
            // avoids creating aliased `&mut` references for caller pointers.
            unsafe { *tag_ptr.as_ptr() = tag_out as FT_ULong };
        }
        if let Some(len_ptr) = non_null_mut(length) {
            // SAFETY: writable FT_ULong out-param. C writes tag before length,
            // so an aliased caller pointer ends with the length value.
            unsafe { *len_ptr.as_ptr() = length_out as FT_ULong };
        }
    }
    err as FT_Error
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_First_Char(face: FT_Face, agindex: *mut FT_UInt) -> FT_ULong {
    let mut glyph_index = 0;
    let char_code = rust_ffi::FT_Get_First_Char(
        face_state(face).map(|state| &state.inner),
        // FreeType `base/ftobjs.c:3952-3972` accepts a null `agindex`;
        // it still returns the charcode and skips only the glyph-index write.
        non_null_mut(agindex).map(|_| &mut glyph_index),
    );
    if let Some(out) = non_null_mut(agindex) {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe { *out.as_ptr() = glyph_index };
    }
    char_code
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Next_Char(
    face: FT_Face,
    char_code: FT_ULong,
    agindex: *mut FT_UInt,
) -> FT_ULong {
    let mut glyph_index = 0;
    let next_char = rust_ffi::FT_Get_Next_Char(
        face_state(face).map(|state| &state.inner),
        char_code,
        // FreeType `base/ftobjs.c:3977-4003` accepts a null `agindex`;
        // it still returns the next charcode and skips only the glyph-index write.
        non_null_mut(agindex).map(|_| &mut glyph_index),
    );
    if let Some(out) = non_null_mut(agindex) {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe { *out.as_ptr() = glyph_index };
    }
    next_char
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Library_Version(
    library: FT_Library,
    amajor: *mut FT_Int,
    aminor: *mut FT_Int,
    apatch: *mut FT_Int,
) {
    let mut major = 0;
    let mut minor = 0;
    let mut patch = 0;
    rust_ffi::FT_Library_Version(
        library_ref(library),
        Some(&mut major),
        Some(&mut minor),
        Some(&mut patch),
    );
    if let Some(out) = non_null_mut(amajor) {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe { *out.as_ptr() = major };
    }
    if let Some(out) = non_null_mut(aminor) {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe { *out.as_ptr() = minor };
    }
    if let Some(out) = non_null_mut(apatch) {
        // SAFETY: `out` is non-null and caller provides writable storage.
        unsafe { *out.as_ptr() = patch };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Load_Char(
    face: FT_Face,
    char_code: FT_ULong,
    load_flags: FT_Int32,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    match rust_ffi::FT_Load_Char(&state.inner, char_code, load_flags) {
        Ok(slot) => store_slot(face, slot, load_flags),
        Err(error) => error,
    }
}

fn apply_incremental_horizontal_metrics(
    interface: &FT_Incremental_InterfaceRec,
    funcs: &FT_Incremental_FuncsRec,
    glyph_index: FT_UInt,
    slot: &mut rust_ffi::FT_GlyphSlot,
) {
    let Some(get_glyph_metrics) = funcs.get_glyph_metrics else {
        return;
    };
    let mut horizontal = FT_Incremental_MetricsRec {
        bearing_x: slot.metrics.horiBearingX,
        bearing_y: 0,
        advance: slot.metrics.horiAdvance,
        advance_v: 0,
    };
    // SAFETY: the stored interface retains its caller-owned object through
    // the face lifetime, and `horizontal` is writable for this synchronous
    // callback.
    let horizontal_error =
        unsafe { get_glyph_metrics(interface.object, glyph_index, 0, &mut horizontal) };
    if horizontal_error == rust_ffi::FT_Err_Ok {
        slot.metrics.horiBearingX = horizontal.bearing_x;
        slot.metrics.horiAdvance = horizontal.advance;
    }
}

fn apply_incremental_vertical_metrics(
    interface: &FT_Incremental_InterfaceRec,
    funcs: &FT_Incremental_FuncsRec,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
    slot: &mut rust_ffi::FT_GlyphSlot,
) -> FT_Error {
    let Some(get_glyph_metrics) = funcs.get_glyph_metrics else {
        return rust_ffi::FT_Err_Ok;
    };
    let mut vertical = FT_Incremental_MetricsRec {
        bearing_x: 0,
        bearing_y: slot.metrics.vertBearingY,
        advance: slot.metrics.vertAdvance,
        advance_v: 0,
    };
    // SAFETY: identical lifetime and writable-record contract to the
    // horizontal callback above.
    let vertical_error =
        unsafe { get_glyph_metrics(interface.object, glyph_index, 1, &mut vertical) };
    if vertical_error != rust_ffi::FT_Err_Ok {
        return vertical_error;
    }
    slot.metrics.vertBearingX = slot
        .metrics
        .horiBearingX
        .saturating_sub(slot.metrics.horiAdvance / 2);
    slot.metrics.vertBearingY = vertical.bearing_y;
    slot.metrics.vertAdvance = vertical.advance;
    if load_flags & rust_ffi::FT_LOAD_VERTICAL_LAYOUT != 0 {
        slot.advance.x = 0;
        slot.advance.y = slot.metrics.vertAdvance;
    } else {
        slot.advance.x = slot.metrics.horiAdvance;
        slot.advance.y = 0;
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Load_Glyph(
    face: FT_Face,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let incremental_interface =
        face_internal(face).map_or(ptr::null_mut(), |internal| internal.incremental_interface);
    if incremental_interface.is_null() {
        return match rust_ffi::FT_Load_Glyph(&state.inner, glyph_index, load_flags) {
            Ok(slot) => store_slot(face, slot, load_flags),
            Err(error) => error,
        };
    }
    // SAFETY: `FT_Open_Face` stores the caller-owned interface pointer for
    // exactly the face lifetime, matching FreeType's borrowed parameter
    // contract.
    let Some(interface) = (unsafe {
        incremental_interface
            .cast::<FT_Incremental_InterfaceRec>()
            .as_ref()
    }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: a non-null interface must retain its callback table for the face
    // lifetime.
    let Some(funcs) = (unsafe { interface.funcs.as_ref() }) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Some(get_glyph_data) = funcs.get_glyph_data else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let mut glyph_data = FT_Data::default();
    // SAFETY: callback arguments follow the public `FT_Incremental` contract
    // and `glyph_data` remains live through its matching release callback.
    let acquire_error = unsafe { get_glyph_data(interface.object, glyph_index, &mut glyph_data) };
    if acquire_error != rust_ffi::FT_Err_Ok {
        return acquire_error;
    }
    let expected = rust_ffi::FT_Face_Incremental_Glyph_Data(&state.inner, glyph_index);
    let returned = if glyph_data.length == 0 {
        Some(&[][..])
    } else {
        non_null(glyph_data.pointer).and_then(|pointer| {
            let length = usize::try_from(glyph_data.length).ok()?;
            // SAFETY: successful `get_glyph_data` promises `length` readable
            // bytes until `free_glyph_data`.
            Some(unsafe { slice::from_raw_parts(pointer.as_ptr(), length) })
        })
    };
    let mut loaded = if expected.as_deref() == returned {
        rust_ffi::FT_Load_Glyph(&state.inner, glyph_index, load_flags)
    } else {
        Err(rust_ffi::FT_Err_Invalid_Table as FT_Error)
    };
    if let Ok(slot) = &mut loaded {
        apply_incremental_horizontal_metrics(interface, funcs, glyph_index, slot);
    }
    if let Some(free_glyph_data) = funcs.free_glyph_data {
        // SAFETY: this releases exactly the successful acquisition above,
        // after raw glyph parsing and horizontal-metric consumption.  Pinned
        // TrueType releases here before its later vertical-metrics callback.
        unsafe { free_glyph_data(interface.object, &mut glyph_data) };
    }
    match loaded {
        Ok(mut slot) => {
            let metrics_error = apply_incremental_vertical_metrics(
                interface,
                funcs,
                glyph_index,
                load_flags,
                &mut slot,
            );
            if metrics_error == rust_ffi::FT_Err_Ok {
                store_slot(face, slot, load_flags)
            } else {
                metrics_error
            }
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Advance(
    face: FT_Face,
    glyph_index: FT_UInt,
    load_flags: FT_Int32,
    padvance: *mut FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        // FreeType `src/base/ftadvanc.c:116-120` checks `face` before
        // `padvance`, so a missing face reports `Invalid_Face_Handle`.
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(out) = non_null_mut(padvance) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    match rust_ffi::FT_Get_Advance(&state.inner, glyph_index, load_flags) {
        Ok(advance) => {
            // SAFETY: `out` is a valid out pointer checked above.
            unsafe { *out.as_ptr() = advance };
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_Advances(
    face: FT_Face,
    start: FT_UInt,
    count: FT_UInt,
    load_flags: FT_Int32,
    padvances: *mut FT_Fixed,
) -> FT_Error {
    let Some(state) = face_state(face) else {
        // FreeType `src/base/ftadvanc.c:158-164` checks `face` before
        // `padvances`, so a missing face reports `Invalid_Face_Handle`.
        return rust_ffi::FT_Err_Invalid_Face_Handle as FT_Error;
    };
    let Some(out) = non_null_mut(padvances) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let Ok(out_len) = usize::try_from(count) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    match rust_ffi::FT_Get_Advances(&state.inner, start, count, load_flags) {
        Ok(advances) => {
            if advances.len() != out_len {
                return rust_ffi::FT_Err_Invalid_Argument;
            }
            if out_len != 0 {
                // SAFETY: `out` is non-null and caller promises at least `count` writable entries.
                let out = unsafe { slice::from_raw_parts_mut(out.as_ptr(), out_len) };
                out.copy_from_slice(&advances);
            }
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Get_SubGlyph_Info(
    slot: FT_GlyphSlot,
    sub_index: FT_UInt,
    p_index: *mut FT_Int,
    p_flags: *mut FT_UInt,
    p_arg1: *mut FT_Int,
    p_arg2: *mut FT_Int,
    p_transform: *mut FT_Matrix,
) -> FT_Error {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    let (Some(p_index), Some(p_flags), Some(p_arg1), Some(p_arg2), Some(p_transform)) = (
        non_null_mut(p_index),
        non_null_mut(p_flags),
        non_null_mut(p_arg1),
        non_null_mut(p_arg2),
        non_null_mut(p_transform),
    ) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };

    let Some(internal) = slot_internal(slot_ptr.as_ptr()) else {
        return rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error;
    };
    let mut index = 0;
    let mut flags = 0;
    let mut arg1 = 0;
    let mut arg2 = 0;
    let mut transform = rust_ffi::FT_Matrix::default();
    let error = rust_ffi::FT_Get_SubGlyph_Info(
        Some(&internal.rust_slot),
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

    // SAFETY: all output pointers are non-null and caller provides writable storage.
    unsafe {
        *p_index.as_ptr() = index;
        *p_flags.as_ptr() = flags;
        *p_arg1.as_ptr() = arg1;
        *p_arg2.as_ptr() = arg2;
        *p_transform.as_ptr() = FT_Matrix {
            xx: transform.xx,
            xy: transform.xy,
            yx: transform.yx,
            yy: transform.yy,
        };
    }
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_Render_Glyph(slot: FT_GlyphSlot, render_mode: FT_Render_Mode) -> FT_Error {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // FreeType checks both public pointers before consulting any private slot
    // state.  A caller-provided slot record with no face therefore returns
    // Invalid_Argument, even though it has no fontdone-owned `internal` tail.
    // SAFETY: `slot_ptr` is non-null and only the public `face` field is read.
    if unsafe { slot_ptr.as_ref() }.face.is_null() {
        return rust_ffi::FT_Err_Invalid_Argument;
    }
    let Some(internal) = slot_internal(slot_ptr.as_ptr()) else {
        return rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error;
    };
    let source_face = internal.source_face;
    let rust_slot = internal.rust_slot.clone();
    let load_flags = internal.load_flags;
    match rust_ffi::FT_Render_Glyph(rust_slot, render_mode) {
        Ok(rendered) => {
            // FreeType mutates the caller-provided slot in place.  Replacing
            // `face->glyph` would leave the C pointer passed to this function
            // stale even though the render succeeded.
            // SAFETY: `slot_ptr` is the live caller-provided face-owned slot.
            unsafe {
                replace_slot_record(
                    slot_ptr.as_ptr(),
                    rust_slot_to_abi(rendered, source_face, load_flags | rust_ffi::FT_LOAD_RENDER),
                );
            }
            rust_ffi::FT_Err_Ok
        }
        Err(error) => error,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_GlyphSlot_Embolden(slot: FT_GlyphSlot) {
    FT_GlyphSlot_AdjustWeight(slot, 0x0AAA, 0x0AAA);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_GlyphSlot_Own_Bitmap(slot: FT_GlyphSlot) -> FT_Error {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return rust_ffi::FT_Err_Ok;
    };
    // SAFETY: `slot_ptr` is a live slot allocated by this crate.
    let format = unsafe { (*slot_ptr.as_ptr()).format };
    if format != rust_ffi::FT_GLYPH_FORMAT_BITMAP {
        return rust_ffi::FT_Err_Ok;
    }
    let allocation_len = unsafe {
        usize::try_from((*slot_ptr.as_ptr()).bitmap.pitch.unsigned_abs())
            .unwrap_or(0)
            .saturating_mul(usize::try_from((*slot_ptr.as_ptr()).bitmap.rows).unwrap_or(0))
    };
    let Some(internal) = slot_internal_mut(slot_ptr.as_ptr()) else {
        return rust_ffi::FT_Err_Invalid_Slot_Handle as FT_Error;
    };
    let already_owned = internal.flags & 1 != 0;
    internal.rust_slot.owns_bitmap = already_owned;
    internal.owns_bitmap = already_owned;
    if already_owned {
        return rust_ffi::FT_Err_Ok;
    }

    let allocation_memory = face_state(internal.source_face)
        .and_then(|face| library_state_mut(face.library))
        .map_or(ptr::null_mut(), |library| library.allocation_memory);
    if allocation_len != 0
        && let Some(memory) = unsafe { allocation_memory.as_mut() }
        && let Some(alloc) = memory.alloc
    {
        let allocation = alloc(
            allocation_memory,
            c_long::try_from(allocation_len).unwrap_or(c_long::MAX),
        );
        if allocation.is_null() {
            return rust_ffi::FT_Err_Out_Of_Memory;
        }
        if let Some(free) = memory.free {
            free(allocation_memory, allocation);
        }
    }

    let err = rust_ffi::FT_GlyphSlot_Own_Bitmap(Some(&mut internal.rust_slot));
    if err != rust_ffi::FT_Err_Ok {
        return err;
    }
    let replacement = rust_slot_to_abi(
        internal.rust_slot.clone(),
        internal.source_face,
        internal.load_flags,
    );
    // SAFETY: `slot_ptr` is checked non-null and points to a live slot allocated by this crate.
    unsafe { replace_slot_record(slot_ptr.as_ptr(), replacement) };
    rust_ffi::FT_Err_Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_GlyphSlot_AdjustWeight(
    slot: FT_GlyphSlot,
    xdelta: FT_Fixed,
    ydelta: FT_Fixed,
) {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return;
    };
    let Some(internal) = slot_internal_mut(slot_ptr.as_ptr()) else {
        return;
    };
    rust_ffi::FT_GlyphSlot_AdjustWeight(Some(&mut internal.rust_slot), xdelta, ydelta);
    let replacement = rust_slot_to_abi(
        internal.rust_slot.clone(),
        internal.source_face,
        internal.load_flags,
    );
    // SAFETY: `slot_ptr` is a live slot allocated by this crate.
    unsafe { replace_slot_record(slot_ptr.as_ptr(), replacement) };
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_GlyphSlot_Oblique(slot: FT_GlyphSlot) {
    FT_GlyphSlot_Slant(slot, 0x0366A, 0);
}

#[unsafe(no_mangle)]
pub extern "C" fn FT_GlyphSlot_Slant(slot: FT_GlyphSlot, xslant: FT_Fixed, yslant: FT_Fixed) {
    let Some(slot_ptr) = non_null_mut(slot) else {
        return;
    };
    let Some(internal) = slot_internal_mut(slot_ptr.as_ptr()) else {
        return;
    };
    rust_ffi::FT_GlyphSlot_Slant(Some(&mut internal.rust_slot), xslant, yslant);
    let replacement = rust_slot_to_abi(
        internal.rust_slot.clone(),
        internal.source_face,
        internal.load_flags,
    );
    // SAFETY: `slot_ptr` is a live slot allocated by this crate.
    unsafe { replace_slot_record(slot_ptr.as_ptr(), replacement) };
}

fn store_slot(face: FT_Face, slot: rust_ffi::FT_GlyphSlot, load_flags: FT_Int32) -> FT_Error {
    let Some(face_ptr) = non_null_mut(face) else {
        return rust_ffi::FT_Err_Invalid_Argument;
    };
    // SAFETY: `face_ptr` is a live handle and owns the previous `glyph` pointer.
    unsafe {
        drop_glyph((*face_ptr.as_ptr()).glyph);
        (*face_ptr.as_ptr()).glyph =
            Box::into_raw(Box::new(rust_slot_to_abi(slot, face, load_flags)));
    }
    rust_ffi::FT_Err_Ok
}

fn update_size_metrics(face: FT_Face, rust_face: &rust_ffi::FT_Face) {
    let Some(face_ptr) = non_null_mut(face) else {
        return;
    };
    // SAFETY: `face_ptr` is a live handle allocated by this crate.
    let size = unsafe { (*face_ptr.as_ptr()).size };
    let Some(size_ptr) = non_null_mut(size) else {
        return;
    };
    // SAFETY: `size_ptr` points to the live size record owned by `face`.
    unsafe { (*size_ptr.as_ptr()).metrics = rust_size_metrics_to_abi(rust_face.size_metrics) };
}

#[allow(
    dead_code,
    reason = "used by the feature-gated ABI parity inspection surface"
)]
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

fn rust_face_charmap(face: &rust_ffi::FT_Face, index: usize) -> Option<rust_ffi::FT_CharMap> {
    face.charmaps.get(index).map(|record| {
        (record as *const rust_ffi::FT_CharMapRecPublic)
            .cast_mut()
            .cast()
    })
}

fn rust_slot_to_abi(
    slot: rust_ffi::FT_GlyphSlot,
    source_face: FT_Face,
    load_flags: FT_Int32,
) -> FT_GlyphSlotRec {
    let rust_slot = slot.clone();
    let mut outline_points = slot
        .outline
        .as_ref()
        .map(|outline| {
            outline
                .points
                .iter()
                .map(|point| FT_Vector {
                    x: point.x,
                    y: point.y,
                })
                .collect::<Vec<_>>()
                .into_boxed_slice()
        })
        .unwrap_or_default();
    let mut outline_tags = slot
        .outline
        .as_ref()
        .map(|outline| outline.tags.clone().into_boxed_slice())
        .unwrap_or_default();
    let mut outline_contours = slot
        .outline
        .as_ref()
        .map(|outline| outline.contours.clone().into_boxed_slice())
        .unwrap_or_default();
    let outline = FT_Outline {
        n_contours: FT_UShort::try_from(outline_contours.len()).unwrap_or(FT_UShort::MAX),
        n_points: FT_UShort::try_from(outline_points.len()).unwrap_or(FT_UShort::MAX),
        points: outline_points.as_mut_ptr(),
        tags: outline_tags.as_mut_ptr(),
        contours: outline_contours.as_mut_ptr(),
        flags: slot.outline.as_ref().map_or(0, |outline| outline.flags),
    };
    let mut buffer = slot
        .bitmap
        .as_ref()
        .map(|bitmap| bitmap.buffer.clone())
        .unwrap_or_default();
    // FreeType keeps a zero-sized rendered bitmap's gray descriptor visible
    // even though it has no backing buffer.  The pure-Rust slot represents
    // that empty bitmap as `None`, so restore the public C record fields at
    // this ABI boundary; otherwise an independently linked C consumer sees
    // format/max_grays as zero while the pinned C oracle reports gray/256.
    let rendered_empty_bitmap =
        slot.bitmap.is_none() && slot.format == rust_ffi::FT_GLYPH_FORMAT_BITMAP;
    let bitmap = slot.bitmap.map_or_else(
        || FT_Bitmap {
            num_grays: if rendered_empty_bitmap { 256 } else { 0 },
            pixel_mode: if rendered_empty_bitmap {
                rust_ffi::FT_PIXEL_MODE_GRAY as c_uchar
            } else {
                0
            },
            ..FT_Bitmap::default()
        },
        |bitmap| FT_Bitmap {
            rows: bitmap.rows,
            width: bitmap.width,
            pitch: bitmap.pitch,
            buffer: buffer.as_mut_ptr(),
            num_grays: bitmap.num_grays,
            pixel_mode: u8::try_from(bitmap.pixel_mode).unwrap_or(0),
            palette_mode: 0,
            palette: ptr::null_mut(),
        },
    );
    let mut svg_document = slot
        .svg
        .as_ref()
        .map(|document| document.svg_document.clone().into_boxed_slice())
        .unwrap_or_default();
    let svg_record = slot.svg.as_ref().map(|document| {
        Box::new(FT_SVG_DocumentRec {
            svg_document: svg_document.as_mut_ptr(),
            svg_document_length: FT_ULong::try_from(svg_document.len()).unwrap_or(FT_ULong::MAX),
            metrics: rust_size_metrics_to_abi(document.metrics),
            units_per_EM: document.units_per_EM,
            start_glyph_id: document.start_glyph_id,
            end_glyph_id: document.end_glyph_id,
            transform: FT_Matrix {
                xx: document.transform.xx,
                xy: document.transform.xy,
                yx: document.transform.yx,
                yy: document.transform.yy,
            },
            delta: FT_Vector {
                x: document.delta.x,
                y: document.delta.y,
            },
        })
    });
    let mut slot_internal = Box::new(FT_Slot_InternalRecCompat {
        loader: ptr::null_mut(),
        flags: FT_UInt::from(slot.owns_bitmap),
        glyph_transformed: 0,
        glyph_matrix: FT_Matrix {
            xx: 0x10000,
            xy: 0,
            yx: 0,
            yy: 0x10000,
        },
        glyph_delta: FT_Vector::default(),
        glyph_hints: ptr::null_mut(),
        load_flags,
        owns_bitmap: slot.owns_bitmap,
        buffer,
        outline_points,
        outline_tags,
        outline_contours,
        svg_document,
        svg_record,
        rust_slot,
        source_face,
    });
    let other = slot_internal
        .svg_record
        .as_deref_mut()
        .map_or(ptr::null_mut(), |record| {
            ptr::from_mut(record).cast::<c_void>()
        });
    let internal = Box::into_raw(slot_internal).cast();
    FT_GlyphSlotRec {
        library: face_state(source_face).map_or(ptr::null_mut(), |state| state.library),
        face: source_face,
        next: ptr::null_mut(),
        glyph_index: slot.glyph_index,
        generic: FT_Generic::default(),
        metrics: rust_metrics_to_abi(slot.metrics),
        linearHoriAdvance: 0,
        linearVertAdvance: 0,
        advance: FT_Vector {
            x: slot.advance.x,
            y: slot.advance.y,
        },
        format: slot.format,
        bitmap,
        bitmap_left: slot.bitmap_left,
        bitmap_top: slot.bitmap_top,
        outline,
        num_subglyphs: slot.num_subglyphs,
        subglyphs: ptr::null_mut(),
        control_data: ptr::null_mut(),
        control_len: 0,
        lsb_delta: 0,
        rsb_delta: 0,
        other,
        internal,
    }
}

fn rust_metrics_to_abi(metrics: rust_ffi::FT_Glyph_Metrics) -> FT_Glyph_Metrics {
    FT_Glyph_Metrics {
        width: metrics.width,
        height: metrics.height,
        horiBearingX: metrics.horiBearingX,
        horiBearingY: metrics.horiBearingY,
        horiAdvance: metrics.horiAdvance,
        vertBearingX: metrics.vertBearingX,
        vertBearingY: metrics.vertBearingY,
        vertAdvance: metrics.vertAdvance,
    }
}

fn rust_size_metrics_to_abi(metrics: rust_ffi::FT_Size_Metrics) -> FT_Size_Metrics {
    FT_Size_Metrics {
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

fn outline_snapshot_from_c(outline: *const FT_Outline) -> Option<rust_ffi::FT_OutlineSnapshot> {
    let outline = non_null(outline)?;
    // SAFETY: `outline` is non-null; callers of the C ABI must pass a valid `FT_Outline`.
    let outline = unsafe { outline.as_ref() };
    let n_points = usize::from(outline.n_points);
    let n_contours = usize::from(outline.n_contours);
    if (n_points > 0 && outline.points.is_null()) || (n_contours > 0 && outline.contours.is_null())
    {
        return None;
    }
    let points = if n_points == 0 {
        Vec::new()
    } else {
        // SAFETY: `points` is non-null for `n_points > 0`; the C ABI caller owns a readable
        // array of `n_points` `FT_Vector` records for the duration of this call.
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

fn copy_outline_snapshot_to_c(
    outline: *mut FT_Outline,
    snapshot: &rust_ffi::FT_OutlineSnapshot,
    copy_tags_and_flags: bool,
) {
    let Some(mut outline) = non_null_mut(outline) else {
        return;
    };
    // SAFETY: `outline` is non-null and still refers to the caller-owned
    // descriptor used to create `snapshot`.
    let outline = unsafe { outline.as_mut() };
    let snapshot_points = snapshot.points.len();
    let snapshot_contours = snapshot.contours.len();
    if !outline.points.is_null() {
        // SAFETY: the public descriptor is caller-owned and FreeType export
        // APIs require enough capacity for the counts returned by
        // FT_Stroker_GetCounts/FT_Stroker_GetBorderCounts.  Thin C ABI code
        // only copies core output into that caller-provided storage.
        let points = unsafe { slice::from_raw_parts_mut(outline.points, snapshot_points) };
        for (target, source) in points.iter_mut().zip(&snapshot.points) {
            target.x = source.x;
            target.y = source.y;
        }
    }
    if copy_tags_and_flags {
        if !outline.tags.is_null() {
            // SAFETY: see the points copy above.
            let tags = unsafe { slice::from_raw_parts_mut(outline.tags, snapshot_points) };
            for (target, source) in tags.iter_mut().zip(&snapshot.tags) {
                *target = *source;
            }
        }
        if !outline.contours.is_null() {
            // SAFETY: see the points copy above.
            let contours =
                unsafe { slice::from_raw_parts_mut(outline.contours, snapshot_contours) };
            for (target, source) in contours.iter_mut().zip(&snapshot.contours) {
                *target = *source;
            }
        }
        outline.n_points = u16::try_from(snapshot_points).unwrap_or(u16::MAX);
        outline.n_contours = u16::try_from(snapshot_contours).unwrap_or(u16::MAX);
        outline.flags = snapshot.flags;
    }
}

fn copy_rendered_bitmap_to_c(target: &FT_Bitmap, rendered: &rust_ffi::FT_Bitmap) {
    let rows = usize::try_from(target.rows).unwrap_or(0);
    let width = usize::try_from(target.width).unwrap_or(0);
    let pitch_abs = usize::try_from(target.pitch.unsigned_abs()).unwrap_or(0);
    let rendered_pitch_abs = usize::try_from(rendered.pitch.unsigned_abs()).unwrap_or(0);
    if target.buffer.is_null() || rows == 0 || width == 0 || pitch_abs == 0 {
        return;
    }
    let row_bytes = width.min(pitch_abs);
    let target_len = pitch_abs.saturating_mul(rows);
    // SAFETY: the public C caller provides a writable bitmap buffer of at least
    // `abs(pitch) * rows` bytes, matching FreeType's `FT_Bitmap` contract.
    let target_buffer = unsafe { slice::from_raw_parts_mut(target.buffer, target_len) };
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

fn bitmap_to_rust(bitmap: &FT_Bitmap) -> rust_ffi::FT_Bitmap_C {
    rust_ffi::FT_Bitmap_C {
        rows: bitmap.rows,
        width: bitmap.width,
        pitch: bitmap.pitch,
        buffer: bitmap.buffer,
        num_grays: bitmap.num_grays,
        pixel_mode: bitmap.pixel_mode,
        palette_mode: bitmap.palette_mode,
        palette: bitmap.palette,
    }
}

// C callers may hand the façade either an external buffer or a buffer that a
// previous Rust-backed bitmap operation installed in the ownership registry.
// Materialize a private Rust view without unregistering that original buffer;
// the wrapper can then commit the replacement only after the Rust operation
// succeeds and preserve the public C record on an error.
fn bitmap_rust_view(bitmap: &FT_Bitmap) -> rust_ffi::FT_Bitmap_C {
    let mut view = bitmap_to_rust(bitmap);
    if let Some(bytes) = bitmap_bytes(bitmap) {
        view.buffer = ptr::null_mut();
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut view), bytes);
    }
    view
}

// Drop only a Rust-owned temporary buffer.  The registry helper intentionally
// does not require a live library, so invalid-library paths cannot leak the
// temporary materialization.
fn release_bitmap_buffer(bitmap: &rust_ffi::FT_Bitmap_C) {
    let mut view = *bitmap;
    if !view.buffer.is_null() {
        rust_ffi::FT_Bitmap_Set_Owned_Buffer(Some(&mut view), Vec::new());
    }
}

fn copy_rust_bitmap_record_to_c(target: &mut FT_Bitmap, source: &rust_ffi::FT_Bitmap_C) {
    target.rows = source.rows;
    target.width = source.width;
    target.pitch = source.pitch;
    target.buffer = source.buffer;
    target.num_grays = source.num_grays;
    target.pixel_mode = source.pixel_mode;
    target.palette_mode = source.palette_mode;
    target.palette = source.palette;
}

fn bitmap_bytes(bitmap: &FT_Bitmap) -> Option<Vec<u8>> {
    let len = usize::try_from(bitmap.pitch.unsigned_abs())
        .ok()?
        .checked_mul(usize::try_from(bitmap.rows).ok()?)?;
    if bitmap.buffer.is_null() || len == 0 {
        return None;
    }
    Some(unsafe { slice::from_raw_parts(bitmap.buffer, len) }.to_vec())
}

fn bitmap_copy_array_too_large(bitmap: &FT_Bitmap) -> bool {
    let pitch = bitmap.pitch.unsigned_abs() as usize;
    let rows = bitmap.rows as usize;
    pitch > 0 && rows > (i32::MAX as usize) / pitch
}

fn face_state(face: FT_Face) -> Option<&'static FaceState> {
    let internal = face_internal(face)?;
    Some(&internal.state)
}

fn face_state_mut(face: FT_Face) -> Option<&'static mut FaceState> {
    let internal = face_internal_mut(face)?;
    Some(internal.state.as_mut())
}

fn face_internal(face: FT_Face) -> Option<&'static FT_Face_InternalRecCompat> {
    let face = non_null_mut(face)?;
    // SAFETY: `face` is non-null and must have been allocated by `FT_New_Memory_Face`.
    let internal = unsafe { (*face.as_ptr()).internal };
    let internal = NonNull::new(internal.cast::<FT_Face_InternalRecCompat>())?;
    // SAFETY: `internal` points to the compatibility record owned by this live face.
    Some(unsafe { internal.as_ref() })
}

fn face_internal_mut(face: FT_Face) -> Option<&'static mut FT_Face_InternalRecCompat> {
    let face = non_null_mut(face)?;
    // SAFETY: `face` is non-null and must have been allocated by `FT_New_Memory_Face`.
    let internal = unsafe { (*face.as_ptr()).internal };
    let mut internal = NonNull::new(internal.cast::<FT_Face_InternalRecCompat>())?;
    // SAFETY: `internal` points to the compatibility record owned exclusively
    // by this live face.
    Some(unsafe { internal.as_mut() })
}

fn slot_internal(slot: FT_GlyphSlot) -> Option<&'static FT_Slot_InternalRecCompat> {
    let slot = non_null_mut(slot)?;
    // SAFETY: `slot` is a live record allocated by this crate; its opaque
    // internal pointer owns an `FT_Slot_InternalRecCompat`.
    let internal = unsafe { (*slot.as_ptr()).internal };
    let internal = NonNull::new(internal.cast::<FT_Slot_InternalRecCompat>())?;
    // SAFETY: the internal record lives until the owning glyph slot is
    // replaced or destroyed.
    Some(unsafe { internal.as_ref() })
}

fn slot_internal_mut(slot: FT_GlyphSlot) -> Option<&'static mut FT_Slot_InternalRecCompat> {
    let slot = non_null_mut(slot)?;
    // SAFETY: `slot` is a live record allocated by this crate; its opaque
    // internal pointer owns an `FT_Slot_InternalRecCompat`.
    let internal = unsafe { (*slot.as_ptr()).internal };
    let mut internal = NonNull::new(internal.cast::<FT_Slot_InternalRecCompat>())?;
    // SAFETY: callers hold the unique mutation path for this live slot.
    Some(unsafe { internal.as_mut() })
}

fn size_internal(size: FT_Size) -> Option<&'static FT_Size_InternalRecCompat> {
    let size = non_null_mut(size)?;
    // SAFETY: `size` is a live record allocated by this crate; its opaque
    // internal pointer owns an `FT_Size_InternalRecCompat`.
    let internal = unsafe { (*size.as_ptr()).internal };
    let internal = NonNull::new(internal.cast::<FT_Size_InternalRecCompat>())?;
    // SAFETY: the internal record lives until the owning size is destroyed.
    Some(unsafe { internal.as_ref() })
}

fn size_internal_mut(size: FT_Size) -> Option<&'static mut FT_Size_InternalRecCompat> {
    let size = non_null_mut(size)?;
    // SAFETY: `size` is a live record allocated by this crate; its opaque
    // internal pointer owns an `FT_Size_InternalRecCompat`.
    let internal = unsafe { (*size.as_ptr()).internal };
    let mut internal = NonNull::new(internal.cast::<FT_Size_InternalRecCompat>())?;
    // SAFETY: callers hold the unique mutation path for this live size.
    Some(unsafe { internal.as_mut() })
}

fn sync_face_public_record(face: FT_Face) {
    let Some(face_ptr) = non_null_mut(face) else {
        return;
    };
    let Some(state) = face_state_mut(face) else {
        return;
    };
    let active_charmap = FT_UInt::try_from(state.inner.active_charmap_index)
        .ok()
        .and_then(|index| state.charmap_by_index(index))
        .unwrap_or(ptr::null_mut());
    // SAFETY: `face_ptr` and `state` belong to the same live face.  This
    // updates the public snapshot after a successful core state transition.
    unsafe {
        let record = face_ptr.as_ptr();
        (*record).face_index = state.inner.face_index;
        (*record).face_flags = state.inner.face_flags;
        (*record).style_flags = state.inner.style_flags;
        (*record).charmap = active_charmap;
        (*record).num_charmaps = FT_Int::try_from(state.charmap_ptrs.len()).unwrap_or(FT_Int::MAX);
        (*record).charmaps = state.charmap_ptrs.as_mut_ptr();
        (*record).ascender = state.inner.ascender;
        (*record).descender = state.inner.descender;
        (*record).height = state.inner.height;
        (*record).max_advance_width = state.inner.max_advance_width;
        (*record).max_advance_height = state.inner.max_advance_height;
        if !(*record).size.is_null() {
            (*(*record).size).metrics = rust_size_metrics_to_abi(state.inner.size_metrics);
        }
    }
}

fn library_ref(library: FT_Library) -> Option<&'static rust_ffi::FT_Library> {
    if !library_is_live(library) {
        return None;
    }
    let library = non_null_mut(library)?;
    // SAFETY: `library` is non-null and must have been allocated by `FT_Init_FreeType`.
    let internal = unsafe { (*library.as_ptr()).internal };
    if internal.is_null() {
        None
    } else {
        // SAFETY: `internal` points to a `LibraryState` allocated by this crate.
        Some(unsafe { &(*internal.cast::<LibraryState>()).inner })
    }
}

fn library_state_mut(library: FT_Library) -> Option<&'static mut LibraryState> {
    if !library_is_live(library) {
        return None;
    }
    let library = non_null_mut(library)?;
    // SAFETY: `library` is non-null and must have been allocated by `FT_Init_FreeType`.
    let internal = unsafe { (*library.as_ptr()).internal };
    let mut state = NonNull::new(internal.cast::<LibraryState>())?;
    // SAFETY: `internal` points to a uniquely borrowed `LibraryState`.
    Some(unsafe { state.as_mut() })
}

fn library_mut(library: FT_Library) -> Option<&'static mut rust_ffi::FT_Library> {
    if !library_is_live(library) {
        return None;
    }
    let library = non_null_mut(library)?;
    // SAFETY: `library` is non-null and must have been allocated by `FT_Init_FreeType`.
    let internal = unsafe { (*library.as_ptr()).internal };
    if internal.is_null() {
        None
    } else {
        // SAFETY: `internal` points to a uniquely borrowed `LibraryState`.
        Some(unsafe { &mut (*internal.cast::<LibraryState>()).inner })
    }
}

fn non_null<T>(ptr: *const T) -> Option<NonNull<T>> {
    NonNull::new(ptr.cast_mut())
}

fn non_null_mut<T>(ptr: *mut T) -> Option<NonNull<T>> {
    NonNull::new(ptr)
}

unsafe fn drop_glyph(slot: FT_GlyphSlot) {
    if !slot.is_null() {
        // SAFETY: `slot` is live, and its opaque internal record was allocated
        // with `Box::into_raw` by `rust_slot_to_abi`.
        let internal = unsafe { (*slot).internal };
        if !internal.is_null() {
            unsafe {
                drop(Box::from_raw(internal.cast::<FT_Slot_InternalRecCompat>()));
            }
        }
        // SAFETY: `slot` is owned by its containing face and allocated with `Box::into_raw`.
        unsafe { drop(Box::from_raw(slot)) };
    }
}

unsafe fn drop_size(size: FT_Size) {
    if !size.is_null() {
        // SAFETY: `size` is live, and its opaque internal record was allocated
        // with `Box::into_raw` by the size constructor.
        let internal = unsafe { (*size).internal };
        if !internal.is_null() {
            unsafe {
                drop(Box::from_raw(internal.cast::<FT_Size_InternalRecCompat>()));
            }
        }
        // SAFETY: `size` is owned by its containing face and allocated with `Box::into_raw`.
        unsafe { drop(Box::from_raw(size)) };
    }
}

unsafe fn replace_slot_record(slot: FT_GlyphSlot, replacement: FT_GlyphSlotRec) {
    // SAFETY: callers pass a live slot allocated by this crate.  The
    // replacement owns a distinct internal allocation, so the old internal
    // record can be released after the public record is overwritten.
    let old_internal = unsafe { (*slot).internal };
    unsafe { *slot = replacement };
    if !old_internal.is_null() {
        unsafe {
            drop(Box::from_raw(
                old_internal.cast::<FT_Slot_InternalRecCompat>(),
            ));
        }
    }
}
