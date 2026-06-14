#![forbid(unsafe_code)]

mod app;
mod capture;
mod cd;
mod child;
mod cmd_subst;
mod cond;
mod error;
mod exec;
mod expand;
mod exports;
mod if_exec;
mod init;
mod intercept;
mod launch;
mod loop_;
mod parse;
mod pipeline;
mod postlaunch;
mod redirect;
mod repl;
mod replacer;
mod resolve;
mod run;
mod script;
mod state;
mod substitute;
mod task;

#[cfg(test)]
mod tests;

use crate::error::LegacyError;
use app::AppError;
use error_stack::{Report, ResultExt, bail};
use std::io::{BufRead, Write};
use sys::fcntl::O_DIRECTORY;

fn main() -> Result<(), Report<AppError>> {
    let _mode = crate::init::init_shellfd()
        .map_err(LegacyError)
        .change_context(AppError::Init)?;
    sys::umask::init();
    let state = sys::fork_cell::ForkCell::new(state::ShellState::new());
    {
        let mut state = state
            .borrow_mut()
            .map_err(LegacyError)
            .change_context(AppError::Borrow)?;
        let cwd = sys::openat2::open(c".", O_DIRECTORY)
            .map_err(LegacyError)
            .change_context(AppError::Cwd)?;
        state.fds.insert(c"CWD".into(), cwd);
    }
    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next() {
        if arg == "-c" {
            let cmd = args.next().ok_or(AppError::Usage)?;
            let code = repl::exec_cmd(cmd.as_bytes(), &state)
                .map_err(LegacyError)
                .change_context(AppError::Exec)?;
            if code != 0 {
                std::process::exit(code);
            }
            return Ok(());
        }
        bail!(AppError::Usage);
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
        if let Err(e) = repl::handle(line, &state) {
            eprintln!("fdshell: command execution failed (syscall error: {e})");
        }
    }
    Ok(())
}
