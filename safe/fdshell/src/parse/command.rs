use crate::error::parse::ParseError;
use crate::parse::CommandLine;
use crate::parse::builtin::is_builtin;
use alloc::vec::Vec;
use error_stack::Report;
use sys::Position;
use sys::ShortCStr;

pub fn parse_command(
    tokens: &[ShortCStr],
    fully_quoted: Vec<bool>,
    quote_masks: Vec<Vec<bool>>,
    set_at: Position,
) -> Result<CommandLine, Report<ParseError>> {
    let mut iter = tokens.iter().peekable();
    let mut builtin_kw = false;
    let builtin = match iter.peek() {
        // `command` (bash) is an alias for the `builtin` prefix: it bypasses
        // user-function lookup.
        Some(t) if t.eq_bytes(b"builtin") || t.eq_bytes(b"command") => {
            iter.next();
            builtin_kw = true;
            true
        }
        Some(t) => is_builtin(t),
        None => false,
    };
    let command = iter.next().ok_or(ParseError::ExpectedCommand)?.clone();
    let mut fq_iter = fully_quoted.into_iter();
    fq_iter.next();
    let mut mask_iter = quote_masks.into_iter();
    mask_iter.next();
    if builtin_kw {
        fq_iter.next();
        mask_iter.next();
    }
    super::command_args::finish_command(
        builtin,
        command,
        &mut iter,
        &mut fq_iter,
        &mut mask_iter,
        set_at,
    )
}
