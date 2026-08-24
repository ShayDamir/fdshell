use alloc::vec::Vec;
use core::fmt::Write;
use error_stack::{Report, ResultExt, bail};

use crate::app::AppError;
use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use sys::fork_cell::ForkCell;
use sys::{ImportedStr, Origin, Position, ScriptText, ShortCStr};

pub(crate) use crate::cond::run_cond_list;
pub(crate) use crate::script::run_script;

pub fn handle(text: &ScriptText, cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    if let Some(control) = run_script(text, cell)? {
        match control {
            LoopControl::Break => bail!(CmdError::BreakOutsideLoop),
            LoopControl::Continue => bail!(CmdError::ContinueOutsideLoop),
            LoopControl::Return => bail!(CmdError::ReturnOutsideFunction),
        }
    }
    Ok(())
}

pub fn exec_cmd(text: &ScriptText, cell: &ForkCell<ShellState>) -> Result<i32, Report<CmdError>> {
    if let Some(control) = run_script(text, cell)? {
        match control {
            LoopControl::Break => bail!(CmdError::BreakOutsideLoop),
            LoopControl::Continue => bail!(CmdError::ContinueOutsideLoop),
            LoopControl::Return => bail!(CmdError::ReturnOutsideFunction),
        }
    }
    let state = cell.borrow().change_context(CmdError::Never)?;
    Ok(state.last_status.exit_code())
}

pub fn run(cell: &ForkCell<ShellState>) -> Result<(), Report<AppError>> {
    // Set $0 to "fdshell" for interactive mode
    // Safe to call here because main.rs returns/exits before reaching this path
    // when in -c or script file mode (positional args already set)
    {
        let mut state = cell.borrow_mut().change_context(AppError::Borrow)?;
        state
            .positional
            .push_back(ImportedStr::shell(ShortCStr::from(c"fdshell")));
    }
    let mut buf = Vec::new();
    loop {
        buf.clear();
        sys::OUT
            .write_all(b"fdshell> ")
            .change_context(AppError::Read)?;
        let mut byte = [0u8; 1];
        loop {
            let n = sys::IN.read(&mut byte).change_context(AppError::Read)?;
            if n == 0 {
                return Ok(());
            }
            if byte[0] == b'\n' {
                break;
            }
            buf.push(byte[0]);
        }
        let line = buf.trim_ascii();
        if line.is_empty() {
            continue;
        }
        let text = ScriptText::new(
            ShortCStr::from_vec(line.to_vec()).change_context(AppError::Read)?,
            Position::new(1, 1),
            Origin::Stdin,
        );
        if let Err(err) = handle(&text, cell) {
            let _ = writeln!(crate::io::Stderr, "{err:?}");
        }
    }
}
