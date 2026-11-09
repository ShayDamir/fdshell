// mutants: -C --lib -C --test -C function_keyword -- -E 'test(/^parse::function_block::/) | binary(function_keyword)'

//! The definer prefix the parse dispatch rejects (task #433): POSIX #2.9 puts a
//! `function_def`'s `{` right behind the definer words, so a **redirection** word
//! written between them is a syntax error in bash (rc 2) and a report here (rc 1)
//! — no definition is made and nothing else runs. The shape is the accepted
//! definer's (`parse::function_block`), and the byte-level rule
//! (`keywords::function`) keeps its span claim untouched: the claim is what puts
//! the whole definition in front of the parser, so the report rides this layer
//! (LESSONS: "Byte-level definer words must be the tokenizer's tokens").
//!
//! The word predicate is the same owners the collection loop uses, in the loop's
//! own order (the principle `parse/command/redirect_first.rs::leads` states) — no
//! new operator vocabulary, and every owner's own quote gate still keeps a quoted
//! `">"` word as word data. A **plain** word in the prefix is the documented
//! leniency (`function f x { …; }`, task #247), so it reports nothing here.

use crate::error::parse::ParseError;
use crate::parse::Token;
use crate::parse::bg_redirect::parse_bg_redirect;
use crate::parse::classify::parse_redirect;
use crate::parse::here_string::parse_here_string;
use crate::parse::heredoc::is_operator;
use crate::parse::word_quoted::keyword_word;
use error_stack::Report;
use sys::ShortCStr;

/// The first word of the prefix — the walk between the definer pair (tokens 0-1
/// in both spellings) and the unquoted `{` token — that is a redirection. The
/// `{` search is [`keyword_word`]'s, the one `is_function_def` uses, so the
/// window never runs past a brace the definer does not see. `None` = nothing to
/// report; the owners' own bails propagate.
pub(crate) fn redirect_before_the_brace<'a>(
    tokens: &'a [Token],
    line: &[u8],
) -> Result<Option<&'a ShortCStr>, Report<ParseError>> {
    let Some(open) = tokens.iter().position(|tok| keyword_word(tok, line, b"{")) else {
        return Ok(None);
    };
    for i in 2..open {
        if let Some(word) = is_redirect(tokens, i, line)? {
            return Ok(Some(word));
        }
    }
    Ok(None)
}

/// The word at `i`, when one of the four owners claims it as a redirection. The
/// `(`/`)` tokens of the split-paren definers live in the same walk
/// (`function f ( ) { … }`, `f( ) { … }`) and no owner claims them.
fn is_redirect<'a>(
    tokens: &'a [Token],
    i: usize,
    line: &[u8],
) -> Result<Option<&'a ShortCStr>, Report<ParseError>> {
    let word = &tokens.get(i).ok_or(ParseError::Never)?.word;
    let claims = parse_bg_redirect(tokens, i)?.is_some()
        || parse_here_string(line, tokens, i)?.is_some()
        || parse_redirect(tokens, i)?.is_some()
        || is_operator(line, tokens, i);
    Ok(claims.then_some(word))
}
