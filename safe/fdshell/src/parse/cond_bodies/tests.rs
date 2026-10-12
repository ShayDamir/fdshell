//! Tests for the condition body extraction.

#![allow(clippy::unwrap_used)]

use super::condition_bodies;
use crate::parse::semi::{find_preceded_by_semi, trim_semi};
use crate::parse::token::tokenize_statement;
use alloc::vec;
use alloc::vec::Vec;
use sys::{Origin, Position, ScriptText, ShortCStr};

/// The condition's bodies (body lines plus the delimiter line) are extracted
/// from the block text, which carries them after the closing keyword.
#[test]
fn condition_bodies_extracted_from_block_text() {
    let line = b"if cat <<EOF; then echo yes; fi\nbody\nEOF";
    let text = ScriptText::new(
        ShortCStr::from_vec(line.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    );
    let tokens = tokenize_statement(line).unwrap();
    let first_then = find_preceded_by_semi(&tokens, 1, b"then").unwrap();
    let cond_tokens = trim_semi(tokens.get(1..first_then).unwrap());
    let bodies = condition_bodies(&text, cond_tokens);
    assert_eq!(
        bodies,
        vec![ShortCStr::from_vec(b"body\nEOF".to_vec()).unwrap()]
    );
}

/// A condition without `<<` operators yields no bodies.
#[test]
fn condition_bodies_no_operators_is_empty() {
    let line = b"if true; then echo yes; fi\nbody\nEOF";
    let text = ScriptText::new(
        ShortCStr::from_vec(line.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    );
    let tokens = tokenize_statement(line).unwrap();
    let first_then = find_preceded_by_semi(&tokens, 1, b"then").unwrap();
    let cond_tokens = trim_semi(tokens.get(1..first_then).unwrap());
    assert!(condition_bodies(&text, cond_tokens).is_empty());
}

/// Two operators in the condition yield two bodies in operator order.
#[test]
fn condition_bodies_multiple_operators_are_in_order() {
    let line = b"if cat <<A && cat <<B; then fi\nbodyA\nA\nbodyB\nB";
    let text = ScriptText::new(
        ShortCStr::from_vec(line.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    );
    let tokens = tokenize_statement(line).unwrap();
    let first_then = find_preceded_by_semi(&tokens, 1, b"then").unwrap();
    let cond_tokens = trim_semi(tokens.get(1..first_then).unwrap());
    let bodies = condition_bodies(&text, cond_tokens);
    assert_eq!(
        bodies,
        vec![
            ShortCStr::from_vec(b"bodyA\nA\n".to_vec()).unwrap(),
            ShortCStr::from_vec(b"bodyB\nB".to_vec()).unwrap(),
        ]
    );
}

/// The condition claims only its own regions of the line's body stream: the
/// `<<` operators written on the condition's physical line **before** the
/// condition already claimed the head of the stream, so an `elif` condition
/// skips them and takes its own region.
#[test]
fn condition_bodies_skip_the_regions_claimed_before_them() {
    // `cat <<X` sits in the `then` body on the same physical line as the elif
    // condition: region 0 is its, the elif condition takes region 1.
    let line = b"if false; then cat <<X; elif cat <<A; then echo b; fi\nbodyX\nX\nbodyA\nA";
    let bodies = cond_bodies_of(line, b"elif");
    assert_eq!(
        bodies,
        vec![ShortCStr::from_vec(b"bodyA\nA".to_vec()).unwrap()]
    );

    // Two operators in the condition, one claimed before it: the claimed
    // region is skipped and the condition takes the next two, in order.
    let line = b"if false; then cat <<X; elif cat <<A && cat <<B; then echo b; fi\nbodyX\nX\nbodyA\nA\nbodyB\nB";
    let bodies = cond_bodies_of(line, b"elif");
    assert_eq!(
        bodies,
        vec![
            ShortCStr::from_vec(b"bodyA\nA\n".to_vec()).unwrap(),
            ShortCStr::from_vec(b"bodyB\nB".to_vec()).unwrap(),
        ]
    );
}

/// Nothing precedes an ordinary condition on its physical line: `skip` is 0
/// and the condition takes region 0 — the shape every `if`/`while`/`until`
/// condition and every multi-line block has.
#[test]
fn condition_bodies_skip_is_zero_on_a_bare_condition_line() {
    let line = b"if false; then echo a; elif cat <<E; then echo b; fi\nbodyE\nE";
    let bodies = cond_bodies_of(line, b"elif");
    assert_eq!(
        bodies,
        vec![ShortCStr::from_vec(b"bodyE\nE".to_vec()).unwrap()]
    );
}

/// A line whose body stream is incomplete yields no bodies: the regions are
/// allocated for **all** the line's operators in order, so a missing delimiter
/// line anywhere on the line leaves the condition with nothing to claim (the
/// parse layer reports the missing delimiter).
#[test]
fn condition_bodies_of_an_incomplete_line_stream_are_empty() {
    // The `then` arm's `<<X` has no body: its delimiter line is searched first
    // and is missing, so the stream has no regions at all.
    let line = b"if false; then cat <<X; elif cat <<A; then echo b; fi\nbodyA\nA";
    assert!(cond_bodies_of(line, b"elif").is_empty());
}

/// The region window is **bounded to the condition's own physical line**: an
/// unterminated heredoc on an *earlier* line never enters it, so the condition
/// keeps its bodies. (This is the observable half of the window's anchor: the
/// success path is invariant to it, the missing-delimiter path is not —
/// LESSONS.)
#[test]
fn condition_bodies_window_is_bounded_to_the_conditions_line() {
    // `<<Z` sits on a line before the condition's and has no delimiter line in
    // the text; the condition's own line is complete, so it claims its region.
    let line = b"cat <<Z\nif cat <<A; then echo y; fi\nbodyA\nA";
    assert_eq!(
        cond_bodies_of(line, b"if"),
        vec![ShortCStr::from_vec(b"bodyA\nA".to_vec()).unwrap()]
    );
}

/// `line_start_of` is panic-free on a text starting with a newline: the anchor
/// is byte 1 and the condition keeps its region. (This is what kills the
/// `cond_bodies.rs:63:75` `+` → `-` mutant: at `n = 0` it computes `0 - 1`,
/// a `usize` underflow — LESSONS.)
#[test]
fn condition_bodies_of_a_text_starting_with_a_newline() {
    let line = b"\nif cat <<A; then echo y; fi\nbodyA\nA";
    assert_eq!(
        cond_bodies_of(line, b"if"),
        vec![ShortCStr::from_vec(b"bodyA\nA".to_vec()).unwrap()]
    );
}

/// The exact region a multi-line block's condition claims when the operator it
/// reads is written **mid-line**: the stream starts after the condition line's
/// newline, so the region is the next line's text up to the operator's
/// delimiter line (the shape bash rejects with a syntax error — the accepted
/// divergence README records, and `script.rs` bails on it too). This is the
/// text that exercises `line_start_of`'s non-trivial window: on this **success
/// path** the operators its window spans and the `skip` it computes shift
/// together, so the extraction is invariant to the window's start (LESSONS;
/// `condition_bodies_window_is_bounded_to_the_conditions_line` pins the
/// path that is *not* invariant).
#[test]
fn condition_bodies_claim_the_mid_line_region_of_a_multi_line_condition() {
    // `while cat <<Z; do` closes on line 1; `cat <<A` sits on line 2, and Z's
    // body stream starts after that line's newline.
    let line = b"while cat <<Z; do\ncat <<A; echo x\nZ\nbodyZ\nA\nbodyA\nA";
    let text = ScriptText::new(
        ShortCStr::from_vec(line.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    );
    let tokens = tokenize_statement(line).unwrap();
    let do_idx = find_preceded_by_semi(&tokens, 1, b"do").unwrap();
    let cond_tokens = trim_semi(tokens.get(1..do_idx).unwrap());
    assert_eq!(
        condition_bodies(&text, cond_tokens),
        vec![ShortCStr::from_vec(b"cat <<A; echo x\nZ\n".to_vec()).unwrap()]
    );
}

/// The block text of a segment in append mode carries **only** the block's own
/// regions: the condition's extraction reads exactly that region, with nothing
/// of the neighbouring statements' bodies in its stream.
#[test]
fn condition_bodies_of_a_pre_sliced_block_text() {
    // `segment/block/keyword.rs` produces this text for
    // `cat <<X; if cat <<A; …; fi; cat <<B`: the pre-block and trailing
    // regions are not in it.
    let line = b"if cat <<A; then echo y; fi\nbodyA\nA";
    let text = ScriptText::new(
        ShortCStr::from_vec(line.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    );
    let tokens = tokenize_statement(line).unwrap();
    let first_then = find_preceded_by_semi(&tokens, 1, b"then").unwrap();
    let cond_tokens = trim_semi(tokens.get(1..first_then).unwrap());
    assert_eq!(
        condition_bodies(&text, cond_tokens),
        vec![ShortCStr::from_vec(b"bodyA\nA".to_vec()).unwrap()]
    );
}

/// The condition of the arm opened by `keyword` (`elif`), as the parse layer
/// extracts it from a full block text.
fn cond_bodies_of(line: &[u8], keyword: &[u8]) -> Vec<ShortCStr> {
    let text = ScriptText::new(
        ShortCStr::from_vec(line.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    );
    let tokens = tokenize_statement(line).unwrap();
    let kw = tokens
        .iter()
        .position(|(t, _, _, _, _)| t.eq_bytes(keyword))
        .unwrap();
    let then = find_preceded_by_semi(&tokens, kw + 1, b"then").unwrap();
    condition_bodies(&text, trim_semi(tokens.get(kw + 1..then).unwrap()))
}
