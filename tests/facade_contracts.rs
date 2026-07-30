//! Public C ABI and WASM façade lifecycle contracts.

#![allow(unused_crate_dependencies)]

use std::ptr;

use fontdone::ffi::{
    FT_Err_Invalid_Argument, FT_Err_Invalid_Face_Handle, FT_Err_Invalid_Library_Handle, FT_Err_Ok,
    FT_PIXEL_MODE_GRAY, FT_PIXEL_MODE_GRAY2,
};
use fontdone_c_abi as c_abi;
use fontdone_wasm as wasm;

fn c_bitmap_bytes(bitmap: &c_abi::FT_Bitmap) -> Vec<u8> {
    let len = bitmap.pitch.unsigned_abs().saturating_mul(bitmap.rows) as c_abi::FT_UInt;
    c_abi::abi_byte_slice(bitmap.buffer, len)
}

fn wasm_bitmap_bytes(bitmap: &wasm::FontdoneWasmBitmap) -> Vec<u8> {
    let len = bitmap.pitch.unsigned_abs().saturating_mul(bitmap.rows);
    c_abi::abi_byte_slice(bitmap.buffer, len)
}

#[test]
fn c_abi_bitmap_contract_covers_validation_copy_conversion_and_cleanup()
-> Result<(), std::num::TryFromIntError> {
    let gray_pixel_mode = u8::try_from(FT_PIXEL_MODE_GRAY)?;
    let gray2_pixel_mode = u8::try_from(FT_PIXEL_MODE_GRAY2)?;

    c_abi::FT_Bitmap_Init(ptr::null_mut());
    c_abi::FT_Bitmap_New(ptr::null_mut());

    let mut dirty = c_abi::FT_Bitmap {
        rows: 9,
        width: 8,
        pitch: 7,
        buffer: ptr::dangling_mut(),
        num_grays: 6,
        pixel_mode: 5,
        palette_mode: 4,
        palette: ptr::dangling_mut(),
    };
    c_abi::FT_Bitmap_Init(&mut dirty);
    assert_eq!(dirty.rows, 0);
    assert_eq!(dirty.width, 0);
    assert_eq!(dirty.pitch, 0);
    assert!(dirty.buffer.is_null());
    assert_eq!(dirty.num_grays, 0);
    assert_eq!(dirty.pixel_mode, 0);
    assert_eq!(dirty.palette_mode, 0);
    assert!(dirty.palette.is_null());

    let mut library = ptr::null_mut();
    assert_eq!(c_abi::FT_Init_FreeType(&mut library), FT_Err_Ok);
    assert!(!library.is_null());

    let mut source_bytes = vec![1, 2, 3, 4, 5, 6];
    let source = c_abi::FT_Bitmap {
        rows: 2,
        width: 3,
        pitch: 3,
        buffer: source_bytes.as_mut_ptr(),
        num_grays: 256,
        pixel_mode: gray_pixel_mode,
        palette_mode: 0,
        palette: ptr::null_mut(),
    };
    let mut target = c_abi::FT_Bitmap::default();

    assert_eq!(
        i64::from(c_abi::FT_Bitmap_Copy(ptr::null_mut(), &source, &mut target)),
        FT_Err_Invalid_Library_Handle
    );
    assert_eq!(
        c_abi::FT_Bitmap_Copy(library, ptr::null(), &mut target),
        FT_Err_Invalid_Argument
    );
    assert_eq!(
        c_abi::FT_Bitmap_Copy(library, &source, ptr::null_mut()),
        FT_Err_Invalid_Argument
    );
    assert_eq!(
        c_abi::FT_Bitmap_Copy(library, &source, &mut target),
        FT_Err_Ok
    );
    assert_eq!(c_bitmap_bytes(&target), source_bytes);
    source_bytes[0] = 99;
    assert_eq!(c_bitmap_bytes(&target), [1, 2, 3, 4, 5, 6]);

    let target_ptr = ptr::from_mut(&mut target);
    assert_eq!(
        c_abi::FT_Bitmap_Copy(library, target_ptr.cast_const(), target_ptr),
        FT_Err_Ok
    );
    assert_eq!(c_bitmap_bytes(&target), [1, 2, 3, 4, 5, 6]);

    assert_eq!(
        i64::from(c_abi::FT_Bitmap_Done(ptr::null_mut(), &mut target)),
        FT_Err_Invalid_Library_Handle
    );
    assert_eq!(
        c_abi::FT_Bitmap_Done(library, ptr::null_mut()),
        FT_Err_Invalid_Argument
    );
    assert_eq!(c_abi::FT_Bitmap_Done(library, &mut target), FT_Err_Ok);
    assert!(target.buffer.is_null());
    assert_eq!(target.rows, 0);
    assert_eq!(target.width, 0);

    let mut packed = vec![0b00_01_10_11];
    let packed_source = c_abi::FT_Bitmap {
        rows: 1,
        width: 4,
        pitch: 1,
        buffer: packed.as_mut_ptr(),
        num_grays: 4,
        pixel_mode: gray2_pixel_mode,
        palette_mode: 0,
        palette: ptr::null_mut(),
    };
    let mut converted = c_abi::FT_Bitmap::default();
    assert_eq!(
        c_abi::FT_Bitmap_Convert(library, &packed_source, &mut converted, 4),
        FT_Err_Ok
    );
    assert_eq!(converted.pixel_mode, gray_pixel_mode);
    assert_eq!(converted.pitch, 4);
    assert_eq!(c_bitmap_bytes(&converted), [0, 1, 2, 3]);
    assert_eq!(c_abi::FT_Bitmap_Done(library, &mut converted), FT_Err_Ok);

    assert_eq!(c_abi::FT_Done_FreeType(library), FT_Err_Ok);
    assert_eq!(
        i64::from(c_abi::FT_Done_FreeType(library)),
        FT_Err_Invalid_Library_Handle
    );

    Ok(())
}

#[test]
fn wasm_bitmap_contract_covers_validation_copy_conversion_and_cleanup() {
    wasm::fontdone_wasm_bitmap_init(ptr::null_mut());
    wasm::fontdone_wasm_bitmap_new(ptr::null_mut());

    let mut dirty = wasm::FontdoneWasmBitmap {
        rows: 9,
        width: 8,
        pitch: 7,
        buffer: ptr::dangling(),
        buffer_len: 6,
        num_grays: 5,
        pixel_mode: 4,
        palette_mode: 3,
        palette: ptr::dangling(),
    };
    wasm::fontdone_wasm_bitmap_new(&mut dirty);
    assert_eq!(dirty.rows, 0);
    assert_eq!(dirty.width, 0);
    assert_eq!(dirty.pitch, 0);
    assert!(dirty.buffer.is_null());
    assert_eq!(dirty.buffer_len, 0);
    assert_eq!(dirty.num_grays, 0);
    assert_eq!(dirty.pixel_mode, 0);
    assert_eq!(dirty.palette_mode, 0);
    assert!(dirty.palette.is_null());

    let mut source_bytes = vec![1, 2, 3, 4, 5, 6];
    let source = wasm::FontdoneWasmBitmap {
        rows: 2,
        width: 3,
        pitch: 3,
        buffer: source_bytes.as_ptr(),
        buffer_len: source_bytes.len(),
        num_grays: 256,
        pixel_mode: FT_PIXEL_MODE_GRAY,
        palette_mode: 0,
        palette: ptr::null(),
    };
    let mut target = wasm::FontdoneWasmBitmap::default();

    assert_eq!(
        i64::from(wasm::fontdone_wasm_bitmap_copy(0, &source, &mut target)),
        FT_Err_Invalid_Library_Handle
    );
    assert_eq!(
        wasm::fontdone_wasm_bitmap_copy(1, ptr::null(), &mut target),
        FT_Err_Invalid_Argument
    );
    assert_eq!(
        wasm::fontdone_wasm_bitmap_copy(1, &source, ptr::null_mut()),
        FT_Err_Invalid_Argument
    );
    assert_eq!(
        wasm::fontdone_wasm_bitmap_copy(1, &source, &mut target),
        FT_Err_Ok
    );
    assert_eq!(wasm_bitmap_bytes(&target), source_bytes);
    source_bytes[0] = 99;
    assert_eq!(wasm_bitmap_bytes(&target), [1, 2, 3, 4, 5, 6]);

    let target_ptr = ptr::from_mut(&mut target);
    assert_eq!(
        wasm::fontdone_wasm_bitmap_copy(1, target_ptr.cast_const(), target_ptr),
        FT_Err_Ok
    );
    assert_eq!(wasm_bitmap_bytes(&target), [1, 2, 3, 4, 5, 6]);

    assert_eq!(
        i64::from(wasm::fontdone_wasm_bitmap_done(0, &mut target)),
        FT_Err_Invalid_Library_Handle
    );
    assert_eq!(
        wasm::fontdone_wasm_bitmap_done(1, ptr::null_mut()),
        FT_Err_Invalid_Argument
    );
    assert_eq!(wasm::fontdone_wasm_bitmap_done(1, &mut target), FT_Err_Ok);
    assert!(target.buffer.is_null());
    assert_eq!(target.buffer_len, 0);
    assert_eq!(target.rows, 0);
    assert_eq!(target.width, 0);

    let packed = [0b00_01_10_11];
    let packed_source = wasm::FontdoneWasmBitmap {
        rows: 1,
        width: 4,
        pitch: 1,
        buffer: packed.as_ptr(),
        buffer_len: packed.len(),
        num_grays: 4,
        pixel_mode: FT_PIXEL_MODE_GRAY2,
        palette_mode: 0,
        palette: ptr::null(),
    };
    let mut converted = wasm::FontdoneWasmBitmap::default();
    assert_eq!(
        wasm::fontdone_wasm_bitmap_convert(1, &packed_source, &mut converted, 4),
        FT_Err_Ok
    );
    assert_eq!(converted.pixel_mode, FT_PIXEL_MODE_GRAY);
    assert_eq!(converted.pitch, 4);
    assert_eq!(wasm_bitmap_bytes(&converted), [0, 1, 2, 3]);
    assert_eq!(
        wasm::fontdone_wasm_bitmap_done(1, &mut converted),
        FT_Err_Ok
    );
}

#[test]
fn facade_null_contracts_cover_cache_transform_logging_and_memory_routes() {
    let mut cmap_cache = ptr::null_mut();
    assert_eq!(
        c_abi::FTC_CMapCache_New(ptr::null_mut(), &mut cmap_cache),
        FT_Err_Invalid_Argument
    );
    assert_eq!(
        c_abi::FTC_CMapCache_New(ptr::null_mut(), ptr::null_mut()),
        FT_Err_Invalid_Argument
    );
    assert_eq!(
        c_abi::FTC_CMapCache_Lookup(ptr::null_mut(), ptr::null_mut(), -1, 0),
        0
    );
    c_abi::FTC_Manager_RemoveFaceID(ptr::null_mut(), ptr::null_mut());

    let mut image_cache = ptr::null_mut();
    assert_eq!(
        c_abi::FTC_ImageCache_New(ptr::null_mut(), &mut image_cache),
        FT_Err_Invalid_Argument
    );
    assert_eq!(
        c_abi::FTC_ImageCache_Lookup(
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            ptr::null_mut(),
            ptr::null_mut(),
        ),
        FT_Err_Invalid_Argument
    );
    assert_eq!(
        c_abi::FTC_ImageCache_LookupScaler(
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
        ),
        FT_Err_Invalid_Argument
    );
    let mut sbit_cache = ptr::null_mut();
    assert_eq!(
        c_abi::FTC_SBitCache_New(ptr::null_mut(), &mut sbit_cache),
        FT_Err_Invalid_Argument
    );
    assert_eq!(
        c_abi::FTC_SBitCache_LookupScaler(
            ptr::null_mut(),
            ptr::null_mut(),
            0,
            0,
            ptr::null_mut(),
            ptr::null_mut(),
        ),
        FT_Err_Invalid_Argument
    );
    c_abi::FTC_Node_Unref(ptr::null_mut(), ptr::null_mut());

    assert_eq!(
        i64::from(c_abi::FT_New_Face(
            ptr::null_mut(),
            ptr::null(),
            0,
            ptr::null_mut(),
        )),
        FT_Err_Invalid_Library_Handle
    );
    assert_eq!(
        i64::from(c_abi::FT_Attach_File(ptr::null_mut(), ptr::null())),
        FT_Err_Invalid_Face_Handle
    );
    assert_eq!(
        i64::from(c_abi::FT_Reference_Face(ptr::null_mut())),
        FT_Err_Invalid_Face_Handle
    );
    c_abi::FT_Get_Transform(ptr::null_mut(), ptr::null_mut(), ptr::null_mut());
    assert_eq!(
        i64::from(c_abi::FT_Get_Sfnt_LangTag(
            ptr::null_mut(),
            0,
            ptr::null_mut(),
        )),
        i64::from(FT_Err_Invalid_Argument)
    );
    let mut empty_outline = c_abi::FT_Outline::default();
    assert_eq!(
        c_abi::FT_Outline_Decompose(&mut empty_outline, ptr::null(), ptr::null_mut(),),
        FT_Err_Invalid_Argument
    );
    assert_eq!(
        i64::from(c_abi::FT_Outline_Decompose(
            ptr::null_mut(),
            ptr::null(),
            ptr::null_mut(),
        )),
        i64::from(fontdone::ffi::FT_Err_Invalid_Outline)
    );
    c_abi::FT_Set_Default_Properties(ptr::null_mut());
    c_abi::FT_Trace_Set_Level(ptr::null());
    c_abi::FT_Trace_Set_Default_Level();
    c_abi::FT_Set_Log_Handler(ptr::null_mut());
    c_abi::FT_Set_Default_Log_Handler();

    assert!(wasm::fontdone_wasm_bitmap_buffer(0).is_null());
    assert_eq!(wasm::fontdone_wasm_bitmap_len(0), 0);
    assert_eq!(wasm::fontdone_wasm_bitmap_width(0), 0);
    assert_eq!(wasm::fontdone_wasm_bitmap_rows(0), 0);
    assert_eq!(wasm::fontdone_wasm_bitmap_pitch(0), 0);
    assert_eq!(
        i64::from(wasm::fontdone_wasm_done_face(0)),
        FT_Err_Invalid_Face_Handle
    );

    let zero_allocation = wasm::fontdone_wasm_malloc(0);
    assert!(!zero_allocation.is_null());
    wasm::fontdone_wasm_free(zero_allocation, 0);
    wasm::fontdone_wasm_free(ptr::null_mut(), usize::MAX);

    let mut wasm_error = FT_Err_Ok;
    assert_eq!(
        wasm::fontdone_wasm_open_face_handle(ptr::null(), 0, 0, 20.0, ptr::null_mut(),),
        0
    );
    assert_eq!(
        wasm::fontdone_wasm_open_face_handle(ptr::null(), 0, 0, 20.0, &mut wasm_error,),
        0
    );
    assert_eq!(wasm_error, FT_Err_Invalid_Argument);

    let font = include_bytes!("fixtures/input/fonts/DejaVuSans.ttf");
    let handle =
        wasm::fontdone_wasm_open_face_handle(font.as_ptr(), font.len(), 0, 20.0, &mut wasm_error);
    assert_ne!(handle, 0);
    assert_eq!(wasm_error, FT_Err_Ok);
    assert!(wasm::fontdone_wasm_bitmap_buffer(handle).is_null());
    assert_eq!(wasm::fontdone_wasm_bitmap_len(handle), 0);
    assert_eq!(wasm::fontdone_wasm_bitmap_width(handle), 0);
    assert_eq!(wasm::fontdone_wasm_bitmap_rows(handle), 0);
    assert_eq!(wasm::fontdone_wasm_bitmap_pitch(handle), 0);
    assert_eq!(wasm::fontdone_wasm_done_face(handle), FT_Err_Ok);
}
