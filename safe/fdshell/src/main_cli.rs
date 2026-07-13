use crate::{AppError, ShellState, exec_cmd};
use error_stack::{Report, ResultExt};
use std::collections::VecDeque;
use std::ffi::CString;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub fn run_cmd_mode(
    all_args: &[CString],
    state: &ForkCell<ShellState>,
) -> Result<(), Report<AppError>> {
    let cmd = all_args.get(1).ok_or(AppError::Usage)?;
    let positional: VecDeque<ShortCStr> = all_args
        .iter()
        .skip(2)
        .map(|a| ShortCStr::from_vec(a.to_bytes().to_vec()).unwrap_or_default())
        .collect();
    {
        let mut state = state.borrow_mut().change_context(AppError::Borrow)?;
        if positional.is_empty() {
            state.set_positional(VecDeque::from([ShortCStr::from(c"sh")]));
        } else {
            state.set_positional(positional);
        }
    }
    match exec_cmd(cmd.to_bytes(), state) {
        Ok(code) => {
            if code != 0 {
                std::process::exit(code);
            }
            Ok(())
        }
        Err(info) => {
            eprintln!("{info:?}");
            std::process::exit(1);
        }
    }
}

pub fn execute_script(
    script_content: &[u8],
    state: &ForkCell<ShellState>,
) -> Result<(), Report<AppError>> {
    match exec_cmd(script_content, state) {
        Ok(code) => {
            if code != 0 {
                std::process::exit(code);
            }
            Ok(())
        }
        Err(info) => {
            eprintln!("{info:?}");
            std::process::exit(1);
        }
    }
}
