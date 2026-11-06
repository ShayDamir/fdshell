mod definer_region;

pub(crate) use definer_region::definer_with_region;

use super::Token;
use super::part_text::part_text;
use super::semi::{List, block_body, ensure_before_closer, walk_until};
use super::word_quoted::{gated_word, keyword_word, span_word};
use crate::error::parse::ParseError;
use crate::keywords::{Closer, name_len};
use error_stack::{Report, ResultExt};
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
    let parens = tokens.get(1).is_some_and(|tok| span_word(tok, b")"));
    parens
        && tokens
            .first()
            .is_some_and(|tok| tok.0.ends_with(b"(") && is_definer(tok, line))
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
/// that `(`, never reaching the quote bytes before it).
fn is_definer(tok: &Token, line: &[u8]) -> bool {
    def_len(&tok.0).is_some() && gated_word(tok, line)
}

/// The definer word: the name token after `function` in the keyword form, the
/// first token (`name(`) in the POSIX form.
fn definer(tokens: &[Token]) -> Option<&ShortCStr> {
    let first = tokens.first().map(|(t, _, _, _, _, _)| t);
    if first.is_some_and(|t| t.eq_bytes(b"function")) {
        return tokens.get(1).map(|(t, _, _, _, _, _)| t);
    }
    first
}

/// The definer name length of a token; `None` = not a definer word.
fn def_len(word: &ShortCStr) -> Option<usize> {
    name_len(word.as_bytes().ok()?)
}

pub(crate) fn tokens_to_function(
    tokens: &[Token],
    text: &ScriptText,
) -> Result<FunctionDef, Report<ParseError>> {
    let line = text.as_bytes().change_context(ParseError::Never)?;
    let name = function_name(tokens)?;
    // `is_function_def` guarantees an unquoted `{` token exists, so the
    // gated brace searches below cannot fail on a well-formed definition.
    let open = tokens
        .iter()
        .position(|tok| keyword_word(tok, line, b"{"))
        .ok_or(ParseError::Never)?;
    // The closer is the shared walk's answer (task #432): the first word that
    // empties the pairing, seeded with **this** block's closer and started just
    // after the `{` word — the definer's twin of the group's token-0 opener, so
    // the definer's own brace is never its closer. A body region's words are
    // blanked by the tokenizer (task #266), so a `}`/`{`/`done` line written
    // **inside a here-doc body** is data and never closes the definition, and a
    // delimiter word or a terminator line's word on the way scores nothing — the
    // walk's structure gate, without which the def builder had to keep its own
    // search (task #305 measured the walk closing the definition on a delimiter
    // word). A **quoted** brace word stays data for both searches, so no
    // accepted divergence moves with the move.
    let close = walk_until(
        tokens,
        line,
        Some(Closer::Brace),
        List::Plain,
        open + 1,
        &|_, depth, _| depth == 0,
    )
    .ok_or(ParseError::FunctionMissingCloseBrace)?;
    let body_tokens = tokens
        .get(open + 1..close)
        .ok_or(ParseError::FunctionMissingCloseBrace)?;
    // The body is a **block body** (POSIX #2.9's `function_def` nests the
    // group's `compound_list` between the braces): it must be a non-empty
    // list, so bash stops on `f() { }` with rc 2 and fdshell reports
    // [`ParseError::FunctionEmptyBody`] (rc 1, README "Errors"). The emptiness
    // test is [`block_body`]'s, shared with every block arm and the group's
    // own `ensure!`; a body of only the plain `&` keeps its `& ;` glue and so
    // is **not** empty (tasks #354/#355). The check runs after the closer
    // search and before any terminator rule — the group's order
    // (`group_block.rs`), which keeps this message stable.
    //
    // The body's text is [`part_text`]'s, the arm-granularity rule every block
    // arm uses (tasks #238/#273): a here-doc written on a `;`-terminated line of
    // the body claims a region **past** the closing brace, and the appended
    // bytes must travel with the stored body, so the text the call re-runs is a
    // contiguous heredoc (task #305). The multi-line form is extension mode,
    // whose regions are already inside the slice, and stays verbatim.
    let trimmed = block_body(body_tokens, line, ParseError::FunctionEmptyBody)?;
    // The def-site twin of the closing-keyword rule task #301 wrote: the body's
    // `}` needs a command-list terminator in front of it — `;` (the newline's
    // token), the plain `&`, or the closer of a compound command, which is what
    // keeps `f() { if :; then :; fi }` and `f() { { :; } }` definitions. The
    // predicate is the shared [`ensure_before_closer`], the group's own
    // comment-predecessor extension (task #404) staying that arm's half. The
    // order is the group's: empty first, then the terminator.
    ensure_before_closer(
        tokens,
        line,
        close,
        ParseError::ExpectedSemicolonBeforeCloseBrace,
    )?;
    let body = part_text(text, trimmed)?;
    Ok(FunctionDef { name, body })
}

/// The definer word with its trailing POSIX `(` stripped.
fn function_name(tokens: &[Token]) -> Result<ShortCStr, Report<ParseError>> {
    let word = definer(tokens).ok_or(ParseError::FunctionEmptyName)?;
    let bytes = word.as_bytes().ok().ok_or(ParseError::FunctionEmptyName)?;
    let len = name_len(bytes).ok_or(ParseError::FunctionEmptyName)?;
    Ok(word.get(..len).ok_or(ParseError::FunctionEmptyName)?)
}

#[cfg(test)]
mod tests;
