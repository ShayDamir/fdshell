#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::*;
use crate::parse::Token;
use crate::parse::token::tokenize_statement;
use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;
use error_stack::Report;
use sys::{Origin, Position, ScriptText};

fn stext(bytes: &[u8]) -> ScriptText {
    ScriptText::new(
        sys::ShortCStr::from_vec(bytes.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// The tokens of the whole script and the index of its `done`, exactly as
/// `tokens_to_wait` derives them, so every row runs against a real `done_idx`.
fn scan(script: &[u8]) -> (Vec<Token>, ScriptText, usize) {
    let text = stext(script);
    let tokens = tokenize_statement(text.as_bytes().unwrap()).unwrap();
    let done_idx = crate::parse::block_tail::closer_index(&tokens, b"done").unwrap();
    (tokens, text, done_idx)
}

/// What one walk of the arm loop reports: each arm's body text, the position the
/// last arm resumed at, and the block's `done_idx`.
struct Walk {
    bodies: Vec<Vec<u8>>,
    pos: usize,
    done_idx: usize,
}

/// Walk the arms the way `tokens_to_wait` does.
fn walk(script: &[u8]) -> Result<Walk, Report<ParseError>> {
    let (tokens, text, done_idx) = scan(script);
    let mut bodies = Vec::new();
    let mut pos = 1;
    while let Some((arm, next)) = arm::next_arm(&tokens, &text, pos, done_idx)? {
        bodies.push(arm.body.as_bytes().unwrap().to_vec());
        pos = next;
    }
    Ok(Walk {
        bodies,
        pos,
        done_idx,
    })
}

/// The follow-up `next_arm` at the position the walk stopped at.
fn resume(script: &[u8]) -> Result<Option<(WaitArm, usize)>, Report<ParseError>> {
    let walk = walk(script)?;
    let (tokens, text, _) = scan(script);
    arm::next_arm(&tokens, &text, walk.pos, walk.done_idx)
}

/// An arm whose body runs up to `done` (one `;` behind it, no `;;` pair):
/// `extract::body` ends the walk at `done_idx` itself, so the follow-up
/// `next_arm` starts **on** the closer and bails `Ok(None)` on the
/// `pos >= done_idx` guard.
#[test]
fn arm_ending_at_done_bails_on_the_next_call() {
    let walk = walk(b"wait\nreadable %rd) echo hi;\ndone").unwrap();
    assert_eq!(walk.bodies, vec![b"echo hi".to_vec()]);
    assert_eq!(walk.pos, walk.done_idx);
    assert!(
        resume(b"wait\nreadable %rd) echo hi;\ndone")
            .unwrap()
            .is_none()
    );
}

/// Only `;`s sit between the last arm's `;;` and `done` (the pair's leftover and
/// the newline's): the leading-`;` skip loop walks them — unbounded, the closer
/// is not a `;` — and the same guard bails `Ok(None)`, so a `;`-only tail ends
/// the block instead of being searched for a `)`.
///
/// This is the row that kills the guard's `>=`→`>` mutant: with `>` the
/// `[pos..done_idx]` region is walked to the closer and the empty search bails
/// `WaitMissingCloseParen`, so the block reports a syntax error where the clean
/// build parses one arm.
#[test]
fn semicolon_only_tail_before_done_bails() {
    let walk = walk(b"wait\nreadable %rd) true ;; ;\ndone").unwrap();
    assert_eq!(walk.bodies, vec![b"true".to_vec()]);
    // The walk stopped in front of the `;` tail, not on the closer.
    assert!(walk.pos < walk.done_idx);
    assert!(
        resume(b"wait\nreadable %rd) true ;; ;\ndone")
            .unwrap()
            .is_none()
    );
}

/// A run of `;`s in front of the first arm is skipped, and the walk finds the arm
/// behind it: the loop stops on the first token that is not a `;`, and a position
/// past the slice is no `;` either (the `.get` + `is_some_and` pair).
#[test]
fn leading_semicolons_are_skipped() {
    let walk = walk(b"wait\n;\n;\nwritable %wr) echo w;\ndone").unwrap();
    assert_eq!(walk.bodies, vec![b"echo w".to_vec()]);
    assert_eq!(walk.pos, walk.done_idx);
}

/// A walk that starts **at** the closer (the caller's loop after the last arm)
/// bails at once: `pos >= done_idx`, before any `)` search runs.
#[test]
fn a_call_at_the_closer_bails() {
    let (tokens, text, done_idx) = scan(b"wait\nreadable %rd) echo hi;\ndone");
    assert!(
        arm::next_arm(&tokens, &text, done_idx, done_idx)
            .unwrap()
            .is_none()
    );
}

/// Every separator test of the arm walk is [`span_word`]'s (task #336): a quoted
/// `";"` in front of an arm is its **pattern word**, never the leading skip, and
/// a quoted `")"` is pattern text, never the arm's head closer. Each row stands
/// next to its unquoted twin.
#[test]
fn a_quoted_separator_is_pattern_text_in_the_arm_walk() {
    // The real `;` in front of the arm is skipped, so the pattern keyword stands
    // at the head of the arm.
    assert_eq!(
        walk(b"wait\n;\nwritable %wr) echo w;\ndone")
            .unwrap()
            .bodies,
        vec![b"echo w".to_vec()]
    );
    // A quoted or escaped `;` is that word, not the skip: the arm's pattern no
    // longer starts with a pattern keyword (bash rejects the same shape), where
    // the ungated walk skipped it and parsed the arm.
    for line in [
        b"wait\n\";\" writable %wr) echo w;\ndone".as_slice(),
        b"wait\n\';' writable %wr) echo w;\ndone",
        b"wait\n\\; writable %wr) echo w;\ndone",
    ] {
        let err = walk(line).err().unwrap();
        assert_eq!(
            err.current_context().to_string(),
            "`wait`: unknown arm pattern keyword",
            "{line:?}"
        );
    }
    // A quoted `)` is pattern text: the arm never reaches its head closer, and
    // the block reports the missing one.
    let err = walk(b"wait\nreadable \")\" %rd echo x;\ndone")
        .err()
        .unwrap();
    assert_eq!(
        err.current_context().to_string(),
        "`wait`: missing ')' after arm pattern"
    );
}
