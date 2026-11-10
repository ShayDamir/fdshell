// mutants: -C --lib -C --test -C function_keyword -- -E 'test(/^parse::function_block::/) | binary(function_keyword)'

mod build_def;
mod definer_prefix;
mod definer_region;

pub(crate) use build_def::tokens_to_function;
pub(crate) use definer_prefix::redirect_before_the_brace;
pub(crate) use definer_region::definer_with_region;

use super::Token;
use super::word_quoted::{gated_word, keyword_word};
use crate::keywords::name_len;
use sys::{ScriptText, ShortCStr};

#[cfg_attr(test, derive(Debug))]
pub struct FunctionDef {
    pub name: ShortCStr,
    pub body: ScriptText,
}

/// A function-definition opener: an unquoted `{` token, and either the POSIX
/// `name() {` definer (`name(` then `)`) or the bash/ksh `function name [()] {`
/// definer. Every word of the definer is gate-tested ([`keyword_word`], task
/// #247): a quoted `function`, a quoted definer name, a quoted `name(` or a
/// quoted brace is word data, so bash's `` f'oo'(): not a valid identifier ``
/// shape defines nothing here either.
pub(crate) fn is_function_def(tokens: &[Token], line: &[u8]) -> bool {
    let open = tokens.iter().any(|tok| keyword_word(tok, line, b"{"));
    open && (posix_def(tokens, line) || keyword_def(tokens, line))
}

/// POSIX `name() {`: token 0 is an unquoted `name(` with a non-empty name,
/// token 1 is `)`.
fn posix_def(tokens: &[Token], line: &[u8]) -> bool {
    // The `)` is [`span_word`]'s: a quoted `")"` is a word, not the definer's
    // parameter-list paren (task #336, bash rejects `f()")" { …; }` rc 2).
    let parens = tokens.get(1).is_some_and(|tok| tok.span_word(b")"));
    parens
        && tokens
            .first()
            .is_some_and(|tok| tok.word.ends_with(b"(") && is_definer(tok, line))
}

/// bash/ksh `function name [()] {`: token 0 is the unquoted `function`
/// keyword, token 1 is an unquoted definer name.
fn keyword_def(tokens: &[Token], line: &[u8]) -> bool {
    let keyword = tokens
        .first()
        .is_some_and(|tok| keyword_word(tok, line, b"function"));
    keyword && tokens.get(1).is_some_and(|tok| is_definer(tok, line))
}

/// The definer word gate: a word with a definer name, past the word gate
/// [`gated_word`] (span test next to the scan-back — the POSIX definer token
/// carries its trailing `(` (`foo'('`), and `quoted_word` breaks its scan at
/// that `(`, never reaching the quote bytes before it.
///
/// Stays free: it reads the raw script text `line` through [`gated_word`], so its
/// data is not the token's own state (task #550).
fn is_definer(tok: &Token, line: &[u8]) -> bool {
    def_len(&tok.word).is_some() && gated_word(tok, line)
}

/// The definer word: the name token after `function` in the keyword form, the
/// first token (`name(`) in the POSIX form.
fn definer(tokens: &[Token]) -> Option<&ShortCStr> {
    let first = tokens.first().map(|Token { word: t, .. }| t);
    if first.is_some_and(|t| t.eq_bytes(b"function")) {
        return tokens.get(1).map(|Token { word: t, .. }| t);
    }
    first
}

/// The definer name length of a token; `None` = not a definer word.
fn def_len(word: &ShortCStr) -> Option<usize> {
    name_len(word.as_bytes().ok()?)
}

#[cfg(test)]
mod tests;
