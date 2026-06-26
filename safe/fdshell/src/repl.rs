use error_stack::{Report, ResultExt};
use std::io::{BufRead, Write};

use crate::app::AppError;
use crate::error::cmd::CmdError;
use crate::state::ShellState;
use sys::ShortCStr;
use sys::fork_cell::ForkCell;

pub(crate) use crate::cond::run_cond_list;
pub(crate) use crate::script::run_script;

pub fn handle(line: &[u8], cell: &ForkCell<ShellState>) -> Result<(), Report<CmdError>> {
    run_script(line, cell)?;
    Ok(())
}

pub fn exec_cmd(line: &[u8], cell: &ForkCell<ShellState>) -> Result<i32, Report<CmdError>> {
    run_script(line, cell)
}

pub(crate) fn run(cell: &ForkCell<ShellState>) -> Result<(), Report<AppError>> {
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
        print!("fdshell> ");
        std::io::stdout().flush().change_context(AppError::Read)?;
        let n = std::io::stdin()
            .lock()
            .read_until(b'\n', &mut buf)
            .change_context(AppError::Read)?;
        if n == 0 {
            println!();
            break;
        }
        let line = buf.trim_ascii();
        if line.is_empty() || line.starts_with(b"#") {
            continue;
        }
        if let Err(err) = handle(line, cell) {
            eprintln!("{err:?}");
        }
    }
    Ok(())
}
