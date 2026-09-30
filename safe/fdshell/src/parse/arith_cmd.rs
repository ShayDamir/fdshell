//! The `((expr))` arithmetic command: a keyword recognized on the raw
//! statement bytes, before tokenizing (the tokenizer splits `)` and `|`).

use crate::error::parse::{ParseError, ParsePosition};
use crate::parse::line::ParsedLine;
use error_stack::Report;
use sys::ScriptText;

/// `Some(ArithCommand)` when `line` is a `((expr))` statement; a parse error
/// when it starts with `((` but is not closed by `))`; `None` otherwise.
pub(crate) fn detect(
    text: &ScriptText,
    line: &[u8],
) -> Result<Option<ParsedLine>, Report<ParseError>> {
    let trimmed = line.trim_ascii();
    if !trimmed.starts_with(b"((") {
        return Ok(None);
    }
    let Some(body) = trimmed
        .strip_prefix(b"((")
        .and_then(|t| t.strip_suffix(b"))"))
    else {
        // `.attach_opaque` needs the explicit `Report::new` form (STYLE 4.14).
        return Err(
            Report::new(ParseError::MalformedArithCommand).attach_opaque(ParsePosition {
                pos: 0,
                input: Some(line.to_vec()),
            }),
        );
    };
    let lead = line
        .iter()
        .take_while(|&&b| b.is_ascii_whitespace())
        .count();
    let body_text = text
        .subslice(lead + 2, body.len())
        .ok_or(ParseError::Never)?;
    Ok(Some(ParsedLine::ArithCommand(body_text)))
}
