#![allow(clippy::unwrap_used)]
#![cfg_attr(test, allow(clippy::indexing_slicing))]

use core::ffi::CStr;
use std::sync::Arc;
use sys::ShortCStr;

/// A long (≥100 byte) static CStr for subslicing tests.
const LONG: &CStr = c"The quick brown fox jumps over the lazy dog. \
    Pack my box with five dozen liquor jugs. \
    How vexingly quick daft zebras jump!";

// --- from_vec boundaries ---

#[test]
fn from_vec_empty() {
    let s = ShortCStr::new();
    assert!(s.is_empty());
}

#[test]
fn from_vec_inline_max() {
    let bytes = b"123456789012345678901234567890"; // 30 bytes
    assert_eq!(bytes.len(), 30);
    let s = ShortCStr::from_vec(bytes.to_vec()).unwrap();
    assert_eq!(s.as_bytes().unwrap(), bytes);
}

#[test]
fn from_vec_first_rc() {
    let bytes = b"1234567890123456789012345678901"; // 31 bytes
    assert_eq!(bytes.len(), 31);
    let s = ShortCStr::from_vec(bytes.to_vec()).unwrap();
    assert_eq!(s.as_bytes().unwrap(), bytes);
}

#[test]
fn from_vec_interior_nul() {
    let result = ShortCStr::from_vec(b"ab\0cd".to_vec());
    assert!(result.is_err());
}

// --- from_static ---

#[test]
fn from_static_empty() {
    let s = ShortCStr::from(c"");
    assert!(s.is_empty());
}

#[test]
fn from_static_single() {
    let s = ShortCStr::from(c"x");
    assert_eq!(s.as_bytes().unwrap(), b"x");
}

#[test]
fn from_static_long() {
    let s = ShortCStr::from(LONG);
    assert_eq!(s.as_bytes().unwrap(), LONG.to_bytes());
}

// --- get OOB ---

#[test]
fn get_oob_range_from() {
    let s = ShortCStr::from(c"hi");
    assert!(s.get(999..).is_none());
}

#[test]
fn get_oob_range() {
    let s = ShortCStr::from(c"hi");
    assert!(s.get(5..10).is_none());
}

// --- get zero-length ---

#[test]
fn get_zero_len_start() {
    let s = ShortCStr::from(c"hello");
    let sub = s.get(0..0).unwrap();
    assert!(sub.is_empty());
}

#[test]
fn get_zero_len_mid() {
    let s = ShortCStr::from(c"hello");
    let sub = s.get(3..3).unwrap();
    assert!(sub.is_empty());
}

// --- get preserves variant on tail ---

#[test]
fn get_full_static_preserves_variant() {
    let s = ShortCStr::from(LONG);
    let sub = s.get(..).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), LONG.to_bytes());
}

#[test]
fn get_full_rc_preserves_variant() {
    let raw = b"hello world this is more than thirty bytes total";
    let s = ShortCStr::from_vec(raw.to_vec()).unwrap();
    let sub = s.get(..).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), raw);
}

// --- existing subslice tests (ported) ---

#[test]
fn inline_subslice_tail() {
    let s = ShortCStr::from(c"hello world");
    let sub = s.get(6..11).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), b"world");
}

#[test]
fn inline_subslice_mid() {
    let s = ShortCStr::from(c"hello world");
    let sub = s.get(1..5).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), b"ello");
}

#[test]
fn static_tail_subslice() {
    let s = ShortCStr::from(LONG);
    let full = s.len();
    let sub = s.get(100..full).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), &LONG.to_bytes()[100..full]);
}

#[test]
fn static_short_mid_subslice() {
    let s = ShortCStr::from(LONG);
    let sub = s.get(10..30).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), &LONG.to_bytes()[10..30]);
}

#[test]
fn rc_tail_subslice() {
    let raw = b"hello world this is a long string over thirty bytes";
    let s = ShortCStr::from_vec(raw.to_vec()).unwrap();
    let full = s.len();
    let sub = s.get(10..full).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), &raw[10..full]);
}

#[test]
fn rc_short_mid_subslice() {
    let s: ShortCStr = c"hello world this is more than thirty bytes total".into();
    let sub = s.get(6..20).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), b"world this is ");
}

#[test]
fn static_long_non_tail() {
    let s = ShortCStr::from(LONG);
    let full = LONG.to_bytes();
    let sub = s.get(10..60).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), &full[10..60]);
}

#[test]
fn rc_long_non_tail() {
    let raw = b"hello world this is a long string over thirty bytes for sure";
    let s = ShortCStr::from_vec(raw.to_vec()).unwrap();
    let sub = s.get(10..55).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), &raw[10..55]);
}

// --- range type variants ---

#[test]
fn get_range_from() {
    let s = ShortCStr::from(c"hello world");
    let sub = s.get(6..).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), b"world");
}

#[test]
fn get_range_to() {
    let s = ShortCStr::from(c"hello world");
    let sub = s.get(..5).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), b"hello");
}

#[test]
fn get_range_full() {
    let s = ShortCStr::from(c"hello");
    let sub = s.get(..).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), b"hello");
}

#[test]
fn get_range_inclusive() {
    let s = ShortCStr::from(c"hello world");
    let sub = s.get(0..=4).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), b"hello");
}

#[test]
fn get_non_tail_range_from() {
    let s = ShortCStr::from(LONG);
    let sub = s.get(10..).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), &LONG.to_bytes()[10..]);
}

#[test]
fn get_non_tail_range_to() {
    let s = ShortCStr::from(c"hello world");
    let sub = s.get(..3).unwrap();
    assert_eq!(sub.as_bytes().unwrap(), b"hel");
}

// --- ExportedCstr ---

#[test]
fn exported_cstr_matches_inline() {
    let s = ShortCStr::from(c"hello");
    assert_eq!(s.export().as_ref().to_bytes(), s.as_bytes().unwrap());
}

#[test]
fn exported_cstr_matches_static() {
    let s = ShortCStr::from(LONG);
    assert_eq!(s.export().as_ref().to_bytes(), s.as_bytes().unwrap());
}

#[test]
fn exported_cstr_matches_rc() {
    let s: ShortCStr = c"hello world this is more than thirty bytes total".into();
    assert_eq!(s.export().as_ref().to_bytes(), s.as_bytes().unwrap());
}

#[test]
fn exported_cstr_from_ref() {
    use sys::ExportedCStr;
    let s = ShortCStr::from(c"hello");
    assert_eq!(
        ExportedCStr::from(&s).as_ref().to_bytes(),
        s.as_bytes().unwrap()
    );
}

#[test]
fn exported_cstr_from_shortcstr() {
    use sys::ExportedCStr;
    let s = ShortCStr::from(c"hello");
    assert_eq!(ExportedCStr::from(s).as_ref().to_bytes(), b"hello");
}

// --- len / is_empty ---

#[test]
fn len_empty() {
    let s = ShortCStr::from(c"");
    assert_eq!(s.len(), 0);
    assert!(s.is_empty());
}

#[test]
fn len_variants() {
    let rc_bytes = b"hello world this is more than thirty bytes total";
    assert_eq!(ShortCStr::from(c"hi").len(), 2);
    assert_eq!(ShortCStr::from(c"hello").len(), 5);
    assert_eq!(ShortCStr::from(LONG).len(), LONG.to_bytes().len());
    let rc = ShortCStr::from_vec(rc_bytes.to_vec()).unwrap();
    assert_eq!(rc.len(), rc_bytes.len());
}

// --- Clone + PartialEq ---

#[test]
fn clone_equals_original() {
    for src in &[
        ShortCStr::from(c""),
        ShortCStr::from(c"hello"),
        ShortCStr::from(LONG),
        c"hello world this is more than thirty bytes total".into(),
    ] {
        assert_eq!(src.clone(), *src);
    }
}

#[test]
fn cross_variant_equal() {
    let a = ShortCStr::from(c"hello");
    let b: ShortCStr = c"hello".into();
    let c = ShortCStr::from(c"hello"); // Static variant
    assert_eq!(a, b);
    assert_eq!(a, c);
    assert_eq!(b, c);
}

#[test]
fn different_content_not_equal() {
    let a = ShortCStr::from(c"hello");
    let b = ShortCStr::from(c"world");
    assert_ne!(a, b);
}

// --- Hash consistency ---

#[test]
fn hash_consistent_across_variants() {
    use core::hash::{Hash, Hasher};
    let a = ShortCStr::from(c"hello");
    let b: ShortCStr = c"hello".into();
    let mut ha = std::collections::hash_map::DefaultHasher::new();
    let mut hb = std::collections::hash_map::DefaultHasher::new();
    a.hash(&mut ha);
    b.hash(&mut hb);
    assert_eq!(ha.finish(), hb.finish());
}

// --- cross-variant equality (from_static vs from_vec) ---

#[test]
fn static_equals_from_vec() {
    let s = ShortCStr::from(c"hello");
    let b: ShortCStr = c"hello".into();
    assert_eq!(s, b);
}

// --- split_once_byte ---

#[test]
fn split_once_byte_mid() {
    let s = ShortCStr::from(c"foo=bar");
    let (l, r) = s.split_once_byte(b'=').unwrap();
    assert_eq!(l.as_bytes().unwrap(), b"foo");
    assert_eq!(r.as_bytes().unwrap(), b"bar");
}

#[test]
fn split_once_byte_start() {
    let s = ShortCStr::from(c"=bar");
    let (l, r) = s.split_once_byte(b'=').unwrap();
    assert_eq!(l.as_bytes().unwrap(), b"");
    assert_eq!(r.as_bytes().unwrap(), b"bar");
}

#[test]
fn split_once_byte_end() {
    let s = ShortCStr::from(c"foo=");
    let (l, r) = s.split_once_byte(b'=').unwrap();
    assert_eq!(l.as_bytes().unwrap(), b"foo");
    assert_eq!(r.as_bytes().unwrap(), b"");
}

#[test]
fn split_once_byte_none() {
    let s = ShortCStr::from(c"foobar");
    assert!(s.split_once_byte(b'=').is_none());
}

#[test]
fn split_once_byte_empty() {
    let s = ShortCStr::from(c"");
    assert!(s.split_once_byte(b'=').is_none());
}

#[test]
fn split_once_byte_long() {
    let s = ShortCStr::from(LONG);
    assert!(s.split_once_byte(b'=').is_none());
}

// --- split_once ---

#[test]
fn split_once_mid() {
    let s = ShortCStr::from(c"foo=bar");
    let (l, r) = s.split_once(b"=").unwrap();
    assert_eq!(l.as_bytes().unwrap(), b"foo");
    assert_eq!(r.as_bytes().unwrap(), b"bar");
}

#[test]
fn split_once_start() {
    let s = ShortCStr::from(c"=bar");
    let (l, r) = s.split_once(b"=").unwrap();
    assert_eq!(l.as_bytes().unwrap(), b"");
    assert_eq!(r.as_bytes().unwrap(), b"bar");
}

#[test]
fn split_once_end() {
    let s = ShortCStr::from(c"foo=");
    let (l, r) = s.split_once(b"=").unwrap();
    assert_eq!(l.as_bytes().unwrap(), b"foo");
    assert_eq!(r.as_bytes().unwrap(), b"");
}

#[test]
fn split_once_none() {
    let s = ShortCStr::from(c"foobar");
    assert!(s.split_once(b"=").is_none());
}

#[test]
fn split_once_empty_sep() {
    let s = ShortCStr::from(c"foo=bar");
    assert!(s.split_once(b"").is_none());
}

#[test]
fn split_once_empty_data() {
    let s = ShortCStr::from(c"");
    assert!(s.split_once(b"=").is_none());
}

#[test]
fn split_once_longer_than_data() {
    let s = ShortCStr::from(c"ab");
    assert!(s.split_once(b"abc").is_none());
}

#[test]
fn split_once_repeated() {
    let s = ShortCStr::from(c"a=b=c");
    let (l, r) = s.split_once(b"=").unwrap();
    assert_eq!(l.as_bytes().unwrap(), b"a");
    assert_eq!(r.as_bytes().unwrap(), b"b=c");
}

#[test]
fn split_once_multibyte_sep() {
    let s = ShortCStr::from(c"Umask:\t0022");
    let (l, r) = s.split_once(b"Umask:\t").unwrap();
    assert_eq!(l.as_bytes().unwrap(), b"");
    assert_eq!(r.as_bytes().unwrap(), b"0022");
}

#[test]
fn split_once_static() {
    let s = ShortCStr::from(LONG);
    let (l, r) = s.split_once(b"lazy dog.").unwrap();
    assert_eq!(
        l.as_bytes().unwrap(),
        b"The quick brown fox jumps over the "
    );
    assert_eq!(
        r.as_bytes().unwrap(),
        b" Pack my box with five dozen liquor jugs. How vexingly quick daft zebras jump!"
    );
}

#[test]
fn split_once_arc() {
    let bytes = b"hello=world this is a longer string that goes to arc";
    let s = ShortCStr::from_vec(bytes.to_vec()).unwrap();
    let (l, r) = s.split_once(b"=").unwrap();
    assert_eq!(l.as_bytes().unwrap(), b"hello");
    assert_eq!(
        r.as_bytes().unwrap(),
        b"world this is a longer string that goes to arc"
    );
}

#[test]
fn split_once_inline() {
    let bytes = b"foo=bar";
    let s = ShortCStr::from_vec(bytes.to_vec()).unwrap();
    let (l, r) = s.split_once(b"=").unwrap();
    assert_eq!(l.as_bytes().unwrap(), b"foo");
    assert_eq!(r.as_bytes().unwrap(), b"bar");
}

#[test]
fn split_once_preserves_static_variant() {
    let s = ShortCStr::from(LONG);
    let (l, r) = s.split_once(b"lazy").unwrap();
    assert_eq!(
        l.as_bytes().unwrap(),
        b"The quick brown fox jumps over the "
    );
    assert_eq!(
        r.as_bytes().unwrap(),
        b" dog. Pack my box with five dozen liquor jugs. How vexingly quick daft zebras jump!"
    );
}

// --- strip_prefix ---

#[test]
fn strip_prefix_match_full() {
    let s = ShortCStr::from(c"hello world");
    let r = s.strip_prefix(b"hello").unwrap();
    assert_eq!(r.as_bytes().unwrap(), b" world");
}

#[test]
fn strip_prefix_partial() {
    let s = ShortCStr::from(c"hello");
    let r = s.strip_prefix(b"he").unwrap();
    assert_eq!(r.as_bytes().unwrap(), b"llo");
}

#[test]
fn strip_prefix_no_match() {
    let s = ShortCStr::from(c"hello");
    assert!(s.strip_prefix(b"x").is_none());
}

#[test]
fn strip_prefix_empty() {
    let s = ShortCStr::from(c"hello");
    let r = s.strip_prefix(b"").unwrap();
    assert_eq!(r.as_bytes().unwrap(), b"hello");
}

#[test]
fn strip_prefix_all() {
    let s = ShortCStr::from(c"hello");
    let r = s.strip_prefix(b"hello").unwrap();
    assert_eq!(r.as_bytes().unwrap(), b"");
}

#[test]
fn strip_prefix_percent() {
    let s = ShortCStr::from(c"%foo");
    let r = s.strip_prefix(b"%").unwrap();
    assert_eq!(r.as_bytes().unwrap(), b"foo");
}

#[test]
fn strip_prefix_long() {
    let s = ShortCStr::from(LONG);
    let prefix = b"The quick ";
    let r = s.strip_prefix(prefix).unwrap();
    assert_eq!(r.as_bytes().unwrap(), &LONG.to_bytes()[prefix.len()..]);
}

// --- new() ---

#[test]
fn new_is_empty() {
    let s = ShortCStr::new();
    assert!(s.is_empty());
    assert_eq!(s.len(), 0);
}

// --- push / extend_from_slice_unchecked ---

#[test]
fn push_up_to_inline_cap() {
    let mut s = ShortCStr::new();
    for (i, &b) in b"abcdefghijklmnopqrstuvwxyzABCD".iter().enumerate() {
        s.push(b).unwrap();
        assert_eq!(s.len(), i + 1);
    }
    assert_eq!(s.as_bytes().unwrap(), b"abcdefghijklmnopqrstuvwxyzABCD");
}

#[test]
fn push_overflows_to_rc() {
    let mut s = ShortCStr::new();
    let payload = b"123456789012345678901234567890!";
    for &b in payload.iter() {
        s.push(b).unwrap();
    }
    assert_eq!(s.as_bytes().unwrap(), payload);
    // should be Arc variant now (31 bytes > INLINE_CAP)
    assert!(s.len() == 31);
}

#[test]
fn push_nul_returns_err() {
    let mut s = ShortCStr::new();
    s.push(b'a').unwrap();
    assert!(s.push(b'\0').is_err());
    // content unchanged
    assert_eq!(s.as_bytes().unwrap(), b"a");
}

#[test]
fn extend_after_rc_mid_subslice() {
    let s: ShortCStr = c"hello world this is more than thirty bytes".into();
    let sub = s.get(6..11).unwrap();
    // sub is an Arc non-tail view → extend_from_slice_unchecked copies
    let mut sub = sub.clone();
    // SAFETY: Inline variant has capacity; Arc variant copies via copy_to_shortcstr.
    unsafe { sub.extend_from_slice_unchecked(b"!") };
    assert_eq!(sub.as_bytes().unwrap(), b"world!");
}

#[test]
fn extend_rc_tail_growth() {
    let raw = b"hello world this is more than thirty bytes";
    let s = ShortCStr::from_vec(raw.to_vec()).unwrap();
    let tail = s.get(6..).unwrap();
    assert_eq!(tail.as_bytes().unwrap(), &raw[6..]);
    let mut tail = tail.clone();
    // SAFETY: Arc tail variant has capacity for one more byte.
    unsafe { tail.extend_from_slice_unchecked(b"!") };
    let mut expected = raw[6..].to_vec();
    expected.push(b'!');
    assert_eq!(tail.as_bytes().unwrap(), &expected);
}

#[test]
fn extend_static_non_tail_rc_copy() {
    // non-tail subslice with n >= INLINE_CAP → case 5 pushes via copy_to_shortcstr
    let s = ShortCStr::from(LONG);
    let sub = s.get(10..50).unwrap(); // 40 bytes > 30 → stays Static
    let mut sub = sub.clone();
    // SAFETY: copies via copy_to_shortcstr into Arc variant.
    unsafe { sub.extend_from_slice_unchecked(b"!") };
    let mut expected = LONG.to_bytes()[10..50].to_vec();
    expected.push(b'!');
    assert_eq!(sub.as_bytes().unwrap(), &expected);
}

#[test]
fn exported_cstr_from_static_non_tail() {
    let s = ShortCStr::from(LONG);
    let sub = s.get(10..50).unwrap();
    assert_eq!(sub.export().as_ref().to_bytes(), &LONG.to_bytes()[10..50]);
}

#[test]
fn exported_cstr_from_static_non_tail_inline() {
    // short non-tail → extend_from_slice_unchecked(&[0]) copies into Inline (case 3)
    let s = ShortCStr::from(c"hello world");
    let sub = s.get(6..).unwrap(); // "world" = 5 bytes ≤ INLINE_CAP
    assert_eq!(sub.export().as_ref().to_bytes(), b"world");
}

#[test]
fn exported_cstr_from_short_non_tail() {
    // short non-tail → ExportedCStr::from appends NUL on Inline (case 1)
    let s = ShortCStr::from(c"hello world");
    let sub = s.get(6..).unwrap();
    assert_eq!(sub.export().as_ref().to_bytes(), b"world");
}

#[test]
fn extend_static_tail_stays_static() {
    // tail subslice > INLINE_CAP → case 2: extend_from_slice_unchecked(&[0]) is no-op
    let s = ShortCStr::from(LONG);
    let tail = s.get(60..).unwrap();
    let len_before = tail.len();
    let mut cloned = tail.clone();
    // SAFETY: Static tail variant with capacity; extend_from_slice_unchecked(&[0]) is no-op here.
    unsafe { cloned.extend_from_slice_unchecked(&[0]) };
    assert_eq!(cloned.len(), len_before);
    assert_eq!(cloned.as_bytes().unwrap(), &LONG.to_bytes()[60..]);
}
// --- ends_with ---

#[test]
fn ends_with_matches() {
    let s = ShortCStr::from(c"hello world");
    assert!(s.ends_with(b"world"));
    assert!(s.ends_with(b""));
    assert!(!s.ends_with(b"hello"));
}

#[test]
fn ends_with_rc() {
    let s: ShortCStr = c"hello world this is long".into();
    assert!(s.ends_with(b"long"));
    assert!(!s.ends_with(b"short"));
}

#[test]
fn contains_found() {
    let s = ShortCStr::from(c"hello world");
    assert!(s.contains(b'o'));
    assert!(s.contains(b'h'));
    assert!(!s.contains(b'z'));
}

#[test]
fn extend_copy_to_inline_via_constructed_arc() {
    // Arc non-tail view < INLINE_CAP → case 3 → copy_to_shortcstr inline path
    let v = Arc::new(b"hello world, this is more than thirty bytes long".to_vec());
    let s = ShortCStr::Arc {
        arc: v,
        offset: 0,
        length: 5,
    };
    let mut s = s;
    // SAFETY: byte is not NUL
    unsafe { s.extend_from_slice_unchecked(b"!") };
    assert_eq!(s.as_bytes().unwrap(), b"hello!");
}

#[test]
fn extend_copy_to_inline_via_constructed_static() {
    // Static non-tail view < INLINE_CAP → case 3 → copy_to_shortcstr inline path
    let s = ShortCStr::Static(c"hello world, this is more than thirty bytes long", 0, 5);
    let mut s = s;
    // SAFETY: byte is not NUL
    unsafe { s.extend_from_slice_unchecked(b"!") };
    assert_eq!(s.as_bytes().unwrap(), b"hello!");
}

#[test]
fn debug_fmt_inline() {
    let s: ShortCStr = c"hello".into();
    assert_eq!(format!("{:?}", s), "\"hello\"");
}

#[test]
fn debug_fmt_static() {
    let s = ShortCStr::from(c"hello");
    assert_eq!(format!("{:?}", s), "\"hello\"");
}

#[test]
fn debug_fmt_arc() {
    let v = Arc::new(b"hello world, this is more than thirty bytes long".to_vec());
    let s = ShortCStr::Arc {
        arc: v,
        offset: 0,
        length: 5,
    };
    assert_eq!(format!("{:?}", s), "\"hello\"");
}

#[test]
fn debug_fmt_invalid() {
    let s = ShortCStr::Arc {
        arc: Arc::new(b"hi".to_vec()),
        offset: 0,
        length: 100,
    };
    assert_eq!(format!("{:?}", s), "\"<BadState>\"");
}

#[test]
fn partial_eq_as_bytes_err() {
    let valid = ShortCStr::from(c"hello");
    let invalid = ShortCStr::Arc {
        arc: Arc::new(b"hi".to_vec()),
        offset: 0,
        length: 100,
    };
    assert_ne!(valid, invalid);
}

// --- fmt::Write ---

#[test]
fn fmt_write_simple() {
    let mut s = ShortCStr::new();
    core::fmt::write(&mut s, format_args!("{}", 42)).unwrap();
    assert!(s.eq_bytes(b"42"));
}

#[test]
fn fmt_write_composite() {
    let mut s = ShortCStr::new();
    core::fmt::write(&mut s, format_args!("{} + {} = {}", 1, 2, 3)).unwrap();
    assert!(s.eq_bytes(b"1 + 2 = 3"));
}

#[test]
fn fmt_write_sequential() {
    let mut s = ShortCStr::new();
    use core::fmt::Write;
    s.write_str("hello").unwrap();
    s.write_str(" ").unwrap();
    s.write_str("world").unwrap();
    assert!(s.eq_bytes(b"hello world"));
}

#[test]
fn fmt_write_overflows_inline() {
    let mut s = ShortCStr::new();
    core::fmt::write(
        &mut s,
        format_args!("{}", "123456789012345678901234567890!"),
    )
    .unwrap();
    assert!(s.eq_bytes(b"123456789012345678901234567890!"));
}

#[test]
fn fmt_write_nul_fails() {
    let mut s = ShortCStr::new();
    use core::fmt::Write;
    assert!(s.write_str("a\0b").is_err());
    assert!(s.eq_bytes(b""));
}

// --- format!() macro ---

#[test]
fn format_simple() {
    let result = sys::format!("hello {}", "world").unwrap();
    assert!(result.eq_bytes(b"hello world"));
}

#[test]
fn format_composite() {
    let result = sys::format!("{} + {} = {}", 1, 2, 3).unwrap();
    assert!(result.eq_bytes(b"1 + 2 = 3"));
}

#[test]
fn format_inline_max() {
    let result = sys::format!("{}", "123456789012345678901234567890").unwrap();
    assert!(result.eq_bytes(b"123456789012345678901234567890"));
}

#[test]
fn format_overflows_to_arc() {
    let result = sys::format!("{}", "123456789012345678901234567890!").unwrap();
    assert!(result.eq_bytes(b"123456789012345678901234567890!"));
}

#[test]
fn format_empty() {
    let result = sys::format!("").unwrap();
    assert!(result.is_empty());
}

#[test]
fn format_nul_err() {
    let result = sys::format!("{}", "a\0b");
    assert!(result.is_err());
}

// --- write!() macro ---

#[test]
fn write_simple() {
    let mut buf = ShortCStr::from(c"hello");
    sys::write!(buf, " {}", "world").unwrap();
    assert!(buf.eq_bytes(b"hello world"));
}

#[test]
fn write_composite() {
    let mut buf = ShortCStr::new();
    sys::write!(buf, "{} + {} = {}", 1, 2, 3).unwrap();
    assert!(buf.eq_bytes(b"1 + 2 = 3"));
}

#[test]
fn write_multiple() {
    let mut buf = ShortCStr::new();
    sys::write!(buf, "hello").unwrap();
    sys::write!(buf, " ").unwrap();
    sys::write!(buf, "world").unwrap();
    assert!(buf.eq_bytes(b"hello world"));
}

#[test]
fn write_overflows_to_arc() {
    let mut buf = ShortCStr::from(c"start");
    sys::write!(buf, " {}", "123456789012345678901234567890!").unwrap();
    assert!(buf.eq_bytes(b"start 123456789012345678901234567890!"));
}

#[test]
fn write_nul_err() {
    let mut buf = ShortCStr::from(c"hello");
    let result = sys::write!(buf, "\0world");
    assert!(result.is_err());
    assert!(buf.eq_bytes(b"hello"));
}

// --- concat ---

#[test]
fn concat_empty() {
    let parts: Vec<ShortCStr> = vec![];
    let result = ShortCStr::concat(&parts.iter().collect::<Vec<_>>()).unwrap();
    assert!(result.is_empty());
}

#[test]
fn concat_single() {
    let s = ShortCStr::from(c"hello");
    let result = ShortCStr::concat(&[&s]).unwrap();
    assert_eq!(result.as_bytes().unwrap(), b"hello");
}

#[test]
fn concat_two_inline() {
    let a = ShortCStr::from(c"hello");
    let b = ShortCStr::from(c" world");
    let result = ShortCStr::concat(&[&a, &b]).unwrap();
    assert_eq!(result.as_bytes().unwrap(), b"hello world");
}

#[test]
fn concat_three_inline() {
    let a = ShortCStr::from(c"foo");
    let b = ShortCStr::from(c"=");
    let c = ShortCStr::from(c"bar");
    let result = ShortCStr::concat(&[&a, &b, &c]).unwrap();
    assert_eq!(result.as_bytes().unwrap(), b"foo=bar");
}

#[test]
fn concat_overflows_to_arc() {
    let a = ShortCStr::from_vec(b"123456789012345678901234567890".to_vec()).unwrap(); // 30 bytes
    let b = ShortCStr::from(c"!");
    let result = ShortCStr::concat(&[&a, &b]).unwrap();
    assert_eq!(
        result.as_bytes().unwrap(),
        b"123456789012345678901234567890!"
    );
}

#[test]
fn concat_static_and_inline() {
    let static_s = ShortCStr::from(c"static");
    let inline_s: ShortCStr = c"hello".into();
    let result = ShortCStr::concat(&[&static_s, &inline_s]).unwrap();
    assert_eq!(result.as_bytes().unwrap(), b"statichello");
}

#[test]
fn concat_with_empty() {
    let a = ShortCStr::from(c"hello");
    let empty = ShortCStr::new();
    let result = ShortCStr::concat(&[&a, &empty]).unwrap();
    assert_eq!(result.as_bytes().unwrap(), b"hello");
}

#[test]
fn concat_empty_parts() {
    let empty1 = ShortCStr::new();
    let empty2 = ShortCStr::new();
    let result = ShortCStr::concat(&[&empty1, &empty2]).unwrap();
    assert!(result.is_empty());
}

#[test]
fn concat_nul_err() {
    // Construct a ShortCStr with NUL byte in its data range manually
    let nul_s = ShortCStr::Arc {
        arc: Arc::new(b"hi\0there".to_vec()),
        offset: 0,
        length: 3, // includes the NUL at index 2
    };
    let result = ShortCStr::concat(&[&nul_s]);
    assert!(result.is_err());
}

#[test]
fn concat_many_parts_inline() {
    let parts: Vec<ShortCStr> = vec![
        ShortCStr::from(c"a"),
        ShortCStr::from(c"b"),
        ShortCStr::from(c"c"),
        ShortCStr::from(c"d"),
        ShortCStr::from(c"e"),
        ShortCStr::from(c"f"),
    ];
    let result = ShortCStr::concat(&parts.iter().collect::<Vec<_>>()).unwrap();
    assert_eq!(result.as_bytes().unwrap(), b"abcdef");
}
