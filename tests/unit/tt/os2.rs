use super::*;

#[test]
fn wws_flag_requires_a_real_os2_version() {
    let mut bytes = vec![0; 100];
    bytes[62..64].copy_from_slice(&256u16.to_be_bytes());

    let versioned = parse_os2(&bytes).unwrap_or_else(|| unreachable!());
    assert!(versioned.is_wws_only());

    bytes[0..2].copy_from_slice(&0xFFFFu16.to_be_bytes());
    let absent = parse_os2(&bytes).unwrap_or_else(|| unreachable!());
    assert!(!absent.is_wws_only());
}
