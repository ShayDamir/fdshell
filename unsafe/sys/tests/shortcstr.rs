#![allow(clippy::unwrap_used)]
#![allow(clippy::indexing_slicing)]

use core::ffi::CStr;
use sys::ShortCStr;

/// A long (≥100 byte) static CStr for subslicing tests.
const LONG: &CStr = c"The quick brown fox jumps over the lazy dog. \
    Pack my box with five dozen liquor jugs. \
    How vexingly quick daft zebras jump!";

// --- verify ---

#[test]
fn verify_inline() {
    assert!(ShortCStr::from_static(c"hello").verify().is_ok());
}

#[test]
fn verify_static() {
    assert!(ShortCStr::from_static(LONG).verify().is_ok());
}

#[test]
fn verify_rc() {
    let s = ShortCStr::from_bytes(b"hello world this is more than thirty bytes total").unwrap();
    assert!(s.verify().is_ok());
}

// --- from_bytes boundaries ---

#[test]
fn from_bytes_empty() {
    let s = ShortCStr::from_bytes(b"").unwrap();
    assert!(s.is_empty());
    assert!(s.verify().is_ok());
}

#[test]
fn from_bytes_inline_max() {
    let bytes = b"123456789012345678901234567890"; // 30 bytes
    assert_eq!(bytes.len(), 30);
    let s = ShortCStr::from_bytes(bytes).unwrap();
    assert_eq!(s.as_bytes(), bytes);
    assert!(s.verify().is_ok());
}

#[test]
fn from_bytes_first_rc() {
    let bytes = b"1234567890123456789012345678901"; // 31 bytes
    assert_eq!(bytes.len(), 31);
    let s = ShortCStr::from_bytes(bytes).unwrap();
    assert_eq!(s.as_bytes(), bytes);
    assert!(s.verify().is_ok());
}

#[test]
fn from_bytes_interior_nul() {
    let result = ShortCStr::from_bytes(b"ab\0cd");
    assert!(result.is_err());
}

// --- from_static ---

#[test]
fn from_static_empty() {
    let s = ShortCStr::from_static(c"");
    assert!(s.is_empty());
    assert!(s.verify().is_ok());
}

#[test]
fn from_static_single() {
    let s = ShortCStr::from_static(c"x");
    assert_eq!(s.as_bytes(), b"x");
    assert!(s.verify().is_ok());
}

#[test]
fn from_static_long() {
    let s = ShortCStr::from_static(LONG);
    assert_eq!(s.as_bytes(), LONG.to_bytes());
    assert!(s.verify().is_ok());
}

// --- get OOB ---

#[test]
fn get_oob_range_from() {
    let s = ShortCStr::from_static(c"hi");
    assert!(s.get(999..).is_none());
}

#[test]
fn get_oob_range() {
    let s = ShortCStr::from_static(c"hi");
    assert!(s.get(5..10).is_none());
}

// --- get zero-length ---

#[test]
fn get_zero_len_start() {
    let s = ShortCStr::from_static(c"hello");
    let sub = s.get(0..0).unwrap();
    assert!(sub.is_empty());
    assert!(sub.verify().is_ok());
}

#[test]
fn get_zero_len_mid() {
    let s = ShortCStr::from_static(c"hello");
    let sub = s.get(3..3).unwrap();
    assert!(sub.is_empty());
    assert!(sub.verify().is_ok());
}

// --- get preserves variant on tail ---

#[test]
fn get_full_static_preserves_variant() {
    let s = ShortCStr::from_static(LONG);
    let sub = s.get(..).unwrap();
    assert_eq!(sub.as_bytes(), LONG.to_bytes());
    assert!(sub.verify().is_ok());
}

#[test]
fn get_full_rc_preserves_variant() {
    let raw = b"hello world this is more than thirty bytes total";
    let s = ShortCStr::from_bytes(raw).unwrap();
    let sub = s.get(..).unwrap();
    assert_eq!(sub.as_bytes(), raw);
    assert!(sub.verify().is_ok());
}

// --- existing subslice tests (ported) ---

#[test]
fn inline_subslice_tail() {
    let s = ShortCStr::from_static(c"hello world");
    let sub = s.get(6..11).unwrap();
    assert_eq!(sub.as_bytes(), b"world");
    assert!(sub.verify().is_ok());
}

#[test]
fn inline_subslice_mid() {
    let s = ShortCStr::from_static(c"hello world");
    let sub = s.get(1..5).unwrap();
    assert_eq!(sub.as_bytes(), b"ello");
    assert!(sub.verify().is_ok());
}

#[test]
fn static_tail_subslice() {
    let s = ShortCStr::from_static(LONG);
    let full = s.len();
    let sub = s.get(100..full).unwrap();
    assert_eq!(sub.as_bytes(), &LONG.to_bytes()[100..full]);
    assert!(sub.verify().is_ok());
}

#[test]
fn static_short_mid_subslice() {
    let s = ShortCStr::from_static(LONG);
    let sub = s.get(10..30).unwrap();
    assert_eq!(sub.as_bytes(), &LONG.to_bytes()[10..30]);
    assert!(sub.verify().is_ok());
}

#[test]
fn rc_tail_subslice() {
    let raw = b"hello world this is a long string over thirty bytes";
    let s = ShortCStr::from_bytes(raw).unwrap();
    let full = s.len();
    let sub = s.get(10..full).unwrap();
    assert_eq!(sub.as_bytes(), &raw[10..full]);
    assert!(sub.verify().is_ok());
}

#[test]
fn rc_short_mid_subslice() {
    let s = ShortCStr::from_bytes(b"hello world this is more than thirty bytes total").unwrap();
    let sub = s.get(6..20).unwrap();
    assert_eq!(sub.as_bytes(), b"world this is ");
    assert!(sub.verify().is_ok());
}

#[test]
fn static_long_non_tail() {
    let s = ShortCStr::from_static(LONG);
    let full = LONG.to_bytes();
    let sub = s.get(10..60).unwrap();
    assert_eq!(sub.as_bytes(), &full[10..60]);
    assert!(sub.verify().is_ok());
}

#[test]
fn rc_long_non_tail() {
    let raw = b"hello world this is a long string over thirty bytes for sure";
    let s = ShortCStr::from_bytes(raw).unwrap();
    let sub = s.get(10..55).unwrap();
    assert_eq!(sub.as_bytes(), &raw[10..55]);
    assert!(sub.verify().is_ok());
}

// --- range type variants ---

#[test]
fn get_range_from() {
    let s = ShortCStr::from_static(c"hello world");
    let sub = s.get(6..).unwrap();
    assert_eq!(sub.as_bytes(), b"world");
    assert!(sub.verify().is_ok());
}

#[test]
fn get_range_to() {
    let s = ShortCStr::from_static(c"hello world");
    let sub = s.get(..5).unwrap();
    assert_eq!(sub.as_bytes(), b"hello");
    assert!(sub.verify().is_ok());
}

#[test]
fn get_range_full() {
    let s = ShortCStr::from_static(c"hello");
    let sub = s.get(..).unwrap();
    assert_eq!(sub.as_bytes(), b"hello");
    assert!(sub.verify().is_ok());
}

#[test]
fn get_range_inclusive() {
    let s = ShortCStr::from_static(c"hello world");
    let sub = s.get(0..=4).unwrap();
    assert_eq!(sub.as_bytes(), b"hello");
    assert!(sub.verify().is_ok());
}

#[test]
fn get_non_tail_range_from() {
    let s = ShortCStr::from_static(LONG);
    let sub = s.get(10..).unwrap();
    assert_eq!(sub.as_bytes(), &LONG.to_bytes()[10..]);
    assert!(sub.verify().is_ok());
}

#[test]
fn get_non_tail_range_to() {
    let s = ShortCStr::from_static(c"hello world");
    let sub = s.get(..3).unwrap();
    assert_eq!(sub.as_bytes(), b"hel");
    assert!(sub.verify().is_ok());
}

// --- as_c_str consistency ---

#[test]
fn as_c_str_matches_as_bytes_inline() {
    let s = ShortCStr::from_static(c"hello");
    assert_eq!(s.as_c_str().to_bytes(), s.as_bytes());
}

#[test]
fn as_c_str_matches_as_bytes_static() {
    let s = ShortCStr::from_static(LONG);
    assert_eq!(s.as_c_str().to_bytes(), s.as_bytes());
}

#[test]
fn as_c_str_matches_as_bytes_rc() {
    let s = ShortCStr::from_bytes(b"hello world this is more than thirty bytes total").unwrap();
    assert_eq!(s.as_c_str().to_bytes(), s.as_bytes());
}

// --- to_c_string correctness ---

#[test]
fn to_c_string_matches_inline() {
    let s = ShortCStr::from_static(c"hello");
    assert_eq!(s.to_c_string().to_bytes(), s.as_bytes());
}

#[test]
fn to_c_string_matches_static() {
    let s = ShortCStr::from_static(LONG);
    assert_eq!(s.to_c_string().to_bytes(), s.as_bytes());
}

#[test]
fn to_c_string_matches_rc() {
    let s = ShortCStr::from_bytes(b"hello world this is more than thirty bytes total").unwrap();
    assert_eq!(s.to_c_string().to_bytes(), s.as_bytes());
}

// --- len / is_empty ---

#[test]
fn len_empty() {
    let s = ShortCStr::from_static(c"");
    assert_eq!(s.len(), 0);
    assert!(s.is_empty());
}

#[test]
fn len_variants() {
    let rc_bytes = b"hello world this is more than thirty bytes total";
    assert_eq!(ShortCStr::from_static(c"hi").len(), 2);
    assert_eq!(ShortCStr::from_static(c"hello").len(), 5);
    assert_eq!(ShortCStr::from_static(LONG).len(), LONG.to_bytes().len());
    let rc = ShortCStr::from_bytes(rc_bytes).unwrap();
    assert_eq!(rc.len(), rc_bytes.len());
}

// --- Clone + PartialEq ---

#[test]
fn clone_equals_original() {
    for src in &[
        ShortCStr::from_static(c""),
        ShortCStr::from_static(c"hello"),
        ShortCStr::from_static(LONG),
        ShortCStr::from_bytes(b"hello world this is more than thirty bytes total").unwrap(),
    ] {
        assert_eq!(src.clone(), *src);
    }
}

#[test]
fn cross_variant_equal() {
    let a = ShortCStr::from_static(c"hello");
    let b = ShortCStr::from_bytes(b"hello").unwrap();
    let c = ShortCStr::from_static(c"hello"); // Static variant
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_eq!(b, c);
}

#[test]
fn different_content_not_equal() {
    let a = ShortCStr::from_static(c"hello");
    let b = ShortCStr::from_static(c"world");
    assert_ne!(a, b);
}

// --- Hash consistency ---

#[test]
fn hash_consistent_across_variants() {
    use core::hash::{Hash, Hasher};
    let a = ShortCStr::from_static(c"hello");
    let b = ShortCStr::from_bytes(b"hello").unwrap();
    let mut ha = std::collections::hash_map::DefaultHasher::new();
    let mut hb = std::collections::hash_map::DefaultHasher::new();
    a.hash(&mut ha);
    b.hash(&mut hb);
    assert_eq!(ha.finish(), hb.finish());
}

// --- cross-variant equality (from_static vs from_bytes) ---

#[test]
fn static_equals_from_bytes() {
    let s = ShortCStr::from_static(c"hello");
    let b = ShortCStr::from_bytes(b"hello").unwrap();
    assert_eq!(s, b);
}

// --- split_once_byte ---

#[test]
fn split_once_mid() {
    let s = ShortCStr::from_static(c"foo=bar");
    let (l, r) = s.split_once_byte(b'=').unwrap();
    assert_eq!(l.as_bytes(), b"foo");
    assert_eq!(r.as_bytes(), b"bar");
    assert!(l.verify().is_ok());
    assert!(r.verify().is_ok());
}

#[test]
fn split_once_start() {
    let s = ShortCStr::from_static(c"=bar");
    let (l, r) = s.split_once_byte(b'=').unwrap();
    assert_eq!(l.as_bytes(), b"");
    assert_eq!(r.as_bytes(), b"bar");
    assert!(l.verify().is_ok());
    assert!(r.verify().is_ok());
}

#[test]
fn split_once_end() {
    let s = ShortCStr::from_static(c"foo=");
    let (l, r) = s.split_once_byte(b'=').unwrap();
    assert_eq!(l.as_bytes(), b"foo");
    assert_eq!(r.as_bytes(), b"");
    assert!(l.verify().is_ok());
    assert!(r.verify().is_ok());
}

#[test]
fn split_once_none() {
    let s = ShortCStr::from_static(c"foobar");
    assert!(s.split_once_byte(b'=').is_none());
}

#[test]
fn split_once_empty() {
    let s = ShortCStr::from_static(c"");
    assert!(s.split_once_byte(b'=').is_none());
}

#[test]
fn split_once_long() {
    let s = ShortCStr::from_static(LONG);
    assert!(s.split_once_byte(b'=').is_none());
}

// --- strip_prefix ---

#[test]
fn strip_prefix_match_full() {
    let s = ShortCStr::from_static(c"hello world");
    let r = s.strip_prefix(b"hello").unwrap();
    assert_eq!(r.as_bytes(), b" world");
    assert!(r.verify().is_ok());
}

#[test]
fn strip_prefix_partial() {
    let s = ShortCStr::from_static(c"hello");
    let r = s.strip_prefix(b"he").unwrap();
    assert_eq!(r.as_bytes(), b"llo");
    assert!(r.verify().is_ok());
}

#[test]
fn strip_prefix_no_match() {
    let s = ShortCStr::from_static(c"hello");
    assert!(s.strip_prefix(b"x").is_none());
}

#[test]
fn strip_prefix_empty() {
    let s = ShortCStr::from_static(c"hello");
    let r = s.strip_prefix(b"").unwrap();
    assert_eq!(r.as_bytes(), b"hello");
    assert!(r.verify().is_ok());
}

#[test]
fn strip_prefix_all() {
    let s = ShortCStr::from_static(c"hello");
    let r = s.strip_prefix(b"hello").unwrap();
    assert_eq!(r.as_bytes(), b"");
    assert!(r.verify().is_ok());
}

#[test]
fn strip_prefix_percent() {
    let s = ShortCStr::from_static(c"%foo");
    let r = s.strip_prefix(b"%").unwrap();
    assert_eq!(r.as_bytes(), b"foo");
    assert!(r.verify().is_ok());
}

#[test]
fn strip_prefix_long() {
    let s = ShortCStr::from_static(LONG);
    let prefix = b"The quick ";
    let r = s.strip_prefix(prefix).unwrap();
    assert_eq!(r.as_bytes(), &LONG.to_bytes()[prefix.len()..]);
    assert!(r.verify().is_ok());
}
