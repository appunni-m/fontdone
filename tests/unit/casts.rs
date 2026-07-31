use super::*;

#[test]
fn checked_narrowing_invariants_accept_boundaries() {
    assert_eq!(i32_from_i64(i64::from(i32::MIN)), i32::MIN);
    assert_eq!(i32_from_i64(i64::from(i32::MAX)), i32::MAX);
    assert_eq!(usize_from_i64(0), 0);
    #[cfg(target_pointer_width = "64")]
    assert_eq!(usize_from_i64(i64::MAX), 9_223_372_036_854_775_807usize);
    #[cfg(target_pointer_width = "32")]
    assert_eq!(usize_from_i64(i64::from(i32::MAX)), 2_147_483_647usize);
}

#[test]
fn checked_narrowing_invariants_reject_out_of_range_values() {
    assert!(std::panic::catch_unwind(|| i32_from_i64(i64::MIN)).is_err());
    assert!(std::panic::catch_unwind(|| i32_from_i64(i64::MAX)).is_err());
    assert!(std::panic::catch_unwind(|| usize_from_i64(-1)).is_err());
    #[cfg(target_pointer_width = "32")]
    assert!(std::panic::catch_unwind(|| usize_from_i64(i64::MAX)).is_err());
}

#[test]
fn infallible_conversions_cover_all_wrappers() {
    assert_eq!(i16_from_i32(0x7FFF), i16::MAX);
    assert_eq!(i16_from_i32(0xFFFF), -1);
    assert_eq!(usize_from_i32(5), 5);
    assert_eq!(i32_from_usize(7), 7);
    assert_eq!(u64_from_i64(-1), u64::MAX);
    assert_eq!(u64_from_i64(0), 0);
    assert_eq!(u32_from_usize(9), 9);
    assert_eq!(u16_from_u32(0xFFFF), u16::MAX);
    assert_eq!(u32_from_i32(1), 1);
    assert_eq!(u16_from_i16(1), 1);
    assert_eq!(i32_from_u64(u64::MAX), -1);
    assert_eq!(u32_from_i64(1), 1);
    assert_eq!(u8_from_i32(255), 255);
    assert_eq!(i32_from_f32(12.5), 12);
    assert_eq!(i32_from_f32(-3.7), -3);
    assert_eq!(i32_from_f32(0.0), 0);
}
