use error_stack::{Report, ResultExt};

use crate::error::cd::CdError;
use crate::state::ShellState;
use alloc::ffi::CString;
use sys::fcntl::{O_DIRECTORY, O_NOFOLLOW};
use sys::{LocalFd, ShortCStr};

pub fn cd(args: &[ShortCStr], state: &mut ShellState) -> Result<(), Report<CdError>> {
    let new_fd = match args.first() {
        None => cd_home()?,
        Some(arg) if arg.eq_bytes(b"-") => cd_var(&c"%OLDCWD".into(), state)?,
        Some(arg) if arg.starts_with(b"%") => cd_var(arg, state)?,
        Some(path) => cd_path(path)?,
    };
    sys::fchdir::fchdir(&new_fd).change_context(CdError::CdPathOpen)?;
    move_cwd(state, new_fd);
    Ok(())
}

fn cd_home() -> Result<LocalFd, Report<CdError>> {
    let home = sys::env::getenv(b"HOME").ok_or(CdError::HomeNotSet)?;
    let cs = CString::new(home).change_context(CdError::CdPathOpen)?;
    open_cwd_dir(&cs)
}

fn cd_var(arg: &ShortCStr, state: &ShellState) -> Result<LocalFd, Report<CdError>> {
    let name = arg.strip_prefix(b"%").ok_or(CdError::Never)?;
    let src = state.fds.get(&name).ok_or(CdError::FdNotSet)?;
    src.try_clone().change_context(CdError::CdPathOpen)
}

fn cd_path(path: &ShortCStr) -> Result<LocalFd, Report<CdError>> {
    let name = sys::RefCStr::from(path.clone());
    open_cwd_dir(&name)
}

fn open_cwd_dir(path: &core::ffi::CStr) -> Result<LocalFd, Report<CdError>> {
    sys::openat2::open(path, O_DIRECTORY | O_NOFOLLOW).change_context(CdError::CdPathOpen)
}

fn move_cwd(state: &mut ShellState, new_cwd: LocalFd) {
    let cwd_key: sys::ShortCStr = c"CWD".into();
    let old_cwd = state.fds.remove(&cwd_key);
    if let Some(old) = old_cwd {
        state.fds.insert(c"OLDCWD".into(), old);
    }
    state.fds.insert(cwd_key, new_cwd);
}

#[cfg(test)]
mod tests;
