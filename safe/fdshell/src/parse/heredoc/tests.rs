#![allow(clippy::unwrap_used, clippy::indexing_slicing)]
use super::{delimiter_token_indices, is_operator, layout, operator_count};
use crate::error::parse::ParseError;
use crate::parse::token::tokenize_statement;
use alloc::vec;
use alloc::vec::Vec;
use sys::ShortCStr;

/// Tokenize (with heredoc bodies blanked) and extract the bodies.
fn specs(line: &[u8]) -> Vec<super::HeredocBody> {
    let tokens = tokenize_statement(line).unwrap();
    layout(line, &tokens).unwrap()
}

#[test]
fn layout_attached_delimiter_expands() {
    let s = specs(b"cat <<EOF\nbody\nEOF");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"body\n".to_vec()).unwrap());
    assert!(s[0].expand);
}

#[test]
fn layout_separate_delimiter_expands() {
    let s = specs(b"cat << EOF\nbody\nEOF");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"body\n".to_vec()).unwrap());
    assert!(s[0].expand);
}

#[test]
fn layout_quoted_delimiter_is_literal() {
    let s = specs(b"cat <<\"Q\"\nx\nQ");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"x\n".to_vec()).unwrap());
    assert!(!s[0].expand);
}

#[test]
fn layout_empty_body() {
    let s = specs(b"cat <<EOF\nEOF");
    assert_eq!(s.len(), 1);
    assert!(s[0].body.is_empty());
}

#[test]
fn layout_empty_quoted_delimiter() {
    // `<<""` mirrors `<<<""`: the body ends at the first blank line, which
    // must be part of the statement span.
    let s = specs(b"cat <<\"\"\nbody\n\n");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"body\n".to_vec()).unwrap());
    assert!(!s[0].expand);
}

#[test]
fn layout_quoted_newline_on_command_line() {
    // The first unquoted newline ends the command line; a newline inside the
    // quotes on it is not a line break.
    let s = specs(b"cat <<EOF \"a\nb\"\nbody\nEOF");
    assert_eq!(s.len(), 1);
    assert_eq!(s[0].body, ShortCStr::from_vec(b"body\n".to_vec()).unwrap());
}

#[test]
fn layout_body_is_opaque_to_tokenizer() {
    // Unbalanced quotes in the body must not break tokenization.
    let s = specs(b"cat <<EOF\n\"unbalanced\nEOF\necho \"x\"");
    assert_eq!(s.len(), 1);
    assert_eq!(
        s[0].body,
        ShortCStr::from_vec(b"\"unbalanced\n".to_vec()).unwrap()
    );
}

#[test]
fn layout_missing_newline_is_unterminated() {
    let line = b"cat <<EOF";
    let tokens = tokenize_statement(line).unwrap();
    assert!(matches!(
        layout(line, &tokens).unwrap_err().current_context(),
        ParseError::UnterminatedHeredoc { .. }
    ));
}

#[test]
fn layout_empty_delimiter_eof_without_blank_line_is_unterminated() {
    // A zero-byte final line (EOF right after the body) is not the blank
    // delimiter line: the statement is unterminated, not silently cut.
    let line = b"cat <<\"\"\nbody\n";
    let tokens = tokenize_statement(line).unwrap();
    assert!(matches!(
        layout(line, &tokens).unwrap_err().current_context(),
        ParseError::UnterminatedHeredoc { .. }
    ));
}

#[test]
fn layout_missing_delimiter_is_unterminated() {
    let line = b"cat <<NOPE\nbody\nnothing";
    let tokens = tokenize_statement(line).unwrap();
    assert!(matches!(
        layout(line, &tokens).unwrap_err().current_context(),
        ParseError::UnterminatedHeredoc { .. }
    ));
}

#[test]
fn layout_bare_operator_at_eol_is_invalid_redirect() {
    let line = b"cat <<";
    let tokens = tokenize_statement(line).unwrap();
    assert!(matches!(
        layout(line, &tokens).unwrap_err().current_context(),
        ParseError::InvalidRedirect
    ));
}

#[test]
fn layout_bare_operator_before_semicolon_is_invalid_redirect() {
    let line = b"cat << ;";
    let tokens = tokenize_statement(line).unwrap();
    assert!(matches!(
        layout(line, &tokens).unwrap_err().current_context(),
        ParseError::InvalidRedirect
    ));
}

#[test]
fn layout_here_string_is_not_heredoc() {
    let line = b"cat <<<x";
    let tokens = tokenize_statement(line).unwrap();
    assert!(layout(line, &tokens).unwrap().is_empty());
}

#[test]
fn layout_attached_word_is_not_operator() {
    let s = specs(b"x<<EOF\nEOF");
    assert!(s.is_empty());
}

#[test]
fn layout_substitution_closing_paren_keeps_word() {
    // A `$( )`-closing `)` does not split the word: `$(x)<<EOF` is one token
    // that does not start with `<<`, so it is argument text, not an operator.
    // The body lines are real statements, never swallowed.
    let line = b"cat $(x)<<EOF\nb\nEOF";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(tokens.len(), 6);
    assert!(
        tokens[1].0.eq_bytes(b"$(x)<<EOF"),
        "the token must stay whole"
    );
    assert!(layout(line, &tokens).unwrap().is_empty());
}

#[test]
fn layout_bare_operator_before_redirect_word_is_invalid() {
    // A bare `<<` takes the next word as delimiter; a word shaped like a
    // redirect/operator is not a delimiter.
    for line in [b"cat << <x", b"cat << >x", b"cat << &x", b"cat << %x"] {
        let tokens = tokenize_statement(line).unwrap();
        assert!(
            matches!(
                layout(line, &tokens).unwrap_err().current_context(),
                ParseError::InvalidRedirect
            ),
            "line={:?}",
            core::str::from_utf8(line).unwrap()
        );
    }
}

#[test]
fn operator_count_skips_command_word() {
    // index 0 is the command word even when it looks like an operator.
    let line = b"<<EOF x";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(operator_count(&tokens), 0);
}

#[test]
fn operator_count_builtin_prefix() {
    // The builtin prefix shifts the command word; `<<EOF` is still an operator.
    let line = b"builtin cat <<EOF";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(operator_count(&tokens), 1);
}

#[test]
fn operator_count_pipe_position_is_not_operator() {
    // The word right after `|` is a command word, not an operator.
    let line = b"a | <<B";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(operator_count(&tokens), 0);
}

#[test]
fn operator_count_per_stage() {
    let line = b"cat <<A | tr <<B";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(operator_count(&tokens[0..2]), 1);
    assert_eq!(operator_count(&tokens[3..5]), 1);
}

#[test]
fn is_operator_bounds_guard() {
    let line = b"cat <<EOF";
    let tokens = tokenize_statement(line).unwrap();
    assert!(!is_operator(&tokens, 0));
    assert!(!is_operator(&tokens, tokens.len() + 1));
}

#[test]
fn delimiter_indices_bare_form_takes_next_token() {
    let line = b"cat << {a,b}";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(delimiter_token_indices(&tokens), vec![2]);
}

#[test]
fn delimiter_indices_attached_form_is_operator_token() {
    let line = b"cat <<{a,b}";
    let tokens = tokenize_statement(line).unwrap();
    assert_eq!(delimiter_token_indices(&tokens), vec![1]);
}

#[test]
fn delimiter_indices_bare_form_at_end_of_input() {
    // A bare `<<` with no following word protects nothing.
    let line = b"cat <<";
    let tokens = tokenize_statement(line).unwrap();
    assert!(delimiter_token_indices(&tokens).is_empty());
}
