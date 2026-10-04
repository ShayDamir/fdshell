use crate::error::parse::ParseError;
use crate::parse::CommandLine;
use crate::parse::Token;
use crate::parse::builtin::is_builtin;
use crate::parse::envassign;
use crate::parse::heredoc::HeredocBody;
use error_stack::{Report, ensure};
use sys::Position;

pub fn parse_command(
    tokens: &[Token],
    line: &[u8],
    specs: &[HeredocBody],
    set_at: Position,
) -> Result<CommandLine, Report<ParseError>> {
    ensure!(
        envassign::has_command_word(tokens),
        ParseError::ExpectedCommand
    );
    // Leading `NAME=value` words are scoped to this command (POSIX 2.9.1);
    // the keyword and builtin checks run on the first word after the prefix.
    let prefix = envassign::prefix_len(tokens);
    // `command` (bash) is an alias for the `builtin` prefix: it bypasses
    // user-function lookup.
    let builtin_kw = tokens
        .get(prefix)
        .is_some_and(|(t, _, _, _, _)| t.eq_bytes(b"builtin") || t.eq_bytes(b"command"));
    let kw = if builtin_kw { 1 } else { 0 };
    let builtin = if builtin_kw {
        true
    } else {
        tokens
            .get(prefix)
            .is_some_and(|(t, _, _, _, _)| is_builtin(t))
    };
    let command = tokens
        .get(prefix + kw)
        .ok_or(ParseError::ExpectedCommand)?
        .0
        .clone();
    super::command_args::finish_command(
        builtin,
        command,
        tokens,
        prefix + kw + 1,
        line,
        specs,
        set_at,
    )
}
