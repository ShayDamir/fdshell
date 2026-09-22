#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::{body_regions, delimiter_word, skip, skip_region};
use alloc::vec;

#[test]
fn delimiter_word_strips_one_quote_pair() {
    let raw = b"EOF";
    assert_eq!(delimiter_word(raw), (b"EOF" as &[u8], false));
    let raw = b"\"Q\"";
    assert_eq!(delimiter_word(raw), (b"Q" as &[u8], true));
    let raw = b"\"\"";
    assert_eq!(delimiter_word(raw), (b"" as &[u8], true));
    // One-sided or trailing bytes: not a quote pair, kept verbatim.
    let raw = b"\"Q";
    assert_eq!(delimiter_word(raw), (b"\"Q" as &[u8], false));
    let raw = b"Q\"";
    assert_eq!(delimiter_word(raw), (b"Q\"" as &[u8], false));
    let raw = b"\"Q\"x";
    assert_eq!(delimiter_word(raw), (b"\"Q\"x" as &[u8], false));
}

// The resume index is the newline terminating the last delimiter line, or
// `line.len()` when that line is last.
#[test]
fn skip_resume_positions() {
    assert_eq!(skip(b"cat <<EOF\nbody\nEOF", 0, 9), Some(18));
    assert_eq!(skip(b"cat <<EOF\nbody\nEOF\n", 0, 9), Some(18));
    // Empty body: the delimiter line follows the command line directly.
    assert_eq!(skip(b"cat <<EOF\nEOF", 0, 9), Some(13));
}

// A bare `<<` takes the next word as the delimiter.
#[test]
fn skip_bare_form_uses_next_word() {
    assert_eq!(skip(b"cat << Q\nbody\nQ", 0, 8), Some(15));
}

// An attached delimiter may start with a non-word byte (`&x`): the attached
// path takes the raw suffix as-is, where the bare-`<<` fallback rejects a
// next word starting with `<`, `>`, `&`, or `%`.
#[test]
fn skip_attached_delimiter_starting_with_non_word_char() {
    assert_eq!(skip(b"wc -c <<&x\n&x", 0, 10), Some(13));
}

// A near-miss delimiter line is body content, not the terminator.
#[test]
fn skip_near_miss_delimiter_is_body() {
    assert_eq!(skip(b"cat <<EOF\nxEOF\nEOF", 0, 9), Some(18));
}

// The body is opaque to `skip`: quote bytes do not affect the resume.
#[test]
fn skip_body_with_quote_byte() {
    assert_eq!(skip(b"cat <<EOF\n\"a\nEOF", 0, 9), Some(16));
}

// A bare `<<` at EOL has no delimiter word: not a heredoc command line.
#[test]
fn skip_bare_heredoc_at_eol_is_none() {
    assert_eq!(skip(b"cat <<", 0, 6), None);
}

// `;` after the operator ends the run before any newline: no extension.
#[test]
fn skip_semicolon_after_operator_is_none() {
    assert_eq!(skip(b"cat << ;", 0, 7), None);
}

// A missing delimiter line: `None`.
#[test]
fn skip_missing_delimiter_is_none() {
    assert_eq!(skip(b"cat <<NOPE\nbody\nnothing", 0, 10), None);
}

// An attached word before `<<` (no word break) is not an operator.
#[test]
fn skip_attached_word_is_not_operator() {
    assert_eq!(skip(b"x<<EOF\nEOF", 0, 6), None);
}

// `<<<` is the here-string form, not a heredoc operator.
#[test]
fn skip_triple_chevron_is_not_operator() {
    assert_eq!(skip(b"cat <<<EOF\nEOF", 0, 10), None);
}

// A `|` before `<<` (only whitespace between) is a pipeline position: the
// word is a command, not an operator. The run ends at the newline (9).
#[test]
fn skip_pipe_position_is_not_operator() {
    assert_eq!(skip(b"a | <<EOF\nEOF", 0, 9), None);
}

// The region is the body lines only: from the first body byte to the start
// of the last delimiter line; the span ends at the resume newline.
#[test]
fn skip_region_covers_body_lines_only() {
    let line = b"cat <<EOF\nbody\nEOF\necho x";
    assert_eq!(skip_region(line, 0, 9), Some(((10, 15), 18, 18)));
}

// An empty final delimiter is a zero-length line: the statement span extends
// through the blank line's terminating newline.
#[test]
fn skip_region_empty_delimiter_span_includes_blank_line() {
    let line = b"cat <<\"\"\nbody\n\nx";
    assert_eq!(skip_region(line, 0, 8), Some(((9, 14), 14, 15)));
    assert_eq!(skip(b"cat <<\"\"\nbody\n\necho A", 0, 8), Some(14));
}

// `<<` inside double quotes or backticks is not an operator.
#[test]
fn skip_quoted_and_backtick_chevron_is_not_operator() {
    assert_eq!(skip(b"cat \" <<EOF\"\nEOF\"", 0, 12), None);
    assert_eq!(skip(b"cat ` <<EOF`\nEOF`", 0, 12), None);
}

// A single-`<` redirect is not a heredoc operator.
#[test]
fn skip_single_chevron_is_not_operator() {
    assert_eq!(skip(b"cat <file\nile", 0, 9), None);
}

// `<<<` in the run is ignored; the later `<<EOF` is the operator.
#[test]
fn skip_triple_then_double_chevron() {
    assert_eq!(skip(b"cat x <<<y <<EOF\nbody\nEOF", 0, 16), Some(25));
}

// A bare `<<` followed by another operator-shaped word has no delimiter word.
#[test]
fn skip_bare_heredoc_before_chevron_word_is_none() {
    assert_eq!(skip(b"cat << <<x\n<<x", 0, 10), None);
}

// Two identical delimiters: each command-line `<<` consumes its own line.
#[test]
fn skip_two_heredocs_same_delimiter() {
    assert_eq!(skip(b"cat <<Q <<Q\nx\nQ\ny\nQ", 0, 11), Some(19));
}

// Two empty-delimiter operators need two blank lines; one is not enough.
#[test]
fn skip_two_empty_delimiters_need_two_blank_lines() {
    assert_eq!(skip(b"cat <<\"\" <<\"\"\n\n", 0, 13), None);
}

// An empty delimiter with no blank line is unterminated. A zero-byte final
// line (EOF right after the body's newline) never matches.
#[test]
fn skip_empty_delimiter_without_blank_line_is_none() {
    assert_eq!(skip(b"cat <<\"\"\n", 0, 8), None);
    assert_eq!(skip(b"wc -c <<\"\"\nbody\n", 0, 10), None);
}

// A `$( )`-closing `)` does not break the word (the tokenizer keeps
// `$(x)<<EOF` whole), so the attached `<<EOF` is not an operator: the body
// lines are not swallowed. A top-level `)` still breaks the word.
#[test]
fn skip_substitution_closing_paren_is_not_word_break() {
    assert_eq!(skip(b"cat $(x)<<EOF\nb\nEOF", 0, 13), None);
    assert_eq!(skip(b"echo (x)<<EOF\nb\nEOF", 0, 13), Some(19));
}

#[test]
fn body_regions_finds_body_span() {
    let line = b"cat <<EOF\nbody\nEOF\necho x";
    assert_eq!(body_regions(line), vec![(10, 15)]);
}

#[test]
fn body_regions_none_for_plain_statements() {
    assert!(body_regions(b"echo hi").is_empty());
    assert!(body_regions(b"cat <<<hi\necho x").is_empty());
    assert!(body_regions(b"x<<EOF\nEOF").is_empty());
}

// A `<<` inside a comment is comment text, not an operator: it must not
// register a body region or swallow the following delimiter-shaped line.
#[test]
fn body_regions_comment_heredoc_is_not_a_region() {
    assert!(body_regions(b"# <<EOF\na\nEOF").is_empty());
    assert!(body_regions(b"echo hi # <<EOF\nEOF").is_empty());
}

// A heredoc inside a block body is found by the whole-text scan; body quote
// bytes do not disturb the scan.
#[test]
fn body_regions_inside_block_body() {
    let line = b"if t; then cat <<EOF\n\"q\nEOF\nfi";
    assert_eq!(body_regions(line), vec![(21, 24)]);
}

// Two heredoc commands in one text yield one region each, in order.
#[test]
fn body_regions_multiple_commands() {
    let line = b"cat <<A\nx\nA\ncat <<B\ny\nB";
    assert_eq!(body_regions(line), vec![(8, 10), (20, 22)]);
}
