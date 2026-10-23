use super::Token;
use super::part_text::part_text;
use super::semi::block_body;
use super::word_quoted::{keyword_word, word_quoted};
use crate::error::parse::ParseError;
use crate::keywords::{name_len, quoted_word};
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
    let parens = tokens
        .get(1)
        .is_some_and(|(t, _, _, _, _, _)| t.eq_bytes(b")"));
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

/// The definer word gate: a word with a definer name and no quote byte in
/// its raw span. The span test is [`word_quoted`] (span longer than the
/// token's text) next to [`quoted_word`]: the POSIX definer token carries its
/// trailing `(` (`foo'('`), and `quoted_word` breaks its scan at that `(`,
/// never reaching the quote bytes before it.
fn is_definer(tok: &Token, line: &[u8]) -> bool {
    let (t, start, end, _fq, _mask, _spans) = tok;
    def_len(t).is_some() && !word_quoted(t, *start, *end) && !quoted_word(line, *end)
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
    // The closer is the **last** unquoted `}` word of the token list. A body
    // region's words are blanked by the tokenizer (task #266), so a
    // `}`/`{`/`done` line written **inside a here-doc body** is data and never
    // closes the definition. A **terminator** line's words stay tokens by the
    // same rule, which is why the search keeps this token-list walk: switching
    // it to `semi::closing_keyword_index` (measured, task #305) scores a
    // delimiter line's word and closes on it. The delimiter-word-is-a-brace
    // family is broken on master already and is task **#432**.
    let close = tokens
        .iter()
        .rposition(|tok| keyword_word(tok, line, b"}"))
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
    let body = part_text(
        text,
        block_body(body_tokens, line, ParseError::FunctionEmptyBody)?,
    )?;
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
