use super::{Token, case_block, for_block, function_block, if_block, wait_block, while_block};
use crate::error::parse::ParseError;
use crate::parse::line::ParsedLine;
use error_stack::Report;
use sys::ScriptText;

/// Dispatch on a `name() { … }` opener or a leading block keyword
/// (`case`/`if`/`for`/`while`/`until`), returning `None` for an ordinary command.
pub(super) fn dispatch_keyword(
    raw: &[Token],
    text: &ScriptText,
) -> Result<Option<ParsedLine>, Report<ParseError>> {
    if function_block::is_function_def(raw) {
        return Ok(Some(ParsedLine::Function(
            function_block::tokens_to_function(raw, text)?,
        )));
    }
    let is = |kw: &[u8]| raw.first().is_some_and(|(t, _, _, _)| t.eq_bytes(kw));
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
    if is(b"wait") {
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
