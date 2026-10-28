//! A statement whose word 0 is a redirection: POSIX #2.9 makes a list of
//! redirections with **no command word** a command of its own — the
//! redirections apply, the shell's fds come back afterwards, the status is 0
//! and no fork happens. The parse side builds the `CommandLine` with an
//! **empty** command word; `run::cmd` is the arm that runs it in-process. The
//! family includes the here-doc operators (`<<EOF`, `3<<EOF`, `<<EOF g`), whose
//! body feeds the empty command's standard input (task #345).

use crate::error::parse::ParseError;
use crate::parse::bg_redirect::parse_bg_redirect;
use crate::parse::classify::parse_redirect;
use crate::parse::here_string::parse_here_string;
use crate::parse::heredoc::{HeredocBody, is_operator};
use crate::parse::{BuiltinPrefix, CommandLine, Token, envassign};
use alloc::vec::Vec;
use error_stack::Report;
use sys::{Position, ShortCStr};

/// Whether the word at `p` is a redirection, so the statement is the
/// redirections-only family. The three predicates are the very ones the
/// collection loop uses, in the loop's own order: the gate recognizes exactly
/// what the loop consumes, adds no operator vocabulary, and inherits every
/// operator-quoting gate — `'>f'`, `"&>f"`, `"2>f"` and `2 > f` keep their
/// word-0 roles, and a bare operator whose operand is missing bails
/// `InvalidRedirect` exactly as the loop does.
///
/// A statement **carrying a here-doc body** opens the gate only when word `p`
/// is itself a here-doc operator (`is_operator`, the loop's own test):
/// `<<EOF`, `3<<EOF`, `<<EOF g` are `simple_command = REDIRECT… [word]` with
/// the body on the empty command's stdin (task #345). The leading-operator
/// rows survive on this test, not on the spec count: `> <<EOF`, `>| x <<EOF`,
/// `>x <<EOF` have a non-here-doc word 0, keep the command word their pins
/// record, and reach the run side exactly as before.
pub(super) fn leads(
    tokens: &[Token],
    p: usize,
    line: &[u8],
    specs: &[HeredocBody],
) -> Result<bool, Report<ParseError>> {
    if !specs.is_empty() {
        return Ok(is_operator(line, tokens, p));
    }
    Ok(parse_bg_redirect(tokens, p)?.is_some()
        || parse_here_string(line, tokens, p)?.is_some()
        || parse_redirect(tokens, p)?.is_some())
}

/// Build the redirections-only command: an **empty** command word (the run
/// side's signal), the leading `NAME=value` words as the scoped prefix, and the
/// tail collected from `p` with the command slot open — the first plain word
/// becomes the command (`>f g` runs `g` with the redirect, as bash does).
pub(super) fn build(
    tokens: &[Token],
    p: usize,
    line: &[u8],
    specs: &[HeredocBody],
    set_at: Position,
) -> Result<CommandLine, Report<ParseError>> {
    let mut cmd = CommandLine {
        prefix: BuiltinPrefix::None,
        command: ShortCStr::new(),
        command_mask: Vec::new(),
        command_span: Vec::new(),
        command_quoted: false,
        env_assigns: envassign::collect_range(tokens, p),
        args: Vec::new(),
        args_mask: Vec::new(),
        args_span: Vec::new(),
        args_quoted: Vec::new(),
        captures: Vec::new(),
        redirects: Vec::new(),
        pidvar: None,
        bg_force: false,
    };
    super::collect_tail::collect(&mut cmd, tokens, p, line, specs, set_at, true)?;
    Ok(cmd)
}

#[cfg(test)]
mod tests;
