use crate::error::parse::ParseError;
use crate::parse::CommandLine;
use crate::parse::Token;
use crate::parse::builtin::is_builtin;
use crate::parse::heredoc::HeredocBody;
use error_stack::Report;
use sys::Position;

pub fn parse_command(
    tokens: &[Token],
    line: &[u8],
    specs: &[HeredocBody],
    set_at: Position,
) -> Result<CommandLine, Report<ParseError>> {
    let mut prefix = 0usize;
    // `command` (bash) is an alias for the `builtin` prefix: it bypasses
    // user-function lookup.
    let builtin_kw = tokens
        .first()
        .is_some_and(|(t, _, _, _, _)| t.eq_bytes(b"builtin") || t.eq_bytes(b"command"));
    if builtin_kw {
        prefix = 1;
    }
    let builtin = if builtin_kw {
        true
    } else {
        tokens
            .get(prefix)
            .is_some_and(|(t, _, _, _, _)| is_builtin(t))
    };
    let command = tokens
        .get(prefix)
        .ok_or(ParseError::ExpectedCommand)?
        .0
        .clone();
    super::command_args::finish_command(builtin, command, tokens, prefix + 1, line, specs, set_at)
}
