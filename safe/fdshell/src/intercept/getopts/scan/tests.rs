#![allow(clippy::unwrap_used)]
use super::*;
use crate::intercept::getopts::optstring::Optstring;

fn s(b: &[u8]) -> ShortCStr {
    ShortCStr::from_vec(b.to_vec()).unwrap()
}

/// The plan §2 table, measured on bash 5.3.9 (re-measured on task #590), pinned
/// as the pure scan's answers: `(optstring, word, cursor, next word) -> Scan`.
fn opts(bytes: &[u8]) -> Optstring {
    Optstring::parse(&s(bytes))
}

#[test]
fn a_flag_reports_and_keeps_the_words_rest_for_the_cluster() {
    // `set -- -cval`: the `c` call reports `c` and leaves the cursor on the word.
    let found = scan(1, &s(b"-cval"), 0, &opts(b"ab:c"), Some(&s(b"x")));
    assert_eq!(
        found,
        Scan::Found {
            ch: b'c',
            arg: None,
            next_index: 1,
            next_char: 2
        }
    );
}

#[test]
fn a_flag_never_takes_the_next_word() {
    // Measured `set -- -c val`: `c` reports empty OPTARG and the counter moves
    // by one word only — `val` stands untouched for the next call.
    let found = scan(1, &s(b"-c"), 0, &opts(b"ab:c"), Some(&s(b"val")));
    assert_eq!(
        found,
        Scan::Found {
            ch: b'c',
            arg: None,
            next_index: 2,
            next_char: 0
        }
    );
}

#[test]
fn an_attached_argument_is_the_rest_of_the_word() {
    // Measured `set -- -abc val`: `a` reports, then `b` takes `c` — the attached
    // argument wins over the next word, and the word is consumed.
    let opts = opts(b"ab:c");
    let found = scan(1, &s(b"-abc"), 2, &opts, Some(&s(b"VAL")));
    assert_eq!(
        found,
        Scan::Found {
            ch: b'b',
            arg: Some(s(b"c")),
            next_index: 2,
            next_char: 0
        }
    );
}

#[test]
fn a_cluster_letter_with_nothing_left_takes_the_next_whole_word() {
    // Measured `set -- -ab -b val`: `a` stays mid-word, then `b` takes `-b` —
    // the argument word is consumed too, so the counter lands past both.
    assert_eq!(
        scan(1, &s(b"-ab"), 2, &opts(b"ab:c"), Some(&s(b"-b"))),
        Scan::Found {
            ch: b'b',
            arg: Some(s(b"-b")),
            next_index: 3,
            next_char: 0
        }
    );
}

#[test]
fn a_separate_argument_consumes_both_words() {
    // Measured `set -- -b val` (and the silent-mode row): `b` takes `val`, the
    // counter lands at 3 — both words are consumed.
    assert_eq!(
        scan(1, &s(b"-b"), 0, &opts(b"ab:c"), Some(&s(b"val"))),
        Scan::Found {
            ch: b'b',
            arg: Some(s(b"val")),
            next_index: 3,
            next_char: 0
        }
    );
}

#[test]
fn the_error_walk_on_cval_reports_one_character_per_call() {
    // Measured `set -- -cval`: `c` (OPTIND 1), error on `v` (OPTIND 1), `a`
    // (OPTIND 1), error on `l` (OPTIND 2) — the error is reported per character
    // and the word continues; a word the error ends moves the counter.
    let opts = opts(b"ab:c");
    assert_eq!(
        scan(1, &s(b"-cval"), 2, &opts, None),
        Scan::Presentation {
            ch: b'v',
            problem: Problem::Unknown,
            next_index: 1,
            next_char: 3
        }
    );
    assert_eq!(
        scan(1, &s(b"-cval"), 4, &opts, None),
        Scan::Presentation {
            ch: b'l',
            problem: Problem::Unknown,
            next_index: 2,
            next_char: 0
        }
    );
}

#[test]
fn the_continue_rule_is_bashs_measured_walk() {
    // Measured `set -- -a -x -b val`: `a` (OPTIND 2), the `-x` error (name `?`,
    // rc 0, OPTIND 3), `b`=`val` (OPTIND 5) — **processing continues after a
    // presentation error** (POSIX stops it; the single decision arm in
    // [`presentation`] is that flip, task #590 §3.3).
    let opts = opts(b"ab:c");
    assert_eq!(
        scan(1, &s(b"-a"), 0, &opts, Some(&s(b"-x"))),
        Scan::Found {
            ch: b'a',
            arg: None,
            next_index: 2,
            next_char: 0
        }
    );
    assert_eq!(
        scan(2, &s(b"-x"), 0, &opts, Some(&s(b"-b"))),
        Scan::Presentation {
            ch: b'x',
            problem: Problem::Unknown,
            next_index: 3,
            next_char: 0
        }
    );
    assert_eq!(
        scan(3, &s(b"-b"), 0, &opts, Some(&s(b"val"))),
        Scan::Found {
            ch: b'b',
            arg: Some(s(b"val")),
            next_index: 5,
            next_char: 0
        }
    );
}

#[test]
fn a_missing_option_argument_is_reported_with_the_option_character() {
    // Measured `set -- -b`: `b`, `MissingArgument`, the word consumed (bash
    // answers name `?` rc 0 in verbose mode, name `:` OPTARG `b` in silent mode
    // — the naming is task #591's, the data is pinned here).
    assert_eq!(
        scan(1, &s(b"-b"), 0, &opts(b"ab:c"), None),
        Scan::Presentation {
            ch: b'b',
            problem: Problem::MissingArgument,
            next_index: 2,
            next_char: 0
        }
    );
}

#[test]
fn the_terminator_is_consumed_and_the_answer_names_the_next_word() {
    // Measured `set -- -- x` → OPTIND 2.
    assert_eq!(
        scan(1, &s(b"--"), 0, &opts(b"ab:c"), None),
        Scan::End { index: 2 }
    );
}

#[test]
fn a_non_option_word_ends_the_scan_on_its_own_index() {
    // Measured `set -- foo` → OPTIND stays 1 (and `set -- - ` stays on the
    // dash's own index — the lone dash is not an option word).
    let opts = opts(b"ab:c");
    assert_eq!(scan(1, &s(b"foo"), 0, &opts, None), Scan::End { index: 1 });
    assert_eq!(scan(1, &s(b"-"), 0, &opts, None), Scan::End { index: 1 });
}

#[test]
fn a_non_alphabetic_byte_takes_the_presentation_path() {
    // Decision §4.1: the alphabet is `[A-Za-z0-9]`; bash matches a two-byte `é`
    // (and `#`/`?`) as option characters and fdshell answers the error path —
    // the divergence row for task #592's POSIX_COMPATIBILITY section.
    assert_eq!(
        scan(1, &s(b"-\xc3\xa9"), 0, &opts(b"ab:c"), Some(&s(b"x"))),
        Scan::Presentation {
            ch: 0xc3,
            problem: Problem::Unknown,
            next_index: 1,
            next_char: 2
        }
    );
}

#[test]
fn digits_and_letters_are_option_characters() {
    // Measured: `-7` on `"a7:zZ0"` takes the next word (`7:` carries the marker)
    // and `-Z` reports as a plain flag.
    let opts = opts(b"a7:zZ0");
    assert_eq!(
        scan(1, &s(b"-7"), 0, &opts, Some(&s(b"x"))),
        Scan::Found {
            ch: b'7',
            arg: Some(s(b"x")),
            next_index: 3,
            next_char: 0
        }
    );
    assert_eq!(
        scan(1, &s(b"-Z"), 0, &opts, None),
        Scan::Found {
            ch: b'Z',
            arg: None,
            next_index: 2,
            next_char: 0
        }
    );
}

#[test]
fn an_empty_optstring_leaves_every_word_to_the_error_path() {
    // Measured `getopts "" o` on `-a x`: bash answers `?` and moves to word 2.
    assert_eq!(
        scan(1, &s(b"-a"), 0, &opts(b""), Some(&s(b"x"))),
        Scan::Presentation {
            ch: b'a',
            problem: Problem::Unknown,
            next_index: 2,
            next_char: 0
        }
    );
}

#[test]
fn a_word_that_lost_its_dash_ends_the_scan_on_its_index() {
    // The word list may change under a mid-cluster cursor (`set --` / `shift`
    // without an `OPTIND` touch — bash's answer there reads a stale buffer, so
    // only fdshell's own rule is pinned): no word beginning with `-`, no
    // option word. A cursor past the word's end ends it the same way.
    let opts = opts(b"ab:c");
    assert_eq!(scan(1, &s(b"a"), 2, &opts, None), Scan::End { index: 1 });
    assert_eq!(scan(1, &s(b"-"), 2, &opts, None), Scan::End { index: 1 });
    // A `--` reached mid-cluster is a word with no option character on it, not
    // the terminator: the cursor lands past its end and the scan ends on it.
    assert_eq!(scan(1, &s(b"--"), 2, &opts, None), Scan::End { index: 1 });
}
