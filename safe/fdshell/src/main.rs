#![forbid(unsafe_code)]

use error_stack::{Report, ResultExt};
use fdshell::{AppError, ShellState, init_shellfd, install_debug_hooks, parse_cli_args, run};
use std::ffi::CString;
use sys::fcntl::O_DIRECTORY;
use sys::fork_cell::ForkCell;

fn main() -> Result<(), Report<AppError>> {
    install_debug_hooks();

    let mode = init_shellfd().change_context(AppError::Init)?;
    sys::umask::init();
    let mut state_inner = ShellState::new();
    match mode {
        fdshell::FdShellMode::Nested(fd) => {
            state_inner.set_shell_sock(fd);
        }
        fdshell::FdShellMode::Standalone => {}
    }
    let state = ForkCell::new(state_inner);
    {
        let mut state = state.borrow_mut().change_context(AppError::Borrow)?;
        let cwd = sys::openat2::open(c".", O_DIRECTORY).change_context(AppError::Cwd)?;
        state.insert_cwd(cwd);
    }

    let all_args: Vec<CString> = std::env::args()
        .skip(1)
        .map(|s| CString::new(s).unwrap_or_default())
        .collect();

    if let Some(first) = all_args.first()
        && first.to_bytes() == b"-c"
    {
        return fdshell::main_cli::run_cmd_mode(&all_args, &state);
    }

    let parsed = parse_cli_args(&all_args)?;
    if parsed.dirfd.is_some() && parsed.positional.is_empty() {
        return Err(AppError::MissingScriptPath.into());
    }

    match fdshell::script_loader::load_script_source(&parsed) {
        Ok(Some((script_content, positional))) => {
            {
                let mut state = state.borrow_mut().change_context(AppError::Borrow)?;
                state.set_positional(positional);
            }
            fdshell::main_cli::execute_script(&script_content, &state)
        }
        Ok(None) => run(&state),
        Err(e) => Err(e),
    }
}
