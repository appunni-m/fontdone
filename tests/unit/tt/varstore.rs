use super::*;

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn region(start: i32, peak: i32, end: i32) -> VariationRegion {
    VariationRegion {
        axes: vec![RegionAxis { start, peak, end }],
    }
}

fn store_with_data(data: ItemVariationData, regions: Vec<VariationRegion>) -> ItemVariationStore {
    ItemVariationStore {
        axis_count: 1,
        regions,
        data: vec![data],
    }
}

fn minimal_store_bytes() -> Vec<u8> {
    let mut bytes = vec![0; 30];
    put_u16(&mut bytes, 0, 1);
    put_u32(&mut bytes, 2, 12);
    put_u16(&mut bytes, 6, 1);
    put_u32(&mut bytes, 8, 22);

    put_u16(&mut bytes, 12, 1);
    put_u16(&mut bytes, 14, 1);
    put_u16(&mut bytes, 16, 0);
    put_u16(&mut bytes, 18, 0x4000);
    put_u16(&mut bytes, 20, 0x4000);

    put_u16(&mut bytes, 22, 1);
    put_u16(&mut bytes, 24, 1);
    put_u16(&mut bytes, 26, 1);
    put_u16(&mut bytes, 28, 0);
    bytes.extend_from_slice(&7i16.to_be_bytes());
    bytes
}

#[test]
fn item_delta_covers_guards_and_short_delta_widths() {
    let short = store_with_data(
        ItemVariationData {
            item_count: 1,
            word_delta_count: 1,
            long_words: false,
            region_indices: vec![0, 1],
            delta_set: [10i16.to_be_bytes().as_slice(), &[-2i8 as u8]].concat(),
        },
        vec![region(0, 0, 0), region(0, 0, 0)],
    );
    assert_eq!(short.item_delta(0xFFFF, 0xFFFF, &[0]), 0);
    assert_eq!(short.item_delta(1, 0, &[0]), 0);
    assert_eq!(short.item_delta(0, 1, &[0]), 0);
    assert_eq!(short.item_delta(0, 0, &[]), 0);
    assert_eq!(short.item_delta(0, 0, &[0]), 8);

    let no_regions = store_with_data(
        ItemVariationData {
            item_count: 1,
            word_delta_count: 0,
            long_words: false,
            region_indices: Vec::new(),
            delta_set: Vec::new(),
        },
        Vec::new(),
    );
    assert_eq!(no_regions.item_delta(0, 0, &[0]), 0);
}

#[test]
fn item_delta_covers_long_words_and_missing_regions() {
    let long = store_with_data(
        ItemVariationData {
            item_count: 1,
            word_delta_count: 1,
            long_words: true,
            region_indices: vec![0, 1, 9],
            delta_set: [
                100_000i32.to_be_bytes().as_slice(),
                (-2i16).to_be_bytes().as_slice(),
                3i16.to_be_bytes().as_slice(),
            ]
            .concat(),
        },
        vec![region(0, 0, 0), region(0, 0, 0)],
    );
    assert_eq!(long.item_delta(0, 0, &[0]), 99_998);
}

#[test]
fn region_scalar_covers_peak_bounds_and_both_slopes() {
    assert_eq!(region(0, 0, 0).scalar(&[123]), 0x1_0000);
    assert_eq!(region(0, 0x1_0000, 0x2_0000).scalar(&[0x4000]), 0x1_0000);
    assert_eq!(region(0, 0x1_0000, 0x2_0000).scalar(&[0]), 0);
    assert_eq!(region(0, 0x1_0000, 0x2_0000).scalar(&[0x2000]), 0x8000);
    assert_eq!(region(0, 0x1_0000, 0x2_0000).scalar(&[0x6000]), 0x8000);
    assert_eq!(mul_div_round(1, 1, 0), i32::MAX);
    assert_eq!(mul_div_round(-2, 3, 2), -3);
    assert_eq!(mul_div_round(2, -3, 2), -3);
    assert_eq!(mul_div_round(2, 3, -2), -3);
}

#[test]
fn parser_accepts_short_and_long_word_delta_sets() -> Result<(), FontError> {
    let bytes = minimal_store_bytes();
    let parsed = ItemVariationStore::parse(&bytes, 0, 1)?;
    assert_eq!(parsed.item_delta(0, 0, &[0x4000]), 7);

    let mut long = bytes;
    put_u16(&mut long, 24, 0x8001);
    long.truncate(30);
    long.extend_from_slice(&70_000i32.to_be_bytes());
    let parsed = ItemVariationStore::parse(&long, 0, 1)?;
    assert_eq!(parsed.item_delta(0, 0, &[0x4000]), 70_000);
    Ok(())
}

#[test]
fn parser_rejects_invalid_store_headers_regions_and_delta_counts() {
    let mut bytes = minimal_store_bytes();
    put_u16(&mut bytes, 0, 2);
    assert!(ItemVariationStore::parse(&bytes, 0, 1).is_err());

    let mut bytes = minimal_store_bytes();
    put_u16(&mut bytes, 6, 0);
    assert!(ItemVariationStore::parse(&bytes, 0, 1).is_err());

    let bytes = minimal_store_bytes();
    assert!(ItemVariationStore::parse(&bytes, 0, 2).is_err());

    let mut bytes = minimal_store_bytes();
    put_u16(&mut bytes, 14, 0x8000);
    assert!(ItemVariationStore::parse(&bytes, 0, 1).is_err());

    let mut bytes = minimal_store_bytes();
    put_u16(&mut bytes, 24, 2);
    assert!(ItemVariationStore::parse(&bytes, 0, 1).is_err());

    let mut bytes = minimal_store_bytes();
    put_u16(&mut bytes, 28, 1);
    assert!(ItemVariationStore::parse(&bytes, 0, 1).is_err());

    let mut bytes = minimal_store_bytes();
    bytes.truncate(31);
    assert!(ItemVariationStore::parse(&bytes, 0, 1).is_err());
}

#[test]
fn parser_normalizes_invalid_region_axis_order() -> Result<(), FontError> {
    let mut bytes = minimal_store_bytes();
    put_u16(&mut bytes, 16, 0xC000);
    put_u16(&mut bytes, 18, 0x2000);
    put_u16(&mut bytes, 20, 0x4000);
    let parsed = ItemVariationStore::parse(&bytes, 0, 1)?;
    assert_eq!(parsed.item_delta(0, 0, &[0x2000]), 7);
    Ok(())
}

#[test]
fn delta_set_maps_cover_formats_sentinel_clamping_and_errors() -> Result<(), FontError> {
    let store = ItemVariationStore::parse(&minimal_store_bytes(), 0, 1)?;

    let format_zero = [0, 0, 0, 2, 0, 0];
    let map = DeltaSetIndexMap::parse(&format_zero, 0, &store)?;
    assert_eq!(map.get(0), Some((0, 0)));
    assert_eq!(map.get(99), Some((0, 0)));

    let format_one = [1, 0, 0, 0, 0, 1, 0];
    let map = DeltaSetIndexMap::parse(&format_one, 0, &store)?;
    assert_eq!(map.get(0), Some((0, 0)));

    let sentinel = [0, 0x3F, 0, 1, 0xFF, 0xFF, 0xFF, 0xFF];
    let map = DeltaSetIndexMap::parse(&sentinel, 0, &store)?;
    assert_eq!(map.get(0), Some((0xFFFF, 0xFFFF)));

    assert_eq!(
        DeltaSetIndexMap {
            entries: Vec::new()
        }
        .get(0),
        None
    );
    for invalid in [
        &[][..],
        &[0][..],
        &[2, 0, 0, 0][..],
        &[0, 0xC0, 0, 0][..],
        &[0, 0, 0, 1][..],
        &[0, 0, 0, 1, 2][..],
    ] {
        assert!(DeltaSetIndexMap::parse(invalid, 0, &store).is_err());
    }
    Ok(())
}

#[test]
fn primitive_readers_reject_out_of_range_offsets() {
    assert!(read_u16(&[], 0).is_err());
    assert!(read_i16(&[0], 0).is_err());
    assert!(read_i32(&[0, 0, 0], 0).is_err());
    assert!(read_u32(&[0, 0, 0], 0).is_err());
}
