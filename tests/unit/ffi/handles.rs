use super::*;

fn sfnt_with_table(tag: [u8; 4], table: &[u8]) -> Vec<u8> {
    const TABLE_OFFSET: usize = 28;
    let mut data = vec![0; TABLE_OFFSET + table.len()];
    data[..4].copy_from_slice(&crate::tt::TRUE_MAGIC.to_be_bytes());
    data[4..6].copy_from_slice(&1u16.to_be_bytes());
    data[12..16].copy_from_slice(&tag);
    data[20..24].copy_from_slice(&(TABLE_OFFSET as u32).to_be_bytes());
    data[24..28].copy_from_slice(&(table.len() as u32).to_be_bytes());
    data[TABLE_OFFSET..].copy_from_slice(table);
    data
}

fn owned_bitmap(width: u32, rows: u32, pitch: i32, pixel_mode: i32, bytes: Vec<u8>) -> FT_Bitmap_C {
    let mut bitmap = FT_Bitmap_C {
        width,
        rows,
        pitch,
        pixel_mode: pixel_mode as u8,
        ..FT_Bitmap_C::default()
    };
    FT_Bitmap_Set_Owned_Buffer(Some(&mut bitmap), bytes);
    bitmap
}

fn done(library: &FT_Library, bitmap: &mut FT_Bitmap_C) {
    assert_eq!(FT_Bitmap_Done(Some(library), Some(bitmap)), FT_Err_Ok);
}

#[test]
fn error_string_and_diagnostic_guards_match_the_pinned_build() {
    assert_eq!(FT_Error_String(-1), None);
    assert_eq!(FT_Error_String(FT_Err_Max), None);
    assert_eq!(FT_Error_String(FT_Err_Ok), None);

    assert_eq!(
        FT_Sfnt_Load_Name_Diagnostic(&[]),
        FT_Err_Invalid_File_Format
    );
    assert_eq!(
        FT_Sfnt_Load_Name_Diagnostic(&sfnt_with_table(*b"cmap", &[])),
        FT_Err_Table_Missing as FT_Error
    );
    assert_eq!(
        FT_Sfnt_Load_Name_Diagnostic(&sfnt_with_table(*b"name", &[0; 5])),
        FT_Err_Invalid_Stream_Operation as FT_Error
    );

    let mut missing_records = [0u8; 6];
    missing_records[2..4].copy_from_slice(&1u16.to_be_bytes());
    assert_eq!(
        FT_Sfnt_Load_Name_Diagnostic(&sfnt_with_table(*b"name", &missing_records)),
        FT_Err_Name_Table_Missing as FT_Error
    );
    assert_eq!(
        FT_Sfnt_Load_Name_Diagnostic(&sfnt_with_table(*b"name", &[0; 6])),
        FT_Err_Ok
    );
    assert_eq!(
        FT_Open_Face_NonDriver_Diagnostic(),
        FT_Err_Invalid_Handle as FT_Error
    );
    assert_ne!(
        FT_TrueType_Context_Allocation_Failure_Diagnostic(),
        FT_Err_Ok
    );
}

#[test]
fn bitmap_initialization_accepts_null_and_resets_records() {
    FT_Bitmap_Init(None);
    FT_Bitmap_New(None);

    let dangling = std::ptr::NonNull::<u8>::dangling().as_ptr();
    let mut bitmap = FT_Bitmap_C {
        rows: 3,
        width: 4,
        pitch: 5,
        buffer: dangling,
        num_grays: 256,
        pixel_mode: FT_PIXEL_MODE_GRAY as u8,
        palette_mode: 1,
        palette: dangling.cast(),
    };
    FT_Bitmap_Init(Some(&mut bitmap));
    assert_eq!(bitmap, FT_Bitmap_C::default());

    bitmap.width = 9;
    FT_Bitmap_New(Some(&mut bitmap));
    assert_eq!(bitmap, FT_Bitmap_C::default());
}

#[test]
fn bitmap_registry_distinguishes_null_unknown_short_and_owned_buffers() {
    let library = FT_Init_FreeType();
    assert_eq!(bitmap_buffer_len(&FT_Bitmap_C::default()), Some(0));
    assert_eq!(bitmap_pitch_abs(&FT_Bitmap_C::default()), Some(0));
    assert_eq!(bitmap_owned_bytes(&FT_Bitmap_C::default()), None);
    assert_eq!(bitmap_source_bytes(&FT_Bitmap_C::default()), Ok(None));
    assert_eq!(FT_Bitmap_Owned_Buffer_Bytes(None), None);
    FT_Bitmap_Set_Owned_Buffer(None, vec![1]);

    let dangling = std::ptr::NonNull::<u8>::dangling().as_ptr();
    let mut unknown = FT_Bitmap_C {
        rows: 1,
        width: 1,
        pitch: 1,
        buffer: dangling,
        ..FT_Bitmap_C::default()
    };
    assert_eq!(bitmap_owned_bytes(&unknown), None);
    assert_eq!(bitmap_source_bytes(&unknown), Err(FT_Err_Invalid_Argument));
    assert_eq!(FT_Bitmap_Owned_Buffer_Bytes(Some(&unknown)), None);
    done(&library, &mut unknown);

    let mut short = owned_bitmap(2, 1, 2, FT_PIXEL_MODE_GRAY, vec![7]);
    assert_eq!(bitmap_owned_bytes(&short), None);
    assert_eq!(bitmap_source_bytes(&short), Err(FT_Err_Invalid_Argument));
    done(&library, &mut short);

    let mut owned = owned_bitmap(2, 1, 2, FT_PIXEL_MODE_GRAY, vec![7, 8]);
    assert_eq!(bitmap_owned_bytes(&owned), Some(vec![7, 8]));
    assert_eq!(bitmap_source_bytes(&owned), Ok(Some(vec![7, 8])));
    assert_eq!(FT_Bitmap_Owned_Buffer_Bytes(Some(&owned)), Some(vec![7, 8]));
    FT_Bitmap_Set_Owned_Buffer(Some(&mut owned), vec![9, 10]);
    assert_eq!(
        FT_Bitmap_Owned_Buffer_Bytes(Some(&owned)),
        Some(vec![9, 10])
    );
    FT_Bitmap_Set_Owned_Buffer(Some(&mut owned), Vec::new());
    assert!(owned.buffer.is_null());
    assert_eq!(FT_Bitmap_Owned_Buffer_Bytes(Some(&owned)), None);
}

#[test]
fn bitmap_copy_validates_inputs_and_preserves_row_flow() {
    let library = FT_Init_FreeType();
    let source = FT_Bitmap_C::default();
    let mut target = FT_Bitmap_C::default();
    assert_eq!(
        FT_Bitmap_Copy(None, Some(&source), Some(&mut target)),
        FT_Err_Invalid_Library_Handle as FT_Error
    );
    assert_eq!(
        FT_Bitmap_Copy(Some(&library), None, Some(&mut target)),
        FT_Err_Invalid_Argument
    );
    assert_eq!(
        FT_Bitmap_Copy(Some(&library), Some(&source), None),
        FT_Err_Invalid_Argument
    );
    assert_eq!(
        FT_Bitmap_Copy(Some(&library), Some(&source), Some(&mut target)),
        FT_Err_Ok
    );
    assert!(target.buffer.is_null());

    let dangling = std::ptr::NonNull::<u8>::dangling().as_ptr();
    let unknown = FT_Bitmap_C {
        rows: 1,
        width: 1,
        pitch: 1,
        buffer: dangling,
        ..FT_Bitmap_C::default()
    };
    assert_eq!(
        FT_Bitmap_Copy(Some(&library), Some(&unknown), Some(&mut target)),
        FT_Err_Invalid_Argument
    );
    assert!(target.buffer.is_null());

    let mut short = owned_bitmap(2, 1, 2, FT_PIXEL_MODE_GRAY, vec![1]);
    assert_eq!(
        FT_Bitmap_Copy(Some(&library), Some(&short), Some(&mut target)),
        FT_Err_Invalid_Argument
    );
    done(&library, &mut short);

    let mut positive = owned_bitmap(2, 2, 2, FT_PIXEL_MODE_GRAY, vec![1, 2, 3, 4]);
    target.pitch = -1;
    assert_eq!(
        FT_Bitmap_Copy(Some(&library), Some(&positive), Some(&mut target)),
        FT_Err_Ok
    );
    assert_eq!(target.pitch, -2);
    assert_eq!(
        FT_Bitmap_Owned_Buffer_Bytes(Some(&target)),
        Some(vec![3, 4, 1, 2])
    );
    done(&library, &mut target);

    target.pitch = 1;
    assert_eq!(
        FT_Bitmap_Copy(Some(&library), Some(&positive), Some(&mut target)),
        FT_Err_Ok
    );
    assert_eq!(target.pitch, 2);
    assert_eq!(
        FT_Bitmap_Owned_Buffer_Bytes(Some(&target)),
        Some(vec![1, 2, 3, 4])
    );
    done(&library, &mut positive);
    done(&library, &mut target);
}

fn convert(
    library: &FT_Library,
    source: (u32, u32, i32, i32, Vec<u8>),
    alignment: i32,
    target_pitch: i32,
) -> FT_Bitmap_C {
    let (width, rows, pitch, mode, bytes) = source;
    let mut source = owned_bitmap(width, rows, pitch, mode, bytes);
    let mut target = FT_Bitmap_C {
        pitch: target_pitch,
        ..FT_Bitmap_C::default()
    };
    assert_eq!(
        FT_Bitmap_Convert(Some(library), Some(&source), Some(&mut target), alignment,),
        FT_Err_Ok
    );
    done(library, &mut source);
    target
}

#[test]
fn bitmap_convert_covers_supported_encodings_and_alignment_signs() {
    let library = FT_Init_FreeType();
    let cases = [
        (
            4,
            1,
            1,
            FT_PIXEL_MODE_MONO,
            vec![0b1010_0000],
            vec![1, 0, 1, 0],
        ),
        (
            4,
            1,
            1,
            FT_PIXEL_MODE_GRAY2,
            vec![0b0001_1011],
            vec![0, 1, 2, 3],
        ),
        (2, 1, 1, FT_PIXEL_MODE_GRAY4, vec![0xab], vec![10, 11]),
        (3, 1, 3, FT_PIXEL_MODE_GRAY, vec![3, 4, 5], vec![3, 4, 5]),
        (3, 1, 3, FT_PIXEL_MODE_LCD, vec![6, 7, 8], vec![6, 7, 8]),
        (
            3,
            1,
            3,
            FT_PIXEL_MODE_LCD_V,
            vec![9, 10, 11],
            vec![9, 10, 11],
        ),
        (1, 1, 4, FT_PIXEL_MODE_BGRA, vec![0, 0, 0, 255], vec![255]),
    ];
    for (width, rows, pitch, mode, source, expected) in cases {
        let mut target = convert(&library, (width, rows, pitch, mode, source), 1, 0);
        assert_eq!(FT_Bitmap_Owned_Buffer_Bytes(Some(&target)), Some(expected));
        done(&library, &mut target);
    }

    let mut positive_alignment =
        convert(&library, (3, 1, 3, FT_PIXEL_MODE_GRAY, vec![1, 2, 3]), 4, 0);
    assert_eq!(positive_alignment.pitch, 4);
    done(&library, &mut positive_alignment);

    let mut negative_alignment = convert(
        &library,
        (3, 2, -3, FT_PIXEL_MODE_GRAY, vec![4, 5, 6, 1, 2, 3]),
        -4,
        -1,
    );
    assert_eq!(negative_alignment.pitch, -4);
    assert_eq!(
        FT_Bitmap_Owned_Buffer_Bytes(Some(&negative_alignment)),
        Some(vec![4, 5, 6, 0, 1, 2, 3, 0])
    );
    done(&library, &mut negative_alignment);
}

#[test]
fn bitmap_convert_rejects_invalid_sources_and_records() {
    let library = FT_Init_FreeType();
    let source = FT_Bitmap_C::default();
    let mut target = FT_Bitmap_C::default();
    assert_eq!(
        FT_Bitmap_Convert(None, Some(&source), Some(&mut target), 1),
        FT_Err_Invalid_Library_Handle as FT_Error
    );
    assert_eq!(
        FT_Bitmap_Convert(Some(&library), None, Some(&mut target), 1),
        FT_Err_Invalid_Argument
    );
    assert_eq!(
        FT_Bitmap_Convert(Some(&library), Some(&source), None, 1),
        FT_Err_Invalid_Argument
    );

    let invalid_mode = FT_Bitmap_C {
        pixel_mode: FT_PIXEL_MODE_NONE as u8,
        ..FT_Bitmap_C::default()
    };
    assert_eq!(
        FT_Bitmap_Convert(Some(&library), Some(&invalid_mode), Some(&mut target), 1,),
        FT_Err_Invalid_Argument
    );

    let empty = FT_Bitmap_C {
        pixel_mode: FT_PIXEL_MODE_GRAY as u8,
        ..FT_Bitmap_C::default()
    };
    assert_eq!(
        FT_Bitmap_Convert(Some(&library), Some(&empty), Some(&mut target), 1),
        FT_Err_Ok
    );

    let missing = FT_Bitmap_C {
        rows: 1,
        width: 1,
        pitch: 1,
        pixel_mode: FT_PIXEL_MODE_GRAY as u8,
        ..FT_Bitmap_C::default()
    };
    assert_eq!(
        FT_Bitmap_Convert(Some(&library), Some(&missing), Some(&mut target), 1),
        FT_Err_Invalid_Argument
    );

    let mut narrow = owned_bitmap(2, 1, 1, FT_PIXEL_MODE_GRAY, vec![1]);
    assert_eq!(
        FT_Bitmap_Convert(Some(&library), Some(&narrow), Some(&mut target), 1),
        FT_Err_Invalid_Argument
    );
    done(&library, &mut narrow);
    done(&library, &mut target);
}

#[test]
fn unpack_helpers_validate_bounds_and_decode_rows() {
    let mut output = [0u8; 4];
    assert_eq!(
        unpack_bitmap_mono_row(&[0b1010_0000], &mut output, 4),
        Ok(())
    );
    assert_eq!(output, [1, 0, 1, 0]);
    assert_eq!(
        unpack_bitmap_mono_row(&[], &mut output, 1),
        Err(FT_Err_Invalid_Argument)
    );

    assert_eq!(
        unpack_bitmap_gray2_row(&[0b0001_1011], &mut output, 4),
        Ok(())
    );
    assert_eq!(output, [0, 1, 2, 3]);
    assert_eq!(
        unpack_bitmap_gray2_row(&[], &mut output, 1),
        Err(FT_Err_Invalid_Argument)
    );

    assert_eq!(
        unpack_bitmap_gray4_row(&[0xab, 0xcd], &mut output, 4),
        Ok(())
    );
    assert_eq!(output, [10, 11, 12, 13]);
    assert_eq!(
        unpack_bitmap_gray4_row(&[], &mut output, 1),
        Err(FT_Err_Invalid_Argument)
    );

    assert_eq!(
        unpack_bitmap_bgra_row(&[0, 0, 0, 255], &mut output, 1),
        Ok(())
    );
    assert_eq!(output[0], 255);
    assert_eq!(
        unpack_bitmap_bgra_row(&[0, 0, 0], &mut output, 1),
        Err(FT_Err_Invalid_Argument)
    );
    assert_eq!(gray_for_premultiplied_srgb_bgra(&[0, 0, 0, 0]), 0);
}

#[test]
fn bitmap_done_validates_handles_and_releases_owned_memory() {
    let library = FT_Init_FreeType();
    let mut bitmap = owned_bitmap(1, 1, 1, FT_PIXEL_MODE_GRAY, vec![1]);
    assert_eq!(
        FT_Bitmap_Done(None, Some(&mut bitmap)),
        FT_Err_Invalid_Library_Handle as FT_Error
    );
    assert!(!bitmap.buffer.is_null());
    assert_eq!(
        FT_Bitmap_Done(Some(&library), None),
        FT_Err_Invalid_Argument
    );
    done(&library, &mut bitmap);
    assert_eq!(bitmap, FT_Bitmap_C::default());
    done(&library, &mut bitmap);
}
