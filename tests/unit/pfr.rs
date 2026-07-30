use super::*;

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn push_i16(bytes: &mut Vec<u8>, value: i16) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn push_u24(bytes: &mut Vec<u8>, value: usize) {
    bytes.push(((value >> 16) & 0xff) as u8);
    bytes.push(((value >> 8) & 0xff) as u8);
    bytes.push((value & 0xff) as u8);
}

fn kerning_item(wide_characters: bool, wide_adjustment: bool) -> Vec<u8> {
    let mut item = vec![1];
    push_i16(&mut item, -10);
    item.push(
        (u8::from(wide_characters) * PFR_KERN_2BYTE_CHAR)
            | (u8::from(wide_adjustment) * PFR_KERN_2BYTE_ADJ),
    );
    if wide_characters {
        push_u16(&mut item, 65);
        push_u16(&mut item, 66);
    } else {
        item.extend_from_slice(&[65, 66]);
    }
    if wide_adjustment {
        push_i16(&mut item, -2);
    } else {
        item.push(2);
    }
    item
}

fn physical_font(flags: u8, wide_kerning: bool) -> Vec<u8> {
    let mut bytes = vec![0; 15];
    bytes[2..4].copy_from_slice(&1_000u16.to_be_bytes());
    bytes[4..6].copy_from_slice(&2_000u16.to_be_bytes());
    bytes[6..8].copy_from_slice(&(-20i16).to_be_bytes());
    bytes[8..10].copy_from_slice(&(-30i16).to_be_bytes());
    bytes[10..12].copy_from_slice(&800i16.to_be_bytes());
    bytes[12..14].copy_from_slice(&900i16.to_be_bytes());
    bytes[14] = flags;

    if flags & PFR_PHY_PROPORTIONAL == 0 {
        push_i16(&mut bytes, 700);
    }
    if flags & PFR_PHY_EXTRA_ITEMS != 0 {
        let item = kerning_item(wide_kerning, wide_kerning);
        bytes.push(1);
        bytes.push(item.len() as u8);
        bytes.push(4);
        bytes.extend_from_slice(&item);
    }

    push_u24(&mut bytes, 0);
    bytes.push(0);
    bytes.extend_from_slice(&[0; 6]);
    push_u16(&mut bytes, 2);

    for (code, advance) in [(65u16, 500i16), (66, 600)] {
        if flags & PFR_PHY_2BYTE_CHARCODE != 0 {
            push_u16(&mut bytes, code);
        } else {
            bytes.push(code as u8);
        }
        if flags & PFR_PHY_PROPORTIONAL != 0 {
            push_i16(&mut bytes, advance);
        }
        if flags & PFR_PHY_ASCII_CODE != 0 {
            bytes.push(code as u8);
        }
        if flags & PFR_PHY_2BYTE_GPS_SIZE != 0 {
            push_u16(&mut bytes, 0);
        } else {
            bytes.push(0);
        }
        if flags & PFR_PHY_3BYTE_GPS_OFFSET != 0 {
            push_u24(&mut bytes, 0);
        } else {
            push_u16(&mut bytes, 0);
        }
    }
    bytes
}

fn pfr_stream(logical_flags: u8, physical: &[u8], high_size: bool) -> Vec<u8> {
    const DIRECTORY_OFFSET: usize = PFR_HEADER_SIZE;
    const LOGICAL_OFFSET: usize = DIRECTORY_OFFSET + 7;

    let mut logical = vec![0; 13];
    logical[12] = logical_flags;
    if logical_flags & PFR_LOG_STROKE != 0 {
        logical.push(1);
        if logical_flags & PFR_LOG_2BYTE_STROKE != 0 {
            logical.push(2);
        }
        if logical_flags & PFR_LINE_JOIN_MASK == 0 {
            logical.extend_from_slice(&[0; 3]);
        }
    }
    if logical_flags & PFR_LOG_BOLD != 0 {
        logical.push(1);
        if logical_flags & PFR_LOG_2BYTE_BOLD != 0 {
            logical.push(2);
        }
    }
    if logical_flags & PFR_LOG_EXTRA_ITEMS != 0 {
        logical.extend_from_slice(&[1, 2, 9, 0xaa, 0xbb]);
    }

    let size_offset = logical.len();
    logical.extend_from_slice(&[0; 5]);
    if high_size {
        logical.push((physical.len() >> 16) as u8);
    }
    let physical_offset = LOGICAL_OFFSET + logical.len();
    logical[size_offset..size_offset + 2].copy_from_slice(&(physical.len() as u16).to_be_bytes());
    logical[size_offset + 2] = ((physical_offset >> 16) & 0xff) as u8;
    logical[size_offset + 3] = ((physical_offset >> 8) & 0xff) as u8;
    logical[size_offset + 4] = (physical_offset & 0xff) as u8;

    let mut stream = vec![0; physical_offset + physical.len()];
    stream[..4].copy_from_slice(b"PFR0");
    stream[4..6].copy_from_slice(&4u16.to_be_bytes());
    stream[6..8].copy_from_slice(&0x0D0Au16.to_be_bytes());
    stream[8..10].copy_from_slice(&(PFR_HEADER_SIZE as u16).to_be_bytes());
    stream[12..14].copy_from_slice(&(DIRECTORY_OFFSET as u16).to_be_bytes());
    stream[41] = u8::from(high_size);
    stream[DIRECTORY_OFFSET..DIRECTORY_OFFSET + 2].copy_from_slice(&1u16.to_be_bytes());
    stream[DIRECTORY_OFFSET + 2..DIRECTORY_OFFSET + 4]
        .copy_from_slice(&(logical.len() as u16).to_be_bytes());
    stream[DIRECTORY_OFFSET + 4] = ((LOGICAL_OFFSET >> 16) & 0xff) as u8;
    stream[DIRECTORY_OFFSET + 5] = ((LOGICAL_OFFSET >> 8) & 0xff) as u8;
    stream[DIRECTORY_OFFSET + 6] = (LOGICAL_OFFSET & 0xff) as u8;
    stream[LOGICAL_OFFSET..physical_offset].copy_from_slice(&logical);
    stream[physical_offset..].copy_from_slice(physical);
    stream
}

fn parsed(result: Result<PfrFont, &'static str>) -> PfrFont {
    match result {
        Ok(font) => font,
        Err(error) => panic!("valid synthetic PFR rejected: {error}"),
    }
}

#[test]
fn parses_fixed_and_fully_flagged_physical_fonts() {
    let fixed = parsed(PfrFont::parse_physical(&physical_font(0, false)));
    assert_eq!(fixed.outline_resolution, 1_000);
    assert_eq!(fixed.metrics_resolution, 2_000);
    assert_eq!(
        fixed.bbox,
        BBox {
            x_min: -20,
            y_min: -30,
            x_max: 800,
            y_max: 900,
        }
    );
    assert!(!fixed.proportional);
    assert!(!fixed.vertical);
    assert_eq!(fixed.advances, [700, 700]);
    assert_eq!(fixed.max_advance(), 700);
    assert!(!fixed.has_kerning());

    let flags = PFR_PHY_VERTICAL
        | PFR_PHY_2BYTE_CHARCODE
        | PFR_PHY_PROPORTIONAL
        | PFR_PHY_ASCII_CODE
        | PFR_PHY_2BYTE_GPS_SIZE
        | PFR_PHY_3BYTE_GPS_OFFSET
        | PFR_PHY_EXTRA_ITEMS;
    let proportional = parsed(PfrFont::parse_physical(&physical_font(flags, true)));
    assert!(proportional.proportional);
    assert!(proportional.vertical);
    assert_eq!(proportional.advances, [500, 600]);
    assert_eq!(proportional.advance(1), Some(500));
    assert_eq!(proportional.advance(2), Some(600));
    assert_eq!(proportional.advance(0), None);
    assert_eq!(proportional.advance(3), None);
    assert!(proportional.has_kerning());
    assert_eq!(proportional.kerning(1, 2), -12);
    assert_eq!(proportional.kerning(2, 1), 0);
    assert_eq!(proportional.kerning(0, 1), 0);
    assert_eq!(proportional.kerning(1, 0), 0);
    assert_eq!(proportional.kerning(3, 1), 0);
    assert_eq!(proportional.kerning(1, 3), 0);
}

#[test]
fn parses_narrow_kerning_and_ignores_unknown_extra_items() {
    let mut pairs = Vec::new();
    assert_eq!(
        parse_kerning_item(&kerning_item(false, false), &mut pairs),
        Ok(())
    );
    assert_eq!(
        pairs,
        [PfrKerningPair {
            left: 65,
            right: 66,
            adjustment: -8,
        }]
    );

    let Ok((cursor, ignored)) = parse_physical_extra_items(&[1, 1, 9, 0xaa], 0) else {
        panic!("valid unknown PFR extra item was rejected");
    };
    assert_eq!(cursor, 4);
    assert!(ignored.is_empty());

    let empty = PfrFont {
        outline_resolution: 1,
        metrics_resolution: 1,
        bbox: BBox {
            x_min: 0,
            y_min: 0,
            x_max: 0,
            y_max: 0,
        },
        proportional: false,
        vertical: false,
        advances: Vec::new(),
        char_codes: Vec::new(),
        kerning: Vec::new(),
    };
    assert_eq!(empty.max_advance(), 0);
}

#[test]
fn parses_complete_stream_and_logical_variants() {
    let physical = physical_font(0, false);
    let parsed = parsed(PfrFont::parse(&pfr_stream(0, &physical, false), 0));
    assert_eq!(parsed.advance(1), Some(700));

    let all_optional = PFR_LOG_STROKE
        | PFR_LOG_2BYTE_STROKE
        | PFR_LOG_BOLD
        | PFR_LOG_2BYTE_BOLD
        | PFR_LOG_EXTRA_ITEMS;
    assert!(PfrFont::parse(&pfr_stream(all_optional, &physical, false), 0).is_ok());
    assert!(PfrFont::parse(&pfr_stream(PFR_LOG_STROKE | 1, &physical, false), 0).is_ok());
    assert!(PfrFont::parse(&pfr_stream(PFR_LOG_BOLD, &physical, false), 0).is_ok());

    let mut padded_physical = physical;
    padded_physical.resize(1 << 16, 0);
    assert!(PfrFont::parse(&pfr_stream(0, &padded_physical, true), 0).is_ok());
}

#[test]
fn rejects_each_invalid_header_component() {
    let valid = pfr_stream(0, &physical_font(0, false), false);
    assert_eq!(PfrFont::parse(&[], 0), Err("invalid PFR header"));

    let mut invalid = valid.clone();
    invalid[0] = b'X';
    assert_eq!(PfrFont::parse(&invalid, 0), Err("invalid PFR header"));

    let mut invalid = valid.clone();
    invalid[4..6].copy_from_slice(&5u16.to_be_bytes());
    assert_eq!(PfrFont::parse(&invalid, 0), Err("invalid PFR header"));

    let mut invalid = valid.clone();
    invalid[6..8].copy_from_slice(&0u16.to_be_bytes());
    assert_eq!(PfrFont::parse(&invalid, 0), Err("invalid PFR header"));

    let mut invalid = valid;
    invalid[8..10].copy_from_slice(&57u16.to_be_bytes());
    assert_eq!(PfrFont::parse(&invalid, 0), Err("invalid PFR header"));
}

#[test]
fn rejects_invalid_logical_and_physical_ranges() {
    let valid = pfr_stream(0, &physical_font(0, false), false);
    assert_eq!(
        PfrFont::parse(&valid, 1),
        Err("PFR face index out of range")
    );

    let mut invalid = valid.clone();
    invalid[12..14].copy_from_slice(&u16::MAX.to_be_bytes());
    assert_eq!(
        PfrFont::parse(&invalid, 0),
        Err("truncated PFR logical directory")
    );

    let mut invalid = valid.clone();
    invalid[12..14].copy_from_slice(&55u16.to_be_bytes());
    invalid[55..57].copy_from_slice(&1u16.to_be_bytes());
    assert_eq!(
        PfrFont::parse(&invalid[..58], 0),
        Err("truncated PFR logical directory entry")
    );

    let mut invalid = valid.clone();
    invalid[PFR_HEADER_SIZE + 4..PFR_HEADER_SIZE + 7].copy_from_slice(&[0xff; 3]);
    assert_eq!(
        PfrFont::parse(&invalid, 0),
        Err("PFR logical font outside stream")
    );

    let mut invalid = valid.clone();
    invalid[PFR_HEADER_SIZE + 2..PFR_HEADER_SIZE + 4].copy_from_slice(&17u16.to_be_bytes());
    assert_eq!(
        PfrFont::parse(&invalid, 0),
        Err("truncated PFR logical font")
    );

    let logical_offset = PFR_HEADER_SIZE + 7;
    let mut invalid = valid;
    invalid[logical_offset + 13..logical_offset + 15].copy_from_slice(&u16::MAX.to_be_bytes());
    assert_eq!(
        PfrFont::parse(&invalid, 0),
        Err("PFR physical font outside stream")
    );

    let mut invalid = pfr_stream(0, &physical_font(0, false), true);
    let logical_size =
        u16::from_be_bytes([invalid[PFR_HEADER_SIZE + 2], invalid[PFR_HEADER_SIZE + 3]]) as usize;
    invalid[PFR_HEADER_SIZE + 2..PFR_HEADER_SIZE + 4]
        .copy_from_slice(&((logical_size - 1) as u16).to_be_bytes());
    assert_eq!(
        PfrFont::parse(&invalid, 0),
        Err("truncated high PFR physical font size")
    );
}

#[test]
fn rejects_truncated_physical_variants_and_zero_fields() {
    let variants = [
        physical_font(0, false),
        physical_font(
            PFR_PHY_PROPORTIONAL
                | PFR_PHY_2BYTE_CHARCODE
                | PFR_PHY_ASCII_CODE
                | PFR_PHY_2BYTE_GPS_SIZE
                | PFR_PHY_3BYTE_GPS_OFFSET
                | PFR_PHY_EXTRA_ITEMS,
            true,
        ),
    ];
    for physical in variants {
        for prefix_len in 0..physical.len() {
            assert!(
                PfrFont::parse_physical(&physical[..prefix_len]).is_err(),
                "prefix {prefix_len} of {} bytes unexpectedly parsed",
                physical.len()
            );
        }
    }

    let mut invalid = physical_font(0, false);
    invalid[2..4].copy_from_slice(&0u16.to_be_bytes());
    assert_eq!(
        PfrFont::parse_physical(&invalid),
        Err("invalid zero PFR resolution")
    );

    let mut invalid = physical_font(0, false);
    invalid[4..6].copy_from_slice(&0u16.to_be_bytes());
    assert_eq!(
        PfrFont::parse_physical(&invalid),
        Err("invalid zero PFR resolution")
    );

    let mut invalid = physical_font(0, false);
    invalid[28..30].copy_from_slice(&0u16.to_be_bytes());
    assert_eq!(
        PfrFont::parse_physical(&invalid),
        Err("PFR physical font has no characters")
    );
}

#[test]
fn rejects_malformed_extra_items_and_kerning_pairs() {
    assert_eq!(
        parse_physical_extra_items(&[], 0),
        Err("missing PFR physical extra-item count")
    );
    assert_eq!(
        parse_physical_extra_items(&[1], 0),
        Err("truncated PFR physical extra item")
    );
    assert_eq!(
        parse_physical_extra_items(&[1, 1], 0),
        Err("truncated PFR physical extra item")
    );
    assert_eq!(
        parse_physical_extra_items(&[1, 2, 9, 0], 0),
        Err("PFR extra item outside physical font")
    );
    assert_eq!(
        parse_physical_extra_items(&[1, 3, 4, 0, 0, 0], 0),
        Err("truncated PFR kerning item")
    );

    assert_eq!(
        parse_kerning_item(&[], &mut Vec::new()),
        Err("truncated PFR kerning item")
    );
    for item in [kerning_item(false, false), kerning_item(true, true)] {
        for prefix_len in 4..item.len() {
            assert!(parse_kerning_item(&item[..prefix_len], &mut Vec::new()).is_err());
        }
    }
}

#[test]
fn validates_logical_extra_items_and_primitive_bounds() {
    assert_eq!(
        skip_extra_items(&[], 0),
        Err("missing PFR extra-item count")
    );
    assert_eq!(skip_extra_items(&[1], 0), Err("truncated PFR extra item"));
    assert_eq!(
        skip_extra_items(&[1, 2, 9, 0], 0),
        Err("PFR extra item outside table")
    );
    assert_eq!(skip_extra_items(&[1, 1, 9, 0], 0), Ok(4));

    assert_eq!(checked_skip(usize::MAX, 1), Err("PFR cursor overflow"));
    assert_eq!(range(&[1, 2], usize::MAX, 2), None);
    assert_eq!(be_u16(&[1], 0), None);
    assert_eq!(be_i16(&[1], 0), None);
    assert_eq!(be_u24(&[1, 2], 0), None);
}
