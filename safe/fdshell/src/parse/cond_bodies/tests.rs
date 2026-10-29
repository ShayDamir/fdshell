//! Tests for the condition body extraction.

#![allow(clippy::unwrap_used)]

use super::claimed_bodies;
use super::claimed_regions;
use super::condition_bodies;
use crate::parse::Token;
use crate::parse::semi::{Term, closing_keyword_index, find_preceded_by, token_range, trim_semi};
use crate::parse::token::tokenize_statement;
use crate::scan::heredoc::body_regions;
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
    let first_then = find_preceded_by(&tokens, 1, b"then", line, Term::Semi).unwrap();
    let cond_tokens = trim_semi(tokens.get(1..first_then).unwrap());
    let bodies = condition_bodies(&text, cond_tokens);
    assert_eq!(
        bodies,
        vec![ShortCStr::from_vec(b"body\nEOF".to_vec()).unwrap()]
    );
}

/// The io-number form claims its region in the condition exactly like the
/// unprefixed one: `3<<A` is the condition's operator and the region after the
/// condition line is its body (task #276 — the fd-carrying operator is
/// position-independent; the integration twin is
/// `numbered_heredoc_composes_with_the_block_condition_and_loop_positions`).
#[test]
fn condition_bodies_extracted_from_an_io_number_operator() {
    let line = b"if cat 3<<A; then echo yes; fi\nbodyA\nA";
    let text = ScriptText::new(
        ShortCStr::from_vec(line.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    );
    let tokens = tokenize_statement(line).unwrap();
    let first_then = find_preceded_by(&tokens, 1, b"then", line, Term::Semi).unwrap();
    let cond_tokens = trim_semi(tokens.get(1..first_then).unwrap());
    assert_eq!(
        condition_bodies(&text, cond_tokens),
        vec![ShortCStr::from_vec(b"bodyA\nA".to_vec()).unwrap()]
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
    let first_then = find_preceded_by(&tokens, 1, b"then", line, Term::Semi).unwrap();
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
    let first_then = find_preceded_by(&tokens, 1, b"then", line, Term::Semi).unwrap();
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

/// A line whose **first** operator has no delimiter line claims the stream
/// (the end-of-file shape, task #364), so the condition written behind it —
/// the second operator — takes the empty region at the text end. bash warns
/// twice, prints `bodyA`/`A`, and the `then` arm runs at rc 0
/// (measured 2026-10-29). A missing delimiter line **behind** the first
/// operator starves nothing — see
/// `claimed_bodies_of_an_arm_survive_a_later_unterminated_operator`.
#[test]
fn condition_bodies_of_an_incomplete_line_stream_take_the_empty_tail() {
    // The `then` arm's `<<X` has no body: its delimiter line is searched first,
    // never arrives, and claims the whole stream; the condition gets the empty
    // region behind it.
    let line = b"if false; then cat <<X; elif cat <<A; then echo b; fi\nbodyA\nA";
    assert_eq!(
        cond_bodies_of(line, b"elif"),
        vec![ShortCStr::from_vec(b"".to_vec()).unwrap()]
    );
}

/// The region window is **bounded to the condition's own physical line** — and
/// an unterminated heredoc on an *earlier* line resolves at the end of the
/// text, so its end-of-file claim swallows the condition's own line along with
/// the rest (task #364). bash reads the same lines as body data and prints
/// them at rc 0 with its warning (measured 2026-10-29). (This is the observable
/// half of the window's anchor: the success path is invariant to it, the
/// end-of-file path decides — LESSONS.)
#[test]
fn condition_bodies_window_is_bounded_to_the_conditions_line() {
    // `<<Z` sits on a line before the condition's and has no delimiter line in
    // the text: the end-of-file shape claims every line after the command line.
    let line = b"cat <<Z\nif cat <<A; then echo y; fi\nbodyA\nA";
    assert_eq!(body_regions(line), vec![(8, 43)]);
}

/// `scan::heredoc::line_start` is panic-free on a text starting with a newline: the anchor
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
/// text that exercises `scan::heredoc::line_start`'s non-trivial window: on this **success
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
    let do_idx = find_preceded_by(&tokens, 1, b"do", line, Term::Semi).unwrap();
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
    let first_then = find_preceded_by(&tokens, 1, b"then", line, Term::Semi).unwrap();
    let cond_tokens = trim_semi(tokens.get(1..first_then).unwrap());
    assert_eq!(
        condition_bodies(&text, cond_tokens),
        vec![ShortCStr::from_vec(b"bodyA\nA".to_vec()).unwrap()]
    );
}

/// The regions an **arm body** claims follow the same rule as a condition's:
/// the `<<` operators written on the same physical line **before** the arm
/// already claimed the head of the stream, so the `elif` arm skips the `then`
/// arm's region and claims only its own.
#[test]
fn claimed_regions_of_an_arm_skip_the_operators_written_before_them() {
    let line = b"if false; then cat <<A; elif true; then cat <<B; fi\nbodyA\nA\nbodyB\nB";
    let regions = arm_regions_of(line, b"elif");
    assert_eq!(regions.len(), 1);
    let &(s, e) = regions.first().unwrap();
    assert_eq!(line.get(s..e).unwrap(), b"bodyB\nB");
}

/// An arm with two operators claims both regions, in operator order.
#[test]
fn claimed_regions_of_an_arm_with_two_operators_are_in_order() {
    let line = b"if true; then cat <<A; cat <<B; fi\nbodyA\nA\nbodyB\nB";
    let regions = arm_regions_of(line, b"if");
    assert_eq!(regions.len(), 2);
    let contents: Vec<&[u8]> = regions
        .iter()
        .map(|&(s, e)| line.get(s..e).unwrap())
        .collect();
    assert_eq!(
        contents,
        vec![b"bodyA\nA\n".as_slice(), b"bodyB\nB".as_slice()]
    );
}

/// In extension mode (a multi-line block) the regions a slice claims sit
/// **inside** its token range — the discriminator `part_text` uses to leave
/// the slice's text verbatim.
#[test]
fn claimed_regions_of_an_extension_mode_slice_sit_inside_the_token_range() {
    let line = b"if true; then cat <<T\nbodyT\nT\nfi";
    let regions = arm_regions_of(line, b"if");
    assert_eq!(regions.len(), 1);
    let &(s, e) = regions.first().unwrap();
    assert_eq!(line.get(s..e).unwrap(), b"bodyT\nT\n");
    assert!(s < arm_token_range_end(line, b"if"));
}

/// The io-number form claims the same single region in an arm body written
/// across lines (the shape of the block body `if true; then cat 3<<T; fi`
/// carries its body text): the region is the body line plus the delimiter
/// line, inside the slice's token range (task #276).
#[test]
fn claimed_regions_of_an_io_number_operator_in_an_arm_body() {
    let line = b"if true; then cat 3<<T\nbodyT\nT\nfi";
    let regions = arm_regions_of(line, b"if");
    assert_eq!(regions.len(), 1);
    let &(s, e) = regions.first().unwrap();
    assert_eq!(line.get(s..e).unwrap(), b"bodyT\nT\n");
    assert!(s < arm_token_range_end(line, b"if"));
}

/// A slice with no `<<` operator claims no region.
#[test]
fn claimed_regions_of_a_slice_without_operators_are_empty() {
    let line = b"if true; then echo x; fi";
    assert!(arm_regions_of(line, b"if").is_empty());
}

/// Task #275: the line's regions are allocated **one entry per operator**, so an
/// unterminated operator behind an arm leaves the arm its own complete body, and
/// the missing one claims the stream tail — its bytes are its own text, empty
/// when the stream ran out (the parse layer decides that heredoc itself, task
/// #364; the integration twin is
/// `single_line_block_body_heredoc_of_an_incomplete_line_stream_prints_the_complete_arm`).
#[test]
fn claimed_bodies_of_an_arm_survive_a_later_unterminated_operator() {
    let line = b"if true; then cat <<T; elif cat <<E; then echo b; fi\nbodyT\nT";
    let text = text_of(line);
    let slice = arm_slice(line, b"if");
    let regions = claimed_regions(&text, &slice);
    assert_eq!(regions.len(), 2, "one entry per operator");
    let bodies = claimed_bodies(&text, &slice);
    assert_eq!(
        bodies.first().map(|b| b.as_slice()),
        Some(b"bodyT\nT\n".as_slice()),
        "the arm whose delimiter line arrived keeps its body"
    );
    assert_eq!(
        bodies.get(1).map(|b| b.as_slice()),
        Some(b"".as_slice()),
        "the unterminated operator claims the spent stream — empty bytes"
    );
}

/// The same shape's condition half: the `elif` condition's body of a partially
/// complete stream is the empty tail claim, so its rebuilt command line is the
/// end-of-file shape with an **empty** body (task #364; bash warns for it).
#[test]
fn condition_bodies_of_a_partial_stream_are_the_empty_tail() {
    let line = b"if true; then cat <<T; elif cat <<E; then echo b; fi\nbodyT\nT";
    assert_eq!(
        cond_bodies_of(line, b"elif"),
        vec![ShortCStr::from_vec(b"".to_vec()).unwrap()]
    );
}

/// The mirror of that shape: the line's **first** operator (`<<T`, in the `then`
/// arm) is the unterminated one, so it claims the stream by the same rule that
/// serves the operators behind it (task #364) — the arm carries the claimed
/// region and the `elif` condition takes the empty tail. bash warns twice,
/// prints `bodyE`/`E`, runs `echo b` at rc 0 (measured 2026-10-29; the
/// integration twin is
/// `a_line_whose_first_heredoc_is_unterminated_is_still_a_parse_error`).
#[test]
fn claimed_bodies_of_a_line_whose_first_operator_is_unterminated_claim_the_stream() {
    let line = b"if false; then cat <<T; elif cat <<E; then echo b; fi\nbodyE\nE";
    let text = text_of(line);
    assert_eq!(
        claimed_regions(&text, &arm_slice(line, b"if")),
        vec![(54, 61), (61, 61)]
    );
    assert_eq!(
        claimed_bodies(&text, &arm_slice(line, b"if")),
        vec![b"bodyE\nE\n".to_vec(), Vec::new()]
    );
    assert_eq!(
        cond_bodies_of(line, b"elif"),
        vec![ShortCStr::from_vec(b"".to_vec()).unwrap()]
    );
}

/// The pre-slice `skip` stays exact with a tail region present: a tail claim is
/// an entry of its own, so the condition written after one complete operator
/// takes its own region and never the tail.
#[test]
fn condition_bodies_skip_is_exact_when_a_tail_region_is_present() {
    let line = b"if true; then cat <<X; elif cat <<A; then cat <<E; fi\nbodyX\nX\nbodyA\nA";
    assert_eq!(
        cond_bodies_of(line, b"elif"),
        vec![ShortCStr::from_vec(b"bodyA\nA\n".to_vec()).unwrap()]
    );
}

/// A `ScriptText` over `line`, at the origin every block text carries.
fn text_of(line: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(line.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
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
        .position(|(t, _, _, _, _, _)| t.eq_bytes(keyword))
        .unwrap();
    let then = find_preceded_by(&tokens, kw + 1, b"then", line, Term::Semi).unwrap();
    condition_bodies(&text, trim_semi(tokens.get(kw + 1..then).unwrap()))
}

/// The regions claimed by the arm slice opened by the `then` following
/// `keyword`, cut the way the block parsers cut it.
fn arm_regions_of(line: &[u8], keyword: &[u8]) -> Vec<(usize, usize)> {
    let text = ScriptText::new(
        ShortCStr::from_vec(line.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    );
    claimed_regions(&text, &arm_slice(line, keyword))
}

/// The exclusive end of the arm slice's token range.
fn arm_token_range_end(line: &[u8], keyword: &[u8]) -> usize {
    let slice = arm_slice(line, keyword);
    token_range(&slice).1
}

fn arm_slice(line: &[u8], keyword: &[u8]) -> Vec<Token> {
    let tokens = tokenize_statement(line).unwrap();
    let closing = closing_keyword_index(&tokens, line).unwrap();
    let kw = tokens
        .iter()
        .position(|(t, _, _, _, _, _)| t.eq_bytes(keyword))
        .unwrap();
    let then = find_preceded_by(&tokens, kw + 1, b"then", line, Term::Semi).unwrap();
    trim_semi(tokens.get(then + 1..closing - 1).unwrap()).to_vec()
}
