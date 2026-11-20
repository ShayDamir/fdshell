// mutants: -C --lib -C --test -C repl -C --test -C trap -- -E 'binary(repl) | binary(trap)'

//! The interactive read-complete-execute loop: prompt, continuation buffering
//! and dispatch, one logical input at a time.

use alloc::vec::Vec;
use core::fmt::Write;
use error_stack::{Report, ResultExt};

use crate::app::AppError;
use crate::cmd_subst::MAX_CAPTURED;
use crate::options::IGNOREEOF;
use crate::repl::line::{self, LineReader};
use crate::repl::{complete, handle, prompt};
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, Origin, Position, ScriptText, ShortCStr};

/// The prompt variables' values are read per read through `repl::prompt`,
/// decoded and rendered; these are the fallbacks when `PS1`/`PS2` are
/// **unset** (measured against bash 5.3.9, 2026-11-18, uid-independent). The
/// `PS1` default is bash's raw escape string and renders byte-identically:
/// its escapes are value escapes, and the decoder's no-values provider
/// renders those literally until #617 wires real values.
const PRIMARY_PROMPT: &[u8] = b"\\s-\\v\\$ ";
const CONTINUATION_PROMPT: &[u8] = b"> ";

/// Prompt, read and execute inputs until the end of input.
pub fn run(cell: &ForkCell<ShellState>) -> Result<(), Report<AppError>> {
    seed_state(cell)?;
    let mut input = LineReader::new();
    loop {
        let Some(text) = read_input(cell, &mut input)? else {
            // End of input ends the shell: the EXIT trap runs first, at most
            // once per shell, and its report is written here (the `Ok(())`
            // contract, the plan's decision for task #585).
            if let Err(report) = crate::trap_exec::run_exit_trap(cell) {
                let _ = writeln!(crate::io::Stderr, "{report:?}");
            }
            return Ok(());
        };
        // `PS0`'s display (task #622) goes here, **before** the counter moves:
        // bash's own comment in `reader_loop` puts it before the increment, and the
        // measured `\#` numbers agree — it shows the number of the command it
        // accompanies, and the provider answers `command_number + 1`.
        prompt::ps0::display(cell);
        // The prompt for this input has already been drawn, and it showed
        // `command_number + 1` (bash's rule, measured): the input about to run is
        // the command the counter counts. Blank inputs never get here — bash
        // counts them in neither `\!` nor `\#`.
        cell.borrow_mut()
            .change_context(AppError::Borrow)?
            .command_number += 1;
        if let Err(err) = handle(&text, cell) {
            let _ = writeln!(crate::io::Stderr, "{err:?}");
        }
    }
}

/// Seed `$0` with the invocation name and the interactive flag for the REPL.
/// Only this path needs them: `-c` and script-file modes seed `$0` before
/// dispatching and never enter the read loop.
fn seed_state(cell: &ForkCell<ShellState>) -> Result<(), Report<AppError>> {
    let program = ImportedStr::shell(ShortCStr::from(c"fdshell"));
    // The `is_tty` syscall precedes the borrow: it is a syscall, and a held
    // `RefMut` poisons every nested borrow.
    let interactive = sys::IN.is_tty().change_context(AppError::Read)?;
    let mut state = cell.borrow_mut().change_context(AppError::Borrow)?;
    state.set_program_name(program);
    state.interactive = interactive;
    // Like bash, `ignoreeof` is on only when stdin is a terminal, so a stray
    // Ctrl+D does not kill an interactive shell. The flag is cached because
    // the EOF policy is the conjunction of the option bit and interactivity:
    // a pipe's EOF is sticky, so ignoring it there livelocks (task #291).
    if interactive {
        state.options |= IGNOREEOF;
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
        let bytes = prompt::resolve(cell, &ShortCStr::from(c"PS1"), PRIMARY_PROMPT)?;
        if !line::read_line(cell, &mut buf, &bytes, MAX_CAPTURED, input)? {
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
/// input at EOF runs and reports the parse error); with `ignoreeof` on, a
/// terminal keeps the shell alive and a pipe ends the input (task #291).
fn read_continuation(
    cell: &ForkCell<ShellState>,
    buf: &mut Vec<u8>,
    input: &mut LineReader,
) -> Result<(), Report<AppError>> {
    while !complete::is_complete(buf) {
        buf.push(b'\n');
        let bytes = prompt::resolve(cell, &ShortCStr::from(c"PS2"), CONTINUATION_PROMPT)?;
        if !line::read_line(cell, buf, &bytes, MAX_CAPTURED, input)? {
            break;
        }
    }
    Ok(())
}
