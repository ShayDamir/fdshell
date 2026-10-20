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

/// The `(cmd, bodies, background)` face of every segment a line produces: a
/// statement's command bytes, the **text** of each body region it claims (in
/// the line's own bytes), and its async flag. A block prints `B`, the
/// invalid-split marker `A`; both claim no body region here.
fn statements(line: &[u8]) -> Vec<(&[u8], Vec<&[u8]>, bool)> {
    scan_segments(line, false)
        .iter()
        .map(|seg| match seg {
            Segment::Statement {
                cmd,
                bodies,
                background,
                ..
            } => (
                *cmd,
                bodies
                    .iter()
                    .map(|&(s, e)| line.get(s..e).unwrap_or(b""))
                    .collect(),
                *background,
            ),
            Segment::Block { .. } => (b"B".as_slice(), Vec::new(), false),
            Segment::AsyncInvalid => (b"A".as_slice(), Vec::new(), false),
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
///
/// The indented rows are the load-bearing ones: `bg::async_ahead` skips
/// spaces and tabs between the newline and the `&`, so a row whose `&` sits at
/// the line start cannot tell the skip loop from one that never advances.
/// Without them a backwards loop (`j -= 1`) stops on the newline and reports
/// "no `&` ahead": the runs flush above it, and the `&` below loses its
/// target (`AsyncInvalid`, bash's shape here is rc 2 with line 1 executed —
/// the deferral divergence documented in POSIX_COMPATIBILITY).
#[test]
fn a_line_opening_with_ampersand_defers_the_flush_above_it() {
    let cases: &[(&[u8], &[(&str, bool)])] = &[
        (b"echo a\n& echo b\n", &[("S", true), ("S", false)]),
        (b"echo a &\n& echo b\n", &[("S", true), ("S", false)]),
        (
            b"echo a\n& echo b\nwait",
            &[("S", true), ("S", false), ("S", false)],
        ),
        (b"echo a\n & echo b\n", &[("S", true), ("S", false)]),
        (b"echo a\n\t& echo b\n", &[("S", true), ("S", false)]),
        (b"echo a \n \t & echo b\n", &[("S", true), ("S", false)]),
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

/// The boundary classifier is untouched by task #354: a plain `&` **inside a
/// block's text** is classified `Async` exactly as outside one — the gate that
/// keeps it inside the span lives in the scanner's block arm (see
/// `segment::block::opener`), not in the classification. At segment level the
/// face of such a line is the block (`B`), never the async marker; a `;;` behind
/// a `&` **outside** a block is the invalid split (`A`) the `peek` guard of
/// [`super::split`] catches, and it stays that way — a case arm's `& ;;` never
/// reaches that guard, because the block scan owns the run.
#[test]
fn ampersand_inside_a_block_text_is_classified_the_same() {
    for line in [
        b"case x in x) echo a & ;; esac".as_slice(),
        b"for i in 1; do echo hi & done".as_slice(),
        b"{ echo hi & }".as_slice(),
        b"f(){ echo hi & }".as_slice(),
        b"if true & then echo x; fi".as_slice(),
    ] {
        assert_eq!(
            classes(line).as_slice(),
            &[Boundary::Async],
            "{:?}: the `&` is one Async boundary",
            line
        );
        assert_eq!(
            faces(line),
            alloc::vec![("B", false)],
            "{:?} -> faces",
            line
        );
    }
    // The statement-level guard, unchanged: `;;` behind the `&` is the invalid
    // split, and the arm-shaped line behind a closed block is not its business.
    assert_eq!(
        faces(b"echo a & ;; echo b"),
        alloc::vec![("A", false), ("S", false)],
        "the peek guard still fires at statement level"
    );
}

/// Task #313: a plain `&` **mid-line** splits the physical line into two
/// statements, and each claims the body regions of its **own** operators out
/// of the line's one global region stream (in operator order). Before the fix
/// the flush parked the cursor past the line's regions, so the suffix's body
/// lines ran as statements (`y`/`B` "not found"); the whole suffix was lost.
#[test]
fn a_heredoc_behind_an_ampersand_claims_its_own_region() {
    // The plain form: the prefix is backgrounded, the suffix runs foreground.
    assert_eq!(
        statements(b"cat <<A & cat <<B\nx\nA\ny\nB"),
        alloc::vec![
            (
                b"cat <<A".as_slice(),
                alloc::vec![b"x\nA\n".as_slice()],
                true
            ),
            (
                b"cat <<B".as_slice(),
                alloc::vec![b"y\nB".as_slice()],
                false
            ),
        ],
    );
    // The same delimiter word on both sides: the regions stay in operator
    // order, the delimiter's text never merges them.
    assert_eq!(
        statements(b"cat <<T & cat <<T\nx\nT\ny\nT"),
        alloc::vec![
            (
                b"cat <<T".as_slice(),
                alloc::vec![b"x\nT\n".as_slice()],
                true
            ),
            (
                b"cat <<T".as_slice(),
                alloc::vec![b"y\nT".as_slice()],
                false
            ),
        ],
    );
    // Whitespace around the `&` (the load-bearing row for the skip loop).
    assert_eq!(
        statements(b"cat <<A &   cat <<B\nx\nA\ny\nB"),
        alloc::vec![
            (
                b"cat <<A".as_slice(),
                alloc::vec![b"x\nA\n".as_slice()],
                true
            ),
            (
                b"cat <<B".as_slice(),
                alloc::vec![b"y\nB".as_slice()],
                false
            ),
        ],
    );
    // The `<<-` and io-number suffixes: the region keeps its raw bytes.
    assert_eq!(
        statements(b"cat <<A & cat <<-B\nx\nA\n\ty\n\tB"),
        alloc::vec![
            (
                b"cat <<A".as_slice(),
                alloc::vec![b"x\nA\n".as_slice()],
                true
            ),
            (
                b"cat <<-B".as_slice(),
                alloc::vec![b"\ty\n\tB".as_slice()],
                false
            ),
        ],
    );
    assert_eq!(
        statements(b"cat <<A & cat 3<<B\nx\nA\ny\nB"),
        alloc::vec![
            (
                b"cat <<A".as_slice(),
                alloc::vec![b"x\nA\n".as_slice()],
                true
            ),
            (
                b"cat 3<<B".as_slice(),
                alloc::vec![b"y\nB".as_slice()],
                false
            ),
        ],
    );
    // A pipeline suffix is one statement, its operator claims one region.
    assert_eq!(
        statements(b"cat <<A & cat <<B | wc -l\nx\nA\ny\nB"),
        alloc::vec![
            (
                b"cat <<A".as_slice(),
                alloc::vec![b"x\nA\n".as_slice()],
                true
            ),
            (
                b"cat <<B | wc -l".as_slice(),
                alloc::vec![b"y\nB".as_slice()],
                false
            ),
        ],
    );
    // Three jobs on one line: every `&` ends its own list, the runs behind it
    // are the next statement (LESSONS #326: a `&` never re-targets a flushed run).
    assert_eq!(
        statements(b"cat <<A & cat <<B & cat <<C\nx\nA\ny\nB\nz\nC"),
        alloc::vec![
            (
                b"cat <<A".as_slice(),
                alloc::vec![b"x\nA\n".as_slice()],
                true
            ),
            (
                b"cat <<B".as_slice(),
                alloc::vec![b"y\nB\n".as_slice()],
                true
            ),
            (
                b"cat <<C".as_slice(),
                alloc::vec![b"z\nC".as_slice()],
                false
            ),
        ],
    );
}

/// The `&` that **ends** its physical line keeps the region jump it has always
/// made: the cursor goes past the body regions (the body lines are never
/// statements), and the deferral of `async_ahead` below them is untouched.
#[test]
fn an_ampersand_that_ends_the_line_keeps_the_region_jump() {
    // `&` at the very end: one statement, no body to claim (no body lines).
    assert_eq!(
        statements(b"cat <<A &"),
        alloc::vec![(b"cat <<A".as_slice(), alloc::vec![], true)],
    );
    assert_eq!(
        statements(b"cat <<A &\nbody\nA\n"),
        alloc::vec![(
            b"cat <<A".as_slice(),
            alloc::vec![b"body\nA\n".as_slice()],
            true
        )],
    );
    // A `#` comment behind the `&` ends the line as a comment does: the scan
    // jumps past the region, so the body lines stay a body.
    assert_eq!(
        statements(b"cat <<A & # note\nx\nA"),
        alloc::vec![(b"cat <<A".as_slice(), alloc::vec![b"x\nA".as_slice()], true)],
    );
    // A body that never arrives: no region for anybody (the all-or-nothing
    // line rule, owner #275), the split itself stays valid.
    assert_eq!(
        statements(b"cat <<A & cat <<B\n"),
        alloc::vec![
            (b"cat <<A".as_slice(), alloc::vec![], true),
            (b"cat <<B".as_slice(), alloc::vec![], false),
        ],
    );
}

/// The suffix runs behind a `&` whose line **cannot** carry them keep the
/// shape the same shapes without a `&` already had (the pinned divergences,
/// `tests/heredoc.rs`): a `#` comment behind the `&` ends the physical line
/// for the suffix, so its run is flushed body-less; a glued `&` is a word byte
/// of the delimiter (`token_word_end`), so the line's region list is empty.
#[test]
fn a_suffix_the_line_cannot_carry_is_flushed_body_less() {
    // `cat <<A & cat <<B # note`: the comment ends the physical line, so the
    // suffix's run trails it and is emitted body-less (the rule
    // `segment/line/tests.rs::flush_line_multi_line_runs_are_body_less` pins);
    // the parse layer bails on `cat <<B` (measured rc 1, bash rc 0).
    assert_eq!(
        statements(b"cat <<A & cat <<B # note\nx\nA\ny\nB"),
        alloc::vec![
            (
                b"cat <<A".as_slice(),
                alloc::vec![b"x\nA\n".as_slice()],
                true
            ),
            (b"cat <<B".as_slice(), alloc::vec![], false),
            (b"x".as_slice(), alloc::vec![], false),
            (b"y".as_slice(), alloc::vec![], false),
            (b"B".as_slice(), alloc::vec![], false),
        ],
    );
    // Glued `<<A&`: the byte-level delimiter is `A&`, unreadable as `A`, so the
    // line's region list is empty and the suffix is body-less (owner: the
    // tokenizer's `&`-is-a-word-byte rule, `token_word_end`).
    assert_eq!(
        statements(b"cat <<A& cat <<B\nx\nA\ny\nB"),
        alloc::vec![
            (
                b"cat <<A".as_slice(),
                alloc::vec![b"x\nA\n".as_slice()],
                true
            ),
            (b"cat <<B".as_slice(), alloc::vec![], false),
            (b"y".as_slice(), alloc::vec![], false),
            (b"B".as_slice(), alloc::vec![], false),
        ],
    );
    // A bare `<<` behind the `&`: the prefix flush's window ends before the
    // unreadable operator, so it keeps its own region; the suffix flush's
    // whole-line list is empty, and the parse layer reports the shape
    // (`invalid redirect syntax`, measured rc 1; bash rc 2).
    assert_eq!(
        statements(b"cat <<A & cat <<\nx\nA"),
        alloc::vec![
            (b"cat <<A".as_slice(), alloc::vec![b"x\nA".as_slice()], true),
            (b"cat <<".as_slice(), alloc::vec![], false),
        ],
    );
}

/// A `&`-terminated heredoc run in front of a **block** on the same physical
/// line: the prefix claims its region, the block keeps the rest of the line's
/// region stream (the `body_floor` the flush leaves behind is what stops a
/// later flush of this line from re-entering a claimed region).
#[test]
fn an_ampersand_in_front_of_a_block_leaves_the_block_the_rest() {
    assert_eq!(
        statements(b"cat <<A & if true; then cat <<B\nx\nA\ny\nB\nfi"),
        alloc::vec![
            (
                b"cat <<A".as_slice(),
                alloc::vec![b"x\nA\n".as_slice()],
                true
            ),
            (b"B".as_slice(), alloc::vec![], false),
        ],
    );
    // The `&` inside a block's text is the block's own business (task #354):
    // one block segment, no split at the top level.
    assert_eq!(
        statements(b"if true; then cat <<A & cat <<B\nx\nA\ny\nB\nfi"),
        alloc::vec![(b"B".as_slice(), alloc::vec![], false)],
    );
}
