use crate::error::parse::ParseError;
use crate::parse::BuiltinPrefix;
use crate::parse::CommandLine;
use crate::parse::Token;
use alloc::vec::Vec;
use error_stack::Report;
use sys::{Position, ShortCStr};

/// Collect the tokens after the command word into args, captures and
/// redirects, returning the finished `CommandLine`. Tokens after the first
/// `;` are the heredoc body region: they belong to the body, not the command.
/// The tail loop itself is the shared one (`command::collect_tail`), run here
/// with the command slot **closed**: word 0 is already decided.
pub(super) fn finish_command(
    prefix: BuiltinPrefix,
    command: ShortCStr,
    tokens: &[Token],
    args_from: usize,
    line: &[u8],
    specs: &[crate::parse::heredoc::HeredocBody],
    set_at: Position,
) -> Result<CommandLine, Report<ParseError>> {
    // The command token precedes the first arg; its mask describes word 0,
    // and the words before it are the command's scoped `NAME=value` prefix.
    let word0 = tokens.get(args_from - 1);
    let command_mask = word0.map(|t| t.4.clone()).unwrap_or_default();
    let command_span = word0.map(|t| t.5.clone()).unwrap_or_default();
    let command_quoted = word0.is_some_and(|t| super::word_quoted::word_quoted(&t.0, t.1, t.2));
    // Word 0 is a substitution surface (task #268): its escape pairs are folded
    // by the substitution pass, and its fields are split and globbed, so the
    // raw text, mask and quoted flag travel to `command_word::expand` — the
    // one point that runs before function, intercept and `PATH` dispatch.
    let env_assigns = super::envassign::collect_range(tokens, args_from - 1);
    let mut cmd = CommandLine {
        prefix,
        command,
        command_mask,
        command_span,
        command_quoted,
        env_assigns,
        args: Vec::new(),
        args_mask: Vec::new(),
        args_span: Vec::new(),
        args_quoted: Vec::new(),
        captures: Vec::new(),
        redirects: Vec::new(),
        pidvar: None,
        bg_force: false,
    };
    super::command::collect_tail::collect(&mut cmd, tokens, args_from, line, specs, set_at, false)?;
    Ok(cmd)
}
