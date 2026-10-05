use crate::error::parse::ParseError;
use crate::parse::BuiltinPrefix;
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
    // `builtin NAME` dispatches as a builtin only; `command NAME` (bash:
    // bypasses user functions) dispatches as a builtin if the name is one,
    // else falls through to the external (PATH) search.
    let (kw, cmd_prefix) = match tokens.get(prefix) {
        Some(t) if t.0.eq_bytes(b"builtin") => (1usize, BuiltinPrefix::Builtin),
        Some(t) if t.0.eq_bytes(b"command") => (1usize, BuiltinPrefix::Command),
        Some(t) if is_builtin(&t.0) => (0usize, BuiltinPrefix::Builtin),
        _ => (0usize, BuiltinPrefix::None),
    };
    let command = tokens
        .get(prefix + kw)
        .ok_or(ParseError::ExpectedCommand)?
        .0
        .clone();
    super::command_args::finish_command(
        cmd_prefix,
        command,
        tokens,
        prefix + kw + 1,
        line,
        specs,
        set_at,
    )
}
