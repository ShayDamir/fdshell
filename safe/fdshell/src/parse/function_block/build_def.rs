//! The builder half of a function definition: the definer has already been
//! recognised by [`super`] (`is_function_def`), and this half walks the body
//! between the braces and hands back the [`FunctionDef`] the dispatch stores.
//! The recognition half stays there; the shared walk/gate owners the builder
//! calls are the same ones every other block arm uses.

// mutants: -C --lib -C --test -C function_keyword -- -E 'test(/^parse::function_block::/) | binary(function_keyword)'

use super::{FunctionDef, definer};
use crate::error::parse::ParseError;
use crate::keywords::{Closer, name_len};
use crate::parse::Token;
use crate::parse::part_text::part_text;
use crate::parse::semi::{List, block_body, ensure_before_closer, walk_until};
use crate::parse::word_quoted::keyword_word;
use error_stack::{Report, ResultExt};
use sys::{ScriptText, ShortCStr};

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
