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
use error_stack::{Report, ResultExt};
use std::collections::VecDeque;
use std::ffi::CString;
use sys::ShortCStr;
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

    let all_args: Vec<CString> = std::env::args()
        .skip(1)
        .map(|s| CString::new(s).unwrap_or_default())
        .collect();

    if let Some(first) = all_args.first() {
        let first_bytes = first.to_bytes();
        if first_bytes == b"-c" {
            // -c mode: fdshell -c "command" [name arg1 arg2 ...]
            let cmd = all_args.get(1).ok_or(AppError::Usage)?;
            let positional: VecDeque<ShortCStr> = all_args
                .iter()
                .skip(2)
                .map(|a| ShortCStr::from_vec(a.to_bytes().to_vec()).unwrap_or_default())
                .collect();
            {
                let mut state = state.borrow_mut().change_context(AppError::Borrow)?;
                // $0 = first arg after command (or "sh" if missing), $1.. = rest
                if positional.is_empty() {
                    state.positional.push_back(ShortCStr::from(c"sh"));
                } else {
                    state.positional = positional;
                }
            }
            match repl::exec_cmd(cmd.to_bytes(), &state) {
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
        } else if first_bytes.ends_with(b".sh") || first_bytes == b"-" {
            // Script file mode: fdshell script.sh [arg1 arg2 ...]
            let script_path = first;
            let positional: VecDeque<ShortCStr> = all_args
                .iter()
                .skip(1)
                .map(|a| ShortCStr::from_vec(a.to_bytes().to_vec()).unwrap_or_default())
                .collect();
            {
                let mut state = state.borrow_mut().change_context(AppError::Borrow)?;
                // $0 = script path, $1.. = rest of args
                let mut new_pos = VecDeque::new();
                new_pos.push_back(
                    ShortCStr::from_vec(script_path.to_bytes().to_vec()).unwrap_or_default(),
                );
                new_pos.extend(positional);
                state.positional = new_pos;
            }
            let script_path_str = script_path
                .to_str()
                .map_err(|_| {
                    Report::new(AppError::InvalidUtf8 {
                        field: "script path",
                    })
                })
                .change_context(AppError::ScriptRead)?;
            let script_content =
                std::fs::read(script_path_str).change_context(AppError::ScriptRead)?;
            match repl::exec_cmd(&script_content, &state) {
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
    }

    // Interactive mode
    repl::run(&state)
}
