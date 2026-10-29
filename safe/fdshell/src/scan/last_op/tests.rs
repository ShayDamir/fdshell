#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::{ends_on_operator, walk};

/// The position of the `n`th (0-based) newline in `text` — the index the segment
/// scanner stands on when it asks the predicate.
fn newline(text: &[u8], n: usize) -> usize {
    text.iter()
        .enumerate()
        .filter(|&(_, &b)| b == b'\n')
        .nth(n)
        .map(|(i, _)| i)
        .unwrap_or(text.len())
}

// The plain pair: a line that ends on a bare operator is a join point, a line
// whose last token is a word is not.
#[test]
fn bare_operator_at_the_line_end_is_the_last_token() {
    let text = b"false &&\necho b";
    assert!(ends_on_operator(text, newline(text, 0)));
}

#[test]
fn a_word_token_behind_the_operator_takes_the_answer() {
    let text = b"echo a && b\necho c";
    assert!(!ends_on_operator(text, newline(text, 0)));
}

// Kill pins for the walk's arms — each shape answers "no" because the operator
// is word data, not the text's last token.

#[test]
fn an_operator_inside_a_quoted_word_is_word_data() {
    let text = b"echo a \"b &&\"\necho c";
    assert!(!ends_on_operator(text, newline(text, 0)));
}

#[test]
fn an_operator_inside_an_escape_pair_is_word_data() {
    // The pair is consumed together, so only a lone `&` is left (task #326).
    let text = b"echo a b\\&&\necho c";
    assert!(!ends_on_operator(text, newline(text, 0)));
}

#[test]
fn an_operator_inside_a_substitution_is_word_data() {
    let text = b"echo a $(b && c)\necho c";
    assert!(!ends_on_operator(text, newline(text, 0)));
}

#[test]
fn an_operator_inside_a_closed_group_is_not_the_last_token() {
    // The group's own words and its closer are tokens behind the operator; an
    // *open* group at the line end keeps the operator as the last token.
    let text = b"echo a { b && c; }\necho c";
    assert!(!ends_on_operator(text, newline(text, 0)));
    let open = b"true && { echo a &&\necho b; }";
    assert!(ends_on_operator(open, newline(open, 0)));
}

#[test]
fn an_operator_inside_a_region_is_data() {
    // The `&&` sits in the body, so the walk skips it with the region.
    let text = b"cat <<B\nx && y\nB\necho c";
    assert!(!ends_on_operator(text, newline(text, 1)));
}

#[test]
fn an_operator_inside_a_comment_is_not_a_token() {
    let text = b"echo a #c &&\necho c";
    assert!(!ends_on_operator(text, newline(text, 0)));
}

#[test]
fn a_lone_async_or_pipe_byte_at_the_line_end_is_not_the_operator() {
    // The 2-char match must not read the byte it stands on as its partner: a
    // lone `&` is the async operator (task #326) and a lone `|` cannot span a
    // newline (LESSONS: "A trailing `|` is a hard parse error, not a REPL
    // continuation"), so neither joins.
    for text in [b"echo a &\necho b" as &[u8], b"echo a |\necho b"] {
        assert!(!ends_on_operator(text, newline(text, 0)));
    }
}

#[test]
fn a_glued_pair_counts_as_an_operator_token() {
    // Task #496: `false&&` is the word `false` then the `&&` token.
    let text = b"false&&\necho b";
    assert!(ends_on_operator(text, newline(text, 0)));
}

// Comment text is ignored: the predicate reads it as the lexer does, so an
// operator *before* a comment stays the last token (the REPL's own
// whitespace-tail rule is what refuses this input — task #492).
#[test]
fn an_operator_before_a_comment_stays_the_last_token() {
    let text = b"false && #c\necho b";
    assert!(ends_on_operator(text, newline(text, 0)));
}

// The REPL's granularity: `at == text.len() + 1` includes the end-of-input
// boundary, whose heredoc shape the walk reads through `skip_region`.
#[test]
fn the_end_of_input_boundary_is_included_at_the_repl_granularity() {
    let text = b"false &&";
    assert_eq!(walk(text, text.len() + 1).1, Some((6, 2)));
    // A balanced scope keeps its closer behind the operator, so the operator is
    // no longer the last token — the REPL's whitespace-tail rule decides it.
    let quoted = b"echo \"a && b\"";
    assert_eq!(walk(quoted, quoted.len() + 1).1, None);
    let tail = b"false && #c";
    assert_eq!(walk(tail, tail.len() + 1).1, Some((6, 2)));
    // A word token behind the operator takes the answer away.
    let word = b"x && $(true";
    assert_eq!(walk(word, word.len() + 1).1, None);
}
