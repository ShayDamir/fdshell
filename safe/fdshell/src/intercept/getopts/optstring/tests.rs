#![allow(clippy::unwrap_used)]
use super::*;
use sys::ShortCStr;

fn s(b: &[u8]) -> ShortCStr {
    ShortCStr::from_vec(b.to_vec()).unwrap()
}

/// The decisions of task #590 §4.1/§4.2, pinned at the parser level. bash's
/// answers on the divergence rows (`?`, `#`, `é` as option characters) are
/// measured and recorded for task #592's POSIX_COMPATIBILITY section.
#[test]
fn a_leading_colon_selects_silent_mode_and_is_not_an_option_character() {
    // bash 5.3.9 measured: `:ab:c` answers name `:` with OPTARG `b` on a missing
    // option-argument; plain `ab:c` answers name `?` (task #591 owns the naming).
    let silent = Optstring::parse(&s(b":ab:c"));
    assert!(silent.is_silent());
    assert_eq!(
        silent.lookup(b'b'),
        Some(true),
        "the marker is not the flag"
    );
    let loud = Optstring::parse(&s(b"ab:c"));
    assert!(!loud.is_silent());
    assert_eq!(loud.lookup(b'b'), Some(true));
}

#[test]
fn a_colon_after_an_option_character_is_its_argument_marker() {
    let opts = Optstring::parse(&s(b"ab:c"));
    assert_eq!(opts.lookup(b'a'), Some(false));
    assert_eq!(opts.lookup(b'b'), Some(true));
    assert_eq!(opts.lookup(b'c'), Some(false));
}

#[test]
fn the_alphabet_is_letters_and_digits() {
    // bash accepts digits (measured: `-7` on `a7:zZ0` takes the argument its `:`
    // marker promises, `-Z` reports as a flag, `0` as an option character).
    let opts = Optstring::parse(&s(b"azAZ09:"));
    for ch in b"azAZ0" {
        assert_eq!(
            opts.lookup(*ch),
            Some(false),
            "{ch} is a plain option character"
        );
    }
    assert_eq!(
        opts.lookup(b'9'),
        Some(true),
        "the trailing colon is 9's marker"
    );
    assert_eq!(opts.lookup(b'/'), None, "one below the digit range");
    assert_eq!(opts.lookup(b'@'), None, "one below the uppercase range");
    assert_eq!(opts.lookup(b'`'), None, "one below the lowercase range");
}

#[test]
fn stray_bytes_are_ignored_inside_the_optstring() {
    // §4.1: a `#`, a `?`, a hyphen and a non-ASCII byte are **not** option
    // characters (bash matches `#` and `é` — measured divergences for #592); a
    // word reaching one takes the presentation-error path, and the characters
    // around a stray byte still parse.
    let opts = Optstring::parse(&s(b"a#b?c-\xc3\xa9"));
    for ch in b"abc" {
        assert_eq!(opts.lookup(*ch), Some(false), "{ch} parses past the strays");
    }
    assert_eq!(opts.lookup(b'#'), None);
    assert_eq!(opts.lookup(b'?'), None);
    assert_eq!(opts.lookup(b'-'), None);
    assert_eq!(opts.lookup(0xc3), None);
}

#[test]
fn a_stray_colon_is_ignored() {
    // §4.2: only a leading colon means silence and only the colon **directly**
    // after an option character is a marker; `a::b` keeps the first colon as `a`'s
    // marker (bash reads `::` as an optional argument — the answers split there,
    // recorded for #592) and ignores the second.
    let opts = Optstring::parse(&s(b"a::b"));
    assert!(!opts.is_silent());
    assert_eq!(opts.lookup(b'a'), Some(true));
    assert_eq!(opts.lookup(b'b'), Some(false));
    assert_eq!(opts.lookup(b':'), None);
}

#[test]
fn an_empty_optstring_has_no_option_characters() {
    // Measured: `getopts "" o` sends `-a` to bash's error path (name `?`, rc 0,
    // and the walk continues); fdshell's parser simply lists nothing.
    let opts = Optstring::parse(&s(b""));
    assert!(!opts.is_silent());
    assert_eq!(opts.lookup(b'a'), None);
}

#[test]
fn an_optstring_that_is_only_a_colon_is_silent_and_empty() {
    // Measured: bash answers `?` for `-a` on `":"` with no diagnostic — the
    // lone colon is the silent flag, leaving no option characters behind.
    let opts = Optstring::parse(&s(b":"));
    assert!(opts.is_silent());
    assert_eq!(opts.lookup(b'a'), None);
}
