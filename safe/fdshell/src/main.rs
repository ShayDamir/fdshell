#![forbid(unsafe_code)]

use error_stack::{Report, ResultExt};
use fdshell::{
    AppError, ShellState, exec_cmd, init_shellfd, install_debug_hooks, load_script, parse_cli_args,
    run,
};
use std::collections::VecDeque;
use std::ffi::CString;
use sys::ShortCStr;
use sys::fcntl::{O_CLOEXEC, O_DIRECTORY, O_RDONLY};
use sys::fork_cell::ForkCell;

fn main() -> Result<(), Report<AppError>> {
    install_debug_hooks();

    let _mode = init_shellfd().change_context(AppError::Init)?;
    sys::umask::init();
    let state = ForkCell::new(ShellState::new());
    {
        let mut state = state.borrow_mut().change_context(AppError::Borrow)?;
        let cwd = sys::openat2::open(c".", O_DIRECTORY).change_context(AppError::Cwd)?;
        state.fds.insert(c"CWD".into(), cwd);
    }

    let all_args: Vec<CString> = std::env::args()
        .skip(1)
        .map(|s| CString::new(s).unwrap_or_default())
        .collect();

    // -c mode takes highest priority
    if let Some(first) = all_args.first()
        && first.to_bytes() == b"-c"
    {
        let cmd = all_args.get(1).ok_or(AppError::Usage)?;
        let positional: VecDeque<ShortCStr> = all_args
            .iter()
            .skip(2)
            .map(|a| ShortCStr::from_vec(a.to_bytes().to_vec()).unwrap_or_default())
            .collect();
        {
            let mut state = state.borrow_mut().change_context(AppError::Borrow)?;
            if positional.is_empty() {
                state.positional.push_back(ShortCStr::from(c"sh"));
            } else {
                state.positional = positional;
            }
        }
        match exec_cmd(cmd.to_bytes(), &state) {
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

    // Parse --dirfd and --fd, collect positional args
    let parsed = parse_cli_args(&all_args)?;

    // --dirfd requires a positional path
    if parsed.dirfd.is_some() && parsed.positional.is_empty() {
        return Err(AppError::MissingScriptPath.into());
    }

    // Determine script source and positional args
    let (script_content, positional) = if let Some(fd) = parsed.script_fd {
        // --fd takes precedence: read from fd, positional args become $0..
        let pos: VecDeque<ShortCStr> = parsed
            .positional
            .iter()
            .map(|a| ShortCStr::from_vec(a.to_bytes().to_vec()).unwrap_or_default())
            .collect();
        (load_script(&fd).change_context(AppError::ScriptRead)?, pos)
    } else if let Some(path) = parsed.positional.first() {
        // Path mode: open from path, optionally relative to dirfd
        let fd = if let Some(dirfd) = &parsed.dirfd {
            sys::openat2::openat2(
                dirfd.at(),
                path.as_c_str(),
                &sys::openat2::OpenHow::new((O_RDONLY | O_CLOEXEC) as u64, 0),
            )
            .change_context(AppError::ScriptRead)?
        } else {
            sys::openat2::open(path.as_c_str(), O_RDONLY).change_context(AppError::ScriptRead)?
        };
        let mut pos = VecDeque::new();
        pos.push_back(ShortCStr::from_vec(path.to_bytes().to_vec()).unwrap_or_default());
        pos.extend(
            parsed
                .positional
                .iter()
                .skip(1)
                .map(|a| ShortCStr::from_vec(a.to_bytes().to_vec()).unwrap_or_default()),
        );
        (load_script(&fd).change_context(AppError::ScriptRead)?, pos)
    } else {
        return run(&state);
    };

    // Execute
    {
        let mut state = state.borrow_mut().change_context(AppError::Borrow)?;
        state.positional = positional;
    }
    match exec_cmd(&script_content, &state) {
        Ok(code) => {
            if code != 0 {
                std::process::exit(code);
            }
        }
        Err(info) => {
            eprintln!("{info:?}");
            std::process::exit(1);
        }
    }
    Ok(())
}
