use alloc::vec::Vec;
use core::fmt::Write;
use error_stack::{Report, ResultExt, bail};

use crate::app::AppError;
use crate::error::cmd::CmdError;
use crate::loop_control::LoopControl;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub(crate) use crate::cond::run_cond_list;
pub(crate) use crate::script::run_script;

pub fn handle(line: &[u8], cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    if let Some(control) = run_script(line, cell)? {
        match control {
            LoopControl::Break => bail!(CmdError::BreakOutsideLoop),
            LoopControl::Continue => bail!(CmdError::ContinueOutsideLoop),
        }
    }
    Ok(())
}

pub fn exec_cmd(line: &[u8], cell: &ForkCell<ShellState>) -> Result<i32, Report<CmdError>> {
    if let Some(control) = run_script(line, cell)? {
        match control {
            LoopControl::Break => bail!(CmdError::BreakOutsideLoop),
            LoopControl::Continue => bail!(CmdError::ContinueOutsideLoop),
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
        state.positional.push_back(ShortCStr::from(c"fdshell"));
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
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }
        if let Err(err) = handle(line, cell) {
            let _ = writeln!(crate::io::Stderr, "{err:?}");
        }
    }
}
