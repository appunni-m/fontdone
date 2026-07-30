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
