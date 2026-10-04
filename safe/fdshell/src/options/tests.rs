#![allow(clippy::unwrap_used)]

use sys::ShortCStr;

use super::{
    DOTGLOB, ERREXIT, EXPAND_ALIASES, FAILGLOB, NOCLOBBER, NOGLOB, NOUNSET, NULLGLOB, STRICT,
    VERBOSITY, flags, lookup, name_of, set,
};

#[test]
fn lookup_known_names() {
    assert_eq!(lookup(&ShortCStr::from(c"noclobber")), Some(NOCLOBBER));
    assert_eq!(
        lookup(&ShortCStr::from(c"expand_aliases")),
        Some(EXPAND_ALIASES)
    );
    assert_eq!(lookup(&ShortCStr::from(c"nullglob")), Some(NULLGLOB));
    assert_eq!(lookup(&ShortCStr::from(c"strict")), Some(STRICT));
    assert_eq!(lookup(&ShortCStr::from(c"dotglob")), Some(DOTGLOB));
    assert_eq!(lookup(&ShortCStr::from(c"failglob")), Some(FAILGLOB));
    assert_eq!(lookup(&ShortCStr::from(c"errexit")), Some(ERREXIT));
    assert_eq!(lookup(&ShortCStr::from(c"nounset")), Some(NOUNSET));
    assert_eq!(lookup(&ShortCStr::from(c"noglob")), Some(NOGLOB));
    assert_eq!(lookup(&ShortCStr::from(c"verbose")), Some(VERBOSITY));
}

#[test]
fn lookup_unknown_name_is_none() {
    assert_eq!(lookup(&ShortCStr::from(c"bogus_option")), None);
    assert_eq!(lookup(&ShortCStr::from(c"")), None);
}

#[test]
fn name_of_round_trips() {
    assert_eq!(name_of(NOCLOBBER), Some(b"noclobber".as_slice()));
    assert_eq!(name_of(EXPAND_ALIASES), Some(b"expand_aliases".as_slice()));
    assert_eq!(name_of(NULLGLOB), Some(b"nullglob".as_slice()));
    assert_eq!(name_of(STRICT), Some(b"strict".as_slice()));
    assert_eq!(name_of(DOTGLOB), Some(b"dotglob".as_slice()));
    assert_eq!(name_of(FAILGLOB), Some(b"failglob".as_slice()));
    assert_eq!(name_of(ERREXIT), Some(b"errexit".as_slice()));
    assert_eq!(name_of(NOUNSET), Some(b"nounset".as_slice()));
    assert_eq!(name_of(NOGLOB), Some(b"noglob".as_slice()));
    assert_eq!(name_of(VERBOSITY), Some(b"verbose".as_slice()));
    assert_eq!(name_of(0), None);
    assert_eq!(name_of(1 << 13), None);
}

#[test]
fn flags_lists_active_short_flags_in_table_order() {
    assert_eq!(flags(0), &b""[..]);
    assert_eq!(flags(NOCLOBBER), &b"C"[..]);
    // `expand_aliases` has no short flag, so it never appears in `$-`.
    assert_eq!(flags(EXPAND_ALIASES), &b""[..]);
    assert_eq!(flags(NOCLOBBER | EXPAND_ALIASES), &b"C"[..]);
    assert_eq!(flags(ERREXIT), &b"e"[..]);
    assert_eq!(flags(NOUNSET), &b"u"[..]);
    assert_eq!(flags(NOGLOB), &b"f"[..]);
    assert_eq!(flags(VERBOSITY), &b"v"[..]);
    // Table order: existing flags precede the appended POSIX ones.
    assert_eq!(flags(NOCLOBBER | ERREXIT | NOUNSET), &b"Ceu"[..]);
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
