#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::super::Segment;
use super::Split;
use crate::segment::book::Run;
use alloc::vec;
use alloc::vec::Vec;

fn split<'a, 'b>(
    line: &'a [u8],
    start: usize,
    i: usize,
    pending: &'a [u8],
    runs: &'b mut Vec<Run<'a>>,
    segments: &'b mut Vec<Segment<'a>>,
    sep_since_run: bool,
) -> Split<'a, 'b> {
    Split {
        line,
        start,
        i,
        pending,
        runs,
        segments,
        sep_since_run,
    }
}

#[test]
fn peek_skips_spaces_and_tabs_only() {
    let mut runs = Vec::new();
    let mut segs: Vec<Segment> = Vec::new();
    let cases: &[(&[u8], Option<u8>)] = &[
        (b"a & b", Some(b'b')),
        (b"a &\tb", Some(b'b')),
        (b"a &\nb", None),
        (b"a &", None),
    ];
    for (line, want) in cases {
        let split = split(line, 0, 2, b"", &mut runs, &mut segs, false);
        assert_eq!(split.peek(), *want, "{line:?}");
    }
}

#[test]
fn has_target_accepts_a_run_a_pending_word_a_flushed_statement_a_closed_block() {
    let mut runs = vec![Run {
        bytes: b"echo a".as_slice(),
        off: 0,
        background: false,
    }];
    let mut segs: Vec<Segment> = Vec::new();
    assert!(split(b"", 0, 0, b"", &mut runs, &mut segs, false).has_target());
    assert!(!split(b"", 0, 0, b"", &mut runs, &mut segs, true).has_target());

    let mut closed: Vec<Segment> = vec![Segment::Block {
        block_start: 0,
        end_pos: 4,
        closed: true,
        bodies: Vec::new(),
    }];
    let mut runs = Vec::new();
    assert!(split(b"", 0, 0, b"", &mut runs, &mut closed, false).has_target());
    let mut open: Vec<Segment> = vec![Segment::Block {
        block_start: 0,
        end_pos: 4,
        closed: false,
        bodies: Vec::new(),
    }];
    assert!(!split(b"", 0, 0, b"", &mut runs, &mut open, false).has_target());

    let mut stmts: Vec<Segment> = vec![Segment::Statement {
        cmd: b"echo a".as_slice(),
        off: 0,
        bodies: Vec::new(),
        background: false,
        midrun_open: false,
        join_next: false,
    }];
    // A statement flushed by the deferral above is a target while no `;`
    // closed it (`echo a ;\necho b &`); a flushed `;`-terminated one is not
    // (`echo a ; & b` — bash's syntax error).
    assert!(split(b"", 0, 0, b"", &mut runs, &mut stmts, false).has_target());
    assert!(!split(b"", 0, 0, b"", &mut runs, &mut stmts, true).has_target());
    // A word being scanned (not buffered yet) is a target: `echo hi &`
    // splits before its run is pushed.
    let mut empty: Vec<Segment> = Vec::new();
    assert!(
        split(
            b"",
            0,
            7,
            b"echo hi".as_slice(),
            &mut runs,
            &mut empty,
            true
        )
        .has_target()
    );
}

#[test]
fn mark_marks_the_run_the_pending_word_or_the_flushed_statement() {
    let mut runs = vec![Run {
        bytes: b"echo a".as_slice(),
        off: 0,
        background: false,
    }];
    let mut segs: Vec<Segment> = Vec::new();
    split(b"echo a", 0, 6, b"", &mut runs, &mut segs, false).mark();
    assert!(runs.first().is_some_and(|run| run.background));

    let mut runs = Vec::new();
    let mut segs: Vec<Segment> = Vec::new();
    split(
        b"echo hi &",
        0,
        8,
        b"echo hi".as_slice(),
        &mut runs,
        &mut segs,
        false,
    )
    .mark();
    let run = runs.first().unwrap();
    assert_eq!(run.bytes, b"echo hi".as_slice());
    assert_eq!(run.off, 0);
    assert!(run.background);

    let mut runs = Vec::new();
    let mut segs: Vec<Segment> = vec![Segment::Statement {
        cmd: b"echo a".as_slice(),
        off: 3,
        bodies: Vec::new(),
        background: false,
        midrun_open: false,
        join_next: false,
    }];
    split(b"x\necho a", 0, 2, b"", &mut runs, &mut segs, false).mark();
    match segs.first() {
        Some(Segment::Statement { background, .. }) => assert!(*background),
        _ => panic!("statement expected"),
    }
}

/// The `;`-behind-the-`&` guard is a **statement-level** rule, and task #354
/// does not widen it: with a run to terminate, a `;` behind the `&` (the `;;` of
/// a case clause, a stray `;`) is bash's "syntax error, nothing runs". A case
/// arm's `& ;;` never reaches this check — the block scan owns that run — so
/// these rows are the guard's own pins. A newline ends the peek (the accepted
/// multi-line divergence), so `&\n;` passes.
#[test]
fn valid_is_decided_by_the_byte_behind_the_ampersand() {
    let runs = || {
        vec![Run {
            bytes: b"echo a".as_slice(),
            off: 0,
            background: false,
        }]
    };
    for line in [
        b"echo a & ;; echo b".as_slice(),
        b"echo a &; echo b".as_slice(),
        b"echo a &|b".as_slice(),
        b"echo a && b".as_slice(),
    ] {
        let mut runs = runs();
        let mut segs: Vec<Segment> = Vec::new();
        let at = line.iter().position(|&b| b == b'&').unwrap();
        assert!(
            !split(line, 0, at, b"", &mut runs, &mut segs, false).valid(),
            "{line:?}: a `;`/`|`/`&` behind the `&` is invalid"
        );
    }
    for line in [
        b"echo a & echo b".as_slice(),
        b"echo a &\n; echo b".as_slice(),
        b"echo a &".as_slice(),
    ] {
        let mut runs = runs();
        let mut segs: Vec<Segment> = Vec::new();
        let at = line.iter().position(|&b| b == b'&').unwrap();
        assert!(
            split(line, 0, at, b"", &mut runs, &mut segs, false).valid(),
            "{line:?}: the guard passes (a word behind, a newline, or EOS)"
        );
    }
}
