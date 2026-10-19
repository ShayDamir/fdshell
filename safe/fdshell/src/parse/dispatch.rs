use super::{
    Token, case_block, for_block, function_block, group_block, if_block, wait_block, while_block,
    word_quoted::keyword_word,
};
use crate::error::parse::ParseError;
use crate::parse::line::ParsedLine;
use error_stack::{Report, ResultExt};
use sys::ScriptText;

/// Dispatch on a `name() { … }` opener or a leading block keyword
/// (`{`/`case`/`if`/`for`/`while`/`until`), returning `None` for an ordinary
/// command. A keyword word written with quote bytes (`'if'`, `""case`) is a
/// command word, never a block opener (POSIX §2.9.1, task #247) — the gate is
/// [`keyword_word`], the same quoted-span rule `keywords::keyword_delta`
/// applies to the block openers and closers it scores.
pub(super) fn dispatch_keyword(
    raw: &[Token],
    text: &ScriptText,
) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let line = text.as_bytes().change_context(ParseError::Never)?;
    if function_block::is_function_def(raw, line) {
        return Ok(Some(ParsedLine::Function(
            function_block::tokens_to_function(raw, text)?,
        )));
    }
    let is = |kw: &[u8]| raw.first().is_some_and(|tok| keyword_word(tok, line, kw));
    if is(b"{") {
        return Ok(Some(ParsedLine::Group(group_block::tokens_to_group(
            raw, text,
        )?)));
    }
    if is(b"case") {
        return Ok(Some(ParsedLine::Case(case_block::tokens_to_case(
            raw, text,
        )?)));
    }
    if is(b"if") {
        return Ok(Some(ParsedLine::If(if_block::tokens_to_if(raw, text)?)));
    }
    if is(b"for") {
        return Ok(Some(ParsedLine::For(for_block::tokens_to_for(raw, text)?)));
    }
    if is(b"wait") && wait_block::is_block(raw, text) {
        return Ok(Some(ParsedLine::Wait(wait_block::tokens_to_wait(
            raw, text,
        )?)));
    }
    if is(b"while") {
        return Ok(Some(ParsedLine::While(while_block::tokens_to_loop(
            raw, b"while", text,
        )?)));
    }
    if is(b"until") {
        return Ok(Some(ParsedLine::Until(while_block::tokens_to_loop(
            raw, b"until", text,
        )?)));
    }
    Ok(None)
}
