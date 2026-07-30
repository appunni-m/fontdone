use super::*;

fn put_u16(bytes: &mut [u8], offset: usize, value: u16) {
    bytes[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
}

fn put_u32(bytes: &mut [u8], offset: usize, value: u32) {
    bytes[offset..offset + 4].copy_from_slice(&value.to_be_bytes());
}

fn append_store(bytes: &mut Vec<u8>) {
    let start = bytes.len();
    bytes.resize(start + 32, 0);
    put_u16(bytes, start, 1);
    put_u32(bytes, start + 2, 12);
    put_u16(bytes, start + 6, 1);
    put_u32(bytes, start + 8, 22);
    put_u16(bytes, start + 12, 1);
    put_u16(bytes, start + 14, 1);
    put_u16(bytes, start + 18, 0x4000);
    put_u16(bytes, start + 20, 0x4000);
    put_u16(bytes, start + 22, 1);
    put_u16(bytes, start + 24, 1);
    put_u16(bytes, start + 26, 1);
    put_u16(bytes, start + 28, 0);
    put_u16(bytes, start + 30, 7);
}

#[test]
fn maps_every_supported_vertical_metric_tag() -> Result<(), FontError> {
    let tags = [
        TAG_VASC,
        TAG_VDSC,
        TAG_VLGP,
        TAG_VCRS,
        TAG_VCRN,
        TAG_VCOF,
        u32::from_be_bytes(*b"xxxx"),
    ];
    let store_offset = 12 + tags.len() * 8;
    let mut table = vec![0; store_offset];
    put_u16(&mut table, 0, 1);
    put_u16(&mut table, 6, 8);
    put_u16(&mut table, 8, tags.len() as u16);
    put_u16(&mut table, 10, store_offset as u16);
    for (index, tag) in tags.into_iter().enumerate() {
        let offset = 12 + index * 8;
        put_u32(&mut table, offset, tag);
    }
    append_store(&mut table);

    let parsed = MvarTable::parse(&table, 1)?;
    assert_eq!(
        parsed.vertical_header_deltas(&[0x4000]),
        VerticalHeaderDeltas {
            ascender: 7,
            descender: 7,
            line_gap: 7,
            caret_slope_rise: 7,
            caret_slope_run: 7,
            caret_offset: 7,
        }
    );
    Ok(())
}

#[test]
fn rejects_short_unsupported_and_malformed_value_records() {
    assert!(MvarTable::parse(&[], 1).is_err());

    let mut unsupported = vec![0; 12];
    put_u16(&mut unsupported, 0, 2);
    assert!(MvarTable::parse(&unsupported, 1).is_err());

    let mut short_record = vec![0; 12];
    put_u16(&mut short_record, 0, 1);
    put_u16(&mut short_record, 6, 7);
    assert!(MvarTable::parse(&short_record, 1).is_err());

    let mut truncated = vec![0; 12];
    put_u16(&mut truncated, 0, 1);
    put_u16(&mut truncated, 6, 8);
    put_u16(&mut truncated, 8, 1);
    put_u16(&mut truncated, 10, 12);
    assert!(MvarTable::parse(&truncated, 1).is_err());
    assert!(read_u16(&[], 0).is_err());
    assert!(read_u32(&[0, 0, 0], 0).is_err());
}
