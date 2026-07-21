#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::vec::Vec;
use error_stack::{Report, ResultExt};
use fdshell::{AppError, ShellState, init_shellfd, install_debug_hooks, parse_cli_args, run};
use sys::fcntl::O_DIRECTORY;
use sys::fork_cell::ForkCell;

fn main() -> ! {
    install_debug_hooks();
    match run_main() {
        Ok(()) => sys::exit(0),
        Err(report) => {
            let s = sys::format!("{report:?}\n").unwrap_or_else(|_| sys::ShortCStr::new());
            let _ = sys::ERR.write_str(&s);
            sys::exit(1);
        }
    }
}

fn run_main() -> Result<(), Report<AppError>> {
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

    let all_args: Vec<sys::ShortCStr> = sys::cmdline::read_cmdline()
        .change_context(AppError::Init)?
        .into_iter()
        .skip(1)
        .collect();

    if let Some(first) = all_args.first()
        && first.eq_bytes(b"-c")
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
