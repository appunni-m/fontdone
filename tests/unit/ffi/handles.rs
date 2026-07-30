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

#[test]
fn bitmap_strength_rounding_and_embolden_guards_are_exact() {
    assert_eq!(ft_bitmap_strength_pixels(0), Some(0));
    assert_eq!(ft_bitmap_strength_pixels(31), Some(0));
    assert_eq!(ft_bitmap_strength_pixels(32), Some(1));
    assert_eq!(ft_bitmap_strength_pixels(-64), Some(-1));
    assert_eq!(
        ft_bitmap_strength_pixels((i64::from(i32::MAX) + 1) << 6),
        None
    );
    assert_eq!(ft_bitmap_strength_pixels(FT_Long::MAX), None);

    let library = FT_Init_FreeType();
    assert_eq!(
        FT_Bitmap_Embolden(None, None, 0, 0),
        FT_Err_Invalid_Library_Handle as FT_Error
    );
    assert_eq!(
        FT_Bitmap_Embolden(Some(&library), None, 0, 0),
        FT_Err_Invalid_Argument
    );

    let mut empty = FT_Bitmap_C::default();
    assert_eq!(
        FT_Bitmap_Embolden(Some(&library), Some(&mut empty), 0, 0),
        FT_Err_Invalid_Argument
    );

    let mut bitmap = owned_bitmap(1, 1, 1, FT_PIXEL_MODE_GRAY, vec![20]);
    assert_eq!(
        FT_Bitmap_Embolden(Some(&library), Some(&mut bitmap), -64, 0),
        FT_Err_Invalid_Argument
    );
    assert_eq!(
        FT_Bitmap_Embolden(Some(&library), Some(&mut bitmap), 0, 0),
        FT_Err_Ok
    );
    assert_eq!(
        FT_Bitmap_Embolden(
            Some(&library),
            Some(&mut bitmap),
            (i64::from(i32::MAX) + 1) << 6,
            0,
        ),
        FT_Err_Invalid_Argument
    );
    done(&library, &mut bitmap);

    let mut bgra = owned_bitmap(1, 1, 4, FT_PIXEL_MODE_BGRA, vec![1, 2, 3, 4]);
    assert_eq!(
        FT_Bitmap_Embolden(Some(&library), Some(&mut bgra), 64, 64),
        FT_Err_Ok
    );
    assert_eq!((bgra.width, bgra.rows, bgra.pitch), (1, 1, 4));
    done(&library, &mut bgra);
}

#[test]
fn bitmap_embolden_covers_packed_lcd_and_pitch_variants() {
    let library = FT_Init_FreeType();
    let cases = [
        (
            owned_bitmap(1, 1, 1, FT_PIXEL_MODE_MONO, vec![0x80]),
            9 * 64,
            0,
            (9, 1, 2),
        ),
        (
            owned_bitmap(4, 1, 1, FT_PIXEL_MODE_GRAY2, vec![0b0001_1011]),
            64,
            0,
            (5, 1, 5),
        ),
        (
            owned_bitmap(2, 1, 1, FT_PIXEL_MODE_GRAY4, vec![0xab]),
            64,
            0,
            (3, 1, 3),
        ),
        (
            owned_bitmap(1, 1, 1, FT_PIXEL_MODE_LCD, vec![20]),
            64,
            0,
            (4, 1, 4),
        ),
        (
            owned_bitmap(1, 1, 1, FT_PIXEL_MODE_LCD_V, vec![20]),
            0,
            64,
            (1, 4, 1),
        ),
        (
            owned_bitmap(2, 1, -2, FT_PIXEL_MODE_GRAY, vec![10, 20]),
            64,
            64,
            (3, 2, -3),
        ),
    ];
    for (mut bitmap, x_strength, y_strength, expected) in cases {
        assert_eq!(
            FT_Bitmap_Embolden(Some(&library), Some(&mut bitmap), x_strength, y_strength,),
            FT_Err_Ok
        );
        assert_eq!((bitmap.width, bitmap.rows, bitmap.pitch), expected);
        assert!(FT_Bitmap_Owned_Buffer_Bytes(Some(&bitmap)).is_some());
        done(&library, &mut bitmap);
    }

    let mut invalid = owned_bitmap(1, 1, 1, FT_PIXEL_MODE_NONE, vec![1]);
    assert_eq!(
        FT_Bitmap_Embolden(Some(&library), Some(&mut invalid), 64, 0),
        FT_Err_Invalid_Glyph_Format
    );
    done(&library, &mut invalid);
}

#[test]
fn assure_buffer_and_gray_conversion_validate_owned_storage() {
    let library = FT_Init_FreeType();
    let mut padded = owned_bitmap(2, 1, 4, FT_PIXEL_MODE_GRAY, vec![1, 2, 3, 4]);
    assert_eq!(ft_bitmap_assure_buffer(&mut padded, 1, 0), FT_Err_Ok);
    assert_eq!(
        FT_Bitmap_Owned_Buffer_Bytes(Some(&padded)),
        Some(vec![1, 2, 3, 0])
    );
    done(&library, &mut padded);

    let mut missing = FT_Bitmap_C {
        rows: 1,
        width: 1,
        pitch: 1,
        pixel_mode: FT_PIXEL_MODE_GRAY as u8,
        ..FT_Bitmap_C::default()
    };
    assert_eq!(
        ft_bitmap_assure_buffer(&mut missing, 1, 0),
        FT_Err_Invalid_Argument
    );

    let mut short = owned_bitmap(2, 1, 2, FT_PIXEL_MODE_GRAY, vec![1]);
    assert_eq!(
        ft_bitmap_assure_buffer(&mut short, 1, 0),
        FT_Err_Invalid_Argument
    );
    done(&library, &mut short);

    let mut invalid = owned_bitmap(1, 1, 1, FT_PIXEL_MODE_BGRA, vec![1]);
    assert_eq!(
        ft_bitmap_assure_buffer(&mut invalid, 1, 0),
        FT_Err_Invalid_Glyph_Format
    );
    done(&library, &mut invalid);

    let mut gray2 = owned_bitmap(4, 1, 1, FT_PIXEL_MODE_GRAY2, vec![0b0001_1011]);
    assert_eq!(convert_public_bitmap_to_gray(&mut gray2, 2, 4), FT_Err_Ok);
    assert_eq!((gray2.pitch, gray2.pixel_mode, gray2.num_grays), (4, 2, 4));
    assert_eq!(
        FT_Bitmap_Owned_Buffer_Bytes(Some(&gray2)),
        Some(vec![0, 1, 2, 3])
    );
    done(&library, &mut gray2);

    let mut gray4 = owned_bitmap(2, 1, -1, FT_PIXEL_MODE_GRAY4, vec![0xab]);
    assert_eq!(convert_public_bitmap_to_gray(&mut gray4, 4, 16), FT_Err_Ok);
    assert_eq!(
        (gray4.pitch, gray4.pixel_mode, gray4.num_grays),
        (-2, 2, 16)
    );
    assert_eq!(
        FT_Bitmap_Owned_Buffer_Bytes(Some(&gray4)),
        Some(vec![10, 11])
    );
    done(&library, &mut gray4);
}

#[test]
fn gray_bitmap_helpers_cover_modes_directions_and_bounds() {
    let library = FT_Init_FreeType();
    let cases = [
        (
            owned_bitmap(4, 1, 1, FT_PIXEL_MODE_MONO, vec![0b1010_0000]),
            vec![1, 0, 1, 0],
        ),
        (
            owned_bitmap(4, 1, 1, FT_PIXEL_MODE_GRAY2, vec![0b0001_1011]),
            vec![0, 1, 2, 3],
        ),
        (
            owned_bitmap(2, 1, 1, FT_PIXEL_MODE_GRAY4, vec![0xab]),
            vec![10, 11],
        ),
        (
            owned_bitmap(3, 1, 3, FT_PIXEL_MODE_GRAY, vec![1, 2, 3]),
            vec![1, 2, 3],
        ),
        (
            owned_bitmap(3, 1, 3, FT_PIXEL_MODE_LCD, vec![4, 5, 6]),
            vec![4, 5, 6],
        ),
        (
            owned_bitmap(3, 1, 3, FT_PIXEL_MODE_LCD_V, vec![7, 8, 9]),
            vec![7, 8, 9],
        ),
        (
            owned_bitmap(1, 1, 4, FT_PIXEL_MODE_BGRA, vec![0, 0, 0, 255]),
            vec![255],
        ),
        (
            owned_bitmap(2, 2, -2, FT_PIXEL_MODE_GRAY, vec![1, 2, 3, 4]),
            vec![1, 2, 3, 4],
        ),
    ];
    for (mut bitmap, expected) in cases {
        let gray = match bitmap_to_gray(&bitmap) {
            Ok(gray) => gray,
            Err(error) => panic!("valid bitmap was rejected: {error}"),
        };
        assert_eq!(gray.bytes, expected);
        done(&library, &mut bitmap);
    }

    let positive = GrayBitmap {
        rows: 2,
        width: 2,
        pitch: 3,
        bytes: vec![0; 6],
    };
    assert_eq!(positive.row_range(0), Some(0..2));
    assert_eq!(positive.row_range(1), Some(3..5));
    assert_eq!(positive.row_range(2), None);
    let negative = GrayBitmap {
        pitch: -3,
        ..positive.clone()
    };
    assert_eq!(negative.row_range(0), Some(3..5));
    assert_eq!(negative.row_range(1), Some(0..2));
    let short = GrayBitmap {
        bytes: vec![0],
        ..positive
    };
    assert_eq!(short.row_range(0), None);

    let bitmap = FT_Bitmap_C {
        rows: 2,
        pitch: 3,
        ..FT_Bitmap_C::default()
    };
    assert_eq!(bitmap_row_start(&bitmap, 1), Some(3));
    let bitmap = FT_Bitmap_C {
        pitch: -3,
        ..bitmap
    };
    assert_eq!(bitmap_row_start(&bitmap, 0), Some(3));
    assert_eq!(bitmap_row_start(&bitmap, 2), None);

    let invalid = owned_bitmap(1, 1, 1, FT_PIXEL_MODE_NONE, vec![1]);
    assert!(matches!(
        bitmap_to_gray(&invalid),
        Err(error) if error == FT_Err_Invalid_Argument
    ));
    let mut invalid = invalid;
    done(&library, &mut invalid);
    assert!(matches!(
        bitmap_to_gray(&FT_Bitmap_C::default()),
        Err(error) if error == FT_Err_Invalid_Argument
    ));
}

#[test]
fn bitmap_blend_validates_inputs_and_composites_gray_masks() {
    let library = FT_Init_FreeType();
    let source_offset = FT_Vector { x: 0, y: 64 };
    let color = FT_Color {
        blue: 10,
        green: 20,
        red: 30,
        alpha: 255,
    };
    let source = FT_Bitmap_C::default();
    let mut target = FT_Bitmap_C::default();
    let mut target_offset = FT_Vector::default();
    assert_eq!(
        FT_Bitmap_Blend(
            None,
            Some(&source),
            source_offset,
            Some(&mut target),
            Some(&mut target_offset),
            color,
        ),
        FT_Err_Invalid_Argument
    );
    assert_eq!(
        FT_Bitmap_Blend(
            Some(&library),
            None,
            source_offset,
            Some(&mut target),
            Some(&mut target_offset),
            color,
        ),
        FT_Err_Invalid_Argument
    );

    let invalid_target = FT_Bitmap_C {
        pixel_mode: FT_PIXEL_MODE_GRAY as u8,
        ..FT_Bitmap_C::default()
    };
    target = invalid_target;
    assert_eq!(
        FT_Bitmap_Blend(
            Some(&library),
            Some(&source),
            source_offset,
            Some(&mut target),
            Some(&mut target_offset),
            color,
        ),
        FT_Err_Invalid_Argument
    );

    target = FT_Bitmap_C::default();
    assert_eq!(
        FT_Bitmap_Blend(
            Some(&library),
            Some(&source),
            source_offset,
            Some(&mut target),
            Some(&mut target_offset),
            color,
        ),
        FT_Err_Ok
    );

    let mut mask = owned_bitmap(1, 1, 1, FT_PIXEL_MODE_GRAY, vec![255]);
    assert_eq!(
        FT_Bitmap_Blend(
            Some(&library),
            Some(&mask),
            source_offset,
            Some(&mut target),
            Some(&mut target_offset),
            color,
        ),
        FT_Err_Ok
    );
    assert_eq!((target.width, target.rows, target.pitch), (1, 1, 4));
    assert_eq!(
        FT_Bitmap_Owned_Buffer_Bytes(Some(&target)),
        Some(vec![10, 20, 30, 255])
    );
    assert_eq!(target_offset, FT_Vector { x: 0, y: 64 });

    let second_color = FT_Color {
        blue: 110,
        green: 120,
        red: 130,
        alpha: 128,
    };
    assert_eq!(
        FT_Bitmap_Blend(
            Some(&library),
            Some(&mask),
            source_offset,
            Some(&mut target),
            Some(&mut target_offset),
            second_color,
        ),
        FT_Err_Ok
    );
    assert_eq!(
        FT_Bitmap_Owned_Buffer_Bytes(Some(&target)),
        Some(vec![59, 69, 79, 255])
    );
    done(&library, &mut mask);
    done(&library, &mut target);
}

#[test]
fn bitmap_blend_covers_expansion_non_gray_and_negative_pitch_routes() {
    let library = FT_Init_FreeType();
    let color = FT_Color {
        blue: 1,
        green: 2,
        red: 3,
        alpha: 255,
    };
    let mut source = owned_bitmap(1, 1, 1, FT_PIXEL_MODE_MONO, vec![0x80]);
    let mut target = owned_bitmap(1, 1, 4, FT_PIXEL_MODE_BGRA, vec![9, 8, 7, 255]);
    let mut target_offset = FT_Vector { x: 64, y: 64 };
    assert_eq!(
        FT_Bitmap_Blend(
            Some(&library),
            Some(&source),
            FT_Vector { x: 0, y: 64 },
            Some(&mut target),
            Some(&mut target_offset),
            color,
        ),
        FT_Err_Ok
    );
    assert_eq!((target.width, target.rows, target.pitch), (2, 1, 8));
    assert_eq!(
        FT_Bitmap_Owned_Buffer_Bytes(Some(&target)),
        Some(vec![0, 0, 0, 1, 9, 8, 7, 255])
    );
    done(&library, &mut source);
    done(&library, &mut target);

    let mut source = owned_bitmap(1, 1, -1, FT_PIXEL_MODE_GRAY, vec![255]);
    let mut target = owned_bitmap(1, 1, 4, FT_PIXEL_MODE_BGRA, vec![9, 8, 7, 255]);
    let mut target_offset = FT_Vector { x: 0, y: 64 };
    assert_eq!(
        FT_Bitmap_Blend(
            Some(&library),
            Some(&source),
            FT_Vector { x: 0, y: 64 },
            Some(&mut target),
            Some(&mut target_offset),
            color,
        ),
        FT_Err_Invalid_Argument
    );
    done(&library, &mut source);
    done(&library, &mut target);

    let mut source = owned_bitmap(1, 1, -1, FT_PIXEL_MODE_GRAY, vec![255]);
    let mut target = FT_Bitmap_C::default();
    let mut target_offset = FT_Vector::default();
    assert_eq!(
        FT_Bitmap_Blend(
            Some(&library),
            Some(&source),
            FT_Vector { x: 0, y: 64 },
            Some(&mut target),
            Some(&mut target_offset),
            color,
        ),
        FT_Err_Ok
    );
    assert_eq!(target.pitch, 4);
    assert_eq!(
        FT_Bitmap_Owned_Buffer_Bytes(Some(&target)),
        Some(vec![0, 0, 0, 0])
    );
    done(&library, &mut source);
    done(&library, &mut target);
}
