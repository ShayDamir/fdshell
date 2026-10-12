//! The interactive read-complete-execute loop: prompt, continuation buffering
//! and dispatch, one logical input at a time.

use alloc::vec::Vec;
use core::fmt::Write;
use error_stack::{Report, ResultExt};

use crate::app::AppError;
use crate::cmd_subst::MAX_CAPTURED;
use crate::options::IGNOREEOF;
use crate::repl::{complete, handle, line::{self, LineReader}};
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, Origin, Position, ScriptText, ShortCStr};

/// The primary and the continuation prompt (bash's `PS1`/`PS2`; epic #197
/// owns the prompt variables, so the literals stay here).
const PRIMARY_PROMPT: &[u8] = b"fdshell> ";
const CONTINUATION_PROMPT: &[u8] = b"> ";

/// Prompt, read and execute inputs until the end of input.
pub(super) fn run(cell: &ForkCell<ShellState>) -> Result<(), Report<AppError>> {
    seed_state(cell)?;
    let mut input = LineReader::new();
    loop {
        let Some(text) = read_input(cell, &mut input)? else {
            return Ok(());
        };
        if let Err(err) = handle(&text, cell) {
            let _ = writeln!(crate::io::Stderr, "{err:?}");
        }
    }
}

/// Seed `$0` with the invocation name for interactive mode. Only this path
/// needs it: `-c` and script-file modes seed it before dispatching.
fn seed_state(cell: &ForkCell<ShellState>) -> Result<(), Report<AppError>> {
    let program = ImportedStr::shell(ShortCStr::from(c"fdshell"));
    cell.borrow_mut()
        .change_context(AppError::Borrow)?
        .set_program_name(program);
    // Like bash, `ignoreeof` is on only when stdin is a terminal, so a
    // stray Ctrl+D does not kill an interactive shell; piped input
    // still exits on EOF. The seed borrow above is already dropped: `is_tty`
    // is a syscall, and a held `RefMut` poisons every nested borrow.
    if sys::IN.is_tty().change_context(AppError::Read)? {
        cell.borrow_mut()
            .change_context(AppError::Borrow)?
            .options |= IGNOREEOF;
    }
    Ok(())
}

/// One logical input: the primary line plus its continuation lines, trimmed.
/// `None` is end of input and nothing else — a blank line is skipped and the
/// next line is prompted for, so an empty line never ends the shell.
fn read_input(
    cell: &ForkCell<ShellState>,
    input: &mut LineReader,
) -> Result<Option<ScriptText>, Report<AppError>> {
    loop {
        let mut buf = Vec::new();
        if !line::read_line(cell, &mut buf, PRIMARY_PROMPT, MAX_CAPTURED, input)? {
            return Ok(None);
        }
        read_continuation(cell, &mut buf, input)?;
        let trimmed = buf.trim_ascii();
        if !trimmed.is_empty() {
            return Ok(Some(ScriptText::new(
                ShortCStr::from_vec(trimmed.to_vec()).change_context(AppError::Read)?,
                Position::new(1, 1),
                Origin::Stdin,
            )));
        }
    }
}

/// Buffer incomplete input (open blocks, unterminated heredocs, trailing
/// operators, unbalanced quotes) until it forms a complete construct. On EOF
/// with `ignoreeof` off, stop and execute what we have (bash: incomplete
/// input at EOF runs and reports the parse error); with `ignoreeof` on,
/// `read_line` keeps the shell alive.
fn read_continuation(
    cell: &ForkCell<ShellState>,
    buf: &mut Vec<u8>,
    input: &mut LineReader,
) -> Result<(), Report<AppError>> {
    while !complete::is_complete(buf) {
        buf.push(b'\n');
        if !line::read_line(cell, buf, CONTINUATION_PROMPT, MAX_CAPTURED, input)? {
            break;
        }
    }
    Ok(())
}
