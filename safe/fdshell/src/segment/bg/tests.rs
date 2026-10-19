#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
#![allow(clippy::type_complexity)]
use super::super::{Segment, scan_segments};
use crate::scan::{Boundary, ScanState, boundary};

use alloc::vec::Vec;

/// The boundary class of every `&` byte position in `line`, in byte order.
fn classes(line: &[u8]) -> Vec<Boundary> {
    let mut state = ScanState::new();
    let mut out = Vec::new();
    let mut i = 0;
    while i < line.len() {
        let kind = boundary(line, i, &state);
        // Classify exactly the positions the real scan visits: a `Char`
        // advances through the byte, so a consumed pair (`&&`) is classified
        // once, at its first byte.
        let is_ampersand = matches!(line.get(i), Some(&b'&'));
        if matches!(kind, Boundary::Char) {
            i = state.advance(line, i);
        } else {
            state.word_active = false;
            i += 1;
        }
        if is_ampersand {
            out.push(kind);
        }
    }
    out
}

/// The statement bytes of every statement segment a line produces.
fn seg_bytes(line: &[u8]) -> Vec<&[u8]> {
    scan_segments(line, false)
        .iter()
        .map(|seg| match seg {
            Segment::Statement { cmd, .. } => *cmd,
            _ => b"?",
        })
        .collect()
}

/// The `(kind, background)` face of every segment a line produces
/// (`A` = the invalid-split marker, `B` = a block, `S` = a statement).
fn faces(line: &[u8]) -> Vec<(&'static str, bool)> {
    scan_segments(line, false)
        .iter()
        .map(|seg| match seg {
            Segment::Statement { background, .. } => ("S", *background),
            Segment::Block { .. } => ("B", false),
            Segment::AsyncInvalid => ("A", false),
        })
        .collect()
}

#[test]
fn boundary_classifies_the_plain_ampersand_only_at_a_word_start() {
    let cases: &[(&[u8], &[Boundary])] = &[
        (b"echo a & echo b", &[Boundary::Async]),
        (b"echo a && echo b", &[Boundary::Char, Boundary::Char]),
        (b"echo a > &b", &[Boundary::Char]),
        (b"echo a &>out", &[Boundary::Char]),
        (b"echo a &>&x", &[Boundary::Char, Boundary::Char]),
        (b"echo a &>|&x", &[Boundary::Char, Boundary::Char]),
        (b"exec 12>&-", &[Boundary::Char]),
        (b"exec 12>&9", &[Boundary::Char]),
        (b"exec 12>&&9", &[Boundary::Char, Boundary::Char]),
        (b"echo \"&\"", &[Boundary::Char]),
        (b"echo '&'", &[Boundary::Char]),
        (b"echo a \\&", &[]),
        (b"echo $(sleep 1 &)", &[Boundary::Char]),
        (b"(( 1 & 2 ))", &[Boundary::Char]),
        (b"a&b", &[Boundary::Char]),
        (b"&", &[Boundary::Async]),
        (b"echo a & & b", &[Boundary::Async, Boundary::Async]),
    ];
    for (line, want) in cases {
        assert_eq!(&classes(line), want, "{line:?}");
    }
}

#[test]
fn split_marks_the_last_run_background() {
    let cases: &[(&[u8], &[(&str, bool)])] = &[
        (b"echo hi &", &[("S", true)]),
        (b"echo a & echo b", &[("S", true), ("S", false)]),
        (b"echo a && echo b &", &[("S", true)]),
        (b"echo a | b &", &[("S", true)]),
        (b"echo a & echo b &", &[("S", true), ("S", true)]),
        (b"echo \"&\"", &[("S", false)]),
        (b"echo a \\&", &[("S", false)]),
        (b"x=1 & echo $x", &[("S", true), ("S", false)]),
        (
            b"echo a &\\n; echo b",
            &[("S", true), ("S", false), ("S", false)],
        ), // literal-\n word divergence
    ];
    for (line, want) in cases {
        assert_eq!(&faces(line), want, "{line:?}");
    }
}

/// Each `&` terminates only the command list since the previous one: the
/// flushed runs are never a later `&`'s target, so no statement is dropped.
#[test]
fn an_ampersand_never_targets_an_already_terminated_run() {
    let cases: &[(&[u8], &[&[u8]])] = &[
        (b"echo a & echo b &", &[b"echo a", b"echo b"]),
        (b"echo a & echo b", &[b"echo a", b"echo b"]),
        (b"echo a && echo b &", &[b"echo a && echo b"]),
    ];
    for (line, want) in cases {
        assert_eq!(&seg_bytes(line), want, "{line:?}");
    }
}

/// The multi-line deferral (`segment/line::end_line`'s deferred arm): a
/// newline whose next line opens with the plain `&` leaves the line's runs
/// buffered, so that `&` claims them as its target and the flush resumes on
/// the following line.
#[test]
fn a_line_opening_with_ampersand_defers_the_flush_above_it() {
    let cases: &[(&[u8], &[(&str, bool)])] = &[
        (b"echo a\n& echo b\n", &[("S", true), ("S", false)]),
        (b"echo a &\n& echo b\n", &[("S", true), ("S", false)]),
        (
            b"echo a\n& echo b\nwait",
            &[("S", true), ("S", false), ("S", false)],
        ),
    ];
    for (line, want) in cases {
        assert_eq!(&faces(line), want, "{line:?}");
    }
}

#[test]
fn block_trailing_ampersand_is_consumed_foreground() {
    let cases: &[(&[u8], &[(&str, bool)])] = &[
        (b"if :; then echo x; fi &", &[("B", false)]),
        (
            b"if :; then echo x; fi & echo y",
            &[("B", false), ("S", false)],
        ),
        (b"while :; do echo x; done &", &[("B", false)]),
        (b"f() { echo a; } &", &[("B", false)]),
        (b"{ echo a; } &", &[("B", false)]),
        (b"if :; then echo a & fi", &[("B", false)]),
    ];
    for (line, want) in cases {
        assert_eq!(&faces(line), want, "{line:?}");
    }
}

/// The invalid-split family (`Segment::AsyncInvalid`): nothing to terminate,
/// a `;`-terminated target, or a `;`/`|`/`&` peek behind the `&` — bash's
/// "syntax error, nothing runs" shapes (the pre-pass lives in `script.rs`).
#[test]
fn invalid_splits_are_marked() {
    let cases: &[(&[u8], &[(&str, bool)])] = &[
        (b"& echo b", &[("A", false), ("S", false)]),
        (b"echo a ; & b", &[("A", false), ("S", false)]),
        (b"echo a & ; echo b", &[("A", false), ("S", false)]),
        (b"echo a & | cat", &[("A", false), ("S", false)]),
        (b"echo a & & b", &[("A", false), ("A", false), ("S", false)]),
        (b"echo a && & echo b", &[("S", true), ("S", false)]),
    ];
    for (line, want) in cases {
        assert_eq!(&faces(line), want, "{line:?}");
    }
}

/// The multi-line streaming shape (measured bash 5.3.9: `echo a &\n; echo b`
/// prints `a`, rc 2 — line 1 executes, line 2 is the syntax error; fdshell
/// prints `a` `b` rc 0, its accepted empty-statement divergence). The
/// newline flushes line 1's run as the statement the next line's `&` marks.
#[test]
fn newline_ends_the_peek() {
    assert_eq!(
        faces(b"echo a &\n; echo b".as_slice()),
        [("S", true), ("S", false)]
    );
    assert_eq!(
        seg_bytes(b"echo a &\necho b".as_slice()),
        [b"echo a".as_slice(), b"echo b".as_slice()]
    );
    assert_eq!(
        faces(b"echo a & ; echo b".as_slice()),
        [("A", false), ("S", false)]
    );
}

/// The statement after a group keeps its own `&` split: the group swallows
/// the `&` written inside its own statement tail (the pinned group error).
#[test]
fn group_tail_swallows_the_ampersand_and_the_next_line_splits() {
    assert_eq!(
        faces(b"{ echo a; } &\necho b".as_slice()),
        [("B", false), ("S", false)]
    );
}
