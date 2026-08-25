#![allow(clippy::unwrap_used)]

use sys::ShortCStr;

use super::{EXPAND_ALIASES, NOCLOBBER, flags, lookup, name_of, set};

#[test]
fn lookup_known_names() {
    assert_eq!(lookup(&ShortCStr::from(c"noclobber")), Some(NOCLOBBER));
    assert_eq!(
        lookup(&ShortCStr::from(c"expand_aliases")),
        Some(EXPAND_ALIASES)
    );
}

#[test]
fn lookup_unknown_name_is_none() {
    assert_eq!(lookup(&ShortCStr::from(c"nullglob")), None);
    assert_eq!(lookup(&ShortCStr::from(c"")), None);
}

#[test]
fn name_of_round_trips() {
    assert_eq!(name_of(NOCLOBBER), Some(b"noclobber".as_slice()));
    assert_eq!(name_of(EXPAND_ALIASES), Some(b"expand_aliases".as_slice()));
    assert_eq!(name_of(0), None);
    assert_eq!(name_of(1 << 8), None);
}

#[test]
fn flags_lists_active_short_flags_in_table_order() {
    assert_eq!(flags(0), &b""[..]);
    assert_eq!(flags(NOCLOBBER), &b"C"[..]);
    // `expand_aliases` has no short flag, so it never appears in `$-`.
    assert_eq!(flags(EXPAND_ALIASES), &b""[..]);
    assert_eq!(flags(NOCLOBBER | EXPAND_ALIASES), &b"C"[..]);
}

#[test]
fn set_toggles_bits() {
    let opts = 0u32;
    assert_eq!(set(opts, NOCLOBBER, true), NOCLOBBER);
    assert_eq!(set(opts, NOCLOBBER, false), 0);
    let both = set(opts, NOCLOBBER, true);
    let both = set(both, EXPAND_ALIASES, true);
    assert_eq!(both, NOCLOBBER | EXPAND_ALIASES);
    assert_eq!(set(both, NOCLOBBER, false), EXPAND_ALIASES);
    assert_eq!(set(both, NOCLOBBER, true), both);
}
