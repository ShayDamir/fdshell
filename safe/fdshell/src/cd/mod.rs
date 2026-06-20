use crate::error::cd::CdError;
use crate::state::ShellState;
use std::ffi::CString;
use sys::fcntl::{O_DIRECTORY, O_NOFOLLOW};
use sys::{LocalFd, ShortCStr};

pub fn cd(args: &[ShortCStr], state: &mut ShellState) -> Result<(), CdError> {
    let new_fd = match args.first() {
        None => cd_home()?,
        Some(arg) if arg.eq_bytes(b"-") => cd_var(&c"%OLDCWD".into(), state)?,
        Some(arg) if arg.starts_with(b"%") => cd_var(arg, state)?,
        Some(path) => cd_path(path)?,
    };
    sys::fchdir::fchdir(&new_fd).map_err(|_| CdError::CdPathOpen)?;
    move_cwd(state, new_fd);
    Ok(())
}

fn cd_home() -> Result<LocalFd, CdError> {
    let home = std::env::var_os("HOME").ok_or(CdError::HomeNotSet)?;
    let cs = CString::new(home.as_os_str().as_encoded_bytes()).map_err(|_| CdError::CdPathOpen)?;
    open_cwd_dir(&cs)
}

fn cd_var(arg: &ShortCStr, state: &ShellState) -> Result<LocalFd, CdError> {
    let name = arg.strip_prefix(b"%").ok_or(CdError::CdPathOpen)?;
    let src = state.fds.get(&name).ok_or(CdError::CdPathOpen)?;
    src.try_clone().map_err(|_| CdError::CdPathOpen)
}

fn cd_path(path: &ShortCStr) -> Result<LocalFd, CdError> {
    let name = sys::RefCStr::from(path.clone());
    open_cwd_dir(&name)
}

fn open_cwd_dir(path: &std::ffi::CStr) -> Result<LocalFd, CdError> {
    sys::openat2::open(path, O_DIRECTORY | O_NOFOLLOW).map_err(|_| CdError::CdPathOpen)
}

fn move_cwd(state: &mut ShellState, new_cwd: LocalFd) {
    if let Some(old) = state.fds.remove(&c"CWD".into()) {
        state.fds.insert(c"OLDCWD".into(), old);
    }
    state.fds.insert(c"CWD".into(), new_cwd);
}

#[cfg(test)]
mod tests;
