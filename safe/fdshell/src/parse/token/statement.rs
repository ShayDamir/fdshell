//! Statement tokenization: the heredoc-aware entry point of the tokenizer.

use super::super::Token;
use crate::error::parse::ParseError;
use alloc::vec::Vec;
use error_stack::Report;

/// Tokenize a statement whose heredoc bodies are opaque: the body spans are
/// blanked in a copy first, so arbitrary body bytes — including unbalanced
/// quotes — never break tokenization. Byte offsets are preserved.
///
/// A delimiter (terminator) line is heredoc structure too, and it travels
/// **inside** the statement text (`cond/part.rs::reconstruct` appends the
/// bodies to the command bytes). Its *syntax* bytes are blanked, so a
/// terminator shaped like a command line — `a <<b`, `a|b`, `a$(b`, `a"b` —
/// contributes no operator, no pipe, no substitution and no unbalanced quote to
/// the parse of the statement that carries it. Its **words stay**: every text
/// slice of the block machinery measures its extent by tokens
/// (`parse/semi.rs::verbatim`, `token_range`), so blanking the whole line would
/// truncate the verbatim slice at the terminator's start and destroy the
/// append-mode body+terminator text. The newline stays for the same reason: the
/// `;` token it yields is what keeps the statement behind a terminator a
/// separate statement. The byte-level partner of this rule is
/// `scan::heredoc::operator_count_opaque` (the two counts must agree, LESSONS).
pub(crate) fn tokenize_statement(line: &[u8]) -> Result<Vec<Token>, Report<ParseError>> {
    let regions = crate::scan::heredoc::body_regions(line);
    let terminators = crate::scan::heredoc::terminator_spans(line);
    if regions.is_empty() && terminators.is_empty() {
        return super::tokenize(line);
    }
    let mut masked: Vec<u8> = line.to_vec();
    for (start, end) in regions {
        if let Some(body) = masked.get_mut(start..end) {
            for b in body.iter_mut() {
                *b = b' ';
            }
        }
    }
    for (start, end) in terminators {
        if let Some(word) = masked.get_mut(start..end) {
            for b in word.iter_mut() {
                if is_structural(*b) {
                    *b = b' ';
                }
            }
        }
    }
    super::tokenize(&masked)
}

/// A byte the tokenizer reads as syntax: a POSIX metacharacter, a quote
/// operator, an expansion/escape trigger, or fdshell's `%` capture marker.
/// `{`/`}` are deliberately **not** listed: brace expansion runs on the
/// original text in `brace_expand`, where the terminator protection
/// (`brace_expand/protect::terminator_word`) already applies, and blanking a
/// brace here would change the token text that protection matches.
/// `pub(super)` for the byte-set pin in `token/tests.rs`.
pub(super) fn is_structural(b: u8) -> bool {
    matches!(
        b,
        b';' | b'&'
            | b'('
            | b')'
            | b'<'
            | b'>'
            | b'|'
            | b'$'
            | b'`'
            | b'"'
            | b'\''
            | b'\\'
            | b'#'
            | b'~'
            | b'%'
            | b' '
            | b'\t'
    )
}
