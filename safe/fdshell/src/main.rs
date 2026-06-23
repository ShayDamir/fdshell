#![forbid(unsafe_code)]

mod app;
mod capture;
mod caret;
mod cd;
mod child;
mod cmd_subst;
mod cond;
mod debug;
mod error;
mod exec;
mod expand;
mod exports;
mod for_run;
mod if_exec;
mod init;
mod intercept;
mod keywords;
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

use app::AppError;
use error_stack::{Report, ResultExt, bail};
use sys::fcntl::O_DIRECTORY;

fn main() -> Result<(), Report<AppError>> {
    debug::install_debug_hooks();

    let _mode = crate::init::init_shellfd().change_context(AppError::Init)?;
    sys::umask::init();
    let state = sys::fork_cell::ForkCell::new(state::ShellState::new());
    {
        let mut state = state.borrow_mut().change_context(AppError::Borrow)?;
        let cwd = match sys::openat2::open(c".", O_DIRECTORY) {
            Ok(fd) => fd,
            Err(_) => return Err(Report::new(AppError::Cwd)),
        };
        state.fds.insert(c"CWD".into(), cwd);
    }
    let mut args = std::env::args().skip(1);
    if let Some(arg) = args.next() {
        if arg == "-c" {
            let cmd = args.next().ok_or(AppError::Usage)?;
            match repl::exec_cmd(cmd.as_bytes(), &state) {
                Ok(code) => {
                    if code != 0 {
                        std::process::exit(code);
                    }
                    return Ok(());
                }
                Err(info) => {
                    eprintln!("{info:?}");
                    std::process::exit(1);
                }
            }
        }
        bail!(AppError::Usage);
    }
    repl::run(&state)
}
