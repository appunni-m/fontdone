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
fn parses_default_and_explicit_advance_maps() -> Result<(), FontError> {
    let mut table = vec![0; 12];
    put_u16(&mut table, 0, 1);
    put_u32(&mut table, 4, 12);
    append_store(&mut table);

    let parsed = HvarTable::parse(&table, 1)?;
    assert_eq!(parsed.advance_delta(0, &[0x4000]), 7);
    assert_eq!(parsed.advance_delta(1, &[0x4000]), 0);

    let map_offset = table.len();
    table.extend_from_slice(&[0, 0, 0, 1, 0]);
    put_u32(&mut table, 8, map_offset as u32);
    let parsed = HvarTable::parse(&table, 1)?;
    assert_eq!(parsed.advance_delta(9, &[0x4000]), 7);
    Ok(())
}

#[test]
fn rejects_short_unsupported_and_truncated_hvar_tables() {
    assert!(HvarTable::parse(&[], 1).is_err());

    let mut unsupported = vec![0; 12];
    put_u16(&mut unsupported, 0, 2);
    assert!(HvarTable::parse(&unsupported, 1).is_err());

    let mut missing_store = vec![0; 12];
    put_u16(&mut missing_store, 0, 1);
    put_u32(&mut missing_store, 4, 12);
    assert!(HvarTable::parse(&missing_store, 1).is_err());
    assert!(read_u16(&[], 0).is_err());
    assert!(read_u32(&[0, 0, 0], 0).is_err());
}
