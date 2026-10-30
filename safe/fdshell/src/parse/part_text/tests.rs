//! Tests for the arm-slice text builder (`part_text`).

#![allow(clippy::unwrap_used)]

use super::part_text;
use crate::parse::Token;
use crate::parse::semi::{Term, closing_keyword_index, find_preceded_by, trim_semi, verbatim};
use crate::parse::token::tokenize_statement;
use alloc::vec::Vec;
use sys::{Origin, Position, ScriptText, ShortCStr};

/// In append mode (a single-line block: the bodies sit past the closing
/// keyword) the arm's own regions are appended after a newline, so its text is
/// a self-contained heredoc; the slice's start position and origin are kept.
#[test]
fn part_text_appends_the_slice_regions_in_append_mode() {
    let line = b"if true; then cat <<T; fi\nbodyT\nT";
    let text = block_text(line);
    let slice = arm_slice(&text, b"if");
    let part = part_text(&text, &slice).unwrap();
    assert_eq!(part.data.as_bytes().unwrap(), b"cat <<T\nbodyT\nT");
    // The part starts at the slice's first token, not at the block's start.
    let at_cat = line.iter().position(|&b| b == b'c').unwrap();
    assert_eq!(part.start, text.subslice(at_cat, 1).unwrap().start);
    assert_eq!(part.origin, text.origin);
}

/// In extension mode (a multi-line block: the bodies are interleaved with the
/// arm's own lines) the text is exactly the verbatim slice — unchanged.
#[test]
fn part_text_is_verbatim_in_extension_mode() {
    let line = b"if true; then cat <<T\nbodyT\nT\nfi";
    let text = block_text(line);
    let slice = arm_slice(&text, b"if");
    let part = part_text(&text, &slice).unwrap();
    assert_eq!(part.data, verbatim(&text, &slice).unwrap().data);
    assert_eq!(part.data.as_bytes().unwrap(), b"cat <<T\nbodyT\nT");
}

/// A slice with no `<<` operator gets its verbatim text (no regions, no
/// appended newline).
#[test]
fn part_text_is_verbatim_without_operators() {
    let line = b"if true; then echo x; fi";
    let text = block_text(line);
    let slice = arm_slice(&text, b"if");
    let part = part_text(&text, &slice).unwrap();
    assert_eq!(part.data, verbatim(&text, &slice).unwrap().data);
    assert_eq!(part.data.as_bytes().unwrap(), b"echo x");
}

/// Two operators in one arm slice append **both** bodies, in operator order.
#[test]
fn part_text_appends_two_regions_in_operator_order() {
    let line = b"if true; then cat <<A; cat <<B; fi\nbodyA\nA\nbodyB\nB";
    let text = block_text(line);
    let slice = arm_slice(&text, b"if");
    let part = part_text(&text, &slice).unwrap();
    assert_eq!(
        part.data.as_bytes().unwrap(),
        b"cat <<A; cat <<B\nbodyA\nA\nbodyB\nB"
    );
}

/// An arm on a line that already carries an **earlier** arm's heredoc claims
/// only its own region: the `elif` condition's `<<C` owns region `bodyC\nC`,
/// and the `then` arm below it appends only `bodyT\nT`.
#[test]
fn part_text_appends_only_the_regions_the_slice_claims() {
    let line = b"if false; then :; elif cat <<C; then cat <<T; fi\nbodyC\nC\nbodyT\nT";
    let text = block_text(line);
    let slice = arm_slice(&text, b"elif");
    let part = part_text(&text, &slice).unwrap();
    assert_eq!(part.data.as_bytes().unwrap(), b"cat <<T\nbodyT\nT");
}

/// A region claimed at the end of the text — the end-of-file shape, whose
/// delimiter line never arrived (task #364) — travels with the slice verbatim:
/// the slice's own bytes, a newline, and the claimed lines exactly as the text
/// has them (bash prints them as the body, warns, and runs at rc 0).
#[test]
fn part_text_of_an_end_of_file_region_travels_with_the_slice() {
    let line = b"if true; then cat <<T; fi\nbodyT";
    let text = block_text(line);
    let slice = arm_slice(&text, b"if");
    let part = part_text(&text, &slice).unwrap();
    assert_eq!(part.data.as_bytes().unwrap(), b"cat <<T\nbodyT");
}

fn block_text(line: &[u8]) -> ScriptText {
    ScriptText::new(
        ShortCStr::from_vec(line.to_vec()).unwrap(),
        Position::new(1, 1),
        Origin::Shell,
    )
}

/// The arm slice opened by the `then` following `keyword`, cut the way the
/// block parsers cut it: the `;`-trimmed tokens between that `then` and the
/// token before the block's closing keyword.
fn arm_slice(text: &ScriptText, keyword: &[u8]) -> Vec<Token> {
    let line = text.as_bytes().unwrap();
    let tokens = tokenize_statement(line).unwrap();
    let closing = closing_keyword_index(&tokens, line).unwrap();
    let kw = tokens
        .iter()
        .position(|(t, _, _, _, _, _)| t.eq_bytes(keyword))
        .unwrap();
    let then = find_preceded_by(&tokens, kw + 1, b"then", line, Term::Semi).unwrap();
    trim_semi(tokens.get(then + 1..closing - 1).unwrap()).to_vec()
}

/// A terminator line whose word sits **mid-line** (its syntax bytes are blank for
/// the tokenizer, LESSONS "A heredoc terminator line is structure, not command
/// bytes") leaves its tail bytes behind the slice's last token, so extension mode
/// must own the **whole** terminator line: `cat <<$(x)` + `body` + `$(x)` keeps
/// its `)`, the byte scan matches the terminator, and the text behind it keeps its
/// owner. On `master` the stored text stopped at the surviving word `x`, the
/// terminator could never match, and the block reported a missing closer — the
/// integration rows are
/// `tests/heredoc.rs::a_delimiter_word_with_a_substitution_region_keeps_the_tail`.
/// The newline behind the terminator stays out of the slice exactly as it does on
/// the plain-word pin above (the `;` it tokenizes as drops out of the body).
#[test]
fn extension_mode_owns_a_terminator_line_whose_word_sits_mid_line() {
    let line = b"if true; then cat <<$(x)\nbody\n$(x)\nfi";
    let text = block_text(line);
    let slice = arm_slice(&text, b"if");
    let part = part_text(&text, &slice).unwrap();
    assert_eq!(
        part.data.as_bytes().unwrap(),
        b"cat <<$(x)\nbody\n$(x)",
        "the slice ends at the terminator line's last byte, not at its word"
    );
}
