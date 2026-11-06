#![allow(clippy::unwrap_used)]

use super::word_with_operand_end;

/// The byte index of the `<<` of `b` in `line`.
fn op_at(line: &[u8], b: &[u8]) -> usize {
    line.windows(b.len()).position(|w| w == b).unwrap()
}

/// The operator word and its operand are **one unit**, so the returned end is
/// past both — glued both directions (a `}` operand and a `{` operand, the pair
/// that differs only in the operand's shape), and the io-number form whose digits
/// begin the word (`3<<}`).
#[test]
fn glued_operator_word_and_operand_are_one_unit() {
    let close = b"cat <<} ; echo x";
    assert_eq!(word_with_operand_end(close, op_at(close, b"<<")), Some(7));
    let open = b"cat <<{ ; echo x";
    assert_eq!(word_with_operand_end(open, op_at(open, b"<<")), Some(7));
    let io = b"cat 3<<} x";
    assert_eq!(word_with_operand_end(io, op_at(io, b"3")), Some(8));
}

/// The bare form: the operator word and the operand word behind the whitespace
/// are one unit — the same two operand shapes, and the `<<-` marker rides on the
/// operator word (`<<- {`).
#[test]
fn bare_operator_word_takes_the_operand_after_it() {
    let close = b"cat << } ; echo x";
    assert_eq!(word_with_operand_end(close, op_at(close, b"<<")), Some(8));
    let open = b"cat << { ; echo x";
    assert_eq!(word_with_operand_end(open, op_at(open, b"<<")), Some(8));
    let dash = b"cat <<- { ; echo x";
    assert_eq!(word_with_operand_end(dash, op_at(dash, b"<<")), Some(9));
}

/// A **quoted** operand is word data — the helper answers `None` and the caller's
/// own quote tracking keeps owning the word (a quoted brace word still counts as
/// word data at the brace rules, the accepted divergence
/// `tests/group.rs::literal_brace_argument_in_a_block_body`). A byte that opens
/// no here-doc operator word, a bare `<<` with no operand, and the `<<<`
/// here-string are `None` too.
#[test]
fn quoted_operand_and_non_operator_bytes_answer_none() {
    let quoted = b"cat <<\"}\" ; echo x";
    assert_eq!(word_with_operand_end(quoted, op_at(quoted, b"<<")), None);
    let word = b"cat x <<y ; echo x";
    assert_eq!(word_with_operand_end(word, op_at(word, b"x")), None);
    let bare = b"cat << ; echo x";
    assert_eq!(word_with_operand_end(bare, op_at(bare, b"<<")), None);
    let string = b"cat <<<} ; echo x";
    assert_eq!(word_with_operand_end(string, op_at(string, b"<<")), None);
}

/// An operator word that runs to the **end of the line** has no operand word
/// behind it, so the helper answers `None` and the caller's byte walk continues at
/// the operator's own bytes; an operator whose operand is the last word of the
/// line is still one unit to the end.
#[test]
fn an_operator_word_at_the_end_of_the_line_has_no_operand() {
    let quoted = b"cat <<\"x";
    assert_eq!(word_with_operand_end(quoted, op_at(quoted, b"<<")), None);
    let nothing = b"cat <<";
    assert_eq!(word_with_operand_end(nothing, op_at(nothing, b"<<")), None);
    let last = b"cat <<}";
    assert_eq!(word_with_operand_end(last, op_at(last, b"<<")), Some(7));
    let bare_last = b"cat << }";
    assert_eq!(
        word_with_operand_end(bare_last, op_at(bare_last, b"<<")),
        Some(8)
    );
}

/// An escape pair and a `$( … )` region inside the operand word are consumed
/// whole — the walk `lines::delimiter_span` reads the delimiter word with (task
/// #372) — so the bytes they carry are inside the unit.
#[test]
fn escape_pair_and_substitution_stay_inside_the_unit() {
    let escape = b"cat <<a\\b ; echo x";
    assert_eq!(word_with_operand_end(escape, op_at(escape, b"<<")), Some(9));
    let subst = b"cat <<$(x) ; echo x";
    assert_eq!(word_with_operand_end(subst, op_at(subst, b"<<")), Some(10));
}
