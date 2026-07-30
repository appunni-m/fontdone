use super::*;
use crate::tt::glyf::{GlyphOutline, OutlinePoint};

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn minimal_gvar(axis_count: u16, glyph_count: u16) -> Vec<u8> {
    let offset_count = usize::from(glyph_count) + 1;
    let mut bytes = vec![0; 20 + offset_count * 2];
    put_u16(&mut bytes, 0, 1);
    put_u16(&mut bytes, 4, axis_count);
    put_u32(&mut bytes, 8, 20);
    put_u16(&mut bytes, 12, glyph_count);
    let data_offset = bytes.len() as u32;
    put_u32(&mut bytes, 16, data_offset);
    bytes
}

fn embedded_tuple_glyph(tuple_index_flags: u16, tuple_data: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&1u16.to_be_bytes());
    bytes.extend_from_slice(&10u16.to_be_bytes());
    bytes.extend_from_slice(&(tuple_data.len() as u16).to_be_bytes());
    bytes.extend_from_slice(&(EMBEDDED_PEAK_TUPLE | tuple_index_flags).to_be_bytes());
    bytes.extend_from_slice(&(F2DOT14_ONE as i16).to_be_bytes());
    bytes.extend_from_slice(tuple_data);
    bytes
}

fn one_axis_table(glyph_data: Vec<u8>) -> GvarTable {
    GvarTable {
        axis_count: 1,
        shared_tuples: Vec::new(),
        glyph_offsets: vec![0, glyph_data.len() as u32],
        data_offset: 0,
        data: glyph_data,
    }
}

#[test]
fn axis_normalization_covers_clamps_interpolation_and_degenerate_extents() {
    assert_eq!(normalize_axis_coord(0, -100, 0, 100), 0);
    assert_eq!(normalize_axis_coord(-200, -100, 0, 100), -0x4000);
    assert_eq!(normalize_axis_coord(-50, -100, 0, 100), -0x2000);
    assert_eq!(normalize_axis_coord(50, -100, 0, 100), 0x2000);
    assert_eq!(normalize_axis_coord(200, -100, 0, 100), 0x4000);
    assert_eq!(normalize_axis_coord(1, 0, 0, 0), 0x4000);
    assert_eq!(normalize_axis_delta(10, 0), 0);
    assert_eq!(normalize_axis_delta(10, -1), 0);
}

#[test]
fn tuple_scalar_covers_default_and_intermediate_regions() {
    assert_eq!(tuple_scalar(&[0], None, &[123]), F16DOT16_ONE);
    assert_eq!(tuple_scalar(&[0x4000], None, &[0]), 0);
    assert_eq!(tuple_scalar(&[0x4000], None, &[-1]), 0);
    assert_eq!(tuple_scalar(&[0x4000], None, &[0x4000]), F16DOT16_ONE);
    assert_eq!(tuple_scalar(&[0x4000], None, &[0x2000]), 0x8000);
    assert_eq!(
        tuple_scalar(&[0x2000], Some(&(vec![0], vec![0x4000])), &[0x3000],),
        0x8000
    );
    assert_eq!(div_to_fixed(1, 0), 0);
    assert_eq!(div_to_fixed(1, 2), 0x8000);
}

#[test]
fn point_number_decoder_covers_byte_word_and_all_point_runs() -> Result<(), FontError> {
    assert_eq!(read_point_numbers(&[0], 3)?, (vec![0, 1, 2], 1));
    assert_eq!(read_point_numbers(&[2, 1, 3, 4], 8)?, (vec![3, 7], 4));
    assert_eq!(
        read_point_numbers(&[2, 0x81, 0, 3, 0, 4], 8)?,
        (vec![3, 7], 6)
    );
    assert_eq!(read_point_numbers(&[0x80, 1, 0, 5], 8)?, (vec![5], 4));
    Ok(())
}

#[test]
fn point_number_decoder_rejects_every_truncation_boundary() {
    for bytes in [
        &[][..],
        &[0x80][..],
        &[1][..],
        &[1, 0][..],
        &[1, 0x80, 0][..],
    ] {
        assert!(read_point_numbers(bytes, 8).is_err());
    }
}

#[test]
fn packed_delta_decoder_covers_zero_word_byte_and_truncated_runs() -> Result<(), FontError> {
    assert_eq!(read_packed_deltas(&[0x81], 2)?, (vec![0, 0], 1));
    assert_eq!(
        read_packed_deltas(&[0x41, 0xFF, 0xFE, 0, 3], 2)?,
        (vec![-2, 3], 5)
    );
    assert_eq!(read_packed_deltas(&[1, 0xFE, 3], 2)?, (vec![-2, 3], 3));
    for bytes in [&[][..], &[0x40, 0][..], &[0][..]] {
        assert!(read_packed_deltas(bytes, 1).is_err());
    }
    Ok(())
}

#[test]
fn parser_accepts_both_offset_widths_and_rejects_header_contracts() -> Result<(), FontError> {
    let short_offsets = minimal_gvar(1, 1);
    let parsed = parse_gvar(&short_offsets, 1)?;
    assert!(parsed.glyph_deltas(0, 1, &[0])?.is_none());

    let mut long_offsets = vec![0; 28];
    put_u16(&mut long_offsets, 0, 1);
    put_u16(&mut long_offsets, 4, 1);
    put_u32(&mut long_offsets, 8, 28);
    put_u16(&mut long_offsets, 12, 1);
    put_u16(&mut long_offsets, 14, 1);
    put_u32(&mut long_offsets, 16, 28);
    let parsed = parse_gvar(&long_offsets, 1)?;
    assert!(parsed.glyph_deltas(0, 1, &[0])?.is_none());

    assert!(parse_gvar(&[], 0).is_err());
    let mut bad_version = minimal_gvar(0, 0);
    put_u16(&mut bad_version, 0, 2);
    assert!(parse_gvar(&bad_version, 0).is_err());
    let mismatch = minimal_gvar(0, 1);
    assert!(parse_gvar(&mismatch, 2).is_err());
    Ok(())
}

#[test]
fn glyph_delta_evaluation_covers_guards_shared_and_private_points() -> Result<(), FontError> {
    let table = one_axis_table(embedded_tuple_glyph(0, &[1, 1, 2, 0x81]));
    assert!(table.glyph_deltas_fixed(0, 2, &[])?.is_none());
    assert!(table.glyph_deltas_fixed(1, 2, &[0x4000])?.is_none());
    assert_eq!(
        table.glyph_deltas(0, 2, &[0x4000])?,
        Some(vec![(1, 0), (2, 0)])
    );
    assert_eq!(
        table.glyph_deltas_fixed(0, 2, &[0])?,
        Some(vec![(0, 0), (0, 0)])
    );

    let private = one_axis_table(embedded_tuple_glyph(
        PRIVATE_POINT_NUMBERS,
        &[1, 0, 1, 0, 7, 0x80],
    ));
    assert_eq!(
        private.glyph_deltas(0, 2, &[0x4000])?,
        Some(vec![(0, 0), (7, 0)])
    );

    let shared_index = GvarTable {
        axis_count: 1,
        shared_tuples: Vec::new(),
        glyph_offsets: vec![0, 8],
        data_offset: 0,
        data: vec![0, 1, 0, 8, 0, 0, 0, 0],
    };
    assert!(shared_index.glyph_deltas(0, 1, &[0x4000]).is_err());
    Ok(())
}

#[test]
fn glyph_delta_evaluation_rejects_malformed_glyph_records() {
    for glyph_data in [
        vec![0, 0, 0],
        vec![0, 1, 0, 9, 0, 0, 0, 0],
        vec![0, 1, 0, 4, 0, 0, 0, 0],
        vec![0, 1, 0, 8, 0, 4, 0x80, 0],
    ] {
        let table = one_axis_table(glyph_data);
        assert!(table.glyph_deltas_fixed(0, 1, &[0x4000]).is_err());
    }

    let out_of_range = GvarTable {
        axis_count: 1,
        shared_tuples: Vec::new(),
        glyph_offsets: vec![2, 8],
        data_offset: 4,
        data: vec![0; 8],
    };
    assert!(out_of_range.glyph_deltas_fixed(0, 1, &[0x4000]).is_err());
}

#[test]
fn outline_delta_helpers_recompute_bounds_and_preserve_fractional_sidecar() {
    let mut empty = GlyphOutline::default();
    apply_deltas_to_outline(&mut empty, &[]);
    assert_eq!(
        (
            empty.xmin,
            empty.ymin,
            empty.xmax,
            empty.ymax,
            empty.bbox_xmin
        ),
        (0, 0, 0, 0, 0)
    );

    let mut outline = GlyphOutline {
        points: vec![
            OutlinePoint {
                x: 5,
                y: 10,
                on_curve: true,
                tag: 1,
            },
            OutlinePoint {
                x: 20,
                y: -5,
                on_curve: false,
                tag: 0,
            },
        ],
        ..GlyphOutline::default()
    };
    apply_deltas_to_outline(&mut outline, &[(-10, 2), (3, 4)]);
    assert_eq!(
        (
            outline.xmin,
            outline.ymin,
            outline.xmax,
            outline.ymax,
            outline.bbox_xmin,
        ),
        (-5, -1, 23, 12, -5)
    );

    apply_fixed_deltas_to_outline(&mut outline, &[(0x8000, -0x8000), (0, 0)]);
    assert!(outline.unrounded_points.is_some());
    assert_eq!(fixed_to_int(0x8000), 1);
    assert_eq!(fixed_to_fdot6(0x8000), 32);
}
