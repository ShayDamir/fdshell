//! Busybox-style dispatch: when the binary is invoked under a builtin's name,
//! act as that builtin instead of starting a shell.

use crate::child::{self, handle_builtin_error};
use crate::state::ShellState;
use alloc::vec::Vec;
use core::ffi::CStr;
use sys::{ExportedCStr, LocalFd, ShortCStr};

/// Return the basename of `argv0` if it names a shell builtin, else `None`.
pub fn builtin_name(argv0: &ShortCStr) -> Option<ShortCStr> {
    let base = base_name(argv0)?;
    child::dispatch::is_dispatched(&base).then_some(base)
}

fn base_name(path: &ShortCStr) -> Option<ShortCStr> {
    let bytes = path.as_bytes().ok()?;
    let start = bytes.iter().rposition(|&c| c == b'/').map_or(0, |i| i + 1);
    let rest = bytes.get(start..)?;
    ShortCStr::from_vec(rest.to_vec()).ok()
}

/// Run `name` as a builtin in busybox mode; returns the process exit code.
pub fn run(name: ShortCStr, args: &[ShortCStr], sock: Option<LocalFd>) -> i32 {
    let mut state = ShellState::new();
    if let Some(s) = sock {
        state.set_shell_sock(s);
    }
    let sealed: Vec<ExportedCStr> = args.iter().map(|a| a.export()).collect();
    let refs: Vec<&CStr> = sealed.iter().map(|s| s.as_ref()).collect();
    match child::dispatch::dispatch_builtin(name.clone(), &refs, args, &state) {
        Ok(code) => code,
        Err(report) => match handle_builtin_error(name, report) {
            Ok(code) => code,
            Err(e) => e.current_context().exit_code(),
        },
    }
}
