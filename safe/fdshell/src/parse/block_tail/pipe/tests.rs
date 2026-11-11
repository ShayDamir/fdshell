#![allow(clippy::unwrap_used)]
//! Direct pins of the closer-tail family's pipeline arm (task #490), one row per
//! closer and per arm the shared walk takes, asserted at the parse layer: the
//! promoted rows come back as a `Pipeline` with the block as stage 0, the rows the
//! arm does not answer stay the block's own parse.

use super::*;
use crate::parse::{Stage, parse};
use sys::{Origin, Position, ShortCStr};

fn stext(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// `(stages, block_stage_0)` of the parse: `stages` is `0` when the line stays the
/// block's own statement, and the stage count when the arm hands it to the pipeline
/// parser; the second half says whether stage 0 is the block.
fn arm(line: &[u8]) -> (usize, bool) {
    let parsed = parse(&stext(line)).unwrap();
    match &parsed {
        ParsedLine::Pipeline(p) => (
            p.stages.len(),
            matches!(p.stages.first(), Some(Stage::Block(_))),
        ),
        _ => (0, false),
    }
}

/// The arm fires for every closer of the family it walks (`done`, `fi`, `esac`,
/// the fdshell-only `wait` twin included), past everything the walk's arms own —
/// the path, the here-string, the `&>` family, the here-doc — and for a tail with
/// two pipes, which is the same one step.
#[test]
fn the_arm_promotes_a_block_whose_tail_starts_with_a_pipe() {
    for (line, stages) in [
        (b"for x in 1; do :; done | cat".as_slice(), 2),
        (b"while :; do :; done | cat".as_slice(), 2),
        (b"until :; do :; done | cat".as_slice(), 2),
        (b"if true; then :; fi | cat".as_slice(), 2),
        (b"case x in x) :;; esac | cat".as_slice(), 2),
        (b"wait\n readable %rd) :;;\ndone | cat".as_slice(), 2),
        (b"for x in 1; do :; done | cat | cat".as_slice(), 3),
        (b"for x in 1; do :; done > f | cat".as_slice(), 2),
        (b"for x in 1; do :; done <<<x | cat".as_slice(), 2),
        (b"for x in 1; do :; done &>f | cat".as_slice(), 2),
        (b"for x in a; do :; done <<T | cat\nbody\nT".as_slice(), 2),
    ] {
        assert_eq!(arm(line), (stages, true), "{line:?}");
    }
}

/// The arm answers no when the tail is not a pipe: no tail at all; a comment word
/// (task #296's stop, which is why `done # c | cat` keeps the block's own parse);
/// and the brace closers — the group and the definer have their own tails, and the
/// arithmetic `for` is answered by whichever body it parsed, so none of them is
/// asked (their readings of a pipe behind the closer are pinned at their layers).
#[test]
fn the_arm_answers_no_for_the_rest_of_the_family() {
    for line in [
        b"for x in 1; do :; done".as_slice(),
        b"for x in 1; do :; done # c | cat".as_slice(),
        b"{ echo a; }".as_slice(),
        b"f() { :; }".as_slice(),
        b"for ((i=0;i<2;i++)); do :; done".as_slice(),
    ] {
        assert_eq!(arm(line), (0, false), "{line:?}");
    }
}
